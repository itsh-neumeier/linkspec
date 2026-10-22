//! macOS interpreter against a synthetic registry image (H-M1 … H-M8).
//! Runs on every CI runner. The image follows the hypotheses documented in
//! `src/registry.rs`; real captures replace it once verified on hardware.

use std::path::Path;

use linkspec_backend_macos::registry::{CgDisplay, MacHost, RegEntry, RegistryDump, interpret};
use linkspec_core::observation::*;
use linkspec_core::types::Transport;
use serde_json::{Map, Value, json};

fn props(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}

fn e(id: u64, parent: Option<u64>, class: &str, name: &str, p: Value) -> RegEntry {
    RegEntry {
        id,
        parent,
        class: class.into(),
        name: name.into(),
        props: props(p),
    }
}

pub fn synthetic() -> RegistryDump {
    RegistryDump {
        entries: vec![
            e(1, None, "IORegistryEntry", "Root", json!({})),
            // Port 1: Thunderbolt dock with USB 3 hub and DP.
            e(
                2,
                Some(1),
                "AppleHPMInterfaceType10",
                "Port-USB-C@1",
                json!({
                    "PortNumber": 1,
                    "ConnectionActive": true,
                    "TransportsSupported": ["CC", "USB2", "USB3", "CIO", "DisplayPort"],
                    "TransportsActive": ["CC", "USB2", "USB3", "DisplayPort"],
                    "Plug Orientation": 2
                }),
            ),
            e(
                3,
                Some(2),
                "AppleHPMDevice",
                "pd",
                json!({
                    "SOP' Discover Identity": [0x1800_1234u32, 0, 0x5678_0001u32, 0x0000_2042u32]
                }),
            ),
            // Port 2: empty.
            e(
                4,
                Some(1),
                "AppleHPMInterfaceType10",
                "Port-USB-C@2",
                json!({
                    "PortNumber": 2,
                    "ConnectionActive": false,
                    "TransportsSupported": ["CC", "USB2", "USB3", "CIO", "DisplayPort"],
                    "TransportsActive": []
                }),
            ),
            // XHCI for port 1 with a USB 3 hub and a USB 2 disk behind it.
            e(10, Some(1), "AppleT8122USBXHCI", "usb-drd0", json!({})),
            e(
                11,
                Some(10),
                "IOUSBHostDevice",
                "Hub",
                json!({
                    "idVendor": 0x0bda, "idProduct": 0x0411, "Device Speed": 3,
                    "bDeviceClass": 9, "USB Product Name": "USB3.0 Hub", "bcdUSB": 0x0320
                }),
            ),
            e(
                12,
                Some(11),
                "IOUSBHostDevice",
                "Disk",
                json!({
                    "idVendor": 0x0781, "idProduct": 0x5581, "Device Speed": 2,
                    "USB Product Name": "Flash Disk", "bcdUSB": 0x0200
                }),
            ),
            // Battery with charger details and telemetry.
            e(
                20,
                Some(1),
                "AppleSmartBattery",
                "AppleSmartBattery",
                json!({
                    "ExternalConnected": true, "IsCharging": true, "FullyCharged": false,
                    "CurrentCapacity": 64, "MaxCapacity": 100, "Voltage": 12900, "Amperage": 2100,
                    "AdapterDetails": {
                        "Watts": 96,
                        "UsbHvcMenu": [
                            {"Index": 0, "MaxVoltage": 5000, "MaxCurrent": 3000},
                            {"Index": 1, "MaxVoltage": 9000, "MaxCurrent": 3000},
                            {"Index": 2, "MaxVoltage": 15000, "MaxCurrent": 3000},
                            {"Index": 3, "MaxVoltage": 20000, "MaxCurrent": 4700}
                        ],
                        "UsbHvcHvcIndex": 3
                    },
                    "PowerTelemetryData": {"SystemPowerIn": 61000, "SystemVoltageIn": 20000, "SystemCurrentIn": 3050}
                }),
            ),
        ],
        displays: vec![
            CgDisplay {
                builtin: true,
                current: None,
                max: None,
            },
            CgDisplay {
                builtin: false,
                current: Some(DisplayMode {
                    width: 2560,
                    height: 1440,
                    refresh_hz: 60.0,
                    bits_per_color: None,
                }),
                max: Some(DisplayMode {
                    width: 3840,
                    height: 2160,
                    refresh_hz: 60.0,
                    bits_per_color: None,
                }),
            },
        ],
        host: MacHost {
            model: Some("Mac15,6".into()),
            os_version: Some("15.1".into()),
            arch: "aarch64".into(),
        },
    }
}

#[test]
fn synthetic_registry() {
    let snap = interpret(&synthetic());
    assert_eq!(snap.ports.len(), 2);
    let p1 = &snap.ports[0];
    assert_eq!(p1.id, "macos-port-1");
    assert_eq!(p1.kind, PortKind::Thunderbolt);
    assert_eq!(p1.connected, Some(true));
    assert!(matches!(p1.cable, CableObservation::Emarker { .. }));
    assert!(p1.link.transports.contains(&Transport::Displayport));
    assert_eq!(p1.link.usb_devices.len(), 1);
    assert_eq!(p1.link.usb_devices[0].children.len(), 1);
    assert_eq!(p1.displays.len(), 1);
    let PowerObservation::Available(pd) = &p1.power else {
        panic!("power expected")
    };
    assert_eq!(pd.source_max_w(), Some(94.0));
    assert_eq!(pd.contract.as_ref().unwrap().pdo_index, Some(4));
    assert_eq!(pd.measured_w, Some(61.0));
    assert_eq!(snap.ports[1].connected, Some(false));
    let b = snap.battery.unwrap();
    assert_eq!(b.percent, Some(64.0));
    assert_eq!(b.charging, Some(true));

    // Write the synthetic capture as a fixture (replayed below).
    let out = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/macos/synthetic_tb_port_dock.json");
    if std::env::var_os("LINKSPEC_UPDATE_FIXTURES").is_some() || !out.exists() {
        let doc = json!({
            "schema": linkspec_platform::CAPTURE_SCHEMA,
            "backend": "macos",
            "anonymized": true,
            "synthetic": true,
            "host": snap.host,
            "data": synthetic(),
        });
        std::fs::create_dir_all(out.parent().unwrap()).unwrap();
        std::fs::write(&out, serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
    }
}

#[test]
fn replay_macos_fixture() {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/macos/synthetic_tb_port_dock.json");
    let Ok(text) = std::fs::read_to_string(&p) else {
        return; // written by `synthetic_registry` on the first run
    };
    let doc: Value = serde_json::from_str(&text).unwrap();
    let snap = linkspec_backend_macos::snapshot_from_capture(&doc).unwrap();
    assert_eq!(snap, interpret(&synthetic()));
}

#[test]
fn two_connected_ports_do_not_guess_the_charger_port() {
    let mut d = synthetic();
    for e in &mut d.entries {
        if e.name == "Port-USB-C@2" {
            e.props.insert("ConnectionActive".into(), Value::Bool(true));
        }
    }
    let snap = interpret(&d);
    for p in &snap.ports {
        assert!(
            matches!(p.power, PowerObservation::Unavailable { .. }),
            "{:?}",
            p.power
        );
    }
}
