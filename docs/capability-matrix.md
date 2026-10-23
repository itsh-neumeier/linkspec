# Fähigkeitsmatrix

Pro Plattform und Feld: Quelle, Mindestversion, Rechtebedarf und Prüfstatus.

**Prüfstatus**

| Status | Bedeutung |
|---|---|
| `verified-on-hardware` | Auf echter Hardware mit bekanntem Kabel/Gerät geprüft (`docs/manuell.md`). |
| `verified-in-ci` | Der Code-Pfad läuft auf dem CI-Runner der Plattform gegen das echte Betriebssystem (ohne USB-C-Hardware). |
| `spec-only` | Aus Spezifikation bzw. Plattform-Doku implementiert und mit synthetischen Fixtures getestet. Auf Hardware noch nicht geprüft. |
| `hypothesis` | Name oder Layout ist nicht offiziell dokumentiert. Aus öffentlichen Quellen abgeleitet, Prüfung mit eigenem Capture nötig (ID in Klammern). |
| `nicht verfügbar` | Die Plattform liefert den Wert ohne Zusatzrechte nicht. LinkSpec zeigt den ehrlichen Leerzustand. |

Stand: Version 0.9.0 (vor 1.0). **Noch kein Feld ist `verified-on-hardware`.**
Die Prüfläufe auf echter Hardware stehen in `docs/MAINTAINER_TODO.md` und
`docs/manuell.md`.

## Linux

Quelle: sysfs ohne Root. Doku: Linux-Kernel `Documentation/ABI/testing/`.
Änderungen: Kernel-Uevents über Netlink (`NETLINK_KOBJECT_UEVENT`).

| Feld | Quelle | Mindestversion | Rechte | Status |
|---|---|---|---|---|
| Ports, Partner, Kabel | `/sys/class/typec/` | Kernel 4.12 (Klasse), UCSI-Treiber | keine | spec-only |
| Port-Lage (`_PLD`) | `/sys/class/typec/portN/physical_location/` | 5.19 | keine | spec-only |
| Discover Identity Partner | `portN-partner/identity/*` | 4.12; UCSI-Firmware muss liefern | keine | spec-only |
| E-Marker Kabel | `portN-cable/identity/*` | UCSI ≥ 2.0 mit GET_PD_MESSAGE bzw. TCPM | keine | spec-only |
| „Kein E-Marker“ | – | – | – | nicht verfügbar: leeres/fehlendes Identity lässt sich nicht von „nicht gemeldet“ unterscheiden (H-L1). Anzeige: „Kabeldaten nicht verfügbar“. |
| PDOs Netzteil | `/sys/class/usb_power_delivery/pdN/source-capabilities/` über Link `portN-partner/usb_power_delivery` | 6.0 | keine | spec-only |
| Sink-PDOs Rechner | `portN/usb_power_delivery` → `sink-capabilities` | 6.0 | keine | spec-only |
| PD-Vertrag | `/sys/class/power_supply/ucsi-source-psy-*:NNN/{voltage_now,current_now}`; Zuordnung Connector NNN → `port(NNN-1)` (H-L2) | 5.10 | keine | spec-only |
| Type-C-Strom ohne PD | `portN/power_operation_mode` | 4.12 | keine | spec-only |
| Akku | `/sys/class/power_supply/BAT*/` | alle | keine | spec-only |
| USB-Geräte und Tempo | `/sys/bus/usb/devices/*/{speed,version,idVendor,…}` | alle | keine | spec-only |
| Weitere USB-Geräte (USB-A, intern) | – | – | – | noch nicht umgesetzt (nur Windows) |
| Zuordnung USB-Port ↔ Type-C | Hub-Port-Link `usbN-portM/connector` (und `peer`) | 6.x (je nach Plattform-Firmware) | keine | spec-only |
| Port-Fähigkeit USB | Bus-Speed des verlinkten Root-Hubs | alle | keine | spec-only |
| Overcurrent-Zähler | `usbN-portM/over_current_count` | 5.x | keine | spec-only |
| Orientierung | `portN/orientation` | 5.x | keine | spec-only |
| Thunderbolt/USB4-Link | `/sys/bus/thunderbolt/devices/*/{generation,rx_speed,rx_lanes,tx_speed,tx_lanes}` (Controller-Werte nach Link-Training) | 5.x | keine | spec-only |
| Zuordnung USB4-Port ↔ Type-C | `usb4_portN/connector` (H-L3) | 6.x | keine | hypothesis (H-L3) |
| DisplayPort Alt Mode | Altmode `svid=ff01`, `active`, `displayport/pin_assignment` | 5.x | keine | spec-only |
| Displays, Höchstmodus | `/sys/class/drm/*/status`, `edid` | alle | keine | spec-only |
| Aktueller Displaymodus | – | – | KMS-Zugriff nötig | nicht verfügbar |
| Zuordnung Display ↔ Port | Link `typec_connector` (H-L4), sonst nur wenn genau ein Port DP aktiv hat | 6.x | keine | hypothesis (H-L4) |
| Power Monitor | UCSI-Power-Supply bzw. Akku (`power_now`, `current_now`, `voltage_now`) | alle | keine | spec-only |
| Deep-Probe | – | – | – | Linux liest nur passiv; der Schalter hat keine aktiven Abfragen abzuschalten. |
| Flüssigkeitserkennung | – | – | – | nicht verfügbar (kein generisches sysfs-Attribut) |

## macOS (Apple Silicon, macOS 14+)

Quelle: öffentliche IOKit-Registry (IOService-Ebene) und CoreGraphics. Kein
Root, keine Entitlements, keine privaten Frameworks. Klassen- und
Property-Namen der Apple-Silicon-Treiber sind nicht offiziell dokumentiert:
Alle Zuordnungen unten sind Hypothesen, bis ein eigener Capture sie bestätigt.

| Feld | Quelle | Mindestversion | Rechte | Status |
|---|---|---|---|---|
| Ports | Klasse `AppleHPMInterfaceType*`, Property `PortNumber` (H-M1) | 14 | keine | hypothesis (H-M1) |
| Verbunden | `ConnectionActive` (H-M2) | 14 | keine | hypothesis (H-M2) |
| Transports aktiv/unterstützt | `TransportsActive`, `TransportsSupported` (`USB2`, `USB3`, `CIO`, `DisplayPort`) (H-M3) | 14 | keine | hypothesis (H-M3) |
| USB-Geräte | `IOUSBHostDevice` (`idVendor`, `Device Speed` = `kUSBHostPortSpeed*`, `bcdUSB`) | 11 | keine | spec-only (IOUSBHostFamily-Header) |
| Weitere USB-Geräte (USB-A, intern) | – | – | – | noch nicht umgesetzt (nur Windows) |
| Zuordnung USB ↔ Port | XHCI-Controller `usb-drdN` → Port N+1 (H-M4) | 14 | keine | hypothesis (H-M4) |
| Netzteil-PDOs, Vertrag | `AppleSmartBattery.AdapterDetails.UsbHvcMenu`, `UsbHvcHvcIndex` (H-M5); nur zuordenbar, wenn genau ein Port verbunden ist | 14 | keine | hypothesis (H-M5) |
| Leistungsaufnahme | `AppleSmartBattery.PowerTelemetryData.SystemPowerIn` (H-M5) | 14 | keine | hypothesis (H-M5) |
| Akku | `AppleSmartBattery` (`CurrentCapacity`, `IsCharging`, `FullyCharged`, `Voltage`, `Amperage`) | 11 | keine | spec-only |
| E-Marker, Discover Identity | Properties mit „Identity“/„VDO“ und SOP/SOP' im Namen im Port-Teilbaum (H-M6) | 14 | keine | hypothesis (H-M6) |
| Thunderbolt-Link | `IOThunderboltSwitch*`, `Current Link Speed`/`Current Link Width` (H-M7), Controller `acioN` → Port N+1 | 14 | keine | hypothesis (H-M7) |
| Displays | CoreGraphics `CGDisplayCopyDisplayMode`, `CGDisplayCopyAllDisplayModes` | 10.6 | keine | spec-only |
| Zuordnung Display ↔ Port | nur wenn genau ein Port DP aktiv hat und genau ein externes Display existiert (H-M8) | – | keine | hypothesis (H-M8) |
| SMC-Leistungsdaten | – | – | – | nicht gebaut (ADR 0006) |
| Intel-Macs | – | – | – | nicht unterstützt (Exit 3) |

## Windows 11 (22H2+)

| Feld | Quelle | Mindestversion | Rechte | Status |
|---|---|---|---|---|
| Physische USB-C-Ports | Hub-IOCTL `IOCTL_USB_GET_PORT_CONNECTOR_PROPERTIES` (`PortConnectorIsTypeC`, `PortIsUserConnectable`, Companion; Puffer 18 Byte) | Windows 8 | keine | verified-on-hardware (CELSIUS W5012, `fixtures/windows/celsius_w5012_tier1.json`) |
| USB 2/3 je Port | `_V2` `SupportedUsbProtocols`, auch an leeren Ports | Windows 8 | keine | spec-only |
| Weitere USB-Geräte (USB-A, intern) | Hub-Baum unter den Root-Hubs ohne die USB-C-Ports | Windows 8 | keine | verified-on-hardware (CELSIUS W5012) |
| Geräte, Tempo | `IOCTL_USB_GET_NODE_CONNECTION_INFORMATION_EX` (+ `_V2`) | Windows 8 | keine | spec-only |
| SuperSpeedPlus-Rate, Lanes | `IOCTL_USB_GET_NODE_CONNECTION_SUPERSPEEDPLUS_INFORMATION` (Funktionscode 280, H-W6) | Windows 10 1803 | keine | hypothesis (H-W6) |
| Gerät kann SuperSpeed | `_V2`-Flags `DeviceIsSuperSpeedCapableOrHigher` | Windows 8 | keine | spec-only |
| Gerätenamen | SetupAPI `DEVPKEY_Device_BusReportedDeviceDesc` | Windows 7 | keine | spec-only |
| Deep-Probe | `IOCTL_USB_GET_DESCRIPTOR_FROM_NODE_CONNECTION` (String, BOS) | Windows 7 | keine | spec-only |
| Overcurrent | Verbindungsstatus `DeviceCausedOvercurrent` | Windows 7 | keine | spec-only |
| Port-Fähigkeit USB | nur Untergrenze (Hub-IOCTLs nennen das Port-Maximum nicht) | – | keine | spec-only |
| Ports hinter internem Hub | Hub an nicht erreichbarem Root-Port | – | keine | spec-only |
| PD-Vertrag, PDOs | UCSI/UCM-Stack, nur Kernel | – | Stufe 2 (Dienst, ETW) | nicht verfügbar in Stufe 1; Stufe 2 hypothesis (H-W2) |
| E-Marker | UCSI `GET_CABLE_PROPERTY`/`GET_PD_MESSAGE`, nur Kernel | – | Stufe 2 | nicht verfügbar in Stufe 1; Stufe 2 hypothesis (H-W2) |
| Zuordnung Dienst-Connector ↔ Port | UCSI-Connector N → Port N (H-W5) | – | – | hypothesis (H-W5) |
| Akku | `IOCTL_BATTERY_QUERY_STATUS`/`_INFORMATION` | Windows 2000 | keine | spec-only |
| Power Monitor | Akku-Rate (mW); EMI siehe `docs/spaeter.md` | – | keine | spec-only |
| USB4-Router | Geräteknoten mit Enumerator `USB4` (H-W3), nur bei genau einem Port zuordenbar | Windows 11 | keine | hypothesis (H-W3) |
| USB4-Linkrate | – | – | – | nicht verfügbar |
| Displays | `QueryDisplayConfig`, `DisplayConfigGetDeviceInfo`, EDID aus dem Monitor-Registrierungsschlüssel | Windows 7 | keine | spec-only |
| Zuordnung Display ↔ Port | Ausgang DisplayPort extern/USB-Tunnel und genau ein verbundener Port (H-W4) | – | keine | hypothesis (H-W4) |
| Hinweis „PC lädt langsam“ | – | – | – | nicht abgreifbar gefunden (siehe `MAINTAINER_TODO.md`) |
| Änderungen | `CM_Register_Notification` | Windows 8 | keine | spec-only |

## Hypothesen-Register

| ID | Annahme | Prüfung |
|---|---|---|
| H-L1 | Leeres Kabel-Identity unter UCSI ist mehrdeutig. | Capture mit E-Marker-Kabel und mit einfachem Kabel vergleichen. |
| H-L2 | UCSI-Power-Supply-Suffix `:NNN` ist die 1-basierte Connector-Nummer. | Capture mit Netzteil an verschiedenen Ports. |
| H-L3 | `usb4_portN/connector` verlinkt den Type-C-Port. | Capture an einem USB4-Laptop. |
| H-L4 | DRM-Connector hat Link `typec_connector`. | Capture mit USB-C-Monitor. |
| H-M1–H-M8 | Siehe Tabelle macOS. | `linkspec --capture` auf M1/M2/M3/M4 mit Dock, Netzteil, TB-Kabel. |
| H-W2 | Die Type-C-ETW-Provider liefern PDOs, RDO und Identity als Eigenschaften mit erkennbaren Namen. | `linkspec-service providers` und `console` auf UCSI-Laptop; Rohereignisse im Capture. |
| H-W3 | USB4-Router erscheinen mit Enumerator `USB4`. | Gerätemanager-Export auf USB4-Laptop. |
| H-W4 | Displays an USB-C haben Ausgabetechnik 10 oder 19. | Capture mit USB-C-Monitor. |
| H-W5 | UCSI-Connector N entspricht der N-ten USB-C-Buchse in Location-Reihenfolge. | Capture mit Netzteil an jeder Buchse. |
| H-W6 | Funktionscode 280 ist `USB_GET_NODE_CONNECTION_SUPERSPEEDPLUS_INFORMATION`. | Ein 10-Gbit/s-Gerät zeigt `ssp` im Capture. |
