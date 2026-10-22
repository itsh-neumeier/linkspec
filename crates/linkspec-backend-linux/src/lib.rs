//! LinkSpec Linux backend.
//!
//! Reads sysfs without root rights and listens to kernel uevents. The
//! collector ([`dump`]) and the interpreter ([`interpret`]) are separate:
//! captures are dumps, and fixtures replay through the same interpreter on
//! every CI runner.

pub mod dump;

pub mod interpret;
#[cfg(target_os = "linux")]
mod watch;

use std::path::PathBuf;
use std::sync::mpsc::Receiver;

use linkspec_core::Snapshot;
use linkspec_platform::anonymize::{local_secrets, scrub};
use linkspec_platform::{
    CAPTURE_SCHEMA, ChangeEvent, MonitorCapabilities, PortSource, PowerSample, ReadOptions,
    SourceError,
};

pub use dump::SysfsDump;

/// The Linux backend. `root` is `/sys` in production and a temporary
/// directory in tests.
pub struct LinuxSource {
    root: PathBuf,
    proc_root: PathBuf,
    started: std::time::Instant,
}

impl LinuxSource {
    pub fn new() -> Self {
        Self::with_roots("/sys".into(), "/proc".into())
    }

    pub fn with_roots(root: PathBuf, proc_root: PathBuf) -> Self {
        Self {
            root,
            proc_root,
            started: std::time::Instant::now(),
        }
    }

    /// Collect the raw dump.
    pub fn dump(&self) -> SysfsDump {
        let mut d = dump::Collector::new(&self.root).collect();
        d.host = dump::host_info(&self.root, &self.proc_root);
        d
    }

    fn check_readable(&self) -> Result<(), SourceError> {
        if self.root.join("class/typec").is_dir() {
            Ok(())
        } else {
            Err(SourceError::NotReadable(
                "/sys/class/typec is missing (kernel without USB Type-C class or UCSI)".into(),
            ))
        }
    }
}

impl Default for LinuxSource {
    fn default() -> Self {
        Self::new()
    }
}

impl PortSource for LinuxSource {
    fn name(&self) -> &'static str {
        "linux"
    }

    fn snapshot(&mut self, opts: ReadOptions) -> Result<Snapshot, SourceError> {
        self.check_readable()?;
        let mut s = interpret::interpret(&self.dump());
        // The Linux backend only reads sysfs (passive); deep probe has no
        // active queries here. The flag is still reported.
        s.usb_probe = opts.usb_probe;
        if !opts.usb_probe {
            s.notices.push(linkspec_core::observation::Notice::UsbProbeOff);
        }
        Ok(s)
    }

    fn watch(&mut self) -> Result<Receiver<ChangeEvent>, SourceError> {
        #[cfg(target_os = "linux")]
        {
            watch::spawn()
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(SourceError::Other("uevent watcher only on Linux".into()))
        }
    }

    fn capture(&mut self, _opts: ReadOptions) -> Result<serde_json::Value, SourceError> {
        let d = self.dump();
        let mut data = serde_json::to_value(&d).map_err(|e| SourceError::Other(e.to_string()))?;
        scrub(&mut data, &local_secrets());
        let host = interpret::interpret(&d).host;
        Ok(serde_json::json!({
            "schema": CAPTURE_SCHEMA,
            "backend": "linux",
            "anonymized": true,
            "synthetic": false,
            "host": host,
            "data": data,
        }))
    }

    fn power_sample(&mut self) -> Result<Option<PowerSample>, SourceError> {
        let mut c = dump::Collector::new(&self.root);
        let d = c.collect_power_only();
        Ok(
            interpret::power_sample(&d).map(|(port, w, v, a)| PowerSample {
                t: self.started.elapsed().as_secs_f64(),
                watts: Some((w * 100.0).round() / 100.0),
                volts: Some((v * 100.0).round() / 100.0),
                amps: Some((a * 1000.0).round() / 1000.0),
                port_index: port,
            }),
        )
    }

    fn monitor_capabilities(&self) -> MonitorCapabilities {
        let d = dump::Collector::new(&self.root).collect_power_only();
        let sources: Vec<String> = d
            .files
            .keys()
            .filter_map(|k| k.strip_prefix("class/power_supply/"))
            .filter_map(|k| k.split('/').next())
            .map(|s| {
                if s.starts_with("ucsi-source-psy") {
                    "ucsi-source-psy".to_string()
                } else {
                    s.to_string()
                }
            })
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        let has = !sources.is_empty();
        MonitorCapabilities {
            watts: has,
            volts: has,
            amps: has,
            sources,
        }
    }
}

/// Replay a capture document (`linkspec --capture` output) into a snapshot.
pub fn snapshot_from_capture(doc: &serde_json::Value) -> Result<Snapshot, String> {
    let data = doc.get("data").ok_or("capture without data")?;
    let d: SysfsDump = serde_json::from_value(data.clone()).map_err(|e| e.to_string())?;
    Ok(interpret::interpret(&d))
}
