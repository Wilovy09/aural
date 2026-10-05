//! The controller side: one connection to the Aural this one plays on.

use std::sync::Mutex;

use anyhow::{Context as _, Result};
use futures_util::{SinkExt as _, StreamExt as _};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tokio_tungstenite::tungstenite::Message;

use super::protocol::{Remote, ToController, ToHost};
use super::{Device, Event, Events, Link, trust};
use crate::runtime;

/// The way out to the host, while connected or connecting.
static OUT: Mutex<Option<UnboundedSender<ToHost>>> = Mutex::new(None);

/// Connects to `device`, dropping any connection before it. What happens goes to `events`.
pub fn connect(device: Device, events: Events) {
    let (out, mut outgoing) = unbounded_channel::<ToHost>();
    if let Ok(mut held) = OUT.lock() {
        *held = Some(out);
    }
    let _ = events.send(Event::Link(Link::Connecting(device.name.clone())));
    runtime::spawn(async move {
        let result = run(&device, &events, &mut outgoing).await;
        // A newer connection replaced this one: it reports for itself.
        if outgoing.is_closed() {
            return;
        }
        if let Ok(mut held) = OUT.lock() {
            *held = None;
        }
        let _ = events.send(Event::Link(match result {
            Ok(()) => Link::Idle,
            Err(error) => {
                log::info!("connect: link to {} ended: {error:#}", device.name);
                Link::Failed(format!("se perdió {}", device.name))
            }
        }));
    });
}

async fn run(
    device: &Device,
    events: &Events,
    outgoing: &mut tokio::sync::mpsc::UnboundedReceiver<ToHost>,
) -> Result<()> {
    let url = format!("ws://{}", device.address);
    let (mut socket, _) = tokio::time::timeout(
        std::time::Duration::from_secs(8),
        tokio_tungstenite::connect_async(url.as_str()),
    )
    .await
    .context("no answer")?
    .context("cannot connect")?;
    let me = trust::me();
    let hello = ToHost::Hello {
        id: me.id.clone(),
        name: me.name.clone(),
        token: trust::host_token(&device.id),
    };
    socket
        .send(Message::text(serde_json::to_string(&hello)?))
        .await?;

    loop {
        tokio::select! {
            incoming = socket.next() => {
                let text = match incoming {
                    None | Some(Ok(Message::Close(_))) => return Ok(()),
                    Some(Ok(Message::Text(text))) => text,
                    Some(Ok(_)) => continue,
                    Some(Err(error)) => return Err(error.into()),
                };
                match serde_json::from_str::<ToController>(&text)? {
                    ToController::Welcome { name, .. } => {
                        let _ = events.send(Event::Link(Link::Connected(name)));
                    }
                    ToController::NeedCode => {
                        let _ = events.send(Event::Link(Link::NeedCode(device.name.clone())));
                    }
                    ToController::Paired { token } => trust::keep_host(&device.id, &token),
                    ToController::Denied { reason } => anyhow::bail!("{reason}"),
                    ToController::State { state } => {
                        let _ = events.send(Event::State(state));
                    }
                    ToController::Queue { queue, index } => {
                        let _ = events.send(Event::Queue(queue, index));
                    }
                }
            }
            sending = outgoing.recv() => match sending {
                Some(message) => {
                    socket.send(Message::text(serde_json::to_string(&message)?)).await?;
                }
                // Disconnected on purpose.
                None => {
                    let _ = socket.close(None).await;
                    return Ok(());
                }
            },
        }
    }
}

fn send(message: ToHost) {
    if let Ok(held) = OUT.lock()
        && let Some(out) = held.as_ref()
    {
        let _ = out.send(message);
    }
}

/// Sends the code shown on the host's screen.
pub fn pair(code: &str) {
    send(ToHost::Pair {
        code: code.trim().to_owned(),
    });
}

/// Asks the host's player to do `remote`.
pub fn command(remote: Remote) {
    send(ToHost::Command { remote });
}

/// Leaves the host.
pub fn disconnect() {
    if let Ok(mut held) = OUT.lock() {
        *held = None;
    }
}
