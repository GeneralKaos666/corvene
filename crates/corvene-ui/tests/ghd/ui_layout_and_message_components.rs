//! Port of GitHub Desktop's
//! `app/test/unit/ui/layout-and-message-components-test.tsx`.
//!
//! Only "renders platform-specific keyboard shortcuts" is ported; the
//! `Row` / `Errors` wrappers and the `CommitWarning` variants are React DOM
//! checks (CSS classes, `role="alert"`, a cancelled `contextmenu` event)
//! and are skipped in `tools/ghd-tests/skips/ui2.tsv`.
//!
//! GitHub Desktop's `KeyboardShortcut` (`ui/keyboard-shortcut/
//! keyboard-shortcut.tsx`) takes the macOS keys (`darwinKeys`) and the
//! Windows / Linux keys (`keys`) and renders one `<kbd>` per key, joined by
//! `+` off macOS. Corvene's counterpart is `corvene_ui::widgets::kbd_group`,
//! which takes the macOS keys only and names each one for the platform
//! with `widgets::platform_key` (⌘ → Ctrl, ⇧ → Shift …), so the case's
//! `keys` are what `platform_key` derives from its `darwinKeys`. The caps'
//! texts are ported to `platform_key`; the shortcut's text (`textContent`,
//! the caps plus the `+` separators) is `widgets::keyboard_shortcut_text`,
//! the second half of the case.

use corvene_ui::widgets::{keyboard_shortcut_text, platform_key};

// GHD: unit/ui/layout-and-message-components-test.tsx › layout and message components › renders platform-specific keyboard shortcuts
#[test]
fn renders_platform_specific_keyboard_shortcuts() {
    // `darwinKeys={['⌘', '⇧', 'N']}`; `keys={['Ctrl', 'Shift', 'N']}` is
    // what `platform_key` makes of them off macOS
    let darwin_keys = ["⌘", "⇧", "N"];

    let keys: Vec<&str> = darwin_keys.iter().map(|&key| platform_key(key)).collect();

    assert_eq!(
        keys,
        if cfg!(target_os = "macos") {
            vec!["⌘", "⇧", "N"]
        } else {
            vec!["Ctrl", "Shift", "N"]
        }
    );
}

// GHD: unit/ui/layout-and-message-components-test.tsx › layout and message components › renders platform-specific keyboard shortcuts
#[test]
fn renders_platform_specific_keyboard_shortcuts_text() {
    let darwin_keys = ["⌘", "⇧", "N"];

    assert_eq!(
        keyboard_shortcut_text(&darwin_keys),
        if cfg!(target_os = "macos") {
            "⌘⇧N"
        } else {
            "Ctrl+Shift+N"
        }
    );
}
