# macOS: keine App Sandbox

LinkSpec wird **ohne App Sandbox** ausgeliefert und nicht über den Mac App
Store vertrieben.

Begründung: Die Sandbox blockiert den Zugriff auf viele IOKit-Registry-
Einträge und Benutzer-Clients der Port-Controller. Die Diagnose bräuchte dann
temporäre Ausnahmen (`com.apple.security.temporary-exception.iokit-user-client-class`),
die Apple nur selten genehmigt und die den Zweck der Sandbox aufheben.

Ausgleich:

- Hardened Runtime ist aktiv (`packaging/macos/entitlements.plist` ohne
  Ausnahmen außer keiner).
- Developer-ID-Signatur und Notarisierung sind vorbereitet
  (`docs/MAINTAINER_TODO.md`).
- Keine privaten Frameworks, keine privaten Entitlements, kein Root, kein
  Helper-Daemon, keine Freigaben für Festplattenvollzugriff,
  Bildschirmaufnahme oder Eingabeüberwachung.
- Die WidgetKit-Extension läuft in der Sandbox (Pflicht für Extensions) und
  liest nur `widget.json` aus dem App-Group-Container.
