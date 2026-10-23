//! Worker thread: owns the engine, reacts to platform change events and UI
//! commands, and sends ready-to-show data to the UI thread. The power
//! monitor polls once per second only while its view is visible.

use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::{Duration, Instant};

use linkspec_core::diagnosis::{Domain, Status};
use linkspec_core::negotiation;
use linkspec_core::observation::FaultKind;
use linkspec_core::{I18n, Lang};
use linkspec_engine::{Engine, RunResult};
use linkspec_history::CableRecord;
use linkspec_platform::ChangeEvent;

pub enum Command {
    Refresh,
    Dismiss(String, FaultKind),
    SetLang(Lang),
    SetProbe(bool),
    Monitor(bool),
    Rename(String, String),
    Forget(String),
    Report,
    Quit,
}

#[derive(Debug, Clone)]
pub struct VerdictData {
    pub domain: String,
    pub text: String,
    pub warn: bool,
}

#[derive(Debug, Clone)]
pub struct PortData {
    pub id: String,
    pub label: String,
    pub location: String,
    pub headline: String,
    pub summary: String,
    pub status: i32,
    pub empty: bool,
    pub cable: String,
    pub power: String,
    pub verdicts: Vec<VerdictData>,
    pub banners: Vec<(String, String)>,
    pub devices: Vec<(String, i32)>,
    pub notes: Vec<String>,
    pub details: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct NegData {
    pub port_label: String,
    pub domain: String,
    pub port: String,
    pub cable: String,
    pub device: String,
    pub negotiated: String,
    pub weakest: i32,
}

#[derive(Debug, Clone)]
pub struct DisplayData {
    pub port_label: String,
    pub name: String,
    pub current: String,
    pub max: String,
    pub link: String,
    pub verdict: String,
}

pub enum Update {
    Data {
        ports: Vec<PortData>,
        /// USB devices outside the Type-C ports: (label, depth).
        other_usb: Vec<(String, i32)>,
        negotiation: Vec<NegData>,
        displays: Vec<DisplayData>,
        cables: Vec<CableRecord>,
        service: String,
        worst: Status,
        widget: serde_json::Value,
    },
    Monitor {
        now: String,
        path: String,
        note: String,
    },
    Fault {
        title: String,
        body: String,
    },
    Report(Option<String>),
}

pub fn spawn(mut engine: Engine, send: impl Fn(Update) + Send + 'static) -> Sender<Command> {
    let (tx, rx) = channel();
    let self_tx = tx.clone();
    std::thread::Builder::new()
        .name("linkspec-worker".into())
        .spawn(move || run(&mut engine, rx, self_tx, send))
        .expect("worker thread");
    tx
}

fn run(engine: &mut Engine, rx: Receiver<Command>, tx: Sender<Command>, send: impl Fn(Update)) {
    // Platform change events become refresh commands, so the worker sleeps
    // in `recv` while nothing happens (no polling in idle).
    if let Ok(watch) = engine.source_mut().watch() {
        std::thread::Builder::new()
            .name("linkspec-watch".into())
            .spawn(move || {
                while let Ok(ev) = watch.recv() {
                    if ev == ChangeEvent::Changed && tx.send(Command::Refresh).is_err() {
                        return;
                    }
                }
            })
            .ok();
    }
    let mut monitor = false;
    let mut samples: Vec<f64> = Vec::new();
    let mut last_sample = Instant::now();
    let mut last: Option<RunResult> = None;
    refresh(engine, &send, &mut last, true);
    loop {
        let msg = if monitor {
            rx.recv_timeout(Duration::from_millis(1000).saturating_sub(last_sample.elapsed()))
        } else {
            rx.recv().map_err(|_| RecvTimeoutError::Disconnected)
        };
        match msg {
            Ok(Command::Quit) | Err(RecvTimeoutError::Disconnected) => return,
            Ok(Command::Refresh) => refresh(engine, &send, &mut last, false),
            Ok(Command::Dismiss(id, kind)) => {
                engine.dismiss(&id, kind);
                refresh(engine, &send, &mut last, false);
            }
            Ok(Command::SetLang(l)) => {
                engine.set_lang(l);
                refresh(engine, &send, &mut last, false);
            }
            Ok(Command::SetProbe(p)) => {
                engine.opts.usb_probe = p;
                refresh(engine, &send, &mut last, false);
            }
            Ok(Command::Monitor(on)) => {
                monitor = on;
                samples.clear();
                last_sample = Instant::now() - Duration::from_secs(1);
            }
            Ok(Command::Rename(fp, name)) => {
                if let Some(h) = engine.history() {
                    let _ = h.rename(&fp, Some(&name));
                }
                refresh(engine, &send, &mut last, false);
            }
            Ok(Command::Forget(fp)) => {
                if let Some(h) = engine.history() {
                    let _ = h.forget(&fp);
                }
                refresh(engine, &send, &mut last, false);
            }
            Ok(Command::Report) => {
                let url = last
                    .as_ref()
                    .and_then(|r| report_url(&r.snapshot, &r.diagnosis));
                send(Update::Report(url));
            }
            Err(RecvTimeoutError::Timeout) => {}
        }
        if monitor && last_sample.elapsed() >= Duration::from_millis(1000) {
            last_sample = Instant::now();
            let none_text = engine.i18n().t("cli-monitor-none", &[]);
            match engine.source_mut().power_sample() {
                Ok(Some(s)) => {
                    if let Some(w) = s.watts {
                        samples.push(w);
                        if samples.len() > 120 {
                            samples.remove(0);
                        }
                    }
                    let f = |v: Option<f64>| {
                        v.map(|x| i18n_num(engine.i18n(), x))
                            .unwrap_or_else(|| "–".into())
                    };
                    send(Update::Monitor {
                        now: format!("{} W · {} V · {} A", f(s.watts), f(s.volts), f(s.amps)),
                        path: graph_path(&samples),
                        note: String::new(),
                    });
                }
                _ => send(Update::Monitor {
                    now: "–".into(),
                    path: String::new(),
                    note: none_text,
                }),
            }
        }
    }
}

fn i18n_num(i18n: &I18n, v: f64) -> String {
    i18n.num((v * 100.0).round() / 100.0)
}

/// SVG path for the last samples in a 300×100 view box.
pub fn graph_path(samples: &[f64]) -> String {
    if samples.len() < 2 {
        return String::new();
    }
    let max = samples.iter().copied().fold(1.0f64, |a, b| a.max(b.abs())) * 1.1;
    let step = 300.0 / (samples.len() - 1) as f64;
    let mut s = String::new();
    for (i, w) in samples.iter().enumerate() {
        let x = i as f64 * step;
        let y = 100.0 - (w.abs() / max * 100.0);
        s.push_str(&format!(
            "{}{x:.1} {y:.1} ",
            if i == 0 { "M " } else { "L " }
        ));
    }
    s
}

fn report_url(snap: &linkspec_core::Snapshot, diag: &linkspec_core::Diagnosis) -> Option<String> {
    // Same fields as `linkspec --report` (allow-list, no personal data).
    use linkspec_core::observation::CableObservation;
    let (obs, pd) = snap
        .ports
        .iter()
        .zip(&diag.ports)
        .find(|(o, _)| matches!(o.cable, CableObservation::Emarker { .. }))?;
    let CableObservation::Emarker { identity, .. } = &obs.cable else {
        return None;
    };
    let c = pd.cable.as_ref()?;
    let enc = |s: &str| {
        s.bytes()
            .map(|b| {
                if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                    char::from(b).to_string()
                } else {
                    format!("%{b:02X}")
                }
            })
            .collect::<String>()
    };
    let vdos = std::iter::once(identity.id_header)
        .chain([identity.cert_stat, identity.product])
        .chain(identity.product_type_vdos.iter().copied())
        .map(|v| format!("0x{v:08x}"))
        .collect::<Vec<_>>()
        .join(" ");
    Some(format!(
        "https://github.com/itsh-neumeier/linkspec/issues/new?template=kabel-melden.yml&labels=kabel&title={}&vid={}&pid={}&fingerprint={}&vdos={}&platform={}",
        enc(&format!(
            "Kabel: VID {} PID {}",
            c.vid.clone().unwrap_or_default(),
            c.pid.clone().unwrap_or_default()
        )),
        enc(&c.vid.clone().unwrap_or_default()),
        enc(&c.pid.clone().unwrap_or_default()),
        linkspec_core::cable_fingerprint(identity),
        enc(&vdos),
        enc(&format!(
            "{} {} ({}), LinkSpec {}",
            diag.host.os,
            diag.host.os_version.clone().unwrap_or_default(),
            diag.host.arch,
            diag.host.app_version
        )),
    ))
}

fn refresh(engine: &mut Engine, send: &impl Fn(Update), last: &mut Option<RunResult>, first: bool) {
    let r = match engine.run() {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("diagnosis failed: {e}");
            return;
        }
    };
    let i18n = engine.i18n();
    let dom = |d: Domain| {
        i18n.t(
            match d {
                Domain::Power => "cli-verdict-power",
                Domain::Data => "cli-verdict-data",
                Domain::Display => "cli-verdict-display",
            },
            &[],
        )
    };
    let mut ports = Vec::new();
    let mut neg = Vec::new();
    let mut displays = Vec::new();
    let mut worst = Status::Ok;
    for (obs, p) in r.snapshot.ports.iter().zip(&r.diagnosis.ports) {
        worst = match (worst, p.status) {
            (Status::Fault, _) | (_, Status::Fault) => Status::Fault,
            (Status::Warn, _) | (_, Status::Warn) => Status::Warn,
            _ => Status::Ok,
        };
        let mut details = Vec::new();
        for v in &p.verdicts {
            details.push(format!("{} · {:?}", v.code, v.limiter).to_lowercase());
            for e in &v.evidence {
                details.push(format!(
                    "  {} = {} ({})",
                    e.field,
                    e.value.as_ref().map(|x| x.to_string()).unwrap_or_default(),
                    e.source.as_str()
                ));
            }
        }
        if let Some(c) = &p.cable {
            if let (Some(v), Some(pid)) = (&c.vid, &c.pid) {
                details.push(format!("VID {v} · PID {pid}"));
            }
            if let Some(Some(x)) = &c.xid {
                details.push(format!("XID {x}"));
            }
        }
        if let Some(pw) = &p.power
            && let Some(pdos) = &pw.pdos
        {
            for (i, d) in pdos.iter().enumerate() {
                details.push(format!(
                    "PDO {} {} {} V {} A {} W{}",
                    i + 1,
                    d.kind,
                    d.voltage_v.map(|x| x.to_string()).unwrap_or_default(),
                    d.current_a.map(|x| x.to_string()).unwrap_or_default(),
                    d.max_w.map(|x| x.to_string()).unwrap_or_default(),
                    if d.selected { " ◀" } else { "" }
                ));
            }
        }
        ports.push(PortData {
            id: p.id.clone(),
            label: p.label.clone(),
            location: p.location.clone().unwrap_or_default(),
            headline: p.headline_text.clone(),
            summary: p.summary.clone(),
            status: match p.status {
                Status::Ok => 0,
                Status::Warn => 1,
                Status::Fault => 2,
            },
            empty: p.headline.as_str() == "empty",
            cable: p
                .cable
                .as_ref()
                .map(|c| c.summary.clone())
                .unwrap_or_default(),
            power: p
                .power
                .as_ref()
                .and_then(|x| x.summary.clone())
                .unwrap_or_default(),
            verdicts: p
                .verdicts
                .iter()
                .filter(|v| v.summary != p.summary)
                .map(|v| VerdictData {
                    domain: dom(v.domain),
                    text: v.summary.clone(),
                    warn: matches!(
                        v.limiter,
                        linkspec_core::Limiter::Cable
                            | linkspec_core::Limiter::Charger
                            | linkspec_core::Limiter::Port
                    ),
                })
                .collect(),
            banners: p
                .faults
                .iter()
                .filter(|f| f.banner)
                .map(|f| (f.kind.as_str().to_string(), f.summary.clone()))
                .collect(),
            devices: p
                .devices
                .iter()
                .map(|d| (d.label(i18n), i32::from(d.depth)))
                .collect(),
            notes: p
                .unavailable
                .iter()
                .map(|u| u.summary.clone())
                .chain(p.notes.iter().map(|n| n.summary.clone()))
                .collect(),
            details,
        });
        for n in negotiation::rows(obs, p, i18n) {
            neg.push(NegData {
                port_label: p.label.clone(),
                domain: dom(n.domain),
                port: n.port.unwrap_or_else(|| "–".into()),
                cable: n.cable.unwrap_or_else(|| "–".into()),
                device: n.device.unwrap_or_else(|| "–".into()),
                negotiated: n.negotiated.unwrap_or_else(|| "–".into()),
                weakest: match n.weakest {
                    Some("port") => 1,
                    Some("cable") => 2,
                    Some("device") => 3,
                    _ => 0,
                },
            });
        }
        for d in &p.displays {
            let mode = |m: &linkspec_core::observation::DisplayMode| {
                format!("{}×{} @ {} Hz", m.width, m.height, i18n.num(m.refresh_hz))
            };
            displays.push(DisplayData {
                port_label: p.label.clone(),
                name: d.name.clone().unwrap_or_default(),
                current: d
                    .current
                    .as_ref()
                    .map(mode)
                    .unwrap_or_else(|| i18n.t("unavailable-field", &[])),
                max: d
                    .max
                    .as_ref()
                    .map(mode)
                    .unwrap_or_else(|| i18n.t("unavailable-field", &[])),
                link: d
                    .link_gbps
                    .map(|g| i18n.t("speed-gbps", &[("gbps", g.into())]))
                    .unwrap_or_else(|| i18n.t("unavailable-field", &[])),
                verdict: p
                    .verdicts
                    .iter()
                    .find(|v| v.domain == Domain::Display)
                    .map(|v| v.summary.clone())
                    .unwrap_or_default(),
            });
        }
    }
    let cables = engine
        .history()
        .and_then(|h| h.list().ok())
        .unwrap_or_default();
    let service = r
        .diagnosis
        .notices
        .iter()
        .find(|n| n.code == "service_not_installed")
        .map(|n| n.summary.clone())
        .unwrap_or_else(|| {
            if r.diagnosis.sources.tier == "service" {
                i18n.t("app-service-running", &[])
            } else {
                String::new()
            }
        });
    if !first {
        for f in &r.new_faults {
            let port = r
                .diagnosis
                .ports
                .iter()
                .find(|p| p.id == f.port_id)
                .map(|p| p.label.clone())
                .unwrap_or_default();
            send(Update::Fault {
                title: format!("LinkSpec · {port}"),
                body: i18n.t(
                    &format!("fault-{}", f.kind.as_str()),
                    &[("count", 1u32.into())],
                ),
            });
        }
    }
    let widget = serde_json::json!({
        "schema": "linkspec.widget.v1",
        "generated_at": r.diagnosis.generated_at,
        "ports": r.diagnosis.ports.iter().map(|p| serde_json::json!({
            "label": p.label,
            "headline": p.headline,
            "headline_text": p.headline_text,
            "summary": p.summary,
            "status": p.status,
        })).collect::<Vec<_>>(),
    });
    let other_usb = r
        .diagnosis
        .other_usb_devices
        .iter()
        .map(|d| (d.label(i18n), i32::from(d.depth)))
        .collect();
    send(Update::Data {
        ports,
        other_usb,
        negotiation: neg,
        displays,
        cables,
        service,
        worst,
        widget,
    });
    *last = Some(r);
}
