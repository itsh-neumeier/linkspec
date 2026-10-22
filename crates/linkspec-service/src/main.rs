//! `linkspec-service` – optional Windows service (tier 2, ADR 0003).
//!
//! ```text
//! linkspec-service install     register and start (administrator)
//! linkspec-service uninstall   stop and remove (administrator)
//! linkspec-service run         entry point for the service manager
//! linkspec-service console     run in the foreground (debugging)
//! linkspec-service providers   list matching ETW providers
//! ```

#[cfg_attr(not(windows), allow(dead_code))]
mod etwmap;
#[cfg(windows)]
mod win;

use std::process::ExitCode;

fn main() -> ExitCode {
    let cmd = std::env::args().nth(1).unwrap_or_default();
    #[cfg(windows)]
    {
        let res = match cmd.as_str() {
            "install" => win::install(),
            "uninstall" => win::uninstall(),
            "run" => win::dispatch(),
            "console" => {
                std::thread::spawn(|| {
                    if let Err(e) = win::run_etw() {
                        eprintln!("ETW: {e}");
                    }
                });
                win::run_pipe()
            }
            "providers" => {
                for (name, guid) in win::find_providers() {
                    println!(
                        "{name}  {:08x}-{:04x}-{:04x}",
                        guid.data1, guid.data2, guid.data3
                    );
                }
                Ok(())
            }
            _ => {
                eprintln!("usage: linkspec-service install|uninstall|run|console|providers");
                return ExitCode::from(2);
            }
        };
        match res {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(1)
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (cmd, etwmap::PROVIDER_PATTERNS);
        eprintln!("linkspec-service runs on Windows only.");
        ExitCode::from(3)
    }
}
