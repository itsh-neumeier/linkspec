//! Builds a synthetic sysfs tree on disk (files and symlinks like the real
//! kernel layout), runs the real collector and interpreter, and checks the
//! snapshot. The dump is also written as a fixture (`fixtures/linux/`).

use std::fs;
use std::path::Path;

use linkspec_backend_linux::{LinuxSource, dump::SysfsDump, interpret::interpret};
use linkspec_core::observation::*;
use linkspec_core::types::{FabricGen, Transport, UsbGen};
use linkspec_core::{Context, EmptyCatalog, I18n, Lang, diagnose};
use linkspec_platform::anonymize::violations;
use linkspec_platform::{PortSource, ReadOptions};

fn w(root: &Path, rel: &str, val: &str) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, format!("{val}\n")).unwrap();
}

fn ln(root: &Path, rel: &str, target: &str) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, p).unwrap();
    #[cfg(not(unix))]
    {
        // On Windows runners the collector test runs with a JSON dump only.
        let _ = target;
    }
}

fn edid(name: &str, w_: u32, h: u32, hz: u32) -> Vec<u8> {
    let mut e = vec![0u8; 128];
    e[..8].copy_from_slice(&[0, 255, 255, 255, 255, 255, 255, 0]);
    e[20] = 0x80 | (2 << 4);
    let (hb, vb) = (160u32, 41u32);
    let clock_khz = (w_ + hb) * (h + vb) * hz / 1000;
    let c = (clock_khz / 10) as u16;
    let d = &mut e[54..72];
    d[0..2].copy_from_slice(&c.to_le_bytes());
    d[2] = (w_ & 0xff) as u8;
    d[3] = (hb & 0xff) as u8;
    d[4] = (((w_ >> 8) as u8) << 4) | ((hb >> 8) as u8);
    d[5] = (h & 0xff) as u8;
    d[6] = (vb & 0xff) as u8;
    d[7] = (((h >> 8) as u8) << 4) | ((vb >> 8) as u8);
    let n = &mut e[72..90];
    n[3] = 0xfc;
    for (i, b) in name.bytes().enumerate() {
        n[5 + i] = b;
    }
    n[5 + name.len()] = 0x0a;
    e
}

/// A UCSI laptop: port0 with a USB-C dock (charger + USB 3 hub + DP Alt
/// Mode, E-Marker cable), port1 with a USB4 dock, port2 empty.
fn build(root: &Path) {
    // --- Type-C class (UCSI) ---
    for (i, panel) in [(0, "left"), (1, "left"), (2, "right")] {
        let p = format!("class/typec/port{i}");
        w(root, &format!("{p}/data_role"), "[host] device");
        w(root, &format!("{p}/power_role"), "source [sink]");
        w(root, &format!("{p}/port_type"), "dual");
        w(root, &format!("{p}/usb_power_delivery_revision"), "3.0");
        w(root, &format!("{p}/physical_location/panel"), panel);
        w(
            root,
            &format!("{p}/physical_location/horizontal_position"),
            if i == 1 { "right" } else { "left" },
        );
        ln(
            root,
            &format!("{p}/firmware_node"),
            &format!("../../../../LNXSYBUS:00/USBC000:00/CON{i}"),
        );
        // Port altmode capability: DisplayPort.
        w(root, &format!("{p}/port{i}.0/svid"), "ff01");
        ln(
            root,
            &format!("{p}/usb_power_delivery"),
            &format!("../../../usb_power_delivery/pd{}", 10 + i),
        );
        // Host sink capabilities: 5 V 3 A, 20 V 5 A.
        let sink = format!("class/usb_power_delivery/pd{}/sink-capabilities", 10 + i);
        w(root, &format!("{sink}/1:fixed_supply/voltage"), "5000mV");
        w(
            root,
            &format!("{sink}/1:fixed_supply/operational_current"),
            "3000mA",
        );
        w(root, &format!("{sink}/2:fixed_supply/voltage"), "20000mV");
        w(
            root,
            &format!("{sink}/2:fixed_supply/operational_current"),
            "5000mA",
        );
    }
    w(
        root,
        "class/typec/port0/power_operation_mode",
        "usb_power_delivery",
    );
    w(root, "class/typec/port0/orientation", "reverse");
    // Partner on port0 (dock), PD source with 100 W.
    let pp = "class/typec/port0-partner";
    w(root, &format!("{pp}/supports_usb_power_delivery"), "yes");
    w(root, &format!("{pp}/accessory_mode"), "none");
    w(root, &format!("{pp}/identity/id_header"), "0x6c8017ef");
    w(root, &format!("{pp}/identity/cert_stat"), "0x00000000");
    w(root, &format!("{pp}/identity/product"), "0xa3910100");
    w(
        root,
        &format!("{pp}/identity/product_type_vdo1"),
        "0x45000002",
    );
    w(
        root,
        &format!("{pp}/identity/product_type_vdo2"),
        "0x00000000",
    );
    w(
        root,
        &format!("{pp}/identity/product_type_vdo3"),
        "0x00000000",
    );
    w(root, &format!("{pp}/port0-partner.0/svid"), "ff01");
    w(root, &format!("{pp}/port0-partner.0/active"), "yes");
    w(
        root,
        &format!("{pp}/port0-partner.0/displayport/pin_assignment"),
        "C [D]",
    );
    w(
        root,
        &format!("{pp}/port0-partner.0/displayport/configuration"),
        "[source] sink",
    );
    ln(
        root,
        &format!("{pp}/usb_power_delivery"),
        "../../../usb_power_delivery/pd20",
    );
    let src = "class/usb_power_delivery/pd20/source-capabilities";
    for (i, (v, a)) in [(5000, 3000), (9000, 3000), (15000, 3000), (20000, 5000)]
        .iter()
        .enumerate()
    {
        w(
            root,
            &format!("{src}/{}:fixed_supply/voltage", i + 1),
            &format!("{v}mV"),
        );
        w(
            root,
            &format!("{src}/{}:fixed_supply/maximum_current", i + 1),
            &format!("{a}mA"),
        );
    }
    w(
        root,
        &format!("{src}/5:programmable_supply/minimum_voltage"),
        "3300mV",
    );
    w(
        root,
        &format!("{src}/5:programmable_supply/maximum_voltage"),
        "21000mV",
    );
    w(
        root,
        &format!("{src}/5:programmable_supply/maximum_current"),
        "3000mA",
    );
    // E-Marker cable on port0: passive, 5 A, Gen2.
    let cb = "class/typec/port0-cable";
    w(root, &format!("{cb}/type"), "passive");
    w(root, &format!("{cb}/plug_type"), "type-c");
    w(root, &format!("{cb}/identity/id_header"), "0x18001234");
    w(root, &format!("{cb}/identity/cert_stat"), "0x00000000");
    w(root, &format!("{cb}/identity/product"), "0x56780001");
    w(
        root,
        &format!("{cb}/identity/product_type_vdo1"),
        "0x00002042",
    );
    // UCSI power supply for connector 1 (port0): 20 V, 4.5 A.
    let psy = "class/power_supply/ucsi-source-psy-USBC000:001";
    w(root, &format!("{psy}/type"), "USB");
    w(root, &format!("{psy}/online"), "1");
    w(root, &format!("{psy}/usb_type"), "C [PD] PD_PPS");
    w(root, &format!("{psy}/voltage_now"), "20000000");
    w(root, &format!("{psy}/current_now"), "4500000");
    w(root, &format!("{psy}/current_max"), "5000000");
    // Battery.
    w(root, "class/power_supply/BAT0/type", "Battery");
    w(root, "class/power_supply/BAT0/status", "Charging");
    w(root, "class/power_supply/BAT0/capacity", "81");
    w(root, "class/power_supply/BAT0/power_now", "42500000");
    w(
        root,
        "class/power_supply/BAT0/serial_number",
        "BAT-SECRET-1",
    );

    // --- USB: bus 1 (HS) and bus 2 (SS 10G), root ports linked to Type-C ---
    w(root, "bus/usb/devices/usb1/speed", "480");
    w(root, "bus/usb/devices/usb1/version", " 2.00");
    w(root, "bus/usb/devices/usb2/speed", "10000");
    w(root, "bus/usb/devices/usb2/version", " 3.10");
    for (bus, port, conn) in [
        (1, 1, "port0"),
        (2, 1, "port0"),
        (1, 2, "port1"),
        (2, 2, "port1"),
        (1, 3, "port2"),
        (2, 3, "port2"),
    ] {
        ln(
            root,
            &format!("bus/usb/devices/{bus}-0:1.0/usb{bus}-port{port}/connector"),
            &format!("../../../../../../typec/{conn}"),
        );
        w(
            root,
            &format!("bus/usb/devices/{bus}-0:1.0/usb{bus}-port{port}/over_current_count"),
            "0",
        );
        w(
            root,
            &format!("bus/usb/devices/{bus}-0:1.0/usb{bus}-port{port}/connect_type"),
            "hotplug",
        );
    }
    // Dock hub on port0: USB 3 hub (2-1) and USB 2 hub (1-1) with a mouse.
    for (dev, speed, ver) in [("2-1", "5000", " 3.20"), ("1-1", "480", " 2.10")] {
        w(root, &format!("bus/usb/devices/{dev}/speed"), speed);
        w(root, &format!("bus/usb/devices/{dev}/version"), ver);
        w(root, &format!("bus/usb/devices/{dev}/idVendor"), "0bda");
        w(root, &format!("bus/usb/devices/{dev}/idProduct"), "0411");
        w(root, &format!("bus/usb/devices/{dev}/bDeviceClass"), "09");
        w(
            root,
            &format!("bus/usb/devices/{dev}/product"),
            "USB3.0 Hub",
        );
        w(
            root,
            &format!("bus/usb/devices/{dev}/serial"),
            "HUB-SECRET-2",
        );
    }
    w(root, "bus/usb/devices/1-1.2/speed", "12");
    w(root, "bus/usb/devices/1-1.2/version", " 2.00");
    w(root, "bus/usb/devices/1-1.2/idVendor", "046d");
    w(root, "bus/usb/devices/1-1.2/idProduct", "c077");
    w(root, "bus/usb/devices/1-1.2/product", "USB Optical Mouse");
    w(root, "bus/usb/devices/1-1.2/bDeviceClass", "00");

    // --- Thunderbolt/USB4: host router 0-0 (gen 4), usb4_port2 → port1 ---
    w(root, "bus/thunderbolt/devices/0-0/generation", "4");
    w(root, "bus/thunderbolt/devices/0-0/unique_id", "SECRET-UUID");
    w(root, "bus/thunderbolt/devices/0-0/usb4_port2/link", "usb4");
    ln(
        root,
        "bus/thunderbolt/devices/0-0/usb4_port2/connector",
        "../../../../typec/port1",
    );
    w(root, "bus/thunderbolt/devices/0-2/generation", "4");
    w(root, "bus/thunderbolt/devices/0-2/vendor_name", "Example");
    w(root, "bus/thunderbolt/devices/0-2/device_name", "USB4 Dock");
    w(root, "bus/thunderbolt/devices/0-2/rx_speed", "20.0 Gb/s");
    w(root, "bus/thunderbolt/devices/0-2/rx_lanes", "2");
    w(root, "bus/thunderbolt/devices/0-2/tx_speed", "20.0 Gb/s");
    w(root, "bus/thunderbolt/devices/0-2/tx_lanes", "2");
    w(
        root,
        "bus/thunderbolt/devices/0-2/unique_id",
        "SECRET-UUID-2",
    );
    // Partner on port1 (needed for "connected").
    w(
        root,
        "class/typec/port1-partner/supports_usb_power_delivery",
        "yes",
    );
    w(
        root,
        "class/typec/port1-partner/identity/id_header",
        "0x00000000",
    );

    // --- DRM: one external DP monitor ---
    w(root, "class/drm/card1-DP-1/status", "connected");
    w(root, "class/drm/card1-DP-1/enabled", "enabled");
    fs::write(
        root.join("class/drm/card1-DP-1/edid"),
        edid("Example 27", 2560, 1440, 60),
    )
    .unwrap();
    w(root, "class/drm/card1-eDP-1/status", "connected");
}

fn snapshot_of(root: &Path) -> (SysfsDump, Snapshot) {
    let src = LinuxSource::with_roots(root.to_path_buf(), root.join("proc"));
    let dump = src.dump();
    let snap = interpret(&dump);
    (dump, snap)
}

#[cfg(unix)]
#[test]
fn ucsi_laptop_tree() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    build(root);
    let (dump, snap) = snapshot_of(root);

    // Collector never reads serials or unique ids.
    let text = serde_json::to_string(&dump).unwrap();
    for secret in ["SECRET", "serial", "unique_id"] {
        assert!(!text.contains(secret), "dump contains {secret}");
    }

    assert_eq!(snap.ports.len(), 3);
    let p0 = &snap.ports[0];
    assert_eq!(p0.index, 1);
    assert_eq!(p0.connected, Some(true));
    assert_eq!(p0.capabilities.usb, Some(UsbGen::Gen2));
    assert_eq!(p0.capabilities.sink_max_w, Some(100.0));
    assert_eq!(p0.location.as_ref().unwrap().panel.as_deref(), Some("left"));
    assert!(matches!(p0.cable, CableObservation::Emarker { .. }));
    let PowerObservation::Available(pd) = &p0.power else {
        panic!("power")
    };
    assert_eq!(pd.role, Some(PowerRole::Sink));
    assert_eq!(pd.source_pdos.len(), 5);
    assert_eq!(pd.source_max_w(), Some(100.0));
    assert_eq!(pd.contract.as_ref().unwrap().power_w, 90.0);
    assert!(p0.link.transports.contains(&Transport::Displayport));
    assert_eq!(p0.link.displayport.as_ref().unwrap().lanes, Some(2));
    assert_eq!(p0.link.usb_devices.len(), 2);
    let hs_hub = p0
        .link
        .usb_devices
        .iter()
        .find(|d| d.speed_mbps == 480.0)
        .unwrap();
    assert_eq!(hs_hub.children.len(), 1);
    assert_eq!(p0.displays.len(), 1);
    assert_eq!(p0.displays[0].name.as_deref(), Some("Example 27"));
    assert_eq!(
        p0.health.as_ref().unwrap().orientation.as_deref(),
        Some("reverse")
    );

    let p1 = &snap.ports[1];
    assert_eq!(p1.kind, PortKind::Usb4);
    let f = p1.link.fabric.as_ref().unwrap();
    assert_eq!(f.link_gbps(), Some(40.0));
    assert!(f.measured_by_controller);
    assert_eq!(f.generation, Some(FabricGen::Usb4V1));
    assert!(p1.link.transports.contains(&Transport::Usb4));

    let p2 = &snap.ports[2];
    assert_eq!(p2.connected, Some(false));

    let bat = snap.battery.as_ref().unwrap();
    assert_eq!(bat.percent, Some(81.0));
    assert_eq!(bat.charging, Some(true));

    // Full diagnosis: schema-valid output and sensible verdicts.
    let i18n = I18n::new(Lang::De);
    let diag = diagnose(
        &snap,
        &Context {
            i18n: &i18n,
            catalog: &EmptyCatalog,
            history: None,
            include_raw: true,
            app_version: "test",
            generated_at: "2026-10-20T12:00:00Z".into(),
        },
    );
    assert_eq!(diag.ports[0].headline.as_str(), "display");
    assert_eq!(diag.ports[1].headline.as_str(), "thunderbolt_usb4");
    assert_eq!(diag.ports[2].headline.as_str(), "empty");
    let codes: Vec<_> = diag.ports[0]
        .verdicts
        .iter()
        .map(|v| v.code.as_str())
        .collect();
    assert!(
        codes.contains(&"charging_below_max") || codes.contains(&"power_ok"),
        "{codes:?}"
    );

    // Write the dump as a replayable fixture (synthetic).
    let fixture = serde_json::json!({
        "schema": linkspec_platform::CAPTURE_SCHEMA,
        "backend": "linux",
        "anonymized": true,
        "synthetic": true,
        "host": snap.host,
        "data": dump,
    });
    assert!(
        violations(&fixture, &[]).is_empty(),
        "{:?}",
        violations(&fixture, &[])
    );
    let out = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/linux/ucsi_laptop_dock_usb4.json");
    let text = serde_json::to_string_pretty(&fixture).unwrap() + "\n";
    if std::env::var_os("LINKSPEC_UPDATE_FIXTURES").is_some() || !out.exists() {
        fs::create_dir_all(out.parent().unwrap()).unwrap();
        fs::write(&out, text).unwrap();
    }
}

/// Fixture replay on every OS: the stored dump gives the same snapshot.
#[test]
fn replay_linux_fixture() {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/linux/ucsi_laptop_dock_usb4.json");
    let doc: serde_json::Value = serde_json::from_str(&fs::read_to_string(p).unwrap()).unwrap();
    let snap = linkspec_backend_linux::snapshot_from_capture(&doc).unwrap();
    assert_eq!(snap.ports.len(), 3);
    assert!(matches!(
        snap.ports[0].cable,
        CableObservation::Emarker { .. }
    ));
    assert_eq!(
        snap.ports[1].link.fabric.as_ref().unwrap().link_gbps(),
        Some(40.0)
    );
}

/// Backend smoke test ("no crash, ports is an array") on the real machine.
#[test]
fn live_backend_does_not_crash() {
    let mut src = LinuxSource::new();
    match src.snapshot(ReadOptions::default()) {
        Ok(s) => {
            let v = serde_json::to_value(&s).unwrap();
            assert!(v["ports"].is_array());
        }
        Err(linkspec_platform::SourceError::NotReadable(_)) => {}
        Err(e) => panic!("{e}"),
    }
}
