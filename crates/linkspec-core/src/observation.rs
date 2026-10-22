//! Observations: what a platform backend actually read.
//!
//! Backends fill these structures. They never decide verdicts. Every
//! section carries its [`Source`]. Missing data stays `None` or is listed in
//! `unavailable` with a reason; backends never estimate values.
//!
//! The JSON form of [`Snapshot`] is the fixture format in `fixtures/core/`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::pd::{Pdo, RawIdentity};
use crate::types::{FabricGen, Source, Transport, UsbGen};

/// Operating system family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Os {
    Macos,
    Linux,
    Windows,
}

impl Os {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Macos => "macos",
            Self::Linux => "linux",
            Self::Windows => "windows",
        }
    }
}

/// Host description. Contains no host name, user name or serial number.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostInfo {
    pub os: Os,
    #[serde(default)]
    pub os_version: Option<String>,
    /// Hardware model identifier (for example `Mac15,6`); never a serial.
    #[serde(default)]
    pub model: Option<String>,
    pub arch: String,
}

/// Data access tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// User mode without elevated rights.
    #[default]
    User,
    /// Windows: data from the optional LinkSpec service (ETW).
    Service,
    /// Windows: data from a companion driver (not built, see ADR 0004).
    Driver,
}

/// One complete reading of the machine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub host: HostInfo,
    #[serde(default)]
    pub tier: Tier,
    /// Whether active USB queries (deep probe) were allowed.
    #[serde(default = "default_true")]
    pub usb_probe: bool,
    /// Host battery, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub battery: Option<BatteryObservation>,
    pub ports: Vec<PortObservation>,
    /// Platform-wide notes (for example: Windows service not installed).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<Notice>,
}

fn default_true() -> bool {
    true
}

/// Platform-wide notice codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Notice {
    /// Windows: the optional service is not installed or not running.
    ServiceNotInstalled,
    /// Deep USB probe is disabled.
    UsbProbeOff,
}

/// Reason codes for missing data. Rendered by `unavailable-<code>` strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    /// The platform API does not expose this value.
    PlatformNotExposed,
    /// The value needs administrator rights (Windows tier 2).
    NeedsService,
    /// The kernel or firmware does not report it on this machine.
    FirmwareNotReported,
    /// The port sits behind an internal hub; cable and power data are not
    /// visible.
    BehindInternalHub,
    /// Deep USB probe is disabled.
    UsbProbeOff,
    /// Access was denied (file permissions).
    PermissionDenied,
}

impl Reason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PlatformNotExposed => "platform_not_exposed",
            Self::NeedsService => "needs_service",
            Self::FirmwareNotReported => "firmware_not_reported",
            Self::BehindInternalHub => "behind_internal_hub",
            Self::UsbProbeOff => "usb_probe_off",
            Self::PermissionDenied => "permission_denied",
        }
    }
}

/// A field the platform did not deliver, with the reason.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Unavailable {
    /// Schema path of the field, for example `cable` or `power.pdos`.
    pub field: String,
    pub reason: Reason,
}

/// Physical location of a port (from ACPI `_PLD` or platform data).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortLocation {
    /// `left`, `right`, `front`, `back`, `top`, `bottom`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub panel: Option<String>,
    /// `left`, `center`, `right`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horizontal: Option<String>,
    /// `upper`, `center`, `lower`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertical: Option<String>,
}

/// What kind of physical connector the port is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PortKind {
    /// USB Type-C without known Thunderbolt or USB4 support.
    #[default]
    UsbC,
    /// USB Type-C with USB4 support.
    Usb4,
    /// USB Type-C with Thunderbolt support.
    Thunderbolt,
}

/// One physical USB-C/Thunderbolt port.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PortObservation {
    /// Stable identifier (derived from location path or connector index).
    pub id: String,
    /// 1-based physical index used for the label and the order.
    pub index: u32,
    #[serde(default)]
    pub kind: PortKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<PortLocation>,
    /// `Some(false)` = nothing plugged in. `None` = the platform cannot tell.
    pub connected: Option<bool>,
    /// Port sits behind an internal hub (for example front USB-C of a desktop).
    #[serde(default)]
    pub behind_internal_hub: bool,
    #[serde(default)]
    pub capabilities: PortCapabilities,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partner: Option<PartnerObservation>,
    #[serde(default)]
    pub cable: CableObservation,
    #[serde(default)]
    pub power: PowerObservation,
    #[serde(default)]
    pub link: LinkObservation,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub displays: Vec<DisplayObservation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub faults: Vec<FaultObservation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health: Option<HealthObservation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unavailable: Vec<Unavailable>,
    /// Platform raw values for `--raw` (already anonymized).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub raw: BTreeMap<String, serde_json::Value>,
}

impl PortObservation {
    /// An empty port with no data at all.
    pub fn new(id: impl Into<String>, index: u32) -> Self {
        Self {
            id: id.into(),
            index,
            kind: PortKind::UsbC,
            location: None,
            connected: None,
            behind_internal_hub: false,
            capabilities: PortCapabilities::default(),
            partner: None,
            cable: CableObservation::default(),
            power: PowerObservation::default(),
            link: LinkObservation::default(),
            displays: Vec::new(),
            faults: Vec::new(),
            health: None,
            unavailable: Vec::new(),
            raw: BTreeMap::new(),
        }
    }

    pub fn mark_unavailable(&mut self, field: &str, reason: Reason) {
        if !self.unavailable.iter().any(|u| u.field == field) {
            self.unavailable.push(Unavailable {
                field: field.to_string(),
                reason,
            });
        }
    }
}

/// What the host port itself can do.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PortCapabilities {
    /// Highest USB data class of the port (root hub capability).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usb: Option<UsbGen>,
    /// `usb` is only a proven minimum (the port may support more). The
    /// verdict engine then never blames the port.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub usb_is_lower_bound: bool,
    /// Thunderbolt/USB4 generation the port supports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fabric: Option<FabricGen>,
    /// DisplayPort Alt Mode supported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub displayport: Option<bool>,
    /// Highest power the host accepts on this port (sink), in W.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sink_max_w: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
}

/// The port partner (the device or charger at the other end).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PartnerObservation {
    /// Raw Discover Identity on SOP.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<RawIdentity>,
    /// Partner speaks USB PD.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_pd: Option<bool>,
    /// Partner is an accessory (audio, debug).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accessory: Option<String>,
    /// Vendor/product name, if the platform names the partner.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub source: Source,
}

/// Cable state. `Unknown` means the platform has no cable data at all.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CableObservation {
    /// No information (nothing connected, or not read).
    #[default]
    Unknown,
    /// The platform cannot deliver cable data.
    Unavailable { reason: Reason },
    /// The platform reads cable identities, and this cable has no E-Marker.
    NoEmarker { source: Source },
    /// E-Marker identity read (SOP', optionally SOP'').
    Emarker {
        identity: RawIdentity,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        far_end: Option<RawIdentity>,
        source: Source,
    },
}

/// Power role of the host on this port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PowerRole {
    /// The host takes power from the partner (charger connected).
    Sink,
    /// The host supplies power to the partner.
    Source,
}

/// Power state. `Unknown` means not read; `Unavailable` means the platform
/// cannot read it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum PowerObservation {
    #[default]
    Unknown,
    Unavailable { reason: Reason },
    Available(Box<PowerData>),
}

/// USB Type-C current level advertised without PD (Rp value).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TypecCurrent {
    Default,
    #[serde(rename = "1.5a")]
    A1_5,
    #[serde(rename = "3a")]
    A3,
}

impl TypecCurrent {
    /// Power at 5 V. "Default" is USB 2.0/3.x default current: 0.5–0.9 A;
    /// we use 0.9 A × 5 V as the upper bound.
    pub fn watts(self) -> f64 {
        match self {
            Self::Default => 4.5,
            Self::A1_5 => 7.5,
            Self::A3 => 15.0,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PowerData {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<PowerRole>,
    /// Source capabilities of the charger (when host is sink) or of this
    /// port (when host is source).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_pdos: Vec<Pdo>,
    /// Sink capabilities of the host (when sink) or of the partner.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sink_pdos: Vec<Pdo>,
    /// Negotiated PD contract.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract: Option<Contract>,
    /// Type-C current level when no PD contract is visible.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typec_current: Option<TypecCurrent>,
    /// Measured power flowing through the port, in W.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measured_w: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measured_v: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measured_a: Option<f64>,
    pub source: Source,
}

impl PowerData {
    /// Highest power offered by the source capabilities.
    pub fn source_max_w(&self) -> Option<f64> {
        self.source_pdos
            .iter()
            .filter_map(Pdo::max_w)
            .fold(None, |acc, w| Some(acc.map_or(w, |a: f64| a.max(w))))
    }

    /// Highest power requested by the sink capabilities.
    pub fn sink_max_w(&self) -> Option<f64> {
        self.sink_pdos
            .iter()
            .filter_map(Pdo::max_w)
            .fold(None, |acc, w| Some(acc.map_or(w, |a: f64| a.max(w))))
    }
}

/// The active power contract.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Contract {
    /// 1-based index into `source_pdos`, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pdo_index: Option<u8>,
    pub voltage_v: f64,
    pub current_a: f64,
    pub power_w: f64,
}

/// Host battery.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatteryObservation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percent: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub charging: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_power: Option<bool>,
    /// Power into the battery (positive = charging), in W.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub power_w: Option<f64>,
    pub source: Source,
}

/// Link and data path state.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LinkObservation {
    /// Transports with platform evidence.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transports: Vec<Transport>,
    /// USB devices on this port (tree; hubs carry children).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub usb_devices: Vec<UsbDeviceObservation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fabric: Option<FabricLinkObservation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub displayport: Option<DpLinkObservation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
}

/// A USB device seen on a port.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsbDeviceObservation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor_id: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product_id: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,
    /// Negotiated link rate in Mbit/s.
    pub speed_mbps: f64,
    /// `bcdUSB` from the device descriptor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bcd_usb: Option<u16>,
    /// Highest class the device supports (from BOS or UFP VDO), if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_capable: Option<UsbGen>,
    #[serde(default)]
    pub is_hub: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<UsbDeviceObservation>,
    pub source: Source,
}

impl UsbDeviceObservation {
    pub fn link_gen(&self) -> UsbGen {
        UsbGen::from_link_mbps(self.speed_mbps)
    }

    /// Best estimate of the device's own maximum, from evidence only.
    /// `bcdUSB` 1.x/2.00 proves a USB 2.0-only device; 2.01+ or 3.x does not
    /// prove SuperSpeed support.
    pub fn proven_max(&self) -> Option<UsbGen> {
        if let Some(m) = self.max_capable {
            return Some(m);
        }
        match self.bcd_usb {
            Some(b) if b <= 0x0200 => Some(UsbGen::Usb2),
            _ => None,
        }
    }
}

/// Thunderbolt/USB4 link reading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FabricLinkObservation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation: Option<FabricGen>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lanes: Option<u8>,
    /// Total receive rate in Gbit/s (all lanes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rx_gbps: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tx_gbps: Option<f64>,
    /// True only when the link rate comes from the host controller's own
    /// link training state (not from a static capability).
    #[serde(default)]
    pub measured_by_controller: bool,
    /// Downstream routers, in hop order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub switches: Vec<FabricSwitch>,
    pub source: Source,
}

impl FabricLinkObservation {
    pub fn link_gbps(&self) -> Option<f64> {
        match (self.rx_gbps, self.tx_gbps) {
            (Some(r), Some(t)) => Some(r.min(t)),
            (Some(r), None) => Some(r),
            (None, Some(t)) => Some(t),
            _ => None,
        }
    }
}

/// A router in the Thunderbolt/USB4 fabric.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FabricSwitch {
    /// Hop count from the host router (1 = directly attached).
    pub depth: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation: Option<FabricGen>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_gbps: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lanes: Option<u8>,
}

/// DisplayPort link (Alt Mode or tunnelled).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DpLinkObservation {
    /// Number of DisplayPort lanes in use (1, 2 or 4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lanes: Option<u8>,
    /// Per-lane rate in Gbit/s (1.62, 2.7, 5.4, 8.1, 10, 13.5, 20).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane_gbps: Option<f64>,
    /// Pin assignment (C, D, E) when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin_assignment: Option<String>,
    #[serde(default)]
    pub tunneled: bool,
    pub source: Source,
}

/// A display mode.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DisplayMode {
    pub width: u32,
    pub height: u32,
    pub refresh_hz: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bits_per_color: Option<u8>,
}

/// A monitor on this port.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisplayObservation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current: Option<DisplayMode>,
    /// Highest mode the monitor offers (EDID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<DisplayMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dsc: Option<bool>,
    /// Adapter in the chain (for example `hdmi`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter: Option<String>,
    pub source: Source,
}

/// Fault kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FaultKind {
    Overcurrent,
    DropReconnect,
}

impl FaultKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Overcurrent => "overcurrent",
            Self::DropReconnect => "drop_reconnect",
        }
    }
}

/// A fault seen in this session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FaultObservation {
    pub kind: FaultKind,
    pub count: u32,
    /// RFC 3339 time of the last occurrence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_at: Option<String>,
}

/// Port health counters and pin data.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HealthObservation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overcurrent_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reconnect_count: Option<u32>,
    /// `normal` or `reverse`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orientation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub liquid_detected: Option<bool>,
    /// Voltage at the source side, for cable resistance estimation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_side_v: Option<f64>,
    /// Voltage at the sink side under the same current.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sink_side_v: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_a: Option<f64>,
}

impl HealthObservation {
    /// Cable resistance in milliohm, only when both voltages and the current
    /// were measured at the same time and the current is large enough.
    pub fn cable_resistance_mohm(&self) -> Option<f64> {
        let (vs, vk, i) = (self.source_side_v?, self.sink_side_v?, self.current_a?);
        if i < 0.5 || vs < vk {
            return None;
        }
        Some(((vs - vk) / i * 1000.0).round())
    }
}
