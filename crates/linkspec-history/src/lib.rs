//! Cable history (prompt 5.9): local SQLite file in the user profile.
//!
//! * Windows: `%LOCALAPPDATA%\LinkSpec\history.db`
//! * macOS: `~/Library/Application Support/LinkSpec/history.db`
//! * Linux: `$XDG_DATA_HOME/linkspec/history.db` (default `~/.local/share`)
//!
//! Only cables with an E-Marker get a fingerprint and can be recognized
//! again. The file is readable by the user only (mode 0600 on Unix; the
//! profile ACL on Windows). Nothing leaves the device.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use linkspec_core::HistoryLookup;
use linkspec_core::diagnosis::CableHistoryOut;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum HistoryError {
    #[error("history database: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("history directory: {0}")]
    Io(#[from] std::io::Error),
    #[error("no data directory (HOME/LOCALAPPDATA not set)")]
    NoDataDir,
}

/// Per-platform data directory of LinkSpec.
pub fn data_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("LinkSpec"))
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .map(|h| PathBuf::from(h).join("Library/Application Support/LinkSpec"))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .map(|p| p.join("linkspec"))
    }
}

/// What the history stores about a cable while it is plugged in.
#[derive(Debug, Clone, PartialEq)]
pub struct CableSighting {
    pub fingerprint: String,
    pub vid: u16,
    pub pid: u16,
    /// Localized short description at the time of the sighting.
    pub summary: String,
    pub contract_w: Option<f64>,
    pub link_gbps: Option<f64>,
    pub transports: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CableRecord {
    pub fingerprint: String,
    pub name: Option<String>,
    pub vid: u16,
    pub pid: u16,
    pub summary: String,
    pub first_seen: String,
    pub last_seen: String,
    pub uses: u32,
    pub faults: u32,
    /// Overall verdict: `ok` (no faults) or `faults`.
    pub verdict: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TimelineEntry {
    pub at: String,
    /// `session` or a fault kind.
    pub kind: String,
    pub port_id: String,
    pub until: Option<String>,
    pub contract_w: Option<f64>,
    pub link_gbps: Option<f64>,
    pub transports: Option<String>,
}

pub struct History {
    conn: Mutex<Connection>,
    /// Open sessions: port id → (fingerprint, session row id).
    open: Mutex<BTreeMap<String, (String, i64)>>,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS cables (
    fingerprint TEXT PRIMARY KEY,
    name TEXT,
    vid INTEGER NOT NULL,
    pid INTEGER NOT NULL,
    summary TEXT NOT NULL,
    first_seen TEXT NOT NULL,
    last_seen TEXT NOT NULL,
    uses INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    fingerprint TEXT NOT NULL REFERENCES cables(fingerprint) ON DELETE CASCADE,
    port_id TEXT NOT NULL,
    started_at TEXT NOT NULL,
    ended_at TEXT,
    contract_w REAL,
    link_gbps REAL,
    transports TEXT
);
CREATE TABLE IF NOT EXISTS events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    fingerprint TEXT REFERENCES cables(fingerprint) ON DELETE CASCADE,
    port_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS sessions_fp ON sessions(fingerprint);
CREATE INDEX IF NOT EXISTS events_fp ON events(fingerprint);
PRAGMA user_version = 1;
";

impl History {
    /// Open (or create) the history in the default data directory.
    pub fn open_default() -> Result<Self, HistoryError> {
        let dir = data_dir().ok_or(HistoryError::NoDataDir)?;
        Self::open(&dir.join("history.db"))
    }

    pub fn open(path: &Path) -> Result<Self, HistoryError> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
            restrict_dir(dir)?;
        }
        let conn = Connection::open(path)?;
        restrict_file(path)?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self, HistoryError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self, HistoryError> {
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self {
            conn: Mutex::new(conn),
            open: Mutex::new(BTreeMap::new()),
        })
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Record the state of one port. `sighting = None` means no E-Marker
    /// cable is on the port (unplugged or not identifiable); an open session
    /// on that port ends.
    pub fn observe_port(
        &self,
        port_id: &str,
        sighting: Option<&CableSighting>,
        now: &str,
    ) -> Result<(), HistoryError> {
        let mut open = self.open.lock().unwrap_or_else(|e| e.into_inner());
        let conn = self.conn();
        let current = open.get(port_id).cloned();
        match (current, sighting) {
            (Some((fp, sid)), Some(s)) if fp == s.fingerprint => {
                conn.execute(
                    "UPDATE sessions SET contract_w = COALESCE(?1, contract_w),
                         link_gbps = COALESCE(?2, link_gbps), transports = ?3 WHERE id = ?4",
                    params![s.contract_w, s.link_gbps, s.transports.join(","), sid],
                )?;
                conn.execute(
                    "UPDATE cables SET last_seen = ?1 WHERE fingerprint = ?2",
                    params![now, fp],
                )?;
            }
            (current, new) => {
                if let Some((_, sid)) = current {
                    conn.execute(
                        "UPDATE sessions SET ended_at = ?1 WHERE id = ?2",
                        params![now, sid],
                    )?;
                    open.remove(port_id);
                }
                if let Some(s) = new {
                    conn.execute(
                        "INSERT INTO cables (fingerprint, vid, pid, summary, first_seen, last_seen, uses)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?5, 1)
                         ON CONFLICT(fingerprint) DO UPDATE SET
                             uses = uses + 1, last_seen = excluded.last_seen,
                             summary = excluded.summary",
                        params![s.fingerprint, s.vid, s.pid, s.summary, now],
                    )?;
                    conn.execute(
                        "INSERT INTO sessions (fingerprint, port_id, started_at, contract_w, link_gbps, transports)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![
                            s.fingerprint,
                            port_id,
                            now,
                            s.contract_w,
                            s.link_gbps,
                            s.transports.join(",")
                        ],
                    )?;
                    open.insert(
                        port_id.to_string(),
                        (s.fingerprint.clone(), conn.last_insert_rowid()),
                    );
                }
            }
        }
        Ok(())
    }

    /// Record a fault. It is linked to the cable on that port, if any.
    pub fn record_fault(&self, port_id: &str, kind: &str, at: &str) -> Result<(), HistoryError> {
        let fp = self
            .open
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(port_id)
            .map(|(f, _)| f.clone());
        self.conn().execute(
            "INSERT INTO events (fingerprint, port_id, kind, at) VALUES (?1, ?2, ?3, ?4)",
            params![fp, port_id, kind, at],
        )?;
        Ok(())
    }

    pub fn rename(&self, fingerprint: &str, name: Option<&str>) -> Result<bool, HistoryError> {
        let n = self.conn().execute(
            "UPDATE cables SET name = ?1 WHERE fingerprint = ?2",
            params![name.map(str::trim).filter(|s| !s.is_empty()), fingerprint],
        )?;
        Ok(n > 0)
    }

    pub fn forget(&self, fingerprint: &str) -> Result<bool, HistoryError> {
        let n = self
            .conn()
            .execute("DELETE FROM cables WHERE fingerprint = ?1", [fingerprint])?;
        Ok(n > 0)
    }

    pub fn list(&self) -> Result<Vec<CableRecord>, HistoryError> {
        let conn = self.conn();
        let mut st = conn.prepare(
            "SELECT c.fingerprint, c.name, c.vid, c.pid, c.summary, c.first_seen, c.last_seen, c.uses,
                    (SELECT COUNT(*) FROM events e WHERE e.fingerprint = c.fingerprint)
             FROM cables c ORDER BY c.last_seen DESC",
        )?;
        let rows = st.query_map([], |r| {
            let faults: u32 = r.get(8)?;
            Ok(CableRecord {
                fingerprint: r.get(0)?,
                name: r.get(1)?,
                vid: r.get(2)?,
                pid: r.get(3)?,
                summary: r.get(4)?,
                first_seen: r.get(5)?,
                last_seen: r.get(6)?,
                uses: r.get(7)?,
                faults,
                verdict: if faults == 0 { "ok" } else { "faults" }.to_string(),
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn get(&self, fingerprint: &str) -> Result<Option<CableRecord>, HistoryError> {
        Ok(self
            .list()?
            .into_iter()
            .find(|c| c.fingerprint == fingerprint))
    }

    pub fn timeline(&self, fingerprint: &str) -> Result<Vec<TimelineEntry>, HistoryError> {
        let conn = self.conn();
        let mut out = Vec::new();
        let mut st = conn.prepare(
            "SELECT started_at, port_id, ended_at, contract_w, link_gbps, transports
             FROM sessions WHERE fingerprint = ?1",
        )?;
        for r in st.query_map([fingerprint], |r| {
            Ok(TimelineEntry {
                at: r.get(0)?,
                kind: "session".into(),
                port_id: r.get(1)?,
                until: r.get(2)?,
                contract_w: r.get(3)?,
                link_gbps: r.get(4)?,
                transports: r.get(5)?,
            })
        })? {
            out.push(r?);
        }
        let mut st = conn.prepare("SELECT at, kind, port_id FROM events WHERE fingerprint = ?1")?;
        for r in st.query_map([fingerprint], |r| {
            Ok(TimelineEntry {
                at: r.get(0)?,
                kind: r.get(1)?,
                port_id: r.get(2)?,
                until: None,
                contract_w: None,
                link_gbps: None,
                transports: None,
            })
        })? {
            out.push(r?);
        }
        out.sort_by(|a, b| a.at.cmp(&b.at));
        Ok(out)
    }

    /// Events without a cable (faults on ports without E-Marker cable).
    pub fn port_events(&self, port_id: &str) -> Result<Vec<TimelineEntry>, HistoryError> {
        let conn = self.conn();
        let mut st = conn.prepare(
            "SELECT at, kind FROM events WHERE port_id = ?1 AND fingerprint IS NULL ORDER BY at",
        )?;
        let rows = st.query_map([port_id], |r| {
            Ok(TimelineEntry {
                at: r.get(0)?,
                kind: r.get(1)?,
                port_id: port_id.to_string(),
                until: None,
                contract_w: None,
                link_gbps: None,
                transports: None,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}

impl HistoryLookup for History {
    fn cable(&self, fingerprint: &str) -> Option<CableHistoryOut> {
        let conn = self.conn();
        conn.query_row(
            "SELECT c.name, c.first_seen, c.uses,
                    (SELECT COUNT(*) FROM events e WHERE e.fingerprint = c.fingerprint)
             FROM cables c WHERE c.fingerprint = ?1",
            [fingerprint],
            |r| {
                Ok(CableHistoryOut {
                    fingerprint: fingerprint.to_string(),
                    name: r.get(0)?,
                    first_seen: r.get(1)?,
                    uses: r.get(2)?,
                    faults: r.get(3)?,
                })
            },
        )
        .optional()
        .ok()
        .flatten()
    }
}

#[cfg(unix)]
fn restrict_dir(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
}

#[cfg(unix)]
fn restrict_file(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn restrict_dir(_dir: &Path) -> std::io::Result<()> {
    // The per-user profile directory (%LOCALAPPDATA%) already has a
    // user-only ACL on Windows.
    Ok(())
}

#[cfg(not(unix))]
fn restrict_file(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sighting(fp: &str) -> CableSighting {
        CableSighting {
            fingerprint: fp.into(),
            vid: 0x1234,
            pid: 0x5678,
            summary: "Passiv, 40 Gbit/s".into(),
            contract_w: Some(60.0),
            link_gbps: Some(40.0),
            transports: vec!["usb4".into()],
        }
    }

    /// Criterion 21: the history recognizes an E-Marker cable after it is
    /// plugged in again (also on another port).
    #[test]
    fn recognizes_cable_after_replug() {
        let h = History::open_in_memory().unwrap();
        let s = sighting("00112233aabbccdd");
        h.observe_port("port-1", Some(&s), "2026-10-20T10:00:00Z")
            .unwrap();
        h.observe_port("port-1", Some(&s), "2026-10-20T10:00:05Z")
            .unwrap();
        h.rename("00112233aabbccdd", Some("Schreibtisch-Kabel"))
            .unwrap();
        h.observe_port("port-1", None, "2026-10-20T11:00:00Z")
            .unwrap();
        h.observe_port("port-2", Some(&s), "2026-10-21T09:00:00Z")
            .unwrap();
        let rec = h.get("00112233aabbccdd").unwrap().unwrap();
        assert_eq!(rec.uses, 2);
        assert_eq!(rec.name.as_deref(), Some("Schreibtisch-Kabel"));
        assert_eq!(rec.first_seen, "2026-10-20T10:00:00Z");
        let card = h.cable("00112233aabbccdd").unwrap();
        assert_eq!(card.uses, 2);
        assert_eq!(h.list().unwrap().len(), 1);
        let tl = h.timeline("00112233aabbccdd").unwrap();
        assert_eq!(tl.len(), 2);
        assert_eq!(tl[0].until.as_deref(), Some("2026-10-20T11:00:00Z"));
    }

    /// Criterion 11 (history part): a fault lands in the history.
    #[test]
    fn fault_lands_in_history() {
        let h = History::open_in_memory().unwrap();
        let s = sighting("aaaaaaaaaaaaaaaa");
        h.observe_port("port-1", Some(&s), "t1").unwrap();
        h.record_fault("port-1", "drop_reconnect", "t2").unwrap();
        h.record_fault("port-3", "overcurrent", "t3").unwrap();
        let rec = h.get("aaaaaaaaaaaaaaaa").unwrap().unwrap();
        assert_eq!(rec.faults, 1);
        assert_eq!(rec.verdict, "faults");
        assert!(
            h.timeline("aaaaaaaaaaaaaaaa")
                .unwrap()
                .iter()
                .any(|e| e.kind == "drop_reconnect")
        );
        assert_eq!(h.port_events("port-3").unwrap().len(), 1);
    }

    #[test]
    fn file_is_user_only() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sub/history.db");
        let h = History::open(&p).unwrap();
        h.observe_port("p", Some(&sighting("bbbbbbbbbbbbbbbb")), "t")
            .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
    }
}
