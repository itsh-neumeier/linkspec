//! USB Power Delivery decoders.
//!
//! Reference: *Universal Serial Bus Power Delivery Specification, Revision 3.2,
//! Version 1.1* (USB-IF, 2024), available at <https://www.usb.org/documents>.
//!
//! * Power Data Objects: §6.4.1.2 (Source Capabilities) and §6.4.1.3 (Sink
//!   Capabilities), Augmented PDOs in §6.4.1.2.4.
//! * Request Data Object: §6.4.2.
//! * Discover Identity VDOs: §6.4.4.3.1 (ID Header, Cert Stat, Product,
//!   UFP, DFP, Passive Cable, Active Cable 1 and 2, VPD).
//!
//! Rules: decoders accept every 32-bit value. Reserved bit patterns never
//! fail; they are reported in `reserved` so that the trust checks can flag
//! them. Older revisions (PD 2.0/3.0) use subsets of the same layout.

use serde::{Deserialize, Serialize};

use crate::types::UsbGen;

fn bits(v: u32, hi: u32, lo: u32) -> u32 {
    (v >> lo) & ((1u32 << (hi - lo + 1)) - 1)
}

/// Scale a raw field to a unit value, rounded to avoid float noise.
fn q(raw: u32, factor: f64) -> f64 {
    (f64::from(raw) * factor * 1000.0).round() / 1000.0
}

fn bit(v: u32, n: u32) -> bool {
    (v >> n) & 1 == 1
}

// ---------------------------------------------------------------------------
// Power Data Objects
// ---------------------------------------------------------------------------

/// A decoded Power Data Object. Values are in volts, amperes and watts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Pdo {
    Fixed {
        voltage_v: f64,
        max_current_a: f64,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        epr_capable: bool,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        usb_comm_capable: bool,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        unconstrained_power: bool,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        dual_role_power: bool,
    },
    Battery {
        min_voltage_v: f64,
        max_voltage_v: f64,
        max_power_w: f64,
    },
    Variable {
        min_voltage_v: f64,
        max_voltage_v: f64,
        max_current_a: f64,
    },
    Pps {
        min_voltage_v: f64,
        max_voltage_v: f64,
        max_current_a: f64,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        power_limited: bool,
    },
    SprAvs {
        max_current_15v_a: f64,
        max_current_20v_a: f64,
    },
    EprAvs {
        min_voltage_v: f64,
        max_voltage_v: f64,
        pdp_w: f64,
    },
    /// APDO with a reserved subtype or an otherwise unknown layout.
    Unknown { raw: u32 },
}

/// Which side the PDO describes. Fixed sink PDOs carry the operational
/// current instead of a maximum current, but the bit positions match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdoRole {
    Source,
    Sink,
}

impl Pdo {
    /// Decode one 32-bit PDO.
    pub fn decode(raw: u32, role: PdoRole) -> Self {
        match bits(raw, 31, 30) {
            0b00 => {
                let source = role == PdoRole::Source;
                Pdo::Fixed {
                    voltage_v: q(bits(raw, 19, 10), 0.05),
                    max_current_a: q(bits(raw, 9, 0), 0.01),
                    epr_capable: source && bit(raw, 23),
                    usb_comm_capable: bit(raw, 26),
                    unconstrained_power: bit(raw, 27),
                    dual_role_power: bit(raw, 29),
                }
            }
            0b01 => Pdo::Battery {
                max_voltage_v: q(bits(raw, 29, 20), 0.05),
                min_voltage_v: q(bits(raw, 19, 10), 0.05),
                max_power_w: q(bits(raw, 9, 0), 0.25),
            },
            0b10 => Pdo::Variable {
                max_voltage_v: q(bits(raw, 29, 20), 0.05),
                min_voltage_v: q(bits(raw, 19, 10), 0.05),
                max_current_a: q(bits(raw, 9, 0), 0.01),
            },
            _ => match bits(raw, 29, 28) {
                0b00 => Pdo::Pps {
                    max_voltage_v: q(bits(raw, 24, 17), 0.1),
                    min_voltage_v: q(bits(raw, 15, 8), 0.1),
                    max_current_a: q(bits(raw, 6, 0), 0.05),
                    power_limited: bit(raw, 27),
                },
                0b01 => Pdo::EprAvs {
                    max_voltage_v: q(bits(raw, 25, 17), 0.1),
                    min_voltage_v: q(bits(raw, 15, 8), 0.1),
                    pdp_w: q(bits(raw, 7, 0), 1.0),
                },
                0b10 => Pdo::SprAvs {
                    max_current_15v_a: q(bits(raw, 19, 10), 0.01),
                    max_current_20v_a: q(bits(raw, 9, 0), 0.01),
                },
                _ => Pdo::Unknown { raw },
            },
        }
    }

    /// Highest power this PDO can provide (source) or requests (sink).
    pub fn max_w(&self) -> Option<f64> {
        let w = match *self {
            Pdo::Fixed {
                voltage_v,
                max_current_a,
                ..
            } => voltage_v * max_current_a,
            Pdo::Battery { max_power_w, .. } => max_power_w,
            Pdo::Variable {
                max_voltage_v,
                max_current_a,
                ..
            } => max_voltage_v * max_current_a,
            Pdo::Pps {
                max_voltage_v,
                max_current_a,
                ..
            } => max_voltage_v * max_current_a,
            Pdo::SprAvs {
                max_current_15v_a,
                max_current_20v_a,
            } => (15.0 * max_current_15v_a).max(20.0 * max_current_20v_a),
            Pdo::EprAvs { pdp_w, .. } => pdp_w,
            Pdo::Unknown { .. } => return None,
        };
        Some(round2(w))
    }

    /// Highest voltage of this PDO.
    pub fn max_voltage_v(&self) -> Option<f64> {
        match *self {
            Pdo::Fixed { voltage_v, .. } => Some(voltage_v),
            Pdo::Battery { max_voltage_v, .. }
            | Pdo::Variable { max_voltage_v, .. }
            | Pdo::Pps { max_voltage_v, .. }
            | Pdo::EprAvs { max_voltage_v, .. } => Some(max_voltage_v),
            Pdo::SprAvs {
                max_current_20v_a, ..
            } => Some(if max_current_20v_a > 0.0 { 20.0 } else { 15.0 }),
            Pdo::Unknown { .. } => None,
        }
    }

    /// Highest current of this PDO, if it has a current limit.
    pub fn max_current_a(&self) -> Option<f64> {
        match *self {
            Pdo::Fixed { max_current_a, .. }
            | Pdo::Variable { max_current_a, .. }
            | Pdo::Pps { max_current_a, .. } => Some(max_current_a),
            Pdo::SprAvs {
                max_current_15v_a,
                max_current_20v_a,
            } => Some(max_current_15v_a.max(max_current_20v_a)),
            _ => None,
        }
    }

    pub fn kind_str(&self) -> &'static str {
        match self {
            Pdo::Fixed { .. } => "fixed",
            Pdo::Battery { .. } => "battery",
            Pdo::Variable { .. } => "variable",
            Pdo::Pps { .. } => "pps",
            Pdo::SprAvs { .. } => "spr_avs",
            Pdo::EprAvs { .. } => "epr_avs",
            Pdo::Unknown { .. } => "unknown",
        }
    }
}

pub(crate) fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// Decoded Request Data Object for a Fixed or Variable supply (§6.4.2).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FixedRdo {
    /// 1-based object position of the selected PDO.
    pub object_position: u8,
    pub operating_current_a: f64,
    pub max_operating_current_a: f64,
}

impl FixedRdo {
    pub fn decode(raw: u32) -> Self {
        Self {
            object_position: bits(raw, 31, 28) as u8,
            operating_current_a: q(bits(raw, 19, 10), 0.01),
            max_operating_current_a: q(bits(raw, 9, 0), 0.01),
        }
    }
}

// ---------------------------------------------------------------------------
// Discover Identity
// ---------------------------------------------------------------------------

/// Product type reported in the ID Header VDO.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductType {
    NotApplicable,
    Hub,
    Peripheral,
    Host,
    PowerBrick,
    Psd,
    PassiveCable,
    ActiveCable,
    Vpd,
    Reserved,
}

/// Which SOP the identity was read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sop {
    /// Port partner.
    Sop,
    /// Cable plug near end (SOP').
    SopPrime,
    /// Cable plug far end (SOP'').
    SopDoublePrime,
}

/// ID Header VDO, §6.4.4.3.1.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdHeader {
    pub usb_host_capable: bool,
    pub usb_device_capable: bool,
    /// UFP product type (SOP) or cable plug/VPD type (SOP').
    pub product_type_ufp: ProductType,
    pub modal_operation: bool,
    /// DFP product type (SOP only).
    pub product_type_dfp: ProductType,
    pub vendor_id: u16,
    /// Reserved bit patterns that were set.
    pub reserved_set: bool,
}

impl IdHeader {
    pub fn decode(raw: u32, sop: Sop) -> Self {
        let ufp_bits = bits(raw, 29, 27);
        let dfp_bits = bits(raw, 25, 23);
        let mut reserved_set = bits(raw, 20, 16) != 0;
        let product_type_ufp = match sop {
            Sop::Sop => match ufp_bits {
                0 => ProductType::NotApplicable,
                1 => ProductType::Hub,
                2 => ProductType::Peripheral,
                3 => ProductType::Psd,
                _ => {
                    reserved_set = true;
                    ProductType::Reserved
                }
            },
            Sop::SopPrime | Sop::SopDoublePrime => match ufp_bits {
                0 => ProductType::NotApplicable,
                3 => ProductType::PassiveCable,
                4 => ProductType::ActiveCable,
                6 => ProductType::Vpd,
                _ => {
                    reserved_set = true;
                    ProductType::Reserved
                }
            },
        };
        let product_type_dfp = match sop {
            Sop::Sop => match dfp_bits {
                0 => ProductType::NotApplicable,
                1 => ProductType::Hub,
                2 => ProductType::Host,
                3 => ProductType::PowerBrick,
                _ => {
                    reserved_set = true;
                    ProductType::Reserved
                }
            },
            _ => {
                if dfp_bits != 0 {
                    reserved_set = true;
                }
                ProductType::NotApplicable
            }
        };
        Self {
            usb_host_capable: bit(raw, 31),
            usb_device_capable: bit(raw, 30),
            product_type_ufp,
            modal_operation: bit(raw, 26),
            product_type_dfp,
            vendor_id: bits(raw, 15, 0) as u16,
            reserved_set,
        }
    }
}

/// Product VDO, §6.4.4.3.1.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductVdo {
    pub product_id: u16,
    pub bcd_device: u16,
}

impl ProductVdo {
    pub fn decode(raw: u32) -> Self {
        Self {
            product_id: bits(raw, 31, 16) as u16,
            bcd_device: bits(raw, 15, 0) as u16,
        }
    }
}

/// UFP VDO, §6.4.4.3.1.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UfpVdo {
    pub version: u8,
    pub usb2: bool,
    pub usb2_billboard_only: bool,
    pub usb3: bool,
    pub usb4: bool,
    pub alt_mode_tbt3: bool,
    pub alt_mode_reconfigurable: bool,
    pub alt_mode_non_reconfigurable: bool,
    pub usb_highest_speed: Option<UsbGen>,
}

impl UfpVdo {
    pub fn decode(raw: u32) -> Self {
        let caps = bits(raw, 27, 24);
        let alt = bits(raw, 5, 3);
        Self {
            version: bits(raw, 31, 29) as u8,
            usb2: caps & 0b0001 != 0,
            usb2_billboard_only: caps & 0b0010 != 0,
            usb3: caps & 0b0100 != 0,
            usb4: caps & 0b1000 != 0,
            alt_mode_tbt3: alt & 0b001 != 0,
            alt_mode_reconfigurable: alt & 0b010 != 0,
            alt_mode_non_reconfigurable: alt & 0b100 != 0,
            usb_highest_speed: UsbGen::from_pd_bits(bits(raw, 2, 0)),
        }
    }

    /// Highest data class the device itself supports.
    pub fn max_data(&self) -> Option<UsbGen> {
        let from_caps = if self.usb4 || self.usb3 {
            None
        } else if self.usb2 || self.usb2_billboard_only {
            Some(UsbGen::Usb2)
        } else {
            None
        };
        from_caps.or(self.usb_highest_speed)
    }
}

/// DFP VDO, §6.4.4.3.1.5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DfpVdo {
    pub version: u8,
    pub usb2: bool,
    pub usb3: bool,
    pub usb4: bool,
    pub port_number: u8,
}

impl DfpVdo {
    pub fn decode(raw: u32) -> Self {
        let caps = bits(raw, 26, 24);
        Self {
            version: bits(raw, 31, 29) as u8,
            usb2: caps & 0b001 != 0,
            usb3: caps & 0b010 != 0,
            usb4: caps & 0b100 != 0,
            port_number: bits(raw, 4, 0) as u8,
        }
    }
}

/// Physical medium of a cable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CableKind {
    Passive,
    Active,
    Optical,
    Unknown,
}

impl CableKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passive => "passive",
            Self::Active => "active",
            Self::Optical => "optical",
            Self::Unknown => "unknown",
        }
    }
}

/// Decoded cable VDOs (Passive Cable VDO §6.4.4.3.1.6, Active Cable VDO1
/// §6.4.4.3.1.7 and Active Cable VDO2 §6.4.4.3.1.8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CableVdo {
    pub kind: CableKind,
    pub hw_version: u8,
    pub fw_version: u8,
    pub vdo_version: u8,
    pub epr_capable: bool,
    /// Latency code (bits 16..13), roughly the cable length in metres for
    /// passive cables.
    pub latency_code: u8,
    /// Maximum VBUS voltage in volts (20, 30, 40, 50).
    pub max_vbus_v: u8,
    /// VBUS current handling in amperes (3 or 5); `None` for reserved codes.
    pub current_a: Option<f64>,
    pub usb_highest_speed: Option<UsbGen>,
    /// Active cable only: USB4 supported according to VDO2.
    pub usb4_supported: Option<bool>,
    /// Active cable only: two lanes supported according to VDO2.
    pub two_lanes: Option<bool>,
    /// Active cable only: retimer (true) or redriver (false).
    pub retimer: Option<bool>,
    /// Reserved bit patterns that were set.
    pub reserved_set: Vec<String>,
}

impl CableVdo {
    /// Decode cable VDO1 (and VDO2 for active cables).
    pub fn decode(product_type: ProductType, vdo1: u32, vdo2: Option<u32>) -> Self {
        let mut reserved = Vec::new();
        let active = product_type == ProductType::ActiveCable;
        let current_a = match bits(vdo1, 6, 5) {
            0b01 => Some(3.0),
            0b10 => Some(5.0),
            _ => {
                reserved.push("vbus_current".to_string());
                None
            }
        };
        let max_vbus_v = match bits(vdo1, 10, 9) {
            0 => 20,
            1 => 30,
            2 => 40,
            _ => 50,
        };
        let speed = UsbGen::from_pd_bits(bits(vdo1, 2, 0));
        if speed.is_none() {
            reserved.push("usb_highest_speed".to_string());
        }
        if !active && bit(vdo1, 20) {
            reserved.push("passive_bit20".to_string());
        }
        let mut kind = match product_type {
            ProductType::PassiveCable => CableKind::Passive,
            ProductType::ActiveCable => CableKind::Active,
            _ => CableKind::Unknown,
        };
        let (mut usb4_supported, mut two_lanes, mut retimer) = (None, None, None);
        if active && let Some(v2) = vdo2 {
            if bit(v2, 10) {
                kind = CableKind::Optical;
            }
            retimer = Some(bit(v2, 9));
            // Bit 8: 0 = USB4 supported, 1 = not supported.
            usb4_supported = Some(!bit(v2, 8));
            two_lanes = Some(bit(v2, 3));
        }
        Self {
            kind,
            hw_version: bits(vdo1, 31, 28) as u8,
            fw_version: bits(vdo1, 27, 24) as u8,
            vdo_version: bits(vdo1, 23, 21) as u8,
            epr_capable: bit(vdo1, 17),
            latency_code: bits(vdo1, 16, 13) as u8,
            max_vbus_v,
            current_a,
            usb_highest_speed: speed,
            usb4_supported,
            two_lanes,
            retimer,
            reserved_set: reserved,
        }
    }

    /// Watt class of the cable, derived from current and VBUS rating.
    /// 3 A → 60 W; 5 A → 100 W; 5 A with EPR and 50 V → 240 W.
    pub fn power_class_w(&self) -> Option<u32> {
        match self.current_a {
            Some(c) if c >= 5.0 => {
                if self.epr_capable && self.max_vbus_v >= 50 {
                    Some(240)
                } else {
                    Some(100)
                }
            }
            Some(_) => Some(60),
            None => None,
        }
    }
}

/// A full Discover Identity response, decoded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Identity {
    pub header: IdHeader,
    /// USB-IF XID from the Cert Stat VDO. `None` when zero.
    pub xid: Option<u32>,
    pub product: ProductVdo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ufp: Option<UfpVdo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dfp: Option<DfpVdo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cable: Option<CableVdo>,
}

/// Raw Discover Identity VDOs as delivered by a platform.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawIdentity {
    pub id_header: u32,
    #[serde(default)]
    pub cert_stat: u32,
    #[serde(default)]
    pub product: u32,
    /// Product type VDOs 1..3 in message order.
    #[serde(default)]
    pub product_type_vdos: Vec<u32>,
}

impl RawIdentity {
    /// True when the platform delivered an empty identity (all zero).
    pub fn is_empty(&self) -> bool {
        self.id_header == 0 && self.cert_stat == 0 && self.product == 0
    }

    /// Decode the response. Product type VDO order follows §6.4.4.3.1:
    /// for SOP the UFP VDO comes first, then the DFP VDO for DRD products.
    pub fn decode(&self, sop: Sop) -> Identity {
        let header = IdHeader::decode(self.id_header, sop);
        let vdo = |i: usize| self.product_type_vdos.get(i).copied().filter(|v| *v != 0);
        let (mut ufp, mut dfp, mut cable) = (None, None, None);
        match sop {
            Sop::Sop => {
                let ufp_present = matches!(
                    header.product_type_ufp,
                    ProductType::Hub | ProductType::Peripheral
                );
                let dfp_present = matches!(
                    header.product_type_dfp,
                    ProductType::Hub | ProductType::Host | ProductType::PowerBrick
                );
                let mut idx = 0;
                if ufp_present {
                    ufp = vdo(idx).map(UfpVdo::decode);
                    idx += 1;
                    // §6.4.4.3.1: a pad VDO sits between UFP and DFP VDOs.
                    if dfp_present {
                        idx += 1;
                    }
                }
                if dfp_present {
                    dfp = vdo(idx).map(DfpVdo::decode);
                }
            }
            Sop::SopPrime | Sop::SopDoublePrime => {
                if matches!(
                    header.product_type_ufp,
                    ProductType::PassiveCable | ProductType::ActiveCable
                ) && let Some(v1) = vdo(0)
                {
                    cable = Some(CableVdo::decode(header.product_type_ufp, v1, vdo(1)));
                }
            }
        }
        Identity {
            header,
            xid: Some(self.cert_stat).filter(|x| *x != 0),
            product: ProductVdo::decode(self.product),
            ufp,
            dfp,
            cable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_source_pdo_20v_5a_epr() {
        // Fixed, EPR capable (bit 23), USB comm (bit 26), 20 V, 5 A.
        let raw = (1 << 23) | (1 << 26) | (400 << 10) | 500;
        match Pdo::decode(raw, PdoRole::Source) {
            Pdo::Fixed {
                voltage_v,
                max_current_a,
                epr_capable,
                usb_comm_capable,
                ..
            } => {
                assert!((voltage_v - 20.0).abs() < 1e-9);
                assert!((max_current_a - 5.0).abs() < 1e-9);
                assert!(epr_capable);
                assert!(usb_comm_capable);
            }
            other => panic!("unexpected {other:?}"),
        }
        assert_eq!(Pdo::decode(raw, PdoRole::Source).max_w(), Some(100.0));
    }

    #[test]
    fn pps_apdo() {
        // APDO, PPS, 3.3–21 V, 3 A.
        let raw = (0b11 << 30) | (210 << 17) | (33 << 8) | 60;
        let pdo = Pdo::decode(raw, PdoRole::Source);
        assert_eq!(
            pdo,
            Pdo::Pps {
                min_voltage_v: 3.3,
                max_voltage_v: 21.0,
                max_current_a: 3.0,
                power_limited: false
            }
        );
        assert_eq!(pdo.max_w(), Some(63.0));
    }

    #[test]
    fn epr_avs_apdo() {
        let raw = (0b11 << 30) | (0b01 << 28) | (480 << 17) | (150 << 8) | 140;
        let pdo = Pdo::decode(raw, PdoRole::Source);
        assert_eq!(pdo.max_w(), Some(140.0));
        assert_eq!(pdo.kind_str(), "epr_avs");
    }

    #[test]
    fn spr_avs_apdo() {
        let raw = (0b11 << 30) | (0b10 << 28) | (300 << 10) | 225;
        let pdo = Pdo::decode(raw, PdoRole::Source);
        assert_eq!(pdo.max_w(), Some(45.0));
    }

    #[test]
    fn reserved_apdo_is_unknown_not_error() {
        let raw = (0b11 << 30) | (0b11 << 28) | 12345;
        assert!(matches!(
            Pdo::decode(raw, PdoRole::Source),
            Pdo::Unknown { .. }
        ));
    }

    #[test]
    fn rdo_decode() {
        let rdo = FixedRdo::decode((3 << 28) | (300 << 10) | 300);
        assert_eq!(rdo.object_position, 3);
        assert!((rdo.operating_current_a - 3.0).abs() < 1e-9);
    }

    #[test]
    fn id_header_passive_cable() {
        let raw = (3 << 27) | 0x1234;
        let h = IdHeader::decode(raw, Sop::SopPrime);
        assert_eq!(h.product_type_ufp, ProductType::PassiveCable);
        assert_eq!(h.vendor_id, 0x1234);
        assert!(!h.reserved_set);
    }

    #[test]
    fn id_header_reserved_cable_type_flagged() {
        let raw = (5 << 27) | 0x1234;
        let h = IdHeader::decode(raw, Sop::SopPrime);
        assert_eq!(h.product_type_ufp, ProductType::Reserved);
        assert!(h.reserved_set);
    }

    #[test]
    fn passive_cable_vdo_5a_epr_usb4_gen3() {
        // EPR (17), 50 V (10..9 = 11), 5 A (6..5 = 10), Gen3 (2..0 = 011).
        let vdo = (1 << 17) | (0b11 << 9) | (0b10 << 5) | 0b011;
        let c = CableVdo::decode(ProductType::PassiveCable, vdo, None);
        assert_eq!(c.kind, CableKind::Passive);
        assert_eq!(c.current_a, Some(5.0));
        assert_eq!(c.max_vbus_v, 50);
        assert_eq!(c.usb_highest_speed, Some(UsbGen::Gen3));
        assert_eq!(c.power_class_w(), Some(240));
        assert!(c.reserved_set.is_empty());
    }

    #[test]
    fn passive_cable_3a_usb2() {
        let vdo = 0b01 << 5;
        let c = CableVdo::decode(ProductType::PassiveCable, vdo, None);
        assert_eq!(c.power_class_w(), Some(60));
        assert_eq!(c.usb_highest_speed, Some(UsbGen::Usb2));
    }

    #[test]
    fn active_optical_cable() {
        let vdo1 = (0b10 << 5) | 0b011;
        // Optical (bit 10), retimer (bit 9), USB4 supported (bit 8 = 0), 2 lanes.
        let vdo2 = (1 << 10) | (1 << 9) | (1 << 3);
        let c = CableVdo::decode(ProductType::ActiveCable, vdo1, Some(vdo2));
        assert_eq!(c.kind, CableKind::Optical);
        assert_eq!(c.usb4_supported, Some(true));
        assert_eq!(c.retimer, Some(true));
    }

    #[test]
    fn reserved_current_and_speed_are_reported() {
        let vdo = (0b11 << 5) | 0b111;
        let c = CableVdo::decode(ProductType::PassiveCable, vdo, None);
        assert_eq!(c.current_a, None);
        assert_eq!(c.usb_highest_speed, None);
        assert!(c.reserved_set.iter().any(|r| r == "vbus_current"));
        assert!(c.reserved_set.iter().any(|r| r == "usb_highest_speed"));
    }

    #[test]
    fn sop_identity_with_ufp_and_dfp() {
        // UFP peripheral + DFP host (DRD), UFP VDO USB 3.2 Gen2.
        let raw = RawIdentity {
            id_header: (1 << 31) | (1 << 30) | (2 << 27) | (2 << 23) | 0x05ac,
            cert_stat: 0,
            product: 0x1234_0100,
            product_type_vdos: vec![(0b0101 << 24) | 0b010, 0, 0b010 << 24],
        };
        let id = raw.decode(Sop::Sop);
        assert_eq!(id.header.product_type_ufp, ProductType::Peripheral);
        assert_eq!(id.header.product_type_dfp, ProductType::Host);
        assert_eq!(id.ufp.unwrap().usb_highest_speed, Some(UsbGen::Gen2));
        assert!(id.dfp.unwrap().usb3);
        assert_eq!(id.product.product_id, 0x1234);
        assert_eq!(id.xid, None);
    }

    #[test]
    fn ufp_usb2_only_max_data() {
        let ufp = UfpVdo::decode(0b0001 << 24);
        assert_eq!(ufp.max_data(), Some(UsbGen::Usb2));
    }
}
