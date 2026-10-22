//! Verdict engine: turns a [`Snapshot`] into a [`Diagnosis`].
//!
//! Rules (prompt section 5.2): compare only values that were really read.
//! Every verdict names exactly one limiter, carries a stable `code` and the
//! evidence it is based on. Every `code` has a fixture in `fixtures/core/`.

use serde_json::json;

use crate::catalog::{Catalog, HistoryLookup, cable_fingerprint};
use crate::diagnosis::*;
use crate::display;
use crate::i18n::{Arg, I18n};
use crate::observation::*;
use crate::pd::{CableKind, Identity, Pdo, ProductType, RawIdentity, Sop};
use crate::types::{FabricGen, Source, Transport, UsbGen};

/// Inputs besides the snapshot.
pub struct Context<'a> {
    pub i18n: &'a I18n,
    pub catalog: &'a dyn Catalog,
    pub history: Option<&'a dyn HistoryLookup>,
    pub include_raw: bool,
    pub app_version: &'a str,
    /// RFC 3339 timestamp for `generated_at`.
    pub generated_at: String,
}

/// All verdict codes with their limiter class. The fixture suite checks that
/// each code is covered.
pub const VERDICT_CODES: &[&str] = &[
    "power_ok",
    "charging_below_max",
    "battery_full",
    "cable_limits_power",
    "charger_limits_power",
    "host_limits_power",
    "power_basic_only",
    "power_supplying_device",
    "power_unknown",
    "data_ok",
    "device_at_its_max",
    "cable_limits_data",
    "cable_likely_usb2",
    "port_limits_data",
    "emarker_link_mismatch",
    "no_data_link",
    "data_via_internal_hub",
    "data_unknown",
    "fabric_link_below_cable",
    "fabric_link_unverified",
    "display_at_max",
    "display_below_max",
    "display_link_too_slow",
    "display_lanes_shared",
    "display_unknown",
];

/// Codes that put the port card into the warning state.
const WARN_CODES: &[&str] = &[
    "cable_limits_power",
    "charger_limits_power",
    "cable_limits_data",
    "cable_likely_usb2",
    "port_limits_data",
    "emarker_link_mismatch",
    "no_data_link",
    "fabric_link_below_cable",
    "fabric_link_unverified",
    "display_link_too_slow",
    "display_lanes_shared",
];

pub fn diagnose(snap: &Snapshot, ctx: &Context<'_>) -> Diagnosis {
    let mut ports: Vec<&PortObservation> = snap.ports.iter().collect();
    ports.sort_by(|a, b| a.index.cmp(&b.index).then_with(|| a.id.cmp(&b.id)));
    let notices = snap
        .notices
        .iter()
        .map(|n| {
            let code = match n {
                Notice::ServiceNotInstalled => "service_not_installed",
                Notice::UsbProbeOff => "usb_probe_off",
            };
            NoticeOut {
                code: code.to_string(),
                summary: ctx.i18n.t(&format!("notice-{code}"), &[]),
            }
        })
        .collect();
    Diagnosis {
        schema: SCHEMA_ID.to_string(),
        generated_at: ctx.generated_at.clone(),
        host: HostOut {
            os: snap.host.os.as_str().to_string(),
            os_version: snap.host.os_version.clone(),
            model: snap.host.model.clone(),
            arch: snap.host.arch.clone(),
            app_version: ctx.app_version.to_string(),
        },
        sources: SourcesOut {
            tier: match snap.tier {
                Tier::User => "user",
                Tier::Service => "service",
                Tier::Driver => "driver",
            }
            .to_string(),
            usb_probe: snap.usb_probe,
        },
        ports: ports
            .into_iter()
            .map(|p| diagnose_port(p, snap, ctx))
            .collect(),
        notices,
    }
}

// ---------------------------------------------------------------------------
// Facts
// ---------------------------------------------------------------------------

struct CableFacts {
    state: CableState,
    identity: Option<Identity>,
    raw: Option<RawIdentity>,
    source: Option<Source>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CableState {
    Unknown,
    Unavailable,
    NoEmarker,
    Emarker,
}

impl CableFacts {
    fn from(obs: &CableObservation) -> Self {
        match obs {
            CableObservation::Unknown => Self {
                state: CableState::Unknown,
                identity: None,
                raw: None,
                source: None,
            },
            CableObservation::Unavailable { .. } => Self {
                state: CableState::Unavailable,
                identity: None,
                raw: None,
                source: None,
            },
            CableObservation::NoEmarker { source } => Self {
                state: CableState::NoEmarker,
                identity: None,
                raw: None,
                source: Some(*source),
            },
            CableObservation::Emarker {
                identity, source, ..
            } => Self {
                state: CableState::Emarker,
                identity: Some(identity.decode(Sop::SopPrime)),
                raw: Some(identity.clone()),
                source: Some(*source),
            },
        }
    }

    fn speed(&self) -> Option<UsbGen> {
        self.identity
            .as_ref()
            .and_then(|i| i.cable.as_ref())
            .and_then(|c| c.usb_highest_speed)
    }

    fn power_w(&self) -> Option<f64> {
        self.identity
            .as_ref()
            .and_then(|i| i.cable.as_ref())
            .and_then(|c| c.power_class_w())
            .map(f64::from)
    }

    fn kind(&self) -> CableKind {
        self.identity
            .as_ref()
            .and_then(|i| i.cable.as_ref())
            .map_or(CableKind::Unknown, |c| c.kind)
    }

    /// Thunderbolt/USB4 rate the cable supports according to its marker.
    fn fabric_gbps(&self) -> Option<f64> {
        let speed = self.speed()?;
        Some(match speed {
            UsbGen::Usb2 | UsbGen::Gen1 => 10.0,
            UsbGen::Gen2 => 20.0,
            UsbGen::Gen3 => 40.0,
            UsbGen::Gen4 => 80.0,
        })
    }
}

struct PartnerFacts {
    identity: Option<Identity>,
}

impl PartnerFacts {
    fn from(p: Option<&PartnerObservation>) -> Self {
        Self {
            identity: p
                .and_then(|p| p.identity.as_ref())
                .filter(|i| !i.is_empty())
                .map(|i| i.decode(Sop::Sop)),
        }
    }

    fn data_max(&self) -> Option<UsbGen> {
        self.identity
            .as_ref()
            .and_then(|i| i.ufp.as_ref())
            .and_then(|u| u.max_data())
    }

    fn expects_data(&self) -> bool {
        self.identity.as_ref().is_some_and(|i| {
            i.header.usb_device_capable
                && matches!(
                    i.header.product_type_ufp,
                    ProductType::Peripheral | ProductType::Hub
                )
        })
    }
}

struct Draft {
    domain: Domain,
    limiter: Limiter,
    code: &'static str,
    args: Vec<(&'static str, Arg)>,
    evidence: Vec<Evidence>,
}

fn ev(field: &str, source: Option<Source>, value: serde_json::Value) -> Evidence {
    Evidence {
        field: field.to_string(),
        source: source.unwrap_or(Source::Derived),
        value: Some(value),
    }
}

fn gen_str(g: UsbGen) -> &'static str {
    match g {
        UsbGen::Usb2 => "usb2",
        UsbGen::Gen1 => "gen1",
        UsbGen::Gen2 => "gen2",
        UsbGen::Gen3 => "gen3",
        UsbGen::Gen4 => "gen4",
    }
}

fn gen_label(i18n: &I18n, g: UsbGen) -> String {
    i18n.t(&format!("usb-class-{}", gen_str(g)), &[])
}

// ---------------------------------------------------------------------------
// Port
// ---------------------------------------------------------------------------

fn diagnose_port(port: &PortObservation, snap: &Snapshot, ctx: &Context<'_>) -> PortDiagnosis {
    let i18n = ctx.i18n;
    let cable = CableFacts::from(&port.cable);
    let partner = PartnerFacts::from(port.partner.as_ref());
    let connected = port.connected;

    // Unavailable list: backend entries plus the ones the core can infer.
    let mut unavailable = port.unavailable.clone();
    let mut add_unavail = |field: &str, reason: Reason| {
        if !unavailable.iter().any(|u| u.field == field) {
            unavailable.push(Unavailable {
                field: field.to_string(),
                reason,
            });
        }
    };
    if let CableObservation::Unavailable { reason } = port.cable {
        add_unavail("cable", reason);
    }
    if let PowerObservation::Unavailable { reason } = port.power {
        add_unavail("power", reason);
    }
    if port.behind_internal_hub {
        add_unavail("cable", Reason::BehindInternalHub);
        add_unavail("power", Reason::BehindInternalHub);
    }
    unavailable.sort();

    // Transports, with evidence only.
    let mut transports: Vec<Transport> = port.link.transports.clone();
    for d in &port.link.usb_devices {
        let t = if d.link_gen().is_superspeed() {
            Transport::Usb3
        } else {
            Transport::Usb2
        };
        transports.push(t);
    }
    if port.link.displayport.is_some() && !transports.contains(&Transport::Displayport) {
        transports.push(Transport::Displayport);
    }
    transports.sort();
    transports.dedup();

    let negotiated = port
        .link
        .usb_devices
        .iter()
        .map(UsbDeviceObservation::link_gen)
        .max();
    let device_max = partner.data_max().or_else(|| {
        let all: Vec<Option<UsbGen>> = port
            .link
            .usb_devices
            .iter()
            .map(UsbDeviceObservation::proven_max)
            .collect();
        if !all.is_empty() && all.iter().all(Option::is_some) {
            all.into_iter().flatten().max()
        } else {
            None
        }
    });

    let mut drafts: Vec<Draft> = Vec::new();
    if connected != Some(false) {
        if let Some(d) = power_verdict(port, &cable, snap.battery.as_ref()) {
            drafts.push(d);
        }
        let has_fabric = port.link.fabric.is_some()
            && (transports.contains(&Transport::Usb4)
                || transports.contains(&Transport::Thunderbolt));
        let data = if has_fabric {
            fabric_verdict(i18n, port, &cable)
        } else {
            data_verdict(i18n, port, &cable, &partner, negotiated, device_max)
        };
        if let Some(d) = data {
            drafts.push(d);
        }
        if let Some(d) = display_verdict(port, &transports) {
            drafts.push(d);
        }
    }

    let headline = headline(port, &transports, &partner, &drafts, connected);

    // Render verdicts.
    let verdicts: Vec<Verdict> = drafts
        .iter()
        .map(|d| Verdict {
            domain: d.domain,
            limiter: d.limiter,
            code: d.code.to_string(),
            summary: i18n.t(&format!("verdict-{}", d.code), &d.args),
            evidence: d.evidence.clone(),
        })
        .collect();

    let faults: Vec<FaultOut> = port
        .faults
        .iter()
        .map(|f| FaultOut {
            kind: f.kind,
            count: f.count,
            last_at: f.last_at.clone(),
            banner: true,
            summary: i18n.t(
                &format!("fault-{}", f.kind.as_str()),
                &[("count", Arg::Num(f64::from(f.count)))],
            ),
        })
        .collect();

    let cable_out = cable_out(port, &cable, negotiated, device_max, ctx);
    let trust_warn = cable_out
        .as_ref()
        .and_then(|c| c.trust_flags.as_ref())
        .is_some_and(|f| !f.is_empty());

    let status = if !faults.is_empty() {
        Status::Fault
    } else if trust_warn || verdicts.iter().any(|v| WARN_CODES.contains(&v.code.as_str())) {
        Status::Warn
    } else {
        Status::Ok
    };

    let summary = summary_sentence(&verdicts, &drafts, headline, i18n);

    let mut notes = Vec::new();
    if !snap.usb_probe {
        notes.push(NoteOut {
            code: "usb_probe_off".into(),
            summary: i18n.t("note-usb_probe_off", &[]),
        });
    }
    if cable.state == CableState::Emarker && connected != Some(false) {
        notes.push(NoteOut {
            code: "emarker_is_self_reported".into(),
            summary: i18n.t("note-emarker_is_self_reported", &[]),
        });
    }

    let unavailable_out = unavailable
        .iter()
        .map(|u| UnavailableOut {
            field: u.field.clone(),
            reason: u.reason,
            summary: unavailable_text(i18n, &u.field, u.reason),
        })
        .collect();

    let (label, location) = port_label(port, i18n);

    PortDiagnosis {
        id: port.id.clone(),
        label,
        location,
        headline,
        headline_text: i18n.t(&format!("headline-{}", headline.as_str()), &[]),
        summary,
        status,
        cable: if connected == Some(false) {
            None
        } else {
            cable_out
        },
        power: if connected == Some(false) {
            None
        } else {
            power_out(port, i18n)
        },
        link: if connected == Some(false) {
            None
        } else {
            link_out(port, &transports, negotiated)
        },
        devices: devices_out(&port.link.usb_devices, ctx),
        displays: displays_out(port),
        verdicts,
        faults,
        unavailable: unavailable_out,
        notes,
        health: port.health.as_ref().map(|h| HealthOut {
            overcurrent_count: h.overcurrent_count,
            reset_count: h.reset_count,
            reconnect_count: h.reconnect_count,
            orientation: h.orientation.clone(),
            liquid_detected: h.liquid_detected,
            cable_resistance_mohm: h.cable_resistance_mohm(),
        }),
        raw: if ctx.include_raw {
            Some(raw_out(port, &cable, &partner))
        } else {
            None
        },
    }
}

fn port_label(port: &PortObservation, i18n: &I18n) -> (String, Option<String>) {
    let kind = match port.kind {
        PortKind::UsbC => "usb_c",
        PortKind::Usb4 => "usb4",
        PortKind::Thunderbolt => "thunderbolt",
    };
    let label = i18n.t(
        &format!("port-label-{kind}"),
        &[("index", Arg::Num(f64::from(port.index)))],
    );
    let location = port.location.as_ref().and_then(|l| {
        let mut parts = Vec::new();
        if let Some(p) = &l.panel {
            parts.push(i18n.t(&format!("location-panel-{p}"), &[]));
        }
        if let Some(h) = &l.horizontal {
            parts.push(i18n.t(&format!("location-horizontal-{h}"), &[]));
        }
        if let Some(v) = &l.vertical {
            parts.push(i18n.t(&format!("location-vertical-{v}"), &[]));
        }
        // Unknown location words come back as message ids; drop them.
        parts.retain(|p| !p.starts_with("location-"));
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(", "))
        }
    });
    (label, location)
}

fn unavailable_text(i18n: &I18n, field: &str, reason: Reason) -> String {
    let base = match (field, reason) {
        (_, Reason::NeedsService) => return i18n.t("unavailable-needs_service", &[]),
        (_, Reason::UsbProbeOff) => return i18n.t("note-usb_probe_off", &[]),
        ("power" | "power.pdos" | "power.contract", _) => i18n.t("unavailable-power", &[]),
        ("cable", _) => i18n.t("unavailable-cable", &[]),
        _ => i18n.t("unavailable-field", &[]),
    };
    let why = i18n.t(&format!("reason-{}", reason.as_str()), &[]);
    format!("{base} ({why})")
}

fn summary_sentence(
    verdicts: &[Verdict],
    drafts: &[Draft],
    headline: Headline,
    i18n: &I18n,
) -> String {
    if headline == Headline::Empty {
        return String::new();
    }
    let rank = |v: &Verdict| -> u8 {
        match v.limiter {
            Limiter::Cable | Limiter::Charger | Limiter::Port => 0,
            Limiter::Device => 1,
            Limiter::Unknown if WARN_CODES.contains(&v.code.as_str()) => 0,
            Limiter::Unknown => 2,
            Limiter::None => 3,
        }
    };
    let best = verdicts
        .iter()
        .zip(drafts)
        .min_by_key(|(v, _)| rank(v));
    match best {
        Some((v, d)) => {
            let lead = format!("lead-{}", v.code);
            if i18n.has(&lead) {
                i18n.t(&lead, &d.args)
            } else {
                v.summary.clone()
            }
        }
        None => i18n.t(&format!("summary-{}", headline.as_str()), &[]),
    }
}

// ---------------------------------------------------------------------------
// Headline
// ---------------------------------------------------------------------------

fn headline(
    port: &PortObservation,
    transports: &[Transport],
    partner: &PartnerFacts,
    drafts: &[Draft],
    connected: Option<bool>,
) -> Headline {
    if connected == Some(false) {
        return Headline::Empty;
    }
    if transports.contains(&Transport::Usb4) || transports.contains(&Transport::Thunderbolt) {
        return Headline::ThunderboltUsb4;
    }
    if transports.contains(&Transport::Displayport) || !port.displays.is_empty() {
        return Headline::Display;
    }
    let data_code = drafts
        .iter()
        .find(|d| d.domain == Domain::Data)
        .map(|d| d.code);
    if !port.link.usb_devices.is_empty() {
        let slow = matches!(data_code, Some("cable_likely_usb2"))
            || (matches!(data_code, Some("cable_limits_data"))
                && port.link.usb_devices.iter().all(|d| !d.link_gen().is_superspeed()));
        return if slow {
            Headline::SlowOrChargeOnly
        } else {
            Headline::UsbDevice
        };
    }
    let power_known = matches!(port.power, PowerObservation::Available(_));
    if connected.is_none() && port.partner.is_none() && !power_known {
        return Headline::Empty;
    }
    if data_code == Some("no_data_link") {
        return Headline::SlowOrChargeOnly;
    }
    let _ = partner;
    Headline::ChargingOnly
}

// ---------------------------------------------------------------------------
// Power
// ---------------------------------------------------------------------------

fn power_verdict(
    port: &PortObservation,
    cable: &CableFacts,
    battery: Option<&BatteryObservation>,
) -> Option<Draft> {
    let PowerObservation::Available(pd) = &port.power else {
        return None;
    };
    let src = Some(pd.source);
    let mut evidence = Vec::new();

    if pd.role == Some(PowerRole::Source) {
        let watts = pd
            .contract
            .as_ref()
            .map(|c| c.power_w)
            .or(pd.measured_w)
            .or(pd.typec_current.map(TypecCurrent::watts));
        let Some(watts) = watts else {
            return Some(Draft {
                domain: Domain::Power,
                limiter: Limiter::Unknown,
                code: "power_unknown",
                args: vec![],
                evidence,
            });
        };
        evidence.push(ev("power.contract_w", src, json!(watts)));
        return Some(Draft {
            domain: Domain::Power,
            limiter: Limiter::None,
            code: "power_supplying_device",
            args: vec![("watts", Arg::Num(watts))],
            evidence,
        });
    }

    let charger_max = pd.source_max_w();
    let contract_w = pd.contract.as_ref().map(|c| c.power_w);
    if charger_max.is_none() && contract_w.is_none() {
        if let Some(tc) = pd.typec_current {
            evidence.push(ev("power.typec_current", src, json!(tc)));
            return Some(Draft {
                domain: Domain::Power,
                limiter: Limiter::Unknown,
                code: "power_basic_only",
                args: vec![("watts", Arg::Num(tc.watts()))],
                evidence,
            });
        }
        return Some(Draft {
            domain: Domain::Power,
            limiter: Limiter::Unknown,
            code: "power_unknown",
            args: vec![],
            evidence,
        });
    }

    let cable_w = cable.power_w();
    let host_w = port.capabilities.sink_max_w.or(pd.sink_max_w());
    let drawn = pd.measured_w.or(contract_w);

    if let Some(c) = charger_max {
        evidence.push(ev("power.pdos", src, json!(c)));
    }
    if let Some(c) = contract_w {
        evidence.push(ev("power.contract_w", src, json!(c)));
    }
    if let Some(m) = pd.measured_w {
        evidence.push(ev("power.measured_w", src, json!(m)));
    }
    if let Some(c) = cable_w {
        evidence.push(ev("cable.power_w_max", cable.source, json!(c)));
    }
    if let Some(h) = host_w {
        evidence.push(ev(
            "port.sink_max_w",
            port.capabilities.source.or(src),
            json!(h),
        ));
    }

    if let Some(b) = battery
        && b.full == Some(true)
        && b.charging != Some(true)
    {
        evidence.push(ev("battery.full", Some(b.source), json!(true)));
        return Some(Draft {
            domain: Domain::Power,
            limiter: Limiter::None,
            code: "battery_full",
            args: vec![],
            evidence,
        });
    }

    let known: Vec<f64> = [charger_max, cable_w, host_w].into_iter().flatten().collect();
    let possible = known.iter().copied().fold(f64::INFINITY, f64::min);
    let offer = charger_max.unwrap_or(possible);

    if let Some(d) = drawn
        && possible.is_finite()
        && d < possible * 0.85 - 2.0
    {
        if let Some(b) = battery
            && let Some(p) = b.percent
        {
            evidence.push(ev("battery.percent", Some(b.source), json!(p)));
        }
        return Some(Draft {
            domain: Domain::Power,
            limiter: Limiter::None,
            code: "charging_below_max",
            args: vec![("watts", Arg::Num(d)), ("max", Arg::Num(offer))],
            evidence,
        });
    }

    let lt = |a: Option<f64>, b: Option<f64>| match (a, b) {
        (Some(x), Some(y)) => x + 0.5 < y,
        (Some(_), None) => true,
        _ => false,
    };
    let le = |a: Option<f64>, b: Option<f64>| match (a, b) {
        (Some(x), Some(y)) => x <= y + 0.5,
        (Some(_), None) => true,
        _ => false,
    };

    // Cable strictly below charger and host.
    if cable_w.is_some() && lt(cable_w, charger_max) && (host_w.is_none() || lt(cable_w, host_w)) {
        let others = [charger_max, host_w]
            .into_iter()
            .flatten()
            .fold(f64::INFINITY, f64::min);
        return Some(Draft {
            domain: Domain::Power,
            limiter: Limiter::Cable,
            code: "cable_limits_power",
            args: vec![
                ("watts", Arg::Num(cable_w.unwrap_or_default())),
                ("other", Arg::Num(others)),
            ],
            evidence,
        });
    }
    // Charger lowest (ties with the cable go to the charger).
    if charger_max.is_some()
        && le(charger_max, cable_w.or(Some(f64::INFINITY)))
        && host_w.is_some_and(|h| lt(charger_max, Some(h)))
    {
        return Some(Draft {
            domain: Domain::Power,
            limiter: Limiter::Charger,
            code: "charger_limits_power",
            args: vec![("watts", Arg::Num(charger_max.unwrap_or_default()))],
            evidence,
        });
    }
    // Charger below the cable and the host maximum is not known.
    if host_w.is_none() && cable_w.is_some() && lt(charger_max, cable_w) {
        return Some(Draft {
            domain: Domain::Power,
            limiter: Limiter::Charger,
            code: "charger_limits_power",
            args: vec![("watts", Arg::Num(charger_max.unwrap_or_default()))],
            evidence,
        });
    }
    // Host takes less than charger and cable could give.
    if let Some(h) = host_w
        && lt(Some(h), charger_max)
        && (cable_w.is_none() || lt(Some(h), cable_w))
    {
        return Some(Draft {
            domain: Domain::Power,
            limiter: Limiter::Port,
            code: "host_limits_power",
            args: vec![("watts", Arg::Num(h))],
            evidence,
        });
    }
    let watts = drawn.or(charger_max).unwrap_or(possible);
    Some(Draft {
        domain: Domain::Power,
        limiter: Limiter::None,
        code: "power_ok",
        args: vec![("watts", Arg::Num(watts))],
        evidence,
    })
}

// ---------------------------------------------------------------------------
// Data (USB)
// ---------------------------------------------------------------------------

fn data_verdict(
    i18n: &I18n,
    port: &PortObservation,
    cable: &CableFacts,
    partner: &PartnerFacts,
    negotiated: Option<UsbGen>,
    device_max: Option<UsbGen>,
) -> Option<Draft> {
    let link_src = port.link.source;
    let port_max = port.capabilities.usb;
    let cable_max = cable.speed();
    let mut evidence = Vec::new();
    if let Some(p) = port_max {
        evidence.push(ev(
            "port.usb",
            port.capabilities.source,
            json!(gen_str(p)),
        ));
    }
    if let Some(c) = cable_max {
        evidence.push(ev("cable.speed_gbps", cable.source, json!(c.cable_gbps())));
    }
    if let Some(d) = device_max {
        evidence.push(ev("device.max", link_src, json!(gen_str(d))));
    }

    let Some(neg) = negotiated else {
        // No USB device on the port.
        let data_visible = port.link.source.is_some();
        if partner.expects_data()
            && data_visible
            && !port.behind_internal_hub
            && port_max.is_some()
        {
            evidence.push(ev("link.usb_gbps", link_src, json!(null)));
            return Some(Draft {
                domain: Domain::Data,
                limiter: Limiter::Unknown,
                code: "no_data_link",
                args: vec![],
                evidence,
            });
        }
        return None;
    };
    evidence.push(ev("link.usb_gbps", link_src, json!(neg.link_gbps())));

    // 1. Device at its own maximum.
    if let Some(d) = device_max
        && neg >= d
    {
        let more_possible = cable_max.is_some_and(|c| c > d) || port_max.is_some_and(|p| p > d);
        if more_possible {
            let cable_state = if cable_max.is_some_and(|c| c >= d) {
                "ok"
            } else {
                "unknown"
            };
            return Some(Draft {
                domain: Domain::Data,
                limiter: Limiter::Device,
                code: "device_at_its_max",
                args: vec![
                    ("speed", Arg::Str(gen_label(i18n, neg))),
                    ("cable", Arg::Str(cable_state.to_string())),
                ],
                evidence,
            });
        }
        return Some(Draft {
            domain: Domain::Data,
            limiter: Limiter::None,
            code: "data_ok",
            args: vec![("speed", Arg::Str(gen_label(i18n, neg)))],
            evidence,
        });
    }

    // 2. Behind an internal hub: cable and port path are not visible.
    if port.behind_internal_hub {
        return Some(Draft {
            domain: Domain::Data,
            limiter: Limiter::Unknown,
            code: "data_via_internal_hub",
            args: vec![("speed", Arg::Str(gen_label(i18n, neg)))],
            evidence,
        });
    }

    let dev_above = device_max.is_some_and(|d| d > neg);
    let port_above = port_max.is_some_and(|p| p > neg);

    // 3. E-Marker says more, link stays below, port and device could do more.
    if let Some(c) = cable_max {
        if c <= neg {
            // The cable is the lowest element, if port and device are higher.
            if port_above && dev_above {
                return Some(Draft {
                    domain: Domain::Data,
                    limiter: Limiter::Cable,
                    code: "cable_limits_data",
                    args: vec![("speed", Arg::Str(gen_label(i18n, c)))],
                    evidence,
                });
            }
        } else if port_above && dev_above {
            return Some(Draft {
                domain: Domain::Data,
                limiter: Limiter::Unknown,
                code: "emarker_link_mismatch",
                args: vec![
                    ("speed", Arg::Str(gen_label(i18n, neg))),
                    ("marker", Arg::Str(gen_label(i18n, c))),
                ],
                evidence,
            });
        }
    }

    // 4. Port is the lowest element.
    if let Some(p) = port_max
        && !port.capabilities.usb_is_lower_bound
        && p <= neg
        && dev_above
        && cable_max.is_none_or(|c| c > p)
    {
        return Some(Draft {
            domain: Domain::Data,
            limiter: Limiter::Port,
            code: "port_limits_data",
            args: vec![("speed", Arg::Str(gen_label(i18n, p)))],
            evidence,
        });
    }

    // 5. No E-Marker, USB 2.0 link while port and device do SuperSpeed.
    if cable.state == CableState::NoEmarker && neg == UsbGen::Usb2 && port_above && dev_above {
        return Some(Draft {
            domain: Domain::Data,
            limiter: Limiter::Cable,
            code: "cable_likely_usb2",
            args: vec![],
            evidence,
        });
    }

    // 6. Everything that is known matches the link.
    let all_known_at_link = [port_max, cable_max, device_max]
        .into_iter()
        .flatten()
        .all(|x| x <= neg);
    if all_known_at_link && device_max.is_some() {
        return Some(Draft {
            domain: Domain::Data,
            limiter: Limiter::None,
            code: "data_ok",
            args: vec![("speed", Arg::Str(gen_label(i18n, neg)))],
            evidence,
        });
    }
    Some(Draft {
        domain: Domain::Data,
        limiter: Limiter::Unknown,
        code: "data_unknown",
        args: vec![("speed", Arg::Str(gen_label(i18n, neg)))],
        evidence,
    })
}

// ---------------------------------------------------------------------------
// Data (Thunderbolt / USB4)
// ---------------------------------------------------------------------------

fn fabric_verdict(i18n: &I18n, port: &PortObservation, cable: &CableFacts) -> Option<Draft> {
    let fabric = port.link.fabric.as_ref()?;
    let src = Some(fabric.source);
    let link = fabric.link_gbps();
    let port_gbps = port.capabilities.fabric.map(FabricGen::max_gbps);
    let marker_gbps = cable.fabric_gbps();
    let device_gbps = fabric
        .switches
        .iter()
        .find(|s| s.depth == 1)
        .and_then(|s| s.generation)
        .map(FabricGen::max_gbps);

    let mut evidence = Vec::new();
    if let Some(l) = link {
        evidence.push(ev("link.fabric_gbps", src, json!(l)));
        evidence.push(ev(
            "link.fabric_measured",
            src,
            json!(fabric.measured_by_controller),
        ));
    }
    if let Some(p) = port_gbps {
        evidence.push(ev("port.fabric_gbps", port.capabilities.source, json!(p)));
    }
    if let Some(m) = marker_gbps {
        evidence.push(ev("cable.speed_gbps", cable.source, json!(m)));
    }
    if let Some(d) = device_gbps {
        evidence.push(ev("device.fabric_gbps", src, json!(d)));
    }

    let Some(link) = link else {
        return Some(Draft {
            domain: Domain::Data,
            limiter: Limiter::Unknown,
            code: "data_unknown",
            args: vec![("speed", Arg::Str(i18n.t("speed-fabric", &[])))],
            evidence,
        });
    };
    let gb = |v: f64| Arg::Num(v);

    // Device generation is the lowest element and the link runs at it.
    if let Some(d) = device_gbps
        && link + 0.5 >= d
        && (marker_gbps.is_some_and(|m| m > d) || port_gbps.is_some_and(|p| p > d))
    {
        return Some(Draft {
            domain: Domain::Data,
            limiter: Limiter::Device,
            code: "device_at_its_max",
            args: vec![
                ("speed", Arg::Str(i18n.t("speed-gbps", &[("gbps", Arg::Num(link))]))),
                (
                    "cable",
                    Arg::Str(if marker_gbps.is_some_and(|m| m >= d) {
                        "ok".into()
                    } else {
                        "unknown".into()
                    }),
                ),
            ],
            evidence,
        });
    }

    let below_marker = marker_gbps.is_some_and(|m| link + 0.5 < m);
    let below_port = port_gbps.is_some_and(|p| link + 0.5 < p);
    let device_ok = device_gbps.is_none_or(|d| link + 0.5 < d);
    if below_marker && below_port && device_ok {
        if fabric.measured_by_controller && device_gbps.is_some() {
            return Some(Draft {
                domain: Domain::Data,
                limiter: Limiter::Cable,
                code: "fabric_link_below_cable",
                args: vec![
                    ("link", gb(link)),
                    ("marker", gb(marker_gbps.unwrap_or_default())),
                ],
                evidence,
            });
        }
        return Some(Draft {
            domain: Domain::Data,
            limiter: Limiter::Unknown,
            code: "fabric_link_unverified",
            args: vec![
                ("link", gb(link)),
                ("marker", gb(marker_gbps.unwrap_or_default())),
            ],
            evidence,
        });
    }
    // The cable marker is the lowest element and the link runs at it.
    if let Some(m) = marker_gbps
        && link + 0.5 >= m
        && port_gbps.is_some_and(|p| p > m + 0.5)
        && device_gbps.is_some_and(|d| d > m + 0.5)
    {
        return Some(Draft {
            domain: Domain::Data,
            limiter: Limiter::Cable,
            code: "cable_limits_data",
            args: vec![("speed", Arg::Str(i18n.t("speed-gbps", &[("gbps", Arg::Num(m))])))],
            evidence,
        });
    }
    let all_at_link = [port_gbps, marker_gbps, device_gbps]
        .into_iter()
        .flatten()
        .all(|x| link + 0.5 >= x);
    if all_at_link && (marker_gbps.is_some() || device_gbps.is_some()) {
        return Some(Draft {
            domain: Domain::Data,
            limiter: Limiter::None,
            code: "data_ok",
            args: vec![("speed", Arg::Str(i18n.t("speed-gbps", &[("gbps", Arg::Num(link))])))],
            evidence,
        });
    }
    Some(Draft {
        domain: Domain::Data,
        limiter: Limiter::Unknown,
        code: "data_unknown",
        args: vec![("speed", Arg::Str(i18n.t("speed-gbps", &[("gbps", Arg::Num(link))])))],
        evidence,
    })
}

// ---------------------------------------------------------------------------
// Display
// ---------------------------------------------------------------------------

fn dp_lanes_consistent(port: &PortObservation, transports: &[Transport]) -> Option<u8> {
    let dp = port.link.displayport.as_ref()?;
    let lanes = dp.lanes?;
    // Contradiction rule: with USB 3 active at the same time, DP Alt Mode
    // can use at most 2 lanes (pin assignment D). Drop impossible counts.
    if !dp.tunneled && transports.contains(&Transport::Usb3) && lanes > 2 {
        return None;
    }
    Some(lanes)
}

fn display_verdict(port: &PortObservation, transports: &[Transport]) -> Option<Draft> {
    if port.displays.is_empty() {
        return None;
    }
    let dp = port.link.displayport.as_ref();
    let lanes = dp_lanes_consistent(port, transports);
    let link_gbps = match (lanes, dp.and_then(|d| d.lane_gbps)) {
        (Some(l), Some(r)) => Some(display::dp_payload_gbps(l, r)),
        _ => None,
    };
    let mut worst: Option<Draft> = None;
    for d in &port.displays {
        let src = Some(d.source);
        let mut evidence = Vec::new();
        let (Some(cur), Some(max)) = (d.current, d.max) else {
            worst.get_or_insert(Draft {
                domain: Domain::Display,
                limiter: Limiter::Unknown,
                code: "display_unknown",
                args: vec![],
                evidence,
            });
            continue;
        };
        evidence.push(ev("displays.current", src, json!(cur)));
        evidence.push(ev("displays.max", src, json!(max)));
        if !display::below(&cur, &max) {
            if worst.is_none() {
                worst = Some(Draft {
                    domain: Domain::Display,
                    limiter: Limiter::None,
                    code: "display_at_max",
                    args: vec![],
                    evidence,
                });
            }
            continue;
        }
        let need = display::required_gbps(&max, d.dsc == Some(true));
        evidence.push(ev("displays.required_gbps", Some(Source::Derived), json!(need)));
        let draft = if let Some(cap) = link_gbps {
            evidence.push(ev(
                "link.displayport_gbps",
                dp.map(|x| x.source),
                json!(cap),
            ));
            if need > cap {
                let shared = lanes == Some(2) && transports.contains(&Transport::Usb3);
                if shared {
                    Draft {
                        domain: Domain::Display,
                        limiter: Limiter::Device,
                        code: "display_lanes_shared",
                        args: vec![],
                        evidence,
                    }
                } else {
                    Draft {
                        domain: Domain::Display,
                        limiter: Limiter::Unknown,
                        code: "display_link_too_slow",
                        args: vec![
                            ("need", Arg::Num((need * 10.0).round() / 10.0)),
                            ("cap", Arg::Num((cap * 10.0).round() / 10.0)),
                        ],
                        evidence,
                    }
                }
            } else {
                Draft {
                    domain: Domain::Display,
                    limiter: Limiter::None,
                    code: "display_below_max",
                    args: vec![],
                    evidence,
                }
            }
        } else {
            Draft {
                domain: Domain::Display,
                limiter: Limiter::Unknown,
                code: "display_unknown",
                args: vec![],
                evidence,
            }
        };
        let severity = |c: &str| match c {
            "display_lanes_shared" | "display_link_too_slow" => 3,
            "display_below_max" => 2,
            "display_unknown" => 1,
            _ => 0,
        };
        if worst
            .as_ref()
            .is_none_or(|w| severity(draft.code) > severity(w.code))
        {
            worst = Some(draft);
        }
    }
    worst
}

// ---------------------------------------------------------------------------
// Output sections
// ---------------------------------------------------------------------------

fn hex4(v: u16) -> String {
    format!("{v:04x}")
}

fn cable_out(
    port: &PortObservation,
    cable: &CableFacts,
    negotiated: Option<UsbGen>,
    device_max: Option<UsbGen>,
    ctx: &Context<'_>,
) -> Option<CableOut> {
    let i18n = ctx.i18n;
    match cable.state {
        CableState::Unknown | CableState::Unavailable => None,
        CableState::NoEmarker => Some(CableOut {
            has_emarker: false,
            kind: None,
            speed_gbps: None,
            current_a: None,
            power_w_max: None,
            vid: None,
            pid: None,
            vendor_name: None,
            xid: None,
            certification: None,
            certified_name: None,
            trust_flags: None,
            summary: i18n.t("cable-no-emarker", &[]),
            known_as: None,
            history: None,
        }),
        CableState::Emarker => {
            let id = cable.identity.as_ref()?;
            let raw = cable.raw.as_ref()?;
            let vdo = id.cable.as_ref();
            let vid = id.header.vendor_id;
            let fingerprint = cable_fingerprint(raw);
            let certification = id.xid.map(|x| {
                if ctx.catalog.certified_name(x).is_some() {
                    "listed".to_string()
                } else {
                    "unlisted".to_string()
                }
            });
            let certified_name = id.xid.and_then(|x| ctx.catalog.certified_name(x));
            let mut flags = trust_flags(id, raw, port, negotiated, device_max, cable, i18n);
            flags.sort_by(|a, b| a.code.cmp(&b.code));
            let kind = cable.kind();
            let speed = vdo.and_then(|c| c.usb_highest_speed).map(UsbGen::cable_gbps);
            let current = vdo.and_then(|c| c.current_a);
            let watts = vdo.and_then(|c| c.power_class_w());
            let mut parts = vec![i18n.t(&format!("cable-kind-{}", kind.as_str()), &[])];
            if let Some(s) = speed {
                parts.push(i18n.t("cable-speed", &[("gbps", Arg::Num(s))]));
            }
            if let (Some(a), Some(w)) = (current, watts) {
                parts.push(i18n.t(
                    "cable-power",
                    &[("amps", Arg::Num(a)), ("watts", Arg::Num(f64::from(w)))],
                ));
            }
            Some(CableOut {
                has_emarker: true,
                kind: Some(kind.as_str().to_string()),
                speed_gbps: speed,
                current_a: current,
                power_w_max: watts,
                vid: Some(hex4(vid)),
                pid: Some(hex4(id.product.product_id)),
                vendor_name: Some(ctx.catalog.vendor_name(vid)),
                xid: Some(id.xid.map(|x| format!("{x:08x}"))),
                certification: Some(certification),
                certified_name,
                trust_flags: Some(flags),
                summary: parts.join(", "),
                known_as: ctx.catalog.known_cable(&fingerprint),
                history: ctx.history.and_then(|h| h.cable(&fingerprint)),
            })
        }
    }
}

fn trust_flags(
    id: &Identity,
    raw: &RawIdentity,
    port: &PortObservation,
    negotiated: Option<UsbGen>,
    device_max: Option<UsbGen>,
    cable: &CableFacts,
    i18n: &I18n,
) -> Vec<TrustFlag> {
    let mut out = Vec::new();
    let mut push = |code: &str| {
        out.push(TrustFlag {
            code: code.to_string(),
            summary: i18n.t(&format!("trust-{code}"), &[]),
        })
    };
    if id.header.vendor_id == 0 {
        push("vid_zero");
    }
    let vdo = id.cable.as_ref();
    let reserved = id.header.reserved_set || vdo.is_some_and(|c| !c.reserved_set.is_empty());
    if reserved {
        push("reserved_bits");
    }
    if raw.product_type_vdos.first().copied().unwrap_or(0) == 0 {
        push("cable_vdo_missing");
    }
    if let Some(c) = vdo {
        if c.epr_capable && c.current_a != Some(5.0) {
            push("epr_without_5a");
        }
        if c.epr_capable && c.max_vbus_v < 50 {
            push("epr_without_50v");
        }
        if c.usb4_supported == Some(true) && c.usb_highest_speed == Some(UsbGen::Usb2) {
            push("usb4_flag_on_usb2");
        }
    }
    // Marker claims more speed, the link stays below although port and
    // device could do more. Only a hint; the verdict stays at "unknown".
    if let (Some(neg), Some(marker)) = (negotiated, cable.speed())
        && marker > neg
        && port.capabilities.usb.is_some_and(|p| p > neg)
        && device_max.is_some_and(|d| d > neg)
        && !port.behind_internal_hub
    {
        push("link_below_marker");
    }
    out
}

fn power_out(port: &PortObservation, i18n: &I18n) -> Option<PowerOut> {
    match &port.power {
        PowerObservation::Unknown => None,
        PowerObservation::Unavailable { .. } => Some(PowerOut {
            available: false,
            role: None,
            pdos: None,
            contract_w: None,
            contract_v: None,
            contract_a: None,
            measured_w: None,
            basic_only: false,
            typec_current: None,
            summary: Some(i18n.t("unavailable-power", &[])),
        }),
        PowerObservation::Available(pd) => {
            let selected = pd.contract.as_ref().and_then(|c| c.pdo_index);
            let pdos: Vec<PdoOut> = pd
                .source_pdos
                .iter()
                .enumerate()
                .map(|(i, p)| pdo_out(p, selected == Some((i + 1) as u8)))
                .collect();
            let basic_only = pd.source_pdos.is_empty() && pd.contract.is_none();
            let summary = if let Some(c) = &pd.contract {
                i18n.t(
                    "power-contract-detail",
                    &[
                        ("watts", Arg::Num(c.power_w)),
                        ("volts", Arg::Num(c.voltage_v)),
                        ("amps", Arg::Num(c.current_a)),
                    ],
                )
            } else if let Some(tc) = pd.typec_current {
                i18n.t("power-basic", &[("watts", Arg::Num(tc.watts()))])
            } else {
                i18n.t("power-no-contract", &[])
            };
            Some(PowerOut {
                available: true,
                role: pd.role.map(|r| {
                    match r {
                        PowerRole::Sink => "sink",
                        PowerRole::Source => "source",
                    }
                    .to_string()
                }),
                pdos: if pdos.is_empty() { None } else { Some(pdos) },
                contract_w: Some(pd.contract.as_ref().map(|c| c.power_w)),
                contract_v: pd.contract.as_ref().map(|c| c.voltage_v),
                contract_a: pd.contract.as_ref().map(|c| c.current_a),
                measured_w: pd.measured_w,
                basic_only,
                typec_current: pd.typec_current.map(|t| {
                    match t {
                        TypecCurrent::Default => "default",
                        TypecCurrent::A1_5 => "1.5a",
                        TypecCurrent::A3 => "3a",
                    }
                    .to_string()
                }),
                summary: Some(summary),
            })
        }
    }
}

fn pdo_out(p: &Pdo, selected: bool) -> PdoOut {
    let (voltage, min_voltage) = match *p {
        Pdo::Fixed { voltage_v, .. } => (Some(voltage_v), None),
        Pdo::Battery {
            min_voltage_v,
            max_voltage_v,
            ..
        }
        | Pdo::Variable {
            min_voltage_v,
            max_voltage_v,
            ..
        }
        | Pdo::Pps {
            min_voltage_v,
            max_voltage_v,
            ..
        }
        | Pdo::EprAvs {
            min_voltage_v,
            max_voltage_v,
            ..
        } => (Some(max_voltage_v), Some(min_voltage_v)),
        Pdo::SprAvs { .. } => (p.max_voltage_v(), Some(15.0)),
        Pdo::Unknown { .. } => (None, None),
    };
    PdoOut {
        kind: p.kind_str().to_string(),
        voltage_v: voltage,
        min_voltage_v: min_voltage,
        current_a: p.max_current_a(),
        max_w: p.max_w(),
        selected,
    }
}

fn link_out(
    port: &PortObservation,
    transports: &[Transport],
    negotiated: Option<UsbGen>,
) -> Option<LinkOut> {
    if transports.is_empty() && port.link.fabric.is_none() {
        return None;
    }
    let usb_gbps = port
        .link
        .usb_devices
        .iter()
        .map(|d| d.speed_mbps / 1000.0)
        .fold(None, |acc: Option<f64>, v| Some(acc.map_or(v, |a| a.max(v))));
    let _ = negotiated;
    let fabric = port.link.fabric.as_ref();
    Some(LinkOut {
        transports: transports.to_vec(),
        usb_gbps,
        thunderbolt_gen: fabric
            .and_then(|f| f.generation)
            .map(|g| g.as_str().to_string()),
        lanes: fabric.and_then(|f| f.lanes),
        fabric_gbps: fabric.and_then(|f| f.link_gbps()),
        fabric_measured: fabric.map(|f| f.measured_by_controller),
        displayport_lanes: dp_lanes_consistent(port, transports),
        topology: fabric
            .map(|f| {
                f.switches
                    .iter()
                    .map(|s| FabricHopOut {
                        depth: s.depth,
                        name: match (&s.vendor, &s.name) {
                            (Some(v), Some(n)) => Some(format!("{v} {n}")),
                            (None, Some(n)) => Some(n.clone()),
                            (Some(v), None) => Some(v.clone()),
                            _ => None,
                        },
                        generation: s.generation.map(|g| g.as_str().to_string()),
                        link_gbps: s.link_gbps,
                    })
                    .collect()
            })
            .unwrap_or_default(),
    })
}

fn devices_out(devs: &[UsbDeviceObservation], ctx: &Context<'_>) -> Vec<DeviceOut> {
    fn walk(
        d: &UsbDeviceObservation,
        depth: u8,
        ctx: &Context<'_>,
        out: &mut Vec<DeviceOut>,
    ) {
        let vendor = d
            .vendor
            .clone()
            .or_else(|| d.vendor_id.and_then(|v| ctx.catalog.vendor_name(v)));
        out.push(DeviceOut {
            vendor,
            product: d.product.clone(),
            via: if depth == 0 { "direct" } else { "hub" }.to_string(),
            usb_gbps: d.speed_mbps / 1000.0,
            depth,
            is_hub: d.is_hub,
        });
        for c in &d.children {
            walk(c, depth.saturating_add(1), ctx, out);
        }
    }
    let mut out = Vec::new();
    for d in devs {
        walk(d, 0, ctx, &mut out);
    }
    out
}

fn displays_out(port: &PortObservation) -> Vec<DisplayOut> {
    let dp = port.link.displayport.as_ref();
    let link = match (dp.and_then(|d| d.lanes), dp.and_then(|d| d.lane_gbps)) {
        (Some(l), Some(r)) => Some(display::dp_payload_gbps(l, r)),
        _ => None,
    };
    port.displays
        .iter()
        .map(|d| DisplayOut {
            name: d.name.clone(),
            current: d.current,
            max: d.max,
            dsc: d.dsc,
            adapter: d.adapter.clone(),
            link_gbps: link,
            required_gbps: d
                .max
                .map(|m| display::required_gbps(&m, d.dsc == Some(true))),
        })
        .collect()
}

fn raw_out(port: &PortObservation, cable: &CableFacts, partner: &PartnerFacts) -> serde_json::Value {
    let mut v = serde_json::Map::new();
    if let Some(raw) = &cable.raw {
        v.insert(
            "cable_vdos".into(),
            json!({
                "id_header": format!("0x{:08x}", raw.id_header),
                "cert_stat": format!("0x{:08x}", raw.cert_stat),
                "product": format!("0x{:08x}", raw.product),
                "product_type_vdos": raw.product_type_vdos.iter().map(|x| format!("0x{x:08x}")).collect::<Vec<_>>(),
                "fingerprint": cable_fingerprint(raw),
            }),
        );
    }
    if let Some(id) = &cable.identity {
        v.insert("cable_decoded".into(), json!(id));
    }
    if let Some(id) = &partner.identity {
        v.insert("partner_decoded".into(), json!(id));
    }
    if let PowerObservation::Available(pd) = &port.power {
        v.insert("source_pdos".into(), json!(pd.source_pdos));
        if !pd.sink_pdos.is_empty() {
            v.insert("sink_pdos".into(), json!(pd.sink_pdos));
        }
    }
    if let Some(f) = &port.link.fabric {
        v.insert("fabric".into(), json!(f));
    }
    if !port.raw.is_empty() {
        v.insert("platform".into(), json!(port.raw));
    }
    serde_json::Value::Object(v)
}
