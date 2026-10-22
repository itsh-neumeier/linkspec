# Grenzen

## LinkSpec ist kein Kabeltester

Software sieht nur, was die Port-Controller und die E-Marker der Kabel
melden. Ein E-Marker ist ein kleiner Chip im Stecker, den der Hersteller
programmiert. **E-Marker können falsche Angaben machen** – absichtlich oder
aus Versehen. LinkSpec vergleicht die Angaben mit der tatsächlich
ausgehandelten Verbindung und weist auf Widersprüche hin. Es setzt aber nie
ein „Fälschung“-Abzeichen, weil Software das nicht beweisen kann.

Was LinkSpec nicht messen kann:

- Leiterquerschnitt, Schirmung, Signalqualität, Bitfehlerrate.
- Ob ein Kabel ohne E-Marker USB-3-Leitungen hat, solange kein USB-3-Gerät
  angeschlossen ist.
- Kabelwiderstand – nur, wenn Spannung vor und nach dem Kabel unter Last
  gleichzeitig gemessen vorliegen. Keine der unterstützten Plattformen liefert
  das ohne Zusatzrechte; die Anzeige erscheint daher in v1 nicht.

## Plattform-Grenzen

- **Windows** ohne LinkSpec-Dienst: keine PD-Verträge, keine PDOs, keine
  E-Marker-Daten (diese liegen im Kernel-Stack). Die App zeigt „Erweiterte
  Kabeldaten benötigen den LinkSpec-Dienst“.
- **Windows**: Die Port-Fähigkeit ist nur als Untergrenze bekannt; LinkSpec
  macht deshalb unter Windows nie den Port zum Engpass.
- **Linux**: Ob ein Kabel „keinen E-Marker“ hat oder die Firmware ihn nicht
  meldet, ist unter UCSI nicht unterscheidbar. Der aktuelle Displaymodus ist
  ohne KMS-Zugriff nicht lesbar.
- **macOS**: Netzteil-Daten sind nur einem Port zuordenbar, wenn genau ein
  Port verbunden ist. Kein SMC-Zugriff (ADR 0006). Intel-Macs werden nicht
  unterstützt.
- **Ports hinter internen Hubs** (z. B. Front-USB-C am Desktop): Geräte
  erscheinen, Kabel- und Ladedaten sind „nicht verfügbar“. LinkSpec
  verurteilt solche Ports nie als Nur-Lade-Kabel.
- **Flüssigkeitserkennung, Pin-Belegung**: nur Orientierung, wo die
  Plattform sie meldet (Linux `orientation`, macOS-Hypothese H-M3).

## Daten, die nicht gebündelt sind

- Die USB-IF-Liste zertifizierter Produkte (XID) ist nicht enthalten
  (Lizenz unklar). Ein gemeldetes XID erscheint als „Zertifikat gemeldet, bei
  uns nicht gelistet“ – das ist kein Fehler.
