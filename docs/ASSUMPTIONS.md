# Annahmen

Annahmen, die bei Unklarheit getroffen wurden (konservativste, am besten
dokumentierte Option). Jede Annahme hat eine ID.

| ID | Annahme | Begründung |
|---|---|---|
| A-001 | Alle Phasen liegen auf dem Arbeits-Branch `claude/confident-babbage-i1gttl`; das Überführen nach `main` und die Tags `phase-N` macht der Maintainer per Pull Request. | Die Sitzung darf nur auf den zugewiesenen Branch pushen. Commits sind pro Phase getrennt; die Tags sind lokal angelegt und in `docs/PROGRESS.md` mit Commit-Hash aufgeführt. |
| A-002 | Die CI-Runner haben keine USB-C-Hardware. „Grün auf allen drei Runnern“ heißt: Fixtures, Interpreter, Schema und ein Smoke-Test gegen das echte Betriebssystem. | Hardware-Prüfung ist Maintainer-Aufgabe (`docs/manuell.md`). |
| A-003 | Linux ohne `/sys/class/typec` gilt als „Datenquelle grundsätzlich nicht lesbar“ → Exit 3. | Ohne Type-C-Klasse lassen sich physische Ports nicht bestimmen. |
| A-004 | Kabeltempo laut Marker nach USB-IF-Logo-Namen (5/20/40/80 Gbit/s). | ADR 0009. |
| A-005 | `certification` ist `null`, wenn das Kabel keinen XID meldet; `unlisted`, wenn ein XID gemeldet wird und keine Liste gebündelt ist. | „Kein XID → nichts behaupten.“ Ohne gebündelte Liste ist jedes gemeldete XID „bei uns nicht gelistet“. |
| A-006 | Die Host-Leistungsgrenze (Sink-Maximum) zählt als `port` (der Rechner ist die Port-Seite). | Der Limiter-Enum kennt kein eigenes „host“. |
| A-007 | Zusätzliche, nicht im Auftrag genannte JSON-Keys (`status`, `location`, `notes`, `displays`, `health`, `notices`, `raw`, `history`, `known_as`) sind additive Erweiterungen von v1 und im Schema beschrieben. | Der Auftrag nennt die Keys „verkürzt“. |
| A-008 | Die CLI zeichnet Kabel in der lokalen Historie auf (abschaltbar mit `--no-history`), außer beim Fake-Backend. | Die Historie soll alle Sichtungen kennen; nichts verlässt das Gerät. |
| A-009 | Fixtures für macOS und Windows sind synthetisch und folgen den Hypothesen (H-M…, H-W…). | Ohne eigene Hardware gibt es keine echten Captures; fremde Captures sind nicht erlaubt. |
| A-010 | Die Versionsnummer bleibt bis zu den Hardware-Prüfläufen bei 0.9.x. Version 1.0.0 setzt der Maintainer nach `docs/manuell.md`. | Ein 1.0 ohne einen einzigen Hardware-Lauf wäre ein „stilles fertig“. |
| A-011 | Linux-Tray über StatusNotifierItem (ksni). Unter GNOME ohne Tray-Erweiterung startet die App im Fenstermodus. | Kein zuverlässiger Tray-Host unter GNOME. |
| A-012 | Ein Drop/Reconnect ist ein Abbruch der USB-/Fabric-Verbindung, während der Type-C-Partner verbunden bleibt. | Ein Ab- und Anstecken des Kabels entfernt den Partner und ist kein Drop. |
