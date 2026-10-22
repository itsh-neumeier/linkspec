# ADR 0007 – Vendor-Datenquelle und Lizenz

Status: angenommen (Phase 0)

## Entscheidung

- Vendor-Namen kommen aus `usb.ids` (<http://www.linux-usb.org/usb.ids>).
  Laut <http://www.linux-usb.org/usb-ids.html> darf die Datei unter GPL-2.0+
  **oder** der 3-Klausel-BSD-Lizenz verteilt werden. LinkSpec nutzt die
  **BSD-3-Clause**-Option (MIT-kompatibel). Lizenztext: `data/LICENSE.usb-ids`.
- Das Build-Skript `data/build_vendors.rs` (Build-Skript des Crates
  `linkspec-db`) erzeugt `vendors.db` (SQLite, read-only) und bettet sie in
  das Binary ein. Nur Hersteller-Zeilen werden übernommen.
- **USB-IF-XID-Liste**: nicht gebündelt. Eine Bezugsquelle mit einer Lizenz,
  die das Bündeln erlaubt, ist nicht bekannt. Die Tabelle `xids` bleibt leer;
  `certification` ist daher `unlisted`, wenn ein XID gemeldet wird. Der Weg zur
  Liste steht in `docs/MAINTAINER_TODO.md`.
- **Kabel-Fingerabdrücke**: eigene Liste `data/cables.csv`, startet leer, wird
  aus Meldungen über das Issue-Template „Kabel melden“ gefüllt.
- Keine Netzwerk-Lookups.
