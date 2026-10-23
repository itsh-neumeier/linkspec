# Maintainer-Aufgaben

Was Konten, Geld, Zertifikate, Rechtsprüfung oder echte Hardware braucht.
Alles ist vorbereitet; hier stehen die genauen Schritte.

## 1. Branch, Pull Request, Tags

- [x] Pull Request vom Branch `claude/confident-babbage-i1gttl` nach `main`
      prüfen und mergen (alle Phasen, Commits je Phase): PR #1 am
      2026-10-22, Commit `a55cd5958825fc6aa05a090afbde9395d84cac3d`.
      Nachtrag PR #3 am 2026-10-23, Commit
      `e8dbe151b88194a5a2fe596f79d59b3359abd2e7`.
- [x] Release-PR #4 am 2026-10-23 nach grüner CI gemergt, Commit
      `7e38d1d972ea90259f0e0f2141e9bc1875f94246`.
- [x] Tags `phase-0` … `phase-8` und `v0.9.1` am 2026-10-23 gepusht
      (Zuordnung in `docs/PROGRESS.md`); Release-Tag auf Commit `7e38d1d`.
- [ ] Packaging-Korrektur aus PR #7 für v0.9.1 zum Merge freigeben.
      Prüfung durch manuellen Workflow auf dem Fix-Branch mit
      `tag=v0.9.1` erfolgreich (Lauf `39942631672`, Commit `cb15598`);
      das vorhandene Tag bleibt unverändert.
- [ ] Darauf aufbauenden Prüfsummen-PR für v0.9.1 prüfen und freigeben.
- [ ] Vorbereiteten Release-Entwurf v0.9.1 nach eigener Prüfung und
      ausdrücklicher Freigabe veröffentlichen (Release-ID `421745958`).

## 2. Hardware-Prüfläufe (vor 1.0)

Für jede Plattform die Checkliste in `docs/manuell.md` abarbeiten. Dabei:

- [ ] `linkspec --capture` mit Dock, Netzteil, Thunderbolt-Kabel,
      Billig-Ladekabel und USB-C-Monitor ausführen und die Dateien unter
      `fixtures/{linux,macos,windows}/` ablegen (Feld `"synthetic": false`).
      Vorher prüfen: keine Seriennummer, kein Name (Test
      `fixtures_and_reports_have_no_personal_data` läuft automatisch).
- [ ] Hypothesen H-L1…H-L4, H-M1…H-M8, H-W2…H-W6 in
      `docs/capability-matrix.md` bestätigen oder korrigieren; Status auf
      `verified-on-hardware` setzen.
- [ ] macOS: Abweichende Klassen-/Property-Namen in
      `crates/linkspec-backend-macos/src/registry.rs` anpassen; Fixture
      ersetzen.
- [ ] Windows: `linkspec-service providers` und `linkspec-service console`
      (Administrator) auf einem UCSI-Laptop ausführen. Eine Anfrage
      `{"request":"capture"}` an `\\.\pipe\linkspec-service` liefert die
      Rohereignisse. Danach die Namensmuster in
      `crates/linkspec-service/src/etwmap.rs` anpassen und über ADR 0004
      (Treiber ja/nein) neu entscheiden.
- [ ] Windows: Prüfen, ob die Systemmeldung „PC lädt langsam“ (UcmCx) als
      Ereignis abgreifbar ist (Event Viewer → Microsoft-Windows-USB-UCMCX).
- [ ] Leistungsziele auf echter Hardware messen (`docs/PERFORMANCE.md`).
- [ ] Danach Version auf 1.0.0 setzen (`Cargo.toml` `[workspace.package]`),
      `CHANGELOG` ergänzen, Tag `v1.0.0`.

## 3. Signierung

### macOS (Apple Developer Program, 99 USD/Jahr)

1. Zertifikat „Developer ID Application“ erstellen, als `.p12` exportieren.
2. Repository-Secrets setzen: `MACOS_CERT_P12` (Base64), `MACOS_CERT_PASSWORD`,
   `MACOS_SIGN_IDENTITY` (z. B. `Developer ID Application: Name (TEAMID)`),
   `APPLE_ID`, `APPLE_TEAM_ID`, `APPLE_APP_PASSWORD` (app-spezifisches
   Passwort).
3. Der Release-Workflow signiert dann mit Hardened Runtime
   (`packaging/macos/entitlements.plist`) und notarisiert:
   ```sh
   codesign --force --options runtime --timestamp \
     --entitlements packaging/macos/entitlements.plist \
     --sign "$MACOS_SIGN_IDENTITY" dist/LinkSpec.app
   xcrun notarytool submit dist/LinkSpec.dmg --apple-id "$APPLE_ID" \
     --team-id "$APPLE_TEAM_ID" --password "$APPLE_APP_PASSWORD" --wait
   xcrun stapler staple dist/LinkSpec.dmg
   ```
4. WidgetKit-Extension: App Group `group.io.github.itsh-neumeier.linkspec`
   im Developer-Portal anlegen, Bundle-IDs
   `io.github.itsh-neumeier.linkspec` und `…linkspec.widget` registrieren,
   dann `widgets/macos/` mit Xcode bauen (Anleitung in
   `widgets/macos/README.md`).

### Windows (Code-Signing-Zertifikat, OV/EV oder Azure Trusted Signing)

1. Zertifikat beschaffen (z. B. Azure Trusted Signing, ca. 10 USD/Monat).
2. Secrets: `WINDOWS_CERT_PFX` (Base64), `WINDOWS_CERT_PASSWORD` **oder** die
   Azure-Variablen (`AZURE_TENANT_ID`, `AZURE_CLIENT_ID`,
   `AZURE_CLIENT_SECRET`, `TRUSTED_SIGNING_ACCOUNT`, `TRUSTED_SIGNING_PROFILE`).
3. Signiert werden `linkspec.exe`, `linkspec-app.exe`, `linkspec-service.exe`
   und das MSI:
   ```powershell
   signtool sign /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 /f cert.pfx /p $env:WINDOWS_CERT_PASSWORD dist\*.exe dist\LinkSpec.msi
   ```

### Treiber

Entfällt in v1 (ADR 0004).

## 4. Paketquellen

- [x] SHA256-Werte für v0.9.1 am 2026-10-23 aus `SHA256SUMS` in alle
      vier Paketvorlagen übernommen (Packaging-Build `cb15598`, Tag
      `v0.9.1` auf `7e38d1d`); 15 Paket-Hashes lokal verifiziert.
      Dies bereitet die folgenden Einreichungen nur vor.
- [ ] **winget**: `packaging/windows/winget/` mit URL/SHA256 des Releases
      füllen, Pull Request an `microsoft/winget-pkgs`
      (`wingetcreate submit`).
- [ ] **Scoop**: eigenen Bucket anlegen (`itsh-neumeier/scoop-linkspec`) und
      `packaging/windows/scoop/linkspec.json` mit Hash eintragen.
- [ ] **Homebrew**: Tap `itsh-neumeier/homebrew-linkspec` anlegen; Cask
      `packaging/macos/homebrew/linkspec.rb` (App) und Formula
      `linkspec-cli.rb` mit SHA256 füllen.
- [ ] **AUR**: `packaging/linux/aur/PKGBUILD` mit `sha256sums` füllen und als
      `linkspec-bin` hochladen.
- [ ] **crates.io**: Crates in Abhängigkeitsreihenfolge veröffentlichen
      (`linkspec-core`, `-platform`, `-db`, `-history`, `-fake`, Backends,
      `-engine`, `-cli`). Vorher `data/` und `locales/` in die Crates
      kopieren bzw. per `include` aufnehmen, weil `cargo package` keine Dateien
      außerhalb des Crate-Verzeichnisses einpackt.

## 5. Rechtliches

- [ ] **Slint-Lizenz**: Bedingungen der „Royalty-free Desktop, Mobile and
      Web Applications License v2.0“ prüfen (Namensnennung über
      `AboutSlint` im Über-Dialog ist eingebaut).
- [ ] **USB-IF-XID-Liste**: Bei USB-IF anfragen, ob die Liste zertifizierter
      Produkte gebündelt werden darf. Bei Zusage `data/xids.csv` füllen und
      `docs/data-sources.md` anpassen.
- [ ] **Marken**: „USB4“, „Thunderbolt“, „USB-C“, „DisplayPort“ sind Marken.
      LinkSpec nutzt sie nur beschreibend. Prüfen, ob Logo-Richtlinien eine
      Anpassung der Texte verlangen. Das README nennt WhatCable sachlich als
      Inspiration; prüfen, ob das so bleiben soll.
- [ ] Name „LinkSpec“ auf Kollisionen prüfen (Marken, Paketnamen).

## 6. Repository-Einstellungen

- [ ] Branch-Schutz für `main` mit den CI-Jobs `fmt`, `test (*)`, `cross`,
      `deny`.
- [ ] Issue-Label `kabel` anlegen (für das Formular „Kabel melden“).
