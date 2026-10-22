//! Output model: the `linkspec.diagnose.v1` document.
//!
//! Keys are binding (see `schemas/linkspec.diagnose.v1.json`). Changes are
//! additive only. Unknown values are left out; `null` appears only for
//! columns that exist but have no value.

use serde::{Deserialize, Serialize};

use crate::observation::{DisplayMode, FaultKind, Reason};
use crate::types::{Source, Transport};

pub const SCHEMA_ID: &str = "linkspec.diagnose.v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diagnosis {
    pub schema: String,
    pub generated_at: String,
    pub host: HostOut,
    pub sources: SourcesOut,
    pub ports: Vec<PortDiagnosis>,
    /// Platform-wide notices, localized (additive in v1).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<NoticeOut>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostOut {
    pub os: String,
    pub os_version: Option<String>,
    pub model: Option<String>,
    pub arch: String,
    pub app_version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourcesOut {
    pub tier: String,
    pub usb_probe: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoticeOut {
    pub code: String,
    pub summary: String,
}

/// Headline enum, section 5.1. Never free text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Headline {
    ThunderboltUsb4,
    UsbDevice,
    Display,
    ChargingOnly,
    SlowOrChargeOnly,
    Empty,
}

impl Headline {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ThunderboltUsb4 => "thunderbolt_usb4",
            Self::UsbDevice => "usb_device",
            Self::Display => "display",
            Self::ChargingOnly => "charging_only",
            Self::SlowOrChargeOnly => "slow_or_charge_only",
            Self::Empty => "empty",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PortDiagnosis {
    pub id: String,
    pub label: String,
    /// Localized physical location, if known (additive).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    pub headline: Headline,
    pub headline_text: String,
    pub summary: String,
    /// Overall state for the short view: `ok`, `warn` or `fault`.
    pub status: Status,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cable: Option<CableOut>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub power: Option<PowerOut>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<LinkOut>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub devices: Vec<DeviceOut>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub displays: Vec<DisplayOut>,
    pub verdicts: Vec<Verdict>,
    pub faults: Vec<FaultOut>,
    pub unavailable: Vec<UnavailableOut>,
    /// Card notes (for example "deep probe off"), localized.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<NoteOut>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health: Option<HealthOut>,
    /// Decoded raw values; only with `--raw`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Ok,
    Warn,
    Fault,
}

impl Status {
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Ok => "✓",
            Self::Warn => "!",
            Self::Fault => "✗",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CableOut {
    pub has_emarker: bool,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed_gbps: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_a: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub power_w_max: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<String>,
    /// Present (maybe `null`) whenever an E-Marker was read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor_name: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xid: Option<Option<String>>,
    /// `listed` / `unlisted`; `null` when the cable reports no XID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certification: Option<Option<String>>,
    /// Name from the bundled certification list (only when `listed`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certified_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trust_flags: Option<Vec<TrustFlag>>,
    /// Localized one-line description of the cable.
    pub summary: String,
    /// Known cable from the community fingerprint list (additive).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub known_as: Option<String>,
    /// History summary for this cable (additive).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history: Option<CableHistoryOut>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CableHistoryOut {
    pub fingerprint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub first_seen: String,
    pub uses: u32,
    pub faults: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrustFlag {
    pub code: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PowerOut {
    pub available: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pdos: Option<Vec<PdoOut>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_w: Option<Option<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_v: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_a: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measured_w: Option<f64>,
    /// Only the Type-C current level is visible (no PD details).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub basic_only: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typec_current: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PdoOut {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voltage_v: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_voltage_v: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_a: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_w: Option<f64>,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LinkOut {
    pub transports: Vec<Transport>,
    pub usb_gbps: Option<f64>,
    pub thunderbolt_gen: Option<String>,
    pub lanes: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fabric_gbps: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fabric_measured: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub displayport_lanes: Option<u8>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub topology: Vec<FabricHopOut>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FabricHopOut {
    pub depth: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_gbps: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceOut {
    pub vendor: Option<String>,
    pub product: Option<String>,
    /// `direct` or `hub` (behind a hub/dock).
    pub via: String,
    pub usb_gbps: f64,
    /// Nesting depth below the port (0 = direct).
    #[serde(default)]
    pub depth: u8,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_hub: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisplayOut {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current: Option<DisplayMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<DisplayMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dsc: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_gbps: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_gbps: Option<f64>,
}

/// Verdict domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Domain {
    Power,
    Data,
    Display,
}

/// The single limiting link of a chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Limiter {
    Port,
    Cable,
    Charger,
    Device,
    None,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Verdict {
    pub domain: Domain,
    pub limiter: Limiter,
    pub code: String,
    pub summary: String,
    pub evidence: Vec<Evidence>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    pub field: String,
    pub source: Source,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FaultOut {
    pub kind: FaultKind,
    pub count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_at: Option<String>,
    /// Banner shown on the card until dismissed or the cable is pulled.
    pub banner: bool,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnavailableOut {
    pub field: String,
    pub reason: Reason,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoteOut {
    pub code: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthOut {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overcurrent_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reconnect_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orientation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub liquid_detected: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cable_resistance_mohm: Option<f64>,
}
