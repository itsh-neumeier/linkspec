# Manuelle Prüfliste vor dem Release

Je Plattform (Windows 11, macOS auf Apple Silicon, Linux mit UCSI) mit diesen
Geräten prüfen. Ergebnis abhaken. Weicht das Ergebnis von der Fixture ab:
Fixture anpassen oder Bug melden – die UI nicht weichzeichnen.

| Gerät | Erwartung |
|---|---|
| Thunderbolt-/USB4-Kabel mit E-Marker (40/80 Gbit/s) an TB-Dock | Überschrift „Thunderbolt / USB4“; Kabel passiv/aktiv, Tempo und Strom laut Marker; Link-Tempo; Urteil `data_ok` oder ein belegter Engpass |
| Billiges Ladekabel ohne E-Marker an Smartphone | „Dieses Kabel meldet keine Kennung“; „Langsames USB oder Nur-Lade-Kabel“, wenn keine Daten fließen |
| Hub/Dock mit USB 3 und DisplayPort | Transports USB 3 + DisplayPort; keine widersprüchliche Lane-Zahl; Geräte eingerückt |
| USB-C-Display | Display-Ansicht mit aktuellem und Höchstmodus; Erklärung, wenn unter Maximum |
| PD-Netzteil (65/100 W) | PDO-Liste, gewählter Vertrag markiert; Urteil zu Laden (Netzteil/Kabel/Rechner) |
| Störung: Kabel im Betrieb kurz wackeln | Banner „Die Verbindung ist abgerissen …“, Eintrag in der Kabel-Historie |
| Kabel erneut einstecken | Kabel-Historie erkennt das Kabel wieder (Anzahl Nutzungen + 1) |

## Checkliste je Plattform

### Windows 11
- [ ] Ohne Dienst: Leerzustand „Erweiterte Kabeldaten benötigen den LinkSpec-Dienst“
- [ ] Mit Dienst (MSI-Option): PDOs/E-Marker sichtbar (oder Abweichung dokumentiert)
- [ ] Tray-Flyout am Tray, Fenstermodus mit Taskleiste
- [ ] Toast bei Störung, Autostart über Run-Key
- [ ] `linkspec --json` validiert (`schemas/linkspec.diagnose.v1.json`)

### macOS (Apple Silicon)
- [ ] Menüleisten-Icon (Template), Popover, Fenster mit Dock-Icon
- [ ] Login-Item über SMAppService
- [ ] WidgetKit klein/mittel/groß zeigt den Snapshot
- [ ] Intel-Mac (falls verfügbar): CLI beendet mit Exit 3 und Hinweis

### Linux
- [ ] KDE/XFCE: Tray (StatusNotifierItem); GNOME: Fenster-Fallback
- [ ] Desktop-Benachrichtigung bei Störung
- [ ] XDG-Autostart
- [ ] udev-Regel (optional) installiert, falls Attribute Root brauchen

### Alle
- [ ] Deutsch und Englisch vollständig, Umschalten in den Einstellungen
- [ ] Screenreader liest Überschrift und Urteil (VoiceOver, Narrator, Orca)
- [ ] Tastaturbedienung im Fenster
- [ ] Leistungsziele (`docs/PERFORMANCE.md`)
