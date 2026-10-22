# ADR 0001 – GUI mit Slint, Tray mit tray-icon

Status: angenommen (Phase 0)

## Kontext

LinkSpec braucht ein Tray-/Menüleisten-Flyout und ein Fenster auf Windows 11,
macOS und Linux. Vorgaben: durchgängig Rust, kein WebView, kein
JavaScript/.NET/Electron/Tauri, kein egui, kleine Binärgröße, Hell/Dunkel nach
System, Barrierefreiheit.

## Bewertete Optionen

| Option | Bewertung |
|---|---|
| **Slint** | Native Darstellung mit eigenem Renderer, Hell/Dunkel über `Palette`, Screenreader-Anbindung (AccessKit), kleine Binärgröße, deklarative UI in `.slint`. Lizenz: GPL-3.0 **oder** Royalty-free-Lizenz für Desktop-Apps (Namensnennung) **oder** kommerziell. |
| iced | Reines Rust, MIT. Barrierefreiheit noch unvollständig, kein deklaratives UI-Format. |
| gtk-rs / relm4 | Unter Windows und macOS fremd im Aussehen, große Laufzeitabhängigkeit. |
| Native Toolkits je Plattform (WinUI, AppKit, GTK) | Drei UIs, widerspricht „ein Workspace“. |
| egui | Ausgeschlossen durch die Vorgabe. |

## Entscheidung

- GUI: **Slint 1.x** unter der *Royalty-free Desktop License 2.0*. Die
  Namensnennung erfolgt im Über-Dialog (`AboutSlint`) und in
  `THIRD_PARTY_LICENSES`. Der LinkSpec-Code bleibt MIT.
- Tray: **tray-icon** (Windows-Tray, macOS-Menüleiste). Auf Linux mit dem
  **ksni**-Backend (StatusNotifierItem über D-Bus), damit kein GTK nötig ist.
- Linux ohne Tray-Host (z. B. GNOME ohne Erweiterung): Fallback Fenster.
- Texte kommen aus Fluent (`locales/*.ftl`) über ein Slint-Global `Tr`, nicht
  aus Slint-Übersetzungen, damit es nur eine String-Tabelle gibt.

## Folgen

- Rechtsprüfung der Slint-Lizenzbedingungen steht in `docs/MAINTAINER_TODO.md`.
- Shift-/⌥-Klick auf das Tray-Icon wird von tray-icon nicht mit Modifikatoren
  gemeldet; die technische Detailansicht hat deshalb einen Schalter im Flyout
  (die Vorgabe erlaubt „oder Schalter“).
