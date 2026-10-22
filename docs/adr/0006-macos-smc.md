# ADR 0006 – macOS: kein SMC-Zugriff

Status: angenommen (Phase 0)

## Entscheidung

LinkSpec liest Leistungsdaten auf macOS nur aus öffentlichen
Registry-Properties von `AppleSmartBattery` (`PowerTelemetryData`,
`AdapterDetails`). Ein SMC-Zugriff (`AppleSMC` über `IOConnectCallMethod`)
wird nicht gebaut.

## Begründung

Die SMC-Schlüssel und Strukturen sind nicht dokumentiert, unterscheiden sich
je Chip-Generation und sind ohne eigene Hardware nicht prüfbar. Der Auftrag
erlaubt SMC nur isoliert und abschaltbar mit Fallback; der Nutzen über die
öffentlichen Properties hinaus ist ohne Messungen nicht belegbar.

## Folgen

Kabelwiderstand und Port-Leistung je Port werden auf macOS nicht angezeigt
(`docs/grenzen.md`). Eine spätere ADR kann SMC als abschaltbares Modul
einführen, wenn Captures den Nutzen belegen.
