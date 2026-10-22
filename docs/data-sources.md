# Datenquellen

| Quelle | Inhalt | Lizenz | Gebündelt |
|---|---|---|---|
| `usb.ids` (linux-usb.org), Version siehe `data/usb.ids` | USB-Vendor-Namen | GPL-2.0+ **oder** BSD-3-Clause; genutzt: BSD-3-Clause (`data/LICENSE.usb-ids`) | ja (nur Vendor-Zeilen, in `vendors.db`) |
| USB-IF Integrators List / XID-Liste | zertifizierte Produkte | nicht geklärt | **nein** (`data/xids.csv` leer) |
| Kabel-Fingerabdrücke | Community-Meldungen über „Kabel melden“ | Beiträge unter MIT (Hinweis im Issue-Template) | ja, startet leer (`data/cables.csv`) |

Aktualisieren: `data/usb.ids` ersetzen, `cargo build` erzeugt `vendors.db`
neu (`data/build_vendors.rs`). In den Einstellungen steht eine Zeile mit Stand
und Lizenz der Herstellerliste.

## Spezifikationen und Plattform-Doku

- USB Power Delivery Specification R3.2 V1.1 (USB-IF): PDOs §6.4.1, RDO §6.4.2,
  Discover Identity §6.4.4.3.1.
- USB Type-C Cable and Connector Specification R2.x (Stromstufen, Kabel ohne
  E-Marker: 3 A).
- UCSI Specification 2.x/3.0 (Konzepte; Befehle nur im Kernel nutzbar).
- USB4 Specification v2.0 (Generationen, Lanes).
- VESA DisplayPort 1.4a/2.1 (Kanal-Codierung), VESA E-EDID, CTA-861.
- Linux: `Documentation/ABI/testing/sysfs-class-typec`,
  `sysfs-class-usb_power_delivery`, `sysfs-class-power`,
  `sysfs-bus-thunderbolt`, `sysfs-bus-usb`.
- Apple: IOKit Framework Reference, IOUSBHostFamily (SDK-Header), Quartz
  Display Services, SMAppService.
- Microsoft Learn: `usbioctl.h`, `batclass.h`, Display Configuration API,
  CfgMgr32, ETW/TDH, Service-API.
