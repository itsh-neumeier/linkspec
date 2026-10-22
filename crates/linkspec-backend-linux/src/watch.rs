//! Change notifications through the kernel uevent netlink socket
//! (`NETLINK_KOBJECT_UEVENT`, multicast group 1). No polling, no udevd
//! dependency. Source: netlink(7), and the kernel uevent format
//! `action@devpath\0KEY=VALUE\0…` (Documentation/ABI/testing/sysfs-uevent).

use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use linkspec_platform::{ChangeEvent, SourceError};

/// Subsystems that change USB-C state.
const SUBSYSTEMS: &[&str] = &[
    "typec",
    "usb",
    "thunderbolt",
    "usb_power_delivery",
    "drm",
    "power_supply",
];

/// True when a uevent payload is relevant for LinkSpec.
pub fn relevant(payload: &[u8]) -> bool {
    let mut subsystem = None;
    let mut devpath = "";
    for part in payload.split(|b| *b == 0) {
        let Ok(s) = std::str::from_utf8(part) else {
            continue;
        };
        if let Some(v) = s.strip_prefix("SUBSYSTEM=") {
            subsystem = Some(v);
        } else if let Some(v) = s.strip_prefix("DEVPATH=") {
            devpath = v;
        }
    }
    match subsystem {
        // Battery updates are frequent; only Type-C power supplies matter.
        Some("power_supply") => devpath.contains("ucsi") || devpath.contains("tcpm"),
        Some(s) => SUBSYSTEMS.contains(&s),
        None => false,
    }
}

pub fn spawn() -> Result<Receiver<ChangeEvent>, SourceError> {
    let (tx, rx) = channel();
    let sock = socket::UeventSocket::open().map_err(|e| SourceError::Other(e.to_string()))?;
    std::thread::Builder::new()
        .name("linkspec-uevent".into())
        .spawn(move || run(sock, tx))
        .map_err(|e| SourceError::Other(e.to_string()))?;
    Ok(rx)
}

fn run(sock: socket::UeventSocket, tx: Sender<ChangeEvent>) {
    let mut buf = vec![0u8; 16 * 1024];
    loop {
        match sock.recv(&mut buf, None) {
            Ok(Some(n)) if relevant(&buf[..n]) => {
                // Coalesce bursts (plugging a dock produces dozens of events).
                while let Ok(Some(_)) = sock.recv(&mut buf, Some(Duration::from_millis(250))) {}
                if tx.send(ChangeEvent::Changed).is_err() {
                    return;
                }
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => {
                let _ = tx.send(ChangeEvent::Stopped);
                return;
            }
        }
    }
}

mod socket {
    use std::io;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::time::Duration;

    pub struct UeventSocket {
        fd: OwnedFd,
    }

    impl UeventSocket {
        pub fn open() -> io::Result<Self> {
            // SAFETY: plain socket(2) call; the returned descriptor is owned
            // by `OwnedFd` right away.
            let raw = unsafe {
                libc::socket(
                    libc::AF_NETLINK,
                    libc::SOCK_DGRAM | libc::SOCK_CLOEXEC,
                    libc::NETLINK_KOBJECT_UEVENT,
                )
            };
            if raw < 0 {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: `raw` is a valid, newly created descriptor.
            let fd = unsafe { OwnedFd::from_raw_fd(raw) };
            // SAFETY: zeroed sockaddr_nl is a valid value; we set the fields
            // documented in netlink(7).
            let mut addr: libc::sockaddr_nl = unsafe { std::mem::zeroed() };
            addr.nl_family = libc::AF_NETLINK as libc::sa_family_t;
            addr.nl_pid = 0;
            addr.nl_groups = 1;
            // SAFETY: `addr` lives for the call; the length matches the type.
            let rc = unsafe {
                libc::bind(
                    fd.as_raw_fd(),
                    (&addr as *const libc::sockaddr_nl).cast(),
                    std::mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t,
                )
            };
            if rc < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(Self { fd })
        }

        /// Receive one datagram. With a timeout, `Ok(None)` means no data.
        pub fn recv(&self, buf: &mut [u8], timeout: Option<Duration>) -> io::Result<Option<usize>> {
            if let Some(t) = timeout {
                let mut pfd = libc::pollfd {
                    fd: self.fd.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                };
                // SAFETY: one valid pollfd for the duration of the call.
                let rc = unsafe { libc::poll(&mut pfd, 1, t.as_millis() as libc::c_int) };
                if rc < 0 {
                    return Err(io::Error::last_os_error());
                }
                if rc == 0 {
                    return Ok(None);
                }
            }
            // SAFETY: `buf` is valid for `buf.len()` bytes.
            let n =
                unsafe { libc::recv(self.fd.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len(), 0) };
            if n < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(Some(n as usize))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_events() {
        assert!(relevant(
            b"add@/devices/x/typec/port0-partner\0ACTION=add\0SUBSYSTEM=typec\0"
        ));
        assert!(!relevant(b"change@/devices/x/power_supply/BAT0\0SUBSYSTEM=power_supply\0DEVPATH=/devices/LNXSYSTM:00/BAT0\0"));
        assert!(relevant(
            b"change@/x\0SUBSYSTEM=power_supply\0DEVPATH=/devices/x/ucsi-source-psy-USBC000:001\0"
        ));
        assert!(!relevant(b"add@/x\0SUBSYSTEM=net\0"));
    }
}
