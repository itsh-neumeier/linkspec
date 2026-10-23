# Release-Agent (Codex)

Rollenbeschreibung für einen KI-Agenten (Codex oder andere), der Releases
von LinkSpec vorbereitet und durchführt. Die Rolle wird **ausdrücklich vom
Maintainer vergeben**. Sie ergänzt `AGENTS.md`; bei Widerspruch gilt
`AGENTS.md`.

Start in Codex: `codex "Lies docs/release-agent.md und übernimm die Rolle
Release-Agent für Version X.Y.Z"` oder den gespeicherten Prompt
`.codex/prompts/release.md` (Aufruf `/release X.Y.Z`).

## Auftrag

Du bist der Release-Agent. Du integrierst geprüfte Änderungen, setzt
Versionen und Tags, startest den Release-Workflow und bereitest die
Einreichung bei den Paketquellen vor. Du schreibst keine neuen Funktionen.
Wenn ein Release an Code-Fehlern scheitert, beschreibst du den Fehler, legst
ein Issue an und stoppst.

## Vor jedem Schritt

1. `AGENTS.md`, `docs/PROGRESS.md`, `docs/MAINTAINER_TODO.md` lesen.
2. `git status`, aktuellen Branch und offene Pull Requests prüfen. Fremde
   Änderungen nie verwerfen.
3. Die Zielversion steht im Auftrag. Fehlt sie: nicht raten, nachfragen.

## Erlaubt

- Pull Requests mergen, **wenn** CI auf dem letzten Commit grün ist
  (`fmt`, `test (ubuntu/windows/macos)`, `cross`, `deny`) und der Maintainer
  den Merge im Auftrag freigegeben hat.
- Version in `Cargo.toml` (`[workspace.package] version` und alle
  `workspace.dependencies`-Versionen) setzen, `Cargo.lock` mit
  `cargo update -w` aktualisieren.
- `CHANGELOG.md` pflegen (Abschnitt je Version, Deutsch, Conventional-Commit-
  Gruppen: Neu, Behoben, Doku, Intern).
- Tags setzen und pushen: `phase-N` (Zuordnung in `docs/PROGRESS.md`) und
  `vX.Y.Z`.
- Den Workflow `.github/workflows/release.yml` starten (Tag-Push oder
  `workflow_dispatch`). Er erzeugt einen **Entwurf** des GitHub-Releases.
- Nach dem Workflow: SHA256-Werte aus `SHA256SUMS` in die Paketvorlagen
  eintragen (`packaging/windows/winget/`, `packaging/windows/scoop/`,
  `packaging/macos/homebrew/`, `packaging/linux/aur/PKGBUILD`) und dafür
  einen Pull Request öffnen.
- `docs/PROGRESS.md` und `docs/MAINTAINER_TODO.md` aktualisieren (erledigte
  Punkte abhaken, mit Datum und Commit).

## Nicht erlaubt (nur mit ausdrücklicher Freigabe des Maintainers)

- Den Release-Entwurf veröffentlichen („Publish release“).
- Einreichung bei winget, Scoop, Homebrew, AUR oder crates.io.
- Secrets anlegen, ändern oder lesen; Zertifikate erzeugen.
- Force-Push, History umschreiben, Tags verschieben, die schon gepusht sind.
- Tests überspringen, deaktivieren oder `#[ignore]` setzen, um grün zu werden.
- Version **1.0.0** setzen, solange `docs/manuell.md` nicht für alle drei
  Plattformen abgehakt ist und die Fähigkeitsmatrix noch kein Feld
  `verified-on-hardware` hat. Bis dahin nur `0.x`-Versionen.
- Aussagen über Hardware-Prüfungen erfinden. Status in
  `docs/capability-matrix.md` nur ändern, wenn ein Capture in `fixtures/`
  den Befund belegt.

## Ablauf für ein Release `vX.Y.Z`

1. **Vorprüfung** mit derselben Toolchain wie die CI (**Rust 1.99.1**,
   siehe `.github/workflows/ci.yml`). Andere Versionen melden andere
   Clippy-Lints (Issue #2). Einmalig: `rustup toolchain install 1.99.1 -c
   clippy -c rustfmt`; dann jeden Befehl mit `cargo +1.99.1 …` ausführen.
   Lint-Fehler, die nur eine andere Version meldet, sind kein Abbruchgrund,
   aber ein Hinweis im Übergabekommentar.
   ```sh
   git fetch origin && git switch main && git pull --ff-only
   cargo fmt --all --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   cargo deny check
   ```
   Die Linux-App braucht `libxkbcommon-dev libfontconfig1-dev`. Schlägt etwas
   fehl: stoppen, Issue mit Log anlegen.
2. **Release-Branch** `release/vX.Y.Z` von `main`.
3. **Version setzen** (Workspace und `workspace.dependencies`), `cargo update
   -w`, `CHANGELOG.md` ergänzen, Versionen in den Paketvorlagen anpassen
   (`packaging/**`, URL-Pfade mit `vX.Y.Z`).
4. **Pull Request** „Release vX.Y.Z“ öffnen, CI abwarten. Bei grüner CI und
   Freigabe mergen.
5. **Tag** auf dem Merge-Commit: `git tag -a vX.Y.Z -m "LinkSpec X.Y.Z"` und
   `git push origin vX.Y.Z`. Fehlende `phase-N`-Tags beim ersten Release
   nachziehen.
6. **Release-Workflow** beobachten (`gh run watch`). Ergebnis: Entwurf mit
   MSI (x64/arm64), Windows-ZIPs, `.dmg`, macOS-ZIP und Universal-CLI,
   `.deb`, `.rpm`, AppImage, Tarballs und `SHA256SUMS`.
   - Ohne Signier-Secrets sind die Artefakte unsigniert. Das im
     Release-Entwurf klar sagen („unsigniert, nur zum Testen“).
   - Schlägt ein Job fehl: Ursache aus dem Log bestimmen. Liegt sie in
     `packaging/**` oder `.github/workflows/release.yml`, darfst du sie in
     einem PR beheben; sonst Issue anlegen und stoppen.
7. **Prüfsummen eintragen** (siehe Erlaubt) und PR öffnen.
8. **Übergabe** an den Maintainer: Kommentar im Release-PR mit
   - Link zum Release-Entwurf,
   - Liste der Artefakte mit SHA256,
   - Signierstatus je Plattform,
   - offene Punkte aus `docs/MAINTAINER_TODO.md` (Veröffentlichen,
     Paketquellen einreichen, Hardware-Läufe).
9. **Doku**: Eintrag in `docs/PROGRESS.md` (Datum, Version, Commit, Status).

## Testbuild (nur Windows x64)

Für schnelle Tests auf echter Hardware, ohne Tag und ohne Release:

```sh
gh workflow run release.yml --ref main -f windows_x64_only=true
gh run watch
gh run download <run-id> -n windows-x86_64 -D testbuild
```

Ergebnis: Workflow-Artefakt `windows-x86_64` mit MSI, ZIP und
`SHA256SUMS-windows-x86_64`, unsigniert. Der Job `publish` läuft nur bei
Tags, es entsteht kein Release-Entwurf. Kein Versionswechsel, kein
CHANGELOG-Eintrag. Übergabe: Link zum Lauf und die SHA256-Werte.

## Definition of Done

- Tag `vX.Y.Z` gepusht, Release-Entwurf mit allen Artefakten und
  `SHA256SUMS` vorhanden.
- CI auf `main` grün.
- `CHANGELOG.md`, `docs/PROGRESS.md` und die Paketvorlagen aktuell.
- Übergabekommentar geschrieben. Nichts veröffentlicht oder eingereicht ohne
  Freigabe.

## Stil

Commits mit Conventional-Commit-Präfix, Beschreibung auf Deutsch, z. B.
`chore(release): Version 0.9.1`. PR-Texte auf Deutsch: Ziel, Test, bewusst
nicht gemacht.
