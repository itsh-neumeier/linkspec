//! Criterion 11: overcurrent and drop/reconnect produce banners in the model
//! (the history entry is tested in `linkspec-history`).

use linkspec_core::observation::{
    FaultKind, HealthObservation, HostInfo, LinkObservation, Os, PortObservation, Snapshot,
    UsbDeviceObservation,
};
use linkspec_core::session::SessionTracker;
use linkspec_core::types::Source;
use linkspec_core::{Context, EmptyCatalog, I18n, Lang, diagnose};

fn base(connected: bool, data: bool, oc: u32) -> Snapshot {
    let mut p = PortObservation::new("port-1", 1);
    p.connected = Some(connected);
    p.health = Some(HealthObservation {
        overcurrent_count: Some(oc),
        ..Default::default()
    });
    if data {
        p.link = LinkObservation {
            usb_devices: vec![UsbDeviceObservation {
                vendor_id: None,
                product_id: None,
                vendor: None,
                product: Some("Disk".into()),
                speed_mbps: 5000.0,
                bcd_usb: Some(0x0320),
                max_capable: None,
                is_hub: false,
                children: vec![],
                source: Source::Fixture,
            }],
            source: Some(Source::Fixture),
            ..Default::default()
        };
    }
    Snapshot {
        host: HostInfo {
            os: Os::Linux,
            os_version: None,
            model: None,
            arch: "x86_64".into(),
        },
        tier: Default::default(),
        usb_probe: true,
        battery: None,
        ports: vec![p],
        notices: vec![],
    }
}

#[test]
fn drop_reconnect_and_overcurrent_raise_banners() {
    let mut t = SessionTracker::new();
    let mut s1 = base(true, true, 0);
    assert!(t.update(&mut s1, "2026-10-20T12:00:00Z").is_empty());
    // Data goes away while the partner stays.
    let mut s2 = base(true, false, 0);
    assert!(t.update(&mut s2, "2026-10-20T12:00:01Z").is_empty());
    // Data comes back → drop/reconnect.
    let mut s3 = base(true, true, 0);
    let ev = t.update(&mut s3, "2026-10-20T12:00:02Z");
    assert_eq!(ev.len(), 1);
    assert_eq!(ev[0].kind, FaultKind::DropReconnect);
    assert_eq!(s3.ports[0].faults.len(), 1);
    // Overcurrent counter increases.
    let mut s4 = base(true, true, 1);
    let ev = t.update(&mut s4, "2026-10-20T12:00:03Z");
    assert_eq!(ev.len(), 1);
    assert_eq!(ev[0].kind, FaultKind::Overcurrent);
    assert_eq!(s4.ports[0].faults.len(), 2);

    // The banner is in the diagnosis model and sets the fault status.
    let i18n = I18n::new(Lang::De);
    let d = diagnose(
        &s4,
        &Context {
            i18n: &i18n,
            catalog: &EmptyCatalog,
            history: None,
            include_raw: false,
            app_version: "test",
            generated_at: "2026-10-20T12:00:03Z".into(),
        },
    );
    let port = &d.ports[0];
    assert_eq!(port.status, linkspec_core::diagnosis::Status::Fault);
    assert!(port.faults.iter().all(|f| f.banner));
    assert!(port.faults.iter().any(|f| f.summary.contains("Überstrom")));

    // Banner stays on the next snapshot ...
    let mut s5 = base(true, true, 1);
    t.update(&mut s5, "2026-10-20T12:00:04Z");
    assert_eq!(s5.ports[0].faults.len(), 2);
    // ... until dismissed ...
    t.dismiss("port-1", FaultKind::Overcurrent);
    let mut s6 = base(true, true, 1);
    t.update(&mut s6, "2026-10-20T12:00:05Z");
    assert_eq!(s6.ports[0].faults.len(), 1);
    // ... or the cable is pulled.
    let mut s7 = base(false, false, 1);
    t.update(&mut s7, "2026-10-20T12:00:06Z");
    assert!(s7.ports[0].faults.is_empty());
}

#[test]
fn unplug_and_replug_is_not_a_drop() {
    let mut t = SessionTracker::new();
    let mut s1 = base(true, true, 0);
    t.update(&mut s1, "t1");
    let mut s2 = base(false, false, 0);
    t.update(&mut s2, "t2");
    let mut s3 = base(true, true, 0);
    assert!(t.update(&mut s3, "t3").is_empty());
    assert!(s3.ports[0].faults.is_empty());
}
