//! LinkSpec command line (library part, shared with tests and the app).

pub mod dashboard;
pub mod render;
pub mod report;

use std::io::IsTerminal;

/// Use colours only when stdout is a terminal and `NO_COLOR` is not set.
/// On Windows the console must also accept ANSI sequences: crossterm turns
/// on `ENABLE_VIRTUAL_TERMINAL_PROCESSING`; without it the escape codes
/// would print as text (`←[2m`).
pub fn color_enabled() -> bool {
    std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none() && ansi_supported()
}

#[cfg(windows)]
fn ansi_supported() -> bool {
    ratatui::crossterm::ansi_support::supports_ansi()
}

#[cfg(not(windows))]
fn ansi_supported() -> bool {
    true
}
