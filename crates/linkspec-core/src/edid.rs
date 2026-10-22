//! Minimal EDID decoder: monitor name, colour depth, and the detailed
//! timing descriptors (base block and CTA-861 extensions).
//!
//! Source: VESA E-EDID Standard Release A, Revision 2 (structure of the base
//! block, 18-byte descriptors) and CTA-861 (extension block, byte 2 is the
//! DTD offset). The serial number (descriptor tag 0xFF and bytes 12..15) is
//! never decoded or stored.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdidMode {
    pub width: u32,
    pub height: u32,
    /// Refresh rate in millihertz (integer to keep `Eq`).
    pub refresh_mhz: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdidSummary {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bits_per_color: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred: Option<EdidMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<EdidMode>,
}

const HEADER: [u8; 8] = [0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00];

fn dtd(d: &[u8]) -> Option<EdidMode> {
    if d.len() < 18 {
        return None;
    }
    let clock_khz = u32::from(u16::from_le_bytes([d[0], d[1]])) * 10;
    if clock_khz == 0 {
        return None;
    }
    let h_active = u32::from(d[2]) | (u32::from(d[4] & 0xf0) << 4);
    let h_blank = u32::from(d[3]) | (u32::from(d[4] & 0x0f) << 8);
    let v_active = u32::from(d[5]) | (u32::from(d[7] & 0xf0) << 4);
    let v_blank = u32::from(d[6]) | (u32::from(d[7] & 0x0f) << 8);
    let total = (h_active + h_blank) * (v_active + v_blank);
    if total == 0 || h_active == 0 || v_active == 0 {
        return None;
    }
    let refresh_mhz = (u64::from(clock_khz) * 1_000_000 / u64::from(total)) as u32;
    Some(EdidMode {
        width: h_active,
        height: v_active,
        refresh_mhz,
    })
}

fn score(m: &EdidMode) -> u64 {
    u64::from(m.width) * u64::from(m.height) * u64::from(m.refresh_mhz)
}

pub fn parse(bytes: &[u8]) -> Option<EdidSummary> {
    if bytes.len() < 128 || bytes[..8] != HEADER {
        return None;
    }
    let mut modes = Vec::new();
    let mut name = None;
    for off in [54usize, 72, 90, 108] {
        let d = &bytes[off..off + 18];
        if d[0] != 0 || d[1] != 0 {
            if let Some(m) = dtd(d) {
                modes.push(m);
            }
        } else if d[3] == 0xfc {
            let text: String = d[5..18]
                .iter()
                .take_while(|b| **b != 0x0a && **b != 0)
                .map(|b| char::from(*b))
                .collect();
            let t = text.trim().to_string();
            if !t.is_empty() {
                name = Some(t);
            }
        }
    }
    let input = bytes[20];
    let bits_per_color = if input & 0x80 != 0 {
        match (input >> 4) & 0x07 {
            1 => Some(6),
            2 => Some(8),
            3 => Some(10),
            4 => Some(12),
            5 => Some(14),
            6 => Some(16),
            _ => None,
        }
    } else {
        None
    };
    // CTA-861 extensions.
    let ext = usize::from(bytes[126]);
    for i in 1..=ext {
        let b = i * 128;
        if bytes.len() < b + 128 {
            break;
        }
        let blk = &bytes[b..b + 128];
        if blk[0] != 0x02 {
            continue;
        }
        let mut off = usize::from(blk[2]);
        if off < 4 {
            continue;
        }
        while off + 18 <= 127 {
            if let Some(m) = dtd(&blk[off..off + 18]) {
                modes.push(m);
            }
            off += 18;
        }
    }
    let preferred = modes.first().copied();
    let max = modes.iter().copied().max_by_key(score);
    Some(EdidSummary {
        name,
        bits_per_color,
        preferred,
        max,
    })
}

/// Build a synthetic EDID (tests and fixtures only).
#[doc(hidden)]
pub fn synthetic(name: &str, w: u32, h: u32, hz: u32) -> Vec<u8> {
    let mut e = vec![0u8; 128];
    e[..8].copy_from_slice(&HEADER);
    e[20] = 0x80 | (3 << 4); // digital, 10 bpc
    // DTD: CVT-RB like timing with 160 px / 62 lines blanking.
    let (hb, vb) = (160u32, 62u32);
    let clock_khz = (w + hb) * (h + vb) * hz / 1000;
    let c = (clock_khz / 10) as u16;
    let d = &mut e[54..72];
    d[0..2].copy_from_slice(&c.to_le_bytes());
    d[2] = (w & 0xff) as u8;
    d[3] = (hb & 0xff) as u8;
    d[4] = (((w >> 8) as u8) << 4) | ((hb >> 8) as u8);
    d[5] = (h & 0xff) as u8;
    d[6] = (vb & 0xff) as u8;
    d[7] = (((h >> 8) as u8) << 4) | ((vb >> 8) as u8);
    let n = &mut e[72..90];
    n[3] = 0xfc;
    for (i, b) in name.bytes().take(13).enumerate() {
        n[5 + i] = b;
    }
    if name.len() < 13 {
        n[5 + name.len()] = 0x0a;
    }
    // Serial descriptor that must never be decoded.
    let s = &mut e[90..108];
    s[3] = 0xff;
    s[5..11].copy_from_slice(b"SECRET");
    e
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_name_and_mode_without_serial() {
        let e = synthetic("Example 27", 3840, 2160, 60);
        let s = parse(&e).unwrap();
        assert_eq!(s.name.as_deref(), Some("Example 27"));
        let m = s.max.unwrap();
        assert_eq!((m.width, m.height), (3840, 2160));
        assert!((59_000..61_000).contains(&m.refresh_mhz), "{}", m.refresh_mhz);
        assert_eq!(s.bits_per_color, Some(10));
        assert!(!serde_json::to_string(&s).unwrap().contains("SECRET"));
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse(&[0u8; 10]).is_none());
        assert!(parse(&[1u8; 128]).is_none());
    }
}
