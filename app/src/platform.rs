//! Platform helpers: autostart, notifications, widget snapshot, dock icon,
//! update check, opening URLs.

use std::path::PathBuf;

/// Start at login.
/// * Windows: `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
/// * macOS: `SMAppService.mainApp` (macOS 13+; needs the signed .app).
/// * Linux: XDG autostart entry (`~/.config/autostart/linkspec.desktop`).
pub fn set_autostart(on: bool) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Registry::*;
        let wide = |s: &str| {
            s.encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<u16>>()
        };
        let key = wide(r"Software\Microsoft\Windows\CurrentVersion\Run");
        let name = wide("LinkSpec");
        let mut hkey: HKEY = std::ptr::null_mut();
        // SAFETY: registry calls with valid strings; key closed below.
        unsafe {
            if RegOpenKeyExW(HKEY_CURRENT_USER, key.as_ptr(), 0, KEY_SET_VALUE, &mut hkey) != 0 {
                return Err("RegOpenKeyExW".into());
            }
            let rc = if on {
                let val = wide(&format!("\"{}\" --tray", exe.display()));
                RegSetValueExW(
                    hkey,
                    name.as_ptr(),
                    0,
                    REG_SZ,
                    val.as_ptr().cast(),
                    (val.len() * 2) as u32,
                )
            } else {
                RegDeleteValueW(hkey, name.as_ptr())
            };
            RegCloseKey(hkey);
            if rc != 0 && on {
                return Err(format!("RegSetValueExW {rc}"));
            }
        }
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        let _ = exe;
        use objc2_service_management::SMAppService;
        // SAFETY: documented SMAppService API on the main app bundle.
        unsafe {
            let svc = SMAppService::mainAppService();
            let r = if on {
                svc.registerAndReturnError()
            } else {
                svc.unregisterAndReturnError()
            };
            r.map_err(|e| e.to_string())
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let dir = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .ok_or("no config dir")?
            .join("autostart");
        let file = dir.join("linkspec.desktop");
        if on {
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let text = format!(
                "[Desktop Entry]\nType=Application\nName=LinkSpec\nExec=\"{}\" --tray\nIcon=linkspec\nX-GNOME-Autostart-enabled=true\nNoDisplay=false\n",
                exe.display()
            );
            std::fs::write(file, text).map_err(|e| e.to_string())
        } else {
            let _ = std::fs::remove_file(file);
            Ok(())
        }
    }
}

pub fn notify(title: &str, body: &str) {
    let r = notify_rust::Notification::new()
        .summary(title)
        .body(body)
        .appname("LinkSpec")
        .show();
    if let Err(e) = r {
        tracing::debug!("notification failed: {e}");
    }
}

/// Snapshot for widgets (macOS WidgetKit, Windows widget board). Only
/// headlines and summaries, no identifiers.
pub fn write_widget_snapshot(v: &serde_json::Value) {
    let mut paths: Vec<PathBuf> = Vec::new();
    if let Some(d) = linkspec_history::data_dir() {
        paths.push(d.join("widget.json"));
    }
    #[cfg(target_os = "macos")]
    if let Some(h) = std::env::var_os("HOME") {
        // App Group container shared with the widget extension (ADR 0005).
        paths.push(
            PathBuf::from(h).join(
                "Library/Group Containers/group.io.github.itsh-neumeier.linkspec/widget.json",
            ),
        );
    }
    let text = serde_json::to_string(v).unwrap_or_default();
    for p in paths {
        if let Some(dir) = p.parent()
            && std::fs::create_dir_all(dir).is_ok()
        {
            let _ = std::fs::write(&p, &text);
        }
    }
}

pub fn open_url(url: &str) {
    let mut cmd = if cfg!(target_os = "macos") {
        let mut c = std::process::Command::new("open");
        c.arg(url);
        c
    } else if cfg!(windows) {
        let mut c = std::process::Command::new("rundll32");
        c.args(["url.dll,FileProtocolHandler", url]);
        c
    } else {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(url);
        c
    };
    let _ = cmd.spawn();
}

/// macOS: hide the Dock icon in tray mode (accessory app), show it in
/// window mode.
pub fn set_dock_icon(visible: bool) {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let app = NSApplication::sharedApplication(mtm);
            app.setActivationPolicy(if visible {
                NSApplicationActivationPolicy::Regular
            } else {
                NSApplicationActivationPolicy::Accessory
            });
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = visible;
}

/// Windows: `SystemUsesLightTheme` = 1 means a light taskbar (Windows 10/11
/// personalization setting). Other systems: false (white glyph).
pub fn taskbar_is_light() -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Registry::*;
        let wide = |s: &str| {
            s.encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<u16>>()
        };
        let key = wide(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
        let name = wide("SystemUsesLightTheme");
        let mut value = 0u32;
        let mut size = std::mem::size_of::<u32>() as u32;
        // SAFETY: reads one DWORD into a local of the given size.
        let rc = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_DWORD,
                std::ptr::null_mut(),
                (&mut value as *mut u32).cast(),
                &mut size,
            )
        };
        rc == 0 && value == 1
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Linux: a tray needs a StatusNotifierItem host. GNOME has none without an
/// extension, so LinkSpec falls back to the window there.
pub fn tray_probably_available() -> bool {
    if cfg!(all(unix, not(target_os = "macos"))) {
        let desktop = std::env::var("XDG_CURRENT_DESKTOP")
            .unwrap_or_default()
            .to_ascii_lowercase();
        let has_display =
            std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("DISPLAY").is_some();
        // Unity is GNOME-based but ships a tray host.
        let gnome_without_tray = desktop.contains("gnome") && !desktop.contains("unity");
        has_display && !gnome_without_tray
    } else {
        true
    }
}

// --- Update check ------------------------------------------------------------

const RELEASES_API: &str = "https://api.github.com/repos/itsh-neumeier/linkspec/releases/latest";
const MIN_INTERVAL_SECS: u64 = 6 * 3600;

/// Installed through a package manager: the package manager updates.
pub fn packaged() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|d| d.join("linkspec-packaged")))
        .is_some_and(|m| m.exists())
}

fn version_tuple(v: &str) -> Vec<u64> {
    v.trim_start_matches('v')
        .split(['.', '-'])
        .map_while(|p| p.parse().ok())
        .collect()
}

pub fn newer(remote: &str, local: &str) -> bool {
    version_tuple(remote) > version_tuple(local)
}

/// Ask GitHub for the latest release (no hardware data is sent; only the
/// app version in the user agent). Returns `Some((tag, url))` when newer.
/// `force` ignores the 6-hour interval (menu "check for updates").
pub fn check_update(force: bool) -> Result<Option<(String, String)>, String> {
    let stamp = linkspec_history::data_dir().map(|d| d.join("update-check"));
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if !force
        && let Some(s) = &stamp
        && let Ok(t) = std::fs::read_to_string(s)
        && let Ok(t) = t.trim().parse::<u64>()
        && now.saturating_sub(t) < MIN_INTERVAL_SECS
    {
        return Ok(None);
    }
    if let Some(s) = &stamp {
        let _ = std::fs::write(s, now.to_string());
    }
    let body: serde_json::Value = ureq::get(RELEASES_API)
        .header(
            "User-Agent",
            &format!("LinkSpec/{}", linkspec_engine::APP_VERSION),
        )
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .read_json()
        .map_err(|e| e.to_string())?;
    let tag = body["tag_name"].as_str().unwrap_or_default().to_string();
    let url = body["html_url"].as_str().unwrap_or_default().to_string();
    Ok(newer(&tag, linkspec_engine::APP_VERSION).then_some((tag, url)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_compare() {
        assert!(newer("v1.0.0", "0.9.0"));
        assert!(newer("1.0.1", "1.0.0"));
        assert!(!newer("v1.0.0", "1.0.0"));
        assert!(!newer("garbage", "1.0.0"));
    }
}
