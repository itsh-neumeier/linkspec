# Fortschritt

**Aktuelle Phase:** 8 (Release-Vorbereitung) – Software fertig bis auf die
Maintainer-Aufgaben in `docs/MAINTAINER_TODO.md`.
**Letzter erledigter Schritt:** siehe Tabelle unten.
**Nächster Schritt:** Packaging-Korrektur auf
`fix/release-v0.9.1-packaging` prüfen und mergen (eigene Maintainer-Freigabe
erforderlich; die bisherige Freigabe bezog sich auf PR #4). Danach den
Release-Workflow manuell mit `tag=v0.9.1` wiederholen, ohne das Tag zu
verschieben. Anschließend Entwurf prüfen, Prüfsummen-PR und Übergabe.
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

Die Tags `phase-0` bis `phase-8` und `v0.9.1` wurden am 2026-10-23 mit
Maintainer-Freigabe gepusht. `v0.9.1` zeigt auf den Merge-Commit von PR #4:
`7e38d1d972ea90259f0e0f2141e9bc1875f94246`. Die Phasen-Zuordnung:

| Tag | Commit | Inhalt |
|---|---|---|
| `phase-0` | `3ce0e59` | Grundgerüst, Schema, Fixtures, Capture-Format |
| `phase-1`, `phase-2` | `1ff604f` | Core, CLI, Backends Linux/macOS |
| `phase-3` | `9f38a13` | Windows Stufe 1 und Dienst |
| `phase-4` … `phase-7` | `51d29ec` | App, Dashboard, Monitor, Historie, Widgets |
| `phase-8` | `d0e07de` | Abschlussstand des ursprünglichen Phasen-PRs #1: Paketierung, Doku, Leistung |

## CI-Stand

CI auf `main` bei `7e38d1d`: alle acht Jobs grün (Lauf `39939159272`).
Zuvor waren alle acht Jobs des Release-PRs #4 auf `9f2e161` grün
(Lauf `39937360036`).

Erster Release-Lauf `39939267276`: Linux x64/ARM64 und macOS erfolgreich,
Windows ARM64 beim Paketieren fehlgeschlagen (WiX v7 verlangt EULA-Annahme;
zusätzlich Selbst-Hashing der offenen Prüfsummendatei). Windows x64 dadurch
abgebrochen, Entwurfs-Job übersprungen. In Linux-Artefakten fehlt wegen des
Upload-Musters `dist/*.*` die Datei `SHA256SUMS-linux-*`.

## Verlauf

Neueste Einträge oben. Format: Datum – Phase – Schritt – Commit.

- 2026-10-23 – 8 – Release v0.9.1: PR #4 nach vollständig grüner CI und
  Maintainer-Freigabe gemergt, Commit
  `7e38d1d972ea90259f0e0f2141e9bc1875f94246`. Release- und Phasen-Tags
  gepusht, CI auf `main` grün. Status: Packaging-Korrektur für den ersten
  Release-Lauf vorbereitet; noch kein Entwurf und keine Release-Prüfsummen.
  WiX 4.0.6 festgelegt, unversionierte MSI-Dateien auf einzelne Komponenten
  verteilt, Windows-Selbst-Hashing und Linux-Upload korrigiert; lokale
  MSI-Smoke-Builds für x64/ARM64 mit Testdateien erfolgreich. Der erneute
  Lauf darf nur Packaging-/Doku-Abweichungen zum unveränderten Release-Tag
  enthalten und soll ausschließlich einen Entwurf erzeugen.
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
