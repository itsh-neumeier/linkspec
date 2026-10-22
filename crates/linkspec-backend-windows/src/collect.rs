//! Hub walk, generic over [`HubApi`]. The Win32 implementation lives in
//! `win32.rs`; tests use a fake. Passive IOCTLs read what the hub driver
//! already knows. Active device requests (string and BOS descriptors) only
//! run with deep probe and are counted.

use std::collections::BTreeMap;

use crate::ioctl::{
    self, CONN_INFO_EX_SIZE, CONN_INFO_V2_SIZE, SSP_INFO_SIZE, parse_conn_info_ex,
    parse_conn_info_v2, parse_connector_props, parse_hub_info_ex, parse_ssp_info,
};
use crate::model::{HubDump, HubPortDump};

/// Passive hub requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Req {
    HubInfoEx,
    ConnectorProps,
    ConnInfoEx,
    ConnInfoV2,
    SspInfo,
}

impl Req {
    pub fn min_size(self) -> usize {
        match self {
            Req::HubInfoEx => 8,
            Req::ConnectorProps => 16,
            Req::ConnInfoEx => CONN_INFO_EX_SIZE,
            Req::ConnInfoV2 => CONN_INFO_V2_SIZE,
            Req::SspInfo => SSP_INFO_SIZE,
        }
    }
}

/// Access to USB hubs. `hub` is the hub's device interface path.
pub trait HubApi {
    /// Device interface paths of all hubs (root and external).
    fn hubs(&mut self) -> Vec<String>;
    fn ioctl(&mut self, hub: &str, req: Req, port: u32) -> Option<Vec<u8>>;
    /// Interface path of the hub attached to a port.
    fn child_hub(&mut self, hub: &str, port: u32) -> Option<String>;
    /// Location path of a hub.
    fn hub_location(&mut self, hub: &str) -> Option<String>;
    /// Passive device texts: (description, manufacturer).
    fn device_text(&mut self, hub: &str, port: u32) -> (Option<String>, Option<String>);
    /// Active: GET_DESCRIPTOR through the hub (deep probe).
    fn descriptor(&mut self, hub: &str, port: u32, dtype: u8, index: u8) -> Option<Vec<u8>>;
}

/// Normalize interface paths for comparison (Windows paths are
/// case-insensitive and sometimes differ in the `\\?\` / `\\.\` prefix).
pub fn norm(path: &str) -> String {
    path.trim_start_matches("\\\\?\\")
        .trim_start_matches("\\\\.\\")
        .to_ascii_lowercase()
}

/// Walk all hubs. Returns the dumps and the number of active requests.
pub fn collect_hubs(api: &mut dyn HubApi, usb_probe: bool) -> (Vec<HubDump>, u64) {
    let mut paths = api.hubs();
    // Deterministic keys: sort by location path, then by interface path.
    let mut with_loc: Vec<(Option<String>, String)> = paths
        .drain(..)
        .map(|p| (api.hub_location(&p), p))
        .collect();
    with_loc.sort();
    let keys: BTreeMap<String, String> = with_loc
        .iter()
        .enumerate()
        .map(|(i, (_, p))| (norm(p), format!("hub-{}", i + 1)))
        .collect();
    let key_of = |p: &str| keys.get(&norm(p)).cloned();
    let mut active = 0u64;
    let mut out = Vec::new();
    for (loc, path) in &with_loc {
        let Some(info) = api
            .ioctl(path, Req::HubInfoEx, 0)
            .and_then(|b| parse_hub_info_ex(&b))
        else {
            continue;
        };
        let mut ports = Vec::new();
        for port in 1..=u32::from(info.highest_port) {
            let connector = api
                .ioctl(path, Req::ConnectorProps, port)
                .and_then(|b| parse_connector_props(&b));
            let companion_hub = connector
                .as_ref()
                .and_then(|c| c.companion_hub_link.as_deref())
                .and_then(key_of);
            let conn = api
                .ioctl(path, Req::ConnInfoEx, port)
                .and_then(|b| parse_conn_info_ex(&b));
            let present = conn.as_ref().is_some_and(|c| c.status.device_present());
            let (mut v2, mut ssp) = (None, None);
            let (mut description, mut manufacturer) = (None, None);
            let (mut product_string, mut bos_max, mut child_hub) = (None, None, None);
            if present {
                v2 = api
                    .ioctl(path, Req::ConnInfoV2, port)
                    .and_then(|b| parse_conn_info_v2(&b));
                if v2.is_some_and(|v| v.operating_at_ssp) {
                    ssp = api
                        .ioctl(path, Req::SspInfo, port)
                        .and_then(|b| parse_ssp_info(&b));
                }
                (description, manufacturer) = api.device_text(path, port);
                if conn.as_ref().is_some_and(|c| c.is_hub) {
                    child_hub = api.child_hub(path, port).and_then(|c| key_of(&c));
                }
                if usb_probe {
                    let c = conn.as_ref().expect("present implies conn");
                    if c.i_product != 0 {
                        active += 1;
                        product_string = api
                            .descriptor(path, port, 0x03, c.i_product)
                            .and_then(|b| ioctl::parse_string_descriptor(&b));
                    }
                    if c.bcd_usb >= 0x0201 {
                        active += 1;
                        bos_max = api
                            .descriptor(path, port, 0x0f, 0)
                            .and_then(|b| ioctl::bos_max_class(&b));
                    }
                }
            }
            ports.push(HubPortDump {
                port,
                connector,
                companion_hub,
                conn,
                v2,
                ssp,
                description,
                manufacturer,
                product_string,
                bos_max,
                child_hub,
            });
        }
        out.push(HubDump {
            key: key_of(path).unwrap_or_default(),
            location: loc.clone(),
            info,
            ports,
        });
    }
    (out, active)
}
