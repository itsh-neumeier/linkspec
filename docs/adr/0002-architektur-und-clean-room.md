# ADR 0002 – Architektur: Collector/Interpreter, Engine-Crate, Clean Room

Status: angenommen (Phase 0)

## Entscheidung

1. Jedes Backend trennt einen **Collector** (OS-spezifisch, nur auf seiner
   Plattform kompiliert) von einem **Interpreter** (plattformunabhängig). Der
   Collector erzeugt einen Dump (sysfs-Abbild, IOKit-Registry-Abbild,
   Windows-Hub-Abbild). Der Interpreter macht daraus einen `Snapshot`.
2. `--capture` schreibt genau diesen Dump (anonymisiert). Fixtures sind
   Captures. So läuft jede Fixture auf allen CI-Runnern durch denselben Code
   wie auf der echten Maschine.
3. Zusätzlich zur Struktur aus dem Auftrag gibt es `linkspec-engine`: Backend-
   Auswahl, Sitzungs-Tracker, Historie und Einstellungen. CLI und App nutzen
   denselben Ablauf („kein zweites Diagnose-Hirn“).
4. `linkspec-core` enthält Modelle, PD-Decoder, Urteils-Engine, Texte (Fluent)
   und die Negotiation-Tabelle, ohne OS-APIs.

## Clean Room

Quellen sind ausschließlich öffentliche Spezifikationen (USB PD R3.2, UCSI,
USB4, VESA), offizielle Plattform-Doku (Apple, Microsoft Learn, Linux-ABI-Doku)
und eigene Messungen. Es wurde kein Code, kein Text, keine Datenbank und keine
Fixture eines anderen Projekts verwendet. Wo Namen nur aus öffentlichen
Ausgaben (z. B. `ioreg`-Ausgaben in Artikeln) bekannt sind, sind sie als
Hypothese markiert und werden mit eigenen Captures geprüft.
