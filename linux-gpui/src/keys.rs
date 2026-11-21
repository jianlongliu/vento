// SPDX-License-Identifier: GPL-3.0-or-later
//
//! GPUI keystrokes as the far end wants them: an X11 keysym plus a stable
//! identity per logical key, and the modifier keys a chord implies.
//!
//! GPUI delivers logical keys with the modifiers resolved (Ctrl+C arrives as
//! key `c` with `control`), so a chord is replayed to the far end as the
//! modifier's own key down, the key, then the modifier up. Shift is not
//! replayed for typed characters because `key_char` already carries the
//! shifted result; it is replayed for named keys (Shift+Left selects).

use gpui::Keystroke;

pub const SHIFT_L: u32 = 0xffe1;
pub const CONTROL_L: u32 = 0xffe3;
pub const ALT_L: u32 = 0xffe9;
pub const SUPER_L: u32 = 0xffeb;

const SHIFT_ID: i64 = 1_000_001;
const CONTROL_ID: i64 = 1_000_002;
const ALT_ID: i64 = 1_000_003;
const SUPER_ID: i64 = 1_000_004;

/// The modifier keys a keystroke needs replayed, as `(key_id, keysym)`.
pub fn modifiers(keystroke: &Keystroke) -> Vec<(i64, u32)> {
    let mut mods = Vec::new();
    if keystroke.modifiers.control && !is_modifier_key(&keystroke.key) {
        mods.push((CONTROL_ID, CONTROL_L));
    }
    if keystroke.modifiers.alt && !is_modifier_key(&keystroke.key) {
        mods.push((ALT_ID, ALT_L));
    }
    if keystroke.modifiers.shift
        && !is_modifier_key(&keystroke.key)
        && (keystroke.key_char.is_none()
            || keystroke.modifiers.control
            || keystroke.modifiers.alt
            || keystroke.modifiers.platform)
    {
        mods.push((SHIFT_ID, SHIFT_L));
    }
    if keystroke.modifiers.platform && !is_modifier_key(&keystroke.key) {
        mods.push((SUPER_ID, SUPER_L));
    }
    mods
}

fn is_modifier_key(key: &str) -> bool {
    matches!(
        key,
        "shift"
            | "control"
            | "ctrl"
            | "alt"
            | "option"
            | "platform"
            | "super"
            | "meta"
            | "function"
            | "fn"
            | "capslock"
            | "caps_lock"
    )
}

/// The keysym to send for the key itself.
pub fn keysym(keystroke: &Keystroke) -> Option<u32> {
    let chord =
        keystroke.modifiers.control || keystroke.modifiers.alt || keystroke.modifiers.platform;
    if chord || keystroke.key_char.is_none() {
        return name_to_keysym(&keystroke.key);
    }
    let c = keystroke.key_char.as_ref()?.chars().next()?;
    Some(char_keysym(c))
}

/// A stable identity for this logical key within the process.
pub fn key_id(key: &str) -> i64 {
    // FNV-1a; the only requirement is that a release finds its press.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in key.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    (hash & 0x0fff_ffff_ffff_ffff) as i64
}

fn char_keysym(c: char) -> u32 {
    let cp = c as u32;
    if cp <= 0xff { cp } else { 0x0100_0000 | cp }
}

fn name_to_keysym(key: &str) -> Option<u32> {
    if let Some(c) = single_char(key) {
        return Some(char_keysym(c));
    }
    let sym = match key {
        "return" | "enter" => 0xff0d,
        "tab" => 0xff09,
        "backspace" => 0xff08,
        "escape" | "esc" => 0xff1b,
        "delete" => 0xffff,
        "insert" => 0xff63,
        "home" => 0xff50,
        "end" => 0xff57,
        "pageup" | "page_up" => 0xff55,
        "pagedown" | "page_down" => 0xff56,
        "left" | "arrowleft" | "arrow_left" => 0xff51,
        "up" | "arrowup" | "arrow_up" => 0xff52,
        "right" | "arrowright" | "arrow_right" => 0xff53,
        "down" | "arrowdown" | "arrow_down" => 0xff54,
        "space" => 0x20,
        "f1" => 0xffbe,
        "f2" => 0xffbf,
        "f3" => 0xffc0,
        "f4" => 0xffc1,
        "f5" => 0xffc2,
        "f6" => 0xffc3,
        "f7" => 0xffc4,
        "f8" => 0xffc5,
        "f9" => 0xffc6,
        "f10" => 0xffc7,
        "f11" => 0xffc8,
        "f12" => 0xffc9,
        "shift" => SHIFT_L,
        "control" | "ctrl" => CONTROL_L,
        "alt" | "option" => ALT_L,
        "platform" | "super" | "meta" | "cmd" | "command" => SUPER_L,
        "capslock" | "caps_lock" => 0xffe5,
        "numlock" | "num_lock" => 0xff7f,
        "menu" => 0xff67,
        "print" | "printscreen" => 0xff61,
        "pause" => 0xff13,
        "kp_enter" => 0xff8d,
        _ => return None,
    };
    Some(sym)
}

fn single_char(key: &str) -> Option<char> {
    let mut chars = key.chars();
    let c = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    if c.is_ascii_uppercase() {
        Some(c.to_ascii_lowercase())
    } else {
        Some(c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::Modifiers;

    fn stroke(
        key: &str,
        key_char: Option<&str>,
        control: bool,
        alt: bool,
        shift: bool,
        platform: bool,
    ) -> Keystroke {
        Keystroke {
            modifiers: Modifiers {
                control,
                alt,
                shift,
                platform,
                function: false,
            },
            key: key.to_string(),
            key_char: key_char.map(str::to_string),
        }
    }

    #[test]
    fn characters_and_named_keys_become_keysyms() {
        assert_eq!(
            keysym(&stroke("a", Some("a"), false, false, false, false)),
            Some(0x61)
        );
        assert_eq!(
            keysym(&stroke("a", Some("A"), false, false, true, false)),
            Some(0x41)
        );
        assert_eq!(
            keysym(&stroke("return", None, false, false, false, false)),
            Some(0xff0d)
        );
        assert_eq!(
            keysym(&stroke("left", None, false, false, false, false)),
            Some(0xff51)
        );
        assert_eq!(
            keysym(&stroke("f12", None, false, false, false, false)),
            Some(0xffc9)
        );
    }

    #[test]
    fn a_chord_uses_the_key_name_and_replays_its_modifiers() {
        let ctrl_c = stroke("c", Some("c"), true, false, false, false);
        assert_eq!(keysym(&ctrl_c), Some(0x63));
        assert_eq!(modifiers(&ctrl_c), vec![(CONTROL_ID, CONTROL_L)]);

        let ctrl_shift_c = stroke("c", Some("C"), true, false, true, false);
        assert_eq!(
            modifiers(&ctrl_shift_c),
            vec![(CONTROL_ID, CONTROL_L), (SHIFT_ID, SHIFT_L)]
        );
    }

    #[test]
    fn shift_is_replayed_for_named_keys_but_not_for_typed_characters() {
        let shifted_letter = stroke("a", Some("A"), false, false, true, false);
        assert!(modifiers(&shifted_letter).is_empty());

        let shifted_left = stroke("left", None, false, false, true, false);
        assert_eq!(modifiers(&shifted_left), vec![(SHIFT_ID, SHIFT_L)]);
    }

    #[test]
    fn unicode_becomes_a_unicode_keysym() {
        assert_eq!(
            keysym(&stroke("é", Some("é"), false, false, false, false)),
            Some(0xe9)
        );
        assert_eq!(
            keysym(&stroke("中", Some("中"), false, false, false, false)),
            Some(0x0100_0000 | 0x4e2d)
        );
    }

    #[test]
    fn key_identities_are_stable_and_distinct() {
        assert_eq!(key_id("a"), key_id("a"));
        assert_ne!(key_id("a"), key_id("b"));
        assert_ne!(key_id("left"), key_id("right"));
    }
}
