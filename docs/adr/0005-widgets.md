# ADR 0005 – Widgets: WidgetKit in Swift (Ausnahme), Windows-Widget später

Status: angenommen (Phase 7)

## Entscheidung

- **macOS**: WidgetKit verlangt eine Widget-Extension in Swift/SwiftUI. Das ist
  die einzige Ausnahme von „durchgängig Rust“. Die Extension
  (`widgets/macos/`) zeigt nur einen Snapshot an (`widget.json`, geschrieben
  von der App in den App-Group-Container) und enthält keine Diagnoselogik.
- **Windows**: Ein Widget-Board-Provider braucht Paket-Identität (MSIX),
  einen registrierten COM-Server und Adaptive Cards. Ohne signiertes MSIX
  lässt er sich nicht installieren. Die App schreibt bereits denselben
  Snapshot (`%LOCALAPPDATA%\LinkSpec\widget.json`); der Provider ist in
  `docs/spaeter.md` verschoben.
- **Linux**: keine Widgets in v1.

## Snapshot-Format `linkspec.widget.v1`

```json
{ "schema": "linkspec.widget.v1", "generated_at": "…",
  "ports": [ { "label": "USB-C 1", "headline": "usb_device",
               "headline_text": "USB-Gerät", "summary": "…", "status": "ok" } ] }
```
