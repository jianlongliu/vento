// SPDX-License-Identifier: GPL-3.0-or-later
//
//! App preferences, saved in `$XDG_DATA_HOME/vento/settings.json`:
//! the theme choice and the defaults for new connections.

use std::fs;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::store;
use crate::theme::ThemeChoice;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub theme: ThemeChoice,
    #[serde(default = "yes")]
    pub send_clipboard: bool,
    #[serde(default = "yes")]
    pub receive_clipboard: bool,
    #[serde(default = "yes")]
    pub grab_keyboard: bool,
}

fn yes() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: ThemeChoice::Auto,
            send_clipboard: true,
            receive_clipboard: true,
            grab_keyboard: true,
        }
    }
}

pub fn path() -> PathBuf {
    store::data_dir().join("settings.json")
}

pub fn load() -> Settings {
    let mut settings: Settings = fs::read(path())
        .ok()
        .and_then(|data| serde_json::from_slice(&data).ok())
        .unwrap_or_default();
    if let Some(theme) = std::env::var_os("VENTOX_THEME")
        .and_then(|value| value.into_string().ok())
        .and_then(|value| match value.as_str() {
            "auto" => Some(ThemeChoice::Auto),
            "system" => Some(ThemeChoice::System),
            "light" | "latte" => Some(ThemeChoice::Light),
            "dark" | "mocha" => Some(ThemeChoice::Dark),
            _ => None,
        })
    {
        settings.theme = theme;
    }
    settings
}

pub fn save(settings: &Settings) -> io::Result<()> {
    let path = path();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let data = serde_json::to_vec_pretty(settings)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, data)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_settings_files_get_the_new_defaults() {
        let settings: Settings = serde_json::from_str(r#"{"theme":"dark"}"#).unwrap();
        assert_eq!(settings.theme, ThemeChoice::Dark);
        assert!(settings.send_clipboard);
        assert!(settings.receive_clipboard);
        assert!(settings.grab_keyboard);
    }

    #[test]
    fn settings_round_trip() {
        let settings = Settings {
            theme: ThemeChoice::Light,
            send_clipboard: false,
            receive_clipboard: true,
            grab_keyboard: false,
        };
        let data = serde_json::to_vec(&settings).unwrap();
        let decoded: Settings = serde_json::from_slice(&data).unwrap();
        assert_eq!(decoded.theme, settings.theme);
        assert!(!decoded.send_clipboard);
        assert!(decoded.receive_clipboard);
        assert!(!decoded.grab_keyboard);
    }
}
