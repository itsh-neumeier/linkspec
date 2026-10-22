//! Windows service: ETW real-time session on the Type-C providers and a
//! named pipe server for the (unprivileged) app and CLI.
//!
//! * ETW consumer API: StartTraceW, EnableTraceEx2, OpenTraceW,
//!   ProcessTrace; TDH for decoding
//!   (<https://learn.microsoft.com/windows/win32/etw/consuming-events>).
//! * Service API: StartServiceCtrlDispatcherW, RegisterServiceCtrlHandlerExW
//!   (<https://learn.microsoft.com/windows/win32/services/service-programs>).
//! * Pipe security: SDDL `D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;AU)` – full
//!   access for SYSTEM and administrators, read/write for authenticated
//!   users. The pipe only answers fixed read requests.

use std::ffi::c_void;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex, OnceLock};

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
use windows_sys::Win32::Storage::FileSystem::PIPE_ACCESS_DUPLEX;
use windows_sys::Win32::System::Diagnostics::Etw::*;
use windows_sys::Win32::System::Pipes::*;
use windows_sys::Win32::System::Services::*;
use windows_sys::core::GUID;

use crate::etwmap::{PropValue, RawEvent, State, provider_matches};

pub const SERVICE_NAME: &str = "LinkSpecService";
const SESSION_NAME: &str = "LinkSpec-TypeC";
/// Standard access right `DELETE` (winnt.h).
const DELETE_ACCESS: u32 = 0x0001_0000;
const PIPE: &str = r"\\.\pipe\linkspec-service";

static STATE: OnceLock<Arc<Mutex<State>>> = OnceLock::new();

fn state() -> Arc<Mutex<State>> {
    STATE
        .get_or_init(|| Arc::new(Mutex::new(State::default())))
        .clone()
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn read_wide(base: *const u8, offset: u32) -> String {
    if offset == 0 {
        return String::new();
    }
    // SAFETY: TDH buffers contain NUL-terminated UTF-16 strings at the
    // given offsets.
    unsafe {
        let p = base.add(offset as usize) as *const u16;
        let mut len = 0;
        while *p.add(len) != 0 && len < 1024 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(p, len))
    }
}

/// Providers installed on this machine that match the patterns.
pub fn find_providers() -> Vec<(String, GUID)> {
    let mut out = Vec::new();
    // SAFETY: two-call pattern; the buffer is sized by the first call.
    unsafe {
        let mut size = 0u32;
        TdhEnumerateProviders(std::ptr::null_mut(), &mut size);
        if size == 0 {
            return out;
        }
        let mut buf = vec![0u8; size as usize];
        let info = buf.as_mut_ptr() as *mut PROVIDER_ENUMERATION_INFO;
        if TdhEnumerateProviders(info, &mut size) != 0 {
            return out;
        }
        let n = (*info).NumberOfProviders as usize;
        let arr = std::ptr::addr_of!((*info).TraceProviderInfoArray) as *const TRACE_PROVIDER_INFO;
        for i in 0..n {
            let p = &*arr.add(i);
            let name = read_wide(buf.as_ptr(), p.ProviderNameOffset);
            if provider_matches(&name) {
                out.push((name, p.ProviderGuid));
            }
        }
    }
    out
}

fn decode(rec: *const EVENT_RECORD) -> Option<RawEvent> {
    // SAFETY: `rec` comes from ProcessTrace and is valid for this call.
    unsafe {
        let mut size = 0u32;
        TdhGetEventInformation(rec, 0, std::ptr::null(), std::ptr::null_mut(), &mut size);
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        let info = buf.as_mut_ptr() as *mut TRACE_EVENT_INFO;
        if TdhGetEventInformation(rec, 0, std::ptr::null(), info, &mut size) != 0 {
            return None;
        }
        let provider = read_wide(buf.as_ptr(), (*info).ProviderNameOffset);
        let props_ptr =
            std::ptr::addr_of!((*info).EventPropertyInfoArray) as *const EVENT_PROPERTY_INFO;
        let mut props = Vec::new();
        for i in 0..(*info).TopLevelPropertyCount as usize {
            let pi = &*props_ptr.add(i);
            if pi.Flags & PropertyStruct != 0 {
                continue;
            }
            let name_w = buf.as_ptr().add(pi.NameOffset as usize) as *const u16;
            let name = read_wide(buf.as_ptr(), pi.NameOffset);
            let desc = PROPERTY_DATA_DESCRIPTOR {
                PropertyName: name_w as u64,
                ArrayIndex: u32::MAX,
                Reserved: 0,
            };
            let mut psize = 0u32;
            if TdhGetPropertySize(rec, 0, std::ptr::null(), 1, &desc, &mut psize) != 0
                || psize == 0
                || psize > 4096
            {
                continue;
            }
            let mut data = vec![0u8; psize as usize];
            if TdhGetProperty(rec, 0, std::ptr::null(), 1, &desc, psize, data.as_mut_ptr()) != 0 {
                continue;
            }
            let in_type = pi.Anonymous1.nonStructType.InType;
            // TDH_INTYPE_UINT8=3? Use the element size: 1, 2, 4, 8 bytes.
            let width = match in_type {
                2 | 3 => 1,  // INT8, UINT8
                4 | 5 => 2,  // INT16, UINT16
                6..=8 => 4,  // INT32, UINT32, HEXINT32 is 20
                9 | 10 => 8, // INT64, UINT64
                20 => 4,     // HEXINT32
                21 => 8,     // HEXINT64
                1 => {
                    let w: Vec<u16> = data
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|c| u16::from_le_bytes([c[0], c[1]]))
                        .take_while(|c| *c != 0)
                        .collect();
                    props.push((name, PropValue::Text(String::from_utf16_lossy(&w))));
                    continue;
                }
                _ => 0,
            };
            if width == 0 {
                continue;
            }
            let vals: Vec<u64> = data
                .chunks_exact(width)
                .map(|c| {
                    let mut b = [0u8; 8];
                    b[..width].copy_from_slice(c);
                    u64::from_le_bytes(b)
                })
                .collect();
            let v = if vals.len() == 1 {
                PropValue::Int(vals[0])
            } else {
                PropValue::Ints(vals)
            };
            props.push((name, v));
        }
        Some(RawEvent {
            provider,
            event_id: (*rec).EventHeader.EventDescriptor.Id,
            props,
        })
    }
}

unsafe extern "system" fn on_event(rec: *mut EVENT_RECORD) {
    if let Some(ev) = decode(rec)
        && let Ok(mut s) = state().lock()
    {
        s.apply(ev);
    }
}

/// Start the real-time session and process events on this thread.
pub fn run_etw() -> Result<(), String> {
    let providers = find_providers();
    if providers.is_empty() {
        return Err("no Type-C ETW provider found".into());
    }
    if let Ok(mut s) = state().lock() {
        s.providers = providers.iter().map(|p| p.0.clone()).collect();
    }
    let name = wide(SESSION_NAME);
    let props_size = std::mem::size_of::<EVENT_TRACE_PROPERTIES>() + name.len() * 2 + 2;
    // SAFETY: EVENT_TRACE_PROPERTIES followed by space for the session
    // name, as documented for StartTraceW.
    unsafe {
        let mut buf = vec![0u8; props_size];
        let props = buf.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES;
        (*props).Wnode.BufferSize = props_size as u32;
        (*props).Wnode.Flags = WNODE_FLAG_TRACED_GUID;
        (*props).Wnode.ClientContext = 1;
        (*props).LogFileMode = EVENT_TRACE_REAL_TIME_MODE;
        (*props).LoggerNameOffset = std::mem::size_of::<EVENT_TRACE_PROPERTIES>() as u32;
        // Stop a stale session with the same name first.
        ControlTraceW(
            CONTROLTRACE_HANDLE { Value: 0 },
            name.as_ptr(),
            props,
            EVENT_TRACE_CONTROL_STOP,
        );
        (*props).Wnode.BufferSize = props_size as u32;
        (*props).LoggerNameOffset = std::mem::size_of::<EVENT_TRACE_PROPERTIES>() as u32;
        let mut h = CONTROLTRACE_HANDLE { Value: 0 };
        let rc = StartTraceW(&mut h, name.as_ptr(), props);
        if rc != 0 {
            return Err(format!("StartTraceW failed: {rc}"));
        }
        for (_, guid) in &providers {
            EnableTraceEx2(
                h,
                guid,
                EVENT_CONTROL_CODE_ENABLE_PROVIDER,
                5,
                0,
                0,
                0,
                std::ptr::null(),
            );
        }
        let mut lf: EVENT_TRACE_LOGFILEW = std::mem::zeroed();
        let mut lname = name.clone();
        lf.LoggerName = lname.as_mut_ptr();
        lf.Anonymous1.ProcessTraceMode =
            PROCESS_TRACE_MODE_REAL_TIME | PROCESS_TRACE_MODE_EVENT_RECORD;
        lf.Anonymous2.EventRecordCallback = Some(on_event);
        let th = OpenTraceW(&mut lf);
        if th.Value == u64::MAX {
            return Err("OpenTraceW failed".into());
        }
        ProcessTrace(&th, 1, std::ptr::null(), std::ptr::null());
        CloseTrace(th);
    }
    Ok(())
}

/// Serve requests on the named pipe forever.
pub fn run_pipe() -> Result<(), String> {
    let sddl = wide("D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;AU)");
    let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    // SAFETY: SDDL conversion with valid out pointer.
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut sd,
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err("security descriptor".into());
    }
    let sa = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: sd,
        bInheritHandle: 0,
    };
    let name = wide(PIPE);
    loop {
        // SAFETY: valid name and attributes; the handle is closed via File.
        let h = unsafe {
            CreateNamedPipeW(
                name.as_ptr(),
                PIPE_ACCESS_DUPLEX,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                PIPE_UNLIMITED_INSTANCES,
                64 * 1024,
                4096,
                0,
                &sa,
            )
        };
        if h == INVALID_HANDLE_VALUE {
            return Err("CreateNamedPipeW failed".into());
        }
        // SAFETY: blocking connect on our own pipe handle.
        let connected = unsafe { ConnectNamedPipe(h, std::ptr::null_mut()) } != 0
            || unsafe { GetLastError() } == ERROR_PIPE_CONNECTED;
        // SAFETY: we own `h`; File closes it.
        let mut f: std::fs::File = unsafe {
            use std::os::windows::io::FromRawHandle;
            std::fs::File::from_raw_handle(h)
        };
        if !connected {
            continue;
        }
        let mut req = [0u8; 512];
        let n = f.read(&mut req).unwrap_or(0);
        let text = String::from_utf8_lossy(&req[..n]);
        let reply = match state().lock() {
            Ok(s) if text.contains("\"capture\"") => {
                serde_json::to_string(&s.recent).unwrap_or_default()
            }
            Ok(s) => serde_json::to_string(&s.data()).unwrap_or_default(),
            Err(_) => "{}".into(),
        };
        let _ = f.write_all(reply.as_bytes());
        let _ = f.write_all(b"\n");
        let _ = f.flush();
        // SAFETY: flush and disconnect the client before closing.
        unsafe {
            DisconnectNamedPipe(h);
        }
    }
}

// --- Service plumbing --------------------------------------------------------

static STATUS_HANDLE: OnceLock<usize> = OnceLock::new();

fn set_status(state: u32) {
    let Some(h) = STATUS_HANDLE.get() else { return };
    let st = SERVICE_STATUS {
        dwServiceType: SERVICE_WIN32_OWN_PROCESS,
        dwCurrentState: state,
        dwControlsAccepted: if state == SERVICE_RUNNING {
            SERVICE_ACCEPT_STOP | SERVICE_ACCEPT_SHUTDOWN
        } else {
            0
        },
        dwWin32ExitCode: 0,
        dwServiceSpecificExitCode: 0,
        dwCheckPoint: 0,
        dwWaitHint: 0,
    };
    // SAFETY: handle from RegisterServiceCtrlHandlerExW.
    unsafe {
        SetServiceStatus(*h as SERVICE_STATUS_HANDLE, &st);
    }
}

unsafe extern "system" fn control(ctrl: u32, _t: u32, _d: *mut c_void, _c: *mut c_void) -> u32 {
    if ctrl == SERVICE_CONTROL_STOP || ctrl == SERVICE_CONTROL_SHUTDOWN {
        set_status(SERVICE_STOPPED);
        std::process::exit(0);
    }
    NO_ERROR
}

unsafe extern "system" fn service_main(_argc: u32, _argv: *mut windows_sys::core::PWSTR) {
    let name = wide(SERVICE_NAME);
    // SAFETY: registering the control handler for this service.
    let h = unsafe {
        RegisterServiceCtrlHandlerExW(name.as_ptr(), Some(control), std::ptr::null_mut())
    };
    let _ = STATUS_HANDLE.set(h as usize);
    set_status(SERVICE_RUNNING);
    std::thread::spawn(|| {
        let _ = run_etw();
    });
    let _ = run_pipe();
    set_status(SERVICE_STOPPED);
}

pub fn dispatch() -> Result<(), String> {
    let mut name = wide(SERVICE_NAME);
    let table = [
        SERVICE_TABLE_ENTRYW {
            lpServiceName: name.as_mut_ptr(),
            lpServiceProc: Some(service_main),
        },
        SERVICE_TABLE_ENTRYW {
            lpServiceName: std::ptr::null_mut(),
            lpServiceProc: None,
        },
    ];
    // SAFETY: null-terminated service table.
    if unsafe { StartServiceCtrlDispatcherW(table.as_ptr()) } == 0 {
        return Err("StartServiceCtrlDispatcherW failed (start via the service manager)".into());
    }
    Ok(())
}

pub fn install() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let bin = wide(&format!("\"{}\" run", exe.display()));
    let name = wide(SERVICE_NAME);
    let display = wide("LinkSpec Service");
    // SAFETY: SCM calls with valid strings; handles closed.
    unsafe {
        let scm = OpenSCManagerW(
            std::ptr::null(),
            std::ptr::null(),
            SC_MANAGER_CREATE_SERVICE,
        );
        if scm.is_null() {
            return Err("OpenSCManagerW failed (administrator rights needed)".into());
        }
        let svc = CreateServiceW(
            scm,
            name.as_ptr(),
            display.as_ptr(),
            SERVICE_ALL_ACCESS,
            SERVICE_WIN32_OWN_PROCESS,
            SERVICE_AUTO_START,
            SERVICE_ERROR_NORMAL,
            bin.as_ptr(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
        );
        if svc.is_null() {
            CloseServiceHandle(scm);
            return Err(format!("CreateServiceW failed: {}", GetLastError()));
        }
        StartServiceW(svc, 0, std::ptr::null());
        CloseServiceHandle(svc);
        CloseServiceHandle(scm);
    }
    Ok(())
}

pub fn uninstall() -> Result<(), String> {
    let name = wide(SERVICE_NAME);
    // SAFETY: SCM calls; handles closed.
    unsafe {
        let scm = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_CONNECT);
        if scm.is_null() {
            return Err("OpenSCManagerW failed".into());
        }
        let svc = OpenServiceW(scm, name.as_ptr(), SERVICE_STOP | DELETE_ACCESS);
        if svc.is_null() {
            CloseServiceHandle(scm);
            return Err("service not installed".into());
        }
        let mut st: SERVICE_STATUS = std::mem::zeroed();
        ControlService(svc, SERVICE_CONTROL_STOP, &mut st);
        DeleteService(svc);
        CloseServiceHandle(svc);
        CloseServiceHandle(scm);
    }
    Ok(())
}
