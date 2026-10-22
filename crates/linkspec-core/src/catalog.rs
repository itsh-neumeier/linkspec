//! Lookups that the core needs but does not own: vendor names, the
//! certification list, the community cable list and the cable history.

use sha2::{Digest, Sha256};

use crate::diagnosis::CableHistoryOut;
use crate::pd::RawIdentity;

/// Read-only reference data (see `linkspec-db`).
pub trait Catalog {
    /// Vendor name for a USB vendor ID.
    fn vendor_name(&self, vid: u16) -> Option<String>;
    /// Product name for a certified XID, if the bundled list has it.
    fn certified_name(&self, xid: u32) -> Option<String>;
    /// Whether a certification list is bundled at all.
    fn has_certification_list(&self) -> bool;
    /// Name of a known cable from the community fingerprint list.
    fn known_cable(&self, fingerprint: &str) -> Option<String>;
}

/// A catalog without data.
#[derive(Debug, Default, Clone, Copy)]
pub struct EmptyCatalog;

impl Catalog for EmptyCatalog {
    fn vendor_name(&self, _vid: u16) -> Option<String> {
        None
    }
    fn certified_name(&self, _xid: u32) -> Option<String> {
        None
    }
    fn has_certification_list(&self) -> bool {
        false
    }
    fn known_cable(&self, _fingerprint: &str) -> Option<String> {
        None
    }
}

/// History lookup for the port card (see `linkspec-history`).
pub trait HistoryLookup {
    fn cable(&self, fingerprint: &str) -> Option<CableHistoryOut>;
}

/// Stable fingerprint of an E-Marker cable: SHA-256 over VID, PID, bcdDevice,
/// XID and the cable VDOs. The ID header is included without reserved bits
/// so that the value does not change between platforms that mask them.
/// Cables without E-Marker have no fingerprint.
pub fn cable_fingerprint(id: &RawIdentity) -> String {
    let mut h = Sha256::new();
    h.update(b"linkspec-cable-v1");
    h.update(id.id_header.to_le_bytes());
    h.update(id.cert_stat.to_le_bytes());
    h.update(id.product.to_le_bytes());
    for v in &id.product_type_vdos {
        h.update(v.to_le_bytes());
    }
    let digest = h.finalize();
    digest.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_is_stable_and_distinct() {
        let a = RawIdentity {
            id_header: 0x1800_1234,
            cert_stat: 0,
            product: 0x5678_0001,
            product_type_vdos: vec![0x0001_e0c3],
        };
        let mut b = a.clone();
        b.product_type_vdos = vec![0x0001_e0c2];
        assert_eq!(cable_fingerprint(&a), cable_fingerprint(&a.clone()));
        assert_ne!(cable_fingerprint(&a), cable_fingerprint(&b));
        assert_eq!(cable_fingerprint(&a).len(), 16);
    }
}
