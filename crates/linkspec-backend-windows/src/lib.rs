//! LinkSpec Windows backend.
//!
//! Tier 1 (always): user mode without administrator rights – hub IOCTLs,
//! battery IOCTLs, display configuration, `CM_Register_Notification`.
//! Tier 2 (optional): PD and E-Marker data from the LinkSpec service through
//! a named pipe (see `linkspec-service`). Tier 3 (driver) is not built
//! (ADR 0004).

pub mod collect;
pub mod interpret;
pub mod ioctl;
pub mod model;
#[cfg(windows)]
mod win32;

use std::sync::mpsc::Receiver;

use linkspec_core::Snapshot;
use linkspec_platform::{
    CAPTURE_SCHEMA, ChangeEvent, MonitorCapabilities, PortSource, PowerSample, ReadOptions,
    SourceError,
};

pub use interpret::interpret;
pub use model::WinDump;

#[cfg(windows)]
pub use win32::PIPE_NAME;

pub struct WindowsSource {
    active_queries: u64,
    started: std::time::Instant,
}

impl WindowsSource {
    pub fn new() -> Result<Self, SourceError> {
        if !cfg!(windows) {
            return Err(SourceError::Unsupported(linkspec_platform::Unsupported::Os));
        }
        Ok(Self {
            active_queries: 0,
            started: std::time::Instant::now(),
        })
    }

    #[cfg(windows)]
    fn dump(&mut self, opts: ReadOptions) -> WinDump {
        let mut api = win32::Win32Hubs::new();
        let (hubs, active) = collect::collect_hubs(&mut api, opts.usb_probe);
        self.active_queries += active;
        WinDump {
            hubs,
            batteries: win32::batteries(),
            displays: win32::displays(),
            usb4_routers: win32::usb4_routers(),
            service: win32::service_state(),
            host: win32::host(),
        }
    }

    #[cfg(not(windows))]
    fn dump(&mut self, _opts: ReadOptions) -> WinDump {
        WinDump::default()
    }
}

impl PortSource for WindowsSource {
    fn name(&self) -> &'static str {
        "windows"
    }

    fn snapshot(&mut self, opts: ReadOptions) -> Result<Snapshot, SourceError> {
        let mut s = interpret(&self.dump(opts));
        s.usb_probe = opts.usb_probe;
        if !opts.usb_probe {
            s.notices
                .push(linkspec_core::observation::Notice::UsbProbeOff);
        }
        Ok(s)
    }

    fn watch(&mut self) -> Result<Receiver<ChangeEvent>, SourceError> {
        #[cfg(windows)]
        {
            win32::watch()
        }
        #[cfg(not(windows))]
        {
            Err(SourceError::Unsupported(linkspec_platform::Unsupported::Os))
        }
    }

    fn capture(&mut self, opts: ReadOptions) -> Result<serde_json::Value, SourceError> {
        let d = self.dump(opts);
        let mut data = serde_json::to_value(&d).map_err(|e| SourceError::Other(e.to_string()))?;
        linkspec_platform::anonymize::scrub(
            &mut data,
            &linkspec_platform::anonymize::local_secrets(),
        );
        Ok(serde_json::json!({
            "schema": CAPTURE_SCHEMA,
            "backend": "windows",
            "anonymized": true,
            "synthetic": false,
            "host": interpret(&d).host,
            "data": data,
        }))
    }

    fn power_sample(&mut self) -> Result<Option<PowerSample>, SourceError> {
        #[cfg(windows)]
        {
            // Battery charge/discharge rate; per-port power needs the service.
            let b = win32::batteries();
            Ok(b.first().and_then(|b| {
                let w = f64::from(b.rate_mw?) / 1000.0;
                let v = b.voltage_mv.map(|v| f64::from(v) / 1000.0);
                Some(PowerSample {
                    t: self.started.elapsed().as_secs_f64(),
                    watts: Some(w),
                    volts: v,
                    amps: v
                        .filter(|v| *v > 0.0)
                        .map(|v| ((w / v) * 1000.0).round() / 1000.0),
                    port_index: None,
                })
            }))
        }
        #[cfg(not(windows))]
        {
            let _ = self.started;
            Ok(None)
        }
    }

    fn monitor_capabilities(&self) -> MonitorCapabilities {
        MonitorCapabilities {
            watts: true,
            volts: true,
            amps: false,
            sources: vec!["battery_ioctl".into()],
        }
    }

    fn active_query_count(&self) -> u64 {
        self.active_queries
    }
}

/// Replay a capture document into a snapshot.
pub fn snapshot_from_capture(doc: &serde_json::Value) -> Result<Snapshot, String> {
    let data = doc.get("data").ok_or("capture without data")?;
    let d: WinDump = serde_json::from_value(data.clone()).map_err(|e| e.to_string())?;
    Ok(interpret(&d))
}
