//! Build script of `linkspec-db`: turns the bundled data sources in `data/`
//! into the read-only SQLite file `vendors.db` (in `OUT_DIR`), which the
//! crate embeds with `include_bytes!`.
//!
//! Inputs (all in `data/`):
//! * `usb.ids` – USB vendor names, from <http://www.linux-usb.org/usb.ids>,
//!   used under the 3-clause BSD license option (see `data/LICENSE.usb-ids`).
//!   Only vendor lines are imported.
//! * `xids.csv` – USB-IF certification list (XID,name). Empty: no list with a
//!   license that allows bundling is known. See `docs/data-sources.md`.
//! * `cables.csv` – community cable fingerprints (fingerprint,name,source).
//!   Starts empty; filled from reports via the "Kabel melden" issue template.

use std::path::{Path, PathBuf};

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let data = manifest.join("../../data");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("vendors.db");
    for f in ["usb.ids", "xids.csv", "cables.csv"] {
        println!("cargo:rerun-if-changed={}", data.join(f).display());
    }
    println!("cargo:rerun-if-changed=../../data/build_vendors.rs");
    build(&data, &out);
}

fn build(data: &Path, out: &Path) {
    let _ = std::fs::remove_file(out);
    let conn = rusqlite::Connection::open(out).expect("create vendors.db");
    conn.execute_batch(
        "PRAGMA page_size = 4096;
         CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
         CREATE TABLE vendors (vid INTEGER PRIMARY KEY, name TEXT NOT NULL);
         CREATE TABLE xids (xid INTEGER PRIMARY KEY, name TEXT NOT NULL);
         CREATE TABLE cables (fingerprint TEXT PRIMARY KEY, name TEXT NOT NULL, source TEXT NOT NULL);",
    )
    .expect("schema");

    let ids = std::fs::read(data.join("usb.ids")).expect("data/usb.ids");
    // usb.ids is mostly UTF-8; be lenient with stray bytes.
    let ids = String::from_utf8_lossy(&ids);
    let mut version = String::from("unknown");
    let tx = conn.unchecked_transaction().unwrap();
    {
        let mut ins = tx
            .prepare("INSERT OR REPLACE INTO vendors (vid, name) VALUES (?1, ?2)")
            .unwrap();
        for line in ids.lines() {
            if let Some(v) = line.strip_prefix("# Version:") {
                version = v.trim().to_string();
                continue;
            }
            // Vendor lines: 4 hex digits, two spaces, name. Stop at the
            // class section ("C 00  ...").
            if line.starts_with("C ") {
                break;
            }
            let b = line.as_bytes();
            if b.len() > 6 && b[..4].iter().all(u8::is_ascii_hexdigit) && &line[4..6] == "  " {
                let vid = u16::from_str_radix(&line[..4], 16).unwrap();
                ins.execute(rusqlite::params![vid, line[6..].trim()])
                    .unwrap();
            }
        }
        let mut ins = tx
            .prepare("INSERT OR REPLACE INTO xids (xid, name) VALUES (?1, ?2)")
            .unwrap();
        for (xid, name) in csv_rows(&data.join("xids.csv")) {
            if let Ok(x) = u32::from_str_radix(xid.trim_start_matches("0x"), 16) {
                ins.execute(rusqlite::params![x, name]).unwrap();
            }
        }
        let mut ins = tx
            .prepare(
                "INSERT OR REPLACE INTO cables (fingerprint, name, source) VALUES (?1, ?2, ?3)",
            )
            .unwrap();
        for line in read_lines(&data.join("cables.csv")) {
            let cols: Vec<&str> = line.splitn(3, ',').collect();
            if cols.len() == 3 {
                ins.execute(rusqlite::params![
                    cols[0].trim(),
                    cols[1].trim(),
                    cols[2].trim()
                ])
                .unwrap();
            }
        }
        let mut meta = tx
            .prepare("INSERT INTO meta (key, value) VALUES (?1, ?2)")
            .unwrap();
        meta.execute(["usb_ids_version", &version]).unwrap();
        meta.execute(["usb_ids_license", "BSD-3-Clause"]).unwrap();
        meta.execute(["xid_list", "none"]).unwrap();
    }
    tx.commit().unwrap();
    conn.execute_batch("VACUUM;").unwrap();
}

fn read_lines(p: &Path) -> Vec<String> {
    std::fs::read_to_string(p)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .skip(1) // header
        .map(str::to_string)
        .collect()
}

fn csv_rows(p: &Path) -> Vec<(String, String)> {
    read_lines(p)
        .into_iter()
        .filter_map(|l| {
            l.split_once(',')
                .map(|(a, b)| (a.trim().to_string(), b.trim().to_string()))
        })
        .collect()
}
