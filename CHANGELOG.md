# Changelog

Format: je Version ein Abschnitt (Neu, Behoben, Doku, Intern). Gepflegt vom
Release-Agenten (`docs/release-agent.md`).

## [0.9.1] – 2026-10-23

### Neu
- Erste vollständige Fassung (Phasen 0–8): Core mit USB-PD-R3.2-Decodern und
  Urteils-Engine, CLI `linkspec`, Backends für Linux, macOS und Windows
  (Stufe 1), optionaler Windows-Dienst (Stufe 2), Slint-App mit Tray,
  Kabel-Historie, Paketierung und Release-Workflow.

### Behoben
- Clippy-Fehler in der Erkennung der Tray-Verfügbarkeit behoben (Issue #2,
  PR #3).
- Zurückgezogene transitive Abhängigkeit `clipboard-win 5.4.1` durch
  Version 5.4.3 ersetzt (Issue #5, PR #6).

### Doku
- AGENTS.md, Fähigkeitsmatrix, ADRs 0001–0009, Datenschutz, Grenzen,
  manuelle Prüfliste, Leistung, Release-Agent.
- Rust 1.99.1 als gemeinsame Toolchain für CI und Release-Vorprüfung
  dokumentiert.

### Intern
- CI- und Release-Jobs sowie apt-Schritte mit Zeitlimits versehen;
  Linux-Build-Abhängigkeiten auf die benötigten Pakete reduziert.
- Workspace-Versionen und Paketvorlagen auf 0.9.1 aktualisiert.
