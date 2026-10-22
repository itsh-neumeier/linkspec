//! LinkSpec command line (library part, shared with tests and the app).

pub mod dashboard;
pub mod render;
pub mod report;

use std::io::IsTerminal;

/// Use colours only when stdout is a terminal and `NO_COLOR` is not set.
pub fn color_enabled() -> bool {
    std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}
