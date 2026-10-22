//! Platform abstraction shared by all backends.
//!
//! A backend implements [`PortSource`]. It reads passively by default; active
//! USB queries (deep probe) only run when [`ReadOptions::usb_probe`] is set.
//! Change notifications are event driven (IOKit notifications, udev netlink,
//! `CM_Register_Notification`); polling is reserved for the power monitor.

#![forbid(unsafe_code)]

use std::sync::mpsc::Receiver;

use linkspec_core::Snapshot;
use serde::{Deserialize, Serialize};

pub mod anonymize;

/// Options for one reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadOptions {
    /// Allow active USB queries (descriptor/BOS requests).
    pub usb_probe: bool,
}

impl Default for ReadOptions {
    fn default() -> Self {
        Self { usb_probe: true }
    }
}

/// Errors of a backend.
#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    /// The platform is not supported at all (exit code 3).
    #[error("unsupported platform: {0}")]
    Unsupported(Unsupported),
    /// The data source cannot be read in principle (exit code 3).
    #[error("data source not readable: {0}")]
    NotReadable(String),
    /// Any other error while reading.
    #[error("{0}")]
    Other(String),
}

/// Why the platform is unsupported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unsupported {
    /// Intel Mac: no port controller data through public IOKit paths.
    IntelMac,
    /// Operating system without a backend.
    Os,
}

impl std::fmt::Display for Unsupported {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IntelMac => f.write_str("intel_mac"),
            Self::Os => f.write_str("os"),
        }
    }
}

/// A change notification. Consumers read a new snapshot after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeEvent {
    /// Something in the USB/Type-C/Thunderbolt tree changed.
    Changed,
    /// The watcher stopped (for example, the netlink socket closed).
    Stopped,
}

/// One power monitor sample.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PowerSample {
    /// Seconds since the monitor started.
    pub t: f64,
    pub watts: Option<f64>,
    pub volts: Option<f64>,
    pub amps: Option<f64>,
    /// Port id the sample belongs to, if known (otherwise the system input).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_index: Option<u32>,
}

/// What the power monitor can read on this platform.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorCapabilities {
    pub watts: bool,
    pub volts: bool,
    pub amps: bool,
    /// Human-readable names of the source (for example `ucsi-source-psy`).
    pub sources: Vec<String>,
}

/// A platform backend.
pub trait PortSource: Send {
    /// Short backend name (`macos`, `linux`, `windows`, `fake`).
    fn name(&self) -> &'static str;

    /// Read a complete snapshot.
    fn snapshot(&mut self, opts: ReadOptions) -> Result<Snapshot, SourceError>;

    /// Start the change watcher. The receiver gets [`ChangeEvent::Changed`]
    /// for every relevant change. Backends coalesce bursts.
    fn watch(&mut self) -> Result<Receiver<ChangeEvent>, SourceError>;

    /// Anonymized raw platform dump for `--capture` and fixtures.
    fn capture(&mut self, opts: ReadOptions) -> Result<serde_json::Value, SourceError>;

    /// One power sample (power monitor only; may poll).
    fn power_sample(&mut self) -> Result<Option<PowerSample>, SourceError> {
        Ok(None)
    }

    /// What the power monitor can read.
    fn monitor_capabilities(&self) -> MonitorCapabilities {
        MonitorCapabilities::default()
    }

    /// Number of active USB queries done so far (deep probe). Used by tests
    /// to prove that `usb_probe = false` sends no active requests.
    fn active_query_count(&self) -> u64 {
        0
    }
}

/// Capture document wrapper. Every capture carries the schema id and a flag
/// whether it was anonymized.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureDoc {
    pub schema: String,
    pub backend: String,
    pub anonymized: bool,
    pub synthetic: bool,
    pub host: linkspec_core::observation::HostInfo,
    pub data: serde_json::Value,
}

pub const CAPTURE_SCHEMA: &str = "linkspec.capture.v1";
