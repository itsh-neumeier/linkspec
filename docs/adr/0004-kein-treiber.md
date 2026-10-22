# ADR 0004 – Kein eigener Treiber (Stufe 3) in v1

Status: angenommen (Phase 0)

## Entscheidung

Ein KMDF-Companion für UCSI wird in v1 **nicht** gebaut.

## Begründung

- Stufe 2 (ETW) ist noch nicht auf Hardware bewertet. Der Auftrag erlaubt
  Stufe 3 nur, wenn Stufe 2 nicht reicht.
- Ein Treiber braucht Produktionssignierung (EV-Zertifikat, Attestation über
  das Partner Center). Das ist ohne Maintainer-Konten nicht möglich.
- Ein Treiber im UCSI-Stack ist ein Stabilitätsrisiko für Ladefunktionen.

## Folgen

`driver/windows/` bleibt leer. Die Bewertung nach den Hardware-Prüfläufen
steht in `docs/MAINTAINER_TODO.md`; eine neue ADR kann diese ablösen.
