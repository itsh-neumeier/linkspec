//! Session tracking: faults over time and banner state (prompt 5.5).
//!
//! A drop/reconnect is a USB data link that disappears and comes back while
//! the Type-C partner stays connected. Unplugging the cable removes the
//! partner too, so it does not count as a drop. Overcurrent comes from port
//! counters that increase.
//!
//! Banners stay until the user dismisses them or the cable is pulled.

use std::collections::BTreeMap;

use crate::observation::{FaultKind, FaultObservation, Snapshot};

#[derive(Debug, Clone, Default)]
struct PortState {
    partner_present: bool,
    data_present: bool,
    /// Data went away while the partner stayed.
    data_lost: bool,
    overcurrent_count: Option<u32>,
    banners: BTreeMap<FaultKind, FaultObservation>,
}

/// A new fault that the caller should record (history, notification).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaultEvent {
    pub port_id: String,
    pub kind: FaultKind,
    pub at: String,
}

#[derive(Debug, Default)]
pub struct SessionTracker {
    ports: BTreeMap<String, PortState>,
}

impl SessionTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed a new snapshot. Returns new fault events and writes the active
    /// banners into `snap.ports[].faults`.
    pub fn update(&mut self, snap: &mut Snapshot, now: &str) -> Vec<FaultEvent> {
        let mut events = Vec::new();
        for port in &mut snap.ports {
            let st = self.ports.entry(port.id.clone()).or_default();
            let partner = port.connected == Some(true) || port.partner.is_some();
            let data = !port.link.usb_devices.is_empty() || port.link.fabric.is_some();

            if !partner && port.connected == Some(false) {
                // Cable pulled: clear banners.
                st.banners.clear();
                st.data_lost = false;
            } else if st.partner_present && partner {
                if st.data_present && !data {
                    st.data_lost = true;
                } else if st.data_lost && data {
                    st.data_lost = false;
                    raise(st, &port.id, FaultKind::DropReconnect, now, &mut events);
                }
            }

            let oc = port.health.as_ref().and_then(|h| h.overcurrent_count);
            if let (Some(prev), Some(cur)) = (st.overcurrent_count, oc)
                && cur > prev
            {
                raise(st, &port.id, FaultKind::Overcurrent, now, &mut events);
            }
            if oc.is_some() {
                st.overcurrent_count = oc;
            }
            // Faults reported directly by the backend (event based).
            for f in std::mem::take(&mut port.faults) {
                let entry = st.banners.entry(f.kind).or_insert(FaultObservation {
                    kind: f.kind,
                    count: 0,
                    last_at: None,
                });
                entry.count += f.count.max(1);
                entry.last_at = f.last_at.clone().or_else(|| Some(now.to_string()));
                events.push(FaultEvent {
                    port_id: port.id.clone(),
                    kind: f.kind,
                    at: now.to_string(),
                });
            }

            st.partner_present = partner;
            st.data_present = data;
            port.faults = st.banners.values().cloned().collect();
        }
        events
    }

    /// The user closed a banner.
    pub fn dismiss(&mut self, port_id: &str, kind: FaultKind) {
        if let Some(st) = self.ports.get_mut(port_id) {
            st.banners.remove(&kind);
        }
    }
}

fn raise(
    st: &mut PortState,
    port_id: &str,
    kind: FaultKind,
    now: &str,
    events: &mut Vec<FaultEvent>,
) {
    let entry = st.banners.entry(kind).or_insert(FaultObservation {
        kind,
        count: 0,
        last_at: None,
    });
    entry.count += 1;
    entry.last_at = Some(now.to_string());
    events.push(FaultEvent {
        port_id: port_id.to_string(),
        kind,
        at: now.to_string(),
    });
}
