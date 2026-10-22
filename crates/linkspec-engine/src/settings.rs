//! User settings (prompt 5.11), stored as JSON next to the history.

use std::path::PathBuf;

use linkspec_core::Lang;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UiMode {
    #[default]
    Tray,
    Window,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub hide_empty_ports: bool,
    pub start_at_login: bool,
    pub mode: UiMode,
    /// Font scale factor (1.0 = system size).
    pub font_scale: f32,
    pub technical_details: bool,
    pub usb_probe: bool,
    /// `None` = system language.
    pub language: Option<Lang>,
    pub notifications: bool,
    pub update_check: bool,
    pub keep_window_open: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hide_empty_ports: false,
            start_at_login: false,
            mode: UiMode::Tray,
            font_scale: 1.0,
            technical_details: false,
            usb_probe: true,
            language: None,
            notifications: true,
            update_check: true,
            keep_window_open: false,
        }
    }
}

impl Settings {
    pub fn path() -> Option<PathBuf> {
        linkspec_history::data_dir().map(|d| d.join("settings.json"))
    }

    pub fn load() -> Self {
        Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let Some(p) = Self::path() else {
            return Ok(());
        };
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(p, text)
    }

    pub fn lang(&self) -> Lang {
        self.language.unwrap_or_else(crate::system_lang)
    }
}
