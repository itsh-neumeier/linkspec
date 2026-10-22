//! Windows dump model (capture format). Cross-platform.

use linkspec_core::edid::EdidSummary;
use linkspec_core::observation::DisplayMode;
use linkspec_core::pd::RawIdentity;
use linkspec_core::types::UsbGen;
use serde::{Deserialize, Serialize};

use crate::ioctl::{ConnInfoEx, ConnInfoV2, ConnectorProps, HubInfoEx, SspInfo};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WinDump {
    pub hubs: Vec<HubDump>,
    #[serde(default)]
    pub batteries: Vec<BatteryDump>,
    #[serde(default)]
    pub displays: Vec<DisplayDump>,
    /// USB4 router device nodes (hypothesis H-W3).
    #[serde(default)]
    pub usb4_routers: Vec<Usb4Router>,
    #[serde(default)]
    pub service: ServiceState,
    #[serde(default)]
    pub host: WinHost,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WinHost {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default)]
    pub arch: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HubDump {
    /// Anonymized key ("hub-1", …); never the symbolic link.
    pub key: String,
    /// Location path of the hub (for example `PCIROOT(0)#PCI(1400)#USBROOT(0)`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    pub info: HubInfoEx,
    pub ports: Vec<HubPortDump>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HubPortDump {
    pub port: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connector: Option<ConnectorProps>,
    /// Key of the companion hub (USB 2 ↔ USB 3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub companion_hub: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conn: Option<ConnInfoEx>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v2: Option<ConnInfoV2>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssp: Option<SspInfo>,
    /// Device description from the device node (passive).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
    /// Product string descriptor (deep probe only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product_string: Option<String>,
    /// Highest class from the BOS descriptor (deep probe only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bos_max: Option<UsbGen>,
    /// Key of the hub attached to this port.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child_hub: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatteryDump {
    /// `BATTERY_STATUS.PowerState` flags: 1 online, 2 discharging,
    /// 4 charging, 8 critical.
    pub power_state: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capacity_mwh: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_mwh: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voltage_mv: Option<u32>,
    /// Signed rate in mW (positive = charging).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_mw: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisplayDump {
    /// `DISPLAYCONFIG_VIDEO_OUTPUT_TECHNOLOGY`.
    pub output_technology: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current: Option<DisplayMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edid: Option<EdidSummary>,
}

impl DisplayDump {
    /// DisplayPort external (10) or DisplayPort over USB tunnel (19).
    /// Values from `DISPLAYCONFIG_VIDEO_OUTPUT_TECHNOLOGY` (wingdi.h).
    pub fn is_usb_c_candidate(&self) -> bool {
        matches!(self.output_technology, 10 | 19)
    }

    pub fn tunneled(&self) -> bool {
        self.output_technology == 19
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usb4Router {
    /// `host` or `device`.
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor_id: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// State of the optional LinkSpec service (tier 2).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ServiceState {
    #[default]
    NotInstalled,
    Error {
        message: String,
    },
    Running(ServiceData),
}

/// Data the service gathered from ETW (see `linkspec-service`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ServiceData {
    pub version: String,
    pub connectors: Vec<ServiceConnector>,
    /// Names of the ETW providers the service subscribed to.
    #[serde(default)]
    pub providers: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ServiceConnector {
    /// 1-based UCSI connector number.
    pub index: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connected: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partner_identity: Option<RawIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cable_identity: Option<RawIdentity>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_pdos: Vec<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sink_pdos: Vec<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rdo: Option<u32>,
    /// `sink` or `source`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub power_role: Option<String>,
}
