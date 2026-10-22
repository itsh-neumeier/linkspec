# LinkSpec – English strings
# German (de.ftl) is the reference language. Keep ids in sync.

## Headlines (section 5.1)

headline-thunderbolt_usb4 = Thunderbolt / USB4
headline-usb_device = USB device
headline-display = Display connected
headline-charging_only = Charging only
headline-slow_or_charge_only = Slow USB or charge-only cable
headline-empty = Nothing connected

summary-thunderbolt_usb4 = Thunderbolt or USB4 connection active.
summary-usb_device = USB device connected.
summary-display = A display is connected.
summary-charging_only = This port only charges.
summary-slow_or_charge_only = No fast data connection detected.
summary-empty = Nothing connected

## Port names and location

port-label-usb_c = USB-C { $index }
port-label-usb4 = USB4 { $index }
port-label-thunderbolt = Thunderbolt { $index }
location-panel-left = left side
location-panel-right = right side
location-panel-front = front
location-panel-back = back
location-panel-top = top
location-panel-bottom = bottom
location-horizontal-left = left
location-horizontal-center = centre
location-horizontal-right = right
location-vertical-upper = upper
location-vertical-center = middle
location-vertical-lower = lower

## Speeds

usb-class-usb2 = USB 2.0
usb-class-gen1 = USB 3 (5 Gbit/s)
usb-class-gen2 = USB 3 (10 Gbit/s)
usb-class-gen3 = USB4 (40 Gbit/s)
usb-class-gen4 = USB4 (80 Gbit/s)
speed-gbps = { $gbps } Gbit/s
speed-fabric = Thunderbolt/USB4

## Verdicts (section 5.2)

verdict-power_ok = Charging at { $watts } W.
verdict-charging_below_max = Charging at { $watts } W (charger can deliver up to { $max } W).
verdict-battery_full = Battery full, not charging.
verdict-cable_limits_power = The cable limits power to { $watts } W. Charger and computer could do { $other } W.
verdict-charger_limits_power = The charger delivers at most { $watts } W. Cable and computer could take more.
verdict-host_limits_power = The computer takes at most { $watts } W on this port. Cable and charger could deliver more.
verdict-power_basic_only = Only the basic level is visible: USB-C with up to { $watts } W. The system does not report charger details.
verdict-power_supplying_device = This port supplies { $watts } W to the device.
verdict-power_unknown = The charging data is not sufficient for a verdict.
verdict-data_ok = Data runs at { $speed }, as fast as possible.
verdict-device_at_its_max =
    { $cable ->
        [ok] The device is at its limit with { $speed }; the cable is fine.
       *[other] The device is at its limit with { $speed }.
    }
verdict-cable_limits_data = The cable limits data to { $speed }. Port and device could do more.
verdict-cable_likely_usb2 = Probably a USB 2.0 or charge-only cable: port and device could go faster.
verdict-port_limits_data = The port limits data to { $speed }. Device and cable could do more.
verdict-emarker_link_mismatch = Marker and connection do not match: the cable reports { $marker }, the connection runs at { $speed }.
verdict-no_data_link = No data connection: the cable may have no data wires, or the device does not transfer data.
verdict-data_via_internal_hub = Data runs at { $speed } through an internal hub. Cable data is not available on this port.
verdict-data_unknown = Data runs at { $speed }. It is not certain what limits the connection.
verdict-fabric_link_below_cable = The connection runs at only { $link } Gbit/s although the cable reports { $marker } Gbit/s and port and device can do more. The cable is the bottleneck.
verdict-fabric_link_unverified = The connection runs at { $link } Gbit/s, the cable reports { $marker } Gbit/s. Without a controller measurement the cause is unclear.
verdict-display_at_max = The display runs at its highest mode.
verdict-display_below_max = The display runs below its highest mode although the connection could carry more. Check the display settings.
verdict-display_link_too_slow = The display runs below its highest mode: that mode needs { $need } Gbit/s, the DisplayPort link carries { $cap } Gbit/s.
verdict-display_lanes_shared = The display runs below its highest mode: the dock shares the lanes between USB 3 and DisplayPort.
verdict-display_unknown = Display data is missing; the mode cannot be judged.

lead-device_at_its_max =
    { $cable ->
        [ok] The device uses { $speed }. The cable can do more.
       *[other] The device uses { $speed } and is at its limit.
    }

## Cable (section 5.3)

cable-no-emarker = This cable reports no identity
cable-kind-passive = Passive
cable-kind-active = Active
cable-kind-optical = Optical
cable-kind-unknown = Type unknown
cable-speed = { $gbps } Gbit/s
cable-power = { $amps } A / { $watts } W
cable-certification-listed = Certified: { $name }
cable-certification-unlisted = Certificate reported, not in our list
cable-xid-no-list = Certificate reported (XID { $xid }); list not included

trust-vid_zero = Implausible: the vendor ID is 0.
trust-reserved_bits = Implausible: the cable sets reserved bits.
trust-cable_vdo_missing = Incomplete: the cable reports no cable properties.
trust-epr_without_5a = Implausible: 240 W operation (EPR) reported, but not 5 A.
trust-epr_without_50v = Implausible: 240 W operation (EPR) reported, but not 50 V.
trust-usb4_flag_on_usb2 = Implausible: USB4 reported, but USB 2.0 as highest speed.
trust-link_below_marker = The cable reports more speed than the connection reaches, although port and device could do more.

## Power (section 5.4)

power-contract = Contract: { $watts } W
power-contract-detail = Contract: { $watts } W ({ $volts } V / { $amps } A)
power-basic = Basic level: up to { $watts } W
power-no-contract = No PD contract visible
power-pdo-fixed = Fixed
power-pdo-pps = PPS
power-pdo-spr_avs = SPR AVS
power-pdo-epr_avs = EPR AVS
power-pdo-battery = Battery
power-pdo-variable = Variable
power-pdo-unknown = Unknown

## Faults (section 5.5)

fault-overcurrent = Overcurrent on this port ({ $count }×). Check device and cable.
fault-drop_reconnect = The connection dropped and came back ({ $count }×).

## Empty states (section 5.10)

unavailable-power = Charging data not available on this port
unavailable-cable = Cable data not available on this port
unavailable-field = Not available on this system
unavailable-needs_service = Extended cable data requires the LinkSpec service
reason-platform_not_exposed = the operating system does not provide the value
reason-needs_service = requires the LinkSpec service
reason-firmware_not_reported = driver or firmware do not report the value
reason-behind_internal_hub = the port is behind an internal hub
reason-usb_probe_off = USB deep probe off
reason-permission_denied = missing read permission

note-usb_probe_off = USB deep probe off
note-emarker_is_self_reported = Cable data comes from the cable's E-Marker. LinkSpec is not a cable tester.

notice-service_not_installed = Extended cable data requires the LinkSpec service
notice-usb_probe_off = USB deep probe off

## Command line

cli-cable = Cable
cli-power = Power
cli-devices = Devices
cli-link = Link
cli-verdict-power = Power
cli-verdict-data = Data
cli-verdict-display = Display
cli-no-ports = No USB-C or Thunderbolt ports found.
cli-unsupported-intel-mac = LinkSpec supports only Macs with Apple silicon. On Intel Macs, macOS does not provide the USB-C controller data.
cli-unsupported = This platform is not supported: { $reason }
cli-watch-hint = Watching for changes … (Ctrl+C to stop)
cli-monitor-header = Power (W / V / A)
cli-monitor-none = No source on this system provides live power data.
cli-report-opened = The report is open in your browser. Nothing is sent until you submit it on GitHub.
cli-report-no-emarker = No cable with an E-Marker found. Only E-Marker cables can be reported.
cli-report-url = Please open this address in your browser:
cli-capture-written = Capture written: { $path }
cli-app-missing = The app (linkspec-app) was not found.
cli-unavailable = Not available
cli-limiter = Bottleneck
cli-limiter-port = port
cli-limiter-cable = cable
cli-limiter-charger = charger
cli-limiter-device = device
cli-limiter-none = none
cli-limiter-unknown = unclear
cli-fault-banner = Fault
cli-history = History
cli-history-seen = used { $uses }×, since { $since }, { $faults } faults
