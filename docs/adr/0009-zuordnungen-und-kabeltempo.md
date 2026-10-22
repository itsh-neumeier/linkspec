# ADR 0009 – Kabeltempo, Wattklassen und Urteilsregeln

Status: angenommen (Phase 1)

## Entscheidungen

1. **Datentempo laut Marker** folgt den USB-IF-Logo-Namen für zertifizierte
   Kabel: Gen1 → 5 Gbit/s, Gen2 → 20 Gbit/s, Gen3 → 40 Gbit/s,
   Gen4 → 80 Gbit/s, USB 2.0 → 0,48 Gbit/s. Vergleiche in der Urteils-Engine
   laufen über die Signalklasse (USB 2.0, Gen1 … Gen4), nicht über Gbit/s,
   weil ein Gen1-Kabel zwar zwei Lanes, aber kein Gen2 trägt.
2. **Wattklasse**: 3 A → 60 W, 5 A → 100 W, 5 A mit EPR-Bit und 50 V → 240 W
   (USB PD R3.2 §6.4.4.3.1.6). Ohne E-Marker gibt es keine Wattklasse.
3. **Thunderbolt/USB4-Link langsamer als Marker und Port**: `limiter=cable`
   nur, wenn die Linkrate aus dem Controller stammt
   (`measured_by_controller`). Sonst `unknown`.
4. **USB-Link langsamer als Marker** bei Port und Gerät mit mehr Tempo:
   Hinweis „Angabe und Verbindung passen nicht zusammen“, `limiter=unknown`.
5. **Windows-Port-Fähigkeit** ist nur eine Untergrenze
   (`usb_is_lower_bound`); die Engine macht den Port dann nie zum Engpass.
6. **Leistung**: Bezieht der Rechner weniger als 85 % des möglichen Wertes
   (minus 2 W), lautet das Urteil `charging_below_max` (`limiter=none`).
   Sonst bestimmt das schwächste bekannte Glied das Urteil. Gleichstand
   Netzteil/Kabel geht an das Netzteil.
7. **Widerspruchsverbot**: DisplayPort mit 4 Lanes und gleichzeitig USB 3
   (nicht getunnelt) ist unmöglich; die Lane-Zahl wird dann weggelassen.
