# LinkSpec – deutsche Texte (Referenzsprache)
# Technische Begriffe (USB4, PDO, E-Marker, DisplayPort) bleiben Englisch.

## Überschriften (Abschnitt 5.1)

headline-thunderbolt_usb4 = Thunderbolt / USB4
headline-usb_device = USB-Gerät
headline-display = Display verbunden
headline-charging_only = Nur Laden
headline-slow_or_charge_only = Langsames USB oder Nur-Lade-Kabel
headline-empty = Nichts verbunden

summary-thunderbolt_usb4 = Thunderbolt- bzw. USB4-Verbindung aktiv.
summary-usb_device = USB-Gerät verbunden.
summary-display = Ein Display ist verbunden.
summary-charging_only = An diesem Port wird nur geladen.
summary-slow_or_charge_only = Keine schnelle Datenverbindung erkannt.
summary-empty = Nichts verbunden

## Portnamen und Lage

port-label-usb_c = USB-C { $index }
port-label-usb4 = USB4 { $index }
port-label-thunderbolt = Thunderbolt { $index }
location-panel-left = linke Seite
location-panel-right = rechte Seite
location-panel-front = Vorderseite
location-panel-back = Rückseite
location-panel-top = Oberseite
location-panel-bottom = Unterseite
location-horizontal-left = links
location-horizontal-center = Mitte
location-horizontal-right = rechts
location-vertical-upper = oben
location-vertical-center = mittig
location-vertical-lower = unten

## Geschwindigkeiten

usb-class-usb2 = USB 2.0
usb-class-gen1 = USB 3 (5 Gbit/s)
usb-class-gen2 = USB 3 (10 Gbit/s)
usb-class-gen3 = USB4 (40 Gbit/s)
usb-class-gen4 = USB4 (80 Gbit/s)
speed-gbps = { $gbps } Gbit/s
speed-fabric = Thunderbolt/USB4

## Urteile (Abschnitt 5.2)

verdict-power_ok = Lädt mit { $watts } W.
verdict-charging_below_max = Lädt mit { $watts } W (Netzteil kann bis { $max } W).
verdict-battery_full = Akku voll, lädt nicht.
verdict-cable_limits_power = Das Kabel begrenzt die Leistung auf { $watts } W. Netzteil und Rechner könnten { $other } W.
verdict-charger_limits_power = Das Netzteil liefert höchstens { $watts } W. Kabel und Rechner könnten mehr.
verdict-host_limits_power = Der Rechner nimmt an diesem Port höchstens { $watts } W auf. Kabel und Netzteil könnten mehr.
verdict-power_basic_only = Nur die Basisangabe ist sichtbar: USB-C mit bis zu { $watts } W. Details zum Netzteil liefert das System nicht.
verdict-power_supplying_device = Dieser Port versorgt das Gerät mit { $watts } W.
verdict-power_unknown = Die Lade-Angaben reichen für ein Urteil nicht aus.
verdict-data_ok = Daten laufen mit { $speed }, so schnell wie möglich.
verdict-device_at_its_max =
    { $cable ->
        [ok] Das Endgerät ist mit { $speed } am Limit, das Kabel ist in Ordnung.
       *[other] Das Endgerät ist mit { $speed } am Limit.
    }
verdict-cable_limits_data = Das Kabel begrenzt die Daten auf { $speed }. Port und Gerät könnten mehr.
verdict-cable_likely_usb2 = Wahrscheinlich ein USB-2.0- oder Nur-Lade-Kabel: Port und Gerät könnten schneller.
verdict-port_limits_data = Der Port begrenzt die Daten auf { $speed }. Gerät und Kabel könnten mehr.
verdict-emarker_link_mismatch = Angabe und Verbindung passen nicht zusammen: Das Kabel meldet { $marker }, die Verbindung läuft mit { $speed }.
verdict-no_data_link = Keine Datenverbindung: Das Kabel hat vielleicht keine Datenleitungen, oder das Gerät überträgt keine Daten.
verdict-data_via_internal_hub = Daten laufen mit { $speed } über einen internen Hub. Kabeldaten sind an diesem Port nicht verfügbar.
verdict-data_unknown = Daten laufen mit { $speed }. Was die Verbindung begrenzt, lässt sich nicht sicher sagen.
verdict-fabric_link_below_cable = Die Verbindung läuft nur mit { $link } Gbit/s, obwohl das Kabel { $marker } Gbit/s meldet und Port und Gerät mehr können. Das Kabel bremst.
verdict-fabric_link_unverified = Die Verbindung läuft mit { $link } Gbit/s, das Kabel meldet { $marker } Gbit/s. Ohne Messung des Controllers ist die Ursache unklar.
verdict-display_at_max = Das Display läuft mit seinem Höchstmodus.
verdict-display_below_max = Das Display läuft unter seinem Höchstmodus, obwohl die Verbindung mehr tragen würde. Bitte die Anzeige-Einstellungen prüfen.
verdict-display_link_too_slow = Das Display läuft unter seinem Höchstmodus: Dieser braucht { $need } Gbit/s, die DisplayPort-Verbindung trägt { $cap } Gbit/s.
verdict-display_lanes_shared = Das Display läuft unter seinem Höchstmodus: Das Dock teilt die Leitungen zwischen USB 3 und DisplayPort.
verdict-display_unknown = Für das Display fehlen Angaben, um den Modus zu beurteilen.

lead-device_at_its_max =
    { $cable ->
        [ok] Das Endgerät nutzt { $speed }. Das Kabel kann mehr.
       *[other] Das Endgerät nutzt { $speed } und ist damit am Limit.
    }

## Kabel (Abschnitt 5.3)

cable-no-emarker = Dieses Kabel meldet keine Kennung
cable-kind-passive = Passiv
cable-kind-active = Aktiv
cable-kind-optical = Optisch
cable-kind-unknown = Typ unbekannt
cable-speed = { $gbps } Gbit/s
cable-power = { $amps } A / { $watts } W
cable-certification-listed = Zertifiziert: { $name }
cable-certification-unlisted = Zertifikat gemeldet, bei uns nicht gelistet
cable-xid-no-list = Zertifikat gemeldet (XID { $xid }); Liste nicht enthalten

trust-vid_zero = Angabe unplausibel: Die Hersteller-ID ist 0.
trust-reserved_bits = Angabe unplausibel: Das Kabel setzt reservierte Bits.
trust-cable_vdo_missing = Angabe unvollständig: Das Kabel meldet keine Kabel-Eigenschaften.
trust-epr_without_5a = Angabe unplausibel: 240-W-Betrieb (EPR) gemeldet, aber keine 5 A.
trust-epr_without_50v = Angabe unplausibel: 240-W-Betrieb (EPR) gemeldet, aber keine 50 V.
trust-usb4_flag_on_usb2 = Angabe unplausibel: USB4 gemeldet, aber nur USB 2.0 als Höchsttempo.
trust-link_below_marker = Das Kabel meldet mehr Tempo, als die Verbindung erreicht, obwohl Port und Gerät mehr könnten.

## Laden (Abschnitt 5.4)

power-contract = Vertrag: { $watts } W
power-contract-detail = Vertrag: { $watts } W ({ $volts } V / { $amps } A)
power-basic = Basisangabe: bis zu { $watts } W
power-no-contract = Kein PD-Vertrag sichtbar
power-pdo-fixed = Fest
power-pdo-pps = PPS
power-pdo-spr_avs = SPR AVS
power-pdo-epr_avs = EPR AVS
power-pdo-battery = Batterie
power-pdo-variable = Variabel
power-pdo-unknown = Unbekannt

## Störungen (Abschnitt 5.5)

fault-overcurrent = Überstrom an diesem Port ({ $count }×). Gerät und Kabel prüfen.
fault-drop_reconnect = Die Verbindung ist abgerissen und kam wieder ({ $count }×).

## Leerzustände (Abschnitt 5.10)

unavailable-power = Lade-Angaben an diesem Port nicht verfügbar
unavailable-cable = Kabeldaten an diesem Port nicht verfügbar
unavailable-field = Auf diesem System nicht verfügbar
unavailable-needs_service = Erweiterte Kabeldaten benötigen den LinkSpec-Dienst
reason-platform_not_exposed = das Betriebssystem stellt den Wert nicht bereit
reason-needs_service = benötigt den LinkSpec-Dienst
reason-firmware_not_reported = Treiber oder Firmware melden den Wert nicht
reason-behind_internal_hub = der Port hängt an einem internen Hub
reason-usb_probe_off = USB-Tiefenabfrage aus
reason-permission_denied = fehlende Leserechte

note-usb_probe_off = USB-Tiefenabfrage aus
note-emarker_is_self_reported = Die Kabelangaben stammen aus dem E-Marker des Kabels. LinkSpec ist kein Kabeltester.

notice-service_not_installed = Erweiterte Kabeldaten benötigen den LinkSpec-Dienst
notice-usb_probe_off = USB-Tiefenabfrage aus

## Kommandozeile

cli-cable = Kabel
cli-power = Laden
cli-devices = Geräte
cli-link = Verbindung
cli-verdict-power = Laden
cli-verdict-data = Daten
cli-verdict-display = Display
cli-no-ports = Keine USB-C- oder Thunderbolt-Ports gefunden.
cli-unsupported-intel-mac = LinkSpec unterstützt nur Macs mit Apple-Chip. Auf Intel-Macs stellt macOS die Daten der USB-C-Controller nicht bereit.
cli-unsupported = Diese Plattform wird nicht unterstützt: { $reason }
cli-watch-hint = Beobachte Änderungen … (Strg+C beendet)
cli-monitor-header = Leistung (W / V / A)
cli-monitor-none = Auf diesem System liefert keine Quelle Live-Leistungsdaten.
cli-report-opened = Der Bericht ist im Browser geöffnet. Gesendet wird erst, wenn Sie ihn auf GitHub absenden.
cli-report-no-emarker = Kein Kabel mit E-Marker gefunden. Nur E-Marker-Kabel lassen sich melden.
cli-report-url = Bitte diese Adresse im Browser öffnen:
cli-capture-written = Capture geschrieben: { $path }
cli-app-missing = Die App (linkspec-app) wurde nicht gefunden.
cli-unavailable = Nicht verfügbar
cli-limiter = Engpass
cli-limiter-port = Port
cli-limiter-cable = Kabel
cli-limiter-charger = Netzteil
cli-limiter-device = Endgerät
cli-limiter-none = keiner
cli-limiter-unknown = unklar
cli-fault-banner = Störung
cli-history = Verlauf
cli-history-seen = { $uses }× genutzt, seit { $since }, { $faults } Störungen
