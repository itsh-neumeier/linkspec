# LinkSpec WidgetKit-Extension (macOS)

Nur Anzeige (ADR 0005). Die App schreibt `widget.json` (Schema
`linkspec.widget.v1`) in den App-Group-Container
`group.io.github.itsh-neumeier.linkspec`; das Widget liest ihn.

## Bauen (Maintainer, Xcode 15+)

1. In Xcode ein Projekt „LinkSpecWidgetHost“ (macOS App) anlegen und ein
   Target „Widget Extension“ hinzufügen. Den Inhalt von
   `LinkSpecWidget/LinkSpecWidget.swift` in die Extension übernehmen.
2. Für App und Extension die App Group
   `group.io.github.itsh-neumeier.linkspec` aktivieren (Signing & Capabilities).
3. Bundle-IDs: App `io.github.itsh-neumeier.linkspec`, Extension
   `io.github.itsh-neumeier.linkspec.widget`.
4. Die gebaute `LinkSpecWidget.appex` nach
   `LinkSpec.app/Contents/PlugIns/` kopieren und das Bundle mit Developer ID
   signieren (`docs/MAINTAINER_TODO.md`).

Ohne Developer-ID-Signatur lädt macOS die Extension nicht; deshalb ist sie
nicht Teil des unsignierten CI-Builds.
