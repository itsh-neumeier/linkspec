//! Display bandwidth estimates.
//!
//! * Pixel clock: active pixels × refresh × blanking factor. We use the
//!   VESA CVT reduced blanking v2 overhead as a conservative approximation
//!   (≈ 8 % horizontal + vertical blanking). Source: VESA CVT 2.0.
//! * DisplayPort payload: lanes × lane rate × channel coding efficiency.
//!   8b/10b for RBR/HBR/HBR2/HBR3 (≤ 8.1 Gbit/s per lane), 128b/132b for
//!   UHBR10/13.5/20. Source: VESA DisplayPort 1.4a / 2.1 (public summaries).
//! * DSC: compresses to about 12 bits per pixel; we use a factor of 2.5 for
//!   24/30-bit content as a conservative estimate.

use crate::observation::DisplayMode;

const BLANKING_FACTOR: f64 = 1.08;
const DSC_FACTOR: f64 = 2.5;

/// Required link payload in Gbit/s for a mode.
pub fn required_gbps(mode: &DisplayMode, dsc: bool) -> f64 {
    let bpc = f64::from(mode.bits_per_color.unwrap_or(8));
    let bits = f64::from(mode.width) * f64::from(mode.height) * mode.refresh_hz * BLANKING_FACTOR
        * bpc
        * 3.0;
    let gbps = bits / 1e9;
    let gbps = if dsc { gbps / DSC_FACTOR } else { gbps };
    (gbps * 100.0).round() / 100.0
}

/// Usable payload of a DisplayPort link in Gbit/s.
pub fn dp_payload_gbps(lanes: u8, lane_gbps: f64) -> f64 {
    let efficiency = if lane_gbps >= 10.0 { 128.0 / 132.0 } else { 0.8 };
    ((f64::from(lanes) * lane_gbps * efficiency) * 100.0).round() / 100.0
}

/// True when two modes are the same resolution and refresh (±1 Hz).
pub fn same_mode(a: &DisplayMode, b: &DisplayMode) -> bool {
    a.width == b.width && a.height == b.height && (a.refresh_hz - b.refresh_hz).abs() < 1.0
}

/// True when `a` is strictly below `b` in pixels or refresh rate.
pub fn below(a: &DisplayMode, b: &DisplayMode) -> bool {
    let pa = u64::from(a.width) * u64::from(a.height);
    let pb = u64::from(b.width) * u64::from(b.height);
    pa < pb || (pa == pb && a.refresh_hz + 1.0 < b.refresh_hz)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(w: u32, h: u32, hz: f64) -> DisplayMode {
        DisplayMode {
            width: w,
            height: h,
            refresh_hz: hz,
            bits_per_color: Some(8),
        }
    }

    #[test]
    fn uhd60_needs_about_13_gbps() {
        let r = required_gbps(&m(3840, 2160, 60.0), false);
        assert!((12.0..15.0).contains(&r), "{r}");
    }

    #[test]
    fn hbr2_two_lanes_cannot_carry_uhd60_without_dsc() {
        let cap = dp_payload_gbps(2, 5.4);
        assert!(required_gbps(&m(3840, 2160, 60.0), false) > cap);
        assert!(required_gbps(&m(3840, 2160, 60.0), true) < cap);
    }

    #[test]
    fn uhbr_efficiency() {
        assert!((dp_payload_gbps(4, 20.0) - 77.58).abs() < 0.01);
    }
}
