//! Small shared value types: USB signaling classes, Thunderbolt generations,
//! data sources and confidence levels.

use serde::{Deserialize, Serialize};

/// Highest USB signaling class of a port, cable, device or link.
///
/// The classes follow the "USB Highest Speed" field of the USB PD cable and
/// UFP VDOs (USB PD R3.2, §6.4.4.3.1.4 and §6.4.4.3.1.6):
/// `000` USB 2.0, `001` USB 3.2 Gen1, `010` USB 3.2/USB4 Gen2,
/// `011` USB4 Gen3, `100` USB4 Gen4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsbGen {
    Usb2,
    Gen1,
    Gen2,
    Gen3,
    Gen4,
}

impl UsbGen {
    /// Decode the 3-bit "USB Highest Speed" field. Reserved values return `None`.
    pub fn from_pd_bits(bits: u32) -> Option<Self> {
        match bits & 0b111 {
            0 => Some(Self::Usb2),
            1 => Some(Self::Gen1),
            2 => Some(Self::Gen2),
            3 => Some(Self::Gen3),
            4 => Some(Self::Gen4),
            _ => None,
        }
    }

    /// Nominal data rate in Gbit/s that a full-featured USB Type-C cable of
    /// this class carries, using the USB-IF certified cable logo names
    /// ("USB 5Gbps", "USB 20Gbps", "USB 40Gbps", "USB 80Gbps").
    /// See `docs/ASSUMPTIONS.md` (A-004).
    pub fn cable_gbps(self) -> f64 {
        match self {
            Self::Usb2 => 0.48,
            Self::Gen1 => 5.0,
            Self::Gen2 => 20.0,
            Self::Gen3 => 40.0,
            Self::Gen4 => 80.0,
        }
    }

    /// Nominal single-link rate in Gbit/s for a host port or device of this class.
    pub fn link_gbps(self) -> f64 {
        match self {
            Self::Usb2 => 0.48,
            Self::Gen1 => 5.0,
            Self::Gen2 => 10.0,
            Self::Gen3 => 40.0,
            Self::Gen4 => 80.0,
        }
    }

    /// Classify an observed link rate in Mbit/s (Linux `speed`, macOS
    /// "Device Speed", Windows `USB_DEVICE_SPEED`) into a class.
    pub fn from_link_mbps(mbps: f64) -> Self {
        if mbps <= 480.0 {
            Self::Usb2
        } else if mbps <= 5000.0 {
            Self::Gen1
        } else if mbps <= 20000.0 {
            Self::Gen2
        } else if mbps <= 40000.0 {
            Self::Gen3
        } else {
            Self::Gen4
        }
    }

    pub fn is_superspeed(self) -> bool {
        self >= Self::Gen1
    }
}

/// Thunderbolt or USB4 generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FabricGen {
    Tb3,
    Usb4V1,
    Tb4,
    Usb4V2,
    Tb5,
}

impl FabricGen {
    /// Maximum symmetric link rate in Gbit/s (both lanes).
    pub fn max_gbps(self) -> f64 {
        match self {
            Self::Tb3 | Self::Usb4V1 | Self::Tb4 => 40.0,
            Self::Usb4V2 | Self::Tb5 => 80.0,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tb3 => "tb3",
            Self::Usb4V1 => "usb4_v1",
            Self::Tb4 => "tb4",
            Self::Usb4V2 => "usb4_v2",
            Self::Tb5 => "tb5",
        }
    }
}

/// Active transport on a port. Only set when the platform proves it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
    Usb2,
    Usb3,
    Usb4,
    Thunderbolt,
    Displayport,
}

impl Transport {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Usb2 => "usb2",
            Self::Usb3 => "usb3",
            Self::Usb4 => "usb4",
            Self::Thunderbolt => "thunderbolt",
            Self::Displayport => "displayport",
        }
    }
}

/// Where a value came from. Serialized into `evidence[].source`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// macOS IOKit registry.
    Iokit,
    /// macOS CoreGraphics display APIs.
    Coregraphics,
    /// Linux sysfs.
    Sysfs,
    /// Windows configuration manager / SetupAPI device properties.
    Setupapi,
    /// Windows USB hub IOCTLs.
    UsbIoctl,
    /// Windows battery IOCTLs.
    BatteryIoctl,
    /// Windows Energy Meter Interface.
    Emi,
    /// Windows display configuration API.
    Displayconfig,
    /// Windows LinkSpec service (ETW, tier 2).
    Service,
    /// Synthetic or recorded fixture.
    Fixture,
    /// Computed by LinkSpec from other values.
    #[default]
    Derived,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Iokit => "iokit",
            Self::Coregraphics => "coregraphics",
            Self::Sysfs => "sysfs",
            Self::Setupapi => "setupapi",
            Self::UsbIoctl => "usb_ioctl",
            Self::BatteryIoctl => "battery_ioctl",
            Self::Emi => "emi",
            Self::Displayconfig => "displayconfig",
            Self::Service => "service",
            Self::Fixture => "fixture",
            Self::Derived => "derived",
        }
    }
}

/// How much a value can be trusted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    /// Reported by a component (for example an E-Marker). Can be wrong.
    Reported,
    /// Measured or negotiated by a controller.
    Measured,
    /// Computed from other values.
    Derived,
}

/// Format a number of Gbit/s for display: integers without decimals.
pub fn fmt_number(value: f64, decimal_comma: bool) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    let s = if (rounded - rounded.round()).abs() < 1e-9 {
        format!("{}", rounded.round() as i64)
    } else if ((rounded * 10.0) - (rounded * 10.0).round()).abs() < 1e-9 {
        format!("{rounded:.1}")
    } else {
        format!("{rounded:.2}")
    };
    if decimal_comma { s.replace('.', ",") } else { s }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usb_gen_from_bits() {
        assert_eq!(UsbGen::from_pd_bits(0), Some(UsbGen::Usb2));
        assert_eq!(UsbGen::from_pd_bits(3), Some(UsbGen::Gen3));
        assert_eq!(UsbGen::from_pd_bits(4), Some(UsbGen::Gen4));
        assert_eq!(UsbGen::from_pd_bits(5), None);
        assert_eq!(UsbGen::from_pd_bits(7), None);
    }

    #[test]
    fn usb_gen_from_mbps() {
        assert_eq!(UsbGen::from_link_mbps(12.0), UsbGen::Usb2);
        assert_eq!(UsbGen::from_link_mbps(480.0), UsbGen::Usb2);
        assert_eq!(UsbGen::from_link_mbps(5000.0), UsbGen::Gen1);
        assert_eq!(UsbGen::from_link_mbps(10000.0), UsbGen::Gen2);
        assert_eq!(UsbGen::from_link_mbps(20000.0), UsbGen::Gen2);
        assert_eq!(UsbGen::from_link_mbps(40000.0), UsbGen::Gen3);
    }

    #[test]
    fn number_format() {
        assert_eq!(fmt_number(40.0, true), "40");
        assert_eq!(fmt_number(0.48, true), "0,48");
        assert_eq!(fmt_number(0.48, false), "0.48");
        assert_eq!(fmt_number(27.5, true), "27,5");
    }
}
