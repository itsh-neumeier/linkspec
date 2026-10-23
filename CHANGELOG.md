# Changelog

Format: je Version ein Abschnitt (Neu, Behoben, Doku, Intern). Gepflegt vom
Release-Agenten (`docs/release-agent.md`).

## [0.9.2] – 2026-10-23

### Neu
- Windows: Weitere USB-Geräte an USB-A-Buchsen und internen Ports als Baum
  anzeigen, einschließlich ihrer möglichen und ausgehandelten Geschwindigkeit.
- Programm-Icons in die Windows-EXEs einbetten; Tray-Symbol ohne Hintergrund
  passend zum hellen oder dunklen Windows-Design anzeigen.

### Behoben
- Windows: USB-3-Unterstützung pro Port statt pro Hub bestimmen und den
  Puffer für Port-Connector-Eigenschaften korrekt dimensionieren.
- NUL-Auffüllung in USB-Gerätenamen abschneiden und ANSI-Farben in der
  Windows-Konsole aktivieren.
- Windows-Paketierung mit WiX 4.0.6, getrennten MSI-Komponenten und korrekten
  SHA256-Werten; ZIP enthält jetzt CLI und App.
- Linux-Prüfsummen im Workflow-Artefakt mitliefern und alle Release-Pakete
  vor Erstellung des Entwurfs gegen die gemeinsamen Prüfsummen prüfen.

### Doku
- Capture des Windows-Hardwaretests als Fixture und offene
  Plattform-Fähigkeiten dokumentieren.

### Intern
- Windows-x64-Testbuild ohne Tag und Release-Entwurf ermöglichen.
- Workspace-Versionen und Paketvorlagen auf 0.9.2 aktualisieren.

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
