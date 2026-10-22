//! IOKit notifications (first match / terminated) for USB devices and
//! Thunderbolt switches, plus a slow fallback poll (every 5 s) of the port
//! interface state, because property changes of the port controllers do not
//! publish a matching notification.

use std::ffi::{CString, c_void};
use std::sync::mpsc::{Receiver, Sender, channel};

use core_foundation_sys::runloop::{
    CFRunLoopAddSource, CFRunLoopGetCurrent, CFRunLoopRunInMode, kCFRunLoopDefaultMode,
};
use io_kit_sys::keys::{kIOFirstMatchNotification, kIOTerminatedNotification};
use io_kit_sys::types::io_iterator_t;
use io_kit_sys::*;
use linkspec_platform::{ChangeEvent, SourceError};

const CLASSES: &[&str] = &[
    "IOUSBHostDevice",
    "IOThunderboltSwitch",
    "AppleSmartBattery",
];

struct Ctx {
    tx: Sender<ChangeEvent>,
}

unsafe extern "C" fn on_match(refcon: *mut c_void, it: io_iterator_t) {
    // Drain the iterator; this re-arms the notification.
    // SAFETY: IOKit passes a valid iterator; objects are released.
    unsafe {
        loop {
            let o = IOIteratorNext(it);
            if o == 0 {
                break;
            }
            IOObjectRelease(o);
        }
        let ctx = &*(refcon as *const Ctx);
        let _ = ctx.tx.send(ChangeEvent::Changed);
    }
}

pub fn spawn() -> Result<Receiver<ChangeEvent>, SourceError> {
    let (tx, rx) = channel();
    std::thread::Builder::new()
        .name("linkspec-iokit".into())
        .spawn(move || run(tx))
        .map_err(|e| SourceError::Other(e.to_string()))?;
    Ok(rx)
}

fn run(tx: Sender<ChangeEvent>) {
    // The context lives as long as this thread (the run loop never returns
    // except to poll), so leaking it is intended.
    let ctx: &'static Ctx = Box::leak(Box::new(Ctx { tx: tx.clone() }));
    // SAFETY: standard IOKit notification setup on this thread's run loop.
    unsafe {
        let port = IONotificationPortCreate(0);
        if port.is_null() {
            let _ = tx.send(ChangeEvent::Stopped);
            return;
        }
        CFRunLoopAddSource(
            CFRunLoopGetCurrent(),
            IONotificationPortGetRunLoopSource(port),
            kCFRunLoopDefaultMode,
        );
        for class in CLASSES {
            for kind in [kIOFirstMatchNotification, kIOTerminatedNotification] {
                let Ok(c) = CString::new(*class) else {
                    continue;
                };
                let matching = IOServiceMatching(c.as_ptr());
                if matching.is_null() {
                    continue;
                }
                let mut it: io_iterator_t = 0;
                // The matching dictionary is consumed by this call.
                if IOServiceAddMatchingNotification(
                    port,
                    kind,
                    matching as _,
                    on_match,
                    ctx as *const Ctx as *mut c_void,
                    &mut it,
                ) == 0
                {
                    // Arm: drain without sending.
                    loop {
                        let o = IOIteratorNext(it);
                        if o == 0 {
                            break;
                        }
                        IOObjectRelease(o);
                    }
                }
            }
        }
    }
    let mut last = crate::collect::port_signature();
    loop {
        // SAFETY: runs this thread's run loop for up to 5 seconds.
        unsafe {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 5.0, 0);
        }
        let sig = crate::collect::port_signature();
        if sig != last {
            last = sig;
            if tx.send(ChangeEvent::Changed).is_err() {
                return;
            }
        }
    }
}
