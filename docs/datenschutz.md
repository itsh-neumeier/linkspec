# Datenschutz

## Zweck

LinkSpec erklärt USB-C-, Thunderbolt- und USB4-Verbindungen auf dem eigenen
Gerät. Dafür liest es Daten der Port-Controller, angeschlossener Geräte und
der Kabel-E-Marker.

## Was LinkSpec speichert (nur lokal)

| Datei | Inhalt | Ort |
|---|---|---|
| `history.db` | Kabel-Fingerabdruck (Hash aus VID, PID, VDOs), Name (falls vergeben), Zeitpunkte, ausgehandelte Werte, Störungen | Windows `%LOCALAPPDATA%\LinkSpec`, macOS `~/Library/Application Support/LinkSpec`, Linux `$XDG_DATA_HOME/linkspec` |
| `settings.json` | Einstellungen | wie oben |
| `widget.json` | Überschriften und Sätze der Portkarten für Widgets | wie oben (macOS zusätzlich App-Group-Container) |
| `logs/` | Protokoll, Stufe `info`, tägliche Rotation, höchstens 5 Dateien | wie oben |

Verzeichnis und Historie sind nur für den Benutzer lesbar (Unix: 0700/0600;
Windows: Benutzerprofil). Seriennummern werden nie gelesen oder gespeichert.

## Was das Gerät nie verlässt

Alles oben. LinkSpec hat **keine Telemetrie, keine Analytics, keinen
Crashreporter und keine Netzwerk-Lookups** für Hersteller oder XIDs.

## Der einzige automatische Netzzugriff

Die App fragt höchstens alle 6 Stunden die neueste Release-Version bei
`api.github.com` ab. Gesendet wird nur die Anfrage mit dem User-Agent
`LinkSpec/<Version>`, **keine Hardwaredaten**. Abschaltbar in den
Einstellungen. Installationen über Paketmanager prüfen nicht selbst. Die CLI
hat überhaupt keinen Netzwerkcode (Test `no_network_dependencies`).

## Kabel melden und Captures

- `linkspec --report` und „Kabel melden“ öffnen ein vorausgefülltes GitHub-
  Issue im Browser. Gesendet wird erst, wenn Sie es selbst absenden. Inhalt:
  VID, PID, Typ, Tempo, Strom, XID, Fingerabdruck, VDOs, Betriebssystem und
  LinkSpec-Version.
- `linkspec --capture` schreibt eine Datei auf Ihr Gerät. Sie enthält keine
  Seriennummern, Benutzer- oder Hostnamen, Dateipfade oder Zwischenablage
  (Negativtests: `capture_is_anonymized`,
  `fixtures_and_reports_have_no_personal_data`).

## Abgleich mit dem Code

| Aussage | Nachweis |
|---|---|
| Keine Seriennummern im Capture | `crates/linkspec-platform/src/anonymize.rs`, Linux-Collector `FORBIDDEN`, Test `ucsi_laptop_tree` |
| Keine Netzwerk-Crates in der CLI | Test `no_network_dependencies` |
| Update-Prüfung abschaltbar, höchstens alle 6 h | `app/src/platform.rs` (`check_update`, `MIN_INTERVAL_SECS`) |
| Dateirechte nur Benutzer | Test `file_is_user_only` |
