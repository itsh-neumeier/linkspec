//! IOKit and CoreGraphics collector (macOS only).
//!
//! Uses only public APIs: the IOKit registry functions (Apple "IOKit
//! Framework Reference": `IORegistryEntryCreateIterator`,
//! `IORegistryEntryCreateCFProperties`, `IOObjectGetClass`, …) and Quartz
//! Display Services (`CGGetOnlineDisplayList`, `CGDisplayCopyDisplayMode`,
//! `CGDisplayCopyAllDisplayModes`). No private frameworks, no entitlements,
//! no root.

#![allow(non_upper_case_globals)]

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{CStr, CString, c_char, c_void};

use core_foundation_sys::array::{
    CFArrayGetCount, CFArrayGetTypeID, CFArrayGetValueAtIndex, CFArrayRef,
};
use core_foundation_sys::base::{CFGetTypeID, CFRelease, CFTypeRef, kCFAllocatorDefault};
use core_foundation_sys::data::{CFDataGetBytePtr, CFDataGetLength, CFDataGetTypeID, CFDataRef};
use core_foundation_sys::dictionary::{
    CFDictionaryGetCount, CFDictionaryGetKeysAndValues, CFDictionaryGetTypeID, CFDictionaryRef,
    CFMutableDictionaryRef,
};
use core_foundation_sys::number::{
    CFBooleanGetTypeID, CFBooleanGetValue, CFBooleanRef, CFNumberGetType, CFNumberGetTypeID,
    CFNumberGetValue, CFNumberIsFloatType, CFNumberRef, kCFNumberFloat64Type, kCFNumberSInt64Type,
};
use core_foundation_sys::string::{
    CFStringGetCString, CFStringGetLength, CFStringGetMaximumSizeForEncoding, CFStringGetTypeID,
    CFStringRef, kCFStringEncodingUTF8,
};
use io_kit_sys::types::{io_iterator_t, io_object_t, io_registry_entry_t};
use io_kit_sys::*;
use serde_json::{Map, Value};

use crate::registry::{CgDisplay, MacHost, RegEntry, RegistryDump, class_matches};
use linkspec_core::observation::DisplayMode;

const KERN_SUCCESS: i32 = 0;
/// `kIOMainPortDefault` (0, `MACH_PORT_NULL`), see IOKitLib.h.
const MAIN_PORT: u32 = 0;
const MAX_DEPTH: usize = 8;

fn plane() -> *const c_char {
    c"IOService".as_ptr()
}

/// Owned io_object_t that is released on drop.
struct Obj(io_object_t);

impl Drop for Obj {
    fn drop(&mut self) {
        if self.0 != 0 {
            // SAFETY: we own one reference to this object.
            unsafe {
                IOObjectRelease(self.0);
            }
        }
    }
}

fn name_buf() -> [c_char; 128] {
    [0; 128]
}

fn class_of(o: io_object_t) -> String {
    let mut buf = name_buf();
    // SAFETY: io_name_t is a 128-byte buffer.
    if unsafe { IOObjectGetClass(o, buf.as_mut_ptr()) } != KERN_SUCCESS {
        return String::new();
    }
    // SAFETY: IOKit writes a NUL-terminated string into the buffer.
    unsafe { CStr::from_ptr(buf.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}

fn name_of(o: io_registry_entry_t) -> String {
    let mut buf = name_buf();
    // SAFETY: io_name_t is a 128-byte buffer.
    if unsafe { IORegistryEntryGetName(o, buf.as_mut_ptr()) } != KERN_SUCCESS {
        return String::new();
    }
    // SAFETY: NUL-terminated by IOKit.
    unsafe { CStr::from_ptr(buf.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}

fn entry_id(o: io_registry_entry_t) -> Option<u64> {
    let mut id = 0u64;
    // SAFETY: valid out pointer.
    (unsafe { IORegistryEntryGetRegistryEntryID(o, &mut id) } == KERN_SUCCESS).then_some(id)
}

fn parent_id(o: io_registry_entry_t) -> Option<u64> {
    let mut parent: io_registry_entry_t = 0;
    // SAFETY: valid out pointer; the parent reference is released by `Obj`.
    if unsafe { IORegistryEntryGetParentEntry(o, plane(), &mut parent) } != KERN_SUCCESS {
        return None;
    }
    let p = Obj(parent);
    entry_id(p.0)
}

/// Convert a CFString to a Rust string.
///
/// # Safety
/// `s` must be a valid CFStringRef.
unsafe fn cf_string(s: CFStringRef) -> Option<String> {
    // SAFETY: caller guarantees a valid CFString.
    unsafe {
        let len = CFStringGetLength(s);
        let max = CFStringGetMaximumSizeForEncoding(len, kCFStringEncodingUTF8) + 1;
        let mut buf = vec![0u8; max.max(1) as usize];
        if CFStringGetCString(s, buf.as_mut_ptr().cast(), max, kCFStringEncodingUTF8) == 0 {
            return None;
        }
        CStr::from_ptr(buf.as_ptr().cast())
            .to_str()
            .ok()
            .map(str::to_string)
    }
}

/// Convert any property-list CF value to JSON. CFData becomes a hex string.
///
/// # Safety
/// `v` must be a valid CF object.
unsafe fn cf_to_json(v: CFTypeRef, depth: usize) -> Value {
    if v.is_null() || depth > MAX_DEPTH {
        return Value::Null;
    }
    // SAFETY: `v` is a valid CF object; each branch checks the type id
    // before casting.
    unsafe {
        let t = CFGetTypeID(v);
        if t == CFStringGetTypeID() {
            cf_string(v as CFStringRef).map_or(Value::Null, Value::String)
        } else if t == CFBooleanGetTypeID() {
            Value::Bool(CFBooleanGetValue(v as CFBooleanRef))
        } else if t == CFNumberGetTypeID() {
            let n = v as CFNumberRef;
            if CFNumberIsFloatType(n) != 0 {
                let mut f = 0f64;
                CFNumberGetValue(n, kCFNumberFloat64Type, (&mut f as *mut f64).cast());
                serde_json::Number::from_f64(f).map_or(Value::Null, Value::Number)
            } else {
                let _ = CFNumberGetType(n);
                let mut i = 0i64;
                CFNumberGetValue(n, kCFNumberSInt64Type, (&mut i as *mut i64).cast());
                Value::from(i)
            }
        } else if t == CFDataGetTypeID() {
            let d = v as CFDataRef;
            let len = CFDataGetLength(d).clamp(0, 4096) as usize;
            let ptr = CFDataGetBytePtr(d);
            if ptr.is_null() {
                return Value::Null;
            }
            let bytes = std::slice::from_raw_parts(ptr, len);
            Value::String(bytes.iter().map(|b| format!("{b:02x}")).collect())
        } else if t == CFArrayGetTypeID() {
            let a = v as CFArrayRef;
            let n = CFArrayGetCount(a).clamp(0, 256);
            Value::Array(
                (0..n)
                    .map(|i| cf_to_json(CFArrayGetValueAtIndex(a, i), depth + 1))
                    .collect(),
            )
        } else if t == CFDictionaryGetTypeID() {
            Value::Object(cf_dict(v as CFDictionaryRef, depth + 1))
        } else {
            Value::Null
        }
    }
}

/// # Safety
/// `d` must be a valid CFDictionary.
unsafe fn cf_dict(d: CFDictionaryRef, depth: usize) -> Map<String, Value> {
    let mut out = Map::new();
    // SAFETY: the key/value buffers have `count` slots as required.
    unsafe {
        let count = CFDictionaryGetCount(d).max(0) as usize;
        let mut keys: Vec<*const c_void> = vec![std::ptr::null(); count];
        let mut vals: Vec<*const c_void> = vec![std::ptr::null(); count];
        CFDictionaryGetKeysAndValues(d, keys.as_mut_ptr(), vals.as_mut_ptr());
        for i in 0..count {
            let k = keys[i];
            if k.is_null() || CFGetTypeID(k) != CFStringGetTypeID() {
                continue;
            }
            if let Some(key) = cf_string(k as CFStringRef) {
                out.insert(key, cf_to_json(vals[i], depth));
            }
        }
    }
    out
}

fn properties(o: io_registry_entry_t) -> Map<String, Value> {
    let mut dict: CFMutableDictionaryRef = std::ptr::null_mut();
    // SAFETY: valid out pointer; we release the dictionary below.
    let kr = unsafe { IORegistryEntryCreateCFProperties(o, &mut dict, kCFAllocatorDefault, 0) };
    if kr != KERN_SUCCESS || dict.is_null() {
        return Map::new();
    }
    // SAFETY: `dict` is a valid dictionary owned by us.
    let m = unsafe { cf_dict(dict as CFDictionaryRef, 0) };
    // SAFETY: release our reference.
    unsafe { CFRelease(dict as CFTypeRef) };
    m
}

struct Raw {
    parent: Option<u64>,
    class: String,
    name: String,
    props: Option<Map<String, Value>>,
}

/// Walk the IOService plane and keep allow-listed entries (with
/// properties) plus their ancestors (class and name only).
pub fn collect_registry(filter: &dyn Fn(&str) -> bool) -> Vec<RegEntry> {
    let mut all: BTreeMap<u64, Raw> = BTreeMap::new();
    // SAFETY: plain IOKit calls; every returned object is wrapped in `Obj`.
    unsafe {
        let root = Obj(IORegistryGetRootEntry(MAIN_PORT));
        let mut it: io_iterator_t = 0;
        if IORegistryEntryCreateIterator(root.0, plane(), kIORegistryIterateRecursively, &mut it)
            != KERN_SUCCESS
        {
            return Vec::new();
        }
        let it = Obj(it);
        loop {
            let e = IOIteratorNext(it.0);
            if e == 0 {
                break;
            }
            let e = Obj(e);
            let Some(id) = entry_id(e.0) else { continue };
            let class = class_of(e.0);
            let keep = filter(&class);
            all.insert(
                id,
                Raw {
                    parent: parent_id(e.0),
                    name: name_of(e.0),
                    props: keep.then(|| properties(e.0)),
                    class,
                },
            );
        }
    }
    // Keep matched entries and their ancestor chains.
    let mut keep: BTreeSet<u64> = BTreeSet::new();
    for (id, r) in &all {
        if r.props.is_some() {
            let mut cur = Some(*id);
            let mut guard = 0;
            while let Some(c) = cur {
                guard += 1;
                if !keep.insert(c) || guard > 64 {
                    break;
                }
                cur = all.get(&c).and_then(|x| x.parent);
            }
        }
    }
    // Remap ids to sequential numbers (no registry ids in captures).
    let map: BTreeMap<u64, u64> = keep
        .iter()
        .enumerate()
        .map(|(i, id)| (*id, i as u64 + 1))
        .collect();
    keep.iter()
        .filter_map(|id| {
            let r = all.get(id)?;
            Some(RegEntry {
                id: map[id],
                parent: r.parent.and_then(|p| map.get(&p).copied()),
                class: r.class.clone(),
                name: r.name.clone(),
                props: r.props.clone().unwrap_or_default(),
            })
        })
        .collect()
}

// --- CoreGraphics ---------------------------------------------------------

type CGDirectDisplayID = u32;
type CGDisplayModeRef = *mut c_void;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGGetOnlineDisplayList(max: u32, displays: *mut CGDirectDisplayID, count: *mut u32) -> i32;
    fn CGDisplayIsBuiltin(display: CGDirectDisplayID) -> u32;
    fn CGDisplayCopyDisplayMode(display: CGDirectDisplayID) -> CGDisplayModeRef;
    fn CGDisplayCopyAllDisplayModes(
        display: CGDirectDisplayID,
        options: CFDictionaryRef,
    ) -> CFArrayRef;
    fn CGDisplayModeGetPixelWidth(mode: CGDisplayModeRef) -> usize;
    fn CGDisplayModeGetPixelHeight(mode: CGDisplayModeRef) -> usize;
    fn CGDisplayModeGetRefreshRate(mode: CGDisplayModeRef) -> f64;
    fn CGDisplayModeRelease(mode: CGDisplayModeRef);
}

fn mode_of(m: CGDisplayModeRef) -> DisplayMode {
    // SAFETY: `m` is a valid display mode.
    unsafe {
        DisplayMode {
            width: CGDisplayModeGetPixelWidth(m) as u32,
            height: CGDisplayModeGetPixelHeight(m) as u32,
            refresh_hz: CGDisplayModeGetRefreshRate(m).round(),
            bits_per_color: None,
        }
    }
}

pub fn collect_displays() -> Vec<CgDisplay> {
    let mut ids = [0u32; 16];
    let mut count = 0u32;
    // SAFETY: buffer of 16 ids.
    if unsafe { CGGetOnlineDisplayList(16, ids.as_mut_ptr(), &mut count) } != 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for &id in ids.iter().take(count as usize) {
        // SAFETY: valid display id from the list; modes are released.
        unsafe {
            let builtin = CGDisplayIsBuiltin(id) != 0;
            let cur = CGDisplayCopyDisplayMode(id);
            let current = (!cur.is_null()).then(|| mode_of(cur));
            if !cur.is_null() {
                CGDisplayModeRelease(cur);
            }
            let all = CGDisplayCopyAllDisplayModes(id, std::ptr::null());
            let mut max: Option<DisplayMode> = None;
            if !all.is_null() {
                for i in 0..CFArrayGetCount(all) {
                    let m = mode_of(CFArrayGetValueAtIndex(all, i) as CGDisplayModeRef);
                    let score = |d: &DisplayMode| {
                        u64::from(d.width) * u64::from(d.height) * (d.refresh_hz as u64).max(1)
                    };
                    if max.as_ref().is_none_or(|x| score(&m) > score(x)) {
                        max = Some(m);
                    }
                }
                CFRelease(all as CFTypeRef);
            }
            out.push(CgDisplay {
                builtin,
                current,
                max,
            });
        }
    }
    out
}

// --- Host -----------------------------------------------------------------

fn sysctl_string(name: &str) -> Option<String> {
    let cname = CString::new(name).ok()?;
    let mut len: libc::size_t = 0;
    // SAFETY: first call queries the size.
    if unsafe {
        libc::sysctlbyname(
            cname.as_ptr(),
            std::ptr::null_mut(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    } != 0
    {
        return None;
    }
    let mut buf = vec![0u8; len];
    // SAFETY: buffer has `len` bytes.
    if unsafe {
        libc::sysctlbyname(
            cname.as_ptr(),
            buf.as_mut_ptr().cast(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    } != 0
    {
        return None;
    }
    let s = String::from_utf8_lossy(&buf[..len])
        .trim_end_matches('\0')
        .trim()
        .to_string();
    (!s.is_empty()).then_some(s)
}

pub fn host() -> MacHost {
    MacHost {
        model: sysctl_string("hw.model"),
        os_version: sysctl_string("kern.osproductversion"),
        arch: std::env::consts::ARCH.to_string(),
    }
}

pub fn collect() -> RegistryDump {
    RegistryDump {
        entries: collect_registry(&class_matches),
        displays: collect_displays(),
        host: host(),
    }
}

/// Cheap state signature for the fallback poll: connection and transport
/// state of the port interfaces.
pub fn port_signature() -> String {
    let entries = collect_registry(&|c: &str| c.starts_with("AppleHPMInterfaceType"));
    let mut s = String::new();
    for e in entries.iter().filter(|e| !e.props.is_empty()) {
        for k in ["ConnectionActive", "TransportsActive"] {
            if let Some(v) = e.props.get(k) {
                s.push_str(&v.to_string());
            }
        }
    }
    s
}
