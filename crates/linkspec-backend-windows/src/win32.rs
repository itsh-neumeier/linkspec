//! Win32 access (tier 1, user mode, no administrator rights).
//!
//! * SetupAPI / CfgMgr32: device interfaces, device properties
//!   (<https://learn.microsoft.com/windows-hardware/drivers/install/setupapi>).
//! * USB hub IOCTLs (`usbioctl.h`), opened like the USBView sample does.
//! * Battery IOCTLs (`IOCTL_BATTERY_QUERY_TAG/_STATUS/_INFORMATION`).
//! * Display configuration (`QueryDisplayConfig`, `DisplayConfigGetDeviceInfo`)
//!   and EDID from the monitor device registry key.
//! * `CM_Register_Notification` for change events.
//! * Named pipe client for the optional LinkSpec service.

#![allow(clippy::upper_case_acronyms)]

use std::collections::BTreeMap;
use std::ffi::c_void;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use windows_sys::Win32::Devices::DeviceAndDriverInstallation::*;
use windows_sys::Win32::Devices::Display::*;
use windows_sys::Win32::Devices::Properties::*;
use windows_sys::Win32::Devices::Usb::*;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Storage::FileSystem::*;
use windows_sys::Win32::System::IO::DeviceIoControl;
use windows_sys::Win32::System::Power::*;
use windows_sys::Win32::System::Registry::*;
use windows_sys::core::GUID;

use linkspec_core::observation::DisplayMode;
use linkspec_platform::{ChangeEvent, SourceError};

use crate::collect::{HubApi, Req, norm};
use crate::model::{BatteryDump, DisplayDump, ServiceData, ServiceState, Usb4Router, WinHost};

/// `IOCTL_USB_GET_NODE_CONNECTION_SUPERSPEEDPLUS_INFORMATION`:
/// CTL_CODE(FILE_DEVICE_USB, 280, METHOD_BUFFERED, FILE_ANY_ACCESS)
/// (usbioctl.h, function code 280; hypothesis H-W6 – the call fails safely
/// if the code is wrong).
const IOCTL_USB_SSP_INFO: u32 = (0x22 << 16) | (280 << 2);

pub const PIPE_NAME: &str = r"\\.\pipe\linkspec-service";

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn from_wide(w: &[u16]) -> String {
    let end = w.iter().position(|c| *c == 0).unwrap_or(w.len());
    String::from_utf16_lossy(&w[..end])
}

struct Handle(HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
            // SAFETY: we own the handle.
            unsafe { CloseHandle(self.0) };
        }
    }
}

fn open(path: &str, access: u32) -> Option<Handle> {
    let w = wide(path);
    // SAFETY: valid NUL-terminated path; handle is owned by `Handle`.
    let h = unsafe {
        CreateFileW(
            w.as_ptr(),
            access,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        )
    };
    (h != INVALID_HANDLE_VALUE).then_some(Handle(h))
}

fn ioctl(h: &Handle, code: u32, input: &[u8], out_len: usize) -> Option<Vec<u8>> {
    let mut out = vec![0u8; out_len];
    let mut returned = 0u32;
    // SAFETY: buffers are valid for the given lengths.
    let ok = unsafe {
        DeviceIoControl(
            h.0,
            code,
            input.as_ptr().cast(),
            input.len() as u32,
            out.as_mut_ptr().cast(),
            out.len() as u32,
            &mut returned,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return None;
    }
    out.truncate(returned as usize);
    Some(out)
}

/// Device interfaces of a class: (path, devinst).
fn interfaces(guid: &GUID) -> Vec<(String, u32)> {
    let mut out = Vec::new();
    // SAFETY: SetupAPI enumeration with correctly sized structures; the
    // device info list is destroyed at the end.
    unsafe {
        let set = SetupDiGetClassDevsW(
            guid,
            std::ptr::null(),
            std::ptr::null_mut(),
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        );
        if set == INVALID_HANDLE_VALUE as HDEVINFO {
            return out;
        }
        let mut i = 0;
        loop {
            let mut did: SP_DEVICE_INTERFACE_DATA = std::mem::zeroed();
            did.cbSize = std::mem::size_of::<SP_DEVICE_INTERFACE_DATA>() as u32;
            if SetupDiEnumDeviceInterfaces(set, std::ptr::null(), guid, i, &mut did) == 0 {
                break;
            }
            i += 1;
            let mut needed = 0u32;
            SetupDiGetDeviceInterfaceDetailW(
                set,
                &did,
                std::ptr::null_mut(),
                0,
                &mut needed,
                std::ptr::null_mut(),
            );
            if needed == 0 {
                continue;
            }
            let mut buf = vec![0u8; needed as usize + 8];
            let detail = buf.as_mut_ptr() as *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W;
            (*detail).cbSize = std::mem::size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
            let mut dev: SP_DEVINFO_DATA = std::mem::zeroed();
            dev.cbSize = std::mem::size_of::<SP_DEVINFO_DATA>() as u32;
            if SetupDiGetDeviceInterfaceDetailW(
                set,
                &did,
                detail,
                needed,
                std::ptr::null_mut(),
                &mut dev,
            ) == 0
            {
                continue;
            }
            let path_ptr = std::ptr::addr_of!((*detail).DevicePath) as *const u16;
            let max = (needed as usize).saturating_sub(4) / 2;
            let path = from_wide(std::slice::from_raw_parts(path_ptr, max));
            out.push((path, dev.DevInst));
        }
        SetupDiDestroyDeviceInfoList(set);
    }
    out
}

/// A string (or first string of a string list) property of a devnode.
fn devnode_string(devinst: u32, key: &DEVPROPKEY) -> Option<String> {
    let mut ty: DEVPROPTYPE = 0;
    let mut size = 0u32;
    // SAFETY: size query, then read into a buffer of that size.
    unsafe {
        CM_Get_DevNode_PropertyW(devinst, key, &mut ty, std::ptr::null_mut(), &mut size, 0);
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u16; (size as usize).div_ceil(2)];
        if CM_Get_DevNode_PropertyW(devinst, key, &mut ty, buf.as_mut_ptr().cast(), &mut size, 0)
            != CR_SUCCESS
        {
            return None;
        }
        let s = from_wide(&buf);
        (!s.is_empty()).then_some(s)
    }
}

fn children(devinst: u32) -> Vec<u32> {
    let mut out = Vec::new();
    let mut child = 0u32;
    // SAFETY: CfgMgr32 tree walk with valid out pointers.
    unsafe {
        if CM_Get_Child(&mut child, devinst, 0) != CR_SUCCESS {
            return out;
        }
        out.push(child);
        let mut cur = child;
        let mut next = 0u32;
        while CM_Get_Sibling(&mut next, cur, 0) == CR_SUCCESS {
            out.push(next);
            cur = next;
        }
    }
    out
}

/// Win32 implementation of [`HubApi`].
pub struct Win32Hubs {
    devinst: BTreeMap<String, u32>,
    handles: BTreeMap<String, Handle>,
}

impl Win32Hubs {
    pub fn new() -> Self {
        Self {
            devinst: BTreeMap::new(),
            handles: BTreeMap::new(),
        }
    }

    fn handle(&mut self, hub: &str) -> Option<&Handle> {
        let k = norm(hub);
        if !self.handles.contains_key(&k) {
            let h = open(hub, GENERIC_WRITE)?;
            self.handles.insert(k.clone(), h);
        }
        self.handles.get(&k)
    }

    fn name_ioctl(&mut self, hub: &str, code: u32, port: u32) -> Option<String> {
        let h = self.handle(hub)?;
        let mut input = vec![0u8; 8];
        input[0..4].copy_from_slice(&port.to_le_bytes());
        let first = ioctl(h, code, &input, 8 + 512)?;
        let actual = u32::from_le_bytes(first.get(4..8)?.try_into().ok()?) as usize;
        let buf = if actual > first.len() {
            ioctl(h, code, &input, actual)?
        } else {
            first
        };
        let words: Vec<u16> = buf
            .get(8..)?
            .as_chunks::<2>().0.iter()
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        let s = from_wide(&words);
        (!s.is_empty()).then_some(s)
    }
}

impl Default for Win32Hubs {
    fn default() -> Self {
        Self::new()
    }
}

impl HubApi for Win32Hubs {
    fn hubs(&mut self) -> Vec<String> {
        interfaces(&GUID_DEVINTERFACE_USB_HUB)
            .into_iter()
            .map(|(p, d)| {
                self.devinst.insert(norm(&p), d);
                p
            })
            .collect()
    }

    fn ioctl(&mut self, hub: &str, req: Req, port: u32) -> Option<Vec<u8>> {
        let h = self.handle(hub)?;
        let mut input = vec![0u8; req.min_size()];
        if req != Req::HubInfoEx {
            input[0..4].copy_from_slice(&port.to_le_bytes());
        }
        match req {
            Req::HubInfoEx => ioctl(h, IOCTL_USB_GET_HUB_INFORMATION_EX, &[], 128),
            Req::ConnectorProps => {
                let first = ioctl(h, IOCTL_USB_GET_PORT_CONNECTOR_PROPERTIES, &input, 16)?;
                let actual = u32::from_le_bytes(first.get(4..8)?.try_into().ok()?) as usize;
                if actual > 16 {
                    ioctl(
                        h,
                        IOCTL_USB_GET_PORT_CONNECTOR_PROPERTIES,
                        &vec_with_port(port, actual),
                        actual,
                    )
                } else {
                    Some(first)
                }
            }
            Req::ConnInfoEx => ioctl(
                h,
                IOCTL_USB_GET_NODE_CONNECTION_INFORMATION_EX,
                &input,
                35 + 32 * 7,
            ),
            Req::ConnInfoV2 => {
                input[4..8].copy_from_slice(&16u32.to_le_bytes());
                // Caller understands USB 1.1, 2.0 and 3.x.
                input[8..12].copy_from_slice(&0b111u32.to_le_bytes());
                ioctl(
                    h,
                    IOCTL_USB_GET_NODE_CONNECTION_INFORMATION_EX_V2,
                    &input,
                    16,
                )
            }
            Req::SspInfo => {
                input[4..8].copy_from_slice(&24u32.to_le_bytes());
                ioctl(h, IOCTL_USB_SSP_INFO, &input, 24)
            }
        }
    }

    fn child_hub(&mut self, hub: &str, port: u32) -> Option<String> {
        self.name_ioctl(hub, IOCTL_USB_GET_NODE_CONNECTION_NAME, port)
            .map(|n| format!(r"\\?\{n}"))
    }

    fn hub_location(&mut self, hub: &str) -> Option<String> {
        let d = *self.devinst.get(&norm(hub))?;
        devnode_string(d, &DEVPKEY_Device_LocationPaths)
    }

    fn device_text(&mut self, hub: &str, port: u32) -> (Option<String>, Option<String>) {
        let Some(driver) = self.name_ioctl(hub, IOCTL_USB_GET_NODE_CONNECTION_DRIVERKEY_NAME, port)
        else {
            return (None, None);
        };
        let Some(hub_dev) = self.devinst.get(&norm(hub)).copied() else {
            return (None, None);
        };
        for child in children(hub_dev) {
            if devnode_string(child, &DEVPKEY_Device_Driver)
                .is_some_and(|d| d.eq_ignore_ascii_case(&driver))
            {
                return (
                    devnode_string(child, &DEVPKEY_Device_BusReportedDeviceDesc)
                        .or_else(|| devnode_string(child, &DEVPKEY_Device_DeviceDesc)),
                    devnode_string(child, &DEVPKEY_Device_Manufacturer)
                        .filter(|m| !m.starts_with('(')),
                );
            }
        }
        (None, None)
    }

    fn descriptor(&mut self, hub: &str, port: u32, dtype: u8, index: u8) -> Option<Vec<u8>> {
        let h = self.handle(hub)?;
        // USB_DESCRIPTOR_REQUEST (packed): ConnectionIndex, SetupPacket, Data.
        let mut req = vec![0u8; 12 + 255];
        req[0..4].copy_from_slice(&port.to_le_bytes());
        req[4] = 0x80; // device-to-host, standard, device
        req[5] = 0x06; // GET_DESCRIPTOR
        let w_value = (u16::from(dtype) << 8) | u16::from(index);
        req[6..8].copy_from_slice(&w_value.to_le_bytes());
        let w_index: u16 = if dtype == 0x03 { 0x0409 } else { 0 };
        req[8..10].copy_from_slice(&w_index.to_le_bytes());
        req[10..12].copy_from_slice(&255u16.to_le_bytes());
        let len = req.len();
        let out = ioctl(h, IOCTL_USB_GET_DESCRIPTOR_FROM_NODE_CONNECTION, &req, len)?;
        out.get(12..).map(<[u8]>::to_vec)
    }
}

fn vec_with_port(port: u32, len: usize) -> Vec<u8> {
    let mut v = vec![0u8; len];
    v[0..4].copy_from_slice(&port.to_le_bytes());
    v
}

// --- Battery ---------------------------------------------------------------

pub fn batteries() -> Vec<BatteryDump> {
    let mut out = Vec::new();
    for (path, _) in interfaces(&GUID_DEVICE_BATTERY) {
        let Some(h) = open(&path, GENERIC_READ | GENERIC_WRITE) else {
            continue;
        };
        let wait = 0u32.to_le_bytes();
        let Some(tag) = ioctl(&h, IOCTL_BATTERY_QUERY_TAG, &wait, 4)
            .and_then(|b| {
                b.get(0..4)
                    .map(|s| u32::from_le_bytes(s.try_into().unwrap()))
            })
            .filter(|t| *t != 0)
        else {
            continue;
        };
        // BATTERY_QUERY_INFORMATION { tag, level = BatteryInformation (0), AtRate }.
        let mut q = vec![0u8; 12];
        q[0..4].copy_from_slice(&tag.to_le_bytes());
        let full = ioctl(&h, IOCTL_BATTERY_QUERY_INFORMATION, &q, 32).and_then(|b| {
            b.get(16..20)
                .map(|s| u32::from_le_bytes(s.try_into().unwrap()))
        });
        // BATTERY_WAIT_STATUS { tag, timeout, power state, low, high }.
        let mut w = vec![0u8; 20];
        w[0..4].copy_from_slice(&tag.to_le_bytes());
        let Some(st) = ioctl(&h, IOCTL_BATTERY_QUERY_STATUS, &w, 16) else {
            continue;
        };
        let u = |o: usize| {
            st.get(o..o + 4)
                .map(|s| u32::from_le_bytes(s.try_into().unwrap()))
        };
        let unknown = 0xffff_ffffu32;
        out.push(BatteryDump {
            power_state: u(0).unwrap_or(0),
            capacity_mwh: u(4).filter(|v| *v != unknown),
            full_mwh: full.filter(|v| *v != unknown && *v != 0),
            voltage_mv: u(8).filter(|v| *v != unknown),
            rate_mw: u(12).filter(|v| *v != 0x8000_0000).map(|v| v as i32),
        });
    }
    out
}

// --- Displays --------------------------------------------------------------

fn monitor_edid(device_path: &str) -> Option<Vec<u8>> {
    let target = norm(device_path);
    // SAFETY: SetupAPI enumeration of monitor interfaces; registry key closed.
    unsafe {
        let set = SetupDiGetClassDevsW(
            &GUID_DEVINTERFACE_MONITOR,
            std::ptr::null(),
            std::ptr::null_mut(),
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        );
        if set == INVALID_HANDLE_VALUE as HDEVINFO {
            return None;
        }
        let mut result = None;
        for (path, devinst) in interfaces(&GUID_DEVINTERFACE_MONITOR) {
            if norm(&path) != target {
                continue;
            }
            let mut dev: SP_DEVINFO_DATA = std::mem::zeroed();
            dev.cbSize = std::mem::size_of::<SP_DEVINFO_DATA>() as u32;
            let mut i = 0;
            while SetupDiEnumDeviceInfo(set, i, &mut dev) != 0 {
                i += 1;
                if dev.DevInst != devinst {
                    continue;
                }
                let key = SetupDiOpenDevRegKey(set, &dev, DICS_FLAG_GLOBAL, 0, DIREG_DEV, KEY_READ);
                if key.is_null() || std::ptr::eq(key, INVALID_HANDLE_VALUE as HKEY) {
                    break;
                }
                let name = wide("EDID");
                let mut buf = vec![0u8; 1024];
                let mut size = buf.len() as u32;
                if RegQueryValueExW(
                    key,
                    name.as_ptr(),
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    buf.as_mut_ptr(),
                    &mut size,
                ) == 0
                {
                    buf.truncate(size as usize);
                    result = Some(buf);
                }
                RegCloseKey(key);
                break;
            }
        }
        SetupDiDestroyDeviceInfoList(set);
        result
    }
}

pub fn displays() -> Vec<DisplayDump> {
    let mut out = Vec::new();
    // SAFETY: buffer sizes from GetDisplayConfigBufferSizes; structures are
    // zero-initialised plain data.
    unsafe {
        let (mut np, mut nm) = (0u32, 0u32);
        if GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut np, &mut nm) != 0 {
            return out;
        }
        let mut paths: Vec<DISPLAYCONFIG_PATH_INFO> = vec![std::mem::zeroed(); np as usize];
        let mut modes: Vec<DISPLAYCONFIG_MODE_INFO> = vec![std::mem::zeroed(); nm as usize];
        if QueryDisplayConfig(
            QDC_ONLY_ACTIVE_PATHS,
            &mut np,
            paths.as_mut_ptr(),
            &mut nm,
            modes.as_mut_ptr(),
            std::ptr::null_mut(),
        ) != 0
        {
            return out;
        }
        paths.truncate(np as usize);
        modes.truncate(nm as usize);
        for p in &paths {
            let t = &p.targetInfo;
            let src_idx = p.sourceInfo.Anonymous.modeInfoIdx as usize;
            let rr = t.refreshRate;
            let hz = if rr.Denominator != 0 {
                (f64::from(rr.Numerator) / f64::from(rr.Denominator)).round()
            } else {
                0.0
            };
            let current = modes
                .get(src_idx)
                .filter(|m| m.infoType == DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE)
                .map(|m| {
                    let s = m.Anonymous.sourceMode;
                    DisplayMode {
                        width: s.width,
                        height: s.height,
                        refresh_hz: hz,
                        bits_per_color: None,
                    }
                });
            let mut name: DISPLAYCONFIG_TARGET_DEVICE_NAME = std::mem::zeroed();
            name.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME;
            name.header.size = std::mem::size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32;
            name.header.adapterId = t.adapterId;
            name.header.id = t.id;
            let edid = if DisplayConfigGetDeviceInfo(&mut name.header) == 0 {
                monitor_edid(&from_wide(&name.monitorDevicePath))
                    .and_then(|e| linkspec_core::edid::parse(&e))
            } else {
                None
            };
            out.push(DisplayDump {
                output_technology: t.outputTechnology,
                current,
                edid,
            });
        }
    }
    out
}

// --- USB4 routers (H-W3) -----------------------------------------------------

pub fn usb4_routers() -> Vec<Usb4Router> {
    let mut out = Vec::new();
    let filter = wide("USB4");
    // SAFETY: size query, then the list; strings are parsed from the buffer.
    unsafe {
        let mut len = 0u32;
        if CM_Get_Device_ID_List_SizeW(
            &mut len,
            filter.as_ptr(),
            CM_GETIDLIST_FILTER_ENUMERATOR | CM_GETIDLIST_FILTER_PRESENT,
        ) != CR_SUCCESS
            || len == 0
        {
            return out;
        }
        let mut buf = vec![0u16; len as usize];
        if CM_Get_Device_ID_ListW(
            filter.as_ptr(),
            buf.as_mut_ptr(),
            len,
            CM_GETIDLIST_FILTER_ENUMERATOR | CM_GETIDLIST_FILTER_PRESENT,
        ) != CR_SUCCESS
        {
            return out;
        }
        for id in buf.split(|c| *c == 0).filter(|s| !s.is_empty()) {
            let id_str = String::from_utf16_lossy(id);
            let upper = id_str.to_ascii_uppercase();
            let kind = if upper.contains("ROOT_DEVICE_ROUTER") || upper.contains("HOST_ROUTER") {
                "host"
            } else if upper.contains("VID_") || upper.contains("DEVICE_ROUTER") {
                "device"
            } else {
                continue;
            };
            let vendor_id = upper
                .split("VID_")
                .nth(1)
                .and_then(|r| u16::from_str_radix(r.get(0..4)?, 16).ok());
            let mut devinst = 0u32;
            let wid: Vec<u16> = id.iter().copied().chain(std::iter::once(0)).collect();
            let description =
                if CM_Locate_DevNodeW(&mut devinst, wid.as_ptr(), CM_LOCATE_DEVNODE_NORMAL)
                    == CR_SUCCESS
                {
                    devnode_string(devinst, &DEVPKEY_Device_DeviceDesc)
                } else {
                    None
                };
            // The instance id is never stored (it can contain serials).
            out.push(Usb4Router {
                kind: kind.to_string(),
                vendor_id,
                description,
            });
        }
    }
    out
}

// --- Host --------------------------------------------------------------------

fn reg_string(root: HKEY, sub: &str, value: &str) -> Option<String> {
    let s = wide(sub);
    let v = wide(value);
    let mut buf = vec![0u16; 256];
    let mut size = (buf.len() * 2) as u32;
    // SAFETY: buffer size in bytes is passed.
    let rc = unsafe {
        RegGetValueW(
            root,
            s.as_ptr(),
            v.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            buf.as_mut_ptr().cast(),
            &mut size,
        )
    };
    (rc == 0).then(|| from_wide(&buf)).filter(|x| !x.is_empty())
}

pub fn host() -> WinHost {
    let nt = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
    let build = reg_string(HKEY_LOCAL_MACHINE, nt, "CurrentBuildNumber");
    let display = reg_string(HKEY_LOCAL_MACHINE, nt, "DisplayVersion");
    WinHost {
        os_version: build.map(|b| match display {
            Some(d) => format!("10.0.{b} ({d})"),
            None => format!("10.0.{b}"),
        }),
        model: reg_string(
            HKEY_LOCAL_MACHINE,
            r"HARDWARE\DESCRIPTION\System\BIOS",
            "SystemProductName",
        ),
        arch: std::env::consts::ARCH.to_string(),
    }
}

// --- Service client ----------------------------------------------------------

pub fn service_state() -> ServiceState {
    use std::io::{Read, Write};
    let Some(h) = open(PIPE_NAME, GENERIC_READ | GENERIC_WRITE) else {
        return ServiceState::NotInstalled;
    };
    // SAFETY: wrap the owned pipe handle into a File for buffered I/O.
    let mut f: std::fs::File = unsafe {
        use std::os::windows::io::FromRawHandle;
        let raw = h.0;
        std::mem::forget(h);
        std::fs::File::from_raw_handle(raw as _)
    };
    if f.write_all(b"{\"request\":\"snapshot\"}\n").is_err() {
        return ServiceState::Error {
            message: "write failed".into(),
        };
    }
    let mut text = String::new();
    let mut buf = [0u8; 4096];
    loop {
        match f.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                text.push_str(&String::from_utf8_lossy(&buf[..n]));
                if text.ends_with('\n') || text.len() > 1 << 20 {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    match serde_json::from_str::<ServiceData>(text.trim()) {
        Ok(d) => ServiceState::Running(d),
        Err(e) => ServiceState::Error {
            message: e.to_string(),
        },
    }
}

// --- Notifications -----------------------------------------------------------

struct NotifyCtx {
    tx: Sender<()>,
}

unsafe extern "system" fn on_notify(
    _h: HCMNOTIFICATION,
    context: *const c_void,
    _action: CM_NOTIFY_ACTION,
    _data: *const CM_NOTIFY_EVENT_DATA,
    _size: u32,
) -> u32 {
    // SAFETY: the context is the leaked `NotifyCtx` registered below.
    let ctx = unsafe { &*(context as *const NotifyCtx) };
    let _ = ctx.tx.send(());
    ERROR_SUCCESS
}

pub fn watch() -> Result<Receiver<ChangeEvent>, SourceError> {
    let (raw_tx, raw_rx) = channel::<()>();
    let (tx, rx) = channel();
    // Lives for the process lifetime (the registration is never removed).
    let ctx: &'static NotifyCtx = Box::leak(Box::new(NotifyCtx { tx: raw_tx }));
    let classes = [
        GUID_DEVINTERFACE_USB_DEVICE,
        GUID_DEVINTERFACE_USB_HUB,
        GUID_DEVINTERFACE_MONITOR,
        GUID_DEVICE_BATTERY,
    ];
    let mut ok = 0;
    for g in classes {
        // SAFETY: filter is a zero-initialised CM_NOTIFY_FILTER with the
        // documented fields set; the context outlives the registration.
        unsafe {
            let mut f: CM_NOTIFY_FILTER = std::mem::zeroed();
            f.cbSize = std::mem::size_of::<CM_NOTIFY_FILTER>() as u32;
            f.FilterType = CM_NOTIFY_FILTER_TYPE_DEVICEINTERFACE;
            f.u.DeviceInterface.ClassGuid = g;
            let mut h: HCMNOTIFICATION = std::ptr::null_mut();
            if CM_Register_Notification(
                &f,
                ctx as *const NotifyCtx as *const c_void,
                Some(on_notify),
                &mut h,
            ) == CR_SUCCESS
            {
                ok += 1;
            }
        }
    }
    if ok == 0 {
        return Err(SourceError::Other("CM_Register_Notification failed".into()));
    }
    std::thread::Builder::new()
        .name("linkspec-cm-notify".into())
        .spawn(move || {
            while raw_rx.recv().is_ok() {
                // Coalesce bursts (a dock produces many events).
                while raw_rx.recv_timeout(Duration::from_millis(300)).is_ok() {}
                if tx.send(ChangeEvent::Changed).is_err() {
                    return;
                }
            }
        })
        .map_err(|e| SourceError::Other(e.to_string()))?;
    Ok(rx)
}
