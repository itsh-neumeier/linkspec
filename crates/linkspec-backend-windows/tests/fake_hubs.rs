//! Windows collector and interpreter against a fake hub tree. Runs on every
//! platform. Also writes `fixtures/windows/*.json`.

use std::collections::BTreeMap;
use std::path::Path;

use linkspec_backend_windows::collect::{HubApi, Req, collect_hubs};
use linkspec_backend_windows::ioctl::build;
use linkspec_backend_windows::model::*;
use linkspec_backend_windows::interpret;
use linkspec_core::observation::*;
use linkspec_core::types::UsbGen;

/// hub path → (hub type, port → buffers)
#[derive(Default)]
struct FakeHubs {
    hubs: BTreeMap<String, (u32, BTreeMap<u32, FakePort>)>,
    locations: BTreeMap<String, String>,
    active: u64,
}

#[derive(Default, Clone)]
struct FakePort {
    connector: Vec<u8>,
    conn: Option<Vec<u8>>,
    v2: Option<Vec<u8>>,
    ssp: Option<Vec<u8>>,
    child: Option<String>,
    desc: Option<String>,
}

impl HubApi for FakeHubs {
    fn hubs(&mut self) -> Vec<String> {
        self.hubs.keys().cloned().collect()
    }
    fn ioctl(&mut self, hub: &str, req: Req, port: u32) -> Option<Vec<u8>> {
        let (t, ports) = self.hubs.get(hub)?;
        if req == Req::HubInfoEx {
            return Some(build::hub_info_ex(*t, ports.len() as u16));
        }
        let p = ports.get(&port)?;
        match req {
            Req::ConnectorProps => Some(p.connector.clone()),
            Req::ConnInfoEx => Some(
                p.conn
                    .clone()
                    .unwrap_or_else(|| build::conn_info_ex(port, 0, 0, false, 0, 0, 0, 0)),
            ),
            Req::ConnInfoV2 => p.v2.clone(),
            Req::SspInfo => p.ssp.clone(),
            Req::HubInfoEx => unreachable!(),
        }
    }
    fn child_hub(&mut self, hub: &str, port: u32) -> Option<String> {
        self.hubs.get(hub)?.1.get(&port)?.child.clone()
    }
    fn hub_location(&mut self, hub: &str) -> Option<String> {
        self.locations.get(hub).cloned()
    }
    fn device_text(&mut self, hub: &str, port: u32) -> (Option<String>, Option<String>) {
        (
            self.hubs.get(hub).and_then(|h| h.1.get(&port)).and_then(|p| p.desc.clone()),
            None,
        )
    }
    fn descriptor(&mut self, _hub: &str, _port: u32, dtype: u8, _index: u8) -> Option<Vec<u8>> {
        self.active += 1;
        match dtype {
            0x0f => Some(vec![5, 0x0f, 15, 0, 1, 10, 0x10, 0x03, 0, 0, 0, 0, 0, 0, 0]),
            _ => None,
        }
    }
}

const SS: &str = r"\\?\USB#ROOT_HUB30#4&aaaa&0&0#{f18a0e88-c30c-11d0-8815-00a0c906bed8}";
const HS: &str = r"\\?\USB#ROOT_HUB20#4&bbbb&0&0#{f18a0e88-c30c-11d0-8815-00a0c906bed8}";
const INT: &str = r"\\?\USB#VID_8087&PID_0029#SERIAL123#{f18a0e88-c30c-11d0-8815-00a0c906bed8}";

/// Laptop: two USB-C ports on xHCI (USB 3 root hub + USB 2 companion).
/// Port 1: USB 3.2 Gen 2 SSD at 10 Gbit/s. Port 2: USB 2.0 stick.
/// Port 3 (hardwired): internal hub with a front USB-C port.
fn fake() -> FakeHubs {
    let mut f = FakeHubs::default();
    let mut ss = BTreeMap::new();
    let mut hs = BTreeMap::new();
    ss.insert(
        1,
        FakePort {
            connector: build::connector_props(1, true, true, 1, Some(HS)),
            conn: Some(build::conn_info_ex(1, 1, 3, false, 0x0320, 0x0781, 0x55ae, 0)),
            v2: Some(build::conn_info_v2(1, 0b100, 0b1101)),
            ssp: Some(build::ssp_info(1, 10, 1)),
            desc: Some("Extreme SSD".into()),
            child: None,
        },
    );
    hs.insert(
        1,
        FakePort {
            connector: build::connector_props(1, true, true, 1, Some(SS)),
            ..Default::default()
        },
    );
    ss.insert(
        2,
        FakePort {
            connector: build::connector_props(2, true, true, 2, Some(HS)),
            ..Default::default()
        },
    );
    hs.insert(
        2,
        FakePort {
            connector: build::connector_props(2, true, true, 2, Some(SS)),
            conn: Some(build::conn_info_ex(2, 1, 2, false, 0x0200, 0x090c, 0x1000, 0)),
            v2: Some(build::conn_info_v2(2, 0b010, 0)),
            desc: Some("USB Stick".into()),
            ..Default::default()
        },
    );
    // Hardwired internal hub on HS port 3.
    hs.insert(
        3,
        FakePort {
            connector: build::connector_props(3, false, false, 0, None),
            conn: Some(build::conn_info_ex(3, 1, 2, true, 0x0210, 0x8087, 0x0029, 9)),
            v2: Some(build::conn_info_v2(3, 0b010, 0)),
            child: Some(INT.into()),
            desc: Some("Generic USB Hub".into()),
            ..Default::default()
        },
    );
    let mut int = BTreeMap::new();
    int.insert(
        1,
        FakePort {
            connector: build::connector_props(1, true, true, 0, None),
            conn: Some(build::conn_info_ex(1, 1, 2, false, 0x0210, 0x1234, 0x0001, 0)),
            v2: Some(build::conn_info_v2(1, 0b010, 0)),
            desc: Some("Front device".into()),
            ..Default::default()
        },
    );
    f.hubs.insert(SS.into(), (1, ss));
    f.hubs.insert(HS.into(), (1, hs));
    f.hubs.insert(INT.into(), (2, int));
    f.locations.insert(SS.into(), "PCIROOT(0)#PCI(1400)#USBROOT(0)".into());
    f.locations.insert(HS.into(), "PCIROOT(0)#PCI(1400)#USBROOT(0)".into());
    f.locations.insert(INT.into(), "PCIROOT(0)#PCI(1400)#USBROOT(0)#USB(3)".into());
    f
}

fn dump(probe: bool) -> (WinDump, u64) {
    let mut api = fake();
    let (hubs, active) = collect_hubs(&mut api, probe);
    assert_eq!(active, api.active, "counter matches real active calls");
    (
        WinDump {
            hubs,
            batteries: vec![BatteryDump {
                power_state: 1 | 4,
                capacity_mwh: Some(40000),
                full_mwh: Some(50000),
                voltage_mv: Some(12000),
                rate_mw: Some(25000),
            }],
            displays: vec![],
            usb4_routers: vec![],
            service: ServiceState::NotInstalled,
            host: WinHost {
                os_version: Some("10.0.26100".into()),
                model: None,
                arch: "x86_64".into(),
            },
        },
        active,
    )
}

#[test]
fn laptop_hub_tree() {
    let (d, active) = dump(true);
    assert!(active > 0);
    // No symbolic links (they contain serials) in the dump.
    let text = serde_json::to_string(&d).unwrap();
    assert!(!text.contains("SERIAL123"));
    assert!(!text.contains("ROOT_HUB"));

    let s = interpret(&d);
    assert_eq!(s.tier, Tier::User);
    assert!(s.notices.contains(&Notice::ServiceNotInstalled));
    assert_eq!(s.ports.len(), 3, "{:#?}", s.ports.iter().map(|p| &p.id).collect::<Vec<_>>());
    let p1 = &s.ports[0];
    assert_eq!(p1.link.usb_devices.len(), 1);
    assert_eq!(p1.link.usb_devices[0].speed_mbps, 10000.0);
    assert_eq!(p1.link.usb_devices[0].max_capable, Some(UsbGen::Gen1));
    assert_eq!(p1.capabilities.usb, Some(UsbGen::Gen2));
    assert!(p1.capabilities.usb_is_lower_bound);
    assert!(matches!(p1.cable, CableObservation::Unavailable { reason: Reason::NeedsService }));
    let p2 = &s.ports[1];
    assert_eq!(p2.link.usb_devices[0].speed_mbps, 480.0);
    let front = s.ports.iter().find(|p| p.behind_internal_hub).expect("front port");
    assert!(matches!(front.cable, CableObservation::Unavailable { reason: Reason::BehindInternalHub }));
    let b = s.battery.unwrap();
    assert_eq!(b.percent, Some(80.0));
    assert_eq!(b.charging, Some(true));

    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/windows/laptop_tier1.json");
    if std::env::var_os("LINKSPEC_UPDATE_FIXTURES").is_some() || !out.exists() {
        let doc = serde_json::json!({
            "schema": linkspec_platform::CAPTURE_SCHEMA,
            "backend": "windows",
            "anonymized": true,
            "synthetic": true,
            "host": s.host,
            "data": d,
        });
        std::fs::create_dir_all(out.parent().unwrap()).unwrap();
        std::fs::write(&out, serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
    }
}

/// Criterion 19 on the real collector: deep probe off → zero active requests.
#[test]
fn probe_off_no_active_requests() {
    let (_, active) = dump(false);
    assert_eq!(active, 0);
}

#[test]
fn service_data_fills_power_and_cable() {
    let (mut d, _) = dump(false);
    d.service = ServiceState::Running(ServiceData {
        version: "test".into(),
        providers: vec!["Microsoft-Windows-USB-UCMUCSICX".into()],
        connectors: vec![ServiceConnector {
            index: 1,
            connected: Some(true),
            partner_identity: None,
            cable_identity: Some(linkspec_core::pd::RawIdentity {
                id_header: 0x1800_1234,
                cert_stat: 0,
                product: 0x5678_0001,
                product_type_vdos: vec![0x0000_2042],
            }),
            source_pdos: vec![(100 << 10) | 300, (400 << 10) | 500],
            sink_pdos: vec![],
            rdo: Some((2 << 28) | (300 << 10) | 300),
            power_role: Some("sink".into()),
        }],
    });
    let s = interpret(&d);
    assert_eq!(s.tier, Tier::Service);
    let p1 = &s.ports[0];
    assert!(matches!(p1.cable, CableObservation::Emarker { .. }));
    let PowerObservation::Available(pd) = &p1.power else { panic!() };
    assert_eq!(pd.source_max_w(), Some(100.0));
    assert_eq!(pd.contract.as_ref().unwrap().power_w, 60.0);
}

#[test]
fn replay_windows_fixture() {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/windows/laptop_tier1.json");
    let Ok(text) = std::fs::read_to_string(p) else { return };
    let doc: serde_json::Value = serde_json::from_str(&text).unwrap();
    let s = linkspec_backend_windows::snapshot_from_capture(&doc).unwrap();
    assert_eq!(s.ports.len(), 3);
}
