# Später (nicht in v1)

| Thema | Grund für die Verschiebung |
|---|---|
| Andere Link-Typen (Netzwerkkabel, HDMI, …) | v1 bleibt bei USB-C/Thunderbolt/USB4. |
| Windows-Widget-Board-Provider | Braucht MSIX-Paketidentität und signierten COM-Server (ADR 0005). Snapshot `widget.json` ist vorhanden. |
| GNOME-/KDE-Widgets | Nicht in v1 vorgesehen. |
| Flatpak, Snap | Sandbox schränkt sysfs ein; braucht Ausnahmen (`--device=all`, `--filesystem=/sys`). |
| Windows Energy Meter Interface (EMI) im Power Monitor | EMI-Kanäle sind gerätespezifisch und ohne Hardware nicht prüfbar. Der Power Monitor nutzt unter Windows die Akku-Rate. |
| macOS SMC-Leistungsdaten | ADR 0006. |
| Treiber-Companion Windows (Stufe 3) | ADR 0004. |
| Kabelwiderstand | Keine Plattform liefert Spannung vor und nach dem Kabel ohne Zusatzrechte. Die Berechnung ist im Modell vorhanden (`HealthObservation::cable_resistance_mohm`). |
| Linux: aktueller Displaymodus über KMS | Braucht `/dev/dri`-Zugriff; bewusst nicht als Pflicht eingebaut. |
| DSC-Erkennung und Adapter-Erkennung | Kein öffentliches, plattformübergreifendes Feld gefunden; das Modell trägt `dsc` und `adapter`. |
| Selbst-Update des App-Bundles | v1 zeigt nur einen Hinweis mit Link. |
