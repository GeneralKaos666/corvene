//! Android keyboard handling.
//!
//! This module provides keyboard input support for Android, mapping
//! Android NDK key codes and meta-state to GPUI `Keystroke` / `Modifiers`
//! types.  It mirrors the approach used by `gpui_linux::keyboard`.
//!
//! ## Key code sources
//!
//! Android key codes come from the NDK `AKeyEvent` API
//! (`<android/input.h>`).  The constants below match
//! `android.view.KeyEvent.KEYCODE_*`.
//!
//! ## Meta-state flags
//!
//! `AKeyEvent_getMetaState()` returns a bitmask of
//! `AMETA_*` flags defined in `<android/input.h>`.

use gpui::{Keystroke, Modifiers};

// ── Android meta-state flags (from <android/input.h>) ────────────────────────

/// No meta keys pressed.
#[allow(dead_code)]
pub const AMETA_NONE: i32 = 0;
/// Shift key is pressed.
pub const AMETA_SHIFT_ON: i32 = 0x01;
/// Alt key is pressed.
pub const AMETA_ALT_ON: i32 = 0x02;
/// Ctrl key is pressed.
pub const AMETA_CTRL_ON: i32 = 0x1000;
/// Meta (Windows/Command) key is pressed.
pub const AMETA_META_ON: i32 = 0x10000;
/// Caps Lock is on.
pub const AMETA_CAPS_LOCK_ON: i32 = 0x100000;
/// Function key is pressed.
pub const AMETA_FUNCTION_ON: i32 = 0x08;

// ── Android key codes (from <android/keycodes.h>) ────────────────────────────
// Only the subset GPUI cares about is listed here.

pub const AKEYCODE_UNKNOWN: i32 = 0;
pub const AKEYCODE_SOFT_LEFT: i32 = 1;
pub const AKEYCODE_SOFT_RIGHT: i32 = 2;
pub const AKEYCODE_HOME: i32 = 3;
pub const AKEYCODE_BACK: i32 = 4;
pub const AKEYCODE_0: i32 = 7;
pub const AKEYCODE_1: i32 = 8;
pub const AKEYCODE_2: i32 = 9;
pub const AKEYCODE_3: i32 = 10;
pub const AKEYCODE_4: i32 = 11;
pub const AKEYCODE_5: i32 = 12;
pub const AKEYCODE_6: i32 = 13;
pub const AKEYCODE_7: i32 = 14;
pub const AKEYCODE_8: i32 = 15;
pub const AKEYCODE_9: i32 = 16;
pub const AKEYCODE_STAR: i32 = 17;
pub const AKEYCODE_POUND: i32 = 18;
pub const AKEYCODE_DPAD_UP: i32 = 19;
pub const AKEYCODE_DPAD_DOWN: i32 = 20;
pub const AKEYCODE_DPAD_LEFT: i32 = 21;
pub const AKEYCODE_DPAD_RIGHT: i32 = 22;
pub const AKEYCODE_DPAD_CENTER: i32 = 23;
pub const AKEYCODE_A: i32 = 29;
pub const AKEYCODE_B: i32 = 30;
pub const AKEYCODE_C: i32 = 31;
pub const AKEYCODE_D: i32 = 32;
pub const AKEYCODE_E: i32 = 33;
pub const AKEYCODE_F: i32 = 34;
pub const AKEYCODE_G: i32 = 35;
pub const AKEYCODE_H: i32 = 36;
pub const AKEYCODE_I: i32 = 37;
pub const AKEYCODE_J: i32 = 38;
pub const AKEYCODE_K: i32 = 39;
pub const AKEYCODE_L: i32 = 40;
pub const AKEYCODE_M: i32 = 41;
pub const AKEYCODE_N: i32 = 42;
pub const AKEYCODE_O: i32 = 43;
pub const AKEYCODE_P: i32 = 44;
pub const AKEYCODE_Q: i32 = 45;
pub const AKEYCODE_R: i32 = 46;
pub const AKEYCODE_S: i32 = 47;
pub const AKEYCODE_T: i32 = 48;
pub const AKEYCODE_U: i32 = 49;
pub const AKEYCODE_V: i32 = 50;
pub const AKEYCODE_W: i32 = 51;
pub const AKEYCODE_X: i32 = 52;
pub const AKEYCODE_Y: i32 = 53;
pub const AKEYCODE_Z: i32 = 54;
pub const AKEYCODE_COMMA: i32 = 55;
pub const AKEYCODE_PERIOD: i32 = 56;
pub const AKEYCODE_ALT_LEFT: i32 = 57;
pub const AKEYCODE_ALT_RIGHT: i32 = 58;
pub const AKEYCODE_SHIFT_LEFT: i32 = 59;
pub const AKEYCODE_SHIFT_RIGHT: i32 = 60;
pub const AKEYCODE_TAB: i32 = 61;
pub const AKEYCODE_SPACE: i32 = 62;
pub const AKEYCODE_ENTER: i32 = 66;
pub const AKEYCODE_DEL: i32 = 67; // Backspace
pub const AKEYCODE_GRAVE: i32 = 68;
pub const AKEYCODE_MINUS: i32 = 69;
pub const AKEYCODE_EQUALS: i32 = 70;
pub const AKEYCODE_LEFT_BRACKET: i32 = 71;
pub const AKEYCODE_RIGHT_BRACKET: i32 = 72;
pub const AKEYCODE_BACKSLASH: i32 = 73;
pub const AKEYCODE_SEMICOLON: i32 = 74;
pub const AKEYCODE_APOSTROPHE: i32 = 75;
pub const AKEYCODE_SLASH: i32 = 76;
pub const AKEYCODE_AT: i32 = 77;
pub const AKEYCODE_ESCAPE: i32 = 111;
pub const AKEYCODE_FORWARD_DEL: i32 = 112; // Delete
pub const AKEYCODE_CTRL_LEFT: i32 = 113;
pub const AKEYCODE_CTRL_RIGHT: i32 = 114;
pub const AKEYCODE_CAPS_LOCK: i32 = 115;
pub const AKEYCODE_SCROLL_LOCK: i32 = 116;
pub const AKEYCODE_META_LEFT: i32 = 117;
pub const AKEYCODE_META_RIGHT: i32 = 118;
pub const AKEYCODE_FUNCTION: i32 = 119;
pub const AKEYCODE_SYSRQ: i32 = 120;
pub const AKEYCODE_BREAK: i32 = 121;
pub const AKEYCODE_MOVE_HOME: i32 = 122;
pub const AKEYCODE_MOVE_END: i32 = 123;
pub const AKEYCODE_INSERT: i32 = 124;
pub const AKEYCODE_PAGE_UP: i32 = 92;
pub const AKEYCODE_PAGE_DOWN: i32 = 93;
pub const AKEYCODE_F1: i32 = 131;
pub const AKEYCODE_F2: i32 = 132;
pub const AKEYCODE_F3: i32 = 133;
pub const AKEYCODE_F4: i32 = 134;
pub const AKEYCODE_F5: i32 = 135;
pub const AKEYCODE_F6: i32 = 136;
pub const AKEYCODE_F7: i32 = 137;
pub const AKEYCODE_F8: i32 = 138;
pub const AKEYCODE_F9: i32 = 139;
pub const AKEYCODE_F10: i32 = 140;
pub const AKEYCODE_F11: i32 = 141;
pub const AKEYCODE_F12: i32 = 142;
pub const AKEYCODE_NUM_LOCK: i32 = 143;
pub const AKEYCODE_NUMPAD_0: i32 = 144;
pub const AKEYCODE_NUMPAD_1: i32 = 145;
pub const AKEYCODE_NUMPAD_2: i32 = 146;
pub const AKEYCODE_NUMPAD_3: i32 = 147;
pub const AKEYCODE_NUMPAD_4: i32 = 148;
pub const AKEYCODE_NUMPAD_5: i32 = 149;
pub const AKEYCODE_NUMPAD_6: i32 = 150;
pub const AKEYCODE_NUMPAD_7: i32 = 151;
pub const AKEYCODE_NUMPAD_8: i32 = 152;
pub const AKEYCODE_NUMPAD_9: i32 = 153;
pub const AKEYCODE_NUMPAD_DIVIDE: i32 = 154;
pub const AKEYCODE_NUMPAD_MULTIPLY: i32 = 155;
pub const AKEYCODE_NUMPAD_SUBTRACT: i32 = 156;
pub const AKEYCODE_NUMPAD_ADD: i32 = 157;
pub const AKEYCODE_NUMPAD_DOT: i32 = 158;
pub const AKEYCODE_NUMPAD_ENTER: i32 = 160;
pub const AKEYCODE_VOLUME_UP: i32 = 24;
pub const AKEYCODE_VOLUME_DOWN: i32 = 25;
pub const AKEYCODE_VOLUME_MUTE: i32 = 164;
pub const AKEYCODE_MENU: i32 = 82;

// ── Android key actions ──────────────────────────────────────────────────────

/// `AKEY_EVENT_ACTION_DOWN`
pub const AKEY_EVENT_ACTION_DOWN: i32 = 0;
/// `AKEY_EVENT_ACTION_UP`
pub const AKEY_EVENT_ACTION_UP: i32 = 1;
/// `AKEY_EVENT_ACTION_MULTIPLE` — used for character repeat.
pub const AKEY_EVENT_ACTION_MULTIPLE: i32 = 2;

// ── Conversion helpers ───────────────────────────────────────────────────────

/// Convert an Android `meta_state` bitmask to GPUI [`Modifiers`].
pub fn android_meta_to_modifiers(meta_state: i32) -> Modifiers {
    Modifiers {
        control: meta_state & AMETA_CTRL_ON != 0,
        alt: meta_state & AMETA_ALT_ON != 0,
        shift: meta_state & AMETA_SHIFT_ON != 0,
        platform: meta_state & AMETA_META_ON != 0,
        function: meta_state & AMETA_FUNCTION_ON != 0,
    }
}

/// Check whether Caps Lock is active from a meta-state bitmask.
pub fn android_meta_caps_lock(meta_state: i32) -> bool {
    meta_state & AMETA_CAPS_LOCK_ON != 0
}

/// Convert an Android key code to a GPUI key name string.
///
/// Returns `None` for key codes that should be ignored (e.g. pure modifier
/// keys, volume buttons, etc.).
pub fn android_keycode_to_key(key_code: i32) -> Option<String> {
    let key = match key_code {
        // Letters
        AKEYCODE_A => "a",
        AKEYCODE_B => "b",
        AKEYCODE_C => "c",
        AKEYCODE_D => "d",
        AKEYCODE_E => "e",
        AKEYCODE_F => "f",
        AKEYCODE_G => "g",
        AKEYCODE_H => "h",
        AKEYCODE_I => "i",
        AKEYCODE_J => "j",
        AKEYCODE_K => "k",
        AKEYCODE_L => "l",
        AKEYCODE_M => "m",
        AKEYCODE_N => "n",
        AKEYCODE_O => "o",
        AKEYCODE_P => "p",
        AKEYCODE_Q => "q",
        AKEYCODE_R => "r",
        AKEYCODE_S => "s",
        AKEYCODE_T => "t",
        AKEYCODE_U => "u",
        AKEYCODE_V => "v",
        AKEYCODE_W => "w",
        AKEYCODE_X => "x",
        AKEYCODE_Y => "y",
        AKEYCODE_Z => "z",

        // Number row
        AKEYCODE_0 => "0",
        AKEYCODE_1 => "1",
        AKEYCODE_2 => "2",
        AKEYCODE_3 => "3",
        AKEYCODE_4 => "4",
        AKEYCODE_5 => "5",
        AKEYCODE_6 => "6",
        AKEYCODE_7 => "7",
        AKEYCODE_8 => "8",
        AKEYCODE_9 => "9",

        // Punctuation / symbols
        AKEYCODE_COMMA => ",",
        AKEYCODE_PERIOD => ".",
        AKEYCODE_SPACE => "space",
        AKEYCODE_GRAVE => "`",
        AKEYCODE_MINUS => "-",
        AKEYCODE_EQUALS => "=",
        AKEYCODE_LEFT_BRACKET => "[",
        AKEYCODE_RIGHT_BRACKET => "]",
        AKEYCODE_BACKSLASH => "\\",
        AKEYCODE_SEMICOLON => ";",
        AKEYCODE_APOSTROPHE => "'",
        AKEYCODE_SLASH => "/",
        AKEYCODE_AT => "@",
        AKEYCODE_STAR => "*",
        AKEYCODE_POUND => "#",

        // Navigation
        AKEYCODE_ENTER => "enter",
        AKEYCODE_ESCAPE => "escape",
        AKEYCODE_DEL => "backspace",
        AKEYCODE_FORWARD_DEL => "delete",
        AKEYCODE_TAB => "tab",
        AKEYCODE_DPAD_UP => "up",
        AKEYCODE_DPAD_DOWN => "down",
        AKEYCODE_DPAD_LEFT => "left",
        AKEYCODE_DPAD_RIGHT => "right",
        AKEYCODE_DPAD_CENTER => "enter",
        AKEYCODE_MOVE_HOME => "home",
        AKEYCODE_MOVE_END => "end",
        AKEYCODE_INSERT => "insert",
        AKEYCODE_PAGE_UP => "pageup",
        AKEYCODE_PAGE_DOWN => "pagedown",

        // Function keys
        AKEYCODE_F1 => "f1",
        AKEYCODE_F2 => "f2",
        AKEYCODE_F3 => "f3",
        AKEYCODE_F4 => "f4",
        AKEYCODE_F5 => "f5",
        AKEYCODE_F6 => "f6",
        AKEYCODE_F7 => "f7",
        AKEYCODE_F8 => "f8",
        AKEYCODE_F9 => "f9",
        AKEYCODE_F10 => "f10",
        AKEYCODE_F11 => "f11",
        AKEYCODE_F12 => "f12",

        // Numpad
        AKEYCODE_NUMPAD_0 => "0",
        AKEYCODE_NUMPAD_1 => "1",
        AKEYCODE_NUMPAD_2 => "2",
        AKEYCODE_NUMPAD_3 => "3",
        AKEYCODE_NUMPAD_4 => "4",
        AKEYCODE_NUMPAD_5 => "5",
        AKEYCODE_NUMPAD_6 => "6",
        AKEYCODE_NUMPAD_7 => "7",
        AKEYCODE_NUMPAD_8 => "8",
        AKEYCODE_NUMPAD_9 => "9",
        AKEYCODE_NUMPAD_DIVIDE => "/",
        AKEYCODE_NUMPAD_MULTIPLY => "*",
        AKEYCODE_NUMPAD_SUBTRACT => "-",
        AKEYCODE_NUMPAD_ADD => "+",
        AKEYCODE_NUMPAD_DOT => ".",
        AKEYCODE_NUMPAD_ENTER => "enter",

        // Menu key (context menu on some keyboards)
        AKEYCODE_MENU => "menu",

        // Ignore pure modifier keys, volume, etc.
        AKEYCODE_SHIFT_LEFT | AKEYCODE_SHIFT_RIGHT | AKEYCODE_CTRL_LEFT | AKEYCODE_CTRL_RIGHT
        | AKEYCODE_ALT_LEFT | AKEYCODE_ALT_RIGHT | AKEYCODE_META_LEFT | AKEYCODE_META_RIGHT
        | AKEYCODE_FUNCTION | AKEYCODE_CAPS_LOCK | AKEYCODE_NUM_LOCK | AKEYCODE_SCROLL_LOCK
        | AKEYCODE_VOLUME_UP | AKEYCODE_VOLUME_DOWN | AKEYCODE_VOLUME_MUTE | AKEYCODE_HOME
        | AKEYCODE_SYSRQ | AKEYCODE_BREAK => return None,

        // Unknown / unmapped
        _ => return None,
    };

    Some(key.to_string())
}

/// Build a GPUI [`Keystroke`] from an Android key event.
///
/// Returns `None` if the key code should be ignored (modifier-only keys, etc.).
///
/// `unicode_char` — the Unicode character the device's key character map
/// gives for the key and meta state. Pass `0` if none.
pub fn android_key_to_keystroke(
    key_code: i32,
    modifiers: Modifiers,
    unicode_char: u32,
) -> Option<Keystroke> {
    let key = android_keycode_to_key(key_code)?;

    // The character the key types: none while Ctrl or Meta is held (those
    // are shortcuts), else what the key character map says, else the key
    // itself.
    let key_char = if modifiers.control || modifiers.platform {
        None
    } else if unicode_char != 0 {
        char::from_u32(unicode_char)
            .filter(|c| !c.is_control())
            .map(|c| c.to_string())
    } else if key == "space" {
        Some(" ".to_string())
    } else if key.len() == 1 {
        let ch = key.chars().next().unwrap_or_default();
        if modifiers.shift && ch.is_ascii_alphabetic() {
            Some(ch.to_ascii_uppercase().to_string())
        } else {
            Some(key.clone())
        }
    } else {
        None
    };

    Some(Keystroke {
        modifiers,
        key,
        key_char,
    })
}

/// Create a [`Keystroke`] for a software-keyboard text insertion.
///
/// Software keyboards on Android deliver text as strings rather than
/// individual key events.  This helper creates a keystroke for a single
/// character.
pub fn character_to_keystroke(c: char) -> Keystroke {
    Keystroke {
        modifiers: Modifiers::default(),
        key: c.to_lowercase().to_string(),
        key_char: Some(c.to_string()),
    }
}

/// Create a backspace [`Keystroke`].
pub fn backspace_keystroke() -> Keystroke {
    Keystroke {
        modifiers: Modifiers::default(),
        key: "backspace".to_string(),
        key_char: None,
    }
}

/// Android keyboard layout descriptor.
///
/// Analogous to `LinuxKeyboardLayout` in `gpui_linux`.
#[derive(Clone, Debug)]
pub struct AndroidKeyboardLayout {
    name: String,
}

impl AndroidKeyboardLayout {
    /// Create a new keyboard layout with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

impl gpui::PlatformKeyboardLayout for AndroidKeyboardLayout {
    fn id(&self) -> &str {
        &self.name
    }

    fn name(&self) -> &str {
        &self.name
    }
}
