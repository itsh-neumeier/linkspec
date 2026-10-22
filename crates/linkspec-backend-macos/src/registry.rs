//! IOKit registry image and its interpretation.
//!
//! The collector ([`crate::collect`], macOS only) copies allow-listed
//! registry entries with their properties into a [`RegistryDump`]. The
//! interpreter here turns a dump into a [`Snapshot`] and runs on every
//! platform, so captures from real Macs become fixtures.
//!
//! **Hypotheses.** Apple documents the IOKit APIs, but not the class names
//! and property keys of the Apple silicon USB-C/Thunderbolt drivers. The
//! names below come from public `ioreg` outputs and must be verified against
//! our own captures (`linkspec --capture`). Each hypothesis has an id
//! (H-M1 …) that `docs/capability-matrix.md` lists with its check status.
//! Public, documented items: IOKit registry API (Apple IOKit reference),
//! `IOUSBHostDevice` properties and the `kUSBHostPortSpeed*` values
//! (IOUSBHostFamilyDefinitions.h in the macOS SDK), CoreGraphics display
//! modes (Quartz Display Services reference).

use std::collections::{BTreeMap, BTreeSet};

use linkspec_core::observation::*;
use linkspec_core::pd::RawIdentity;
use linkspec_core::types::{Source, Transport, UsbGen};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RegistryDump {
    pub entries: Vec<RegEntry>,
    #[serde(default)]
    pub displays: Vec<CgDisplay>,
    #[serde(default)]
    pub host: MacHost,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegEntry {
    /// Sequential id (anonymized registry entry id).
    pub id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<u64>,
    pub class: String,
    pub name: String,
    /// Properties (only for allow-listed classes; ancestors carry none).
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub props: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CgDisplay {
    pub builtin: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current: Option<DisplayMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<DisplayMode>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MacHost {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os_version: Option<String>,
    #[serde(default)]
    pub arch: String,
}

/// Class name prefixes and fragments the collector keeps (with properties).
pub const CLASS_FILTER: &[&str] = &[
    "AppleHPMInterface",
    "AppleHPMDevice",
    "IOPort",
    "AppleTypeC",
    "AppleSmartBattery",
    "IOUSBHostDevice",
    "XHCI",
    "IOThunderbolt",
    "PowerSource",
    "AppleTCController",
];

pub fn class_matches(class: &str) -> bool {
    CLASS_FILTER.iter().any(|f| class.contains(f))
}

struct Reg<'a> {
    by_id: BTreeMap<u64, &'a RegEntry>,
    children: BTreeMap<u64, Vec<u64>>,
}

impl<'a> Reg<'a> {
    fn new(d: &'a RegistryDump) -> Self {
        let mut by_id = BTreeMap::new();
        let mut children: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
        for e in &d.entries {
            by_id.insert(e.id, e);
            if let Some(p) = e.parent {
                children.entry(p).or_default().push(e.id);
            }
        }
        Self { by_id, children }
    }

    fn ancestors(&self, id: u64) -> Vec<&'a RegEntry> {
        let mut out = Vec::new();
        let mut cur = self.by_id.get(&id).and_then(|e| e.parent);
        let mut guard = 0;
        while let Some(p) = cur {
            guard += 1;
            if guard > 64 {
                break;
            }
            let Some(e) = self.by_id.get(&p) else { break };
            out.push(*e);
            cur = e.parent;
        }
        out
    }

    fn descendants(&self, id: u64) -> Vec<&'a RegEntry> {
        let mut out = Vec::new();
        let mut stack = vec![id];
        let mut seen = BTreeSet::new();
        while let Some(i) = stack.pop() {
            if !seen.insert(i) {
                continue;
            }
            for c in self.children.get(&i).into_iter().flatten() {
                if let Some(e) = self.by_id.get(c) {
                    out.push(*e);
                }
                stack.push(*c);
            }
        }
        out
    }
}

fn num(props: &Map<String, Value>, keys: &[&str]) -> Option<f64> {
    keys.iter()
        .find_map(|k| props.get(*k).and_then(Value::as_f64))
}

fn boolean(props: &Map<String, Value>, keys: &[&str]) -> Option<bool> {
    keys.iter().find_map(|k| match props.get(*k)? {
        Value::Bool(b) => Some(*b),
        Value::Number(n) => n.as_i64().map(|v| v != 0),
        _ => None,
    })
}

fn string<'a>(props: &'a Map<String, Value>, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|k| props.get(*k).and_then(Value::as_str))
}

fn strings(props: &Map<String, Value>, key: &str) -> Vec<String> {
    props
        .get(key)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Parse a VDO list from a number array or a hex data blob (little endian).
fn vdos(v: &Value) -> Option<Vec<u32>> {
    match v {
        Value::Array(a) => {
            let out: Vec<u32> = a
                .iter()
                .filter_map(|x| x.as_u64().map(|n| n as u32))
                .collect();
            (out.len() == a.len() && !out.is_empty()).then_some(out)
        }
        Value::String(s) if s.len() % 8 == 0 && s.chars().all(|c| c.is_ascii_hexdigit()) => {
            let bytes: Vec<u8> = (0..s.len())
                .step_by(2)
                .filter_map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
                .collect();
            Some(
                bytes
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                    .collect(),
            )
        }
        _ => None,
    }
}

fn raw_identity(v: &[u32]) -> Option<RawIdentity> {
    if v.len() < 3 || v[0] == 0 {
        return None;
    }
    Some(RawIdentity {
        id_header: v[0],
        cert_stat: v[1],
        product: v[2],
        product_type_vdos: v[3..].iter().copied().take(3).collect(),
    })
}

/// H-M6: Discover Identity in a port subtree. Keys that mention SOP' (or
/// "Cable") carry the cable identity, plain SOP the partner.
fn find_identities(entries: &[&RegEntry]) -> (Option<RawIdentity>, Option<RawIdentity>) {
    let (mut partner, mut cable) = (None, None);
    for e in entries {
        for (k, v) in &e.props {
            let kl = k.to_ascii_lowercase();
            if !(kl.contains("identity") || kl.contains("vdo")) {
                continue;
            }
            let Some(list) = vdos(v).and_then(|l| raw_identity(&l)) else {
                continue;
            };
            if kl.contains("sop'") || kl.contains("sop1") || kl.contains("cable") {
                cable.get_or_insert(list);
            } else if kl.contains("sop") || kl.contains("partner") {
                partner.get_or_insert(list);
            }
        }
    }
    (partner, cable)
}

/// `kUSBHostPortSpeed*` → Mbit/s.
fn usb_speed_mbps(code: f64) -> Option<f64> {
    match code as i64 {
        0 => Some(1.5),
        1 => Some(12.0),
        2 => Some(480.0),
        3 => Some(5000.0),
        4 => Some(10000.0),
        5 => Some(20000.0),
        _ => None,
    }
}

fn usb_device(reg: &Reg<'_>, e: &RegEntry) -> Option<UsbDeviceObservation> {
    let p = &e.props;
    let speed = num(p, &["Device Speed", "USBSpeed"]).and_then(usb_speed_mbps)?;
    let mut children: Vec<UsbDeviceObservation> = reg
        .descendants(e.id)
        .into_iter()
        .filter(|c| c.class == "IOUSBHostDevice")
        .filter(|c| {
            // Direct child device: nearest IOUSBHostDevice ancestor is `e`.
            reg.ancestors(c.id)
                .into_iter()
                .find(|a| a.class == "IOUSBHostDevice")
                .is_some_and(|a| a.id == e.id)
        })
        .filter_map(|c| usb_device(reg, c))
        .collect();
    children.sort_by(|a, b| b.speed_mbps.total_cmp(&a.speed_mbps));
    Some(UsbDeviceObservation {
        vendor_id: num(p, &["idVendor"]).map(|v| v as u16),
        product_id: num(p, &["idProduct"]).map(|v| v as u16),
        vendor: string(p, &["USB Vendor Name", "kUSBVendorString"]).map(str::to_string),
        product: string(p, &["USB Product Name", "kUSBProductString"]).map(str::to_string),
        speed_mbps: speed,
        bcd_usb: num(p, &["bcdUSB"]).map(|v| v as u16),
        max_capable: None,
        is_hub: num(p, &["bDeviceClass"]) == Some(9.0),
        children,
        source: Source::Iokit,
    })
}

/// Trailing number of a registry name ("usb-drd1" → 1, "acio0" → 0).
fn trailing_index(name: &str, prefix: &str) -> Option<u32> {
    name.strip_prefix(prefix)?.parse().ok()
}

pub fn interpret(dump: &RegistryDump) -> Snapshot {
    let reg = Reg::new(dump);
    let host = HostInfo {
        os: Os::Macos,
        os_version: dump.host.os_version.clone(),
        model: dump.host.model.clone(),
        arch: if dump.host.arch.is_empty() {
            "arm64".into()
        } else {
            dump.host.arch.clone()
        },
    };

    // H-M1: one AppleHPMInterfaceType* entry per physical port.
    let mut hpm: Vec<&RegEntry> = dump
        .entries
        .iter()
        .filter(|e| e.class.starts_with("AppleHPMInterfaceType"))
        .collect();
    hpm.sort_by_key(|e| {
        num(&e.props, &["PortNumber", "Port Number"])
            .map(|n| n as u64)
            .unwrap_or(1000 + e.id)
    });

    let battery_entry = dump.entries.iter().find(|e| e.class == "AppleSmartBattery");
    let battery = battery_entry.map(|b| battery(&b.props));

    // USB devices by port index (H-M4: XHCI controller "usb-drdN" → port N+1).
    let mut usb_by_port: BTreeMap<u32, Vec<UsbDeviceObservation>> = BTreeMap::new();
    for e in dump.entries.iter().filter(|e| e.class == "IOUSBHostDevice") {
        let anc = reg.ancestors(e.id);
        if anc.iter().any(|a| a.class == "IOUSBHostDevice") {
            continue; // not top level
        }
        let Some(idx) = anc.iter().find_map(|a| trailing_index(&a.name, "usb-drd")) else {
            continue;
        };
        if let Some(d) = usb_device(&reg, e) {
            usb_by_port.entry(idx + 1).or_default().push(d);
        }
    }

    // Thunderbolt link by port index (H-M7: controller "acioN" → port N+1).
    let mut tb_by_port: BTreeMap<u32, FabricLinkObservation> = BTreeMap::new();
    for e in dump
        .entries
        .iter()
        .filter(|e| e.class.starts_with("IOThunderboltSwitch"))
    {
        let anc = reg.ancestors(e.id);
        let Some(idx) = anc.iter().find_map(|a| trailing_index(&a.name, "acio")) else {
            continue;
        };
        let depth = anc
            .iter()
            .filter(|a| a.class.starts_with("IOThunderboltSwitch"))
            .count() as u8;
        if depth == 0 {
            continue; // host router
        }
        // Link state on the switch's upstream port (H-M7).
        let link = reg
            .descendants(e.id)
            .into_iter()
            .chain(std::iter::once(e))
            .find_map(|x| {
                let speed = num(&x.props, &["Current Link Speed"])?;
                let width = num(&x.props, &["Current Link Width"]).unwrap_or(1.0);
                // Encoded as lane rate code: 0x2 = 10 Gbit/s, 0x4/0x8 = 20 Gbit/s.
                let lane = match speed as u32 {
                    0x2 => 10.0,
                    0x4 | 0x8 => 20.0,
                    0x10 => 40.0,
                    _ => return None,
                };
                Some((lane * width.clamp(1.0, 2.0), width as u8))
            });
        let entry = tb_by_port.entry(idx + 1).or_insert(FabricLinkObservation {
            generation: None,
            lanes: None,
            rx_gbps: None,
            tx_gbps: None,
            measured_by_controller: false,
            switches: vec![],
            source: Source::Iokit,
        });
        if depth == 1
            && let Some((gbps, lanes)) = link
        {
            entry.rx_gbps = Some(gbps);
            entry.tx_gbps = Some(gbps);
            entry.lanes = Some(lanes);
            entry.measured_by_controller = true;
        }
        entry.switches.push(FabricSwitch {
            depth,
            vendor: string(&e.props, &["Vendor Name"]).map(str::to_string),
            name: string(&e.props, &["Device Model Name"]).map(str::to_string),
            generation: None,
            link_gbps: link.map(|l| l.0),
            lanes: link.map(|l| l.1),
        });
    }

    let ext_displays: Vec<&CgDisplay> = dump.displays.iter().filter(|d| !d.builtin).collect();
    let mut ports = Vec::new();
    for (i, e) in hpm.iter().enumerate() {
        let index = num(&e.props, &["PortNumber", "Port Number"])
            .map(|n| n as u32)
            .unwrap_or(i as u32 + 1);
        ports.push(port(&reg, e, index, &usb_by_port, &tb_by_port));
    }

    // H-M8: map one external display to the only port with DP active.
    let dp_ports: Vec<usize> = ports
        .iter()
        .enumerate()
        .filter(|(_, p)| p.link.transports.contains(&Transport::Displayport))
        .map(|(i, _)| i)
        .collect();
    if dp_ports.len() == 1 && ext_displays.len() == 1 {
        let d = ext_displays[0];
        ports[dp_ports[0]].displays.push(DisplayObservation {
            name: None,
            current: d.current,
            max: d.max,
            dsc: None,
            adapter: None,
            source: Source::Coregraphics,
        });
    }

    // H-M5: charger data from AppleSmartBattery, only when exactly one
    // port is connected (the battery does not name the port).
    if let Some(b) = battery_entry {
        let connected: Vec<usize> = ports
            .iter()
            .enumerate()
            .filter(|(_, p)| p.connected == Some(true))
            .map(|(i, _)| i)
            .collect();
        let external = boolean(&b.props, &["ExternalConnected"]) == Some(true);
        if external && connected.len() == 1 {
            ports[connected[0]].power = adapter_power(&b.props);
        } else if external {
            for i in connected {
                ports[i].power = PowerObservation::Unavailable {
                    reason: Reason::PlatformNotExposed,
                };
            }
        }
    }

    Snapshot {
        host,
        tier: Tier::User,
        usb_probe: true,
        battery,
        ports,
        notices: vec![],
    }
}

fn port(
    reg: &Reg<'_>,
    e: &RegEntry,
    index: u32,
    usb_by_port: &BTreeMap<u32, Vec<UsbDeviceObservation>>,
    tb_by_port: &BTreeMap<u32, FabricLinkObservation>,
) -> PortObservation {
    let mut p = PortObservation::new(format!("macos-port-{index}"), index);
    let supported = strings(&e.props, "TransportsSupported");
    let active = strings(&e.props, "TransportsActive");
    let has = |list: &[String], t: &str| list.iter().any(|x| x.eq_ignore_ascii_case(t));
    p.kind = if has(&supported, "CIO") {
        PortKind::Thunderbolt
    } else {
        PortKind::UsbC
    };
    p.capabilities = PortCapabilities {
        // Apple specifies USB 3 up to 10 Gbit/s on all USB-C ports of
        // Apple silicon Macs (technical specifications pages).
        usb: if has(&supported, "USB3") {
            Some(UsbGen::Gen2)
        } else if has(&supported, "USB2") {
            Some(UsbGen::Usb2)
        } else {
            None
        },
        usb_is_lower_bound: false,
        fabric: None,
        displayport: Some(has(&supported, "DisplayPort")).filter(|b| *b),
        sink_max_w: None,
        source: Some(Source::Iokit),
    };
    let connected = boolean(&e.props, &["ConnectionActive"]);
    p.connected = connected;
    if let Some(o) = num(&e.props, &["Plug Orientation"]) {
        p.health = Some(HealthObservation {
            orientation: match o as i64 {
                1 => Some("normal".into()),
                2 => Some("reverse".into()),
                _ => None,
            },
            ..Default::default()
        });
    }
    for k in [
        "TransportsSupported",
        "TransportsActive",
        "PortType",
        "ConnectionActive",
    ] {
        if let Some(v) = e.props.get(k) {
            p.raw.insert(k.to_string(), v.clone());
        }
    }
    if connected != Some(true) {
        return p;
    }

    let subtree = reg.descendants(e.id);
    let mut scope: Vec<&RegEntry> = vec![e];
    scope.extend(subtree);
    let (partner, cable) = find_identities(&scope);
    p.partner = Some(PartnerObservation {
        identity: partner,
        supports_pd: None,
        accessory: None,
        name: None,
        source: Source::Iokit,
    });
    p.cable = match cable {
        Some(id) => CableObservation::Emarker {
            identity: id,
            far_end: None,
            source: Source::Iokit,
        },
        None => CableObservation::Unavailable {
            reason: Reason::PlatformNotExposed,
        },
    };
    p.power = PowerObservation::Unavailable {
        reason: Reason::PlatformNotExposed,
    };

    let mut transports = Vec::new();
    if has(&active, "CIO") {
        transports.push(Transport::Thunderbolt);
    }
    if has(&active, "DisplayPort") {
        transports.push(Transport::Displayport);
        p.link.displayport = Some(DpLinkObservation {
            lanes: None,
            lane_gbps: None,
            pin_assignment: None,
            tunneled: false,
            source: Source::Iokit,
        });
    }
    p.link.transports = transports;
    p.link.usb_devices = usb_by_port.get(&index).cloned().unwrap_or_default();
    p.link.source = Some(Source::Iokit);
    if let Some(f) = tb_by_port.get(&index) {
        p.link.fabric = Some(f.clone());
        if !p.link.transports.contains(&Transport::Thunderbolt) {
            p.link.transports.push(Transport::Thunderbolt);
        }
    }
    p
}

fn battery(props: &Map<String, Value>) -> BatteryObservation {
    let cur = num(props, &["CurrentCapacity"]);
    let max = num(props, &["MaxCapacity"]).filter(|m| *m > 0.0);
    let percent = match (cur, max) {
        (Some(c), Some(m)) if m <= 100.0 => Some(c),
        (Some(c), Some(m)) => Some((c / m * 100.0).round()),
        _ => None,
    };
    let v = num(props, &["Voltage"]);
    let a = num(props, &["Amperage", "InstantAmperage"]);
    BatteryObservation {
        percent,
        charging: boolean(props, &["IsCharging"]),
        full: boolean(props, &["FullyCharged"]),
        external_power: boolean(props, &["ExternalConnected"]),
        power_w: match (v, a) {
            (Some(v), Some(a)) => Some(((v * a) / 1e6 * 100.0).round() / 100.0),
            _ => None,
        },
        source: Source::Iokit,
    }
}

/// H-M5: "AdapterDetails" with "UsbHvcMenu" (PDO list, mV/mA) and
/// "UsbHvcHvcIndex" (selected entry); "PowerTelemetryData.SystemPowerIn"
/// (mW) for the measured input power.
fn adapter_power(props: &Map<String, Value>) -> PowerObservation {
    let Some(ad) = props.get("AdapterDetails").and_then(Value::as_object) else {
        return PowerObservation::Unavailable {
            reason: Reason::PlatformNotExposed,
        };
    };
    let mut pd = PowerData {
        role: Some(PowerRole::Sink),
        source: Source::Iokit,
        ..Default::default()
    };
    if let Some(menu) = ad.get("UsbHvcMenu").and_then(Value::as_array) {
        for item in menu {
            let Some(o) = item.as_object() else { continue };
            let (Some(mv), Some(ma)) = (num(o, &["MaxVoltage"]), num(o, &["MaxCurrent"])) else {
                continue;
            };
            pd.source_pdos.push(linkspec_core::pd::Pdo::Fixed {
                voltage_v: mv / 1000.0,
                max_current_a: ma / 1000.0,
                epr_capable: false,
                usb_comm_capable: false,
                unconstrained_power: false,
                dual_role_power: false,
            });
        }
    }
    let selected = num(ad, &["UsbHvcHvcIndex"]).map(|i| i as usize);
    if let Some(i) = selected
        && let Some(linkspec_core::pd::Pdo::Fixed {
            voltage_v,
            max_current_a,
            ..
        }) = pd.source_pdos.get(i)
    {
        pd.contract = Some(Contract {
            pdo_index: Some(i as u8 + 1),
            voltage_v: *voltage_v,
            current_a: *max_current_a,
            power_w: ((voltage_v * max_current_a) * 100.0).round() / 100.0,
        });
    } else if let (Some(mv), Some(ma)) = (num(ad, &["Voltage"]), num(ad, &["Current"])) {
        pd.contract = Some(Contract {
            pdo_index: None,
            voltage_v: mv / 1000.0,
            current_a: ma / 1000.0,
            power_w: ((mv * ma) / 1e6 * 100.0).round() / 100.0,
        });
    }
    if let Some(tel) = props.get("PowerTelemetryData").and_then(Value::as_object) {
        pd.measured_w =
            num(tel, &["SystemPowerIn"]).map(|mw| (mw / 1000.0 * 100.0).round() / 100.0);
        pd.measured_v = num(tel, &["SystemVoltageIn"]).map(|mv| mv / 1000.0);
        pd.measured_a = num(tel, &["SystemCurrentIn"]).map(|ma| ma / 1000.0);
    }
    PowerObservation::Available(Box::new(pd))
}

/// Power sample for the monitor (H-M5).
pub fn power_sample(dump: &RegistryDump) -> Option<(f64, Option<f64>, Option<f64>)> {
    let b = dump
        .entries
        .iter()
        .find(|e| e.class == "AppleSmartBattery")?;
    let tel = b.props.get("PowerTelemetryData")?.as_object()?;
    let w = num(tel, &["SystemPowerIn"])? / 1000.0;
    Some((
        w,
        num(tel, &["SystemVoltageIn"]).map(|v| v / 1000.0),
        num(tel, &["SystemCurrentIn"]).map(|a| a / 1000.0),
    ))
}
