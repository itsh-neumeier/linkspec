//! Byte-level parsers for the USB hub IOCTL output buffers.
//!
//! Layouts follow `usbioctl.h` / `usbspec.h` from the Windows SDK, as
//! documented on Microsoft Learn:
//! * `USB_NODE_CONNECTION_INFORMATION_EX` (packed, 1-byte alignment):
//!   <https://learn.microsoft.com/windows/win32/api/usbioctl/ns-usbioctl-usb_node_connection_information_ex>
//! * `USB_NODE_CONNECTION_INFORMATION_EX_V2`:
//!   <https://learn.microsoft.com/windows/win32/api/usbioctl/ns-usbioctl-usb_node_connection_information_ex_v2>
//! * `USB_PORT_CONNECTOR_PROPERTIES`:
//!   <https://learn.microsoft.com/windows/win32/api/usbioctl/ns-usbioctl-usb_port_connector_properties>
//! * `USB_NODE_CONNECTION_SUPERSPEEDPLUS_INFORMATION`:
//!   <https://learn.microsoft.com/windows/win32/api/usbioctl/ns-usbioctl-usb_node_connection_superspeedplus_information>
//! * `USB_HUB_INFORMATION_EX`:
//!   <https://learn.microsoft.com/windows/win32/api/usbioctl/ns-usbioctl-usb_hub_information_ex>
//!
//! Parsing on byte buffers keeps the logic testable on every platform.

use serde::{Deserialize, Serialize};

fn u16le(b: &[u8], off: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*b.get(off)?, *b.get(off + 1)?]))
}

fn u32le(b: &[u8], off: usize) -> Option<u32> {
    Some(u32::from_le_bytes([
        *b.get(off)?,
        *b.get(off + 1)?,
        *b.get(off + 2)?,
        *b.get(off + 3)?,
    ]))
}

/// `USB_HUB_TYPE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HubType {
    Root,
    Usb20,
    Usb30,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HubInfoEx {
    pub hub_type: HubType,
    pub highest_port: u16,
}

pub fn parse_hub_info_ex(b: &[u8]) -> Option<HubInfoEx> {
    let t = u32le(b, 0)?;
    Some(HubInfoEx {
        hub_type: match t {
            1 => HubType::Root,
            2 => HubType::Usb20,
            3 => HubType::Usb30,
            _ => HubType::Unknown,
        },
        highest_port: u16le(b, 4)?,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorProps {
    pub user_connectable: bool,
    pub debug_capable: bool,
    pub multiple_companions: bool,
    pub type_c: bool,
    pub companion_index: u16,
    pub companion_port: u16,
    /// Symbolic link of the companion hub (only used during collection,
    /// replaced by a hub key before it is stored).
    #[serde(skip)]
    pub companion_hub_link: Option<String>,
}

pub fn parse_connector_props(b: &[u8]) -> Option<ConnectorProps> {
    let props = u32le(b, 8)?;
    let actual = u32le(b, 4)? as usize;
    let name = if actual > 16 && b.len() >= actual {
        let words: Vec<u16> = b[16..actual]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .take_while(|w| *w != 0)
            .collect();
        let s = String::from_utf16_lossy(&words);
        (!s.is_empty()).then_some(s)
    } else {
        None
    };
    Some(ConnectorProps {
        user_connectable: props & 0b0001 != 0,
        debug_capable: props & 0b0010 != 0,
        multiple_companions: props & 0b0100 != 0,
        type_c: props & 0b1000 != 0,
        companion_index: u16le(b, 12)?,
        companion_port: u16le(b, 14)?,
        companion_hub_link: name,
    })
}

/// `USB_CONNECTION_STATUS`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnStatus {
    NoDevice,
    Connected,
    FailedEnumeration,
    GeneralFailure,
    Overcurrent,
    NotEnoughPower,
    NotEnoughBandwidth,
    HubNestedTooDeeply,
    InLegacyHub,
    Enumerating,
    Reset,
    Unknown,
}

impl ConnStatus {
    fn from(v: u32) -> Self {
        match v {
            0 => Self::NoDevice,
            1 => Self::Connected,
            2 => Self::FailedEnumeration,
            3 => Self::GeneralFailure,
            4 => Self::Overcurrent,
            5 => Self::NotEnoughPower,
            6 => Self::NotEnoughBandwidth,
            7 => Self::HubNestedTooDeeply,
            8 => Self::InLegacyHub,
            9 => Self::Enumerating,
            10 => Self::Reset,
            _ => Self::Unknown,
        }
    }

    pub fn device_present(self) -> bool {
        !matches!(self, Self::NoDevice | Self::Unknown)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnInfoEx {
    pub status: ConnStatus,
    /// `USB_DEVICE_SPEED`: 0 low, 1 full, 2 high, 3 super.
    pub speed: u8,
    pub is_hub: bool,
    pub bcd_usb: u16,
    pub device_class: u8,
    pub vendor_id: u16,
    pub product_id: u16,
    pub i_product: u8,
}

/// Size of the fixed part of `USB_NODE_CONNECTION_INFORMATION_EX`.
pub const CONN_INFO_EX_SIZE: usize = 35;

pub fn parse_conn_info_ex(b: &[u8]) -> Option<ConnInfoEx> {
    if b.len() < CONN_INFO_EX_SIZE {
        return None;
    }
    // Device descriptor at offset 4 (18 bytes).
    let d = &b[4..22];
    Some(ConnInfoEx {
        bcd_usb: u16le(d, 2)?,
        device_class: d[4],
        vendor_id: u16le(d, 8)?,
        product_id: u16le(d, 10)?,
        i_product: d[15],
        speed: b[23],
        is_hub: b[24] != 0,
        status: ConnStatus::from(u32le(b, 31)?),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnInfoV2 {
    pub usb110: bool,
    pub usb200: bool,
    pub usb300: bool,
    pub operating_at_ss: bool,
    pub ss_capable: bool,
    pub operating_at_ssp: bool,
    pub ssp_capable: bool,
}

pub const CONN_INFO_V2_SIZE: usize = 16;

pub fn parse_conn_info_v2(b: &[u8]) -> Option<ConnInfoV2> {
    let protocols = u32le(b, 8)?;
    let flags = u32le(b, 12)?;
    Some(ConnInfoV2 {
        usb110: protocols & 0b001 != 0,
        usb200: protocols & 0b010 != 0,
        usb300: protocols & 0b100 != 0,
        operating_at_ss: flags & 0b0001 != 0,
        ss_capable: flags & 0b0010 != 0,
        operating_at_ssp: flags & 0b0100 != 0,
        ssp_capable: flags & 0b1000 != 0,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SspInfo {
    /// Total receive rate in Gbit/s (lane rate × lanes).
    pub rx_gbps: f64,
    pub rx_lanes: u32,
    pub tx_gbps: f64,
    pub tx_lanes: u32,
}

pub const SSP_INFO_SIZE: usize = 24;

/// `USB_DEVICE_SUPERSPEEDPLUS_SPEED`: lane speed = mantissa × 1000^exponent
/// bit/s (exponent 0 = b/s, 1 = kb/s, 2 = Mb/s, 3 = Gb/s).
fn ssp_speed_gbps(v: u32) -> f64 {
    let exponent = (v >> 4) & 0b11;
    let mantissa = f64::from(v >> 16);
    let bps = mantissa * 1000f64.powi(exponent as i32);
    bps / 1e9
}

pub fn parse_ssp_info(b: &[u8]) -> Option<SspInfo> {
    let rx = u32le(b, 8)?;
    let rx_lanes = u32le(b, 12)?.max(1);
    let tx = u32le(b, 16)?;
    let tx_lanes = u32le(b, 20)?.max(1);
    Some(SspInfo {
        rx_gbps: ssp_speed_gbps(rx) * f64::from(rx_lanes),
        rx_lanes,
        tx_gbps: ssp_speed_gbps(tx) * f64::from(tx_lanes),
        tx_lanes,
    })
}

/// From a BOS descriptor (deep probe): highest SuperSpeed capability.
/// Device capability types (USB 3.2 spec, table 9-14): 0x03 SuperSpeed USB,
/// 0x0A SuperSpeedPlus.
pub fn bos_max_class(bos: &[u8]) -> Option<linkspec_core::types::UsbGen> {
    use linkspec_core::types::UsbGen;
    if bos.len() < 5 || bos[1] != 0x0f {
        return None;
    }
    let total = usize::from(u16le(bos, 2)?).min(bos.len());
    let mut off = usize::from(bos[0]);
    let mut best: Option<UsbGen> = Some(UsbGen::Usb2);
    while off + 3 <= total {
        let len = usize::from(bos[off]);
        if len < 3 {
            break;
        }
        if bos[off + 1] == 0x10 {
            match bos[off + 2] {
                0x03 => best = best.max(Some(UsbGen::Gen1)),
                0x0a => best = best.max(Some(UsbGen::Gen2)),
                _ => {}
            }
        }
        off += len;
    }
    best
}

/// Decode a USB string descriptor (UTF-16LE, deep probe).
pub fn parse_string_descriptor(b: &[u8]) -> Option<String> {
    if b.len() < 2 || b[1] != 0x03 {
        return None;
    }
    let len = usize::from(b[0]).min(b.len());
    let words: Vec<u16> = b[2..len]
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let s = String::from_utf16_lossy(&words).trim().to_string();
    (!s.is_empty()).then_some(s)
}

// --- Builders for tests and fixtures ------------------------------------

#[doc(hidden)]
pub mod build {
    pub fn hub_info_ex(hub_type: u32, ports: u16) -> Vec<u8> {
        let mut b = vec![0u8; 8];
        b[0..4].copy_from_slice(&hub_type.to_le_bytes());
        b[4..6].copy_from_slice(&ports.to_le_bytes());
        b
    }

    pub fn connector_props(
        port: u32,
        user: bool,
        type_c: bool,
        companion_port: u16,
        companion_hub: Option<&str>,
    ) -> Vec<u8> {
        let name: Vec<u16> = companion_hub
            .map(|s| s.encode_utf16().chain(std::iter::once(0)).collect())
            .unwrap_or_else(|| vec![0]);
        let total = 16 + name.len() * 2;
        let mut b = vec![0u8; total];
        b[0..4].copy_from_slice(&port.to_le_bytes());
        b[4..8].copy_from_slice(&(total as u32).to_le_bytes());
        let props = u32::from(user) | (u32::from(type_c) << 3);
        b[8..12].copy_from_slice(&props.to_le_bytes());
        b[12..14].copy_from_slice(&(u16::from(companion_hub.is_some())).to_le_bytes());
        b[14..16].copy_from_slice(&companion_port.to_le_bytes());
        for (i, w) in name.iter().enumerate() {
            b[16 + i * 2..18 + i * 2].copy_from_slice(&w.to_le_bytes());
        }
        b
    }

    #[allow(clippy::too_many_arguments)]
    pub fn conn_info_ex(
        port: u32,
        status: u32,
        speed: u8,
        hub: bool,
        bcd_usb: u16,
        vid: u16,
        pid: u16,
        class: u8,
    ) -> Vec<u8> {
        let mut b = vec![0u8; super::CONN_INFO_EX_SIZE];
        b[0..4].copy_from_slice(&port.to_le_bytes());
        b[4] = 18;
        b[5] = 1;
        b[6..8].copy_from_slice(&bcd_usb.to_le_bytes());
        b[8] = class;
        b[12..14].copy_from_slice(&vid.to_le_bytes());
        b[14..16].copy_from_slice(&pid.to_le_bytes());
        b[19] = 2; // iProduct
        b[23] = speed;
        b[24] = u8::from(hub);
        b[31..35].copy_from_slice(&status.to_le_bytes());
        b
    }

    pub fn conn_info_v2(port: u32, protocols: u32, flags: u32) -> Vec<u8> {
        let mut b = vec![0u8; 16];
        b[0..4].copy_from_slice(&port.to_le_bytes());
        b[4..8].copy_from_slice(&16u32.to_le_bytes());
        b[8..12].copy_from_slice(&protocols.to_le_bytes());
        b[12..16].copy_from_slice(&flags.to_le_bytes());
        b
    }

    /// Lane speed in Gbit/s (exponent 3).
    pub fn ssp_info(port: u32, lane_gbps: u16, lanes: u32) -> Vec<u8> {
        let speed = (u32::from(lane_gbps) << 16) | (3 << 4);
        let mut b = vec![0u8; 24];
        b[0..4].copy_from_slice(&port.to_le_bytes());
        b[4..8].copy_from_slice(&24u32.to_le_bytes());
        b[8..12].copy_from_slice(&speed.to_le_bytes());
        b[12..16].copy_from_slice(&lanes.to_le_bytes());
        b[16..20].copy_from_slice(&speed.to_le_bytes());
        b[20..24].copy_from_slice(&lanes.to_le_bytes());
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use linkspec_core::types::UsbGen;

    #[test]
    fn conn_info_roundtrip() {
        let b = build::conn_info_ex(3, 1, 2, false, 0x0210, 0x0781, 0x5581, 0);
        let c = parse_conn_info_ex(&b).unwrap();
        assert_eq!(c.status, ConnStatus::Connected);
        assert_eq!(c.speed, 2);
        assert_eq!(c.vendor_id, 0x0781);
        assert_eq!(c.bcd_usb, 0x0210);
        assert_eq!(c.i_product, 2);
        assert!(parse_conn_info_ex(&b[..20]).is_none());
    }

    #[test]
    fn overcurrent_status() {
        let b = build::conn_info_ex(1, 4, 0, false, 0, 0, 0, 0);
        assert_eq!(
            parse_conn_info_ex(&b).unwrap().status,
            ConnStatus::Overcurrent
        );
    }

    #[test]
    fn connector_props_with_companion() {
        let b = build::connector_props(2, true, true, 5, Some("hub-usb2"));
        let p = parse_connector_props(&b).unwrap();
        assert!(p.user_connectable && p.type_c);
        assert_eq!(p.companion_port, 5);
        assert_eq!(p.companion_hub_link.as_deref(), Some("hub-usb2"));
    }

    #[test]
    fn ssp_rates() {
        let s = parse_ssp_info(&build::ssp_info(1, 10, 2)).unwrap();
        assert!((s.rx_gbps - 20.0).abs() < 1e-9);
        assert_eq!(s.rx_lanes, 2);
    }

    #[test]
    fn v2_flags() {
        let v = parse_conn_info_v2(&build::conn_info_v2(1, 0b110, 0b1010)).unwrap();
        assert!(v.usb300 && v.ss_capable && v.ssp_capable && !v.operating_at_ss);
    }

    #[test]
    fn bos_superspeed_plus() {
        // BOS header (5) + SS cap (10) + SSP cap (12).
        let mut b = vec![5u8, 0x0f, 27, 0, 2];
        b.extend([10, 0x10, 0x03, 0, 0, 0, 0, 0, 0, 0]);
        b.extend([12, 0x10, 0x0a, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(bos_max_class(&b), Some(UsbGen::Gen2));
        assert_eq!(bos_max_class(&[5, 0x0f, 5, 0, 0]), Some(UsbGen::Usb2));
    }

    #[test]
    fn string_descriptor() {
        let mut b = vec![0u8, 3];
        for w in "Disk".encode_utf16() {
            b.extend(w.to_le_bytes());
        }
        b[0] = b.len() as u8;
        assert_eq!(parse_string_descriptor(&b).as_deref(), Some("Disk"));
    }
}
