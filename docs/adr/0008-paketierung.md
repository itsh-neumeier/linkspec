# ADR 0008 – Paketierung und Auslieferung

Status: angenommen (Phase 8)

## Entscheidung

| Plattform | Artefakte | Werkzeug |
|---|---|---|
| Windows | MSI (App, CLI, optional Dienst), portable ZIP (CLI), winget-Manifest, Scoop-Manifest | WiX Toolset v4 (`packaging/windows/linkspec.wxs`) |
| macOS | `.app` (arm64) mit eingebetteter CLI, Universal-CLI (`lipo`, Intel-Teil gibt nur die Meldung aus), `.dmg` und `.zip`, Homebrew-Cask und -Formula | `packaging/macos/build-app.sh` |
| Linux | `.deb`, `.rpm`, AppImage, Tarball, AUR-PKGBUILD, optionale udev-Regel | `cargo-deb`, `cargo-generate-rpm`, `appimagetool`, `packaging/linux/` |

- Release-Builds laufen im Workflow `.github/workflows/release.yml` bei Tags
  `v*`. Zu jedem Artefakt gibt es eine SHA256-Prüfsumme (`SHA256SUMS`).
- Reproduzierbarkeit: fester Toolchain-Stand (`rust-toolchain.toml`),
  `--locked`, `SOURCE_DATE_EPOCH` aus dem Commit, `strip = true`.
- Signierung (Developer ID, Notarisierung, Authenticode) ist vorbereitet, läuft
  aber nur, wenn die Secrets gesetzt sind. Ohne Secrets entstehen unsignierte
  Artefakte. Die Schritte stehen in `docs/MAINTAINER_TODO.md`.
- Paketmanager-Installationen legen die Datei `linkspec-packaged` neben das
  Binary; die App prüft dann nicht selbst auf Updates.
- Flatpak/Snap: `docs/spaeter.md` (die Sandbox schränkt sysfs ein).
