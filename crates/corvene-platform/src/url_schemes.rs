//! Corvene's URL schemes (`x-corvene`, `x-corvene-auth`) - GHD's
//! `setAsDefaultProtocolClient` at launch (`main-process/main.ts`).
//!
//! macOS: `Info.plist` declares them and LaunchServices routes them to the
//! bundle. Linux: the `.desktop` file declares them (`MimeType=
//! x-scheme-handler/…`); like Electron's `setAsDefaultProtocolClient`
//! (`xdg-settings set default-url-scheme-handler`), every launch makes
//! Corvene's entry the default handler with `xdg-mime` when that entry is
//! installed: by the `.deb`, or by Corvene itself when it runs from an
//! AppImage (`crate::desktop_entry`). A bare binary without an installed
//! entry registers nothing and signs in over the loopback callback instead.

#[cfg(not(any(target_os = "macos", windows)))]
pub use crate::desktop_entry::DESKTOP_ID;

/// Whether an `x-corvene-auth://` callback from the browser reaches this
/// process (through the OS and, on Linux, the single-instance socket).
pub fn auth_callback_registered() -> bool {
    #[cfg(target_os = "macos")]
    {
        crate::app_location::running_bundle().is_some()
    }
    // Android: the manifest's intent filter takes the scheme
    #[cfg(target_os = "android")]
    {
        true
    }
    #[cfg(windows)]
    {
        handled_by_this_exe("x-corvene-auth")
    }
    #[cfg(not(any(target_os = "macos", target_os = "android", windows)))]
    {
        default_handler("x-corvene-auth").as_deref() == Some(DESKTOP_ID)
    }
}

/// The installed `.desktop` file, searched like the desktop does
/// (`$XDG_DATA_HOME`, then `$XDG_DATA_DIRS`).
#[cfg(not(any(target_os = "macos", windows)))]
pub fn installed_desktop_entry() -> Option<std::path::PathBuf> {
    let system = std::env::var("XDG_DATA_DIRS").ok();
    dirs::data_dir()
        .map(|home| crate::desktop_entry::user_entry_path(&home))
        .filter(|path| path.is_file())
        .or_else(|| crate::desktop_entry::system_entry(system.as_deref()))
}

/// `xdg-mime query default x-scheme-handler/<scheme>`
#[cfg(not(any(target_os = "macos", windows)))]
fn default_handler(scheme: &str) -> Option<String> {
    let out = std::process::Command::new("xdg-mime")
        .args(["query", "default", &format!("x-scheme-handler/{scheme}")])
        .output()
        .ok()?;
    let id = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (out.status.success() && !id.is_empty()).then_some(id)
}

/// Make Corvene the handler of its schemes (blocking; run off the main
/// thread). An AppImage installs its `.desktop` file first; nothing happens
/// without an installed one.
#[cfg(not(any(target_os = "macos", windows)))]
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

/// Windows: whether the scheme's command in the registry runs this
/// executable.
#[cfg(windows)]
fn handled_by_this_exe(scheme: &str) -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let exe = exe.to_string_lossy().to_lowercase();
    crate::windows::url_scheme_command(scheme)
        .is_some_and(|command| command.to_lowercase().contains(&exe))
}

/// Windows: the schemes are per-user registry keys, which the installer
/// writes and every launch of an installed Corvene repairs (Electron's
/// `setAsDefaultProtocolClient`). A build that is not installed (`cargo
/// run`) only takes schemes nobody handles, so it does not take them from
/// the installed one.
#[cfg(windows)]
pub fn register() {
    // an instance with a data folder of its own (the parity harness) is not
    // the one links should open
    if std::env::var_os("CORVENE_DATA_DIR").is_some() {
        return;
    }
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let installed = exe
        .parent()
        .is_some_and(|dir| dir.join("unins000.exe").is_file());
    for scheme in crate::single_instance::SCHEMES {
        if handled_by_this_exe(scheme) {
            continue;
        }
        let taken = crate::windows::url_scheme_command(scheme).is_some_and(|command| {
            // `"<exe>" "%1"`
            command
                .split('"')
                .nth(1)
                .is_some_and(|path| std::path::Path::new(path).is_file())
        });
        if taken && !installed {
            continue;
        }
        match crate::windows::register_url_scheme(scheme, &exe) {
            Ok(()) => tracing::info!(scheme, "registered as the URL handler"),
            Err(err) => tracing::warn!(scheme, %err, "could not register the URL handler"),
        }
    }
}
