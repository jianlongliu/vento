// SPDX-License-Identifier: GPL-3.0-or-later
//
//! The runtime palette.
//!
//! Four choices: Catppuccin Latte (light), Catppuccin Mocha (dark), the
//! system light/dark preference with Catppuccin, and Auto, which reads the
//! palette omarchy generated from the wallpaper with matugen and falls back
//! to the system choice when that file is not there.
//!
//! The palette is process-wide: views ask `theme::surface()` and friends at
//! paint time, so changing it and calling `refresh_windows` repaints every
//! window with the new colors.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;

use gpui::{FontWeight, Pixels, Rgba, WindowAppearance, px};
use serde::{Deserialize, Serialize};

/// A const-friendly `rgb`, so the palette can live in constants.
pub const fn rgb(hex: u32) -> Rgba {
    Rgba {
        r: ((hex >> 16) & 0xff) as f32 / 255.0,
        g: ((hex >> 8) & 0xff) as f32 / 255.0,
        b: (hex & 0xff) as f32 / 255.0,
        a: 1.0,
    }
}

/// A const-friendly `rgba`, where `hex` is `0xRRGGBBAA`.
pub const fn rgba(hex: u32) -> Rgba {
    Rgba {
        r: ((hex >> 24) & 0xff) as f32 / 255.0,
        g: ((hex >> 16) & 0xff) as f32 / 255.0,
        b: ((hex >> 8) & 0xff) as f32 / 255.0,
        a: (hex & 0xff) as f32 / 255.0,
    }
}

/// Replace a colour's alpha without changing its hue.
pub fn fade(color: Rgba, alpha: f32) -> Rgba {
    Rgba { a: alpha, ..color }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub window_bg: Rgba,
    pub sidebar_bg: Rgba,
    pub surface: Rgba,
    pub surface_raised: Rgba,
    pub surface_hover: Rgba,
    pub surface_active: Rgba,
    pub overlay_scrim: Rgba,
    pub border: Rgba,
    pub border_soft: Rgba,
    pub border_strong: Rgba,
    pub text: Rgba,
    pub text_secondary: Rgba,
    pub text_tertiary: Rgba,
    pub accent: Rgba,
    pub accent_hover: Rgba,
    pub accent_soft: Rgba,
    pub accent_text: Rgba,
    pub green: Rgba,
    pub yellow: Rgba,
    pub red: Rgba,
    pub protocol_vnc: Rgba,
    pub protocol_libvnc: Rgba,
    pub protocol_tigervnc: Rgba,
    pub protocol_rdp: Rgba,
    pub protocol_ironrdp: Rgba,
    pub protocol_spice: Rgba,
    pub protocol_rustdesk: Rgba,
}

impl Palette {
    /// Catppuccin Mocha, the dark palette.
    pub const fn mocha() -> Self {
        Self {
            window_bg: rgb(0x11111b),
            sidebar_bg: rgb(0x181825),
            surface: rgb(0x1e1e2e),
            surface_raised: rgb(0x313244),
            surface_hover: rgb(0x45475a),
            surface_active: rgb(0x585b70),
            overlay_scrim: rgba(0x000000a6),
            border: rgb(0x313244),
            border_soft: rgb(0x242433),
            border_strong: rgb(0x585b70),
            text: rgb(0xcdd6f4),
            text_secondary: rgb(0xa6adc8),
            text_tertiary: rgb(0x6c7086),
            accent: rgb(0x89b4fa),
            accent_hover: rgb(0xb4befe),
            accent_soft: rgba(0x89b4fa29),
            accent_text: rgb(0x11111b),
            green: rgb(0xa6e3a1),
            yellow: rgb(0xf9e2af),
            red: rgb(0xf38ba8),
            protocol_vnc: rgb(0x89b4fa),
            protocol_libvnc: rgb(0x94e2d5),
            protocol_tigervnc: rgb(0xfab387),
            protocol_rdp: rgb(0xcba6f7),
            protocol_ironrdp: rgb(0xb4befe),
            protocol_spice: rgb(0xf5c2e7),
            protocol_rustdesk: rgb(0xa6e3a1),
        }
    }

    /// Catppuccin Latte, the light palette.
    pub const fn latte() -> Self {
        Self {
            window_bg: rgb(0xdce0e8),
            sidebar_bg: rgb(0xe6e9ef),
            surface: rgb(0xeff1f5),
            surface_raised: rgb(0xccd0da),
            surface_hover: rgb(0xbcc0cc),
            surface_active: rgb(0xacb0be),
            overlay_scrim: rgba(0x00000066),
            border: rgb(0xbcc0cc),
            border_soft: rgb(0xbcc0cc),
            border_strong: rgb(0xacb0be),
            text: rgb(0x4c4f69),
            text_secondary: rgb(0x5c5f77),
            text_tertiary: rgb(0x6c6f85),
            accent: rgb(0x1e66f5),
            accent_hover: rgb(0x1656d6),
            accent_soft: rgba(0x1e66f529),
            accent_text: rgb(0xffffff),
            green: rgb(0x40a02b),
            yellow: rgb(0xdf8e1d),
            red: rgb(0xd20f39),
            protocol_vnc: rgb(0x1e66f5),
            protocol_libvnc: rgb(0x179299),
            protocol_tigervnc: rgb(0xfe640b),
            protocol_rdp: rgb(0x8839ef),
            protocol_ironrdp: rgb(0x7287fd),
            protocol_spice: rgb(0xea76cb),
            protocol_rustdesk: rgb(0x40a02b),
        }
    }
}

static PALETTE: RwLock<Palette> = RwLock::new(Palette::mocha());

pub fn apply(palette: Palette) {
    *PALETTE.write().unwrap() = palette;
}

pub fn current() -> Palette {
    *PALETTE.read().unwrap()
}

macro_rules! color {
    ($name:ident, $field:ident) => {
        pub fn $name() -> Rgba {
            PALETTE.read().unwrap().$field
        }
    };
}

color!(window_bg, window_bg);
color!(sidebar_bg, sidebar_bg);
color!(surface, surface);
color!(surface_raised, surface_raised);
color!(surface_hover, surface_hover);
color!(surface_active, surface_active);
color!(overlay_scrim, overlay_scrim);
color!(border, border);
color!(border_soft, border_soft);
color!(border_strong, border_strong);
color!(text, text);
color!(text_secondary, text_secondary);
color!(text_tertiary, text_tertiary);
color!(accent, accent);
color!(accent_hover, accent_hover);
color!(accent_soft, accent_soft);
color!(accent_text, accent_text);
color!(green, green);
color!(yellow, yellow);
color!(red, red);
color!(protocol_vnc, protocol_vnc);
color!(protocol_libvnc, protocol_libvnc);
color!(protocol_tigervnc, protocol_tigervnc);
color!(protocol_rdp, protocol_rdp);
color!(protocol_ironrdp, protocol_ironrdp);
color!(protocol_spice, protocol_spice);
color!(protocol_rustdesk, protocol_rustdesk);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeChoice {
    /// omarchy's wallpaper palette when it exists, otherwise the system
    /// light/dark preference with Catppuccin.
    #[default]
    #[serde(rename = "auto")]
    Auto,
    /// The system light/dark preference with Catppuccin.
    #[serde(rename = "system")]
    System,
    /// Catppuccin Latte.
    #[serde(rename = "light")]
    Light,
    /// Catppuccin Mocha.
    #[serde(rename = "dark")]
    Dark,
}

impl ThemeChoice {
    pub const ALL: [ThemeChoice; 4] = [
        ThemeChoice::Auto,
        ThemeChoice::System,
        ThemeChoice::Light,
        ThemeChoice::Dark,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ThemeChoice::Auto => "Auto",
            ThemeChoice::System => "System",
            ThemeChoice::Light => "Latte",
            ThemeChoice::Dark => "Mocha",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            ThemeChoice::Auto => "Wallpaper colors, then system",
            ThemeChoice::System => "Follow system light/dark",
            ThemeChoice::Light => "Catppuccin Latte",
            ThemeChoice::Dark => "Catppuccin Mocha",
        }
    }
}

/// The palette for a choice. `system_dark` is the window's appearance when
/// one is available; omarchy's generated mode wins when it exists, because
/// that is what the rest of this desktop follows.
pub fn palette_for(choice: ThemeChoice, system_dark: Option<bool>) -> Palette {
    match choice {
        ThemeChoice::Light => Palette::latte(),
        ThemeChoice::Dark => Palette::mocha(),
        ThemeChoice::System => {
            if system_is_dark(system_dark) {
                Palette::mocha()
            } else {
                Palette::latte()
            }
        }
        ThemeChoice::Auto => matugen_palette().unwrap_or_else(|| {
            if system_is_dark(system_dark) {
                Palette::mocha()
            } else {
                Palette::latte()
            }
        }),
    }
}

pub fn system_is_dark(window_dark: Option<bool>) -> bool {
    if let Some(dark) = omarchy_mode() {
        return dark;
    }
    window_dark.unwrap_or(true)
}

pub fn window_dark(appearance: WindowAppearance) -> bool {
    matches!(
        appearance,
        WindowAppearance::Dark | WindowAppearance::VibrantDark
    )
}

fn omarchy_theme_file() -> Option<PathBuf> {
    let state = std::env::var_os("XDG_STATE_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default()
                .join(".local/state")
        });
    let path = state.join("omarchy/current/theme/colors.toml");
    path.is_file().then_some(path)
}

fn omarchy_mode() -> Option<bool> {
    let text = fs::read_to_string(omarchy_theme_file()?).ok()?;
    let values = toml_values(&text);
    match values.get("mode").map(String::as_str) {
        Some("dark") => Some(true),
        Some("light") => Some(false),
        _ => None,
    }
}

fn matugen_palette() -> Option<Palette> {
    let text = fs::read_to_string(omarchy_theme_file()?).ok()?;
    palette_from_matugen(&text)
}

fn palette_from_matugen(text: &str) -> Option<Palette> {
    let values = toml_values(text);
    let get = |key: &str| values.get(key).map(String::as_str);
    let dark = !matches!(get("mode"), Some("light"));
    let background = hex(get("background")?)?;
    let foreground = hex(get("foreground")?)?;
    let accent = hex(get("accent")?)?;

    let darker_background = get("darker_background")
        .and_then(hex)
        .unwrap_or_else(|| blend(background, rgb(0x000000), 0.25));
    let dark_background = get("dark_background")
        .and_then(hex)
        .unwrap_or_else(|| blend(background, rgb(0x000000), 0.12));
    let lighter_background = get("lighter_background")
        .and_then(hex)
        .unwrap_or_else(|| blend(background, foreground, 0.08));
    let muted = get("muted")
        .or_else(|| get("dark_foreground"))
        .and_then(hex)
        .unwrap_or_else(|| blend(foreground, background, 0.35));
    let text_secondary = get("dark_foreground")
        .or_else(|| get("bright_foreground"))
        .and_then(hex)
        .unwrap_or_else(|| blend(foreground, background, 0.22));
    let red = get("red").and_then(hex).unwrap_or_else(|| rgb(0xf38ba8));
    let yellow = get("yellow").and_then(hex).unwrap_or_else(|| rgb(0xf9e2af));
    let green = get("green").and_then(hex).unwrap_or_else(|| rgb(0xa6e3a1));
    let blue = get("blue").and_then(hex).unwrap_or(accent);
    let cyan = get("cyan").and_then(hex).unwrap_or(blue);
    let orange = get("orange")
        .and_then(hex)
        .unwrap_or_else(|| blend(red, yellow, 0.5));
    let magenta = get("magenta")
        .and_then(hex)
        .unwrap_or_else(|| blend(blue, red, 0.5));

    Some(Palette {
        window_bg: darker_background,
        sidebar_bg: dark_background,
        surface: background,
        surface_raised: lighter_background,
        surface_hover: blend(background, foreground, if dark { 0.08 } else { 0.07 }),
        surface_active: blend(background, foreground, if dark { 0.13 } else { 0.12 }),
        overlay_scrim: rgba(0x000000a6),
        border: blend(background, foreground, 0.10),
        border_soft: blend(background, foreground, 0.06),
        border_strong: blend(background, foreground, 0.20),
        text: foreground,
        text_secondary,
        text_tertiary: muted,
        accent,
        accent_hover: if dark {
            blend(accent, rgb(0xffffff), 0.15)
        } else {
            blend(accent, rgb(0x000000), 0.12)
        },
        accent_soft: fade(accent, 0.16),
        accent_text: readable_on(accent),
        green,
        yellow,
        red,
        protocol_vnc: blue,
        protocol_libvnc: cyan,
        protocol_tigervnc: orange,
        protocol_rdp: magenta,
        protocol_ironrdp: blend(blue, magenta, 0.5),
        protocol_spice: red,
        protocol_rustdesk: green,
    })
}

fn toml_values(text: &str) -> HashMap<String, String> {
    let mut values = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches('"').trim_matches('\'');
        values.insert(key.trim().to_string(), value.to_string());
    }
    values
}

fn hex(value: &str) -> Option<Rgba> {
    let value = value.trim().trim_matches('"').trim_matches('\'');
    let hex = value.strip_prefix('#')?;
    match hex.len() {
        6 => u32::from_str_radix(hex, 16).ok().map(rgb),
        8 => u32::from_str_radix(hex, 16).ok().map(rgba),
        _ => None,
    }
}

fn blend(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let t = t.clamp(0.0, 1.0);
    Rgba {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    }
}

fn readable_on(color: Rgba) -> Rgba {
    let luminance = 0.2126 * color.r + 0.7152 * color.g + 0.0722 * color.b;
    if luminance > 0.55 {
        rgb(0x101014)
    } else {
        rgb(0xffffff)
    }
}

// Session chrome sits on a fixed near-black canvas regardless of the app
// theme, so it needs its own light text tokens.
pub const SESSION_TEXT: Rgba = rgb(0xcdd6f4);
pub const SESSION_TEXT_SECONDARY: Rgba = rgb(0xa6adc8);
pub const SESSION_TEXT_TERTIARY: Rgba = rgb(0x9399b2);
pub const SESSION_ACCENT: Rgba = rgb(0x89b4fa);

// Type scale.
pub const TEXT_LARGE_TITLE: Pixels = px(26.);
pub const TEXT_TITLE: Pixels = px(17.);
pub const TEXT_BODY: Pixels = px(13.);
pub const TEXT_SMALL: Pixels = px(11.5);
pub const TEXT_MICRO: Pixels = px(10.5);

// Spacing.
pub const SIDEBAR_WIDTH: Pixels = px(236.);
pub const CONTENT_PADDING: Pixels = px(28.);
pub const CARD_WIDTH: Pixels = px(268.);

pub const WEIGHT_REGULAR: FontWeight = FontWeight::NORMAL;
pub const WEIGHT_MEDIUM: FontWeight = FontWeight::MEDIUM;
pub const WEIGHT_SEMIBOLD: FontWeight = FontWeight::SEMIBOLD;
pub const WEIGHT_BOLD: FontWeight = FontWeight::BOLD;

pub const FONT_FAMILY: &str = "Noto Sans";
pub const MONO_FAMILY: &str = "Noto Sans Mono";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_colors_parse_with_and_without_alpha() {
        assert_eq!(hex("#89b4fa"), Some(rgb(0x89b4fa)));
        assert_eq!(hex("#89b4fa80"), Some(rgba(0x89b4fa80)));
        assert_eq!(hex("nope"), None);
    }

    #[test]
    fn catppuccin_text_contrasts_with_its_background() {
        let mocha = Palette::mocha();
        let latte = Palette::latte();
        for palette in [mocha, latte] {
            let background = 0.2126 * palette.window_bg.r
                + 0.7152 * palette.window_bg.g
                + 0.0722 * palette.window_bg.b;
            let text = 0.2126 * palette.text.r + 0.7152 * palette.text.g + 0.0722 * palette.text.b;
            assert!((background - text).abs() > 0.4);
        }
    }

    #[test]
    fn explicit_choices_resolve_to_their_palettes() {
        assert_eq!(
            palette_for(ThemeChoice::Light, None).window_bg,
            Palette::latte().window_bg
        );
        assert_eq!(
            palette_for(ThemeChoice::Dark, None).window_bg,
            Palette::mocha().window_bg
        );
    }

    #[test]
    fn a_matugen_file_becomes_a_palette() {
        let text = r##"
mode = "dark"
background = "#111318"
dark_background = "#0c0e13"
darker_background = "#0a0b10"
lighter_background = "#282a2f"
foreground = "#e1e2e9"
dark_foreground = "#c3c6cf"
accent = "#a6c8ff"
red = "#f38ba8"
yellow = "#f9e2af"
green = "#a6e3a1"
blue = "#89b4fa"
cyan = "#94e2d5"
orange = "#f6b6ab"
magenta = "#f5c2e7"
"##;
        let palette = palette_from_matugen(text).unwrap();
        assert_eq!(palette.window_bg, rgb(0x0a0b10));
        assert_eq!(palette.surface, rgb(0x111318));
        assert_eq!(palette.accent, rgb(0xa6c8ff));
        assert_eq!(palette.text, rgb(0xe1e2e9));
        assert_eq!(palette.protocol_spice, palette.red);
    }

    #[test]
    fn a_file_without_the_required_colors_is_not_a_palette() {
        assert!(palette_from_matugen("mode = \"dark\"\nbackground = \"#111318\"\n").is_none());
    }
}
