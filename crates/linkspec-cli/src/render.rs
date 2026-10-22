//! Short text output: one block per port, symbols ✓ / ! / ✗.

use std::fmt::Write;

use linkspec_core::I18n;
use linkspec_core::diagnosis::{Diagnosis, Domain, Limiter, PortDiagnosis, Status};

/// Colours only on a terminal and without `NO_COLOR`.
#[derive(Debug, Clone, Copy)]
pub struct Style {
    pub color: bool,
}

impl Style {
    fn paint(&self, code: &str, s: &str) -> String {
        if self.color {
            format!("\x1b[{code}m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    }
    fn bold(&self, s: &str) -> String {
        self.paint("1", s)
    }
    fn dim(&self, s: &str) -> String {
        self.paint("2", s)
    }
    fn status(&self, st: Status) -> String {
        let code = match st {
            Status::Ok => "32",
            Status::Warn => "33",
            Status::Fault => "31",
        };
        self.paint(code, st.symbol())
    }
}

fn domain_label(i18n: &I18n, d: Domain) -> String {
    i18n.t(
        match d {
            Domain::Power => "cli-verdict-power",
            Domain::Data => "cli-verdict-data",
            Domain::Display => "cli-verdict-display",
        },
        &[],
    )
}

fn limiter_label(i18n: &I18n, l: Limiter) -> String {
    let k = match l {
        Limiter::Port => "port",
        Limiter::Cable => "cable",
        Limiter::Charger => "charger",
        Limiter::Device => "device",
        Limiter::None => "none",
        Limiter::Unknown => "unknown",
    };
    i18n.t(&format!("cli-limiter-{k}"), &[])
}

pub fn port_block(p: &PortDiagnosis, i18n: &I18n, style: Style, raw: bool) -> String {
    let mut s = String::new();
    let loc = p
        .location
        .as_ref()
        .map(|l| format!(" ({l})"))
        .unwrap_or_default();
    let _ = writeln!(
        s,
        "{} {}{} — {}",
        style.status(p.status),
        style.bold(&p.label),
        loc,
        p.headline_text
    );
    if !p.summary.is_empty() {
        let _ = writeln!(s, "  {}", p.summary);
    }
    for f in &p.faults {
        let _ = writeln!(s, "  {} {}", style.status(Status::Fault), f.summary);
    }
    if let Some(c) = &p.cable {
        let _ = writeln!(s, "  {}: {}", i18n.t("cli-cable", &[]), c.summary);
        if let Some(Some(cert)) = &c.certification {
            let line = if cert == "listed" {
                i18n.t(
                    "cable-certification-listed",
                    &[("name", c.certified_name.clone().unwrap_or_default().into())],
                )
            } else {
                i18n.t("cable-certification-unlisted", &[])
            };
            let _ = writeln!(s, "    {}", style.dim(&line));
        }
        for t in c.trust_flags.iter().flatten() {
            let _ = writeln!(s, "    ! {}", t.summary);
        }
        if let Some(h) = &c.history {
            let _ = writeln!(
                s,
                "    {}: {}{}",
                i18n.t("cli-history", &[]),
                h.name
                    .as_ref()
                    .map(|n| format!("„{n}“, "))
                    .unwrap_or_default(),
                i18n.t(
                    "cli-history-seen",
                    &[
                        ("uses", h.uses.into()),
                        (
                            "since",
                            h.first_seen
                                .get(..10)
                                .unwrap_or(&h.first_seen)
                                .to_string()
                                .into()
                        ),
                        ("faults", h.faults.into()),
                    ]
                )
            );
        }
    }
    if let Some(pw) = &p.power
        && let Some(summary) = &pw.summary
    {
        let _ = writeln!(s, "  {}: {}", i18n.t("cli-power", &[]), summary);
    }
    if !p.devices.is_empty() {
        let _ = writeln!(s, "  {}:", i18n.t("cli-devices", &[]));
        for d in &p.devices {
            let name = match (&d.vendor, &d.product) {
                (Some(v), Some(p)) => format!("{v} {p}"),
                (None, Some(p)) => p.clone(),
                (Some(v), None) => v.clone(),
                (None, None) => "USB".into(),
            };
            let _ = writeln!(
                s,
                "    {}{} {}",
                "  ".repeat(usize::from(d.depth)),
                name,
                style.dim(&format!(
                    "({})",
                    i18n.t("speed-gbps", &[("gbps", d.usb_gbps.into())])
                ))
            );
        }
    }
    // Further verdicts (the summary already shows the most important one).
    for v in &p.verdicts {
        if v.summary == p.summary && !raw {
            continue;
        }
        let _ = writeln!(s, "  {}: {}", domain_label(i18n, v.domain), v.summary);
        if raw {
            let _ = writeln!(
                s,
                "    {} {} · {}",
                style.dim(&i18n.t("cli-limiter", &[])),
                limiter_label(i18n, v.limiter),
                style.dim(&v.code)
            );
            for e in &v.evidence {
                let _ = writeln!(
                    s,
                    "    {} {} = {} ({})",
                    style.dim("·"),
                    e.field,
                    e.value.as_ref().map(|x| x.to_string()).unwrap_or_default(),
                    e.source.as_str()
                );
            }
        }
    }
    for u in &p.unavailable {
        let _ = writeln!(s, "  {}", style.dim(&u.summary));
    }
    for n in &p.notes {
        if n.code == "emarker_is_self_reported" && !raw {
            continue;
        }
        let _ = writeln!(s, "  {}", style.dim(&n.summary));
    }
    if raw && let Some(r) = &p.raw {
        let pretty = serde_json::to_string_pretty(r).unwrap_or_default();
        for line in pretty.lines() {
            let _ = writeln!(s, "    {}", style.dim(line));
        }
    }
    s
}

pub fn text(d: &Diagnosis, i18n: &I18n, style: Style, raw: bool, hide_empty: bool) -> String {
    let mut s = String::new();
    for n in &d.notices {
        let _ = writeln!(s, "{}", style.dim(&n.summary));
    }
    let ports: Vec<_> = d
        .ports
        .iter()
        .filter(|p| !(hide_empty && p.headline.as_str() == "empty"))
        .collect();
    if ports.is_empty() {
        let _ = writeln!(s, "{}", i18n.t("cli-no-ports", &[]));
    }
    for (i, p) in ports.iter().enumerate() {
        if i > 0 {
            s.push('\n');
        }
        s.push_str(&port_block(p, i18n, style, raw));
    }
    s
}
