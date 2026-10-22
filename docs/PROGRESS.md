# Fortschritt

**Aktuelle Phase:** 8 (Release-Vorbereitung) – Software fertig bis auf die
Maintainer-Aufgaben in `docs/MAINTAINER_TODO.md`.
**Letzter erledigter Schritt:** siehe Tabelle unten.
**Nächster Schritt:** Hardware-Prüfläufe nach `docs/manuell.md`, Captures als
Fixtures ablegen, Hypothesen in `docs/capability-matrix.md` bestätigen,
danach Version 1.0.0.

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

## Verlauf

Neueste Einträge oben. Format: Datum – Phase – Schritt – Commit.

- 2026-10-22 – 0–8 – Grundgerüst bis Paketierung auf dem Branch
  `claude/confident-babbage-i1gttl` (siehe `git log`).
