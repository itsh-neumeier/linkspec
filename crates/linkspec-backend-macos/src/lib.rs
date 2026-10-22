//! LinkSpec macOS backend (Apple silicon, macOS 14+).
//!
//! The only place in LinkSpec that talks to IOKit. Reads the public IOKit
//! registry and CoreGraphics display modes, without root, private
//! frameworks or entitlements. Intel Macs are not supported: there the port
//! controller data is not available through public IOKit paths.

pub mod registry;

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
mod collect;
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
mod watch;

use std::sync::mpsc::Receiver;

use linkspec_core::Snapshot;
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
use linkspec_platform::anonymize::{local_secrets, scrub};
use linkspec_platform::{
    ChangeEvent, MonitorCapabilities, PortSource, PowerSample, ReadOptions, SourceError,
};

pub use registry::{RegistryDump, interpret};

pub struct MacSource {
    #[allow(dead_code)]
    started: std::time::Instant,
}

impl MacSource {
    /// Create the backend. Fails with `Unsupported(IntelMac)` on Intel.
    pub fn new() -> Result<Self, SourceError> {
        if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
            return Err(SourceError::Unsupported(
                linkspec_platform::Unsupported::IntelMac,
            ));
        }
        if !cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            return Err(SourceError::Unsupported(linkspec_platform::Unsupported::Os));
        }
        Ok(Self {
            started: std::time::Instant::now(),
        })
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn dump(&self) -> RegistryDump {
        collect::collect()
    }

    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    fn dump(&self) -> RegistryDump {
        RegistryDump::default()
    }
}

impl PortSource for MacSource {
    fn name(&self) -> &'static str {
        "macos"
    }

    fn snapshot(&mut self, opts: ReadOptions) -> Result<Snapshot, SourceError> {
        let mut s = interpret(&self.dump());
        // macOS reads only registry properties (passive); there are no
        // active USB queries. The flag is reported as set.
        s.usb_probe = opts.usb_probe;
        if !opts.usb_probe {
            s.notices
                .push(linkspec_core::observation::Notice::UsbProbeOff);
        }
        Ok(s)
    }

    fn watch(&mut self) -> Result<Receiver<ChangeEvent>, SourceError> {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            watch::spawn()
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            Err(SourceError::Unsupported(linkspec_platform::Unsupported::Os))
        }
    }

    fn capture(&mut self, _opts: ReadOptions) -> Result<serde_json::Value, SourceError> {
        let d = self.dump();
        #[allow(unused_mut)]
        let mut data = serde_json::to_value(&d).map_err(|e| SourceError::Other(e.to_string()))?;
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        scrub(&mut data, &local_secrets());
        Ok(serde_json::json!({
            "schema": linkspec_platform::CAPTURE_SCHEMA,
            "backend": "macos",
            "anonymized": true,
            "synthetic": false,
            "host": interpret(&d).host,
            "data": data,
        }))
    }

    fn power_sample(&mut self) -> Result<Option<PowerSample>, SourceError> {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            let entries = collect::collect_registry(&|c: &str| c == "AppleSmartBattery");
            let d = RegistryDump {
                entries,
                ..Default::default()
            };
            Ok(registry::power_sample(&d).map(|(w, v, a)| PowerSample {
                t: self.started.elapsed().as_secs_f64(),
                watts: Some((w * 100.0).round() / 100.0),
                volts: v,
                amps: a,
                port_index: None,
            }))
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            Ok(None)
        }
    }

    fn monitor_capabilities(&self) -> MonitorCapabilities {
        MonitorCapabilities {
            watts: true,
            volts: true,
            amps: true,
            sources: vec!["AppleSmartBattery.PowerTelemetryData".into()],
        }
    }
}

/// Replay a capture document into a snapshot.
pub fn snapshot_from_capture(doc: &serde_json::Value) -> Result<Snapshot, String> {
    let data = doc.get("data").ok_or("capture without data")?;
    let d: RegistryDump = serde_json::from_value(data.clone()).map_err(|e| e.to_string())?;
    Ok(interpret(&d))
}
