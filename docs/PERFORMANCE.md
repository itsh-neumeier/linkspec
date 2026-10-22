# Leistung

Ziele (Auftrag, Abschnitt 8) und Messwerte. Messung mit `scripts/perf.sh`
bzw. den Befehlen unten, Release-Profil (`lto = "fat"`, `codegen-units = 1`,
`panic = "abort"`, `strip = true`).

Messumgebung: Linux x86_64 (Cloud-Container, 4 vCPU), X-Server Xvfb, Fake-
Backend mit Fixture `c10_dock_usb3_and_dp.json` bzw. `c07b_…`. Echte Hardware
steht noch aus (`docs/MAINTAINER_TODO.md`).

| Ziel | Messwert (2026-10-22) | Bewertung |
|---|---|---|
| Kaltstart bis sichtbares Fenster/Flyout < 300 ms | 75–104 ms bis zum ersten gezeichneten Frame (5 Läufe; Umgebungsvariable `LINKSPEC_PERF_EXIT_AFTER_FIRST_FRAME`) | erfüllt |
| RAM im Leerlauf < 30 MB | RSS 32,5 MB, davon 8,4 MB privat (anonym) und 21,5 MB gemeinsam genutzte Bibliotheken und Binärcode | knapp über dem Ziel bei RSS; der private Anteil liegt weit darunter. Begründung: RSS zählt geteilte Systembibliotheken (X11, xkbcommon, fontconfig) mit, die auch andere Programme nutzen. |
| CPU im Leerlauf ≈ 0 % | 0 CPU-Ticks in 30 s (Flyout offen, keine Änderungen) | erfüllt |
| Ereignisgesteuert, Polling nur im Power Monitor | Worker schläft in `recv`; Tray-/Menüereignisse wecken den UI-Thread über Handler; Power Monitor tastet nur bei sichtbarem Tab im Sekundentakt ab | erfüllt |
| CLI | 0,00–0,01 s, max. RSS 6,1–6,5 MB (`/usr/bin/time`) | – |
| Binärgröße | `linkspec` 4,9 MB, `linkspec-app` 23,7 MB (Linux x86_64) | – |

## Entscheidungen für die Leistung

- **Slint-Software-Renderer** statt OpenGL (femtovg): Mit OpenGL lag der RSS
  bei ~160 MB (Mesa-Treiber) und der erste Frame bei 290–520 ms. Der
  Software-Renderer zeichnet die einfache Oberfläche schnell genug.
- Kein Polling: `CM_Register_Notification` (Windows), Netlink-Uevents (Linux),
  IOKit-Notifications mit 5-s-Fallback für Port-Eigenschaften (macOS).
- `vendors.db` ist eingebettet und wird nur bei Bedarf befragt.

## Messbefehle

```sh
cargo build --release -p linkspec-cli -p linkspec-app
/usr/bin/time -f "%e s %M KB" target/release/linkspec --fake fixtures/core/c10_dock_usb3_and_dp.json --no-history
LINKSPEC_PERF_EXIT_AFTER_FIRST_FRAME=1 target/release/linkspec-app --window --fake fixtures/core/c10_dock_usb3_and_dp.json
grep -E "VmRSS|RssAnon" /proc/$(pgrep -x linkspec-app)/status
```
