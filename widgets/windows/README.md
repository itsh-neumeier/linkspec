# Windows-Widget

Verschoben nach `docs/spaeter.md` (ADR 0005): Ein Widget-Board-Provider braucht
MSIX-Paketidentität und einen signierten COM-Server. Die App schreibt bereits
den Snapshot `%LOCALAPPDATA%\LinkSpec\widget.json` (Schema
`linkspec.widget.v1`), den ein Provider anzeigen kann.
