//! Bundled reference data (`vendors.db`, read-only SQLite).
//!
//! The database is generated at build time by `data/build_vendors.rs` and
//! embedded into the binary. Nothing is looked up over the network.

#![forbid(unsafe_code)]

use std::sync::Mutex;

use linkspec_core::Catalog;
use rusqlite::{Connection, MAIN_DB, OptionalExtension};

static VENDORS_DB: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/vendors.db"));

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("vendors.db: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

/// Version and license of the bundled data (one line in the settings).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbInfo {
    pub usb_ids_version: String,
    pub usb_ids_license: String,
    pub vendor_count: u32,
    pub xid_count: u32,
    pub cable_count: u32,
}

/// The bundled catalog. Thread-safe (the connection sits behind a mutex).
pub struct VendorDb {
    conn: Mutex<Connection>,
}

impl VendorDb {
    /// Open the embedded database (read-only, in memory).
    pub fn open_bundled() -> Result<Self, DbError> {
        let mut conn = Connection::open_in_memory()?;
        conn.deserialize_bytes(MAIN_DB, VENDORS_DB)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn with<T>(&self, f: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> Option<T> {
        let c = self.conn.lock().ok()?;
        f(&c).ok()
    }

    pub fn info(&self) -> DbInfo {
        let meta = |k: &str| {
            self.with(|c| c.query_row("SELECT value FROM meta WHERE key = ?1", [k], |r| r.get(0)))
                .unwrap_or_else(|| "unknown".to_string())
        };
        let count = |t: &str| {
            self.with(|c| c.query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get(0)))
                .unwrap_or(0)
        };
        DbInfo {
            usb_ids_version: meta("usb_ids_version"),
            usb_ids_license: meta("usb_ids_license"),
            vendor_count: count("vendors"),
            xid_count: count("xids"),
            cable_count: count("cables"),
        }
    }

    /// Size of the embedded file in bytes.
    pub fn embedded_size() -> usize {
        VENDORS_DB.len()
    }
}

impl Catalog for VendorDb {
    fn vendor_name(&self, vid: u16) -> Option<String> {
        self.with(|c| {
            c.query_row("SELECT name FROM vendors WHERE vid = ?1", [vid], |r| {
                r.get(0)
            })
            .optional()
        })
        .flatten()
    }

    fn certified_name(&self, xid: u32) -> Option<String> {
        self.with(|c| {
            c.query_row("SELECT name FROM xids WHERE xid = ?1", [xid], |r| r.get(0))
                .optional()
        })
        .flatten()
    }

    fn has_certification_list(&self) -> bool {
        self.info().xid_count > 0
    }

    fn known_cable(&self, fingerprint: &str) -> Option<String> {
        self.with(|c| {
            c.query_row(
                "SELECT name FROM cables WHERE fingerprint = ?1",
                [fingerprint],
                |r| r.get(0),
            )
            .optional()
        })
        .flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_db_has_vendors() {
        let db = VendorDb::open_bundled().unwrap();
        let info = db.info();
        assert!(info.vendor_count > 1000, "{info:?}");
        assert_eq!(info.usb_ids_license, "BSD-3-Clause");
        // 0x05ac is listed in usb.ids.
        assert!(db.vendor_name(0x05ac).is_some());
        assert_eq!(db.certified_name(0x1234), None);
        assert!(!db.has_certification_list());
    }

    #[test]
    fn db_is_read_only() {
        let db = VendorDb::open_bundled().unwrap();
        let c = db.conn.lock().unwrap();
        assert!(
            c.execute("INSERT INTO vendors (vid, name) VALUES (1, 'x')", [])
                .is_err()
        );
    }
}
