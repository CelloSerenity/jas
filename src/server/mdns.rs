//! mDNS discovery for `_remotepairing._tcp.local.`
//!
//! This task keeps a `ServiceDaemon` browsing in the background and writes
//! the advertised IP and port back to the device whenever a match is found.

use std::net::IpAddr;

use idevice::remote_pairing::{PeerDevice, RpPairingFile};
use mdns_sd::{ServiceDaemon, ServiceEvent};
use tracing::{debug, info, warn};

use crate::server::{db, state::AppState};

const SERVICE_TYPE: &str = "_remotepairing._tcp.local.";

pub async fn run_mdns(state: AppState) {
    if !state.config.discovery.mdns_enabled {
        info!("mDNS discovery disabled in config");
        return;
    }

    let daemon = match ServiceDaemon::new() {
        Ok(d) => d,
        Err(e) => {
            warn!("mDNS: failed to start ServiceDaemon: {e}");
            return;
        }
    };

    let iface = state.config.discovery.mdns_interface.trim();
    if !iface.is_empty() {
        if let Err(e) = daemon.enable_interface(iface) {
            warn!("mDNS: enable_interface({iface}) failed: {e}");
        }
    }

    let receiver = match daemon.browse(SERVICE_TYPE) {
        Ok(r) => r,
        Err(e) => {
            warn!("mDNS: browse({SERVICE_TYPE}) failed: {e}");
            return;
        }
    };

    info!("mDNS: browsing {SERVICE_TYPE}");

    while let Ok(event) = receiver.recv_async().await {
        match event {
            ServiceEvent::ServiceResolved(info) => {
                handle_resolved(&state, &info).await;
            }
            ServiceEvent::SearchStarted(_)
            | ServiceEvent::ServiceFound(_, _)
            | ServiceEvent::ServiceRemoved(_, _)
            | ServiceEvent::SearchStopped(_) => {}
            _ => {}
        }
    }

    warn!("mDNS: event channel closed, discovery task exiting");
}

async fn handle_resolved(state: &AppState, info: &mdns_sd::ResolvedService) {
    let identifier = match info.get_property_val_str("identifier") {
        Some(s) => s,
        None => return,
    };
    let auth_tag = match info.get_property_val_str("authTag") {
        Some(s) => s,
        None => return,
    };

    let ip = match pick_ip(info) {
        Some(ip) => ip,
        None => {
            debug!("mDNS: {} has no usable address", info.get_fullname());
            return;
        }
    };

    let devices = match db::list_devices(&state.db).await {
        Ok(d) => d,
        Err(e) => {
            warn!("mDNS: failed to list devices: {e}");
            return;
        }
    };

    for device in devices {
        let pairing_bytes = match state.crypto.decrypt(&device.pairing_blob) {
            Ok(b) => b,
            Err(e) => {
                debug!("mDNS: decrypt pairing for {} failed: {e}", device.id);
                continue;
            }
        };

        let rpf = match RpPairingFile::from_bytes(&pairing_bytes) {
            Ok(f) => f,
            Err(e) => {
                debug!("mDNS: parse pairing for {} failed: {e:?}", device.id);
                continue;
            }
        };

        let Some(alt_irk) = rpf.alt_irk() else {
            continue;
        };

        if !PeerDevice::validate_auth_tag(alt_irk, identifier, auth_tag) {
            continue;
        }

        let ip_str = ip.to_string();
        let port = i64::from(info.get_port());
        if port == 0 { return; }
        if device.mdns_ip.as_deref() == Some(ip_str.as_str()) && device.port == port {
            // Same IP as last time; still bump mdns_seen_at so the UI shows
            // recent presence.
            if let Err(e) = db::update_device_mdns_ip(&state.db, &device.id, &ip_str, port).await {
                warn!("mDNS: update for {} failed: {e}", device.id);
            }
        } else {
            info!(
                "mDNS: matched {} at {}:{}",
                device.name, ip_str, port
            );
            if let Err(e) = db::update_device_mdns_ip(&state.db, &device.id, &ip_str, port).await {
                warn!("mDNS: update for {} failed: {e}", device.id);
            }
        }
        return;
    }

    debug!("mDNS: no registered device matches identifier {identifier}");
}

fn pick_ip(info: &mdns_sd::ResolvedService) -> Option<IpAddr> {
    let addrs = info.get_addresses().iter().map(|addr| addr.to_ip_addr());
    addrs
        .clone()
        .find(|a| matches!(a, IpAddr::V4(_)))
        .or_else(|| {
            addrs
                .clone()
                .find(|a| matches!(a, IpAddr::V6(v6) if !v6.is_unicast_link_local()))
        })
        .or_else(|| addrs.clone().next())
}

pub async fn discover_port(
    device_ip: &str,
    pairing_bytes: &[u8],
    config: &crate::server::config::DiscoveryConfig,
) -> Option<u16> {
    if !config.mdns_enabled {
        return None;
    }
    let pairing = RpPairingFile::from_bytes(pairing_bytes).ok()?;
    let daemon = ServiceDaemon::new().ok()?;
    let iface = config.mdns_interface.trim();
    if !iface.is_empty() {
        if let Err(e) = daemon.enable_interface(iface) {
            warn!("mDNS: enable_interface({iface}) failed: {e}");
        }
    }
    let result = async {
        let receiver = daemon.browse(SERVICE_TYPE).ok()?;
        while let Ok(event) = receiver.recv_async().await {
            let ServiceEvent::ServiceResolved(info) = event else {
                continue;
            };
            if info.get_port() == 0 {
                continue;
            }
            let matches = if let Some(alt_irk) = pairing.alt_irk() {
                match (
                    info.get_property_val_str("identifier"),
                    info.get_property_val_str("authTag"),
                ) {
                    (Some(identifier), Some(auth_tag)) => {
                        PeerDevice::validate_auth_tag(alt_irk, identifier, auth_tag)
                    }
                    _ => false,
                }
            } else {
                device_ip.parse::<IpAddr>().is_ok_and(|ip| {
                    info.get_addresses()
                        .iter()
                        .any(|addr| addr.to_ip_addr() == ip)
                })
            };
            if matches {
                return Some(info.get_port());
            }
        }
        None
    };
    let port = tokio::time::timeout(std::time::Duration::from_secs(3), result)
        .await
        .ok()
        .flatten();
    let _ = daemon.stop_browse(SERVICE_TYPE);
    let _ = daemon.shutdown();
    port
}
