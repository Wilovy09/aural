//! Aural Connect: one Aural plays and others control it, the way Spotify Connect works.
//! Every Aural is a host others can find on the network ([`discovery`]) and control
//! ([`host`]); any of them can pick another to play on ([`link`]). A controller sends the
//! songs and the commands; the host fetches and plays them with its own engine and tells
//! every controller what plays.

mod discovery;
mod host;
mod link;
pub mod protocol;
mod trust;

use tokio::sync::mpsc::UnboundedSender;

pub use host::{command as host_command, modes, observe};
pub use link::{command, disconnect, pair};
pub use protocol::{Remote, Snapshot};
pub use trust::me;

use crate::library::Song;
use crate::runtime;

/// The port a host listens on, when it is free.
const PORT: u16 = 47821;

/// Another Aural on the network.
#[derive(Clone, Debug, PartialEq)]
pub struct Device {
    pub id: String,
    pub name: String,
    /// Where it listens, `ip:port`.
    pub address: String,
}

/// Where this device's control of another stands.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Link {
    /// Playing here.
    #[default]
    Idle,
    Connecting(String),
    /// The host shows a code, waiting for it here.
    NeedCode(String),
    /// Playing on the named device.
    Connected(String),
    Failed(String),
}

/// A code this device shows, for a controller asking to pair.
#[derive(Clone, Debug, PartialEq)]
pub struct Pairing {
    pub code: String,
    /// The controller's name.
    pub from: String,
}

/// What Connect tells the app.
#[derive(Debug)]
pub enum Event {
    /// The devices found on the network.
    Devices(Vec<Device>),
    Link(Link),
    /// What plays on the host this device controls.
    State(Snapshot),
    Queue(Vec<Song>, usize),
    /// A code to show, or none to hide it.
    Pairing(Option<Pairing>),
    /// A controller's command for this device's player.
    Remote(Remote),
}

pub type Events = UnboundedSender<Event>;

/// Starts hosting and looking for other devices.
pub fn start(events: Events) {
    #[cfg(target_os = "android")]
    if let Err(error) = crate::platform::multicast() {
        log::warn!("connect: no multicast lock, other devices may not show: {error:#}");
    }
    runtime::spawn(async move {
        let port = match host::serve(PORT, events.clone()).await {
            Ok(port) => port,
            Err(error) => {
                log::warn!("connect: cannot host: {error:#}");
                return;
            }
        };
        log::info!("connect: {} hosts on port {port}", me().name);
        if let Err(error) = discovery::run(port, events).await {
            log::warn!("connect: cannot look for devices: {error:#}");
        }
    });
}

/// Connects to `device` to play there.
pub fn connect(device: Device, events: Events) {
    link::connect(device, events);
}

/// A random number, for codes and tokens: the standard library's per-process hashing keys,
/// mixed with the clock.
pub(crate) fn random() -> u64 {
    use std::hash::{BuildHasher as _, Hasher as _};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    );
    hasher.finish()
}
