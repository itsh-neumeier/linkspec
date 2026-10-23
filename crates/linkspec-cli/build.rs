fn main() {
    windows_icon("../../packaging/icons/linkspec.ico");
}

/// Windows: embed the app icon into the .exe (Explorer, taskbar, Alt+Tab).
/// A cross build from another OS has no resource compiler; it only warns.
fn windows_icon(ico: &str) {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    println!("cargo:rerun-if-changed={ico}");
    let mut res = winresource::WindowsResource::new();
    res.set_icon(ico);
    if let Err(e) = res.compile() {
        if cfg!(windows) {
            panic!("embedding the Windows icon failed: {e}");
        }
        println!("cargo:warning=no Windows icon embedded (cross build): {e}");
    }
}
