//! Maps decoded ETW event properties to per-connector state (H-W2).
//!
//! The Type-C stack (UcmCx, UcmUcsiCx, UCSI ACPI client) publishes
//! manifest-based ETW events. Their property names are not documented as a
//! contract. This mapper matches names by pattern and ignores everything it
//! does not understand. The raw events are kept (ring buffer) so that a
//! capture can show what the stack really sends.

use std::collections::{BTreeMap, VecDeque};

use linkspec_backend_windows::model::{ServiceConnector, ServiceData};
use linkspec_core::pd::RawIdentity;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum PropValue {
    Int(u64),
    Ints(Vec<u64>),
    Text(String),
}

impl PropValue {
    fn ints(&self) -> Vec<u32> {
        match self {
            PropValue::Int(v) => vec![*v as u32],
            PropValue::Ints(v) => v.iter().map(|x| *x as u32).collect(),
            PropValue::Text(_) => vec![],
        }
    }
    fn int(&self) -> Option<u64> {
        match self {
            PropValue::Int(v) => Some(*v),
            PropValue::Ints(v) => v.first().copied(),
            PropValue::Text(t) => t.parse().ok(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RawEvent {
    pub provider: String,
    pub event_id: u16,
    pub props: Vec<(String, PropValue)>,
}

#[derive(Default)]
pub struct State {
    pub connectors: BTreeMap<u32, ServiceConnector>,
    pub providers: Vec<String>,
    pub recent: VecDeque<RawEvent>,
}

const RECENT_MAX: usize = 200;

fn lname(s: &str) -> String {
    s.to_ascii_lowercase().replace(['_', ' '], "")
}

impl State {
    pub fn apply(&mut self, ev: RawEvent) {
        let connector = ev.props.iter().find_map(|(k, v)| {
            let k = lname(k);
            (k.contains("connector")
                && (k.contains("index") || k.contains("number") || k.contains("id")))
            .then(|| v.int())
            .flatten()
        });
        if let Some(idx) = connector {
            // UCSI connector numbers are 1-based; some events use 0-based
            // indexes ("ConnectorIndex"). Normalize on the name.
            let zero_based = ev
                .props
                .iter()
                .any(|(k, _)| lname(k).contains("connectorindex"));
            let index = if zero_based {
                idx as u32 + 1
            } else {
                idx as u32
            };
            let c = self
                .connectors
                .entry(index)
                .or_insert_with(|| ServiceConnector {
                    index,
                    ..Default::default()
                });
            let mut cable: Vec<(String, u32)> = vec![];
            let mut partner: Vec<(String, u32)> = vec![];
            for (k, v) in &ev.props {
                let n = lname(k);
                if n.contains("sourcepdo") || n.contains("sourcecap") {
                    c.source_pdos = v.ints();
                } else if n.contains("sinkpdo") || n.contains("sinkcap") {
                    c.sink_pdos = v.ints();
                } else if n == "rdo" || n.contains("requestdataobject") {
                    c.rdo = v.int().map(|x| x as u32);
                } else if n.contains("connectstatus")
                    || n == "connected"
                    || n.contains("partnerconnected")
                {
                    c.connected = v.int().map(|x| x != 0);
                } else if n.contains("powerrole") || n.contains("powerdirection") {
                    c.power_role = v.int().map(|x| {
                        if x == 0 {
                            "sink".into()
                        } else {
                            "source".into()
                        }
                    });
                } else if n.contains("idheader")
                    || n.contains("certstat")
                    || n.contains("productvdo")
                    || n.contains("producttypevdo")
                    || n.contains("cablevdo")
                {
                    let target = if n.contains("cable") || n.contains("sopp") || n.contains("sop'")
                    {
                        &mut cable
                    } else {
                        &mut partner
                    };
                    for x in v.ints() {
                        target.push((n.clone(), x));
                    }
                }
            }
            if let Some(id) = identity(&cable) {
                c.cable_identity = Some(id);
            }
            if let Some(id) = identity(&partner) {
                c.partner_identity = Some(id);
            }
        }
        self.recent.push_back(ev);
        while self.recent.len() > RECENT_MAX {
            self.recent.pop_front();
        }
    }

    pub fn data(&self) -> ServiceData {
        ServiceData {
            version: env!("CARGO_PKG_VERSION").to_string(),
            connectors: self.connectors.values().cloned().collect(),
            providers: self.providers.clone(),
        }
    }
}

fn identity(fields: &[(String, u32)]) -> Option<RawIdentity> {
    let get = |p: &str| fields.iter().find(|(k, _)| k.contains(p)).map(|(_, v)| *v);
    let id_header = get("idheader")?;
    if id_header == 0 {
        return None;
    }
    let vdos: Vec<u32> = fields
        .iter()
        .filter(|(k, _)| k.contains("producttypevdo") || k.contains("cablevdo"))
        .map(|(_, v)| *v)
        .collect();
    Some(RawIdentity {
        id_header,
        cert_stat: get("certstat").unwrap_or(0),
        product: get("productvdo").filter(|_| true).unwrap_or(0),
        product_type_vdos: vdos,
    })
}

/// Provider names the service subscribes to (matched case-insensitively
/// against `TdhEnumerateProviders`). Hypothesis H-W2: names verified on
/// hardware are listed in `docs/capability-matrix.md`.
pub const PROVIDER_PATTERNS: &[&str] = &["ucmucsi", "ucmcx", "ucsi", "usb-ucm", "typec"];

pub fn provider_matches(name: &str) -> bool {
    let n = lname(name);
    PROVIDER_PATTERNS.iter().any(|p| n.contains(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_pdos_and_cable_identity() {
        let mut s = State::default();
        s.apply(RawEvent {
            provider: "Microsoft-Windows-USB-UCMUCSICX".into(),
            event_id: 1,
            props: vec![
                ("ConnectorIndex".into(), PropValue::Int(0)),
                (
                    "SourcePdos".into(),
                    PropValue::Ints(vec![0x0001_912c, 0x0002_d12c]),
                ),
                ("Rdo".into(), PropValue::Int(0x2004_b12c)),
                ("CableIdHeader".into(), PropValue::Int(0x1800_1234)),
                ("CableProductVdo".into(), PropValue::Int(0x5678_0001)),
                ("CableVdo".into(), PropValue::Int(0x0000_2042)),
            ],
        });
        let d = s.data();
        assert_eq!(d.connectors.len(), 1);
        let c = &d.connectors[0];
        assert_eq!(c.index, 1);
        assert_eq!(c.source_pdos.len(), 2);
        assert_eq!(c.cable_identity.as_ref().unwrap().id_header, 0x1800_1234);
    }

    #[test]
    fn ignores_unknown_events() {
        let mut s = State::default();
        s.apply(RawEvent {
            provider: "x".into(),
            event_id: 9,
            props: vec![("Foo".into(), PropValue::Int(1))],
        });
        assert!(s.data().connectors.is_empty());
        assert_eq!(s.recent.len(), 1);
    }

    #[test]
    fn provider_patterns() {
        assert!(provider_matches("Microsoft-Windows-USB-UCMUCSICX"));
        assert!(provider_matches("Microsoft-Windows-USB-UCMCX"));
        assert!(!provider_matches("Microsoft-Windows-Kernel-Power"));
    }
}
