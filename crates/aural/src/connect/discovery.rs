//! Finding the other Aurals on the network: each one announces itself over mDNS as
//! `_aural._tcp` with its id and name, and lists the others it hears.

use std::collections::HashMap;

use anyhow::Result;
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};

use super::{Device, Event, Events, trust};

const SERVICE: &str = "_aural._tcp.local.";

/// Announces this device on `port` and reports the devices heard to `events`. Runs until the
/// process ends.
pub async fn run(port: u16, events: Events) -> Result<()> {
    let daemon = ServiceDaemon::new()?;
    let me = trust::me();
    let host = format!("aural-{}.local.", &me.id[..8.min(me.id.len())]);
    let instance = format!("{} {}", me.name, &me.id[..4.min(me.id.len())]);
    let properties = [("id", me.id.as_str()), ("name", me.name.as_str())];
    let info = ServiceInfo::new(SERVICE, &instance, &host, "", port, &properties[..])?
        .enable_addr_auto();
    daemon.register(info)?;

    let heard = daemon.browse(SERVICE)?;
    // Devices by their mDNS full name, which is what a removal names.
    let mut devices: HashMap<String, Device> = HashMap::new();
    while let Ok(event) = heard.recv_async().await {
        match event {
            ServiceEvent::ServiceResolved(found) => {
                let id = found
                    .txt_properties
                    .get_property_val_str("id")
                    .unwrap_or_default()
                    .to_owned();
                if id.is_empty() || id == me.id {
                    continue;
                }
                let Some(ip) = found.get_addresses_v4().into_iter().next() else {
                    continue;
                };
                let name = found
                    .txt_properties
                    .get_property_val_str("name")
                    .unwrap_or("Aural")
                    .to_owned();
                devices.insert(
                    found.fullname.clone(),
                    Device {
                        id,
                        name,
                        address: format!("{ip}:{}", found.port),
                    },
                );
            }
            ServiceEvent::ServiceRemoved(_, fullname) => {
                devices.remove(&fullname);
            }
            _ => continue,
        }
        let mut list: Vec<Device> = devices.values().cloned().collect();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        list.dedup_by(|a, b| a.id == b.id);
        let _ = events.send(Event::Devices(list));
    }
    Ok(())
}
