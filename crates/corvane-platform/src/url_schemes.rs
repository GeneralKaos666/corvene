//! Corvane's URL schemes (`x-corvane`, `x-corvane-auth`) - GHD's
//! `setAsDefaultProtocolClient` at launch (`main-process/main.ts`).
//!
//! macOS: `Info.plist` declares them and LaunchServices routes them to the
//! bundle. Linux: the `.desktop` file declares them (`MimeType=
//! x-scheme-handler/…`); like Electron's `setAsDefaultProtocolClient`
//! (`xdg-settings set default-url-scheme-handler`), every launch makes
//! Corvane's entry the default handler with `xdg-mime` when that entry is
//! installed: by the `.deb`, or by Corvane itself when it runs from an
//! AppImage (`crate::desktop_entry`). A bare binary without an installed
//! entry registers nothing and signs in over the loopback callback instead.

#[cfg(not(target_os = "macos"))]
pub use crate::desktop_entry::DESKTOP_ID;

/// Whether an `x-corvane-auth://` callback from the browser reaches this
/// process (through the OS and, on Linux, the single-instance socket).
pub fn auth_callback_registered() -> bool {
    #[cfg(target_os = "macos")]
    {
        crate::app_location::running_bundle().is_some()
    }
    #[cfg(not(target_os = "macos"))]
    {
        default_handler("x-corvane-auth").as_deref() == Some(DESKTOP_ID)
    }
}

/// The installed `.desktop` file, searched like the desktop does
/// (`$XDG_DATA_HOME`, then `$XDG_DATA_DIRS`).
#[cfg(not(target_os = "macos"))]
pub fn installed_desktop_entry() -> Option<std::path::PathBuf> {
    let system = std::env::var("XDG_DATA_DIRS").ok();
    dirs::data_dir()
        .map(|home| crate::desktop_entry::user_entry_path(&home))
        .filter(|path| path.is_file())
        .or_else(|| crate::desktop_entry::system_entry(system.as_deref()))
}

/// `xdg-mime query default x-scheme-handler/<scheme>`
#[cfg(not(target_os = "macos"))]
fn default_handler(scheme: &str) -> Option<String> {
    let out = std::process::Command::new("xdg-mime")
        .args(["query", "default", &format!("x-scheme-handler/{scheme}")])
        .output()
        .ok()?;
    let id = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (out.status.success() && !id.is_empty()).then_some(id)
}

/// Make Corvane the handler of its schemes (blocking; run off the main
/// thread). An AppImage installs its `.desktop` file first; nothing happens
/// without an installed one.
#[cfg(not(target_os = "macos"))]
pub fn register() {
    crate::desktop_entry::ensure();
    if installed_desktop_entry().is_none() {
        tracing::debug!("no installed desktop entry: URL schemes not registered");
        return;
    }
    for scheme in crate::single_instance::SCHEMES {
        if default_handler(scheme).as_deref() == Some(DESKTOP_ID) {
            continue;
        }
        let status = std::process::Command::new("xdg-mime")
            .args(["default", DESKTOP_ID, &format!("x-scheme-handler/{scheme}")])
            .status();
        match status {
            Ok(s) if s.success() => tracing::info!(scheme, "registered as the URL handler"),
            Ok(s) => tracing::warn!(scheme, %s, "xdg-mime could not register the URL handler"),
            Err(err) => tracing::warn!(scheme, %err, "xdg-mime is not available"),
        }
    }
}

#[cfg(target_os = "macos")]
pub fn register() {}
