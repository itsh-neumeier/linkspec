//! Negotiation diagnostics (prompt 5.7): port vs. cable vs. device vs.
//! negotiated, per domain, with the weakest link marked. Shared by the app
//! window and the TUI dashboard.

use serde::Serialize;

use crate::diagnosis::{Domain, Limiter, PortDiagnosis};
use crate::i18n::{Arg, I18n};
use crate::observation::{CableObservation, PortObservation, PowerObservation};
use crate::pd::Sop;
use crate::types::{Transport, UsbGen};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NegotiationRow {
    pub domain: Domain,
    pub port: Option<String>,
    pub cable: Option<String>,
    pub device: Option<String>,
    pub negotiated: Option<String>,
    /// Column of the limiter: `port`, `cable`, `device` (charger is shown
    /// in the device column for power) or none.
    pub weakest: Option<&'static str>,
}

fn gbps(i18n: &I18n, v: f64) -> String {
    i18n.t("speed-gbps", &[("gbps", Arg::Num(v))])
}

fn watts(i18n: &I18n, v: f64) -> String {
    format!("{} W", i18n.num(v))
}

fn class(i18n: &I18n, g: UsbGen) -> String {
    let key = match g {
        UsbGen::Usb2 => "usb2",
        UsbGen::Gen1 => "gen1",
        UsbGen::Gen2 => "gen2",
        UsbGen::Gen3 => "gen3",
        UsbGen::Gen4 => "gen4",
    };
    i18n.t(&format!("usb-class-{key}"), &[])
}

pub fn rows(obs: &PortObservation, diag: &PortDiagnosis, i18n: &I18n) -> Vec<NegotiationRow> {
    let weakest = |domain: Domain| -> Option<&'static str> {
        diag.verdicts
            .iter()
            .find(|v| v.domain == domain)
            .and_then(|v| match v.limiter {
                Limiter::Port => Some("port"),
                Limiter::Cable => Some("cable"),
                Limiter::Charger | Limiter::Device => Some("device"),
                _ => None,
            })
    };
    let cable_id = match &obs.cable {
        CableObservation::Emarker { identity, .. } => Some(identity.decode(Sop::SopPrime)),
        _ => None,
    };
    let cable_vdo = cable_id.as_ref().and_then(|c| c.cable.clone());
    let mut out = Vec::new();

    // Power.
    if let PowerObservation::Available(pd) = &obs.power {
        out.push(NegotiationRow {
            domain: Domain::Power,
            port: obs
                .capabilities
                .sink_max_w
                .or(pd.sink_max_w())
                .map(|w| watts(i18n, w)),
            cable: cable_vdo
                .as_ref()
                .and_then(|c| c.power_class_w())
                .map(|w| watts(i18n, f64::from(w))),
            device: pd.source_max_w().map(|w| watts(i18n, w)),
            negotiated: pd
                .contract
                .as_ref()
                .map(|c| watts(i18n, c.power_w))
                .or(pd.measured_w.map(|w| watts(i18n, w))),
            weakest: weakest(Domain::Power),
        });
    }

    // Data.
    let negotiated_usb = obs
        .link
        .usb_devices
        .iter()
        .map(|d| d.speed_mbps)
        .fold(None, |a: Option<f64>, v| Some(a.map_or(v, |x| x.max(v))));
    if let Some(f) = &obs.link.fabric {
        out.push(NegotiationRow {
            domain: Domain::Data,
            port: obs.capabilities.fabric.map(|g| gbps(i18n, g.max_gbps())),
            cable: cable_vdo
                .as_ref()
                .and_then(|c| c.usb_highest_speed)
                .map(|s| gbps(i18n, s.cable_gbps())),
            device: f
                .switches
                .iter()
                .find(|s| s.depth == 1)
                .and_then(|s| s.generation)
                .map(|g| gbps(i18n, g.max_gbps())),
            negotiated: f.link_gbps().map(|g| gbps(i18n, g)),
            weakest: weakest(Domain::Data),
        });
    } else if negotiated_usb.is_some() || obs.capabilities.usb.is_some() {
        let device_max = obs
            .link
            .usb_devices
            .iter()
            .filter_map(|d| d.proven_max())
            .max();
        out.push(NegotiationRow {
            domain: Domain::Data,
            port: obs.capabilities.usb.map(|g| class(i18n, g)),
            cable: cable_vdo
                .as_ref()
                .and_then(|c| c.usb_highest_speed)
                .map(|s| gbps(i18n, s.cable_gbps())),
            device: device_max.map(|g| class(i18n, g)),
            negotiated: negotiated_usb.map(|m| gbps(i18n, m / 1000.0)),
            weakest: weakest(Domain::Data),
        });
    }

    // Display.
    if obs.link.transports.contains(&Transport::Displayport) || !obs.displays.is_empty() {
        let d = obs.displays.first();
        let mode = |m: &crate::observation::DisplayMode| {
            format!("{}×{} @ {} Hz", m.width, m.height, i18n.num(m.refresh_hz))
        };
        let link = diag.displays.first().and_then(|d| d.link_gbps);
        out.push(NegotiationRow {
            domain: Domain::Display,
            port: obs
                .link
                .displayport
                .as_ref()
                .and_then(|dp| dp.lanes)
                .map(|l| format!("{l} × DP")),
            cable: link.map(|g| gbps(i18n, g)),
            device: d.and_then(|d| d.max.as_ref()).map(mode),
            negotiated: d.and_then(|d| d.current.as_ref()).map(mode),
            weakest: weakest(Domain::Display),
        });
    }
    out
}
