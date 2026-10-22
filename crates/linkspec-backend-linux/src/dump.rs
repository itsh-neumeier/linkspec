//! Raw sysfs collection.
//!
//! The collector reads only allow-listed sysfs classes and attributes and
//! stores them as a flat map of relative paths. The same structure is the
//! capture format, so fixtures replay through the real interpreter.
//!
//! Sources (Linux kernel ABI documentation, `Documentation/ABI/testing/`):
//! * `sysfs-class-typec` – ports, partners, cables, plugs, identity VDOs,
//!   alternate modes, `physical_location`.
//! * `sysfs-class-usb_power_delivery` – source/sink capabilities (PDOs).
//! * `sysfs-class-power` – power supplies (UCSI `ucsi-source-psy-*`,
//!   batteries).
//! * `sysfs-bus-thunderbolt` – routers, `generation`, `rx_speed`,
//!   `rx_lanes`, `tx_speed`, `tx_lanes`, `usb4_portN`.
//! * `sysfs-bus-usb` – devices (`speed`, `version`, `idVendor`, …) and hub
//!   ports (`usbN-portM`, `connector`, `over_current_count`, `connect_type`).
//! * `sysfs-class-drm` (connector `status`, `modes`, `edid`).
//!
//! Never collected: serial numbers, unique ids, UUIDs, `uevent` files.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Flat sysfs image. Keys are paths relative to the sysfs root.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SysfsDump {
    /// Attribute values (trimmed text).
    pub files: BTreeMap<String, String>,
    /// Symlinks: last two path components of the target.
    pub links: BTreeMap<String, String>,
    /// Attributes that exist but could not be read (permission denied).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub denied: Vec<String>,
    /// Decoded EDID summaries per DRM connector (raw EDID is never kept,
    /// because it contains the monitor serial number).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub edid: BTreeMap<String, linkspec_core::edid::EdidSummary>,
    /// Host data outside sysfs.
    #[serde(default)]
    pub host: HostDump,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostDump {
    /// Kernel release (`/proc/sys/kernel/osrelease`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kernel: Option<String>,
    /// DMI product name (not the serial).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,
}

/// Attribute names that are never read.
const FORBIDDEN: &[&str] = &[
    "serial",
    "serial_number",
    "unique_id",
    "uuid",
    "product_serial",
    "board_serial",
    "chassis_serial",
    "uevent",
    "descriptors",
    "edid",
    "authorized_default",
    "key",
];

const MAX_ATTR: u64 = 4096;

pub struct Collector<'a> {
    root: &'a Path,
    dump: SysfsDump,
}

impl<'a> Collector<'a> {
    pub fn new(root: &'a Path) -> Self {
        Self {
            root,
            dump: SysfsDump::default(),
        }
    }

    /// Collect everything LinkSpec needs.
    pub fn collect(mut self) -> SysfsDump {
        self.typec();
        self.usb_power_delivery();
        self.power_supply();
        self.thunderbolt();
        self.usb();
        self.drm();
        self.dump
    }

    /// Only power supplies (power monitor; cheap enough for 1 Hz).
    pub fn collect_power_only(&mut self) -> SysfsDump {
        self.power_supply();
        std::mem::take(&mut self.dump)
    }

    fn entries(&self, rel: &str) -> Vec<String> {
        let mut out: Vec<String> = std::fs::read_dir(self.root.join(rel))
            .map(|rd| {
                rd.filter_map(Result::ok)
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect()
            })
            .unwrap_or_default();
        out.sort();
        out
    }

    fn is_dir(&self, rel: &str) -> bool {
        self.root.join(rel).is_dir()
    }

    /// Read regular files in `rel` (not recursive). `allow = None` reads all
    /// small files except forbidden ones.
    fn attrs(&mut self, rel: &str, allow: Option<&[&str]>) {
        for name in self.entries(rel) {
            if FORBIDDEN.contains(&name.as_str()) {
                continue;
            }
            if let Some(a) = allow
                && !a.contains(&name.as_str())
            {
                continue;
            }
            let p = self.root.join(rel).join(&name);
            let Ok(meta) = std::fs::symlink_metadata(&p) else {
                continue;
            };
            if !meta.file_type().is_file() {
                continue;
            }
            let key = format!("{rel}/{name}");
            match read_small(&p) {
                Ok(Some(v)) => {
                    self.dump.files.insert(key, v);
                }
                Ok(None) => {}
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                    self.dump.denied.push(key);
                }
                Err(_) => {}
            }
        }
    }

    fn link(&mut self, rel: &str) {
        let p = self.root.join(rel);
        if let Ok(target) = std::fs::read_link(&p) {
            let comps: Vec<String> = target
                .components()
                .filter_map(|c| match c {
                    std::path::Component::Normal(s) => Some(s.to_string_lossy().to_string()),
                    _ => None,
                })
                .collect();
            let n = comps.len();
            let short = comps[n.saturating_sub(2)..].join("/");
            self.dump.links.insert(rel.to_string(), short);
        }
    }

    fn typec(&mut self) {
        for name in self.entries("class/typec") {
            let rel = format!("class/typec/{name}");
            self.attrs(&rel, None);
            for l in ["usb_power_delivery", "firmware_node"] {
                self.link(&format!("{rel}/{l}"));
            }
            for sub in ["identity", "physical_location"] {
                let s = format!("{rel}/{sub}");
                if self.is_dir(&s) {
                    self.attrs(&s, None);
                }
            }
            // Alternate modes: child directories named "<name>.<n>".
            for child in self.entries(&rel) {
                if child.starts_with(&format!("{name}.")) {
                    let c = format!("{rel}/{child}");
                    self.attrs(&c, None);
                    let dp = format!("{c}/displayport");
                    if self.is_dir(&dp) {
                        self.attrs(&dp, None);
                    }
                }
            }
        }
    }

    fn usb_power_delivery(&mut self) {
        for pd in self.entries("class/usb_power_delivery") {
            let rel = format!("class/usb_power_delivery/{pd}");
            self.attrs(&rel, None);
            for caps in ["source-capabilities", "sink-capabilities"] {
                let c = format!("{rel}/{caps}");
                for pdo in self.entries(&c) {
                    let p = format!("{c}/{pdo}");
                    if self.is_dir(&p) {
                        self.attrs(&p, None);
                    }
                }
            }
        }
    }

    fn power_supply(&mut self) {
        const ALLOW: &[&str] = &[
            "type",
            "online",
            "present",
            "status",
            "scope",
            "usb_type",
            "capacity",
            "voltage_now",
            "voltage_min",
            "voltage_max",
            "current_now",
            "current_max",
            "power_now",
            "charge_now",
            "charge_full",
            "energy_now",
            "energy_full",
        ];
        for name in self.entries("class/power_supply") {
            let rel = format!("class/power_supply/{name}");
            self.attrs(&rel, Some(ALLOW));
        }
    }

    fn thunderbolt(&mut self) {
        const ALLOW: &[&str] = &[
            "generation",
            "rx_speed",
            "rx_lanes",
            "tx_speed",
            "tx_lanes",
            "vendor",
            "vendor_name",
            "device",
            "device_name",
            "authorized",
            "link",
        ];
        for name in self.entries("bus/thunderbolt/devices") {
            let rel = format!("bus/thunderbolt/devices/{name}");
            self.attrs(&rel, Some(ALLOW));
            for child in self.entries(&rel) {
                if child.starts_with("usb4_port") {
                    let c = format!("{rel}/{child}");
                    self.attrs(&c, Some(ALLOW));
                    self.link(&format!("{c}/connector"));
                }
            }
        }
    }

    fn usb(&mut self) {
        const DEV: &[&str] = &[
            "speed",
            "version",
            "idVendor",
            "idProduct",
            "bcdDevice",
            "manufacturer",
            "product",
            "bDeviceClass",
            "maxchild",
            "rx_lanes",
            "tx_lanes",
            "removable",
            "devpath",
            "busnum",
        ];
        const PORT: &[&str] = &["over_current_count", "connect_type", "quirks", "location"];
        for name in self.entries("bus/usb/devices") {
            if name.contains(':') {
                // Interface; hub ports live below "<dev>:1.0".
                let rel = format!("bus/usb/devices/{name}");
                for child in self.entries(&rel) {
                    if child.contains("-port") {
                        let c = format!("{rel}/{child}");
                        self.attrs(&c, Some(PORT));
                        self.link(&format!("{c}/connector"));
                        self.link(&format!("{c}/peer"));
                    }
                }
                continue;
            }
            let rel = format!("bus/usb/devices/{name}");
            self.attrs(&rel, Some(DEV));
            self.link(&format!("{rel}/port"));
        }
    }

    fn drm(&mut self) {
        for name in self.entries("class/drm") {
            if !name.contains('-') {
                continue;
            }
            let rel = format!("class/drm/{name}");
            self.attrs(&rel, Some(&["status", "enabled", "modes", "dpms"]));
            self.link(&format!("{rel}/typec_connector"));
            if let Ok(bytes) = std::fs::read(self.root.join(&rel).join("edid"))
                && let Some(sum) = linkspec_core::edid::parse(&bytes)
            {
                self.dump.edid.insert(rel, sum);
            }
        }
    }
}

fn read_small(p: &Path) -> std::io::Result<Option<String>> {
    use std::io::Read;
    let f = std::fs::File::open(p)?;
    let mut buf = Vec::new();
    f.take(MAX_ATTR).read_to_end(&mut buf)?;
    let s = String::from_utf8_lossy(&buf).trim().to_string();
    Ok(Some(s))
}

/// Host information outside sysfs (kernel release, DMI product name).
pub fn host_info(root: &Path, proc_root: &Path) -> HostDump {
    let read = |p: &Path| {
        std::fs::read_to_string(p)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    HostDump {
        kernel: read(&proc_root.join("sys/kernel/osrelease")),
        product: read(&root.join("class/dmi/id/product_name")),
        arch: Some(std::env::consts::ARCH.to_string()),
    }
}
