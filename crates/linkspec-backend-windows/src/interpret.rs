//! Turns a [`WinDump`] into a [`Snapshot`]. Cross-platform and tested.
//!
//! Physical ports: hub ports with `PortConnectorIsTypeC` and
//! `PortIsUserConnectable`; USB 2 and USB 3 companions are merged into one
//! physical port. Without the service (tier 1), Windows does not expose
//! PD contracts, PDOs or cable E-Markers to user mode: these fields are
//! `unavailable` with reason `needs_service`.

use std::collections::{BTreeMap, BTreeSet};

use linkspec_core::observation::*;
use linkspec_core::pd::{FixedRdo, Pdo, PdoRole};
use linkspec_core::types::{Source, Transport, UsbGen};

use crate::ioctl::{ConnStatus, HubType};
use crate::model::{HubDump, HubPortDump, ServiceConnector, ServiceState, WinDump};

type PortKey = (String, u32);

fn speed_mbps(p: &HubPortDump) -> Option<f64> {
    let c = p.conn.as_ref()?;
    if !c.status.device_present() {
        return None;
    }
    if let Some(s) = p.ssp {
        return Some((s.rx_gbps.min(s.tx_gbps) * 1000.0).round());
    }
    if p.v2.is_some_and(|v| v.operating_at_ssp) {
        return Some(10000.0);
    }
    Some(match c.speed {
        0 => 1.5,
        1 => 12.0,
        2 => 480.0,
        3 => 5000.0,
        _ => return None,
    })
}

fn device(hubs: &BTreeMap<&str, &HubDump>, p: &HubPortDump, depth: u8) -> Option<UsbDeviceObservation> {
    let speed = speed_mbps(p)?;
    let c = p.conn.as_ref()?;
    let max_capable = p.bos_max.or_else(|| {
        let v = p.v2?;
        if v.ssp_capable {
            Some(UsbGen::Gen2)
        } else if v.ss_capable {
            Some(UsbGen::Gen1)
        } else {
            None
        }
    });
    let mut children = Vec::new();
    if depth < 6
        && let Some(h) = p.child_hub.as_deref().and_then(|k| hubs.get(k))
    {
        for cp in &h.ports {
            if let Some(d) = device(hubs, cp, depth + 1) {
                children.push(d);
            }
        }
    }
    Some(UsbDeviceObservation {
        vendor_id: Some(c.vendor_id),
        product_id: Some(c.product_id),
        vendor: p.manufacturer.clone(),
        product: p.product_string.clone().or_else(|| p.description.clone()),
        speed_mbps: speed,
        bcd_usb: Some(c.bcd_usb),
        max_capable,
        is_hub: c.is_hub,
        children,
        source: Source::UsbIoctl,
    })
}

fn sanitize(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

pub fn interpret(dump: &WinDump) -> Snapshot {
    let hubs: BTreeMap<&str, &HubDump> = dump.hubs.iter().map(|h| (h.key.as_str(), h)).collect();

    // Which hub hangs on which port (to know internal hubs).
    let mut parent: BTreeMap<&str, (&str, &HubPortDump)> = BTreeMap::new();
    for h in &dump.hubs {
        for p in &h.ports {
            if let Some(c) = p.child_hub.as_deref() {
                parent.insert(c, (h.key.as_str(), p));
            }
        }
    }
    // A hub is internal when it hangs on a port that the user cannot reach.
    let internal = |hub: &str| -> bool {
        parent.get(hub).is_some_and(|(_, p)| {
            p.connector
                .as_ref()
                .is_some_and(|c| !c.user_connectable)
        })
    };

    // Candidate Type-C ports, merged with companions.
    let mut groups: Vec<BTreeSet<PortKey>> = Vec::new();
    for h in &dump.hubs {
        let eligible = h.info.hub_type == HubType::Root || internal(&h.key);
        if !eligible {
            continue;
        }
        for p in &h.ports {
            let Some(c) = &p.connector else { continue };
            if !(c.type_c && c.user_connectable) {
                continue;
            }
            let me: PortKey = (h.key.clone(), p.port);
            let comp = p
                .companion_hub
                .clone()
                .filter(|_| c.companion_port != 0)
                .map(|k| (k, u32::from(c.companion_port)));
            let existing = groups
                .iter()
                .position(|g| g.contains(&me) || comp.as_ref().is_some_and(|k| g.contains(k)));
            match existing {
                Some(i) => {
                    groups[i].insert(me);
                    if let Some(k) = comp {
                        groups[i].insert(k);
                    }
                }
                None => {
                    let mut g = BTreeSet::new();
                    g.insert(me);
                    if let Some(k) = comp {
                        g.insert(k);
                    }
                    groups.push(g);
                }
            }
        }
    }

    let port_of = |k: &PortKey| -> Option<&HubPortDump> {
        hubs.get(k.0.as_str())?.ports.iter().find(|p| p.port == k.1)
    };
    let hub_is_usb3 = |key: &str| {
        hubs.get(key).is_some_and(|h| {
            h.info.hub_type == HubType::Usb30
                || h.ports.iter().any(|p| p.v2.is_some_and(|v| v.usb300))
                || h.ports
                    .iter()
                    .any(|p| p.conn.as_ref().is_some_and(|c| c.speed >= 3))
        })
    };

    // Order: by (location of the hub, port number) of the first member.
    let mut ordered: Vec<(String, BTreeSet<PortKey>)> = groups
        .into_iter()
        .map(|g| {
            let first = g.iter().next().cloned().unwrap_or_default();
            let loc = hubs
                .get(first.0.as_str())
                .and_then(|h| h.location.clone())
                .unwrap_or_else(|| first.0.clone());
            (format!("{loc}#USB({:02})", first.1), g)
        })
        .collect();
    ordered.sort_by(|a, b| a.0.cmp(&b.0));

    let service = match &dump.service {
        ServiceState::Running(d) => Some(d),
        _ => None,
    };

    let mut ports = Vec::new();
    for (i, (loc, group)) in ordered.iter().enumerate() {
        let index = i as u32 + 1;
        let mut p = PortObservation::new(format!("win-{}", sanitize(loc)), index);
        let members: Vec<(&PortKey, &HubPortDump)> = group
            .iter()
            .filter_map(|k| port_of(k).map(|p| (k, p)))
            .collect();
        p.behind_internal_hub = group.iter().any(|k| internal(&k.0));
        let has_ss = group.iter().any(|k| hub_is_usb3(&k.0));
        let ssp_seen = members
            .iter()
            .any(|(_, m)| m.v2.is_some_and(|v| v.operating_at_ssp));
        p.capabilities = PortCapabilities {
            usb: Some(if ssp_seen {
                UsbGen::Gen2
            } else if has_ss {
                UsbGen::Gen1
            } else {
                UsbGen::Usb2
            }),
            // The hub interfaces do not report the highest port speed.
            usb_is_lower_bound: true,
            source: Some(Source::UsbIoctl),
            ..Default::default()
        };
        let mut overcurrent = false;
        for (_, m) in &members {
            if m.conn.as_ref().is_some_and(|c| c.status == ConnStatus::Overcurrent) {
                overcurrent = true;
            }
            if let Some(d) = device(&hubs, m, 0) {
                p.link.usb_devices.push(d);
            }
        }
        p.link.usb_devices.sort_by(|a, b| b.speed_mbps.total_cmp(&a.speed_mbps));
        p.link.source = Some(Source::UsbIoctl);
        p.health = Some(HealthObservation {
            overcurrent_count: Some(u32::from(overcurrent)),
            ..Default::default()
        });
        // Tier 1 sees only USB devices. No device does not mean "empty".
        p.connected = if p.link.usb_devices.is_empty() { None } else { Some(true) };
        if p.behind_internal_hub {
            p.cable = CableObservation::Unavailable {
                reason: Reason::BehindInternalHub,
            };
            p.power = PowerObservation::Unavailable {
                reason: Reason::BehindInternalHub,
            };
        } else if let Some(sc) = service.and_then(|s| s.connectors.iter().find(|c| c.index == index)) {
            apply_service(&mut p, sc);
        } else {
            let reason = if service.is_some() {
                Reason::FirmwareNotReported
            } else {
                Reason::NeedsService
            };
            if p.connected == Some(true) {
                p.cable = CableObservation::Unavailable { reason };
                p.power = PowerObservation::Unavailable { reason };
            }
        }
        for (k, _) in &members {
            p.raw.insert(
                format!("member_{}_{}", k.0, k.1),
                serde_json::Value::String(if hub_is_usb3(&k.0) { "usb3" } else { "usb2" }.into()),
            );
        }
        ports.push(p);
    }

    // USB4 (H-W3): only map when there is exactly one Type-C port.
    let device_routers: Vec<_> = dump.usb4_routers.iter().filter(|r| r.kind == "device").collect();
    if !device_routers.is_empty() {
        if ports.len() == 1 {
            let p = &mut ports[0];
            p.kind = PortKind::Usb4;
            p.connected = Some(true);
            p.link.transports.push(Transport::Usb4);
            p.link.fabric = Some(FabricLinkObservation {
                generation: None,
                lanes: None,
                rx_gbps: None,
                tx_gbps: None,
                measured_by_controller: false,
                switches: device_routers
                    .iter()
                    .map(|r| FabricSwitch {
                        depth: 1,
                        vendor: None,
                        name: r.description.clone(),
                        generation: None,
                        link_gbps: None,
                        lanes: None,
                    })
                    .collect(),
                source: Source::Setupapi,
            });
        } else {
            for p in &mut ports {
                p.mark_unavailable("link.fabric", Reason::PlatformNotExposed);
            }
        }
    }

    // Displays (H-W4): DisplayPort or USB tunnel outputs; assign only when
    // exactly one port can carry them.
    let candidates: Vec<_> = dump.displays.iter().filter(|d| d.is_usb_c_candidate()).collect();
    if !candidates.is_empty() {
        let connected: Vec<usize> = ports
            .iter()
            .enumerate()
            .filter(|(_, p)| p.connected != Some(false))
            .map(|(i, _)| i)
            .collect();
        if connected.len() == 1 {
            let p = &mut ports[connected[0]];
            for d in candidates {
                if !p.link.transports.contains(&Transport::Displayport) {
                    p.link.transports.push(Transport::Displayport);
                }
                p.displays.push(DisplayObservation {
                    name: d.edid.as_ref().and_then(|e| e.name.clone()),
                    current: d.current,
                    max: d.edid.as_ref().and_then(|e| e.max).map(|m| DisplayMode {
                        width: m.width,
                        height: m.height,
                        refresh_hz: (f64::from(m.refresh_mhz) / 1000.0).round(),
                        bits_per_color: d.edid.as_ref().and_then(|e| e.bits_per_color),
                    }),
                    dsc: None,
                    adapter: None,
                    source: Source::Displayconfig,
                });
                if d.tunneled() {
                    p.link.displayport = Some(DpLinkObservation {
                        lanes: None,
                        lane_gbps: None,
                        pin_assignment: None,
                        tunneled: true,
                        source: Source::Displayconfig,
                    });
                }
            }
        }
    }

    let battery = dump.batteries.first().map(|b| {
        let percent = match (b.capacity_mwh, b.full_mwh) {
            (Some(c), Some(f)) if f > 0 => Some((f64::from(c) / f64::from(f) * 100.0).round().min(100.0)),
            _ => None,
        };
        BatteryObservation {
            percent,
            charging: Some(b.power_state & 4 != 0),
            full: percent.map(|p| p >= 100.0 && b.power_state & 4 == 0),
            external_power: Some(b.power_state & 1 != 0),
            power_w: b.rate_mw.map(|r| f64::from(r) / 1000.0),
            source: Source::BatteryIoctl,
        }
    });

    let mut notices = Vec::new();
    let tier = match &dump.service {
        ServiceState::Running(_) => Tier::Service,
        _ => {
            notices.push(Notice::ServiceNotInstalled);
            Tier::User
        }
    };

    Snapshot {
        host: HostInfo {
            os: Os::Windows,
            os_version: dump.host.os_version.clone(),
            model: dump.host.model.clone(),
            arch: if dump.host.arch.is_empty() {
                std::env::consts::ARCH.to_string()
            } else {
                dump.host.arch.clone()
            },
        },
        tier,
        usb_probe: true,
        battery,
        ports,
        notices,
    }
}

/// Tier 2: data from the service (ETW). Connector N ↔ port N (H-W5).
fn apply_service(p: &mut PortObservation, sc: &ServiceConnector) {
    if let Some(c) = sc.connected {
        p.connected = Some(c);
    }
    p.partner = Some(PartnerObservation {
        identity: sc.partner_identity.clone(),
        supports_pd: Some(!sc.source_pdos.is_empty() || sc.partner_identity.is_some()),
        accessory: None,
        name: None,
        source: Source::Service,
    });
    p.cable = match &sc.cable_identity {
        Some(id) if !id.is_empty() => CableObservation::Emarker {
            identity: id.clone(),
            far_end: None,
            source: Source::Service,
        },
        _ => CableObservation::Unavailable {
            reason: Reason::FirmwareNotReported,
        },
    };
    let role = match sc.power_role.as_deref() {
        Some("source") => Some(PowerRole::Source),
        Some("sink") => Some(PowerRole::Sink),
        _ => None,
    };
    let mut pd = PowerData {
        role,
        source_pdos: sc.source_pdos.iter().map(|r| Pdo::decode(*r, PdoRole::Source)).collect(),
        sink_pdos: sc.sink_pdos.iter().map(|r| Pdo::decode(*r, PdoRole::Sink)).collect(),
        source: Source::Service,
        ..Default::default()
    };
    if let Some(rdo) = sc.rdo.map(FixedRdo::decode) {
        let idx = usize::from(rdo.object_position);
        if let Some(Pdo::Fixed { voltage_v, .. }) = idx.checked_sub(1).and_then(|i| pd.source_pdos.get(i)) {
            pd.contract = Some(Contract {
                pdo_index: Some(rdo.object_position),
                voltage_v: *voltage_v,
                current_a: rdo.operating_current_a,
                power_w: ((voltage_v * rdo.operating_current_a) * 100.0).round() / 100.0,
            });
        }
    }
    p.power = if pd.source_pdos.is_empty() && pd.contract.is_none() && role.is_none() {
        PowerObservation::Unavailable {
            reason: Reason::FirmwareNotReported,
        }
    } else {
        PowerObservation::Available(Box::new(pd))
    };
}
