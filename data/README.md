# data/

Quellen und Lizenzen der gebündelten Daten. `build_vendors.rs` ist das
Build-Skript des Crates `linkspec-db`; es erzeugt beim Bauen `vendors.db`
(SQLite, read-only) und bettet sie ein.

| Datei | Inhalt |
|---|---|
| `usb.ids` | USB-ID-Liste (nur Vendor-Zeilen werden übernommen), BSD-3-Clause (`LICENSE.usb-ids`) |
| `xids.csv` | USB-IF-Zertifizierungsliste – absichtlich leer (Lizenz ungeklärt) |
| `cables.csv` | Community-Kabel-Fingerabdrücke (startet leer) |

Details: `docs/data-sources.md`, ADR 0007.
