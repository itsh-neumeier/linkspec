# ADR 0003 – Windows: Stufe 1 ohne Admin, Stufe 2 als optionaler Dienst

Status: angenommen (Phase 0, umgesetzt Phase 3)

## Kontext

PD-Vertrag, PDO-Liste und Kabel-E-Marker liegen unter Windows im UCSI-/UCM-
Kernel-Stack (`UcmUcsiCx`, `UcmCx`). Die UCSI-Befehle sind nur Kernel-Treibern
dokumentiert. User-Mode ohne Admin erreicht nur Hub-IOCTLs, Geräteeigenschaften,
Akku, EMI und die Display-Konfiguration.

## Entscheidung

- **Stufe 1 (immer)**: Hub-IOCTLs (`IOCTL_USB_GET_PORT_CONNECTOR_PROPERTIES`,
  `…_NODE_CONNECTION_INFORMATION_EX(_V2)`, `…_SUPERSPEEDPLUS_INFORMATION`),
  SetupAPI/CfgMgr32, Akku-IOCTLs, `QueryDisplayConfig`,
  `CM_Register_Notification`. Felder, die Stufe 1 nicht liefert, stehen in
  `unavailable` mit Grund `needs_service`; die App zeigt „Erweiterte
  Kabeldaten benötigen den LinkSpec-Dienst“.
- **Stufe 2 (optional)**: `linkspec-service` als Windows-Dienst (LocalSystem).
  Er startet eine ETW-Echtzeitsitzung auf Providern, deren Name zu den
  Type-C-Mustern passt (`TdhEnumerateProviders`; keine fest eingebauten
  GUIDs, weil diese nicht offiziell dokumentiert sind). Ereigniseigenschaften
  werden über TDH dekodiert und nach Namensmustern auf Connector-Zustand
  abgebildet (Hypothese H-W2). Die App spricht ohne Admin über die Named Pipe
  `\\.\pipe\linkspec-service` (Lesen/Schreiben für angemeldete Benutzer, nur
  feste Leseanfragen).
- Der Dienst ist bei der MSI-Installation abwählbar.

## Folgen

- Ob die Provider die gewünschten Felder tragen, lässt sich nur auf Hardware
  prüfen (`linkspec-service providers` / `console`). Bis dahin ist Stufe 2
  `hypothesis`.
