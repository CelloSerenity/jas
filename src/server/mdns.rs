//! mDNS discovery for `_remotepairing._tcp.local.`
//!
//! This task keeps a `ServiceDaemon` browsing in the background and writes
//! the LAN-resolved IP back to `devices.mdns_ip` whenever a match is found.

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
        }
    }

    warn!("mDNS: event channel closed, discovery task exiting");
}

async fn handle_resolved(state: &AppState, info: &mdns_sd::ServiceInfo) {
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
        if device.mdns_ip.as_deref() == Some(ip_str.as_str()) {
            // Same IP as last time; still bump mdns_seen_at so the UI shows
            // recent presence.
            if let Err(e) = db::update_device_mdns_ip(&state.db, &device.id, &ip_str).await {
                warn!("mDNS: update for {} failed: {e}", device.id);
            }
        } else {
            info!(
                "mDNS: matched {} ({}) at {}",
                device.name, device.udid, ip_str
            );
            if let Err(e) = db::update_device_mdns_ip(&state.db, &device.id, &ip_str).await {
                warn!("mDNS: update for {} failed: {e}", device.id);
            }
        }
        return;
    }

    debug!("mDNS: no registered device matches identifier {identifier}");
}

fn pick_ip(info: &mdns_sd::ServiceInfo) -> Option<IpAddr> {
    let addrs = info.get_addresses();
    addrs
        .iter()
        .find(|a| matches!(a, IpAddr::V4(_)))
        .copied()
        .or_else(|| {
            addrs
                .iter()
                .find(|a| matches!(a, IpAddr::V6(v6) if !v6.is_unicast_link_local()))
                .copied()
        })
        .or_else(|| addrs.iter().next().copied())
}
