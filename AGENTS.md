# AGENTS.md – LinkSpec

Regeln für alle KI-Assistenten (Claude Code, Codex, andere) und Mitwirkende.
`CLAUDE.md` verweist hierher. Fortschritt steht nur in Dateien, nie in
Chatverläufen.

## Start einer Sitzung

1. `docs/PROGRESS.md` lesen und dort fortsetzen (aktuelle Phase, letzter
   Schritt, nächster Schritt).
2. `git status` und aktuellen Branch prüfen. Fremde Änderungen erhalten.
3. Bei Architekturfragen zuerst `docs/adr/` lesen. Eine Änderung gegen eine
   ADR braucht eine neue ADR, die die alte ablöst.

## Was LinkSpec ist

Plattformübergreifendes Diagnosewerkzeug für USB-C, Thunderbolt und USB4
(Windows 11, Linux, macOS auf Apple Silicon). Es sagt pro Port, was anliegt
und welches Glied begrenzt: Port, Kabel, Ladegerät oder Endgerät.
**Kein Kabeltester**: Software sieht nur, was Controller und E-Marker melden.

## Struktur

```
crates/linkspec-core           Modelle, PD-Decoder, Urteils-Engine, Schema-Mapping, i18n. Keine OS-APIs.
crates/linkspec-platform       Trait PortSource, Events, Anonymisierung
crates/linkspec-engine         Backend-Auswahl, Pipeline, Historie, Einstellungen (CLI und App)
crates/linkspec-backend-macos  IOKit + CoreGraphics. Einzige Stelle mit IOKit.
crates/linkspec-backend-linux  sysfs + udev-Netlink
crates/linkspec-backend-windows Stufe 1 (Hub-IOCTLs, Akku, Displays) + Dienst-Client
crates/linkspec-service        Windows-Dienst (Stufe 2, ETW, Named Pipe)
crates/linkspec-db             vendors.db (gebündelt, read-only)
crates/linkspec-history        Kabel-Historie (SQLite im Benutzerprofil)
crates/linkspec-cli            Binary `linkspec` + TUI
crates/linkspec-fake           Fake-Backend für Fixtures
app/                           Slint-App (Tray/Menüleiste, Flyout, Fenster)
widgets/                       macOS WidgetKit (Swift, nur Anzeige)
schemas/  fixtures/  data/  locales/  packaging/  docs/
```

Jedes Backend trennt **Collector** (OS-spezifisch, liefert einen Dump) und
**Interpreter** (plattformunabhängig, Dump → `Snapshot`). Captures sind
Dumps. Fixtures laufen so auf allen CI-Runnern durch den echten Interpreter.

## Befehle

Toolchain wie in der CI: Rust 1.99.1 (`rustup toolchain install 1.99.1`,
`cargo +1.99.1 …`). Ältere oder neuere Versionen melden andere Clippy-Lints.

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p linkspec-cli -- --json                 # echte Hardware
cargo run -p linkspec-cli -- --fake fixtures/core/c10_dock_usb3_and_dp.json
cargo run -p linkspec-app -- --window --fake fixtures/core/c03_cable_limits_power.json
LINKSPEC_UPDATE_FIXTURES=1 cargo test -p linkspec-backend-linux   # Fixtures neu schreiben
INSTA_UPDATE=always cargo test -p linkspec-core --test fixtures    # Text-Snapshots neu
```

Linux braucht für die App: `libxkbcommon-dev libfontconfig1-dev` (Tray über
StatusNotifierItem/D-Bus, kein GTK nötig). Cross-Checks ohne Hardware:
`cargo clippy -p linkspec-backend-windows --target x86_64-pc-windows-msvc`,
`cargo clippy -p linkspec-backend-macos --target aarch64-apple-darwin`.

## Konventionen

- **Rust** stable, Edition 2024. `unsafe` nur in Plattform-FFI, jede Stelle
  mit `// SAFETY:`. `linkspec-core`, `-platform`, `-engine`, `-db`,
  `-history`, `-fake` haben `#![forbid(unsafe_code)]`.
- **Clippy-Warnungen sind Fehler.** Bibliotheken: `thiserror`, Binaries:
  `anyhow` oder eigene Fehlertexte.
- **Nicht raten**: APIs, IOCTLs, sysfs-Attribute, IOKit-Properties,
  ETW-Provider und Spec-Felder mit Quelle belegen (Kommentar oder ADR).
  Unbelegtes als Hypothese (H-M…, H-W…, H-L…) markieren und in
  `docs/capability-matrix.md` führen.
- **Core-Verhalten nur mit Fixture ändern.** Jeder Urteils-`code` hat eine
  Fixture in `fixtures/core/` (Test `every_verdict_code_has_a_fixture`).
- **Unbekannte Bits ignorieren, nicht failen.** Decoder auf USB PD R3.2 und
  UCSI 2.x/3.0, ältere Versionen tolerieren.
- **Texte** nur in `locales/de.ftl` und `locales/en.ftl` (Fluent). Deutsch ist
  Referenzsprache. Beide Dateien haben dieselben IDs (Test).
- **JSON-Schema** `schemas/linkspec.diagnose.v1.json`: nur additive
  Änderungen, dann Minor-Version erhöhen. CLI-Ausgabe wird in Tests validiert.
- **Datenschutz**: keine Seriennummern, Benutzer-/Hostnamen, Pfade in
  Captures, Reports oder Logs (Negativtests in `linkspec-cli/tests`).
  Kein Netzwerk außer der abschaltbaren Update-Prüfung der App.
- **Commits**: Conventional-Commit-Präfix, Beschreibung auf Deutsch.
- **Clean Room**: kein Code, keine Texte, Daten oder Fixtures von WhatCable
  oder anderen Ports. Nur öffentliche Spezifikationen, offizielle
  Plattform-Doku und eigene Messungen. Den Namen „WhatCable“ nicht im
  Produkt verwenden.
- **Sprachen**: Code, JSON-Keys, Logs Englisch; UI Deutsch/Englisch; Doku
  Deutsch; README zweisprachig.

## Rollen

- **Release-Agent**: wird vom Maintainer ausdrücklich vergeben. Regeln und
  Ablauf in [`docs/release-agent.md`](docs/release-agent.md); in Codex
  über den Prompt `.codex/prompts/release.md` (`/release X.Y.Z`).

## Phasen-Abschluss

`cargo fmt --check`, Clippy und alle Tests grün auf allen drei CI-Runnern,
Doku aktuell, Eintrag in `docs/PROGRESS.md`.
