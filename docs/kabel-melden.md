# Kabel melden

LinkSpec erkennt Kabel mit E-Marker über einen Fingerabdruck (Hash aus VID,
PID, bcdDevice, XID und Kabel-VDOs). Die gemeinsame Liste bekannter Kabel
(`data/cables.csv`) entsteht aus Meldungen der Community.

## So melden Sie ein Kabel

1. Kabel anstecken (beide Enden, mit Gerät oder Netzteil).
2. `linkspec --report` ausführen **oder** im Tray-Menü „Kabel melden“ wählen.
3. Der Browser öffnet ein vorausgefülltes GitHub-Issue
   („Kabel melden“-Formular). Ergänzen Sie Marke, Modell, Länge, Aufdruck und
   – wenn vorhanden – Ihre eigene Messung.
4. Erst mit „Submit new issue“ wird etwas gesendet.

## Was im Bericht steht

VID, PID, Typ, Tempo, Strom/Watt laut Marker, XID, Fingerabdruck, die rohen
VDOs, Betriebssystem und LinkSpec-Version sowie die beobachtete Verbindung.
Keine Seriennummern, keine Benutzer- oder Hostnamen.

## Aufnahme in die Liste

Maintainer prüfen die Meldung und tragen `fingerprint,name,source` in
`data/cables.csv` ein (Quelle = Issue-URL). Ohne E-Marker gibt es keinen
Fingerabdruck; solche Kabel lassen sich nicht eindeutig wiedererkennen.
