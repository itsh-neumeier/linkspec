//! `--report`: prefilled GitHub issue for an E-Marker cable. Nothing is
//! sent: the URL opens in the browser and the user submits it (or not).
//! The issue form is `.github/ISSUE_TEMPLATE/kabel-melden.yml`; GitHub
//! fills form fields from query parameters named like the field ids.

use linkspec_core::catalog::cable_fingerprint;
use linkspec_core::diagnosis::{Diagnosis, PortDiagnosis};
use linkspec_core::observation::{CableObservation, Snapshot};

pub const REPO_URL: &str = "https://github.com/itsh-neumeier/linkspec";

/// RFC 3986 percent-encoding for query values.
pub fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Fields of the report for the first port with an E-Marker cable.
pub fn fields(snap: &Snapshot, diag: &Diagnosis) -> Option<Vec<(&'static str, String)>> {
    let (obs, pd): (_, &PortDiagnosis) = snap
        .ports
        .iter()
        .zip(&diag.ports)
        .find(|(o, _)| matches!(o.cable, CableObservation::Emarker { .. }))?;
    let CableObservation::Emarker { identity, .. } = &obs.cable else {
        return None;
    };
    let c = pd.cable.as_ref()?;
    let vdos = std::iter::once(identity.id_header)
        .chain([identity.cert_stat, identity.product])
        .chain(identity.product_type_vdos.iter().copied())
        .map(|v| format!("0x{v:08x}"))
        .collect::<Vec<_>>()
        .join(" ");
    let link = pd
        .link
        .as_ref()
        .map(|l| {
            let t: Vec<_> = l.transports.iter().map(|t| t.as_str()).collect();
            format!(
                "{} {}",
                t.join("+"),
                l.fabric_gbps
                    .or(l.usb_gbps)
                    .map(|g| format!("{g} Gbit/s"))
                    .unwrap_or_default()
            )
        })
        .unwrap_or_default();
    Some(vec![
        ("vid", c.vid.clone().unwrap_or_default()),
        ("pid", c.pid.clone().unwrap_or_default()),
        ("cable_type", c.kind.clone().unwrap_or_default()),
        (
            "speed",
            c.speed_gbps
                .map(|s| format!("{s} Gbit/s"))
                .unwrap_or_default(),
        ),
        (
            "power",
            match (c.current_a, c.power_w_max) {
                (Some(a), Some(w)) => format!("{a} A / {w} W"),
                _ => String::new(),
            },
        ),
        ("xid", c.xid.clone().flatten().unwrap_or_default()),
        ("fingerprint", cable_fingerprint(identity)),
        ("vdos", vdos),
        (
            "platform",
            format!(
                "{} {} ({}), LinkSpec {}",
                diag.host.os,
                diag.host.os_version.clone().unwrap_or_default(),
                diag.host.arch,
                diag.host.app_version
            ),
        ),
        ("observed", link.trim().to_string()),
    ])
}

pub fn url(fields: &[(&'static str, String)]) -> String {
    let title = fields
        .iter()
        .find(|(k, _)| *k == "vid")
        .zip(fields.iter().find(|(k, _)| *k == "pid"))
        .map(|((_, v), (_, p))| format!("Kabel: VID {v} PID {p}"))
        .unwrap_or_else(|| "Kabel melden".into());
    let mut u = format!(
        "{REPO_URL}/issues/new?template=kabel-melden.yml&labels=kabel&title={}",
        encode(&title)
    );
    for (k, v) in fields {
        if !v.is_empty() {
            u.push('&');
            u.push_str(k);
            u.push('=');
            u.push_str(&encode(v));
        }
    }
    u
}

/// Open a URL in the default browser. Returns false if that failed.
pub fn open_browser(url: &str) -> bool {
    let mut cmd = if cfg!(target_os = "macos") {
        let mut c = std::process::Command::new("open");
        c.arg(url);
        c
    } else if cfg!(windows) {
        // rundll32 passes the URL unchanged ("&" is safe here).
        let mut c = std::process::Command::new("rundll32");
        c.args(["url.dll,FileProtocolHandler", url]);
        c
    } else {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(url);
        c
    };
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_query() {
        assert_eq!(encode("a b&c/ä"), "a%20b%26c%2F%C3%A4");
    }
}
