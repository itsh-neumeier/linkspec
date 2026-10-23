# Fortschritt

**Aktuelle Phase:** 8 (Release-Vorbereitung) – Software fertig bis auf die
Maintainer-Aufgaben in `docs/MAINTAINER_TODO.md`.
**Letzter erledigter Schritt:** siehe Tabelle unten.
**Nächster Schritt:** Windows-x64-Testbuild nach PR #9: Packaging-Fix auf
`fix/windows-x64-testbuild` prüfen und zum Merge freigeben; anschließend
den Testbuild auf `main` wiederholen. Ergebnisse und Prüfsummen des
Validierungslaufs werden im Übergabekommentar zu PR #9 dokumentiert.
Der Auftrag umfasst keinen Versionswechsel, Tag oder Release-Entwurf.
Vor 1.0.0: Hardware-Prüfläufe nach `docs/manuell.md`, Captures als Fixtures
ablegen und Hypothesen in `docs/capability-matrix.md` bestätigen.

## Phasen

| Phase | Inhalt | Status |
|---|---|---|
| 0 | Fundament: Workspace, AGENTS.md/CLAUDE.md, Doku-Stammdateien, CI-Matrix, Capture-Werkzeug je Plattform (`--capture`, Collector/Interpreter), synthetische Fixtures, Fähigkeitsmatrix, Schema v1, ADRs 0001–0009 | erledigt |
| 1 | Core + CLI: Modelle, PD-Decoder (R3.2), Urteils-Engine, Überschriften, Texte DE/EN (Fluent), `linkspec`, `--json`, `--watch`, `--raw`, Fake-Backend, Fixture-Suite (Kriterien 1–15) | erledigt |
| 2 | Backends macOS (IOKit, Hypothesen H-M1…H-M8) und Linux (sysfs, Netlink), Deep-Probe-Schalter, Event-Watcher | erledigt (Hardware-Prüfung offen) |
| 3 | Backend Windows: Stufe 1 (Hub-IOCTLs, Akku, Displays, Notifications), Stufe 2 (Dienst, ETW, Named Pipe), kein Treiber (ADR 0004) | erledigt (Hardware-Prüfung offen) |
| 4 | App: Tray/Menüleiste, Flyout, Portkarten, Banner, Fenster, Einstellungen, Autostart, Benachrichtigungen, Detailansicht, Sprachen, Barrierefreiheit | erledigt |
| 5 | Thunderbolt/USB4 (Topologie, Lanes, Generation) und Display-Diagnose (Modus vs. EDID vs. Link) | erledigt (DSC/Adapter: `spaeter.md`) |
| 6 | Power Monitor (Graph, `--monitor`), Negotiation, Port-Gesundheit, Kabel-Historie | erledigt (Kabelwiderstand: `spaeter.md`) |
| 7 | TUI-Dashboard, macOS-Widget (Swift), `--report`, `--capture`, Issue-Template „Kabel melden“ | erledigt (Windows-Widget: `spaeter.md`) |
| 8 | Paketierung aller Plattformen, Release-Workflow, Signierschritte, Update-Hinweis, README, Datenschutz, Grenzen, `manuell.md` | erledigt bis auf Maintainer-Aufgaben |

## Tags

Die Tags sind lokal angelegt; das Pushen von Tags ist in der Arbeitssitzung
nicht erlaubt (nur der Arbeits-Branch). Der Maintainer setzt sie nach dem
Merge (`docs/MAINTAINER_TODO.md`, Abschnitt 1):

| Tag | Commit | Inhalt |
|---|---|---|
| `phase-0` | `3ce0e59` | Grundgerüst, Schema, Fixtures, Capture-Format |
| `phase-1`, `phase-2` | `1ff604f` | Core, CLI, Backends Linux/macOS |
| `phase-3` | `9f38a13` | Windows Stufe 1 und Dienst |
| `phase-4` … `phase-7` | `51d29ec` | App, Dashboard, Monitor, Historie, Widgets |
| `phase-8` | letzter Commit des Branches | Paketierung, Doku, Leistung |

## CI-Stand

Letzter Lauf: `fmt`, `test (ubuntu-latest)`, `test (windows-latest)`,
`test (macos-latest)`, `deny`, Cross-Builds (Windows ARM64, Linux aarch64,
macOS Intel-CLI mit Exit 3) grün. Lokal getestet: `.deb`, `.rpm` und Tarball
bauen (`packaging/linux/build-packages.sh`); AppImage, MSI und `.dmg` laufen
erst im Release-Workflow (noch nie ausgeführt).

## Verlauf

Neueste Einträge oben. Format: Datum – Phase – Schritt – Commit.

- 2026-10-23 – 8 – PR #9 nach acht grünen CI-Jobs auf `fd61693`
  (Lauf `39951331314`) mit Maintainer-Freigabe per Merge-Commit
  `af8c2c896d6f14c1b68984ede9b0e53bd33f3483` integriert. Auch die acht
  CI-Jobs auf `main` grün (Lauf `39952208185`). Windows-x64-Testbuild
  `39952225147` auf diesem Merge-Commit: Anwendung erfolgreich gebaut,
  Paketierung fehlgeschlagen (`WIX7015` durch ungepinntes WiX 7;
  anschließend Selbst-Hashing der offenen Prüfsummendatei). Packaging-Fix
  auf `fix/windows-x64-testbuild`: WiX 4.0.6, getrennte MSI-Komponenten
  für unversionierte Dateien, explizite Fehlerprüfung, App im ZIP,
  ausschließlich MSI/ZIP hashen und Pakete/Prüfsummen hochladen. Lokale
  Prüfung: actionlint, PowerShell-Parser, tatsächlicher Prüfsummen-Code
  mit vorhandener Ausgabedatei und x64-MSI-Smoke-Build mit Testdateien
  grün; keine Installation ausgeführt. Status: Merge des Packaging-Fixes
  benötigt eigene Freigabe. Kein Versionswechsel, Tag, CHANGELOG-Eintrag,
  Release-Entwurf oder Paketquellen-Einreichung für diesen Testbuild.
- 2026-10-23 – 8 – Release v0.9.1 fortgesetzt: `main` bei
  `338bfc7a2b4dbca895e11224d1522cb2ee92c3e5` (PR #6) per Merge in
  `release/v0.9.1` übernommen. `clipboard-win 5.4.3` ersetzt die
  zurückgezogene Version 5.4.1; Issue #5 damit behoben. Status:
  vollständige CI von PR #4 ausstehend; Merge bei grüner CI freigegeben.
- 2026-10-23 – 8 – Release v0.9.1, PR #4, Commit
  `e2f64e52d673c28ec75599982dbc1b4147ec3607`: PR-CI meldet im Job `deny`
  `error[yanked]` für `clipboard-win 5.4.1` in `Cargo.lock`
  (Lauf `39927926652`, Job `120328788465`). Issue #5 mit Fehlerlog
  angelegt. Status: Release-Ablauf gestoppt; Abhängigkeits-Fix liegt
  außerhalb des Release-Agent-Auftrags. Kein Merge, Tag oder
  Release-Workflow gestartet.
- 2026-10-23 – 8 – Release v0.9.1 vorbereitet auf `release/v0.9.1`:
  Versionen, Changelog und Paket-URLs aktualisiert. Vorprüfung auf
  `main` bei `e8dbe151b88194a5a2fe596f79d59b3359abd2e7` mit Rust 1.99.1
  (Windows MSVC): Formatierung, Clippy, 73 Tests (0 ignoriert) und
  `cargo deny check` grün. Deny meldet fünf nicht benötigte
  Lizenzfreigaben; beim Testbuild ein nicht fataler Hinweis zum
  inkrementellen Cache auf dem Netzlaufwerk. Status: Release-PR/CI und
  Merge-Freigabe ausstehend; Release-Entwurf und Prüfsummen folgen nach
  dem Merge.
- 2026-10-23 – 8 – CI: Zeitlimits je Job und apt-Schritt, apt mit Wiederholungen,
  GTK/AppIndicator/xdo aus den Abhängigkeiten entfernt (Tray über ksni).
  Anlass: `test (ubuntu-latest)` hing 6 h in `apt-get`.
- 2026-10-22 – 8 – Software-Renderer, ereignisgesteuerter Leerlauf,
  Leistung gemessen (`docs/PERFORMANCE.md`), Linux-Pakete lokal gebaut.
- 2026-10-22 – 4 – App lauffähig unter X11 (Screenshots in
  `docs/screenshots/`).
- 2026-10-22 – 0–3 – Grundgerüst, Core, CLI, alle Backends, Dienst, CI.
