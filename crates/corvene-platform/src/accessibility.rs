//! System accessibility display options (Corvene addition for the high
//! contrast theme; GHD has none): System Settings › Accessibility › Display
//! › "Increase contrast" on macOS; on Linux the XDG desktop portal's
//! `org.freedesktop.appearance` `contrast` setting (GNOME's Accessibility ›
//! High Contrast, KDE's high-contrast colour schemes).
//!
//! [`reduce_motion`] (`614-system-reduce-motion`) reads the system's Reduce
//! Motion / animation setting; Electron leaves it to the page's
//! `prefers-reduced-motion`, which GHD's stylesheets don't use.
//!
//! [`full_keyboard_access`] (`622-system-keyboard-navigation`) reads
//! macOS's Keyboard › Keyboard navigation, which Electron ignores
//! (desktop/desktop#4623).
#![allow(unexpected_cfgs)] // `objc` macros probe a `cargo-clippy` feature

/// `NSWorkspace.accessibilityDisplayShouldIncreaseContrast`
#[cfg(target_os = "macos")]
pub fn increase_contrast() -> bool {
    use objc::runtime::{BOOL, NO, Object};
    use objc::{class, msg_send, sel, sel_impl};

    #[link(name = "AppKit", kind = "framework")]
    unsafe extern "C" {}

    // SAFETY: a class message and a BOOL property read on the shared
    // workspace, which always exists in an AppKit process.
    unsafe {
        let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        if workspace.is_null() {
            return false;
        }
        let increase: BOOL = msg_send![workspace, accessibilityDisplayShouldIncreaseContrast];
        increase != NO
    }
}

/// `NSWorkspace.accessibilityDisplayShouldReduceMotion` (Accessibility ›
/// Display › Reduce motion).
#[cfg(target_os = "macos")]
pub fn reduce_motion() -> bool {
    use objc::runtime::{BOOL, NO, Object};
    use objc::{class, msg_send, sel, sel_impl};

    // SAFETY: as in `increase_contrast`
    unsafe {
        let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        if workspace.is_null() {
            return false;
        }
        let reduce: BOOL = msg_send![workspace, accessibilityDisplayShouldReduceMotion];
        reduce != NO
    }
}

/// `NSApplication.isFullKeyboardAccessEnabled`: System Settings ›
/// Keyboard › "Keyboard navigation" (Tab moves focus to every control, not
/// only text fields and lists). AppKit reads `AppleKeyboardUIMode` and
/// keeps the value current.
#[cfg(target_os = "macos")]
pub fn full_keyboard_access() -> bool {
    use objc::runtime::{BOOL, NO, Object};
    use objc::{class, msg_send, sel, sel_impl};

    // SAFETY: as in `increase_contrast`, on the shared application
    unsafe {
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        if app.is_null() {
            return true;
        }
        let enabled: BOOL = msg_send![app, isFullKeyboardAccessEnabled];
        enabled != NO
    }
}

/// Windows and Linux have no such setting: Tab reaches every control.
#[cfg(not(target_os = "macos"))]
pub fn full_keyboard_access() -> bool {
    true
}

/// Windows: Settings › Accessibility › Visual effects › Animation effects
/// is off (`SPI_GETCLIENTAREAANIMATION`).
#[cfg(windows)]
pub fn reduce_motion() -> bool {
    !crate::windows::client_area_animation()
}

/// GNOME's Accessibility › Reduce Animation (`enable-animations` false).
#[cfg(not(any(target_os = "macos", windows)))]
pub fn reduce_motion() -> bool {
    std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "enable-animations"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .is_some_and(|o| String::from_utf8_lossy(&o.stdout).trim() == "false")
}

/// Windows: a contrast theme is on.
#[cfg(windows)]
pub fn increase_contrast() -> bool {
    crate::windows::high_contrast()
}

/// Portal `Settings.ReadOne("org.freedesktop.appearance", "contrast")`:
/// `1` is higher contrast. `false` without a portal (or before it answers
/// within 250 ms, as this runs when the window is activated).
#[cfg(not(any(target_os = "macos", windows)))]
pub fn increase_contrast() -> bool {
    portal_contrast() == Some(1)
}

#[cfg(not(any(target_os = "macos", windows)))]
fn portal_contrast() -> Option<u32> {
    use std::sync::OnceLock;
    use std::time::Duration;

    use zbus::zvariant::OwnedValue;

    static BUS: OnceLock<Option<zbus::blocking::Connection>> = OnceLock::new();
    let bus = BUS
        .get_or_init(|| {
            zbus::blocking::connection::Builder::session()
                .ok()?
                .method_timeout(Duration::from_millis(250))
                .build()
                .ok()
        })
        .as_ref()?;
    let call = |method: &str| {
        bus.call_method(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            Some("org.freedesktop.portal.Settings"),
            method,
            &("org.freedesktop.appearance", "contrast"),
        )
    };
    // `ReadOne` (Settings v2) returns the value; the older `Read` wraps it
    // in one more variant
    let reply = call("ReadOne").or_else(|_| call("Read")).ok()?;
    let value: OwnedValue = reply.body().deserialize().ok()?;
    contrast_value(&value)
}

/// The `u` inside the reply, unwrapping `Read`'s extra variant.
#[cfg(not(any(target_os = "macos", windows)))]
fn contrast_value(value: &zbus::zvariant::Value<'_>) -> Option<u32> {
    use zbus::zvariant::Value;
    match value {
        Value::U32(v) => Some(*v),
        Value::Value(inner) => contrast_value(inner),
        _ => None,
    }
}

#[cfg(all(test, not(any(target_os = "macos", windows))))]
mod tests {
    use zbus::zvariant::Value;

    #[test]
    fn unwraps_both_portal_replies() {
        assert_eq!(super::contrast_value(&Value::U32(1)), Some(1));
        let read = Value::Value(Box::new(Value::U32(0)));
        assert_eq!(super::contrast_value(&read), Some(0));
        assert_eq!(super::contrast_value(&Value::Str("x".into())), None);
    }
}
