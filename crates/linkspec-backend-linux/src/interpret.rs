//! Turns a [`SysfsDump`] into a [`Snapshot`]. Pure function, runs on every
//! platform, tested with synthetic sysfs images.

use std::collections::{BTreeMap, BTreeSet};

use linkspec_core::observation::*;
use linkspec_core::pd::{Pdo, RawIdentity};
use linkspec_core::types::{FabricGen, Source, Transport, UsbGen};

use crate::dump::SysfsDump;

struct D<'a>(&'a SysfsDump);

impl<'a> D<'a> {
    fn f(&self, path: &str) -> Option<&'a str> {
        self.0.files.get(path).map(String::as_str)
    }

    fn link_base(&self, path: &str) -> Option<&'a str> {
        self.0
            .links
            .get(path)
            .and_then(|l| l.rsplit('/').next())
            .filter(|s| !s.is_empty())
    }

    /// Immediate child names below `prefix` (from all keys).
    fn children(&self, prefix: &str) -> BTreeSet<&'a str> {
        let p = format!("{prefix}/");
        let mut out = BTreeSet::new();
        for k in self
            .0
            .files
            .keys()
            .chain(self.0.links.keys())
            .chain(self.0.denied.iter())
        {
            if let Some(rest) = k.strip_prefix(&p)
                && let Some(first) = rest.split('/').next()
            {
                out.insert(first);
            }
        }
        out
    }

    fn exists(&self, prefix: &str) -> bool {
        let p = format!("{prefix}/");
        self.0
            .files
            .keys()
            .chain(self.0.links.keys())
            .any(|k| k.starts_with(&p))
    }

    fn hex(&self, path: &str) -> Option<u32> {
        let s = self.f(path)?;
        u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok()
    }

    fn num(&self, path: &str) -> Option<f64> {
        parse_leading_f64(self.f(path)?)
    }
}

fn parse_leading_f64(s: &str) -> Option<f64> {
    let s = s.trim();
    let end = s
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
        .unwrap_or(s.len());
    s[..end].parse().ok()
}

/// Selected value of a sysfs choice attribute: `"[host] device"` → `host`.
fn selected(s: &str) -> Option<&str> {
    let start = s.find('[')?;
    let end = s[start..].find(']')? + start;
    Some(&s[start + 1..end])
}

fn port_number(name: &str) -> Option<u32> {
    name.strip_prefix("port")?.parse().ok()
}

pub fn interpret(dump: &SysfsDump) -> Snapshot {
    let d = D(dump);
    let host = HostInfo {
        os: Os::Linux,
        os_version: dump.host.kernel.clone(),
        model: dump.host.product.clone(),
        arch: dump
            .host
            .arch
            .clone()
            .unwrap_or_else(|| std::env::consts::ARCH.to_string()),
    };

    let usb = UsbIndex::build(&d);
    let tb = TbIndex::build(&d);

    let mut port_names: Vec<(u32, &str)> = d
        .children("class/typec")
        .into_iter()
        .filter_map(|n| port_number(n).map(|i| (i, n)))
        .collect();
    port_names.sort();

    let dp_active_ports: Vec<u32> = port_names
        .iter()
        .filter(|(_, n)| dp_altmode(&d, n).is_some())
        .map(|(i, _)| *i)
        .collect();

    let mut ports = Vec::new();
    for (num, name) in &port_names {
        ports.push(port(&d, *num, name, &usb, &tb, &dp_active_ports));
    }

    Snapshot {
        host,
        tier: Tier::User,
        usb_probe: true,
        battery: battery(&d),
        ports,
        notices: vec![],
    }
}

fn identity(d: &D<'_>, base: &str) -> Option<RawIdentity> {
    let id = format!("{base}/identity");
    let id_header = d.hex(&format!("{id}/id_header"))?;
    let mut vdos = Vec::new();
    for i in 1..=3 {
        vdos.push(d.hex(&format!("{id}/product_type_vdo{i}")).unwrap_or(0));
    }
    while vdos.last() == Some(&0) {
        vdos.pop();
    }
    Some(RawIdentity {
        id_header,
        cert_stat: d.hex(&format!("{id}/cert_stat")).unwrap_or(0),
        product: d.hex(&format!("{id}/product")).unwrap_or(0),
        product_type_vdos: vdos,
    })
}

/// Active DisplayPort alternate mode of the partner: (pin assignment).
fn dp_altmode<'a>(d: &D<'a>, port: &str) -> Option<Option<&'a str>> {
    let partner = format!("class/typec/{port}-partner");
    for child in d.children(&partner) {
        if !child.starts_with(&format!("{port}-partner.")) {
            continue;
        }
        let base = format!("{partner}/{child}");
        let svid = d.f(&format!("{base}/svid")).map(str::to_ascii_lowercase);
        let active = d.f(&format!("{base}/active")) == Some("yes");
        if svid.as_deref() == Some("ff01") && active {
            let pin = d
                .f(&format!("{base}/displayport/pin_assignment"))
                .and_then(selected);
            return Some(pin);
        }
    }
    None
}

fn altmode_active(d: &D<'_>, port: &str, svid: &str) -> bool {
    let partner = format!("class/typec/{port}-partner");
    d.children(&partner).into_iter().any(|child| {
        child.starts_with(&format!("{port}-partner."))
            && d.f(&format!("{partner}/{child}/svid"))
                .is_some_and(|s| s.eq_ignore_ascii_case(svid))
            && d.f(&format!("{partner}/{child}/active")) == Some("yes")
    })
}

fn port_supports_altmode(d: &D<'_>, port: &str, svid: &str) -> bool {
    let base = format!("class/typec/{port}");
    d.children(&base).into_iter().any(|child| {
        child.starts_with(&format!("{port}."))
            && d.f(&format!("{base}/{child}/svid"))
                .is_some_and(|s| s.eq_ignore_ascii_case(svid))
    })
}

fn pdos(d: &D<'_>, pd: &str, caps: &str) -> Vec<Pdo> {
    let base = format!("class/usb_power_delivery/{pd}/{caps}");
    let mut list: Vec<(u32, &str)> = d
        .children(&base)
        .into_iter()
        .filter_map(|c| {
            let (pos, kind) = c.split_once(':')?;
            Some((pos.parse().ok()?, kind))
        })
        .collect();
    list.sort();
    let source = caps.starts_with("source");
    let mut out = Vec::new();
    for (pos, kind) in list {
        let p = format!("{base}/{pos}:{kind}");
        let mv = |a: &str| d.num(&format!("{p}/{a}")).map(|v| v / 1000.0);
        let flag = |a: &str| d.f(&format!("{p}/{a}")) == Some("1");
        let pdo = match kind {
            "fixed_supply" => Pdo::Fixed {
                voltage_v: mv("voltage").unwrap_or(0.0),
                max_current_a: if source {
                    mv("maximum_current")
                } else {
                    mv("operational_current")
                }
                .unwrap_or(0.0),
                epr_capable: false,
                usb_comm_capable: flag("usb_communication_capable"),
                unconstrained_power: flag("unconstrained_power"),
                dual_role_power: flag("dual_role_power"),
            },
            "variable_supply" => Pdo::Variable {
                min_voltage_v: mv("minimum_voltage").unwrap_or(0.0),
                max_voltage_v: mv("maximum_voltage").unwrap_or(0.0),
                max_current_a: mv("maximum_current")
                    .or_else(|| mv("operational_current"))
                    .unwrap_or(0.0),
            },
            "battery" => Pdo::Battery {
                min_voltage_v: mv("minimum_voltage").unwrap_or(0.0),
                max_voltage_v: mv("maximum_voltage").unwrap_or(0.0),
                max_power_w: mv("maximum_power")
                    .or_else(|| mv("operational_power"))
                    .unwrap_or(0.0),
            },
            "programmable_supply" => Pdo::Pps {
                min_voltage_v: mv("minimum_voltage").unwrap_or(0.0),
                max_voltage_v: mv("maximum_voltage").unwrap_or(0.0),
                max_current_a: mv("maximum_current").unwrap_or(0.0),
                power_limited: flag("pps_power_limited"),
            },
            _ => continue,
        };
        out.push(pdo);
    }
    out
}

/// USB topology index: hub ports with a `connector` link, bus speeds.
struct UsbIndex {
    /// typec port name → (bus speeds in Mbit/s of linked root ports).
    port_speeds: BTreeMap<String, Vec<f64>>,
    /// typec port name → root hub port keys (`<bus>-<port>` device names).
    port_devices: BTreeMap<String, Vec<String>>,
    /// typec port name → over-current count (sum over linked hub ports).
    overcurrent: BTreeMap<String, u32>,
    /// typec ports that are reached only through a non-root hub.
    behind_hub: BTreeSet<String>,
    any_usb: bool,
}

impl UsbIndex {
    fn build(d: &D<'_>) -> Self {
        let mut idx = UsbIndex {
            port_speeds: BTreeMap::new(),
            port_devices: BTreeMap::new(),
            overcurrent: BTreeMap::new(),
            behind_hub: BTreeSet::new(),
            any_usb: d.exists("bus/usb/devices"),
        };
        // Hub port directories: bus/usb/devices/<hubdev>:1.0/<prefix>-port<M>
        let mut hub_ports: Vec<(String, String, String)> = Vec::new(); // (iface, portdir, key)
        for iface in d.children("bus/usb/devices") {
            if !iface.contains(':') {
                continue;
            }
            for pd in d.children(&format!("bus/usb/devices/{iface}")) {
                if pd.contains("-port") {
                    hub_ports.push((
                        iface.to_string(),
                        pd.to_string(),
                        format!("bus/usb/devices/{iface}/{pd}"),
                    ));
                }
            }
        }
        let connector_of: BTreeMap<String, String> = hub_ports
            .iter()
            .filter_map(|(_, pd, key)| {
                d.link_base(&format!("{key}/connector"))
                    .map(|c| (pd.clone(), c.to_string()))
            })
            .collect();
        for (iface, pd, key) in &hub_ports {
            // Connector directly, or through the peer (USB2 <-> USB3 companion).
            let conn = connector_of.get(pd).cloned().or_else(|| {
                d.link_base(&format!("{key}/peer"))
                    .and_then(|peer| connector_of.get(peer).cloned())
            });
            let Some(conn) = conn else { continue };
            let hubdev = iface.split(':').next().unwrap_or_default();
            let portnum = pd.rsplit("-port").next().unwrap_or_default();
            let root = hubdev.ends_with("-0");
            if root {
                let bus = hubdev.trim_end_matches("-0");
                if let Some(sp) = d.num(&format!("bus/usb/devices/usb{bus}/speed")) {
                    idx.port_speeds.entry(conn.clone()).or_default().push(sp);
                }
                idx.port_devices
                    .entry(conn.clone())
                    .or_default()
                    .push(format!("{bus}-{portnum}"));
            } else {
                idx.behind_hub.insert(conn.clone());
                idx.port_devices
                    .entry(conn.clone())
                    .or_default()
                    .push(format!("{hubdev}.{portnum}"));
            }
            if let Some(oc) = d.num(&format!("{key}/over_current_count")) {
                *idx.overcurrent.entry(conn.clone()).or_default() += oc as u32;
            }
        }
        idx
    }

    fn max_class(&self, port: &str) -> Option<UsbGen> {
        self.port_speeds
            .get(port)?
            .iter()
            .map(|s| UsbGen::from_link_mbps(*s))
            .max()
    }
}

fn usb_device(d: &D<'_>, name: &str) -> Option<UsbDeviceObservation> {
    let base = format!("bus/usb/devices/{name}");
    let speed = d.num(&format!("{base}/speed"))?;
    let bcd = d.f(&format!("{base}/version")).and_then(|v| {
        let (maj, min) = v.trim().split_once('.')?;
        let maj: u16 = maj.trim().parse().ok()?;
        let min: u16 = u16::from_str_radix(min.trim(), 16).ok()?;
        Some((maj << 8) | min)
    });
    let is_hub = d.f(&format!("{base}/bDeviceClass")) == Some("09");
    // Children: devices named "<name>.<n>".
    let mut children: Vec<UsbDeviceObservation> = d
        .children("bus/usb/devices")
        .into_iter()
        .filter(|c| {
            !c.contains(':')
                && c.strip_prefix(&format!("{name}."))
                    .is_some_and(|r| !r.contains('.'))
        })
        .filter_map(|c| usb_device(d, c))
        .collect();
    children.sort_by(|a, b| b.speed_mbps.total_cmp(&a.speed_mbps));
    Some(UsbDeviceObservation {
        vendor_id: d.hex(&format!("{base}/idVendor")).map(|v| v as u16),
        product_id: d.hex(&format!("{base}/idProduct")).map(|v| v as u16),
        vendor: d.f(&format!("{base}/manufacturer")).map(str::to_string),
        product: d.f(&format!("{base}/product")).map(str::to_string),
        speed_mbps: speed,
        bcd_usb: bcd,
        max_capable: None,
        is_hub,
        children,
        source: Source::Sysfs,
    })
}

/// Thunderbolt/USB4 index: host router ports linked to Type-C connectors.
struct TbIndex {
    /// typec port name → (host router name, adapter number).
    by_port: BTreeMap<String, (String, u32)>,
}

impl TbIndex {
    fn build(d: &D<'_>) -> Self {
        let mut by_port = BTreeMap::new();
        for r in d.children("bus/thunderbolt/devices") {
            if !r.ends_with("-0") {
                continue;
            }
            let base = format!("bus/thunderbolt/devices/{r}");
            for c in d.children(&base) {
                if let Some(n) = c.strip_prefix("usb4_port")
                    && let Ok(n) = n.parse::<u32>()
                    && let Some(conn) = d.link_base(&format!("{base}/{c}/connector"))
                {
                    by_port.insert(conn.to_string(), (r.to_string(), n));
                }
            }
        }
        Self { by_port }
    }
}

fn fabric_gen(g: Option<f64>, total_gbps: Option<f64>) -> Option<FabricGen> {
    match g? as u32 {
        1..=3 => Some(FabricGen::Tb3),
        4 if total_gbps.is_some_and(|t| t > 40.5) => Some(FabricGen::Usb4V2),
        4 => Some(FabricGen::Usb4V1),
        _ => None,
    }
}

fn router_rate(d: &D<'_>, base: &str, dir: &str) -> Option<f64> {
    let speed = d.num(&format!("{base}/{dir}_speed"))?;
    let lanes = d.num(&format!("{base}/{dir}_lanes")).unwrap_or(1.0);
    Some(speed * lanes)
}

fn fabric(
    d: &D<'_>,
    tb: &TbIndex,
    port: &str,
) -> Option<(FabricLinkObservation, Option<FabricGen>)> {
    let (host, adapter) = tb.by_port.get(port)?;
    let host_gen = fabric_gen(
        d.num(&format!("bus/thunderbolt/devices/{host}/generation")),
        None,
    );
    let domain = host.trim_end_matches("-0");
    // Routers in this domain whose first hop is `adapter`.
    let mut switches = Vec::new();
    let mut first: Option<String> = None;
    for r in d.children("bus/thunderbolt/devices") {
        let Some(route) = r
            .strip_prefix(&format!("{domain}-"))
            .filter(|s| !s.contains(':') && !s.contains('.'))
            .and_then(|s| u64::from_str_radix(s, 16).ok())
        else {
            continue;
        };
        if route == 0 || route & 0xff != u64::from(*adapter) {
            continue;
        }
        let depth = (64 - route.leading_zeros()).div_ceil(8) as u8;
        let base = format!("bus/thunderbolt/devices/{r}");
        let rx = router_rate(d, &base, "rx");
        let tx = router_rate(d, &base, "tx");
        let link = match (rx, tx) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        let gen_ = fabric_gen(d.num(&format!("{base}/generation")), link);
        if depth == 1 {
            first = Some(r.to_string());
        }
        switches.push(FabricSwitch {
            depth,
            vendor: d.f(&format!("{base}/vendor_name")).map(str::to_string),
            name: d.f(&format!("{base}/device_name")).map(str::to_string),
            generation: gen_,
            link_gbps: link,
            lanes: d.num(&format!("{base}/rx_lanes")).map(|v| v as u8),
        });
    }
    switches.sort_by_key(|s| s.depth);
    let first = first?;
    let base = format!("bus/thunderbolt/devices/{first}");
    let rx = router_rate(d, &base, "rx");
    let tx = router_rate(d, &base, "tx");
    let gen_ = switches.first().and_then(|s| s.generation);
    Some((
        FabricLinkObservation {
            generation: gen_,
            lanes: d.num(&format!("{base}/rx_lanes")).map(|v| v as u8),
            rx_gbps: rx,
            tx_gbps: tx,
            // rx/tx speed and lanes are read from the lane adapters after
            // link training (sysfs-bus-thunderbolt).
            measured_by_controller: rx.is_some() || tx.is_some(),
            switches,
            source: Source::Sysfs,
        },
        host_gen,
    ))
}

fn port(
    d: &D<'_>,
    num: u32,
    name: &str,
    usb: &UsbIndex,
    tb: &TbIndex,
    dp_active_ports: &[u32],
) -> PortObservation {
    let base = format!("class/typec/{name}");
    let partner_base = format!("{base}-partner");
    let cable_base = format!("{base}-cable");
    let id =
        d.0.links
            .get(&format!("{base}/firmware_node"))
            .map(|f| {
                let clean: String = f
                    .chars()
                    .map(|c| {
                        if c.is_ascii_alphanumeric() {
                            c.to_ascii_lowercase()
                        } else {
                            '-'
                        }
                    })
                    .collect();
                format!("linux-{clean}")
            })
            .unwrap_or_else(|| format!("linux-typec-{name}"));
    let mut p = PortObservation::new(id, num + 1);

    // Location (_PLD via physical_location, kernel 5.19+).
    let loc = |a: &str| {
        d.f(&format!("{base}/physical_location/{a}"))
            .filter(|v| *v != "unknown")
            .map(str::to_string)
    };
    let location = PortLocation {
        panel: loc("panel"),
        horizontal: loc("horizontal_position"),
        vertical: loc("vertical_position"),
    };
    if location.panel.is_some() || location.horizontal.is_some() || location.vertical.is_some() {
        p.location = Some(location);
    }

    // Capabilities.
    let fab = fabric(d, tb, name);
    let host_router_gen = tb.by_port.get(name).and_then(|(h, _)| {
        fabric_gen(
            d.num(&format!("bus/thunderbolt/devices/{h}/generation")),
            None,
        )
    });
    p.kind = match host_router_gen {
        Some(FabricGen::Tb3) => PortKind::Thunderbolt,
        Some(_) => PortKind::Usb4,
        None => PortKind::UsbC,
    };
    let own_pd = d.link_base(&format!("{base}/usb_power_delivery"));
    p.capabilities = PortCapabilities {
        usb: usb.max_class(name),
        fabric: host_router_gen,
        displayport: Some(port_supports_altmode(d, name, "ff01")).filter(|b| *b),
        usb_is_lower_bound: false,
        sink_max_w: own_pd.and_then(|pd| {
            pdos(d, pd, "sink-capabilities")
                .iter()
                .filter_map(Pdo::max_w)
                .reduce(f64::max)
        }),
        source: Some(Source::Sysfs),
    };

    let connected = d.exists(&partner_base);
    p.connected = Some(connected);
    p.behind_internal_hub = usb.behind_hub.contains(name) && !usb.port_speeds.contains_key(name);

    // Health.
    let orientation = d
        .f(&format!("{base}/orientation"))
        .filter(|o| *o == "normal" || *o == "reverse")
        .map(str::to_string);
    let oc = usb.overcurrent.get(name).copied();
    if orientation.is_some() || oc.is_some() {
        p.health = Some(HealthObservation {
            overcurrent_count: oc,
            orientation: if connected { orientation } else { None },
            ..Default::default()
        });
    }

    for k in [
        "data_role",
        "power_role",
        "power_operation_mode",
        "usb_power_delivery_revision",
        "port_type",
    ] {
        if let Some(v) = d.f(&format!("{base}/{k}")) {
            p.raw
                .insert(k.to_string(), serde_json::Value::String(v.to_string()));
        }
    }

    if !connected {
        return p;
    }

    // Partner.
    let partner_id = identity(d, &partner_base).filter(|i| !i.is_empty());
    p.partner = Some(PartnerObservation {
        identity: partner_id,
        supports_pd: d
            .f(&format!("{partner_base}/supports_usb_power_delivery"))
            .map(|v| v == "yes"),
        accessory: d
            .f(&format!("{partner_base}/accessory_mode"))
            .filter(|a| *a != "none")
            .map(str::to_string),
        name: None,
        source: Source::Sysfs,
    });

    // Cable: only a non-empty identity proves an E-Marker. An empty or
    // missing identity cannot be told apart from "not reported" on UCSI.
    p.cable = match identity(d, &cable_base).filter(|i| !i.is_empty()) {
        Some(id) => CableObservation::Emarker {
            identity: id,
            far_end: None,
            source: Source::Sysfs,
        },
        None => CableObservation::Unavailable {
            reason: Reason::FirmwareNotReported,
        },
    };

    // Power.
    let role = d
        .f(&format!("{base}/power_role"))
        .and_then(selected)
        .map(|r| {
            if r == "sink" {
                PowerRole::Sink
            } else {
                PowerRole::Source
            }
        });
    let partner_pd = d.link_base(&format!("{partner_base}/usb_power_delivery"));
    let mut pd = PowerData {
        role,
        source: Source::Sysfs,
        ..Default::default()
    };
    match role {
        Some(PowerRole::Source) => {
            if let Some(own) = own_pd {
                pd.source_pdos = pdos(d, own, "source-capabilities");
            }
            if let Some(pp) = partner_pd {
                pd.sink_pdos = pdos(d, pp, "sink-capabilities");
            }
        }
        _ => {
            if let Some(pp) = partner_pd {
                pd.source_pdos = pdos(d, pp, "source-capabilities");
            }
            if let Some(own) = own_pd {
                pd.sink_pdos = pdos(d, own, "sink-capabilities");
            }
        }
    }
    pd.typec_current = match d.f(&format!("{base}/power_operation_mode")) {
        Some("default") => Some(TypecCurrent::Default),
        Some("1.5A") => Some(TypecCurrent::A1_5),
        Some("3.0A") => Some(TypecCurrent::A3),
        _ => None,
    };
    if let Some((v, a)) = ucsi_contract(d, num + 1) {
        pd.contract = Some(Contract {
            pdo_index: None,
            voltage_v: v,
            current_a: a,
            power_w: ((v * a) * 100.0).round() / 100.0,
        });
    }
    let has_power = !pd.source_pdos.is_empty()
        || pd.contract.is_some()
        || pd.typec_current.is_some()
        || role.is_some();
    p.power = if has_power {
        if pd.source_pdos.is_empty() && role != Some(PowerRole::Source) {
            p.mark_unavailable("power.pdos", Reason::FirmwareNotReported);
        }
        PowerObservation::Available(Box::new(pd))
    } else {
        PowerObservation::Unavailable {
            reason: Reason::FirmwareNotReported,
        }
    };

    // Link: USB devices on the linked root/hub ports.
    let mut transports = Vec::new();
    if let Some(devs) = usb.port_devices.get(name) {
        for dev in devs {
            if let Some(u) = usb_device(d, dev) {
                p.link.usb_devices.push(u);
            }
        }
    }
    p.link
        .usb_devices
        .sort_by(|a, b| b.speed_mbps.total_cmp(&a.speed_mbps));
    if (usb.any_usb && usb.port_speeds.contains_key(name)) || !p.link.usb_devices.is_empty() {
        p.link.source = Some(Source::Sysfs);
    } else {
        p.mark_unavailable("link.usb", Reason::FirmwareNotReported);
    }
    if let Some((f, _)) = fab {
        let tbt =
            f.generation.is_some_and(|g| g == FabricGen::Tb3) || altmode_active(d, name, "8087");
        transports.push(if tbt {
            Transport::Thunderbolt
        } else {
            Transport::Usb4
        });
        p.link.fabric = Some(f);
    } else if altmode_active(d, name, "8087") {
        transports.push(Transport::Thunderbolt);
    }
    if let Some(pin) = dp_altmode(d, name) {
        transports.push(Transport::Displayport);
        let lanes = match pin {
            Some("C") | Some("E") => Some(4),
            Some("D") | Some("F") => Some(2),
            _ => None,
        };
        p.link.displayport = Some(DpLinkObservation {
            lanes,
            lane_gbps: None,
            pin_assignment: pin.map(str::to_string),
            tunneled: false,
            source: Source::Sysfs,
        });
    }
    p.link.transports = transports;

    // Displays: DRM connectors linked to this port, or the only connected
    // external DP connector when exactly one port has DP Alt Mode active.
    for conn in d.children("class/drm") {
        let cbase = format!("class/drm/{conn}");
        if d.f(&format!("{cbase}/status")) != Some("connected") || conn.contains("eDP") {
            continue;
        }
        let linked = d.link_base(&format!("{cbase}/typec_connector")) == Some(name);
        let heuristic = dp_active_ports == [num] && conn.contains("-DP-");
        if !(linked || heuristic) {
            continue;
        }
        let edid = dump_edid(d, &cbase);
        p.displays.push(DisplayObservation {
            name: edid.as_ref().and_then(|e| e.name.clone()),
            current: None,
            max: edid.as_ref().and_then(|e| e.max).map(|m| DisplayMode {
                width: m.width,
                height: m.height,
                refresh_hz: (f64::from(m.refresh_mhz) / 1000.0).round(),
                bits_per_color: edid.as_ref().and_then(|e| e.bits_per_color),
            }),
            dsc: None,
            adapter: None,
            source: Source::Sysfs,
        });
        p.mark_unavailable("displays.current", Reason::PlatformNotExposed);
    }

    p
}

fn dump_edid(d: &D<'_>, conn: &str) -> Option<linkspec_core::edid::EdidSummary> {
    d.0.edid.get(conn).cloned()
}

/// Negotiated contract from the UCSI power supply of connector `con` (1-based).
fn ucsi_contract(d: &D<'_>, con: u32) -> Option<(f64, f64)> {
    let suffix = format!(":{con:03}");
    for psy in d.children("class/power_supply") {
        if !psy.starts_with("ucsi-source-psy-") || !psy.ends_with(&suffix) {
            continue;
        }
        let base = format!("class/power_supply/{psy}");
        if d.f(&format!("{base}/online")) != Some("1") {
            return None;
        }
        let pd_active = d
            .f(&format!("{base}/usb_type"))
            .and_then(selected)
            .is_some_and(|t| t.starts_with("PD"));
        if !pd_active {
            return None;
        }
        let v = d.num(&format!("{base}/voltage_now"))? / 1e6;
        let a = d
            .num(&format!("{base}/current_now"))
            .or_else(|| d.num(&format!("{base}/current_max")))?
            / 1e6;
        if v <= 0.0 || a <= 0.0 {
            return None;
        }
        return Some((v, a));
    }
    None
}

fn battery(d: &D<'_>) -> Option<BatteryObservation> {
    for psy in d.children("class/power_supply") {
        let base = format!("class/power_supply/{psy}");
        if d.f(&format!("{base}/type")) != Some("Battery") {
            continue;
        }
        if d.f(&format!("{base}/scope")) == Some("Device") {
            continue;
        }
        let status = d.f(&format!("{base}/status"));
        let power = d
            .num(&format!("{base}/power_now"))
            .map(|p| p / 1e6)
            .or_else(|| {
                let a = d.num(&format!("{base}/current_now"))?;
                let v = d.num(&format!("{base}/voltage_now"))?;
                Some(a * v / 1e12)
            });
        let sign = if status == Some("Discharging") {
            -1.0
        } else {
            1.0
        };
        return Some(BatteryObservation {
            percent: d.num(&format!("{base}/capacity")),
            charging: status.map(|s| s == "Charging"),
            full: status.map(|s| s == "Full"),
            external_power: status.map(|s| s != "Discharging"),
            power_w: power.map(|p| (p.abs() * sign * 100.0).round() / 100.0),
            source: Source::Sysfs,
        });
    }
    None
}

/// Live power sample for the monitor: UCSI supply of a connector, else the
/// battery input.
pub fn power_sample(dump: &SysfsDump) -> Option<(Option<u32>, f64, f64, f64)> {
    let d = D(dump);
    for psy in d.children("class/power_supply") {
        let base = format!("class/power_supply/{psy}");
        if psy.starts_with("ucsi-source-psy-") && d.f(&format!("{base}/online")) == Some("1") {
            let v = d.num(&format!("{base}/voltage_now"))? / 1e6;
            let a = d.num(&format!("{base}/current_now"))? / 1e6;
            let con = psy.rsplit(':').next().and_then(|n| n.parse::<u32>().ok());
            return Some((con, v * a, v, a));
        }
    }
    for psy in d.children("class/power_supply") {
        let base = format!("class/power_supply/{psy}");
        if d.f(&format!("{base}/type")) == Some("Battery") {
            let v = d.num(&format!("{base}/voltage_now"))? / 1e6;
            let a = d.num(&format!("{base}/current_now")).map(|x| x / 1e6);
            let w = d
                .num(&format!("{base}/power_now"))
                .map(|x| x / 1e6)
                .or(a.map(|a| a * v))?;
            return Some((None, w, v, a.unwrap_or(w / v)));
        }
    }
    None
}
