//! Locate installed applications by bundle identifier and launch them -
//! GHD's `app-path` dependency (`LSCopyApplicationURLsForBundleIdentifier`)
//! and its `spawn('open', …)` calls.
#![allow(unexpected_cfgs)] // `objc` macros probe a `cargo-clippy` feature

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Where the app bundle with this identifier lives, per LaunchServices.
/// `None` when nothing registered on this machine claims the identifier.
#[cfg(target_os = "macos")]
pub fn app_path_for_bundle_id(bundle_id: &str) -> Option<PathBuf> {
    use std::ffi::{CStr, CString};
    use std::os::raw::c_char;

    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};

    // Force the AppKit framework to be linked so `NSWorkspace` resolves even
    // in processes (tests) that never touch the UI.
    #[link(name = "AppKit", kind = "framework")]
    unsafe extern "C" {}

    let c_id = CString::new(bundle_id).ok()?;
    // SAFETY: plain Objective-C message sends on well-known AppKit classes;
    // every returned object is autoreleased and drained with the pool below.
    unsafe {
        let pool: *mut Object = msg_send![class!(NSAutoreleasePool), new];
        let ns_id: *mut Object = msg_send![class!(NSString), stringWithUTF8String: c_id.as_ptr()];
        let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        let url: *mut Object = msg_send![workspace, URLForApplicationWithBundleIdentifier: ns_id];
        let result = if url.is_null() {
            None
        } else {
            let path: *mut Object = msg_send![url, path];
            let cstr: *const c_char = msg_send![path, UTF8String];
            if cstr.is_null() {
                None
            } else {
                Some(PathBuf::from(
                    CStr::from_ptr(cstr).to_string_lossy().into_owned(),
                ))
            }
        };
        let _: () = msg_send![pool, drain];
        result
    }
}

#[cfg(not(target_os = "macos"))]
pub fn app_path_for_bundle_id(_bundle_id: &str) -> Option<PathBuf> {
    None
}

/// First installed bundle out of the candidates, with the identifier that matched.
pub fn first_installed(bundle_ids: &[&str]) -> Option<(String, PathBuf)> {
    bundle_ids
        .iter()
        .find_map(|id| app_path_for_bundle_id(id).map(|p| (id.to_string(), p)))
}

/// Spawn a detached process (stdio ignored) so closing Corvene never takes
/// the launched editor or shell with it.
pub fn spawn_detached(program: impl AsRef<Path>, args: &[&str]) -> std::io::Result<()> {
    let mut command = Command::new(program.as_ref());
    // a `.cmd` launcher (VS Code's `code.cmd`) would flash a console
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(crate::windows::CREATE_NO_WINDOW);
    }
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(drop)
}

/// `open -a <app bundle> <target>` (Linux: `app` is the executable, run
/// with `target`).
pub fn open_with_app(app: &Path, target: &Path) -> std::io::Result<()> {
    if cfg!(target_os = "macos") {
        spawn_detached(
            "/usr/bin/open",
            &["-a", &app.to_string_lossy(), &target.to_string_lossy()],
        )
    } else {
        spawn_detached(app, &[&target.to_string_lossy()])
    }
}

/// `open -b <bundle id> <target>`
#[cfg(target_os = "macos")]
pub fn open_with_bundle(bundle_id: &str, target: &Path) -> std::io::Result<()> {
    spawn_detached(
        "/usr/bin/open",
        &["-b", bundle_id, &target.to_string_lossy()],
    )
}

/// Electron's `shell.showItemInFolder` on Linux (GHD `revealInFileManager`,
/// `platform_util_linux.cc`): `org.freedesktop.FileManager1.ShowItems` with
/// the item's URI so the file manager opens its folder with it selected;
/// without a file manager service, `xdg-open` on the folder.
#[cfg(not(target_os = "macos"))]
pub fn show_item_in_folder(path: &Path) -> std::io::Result<()> {
    // Android: the system's file manager on the folder, through the
    // documents provider
    #[cfg(target_os = "android")]
    {
        let dir = if path.is_dir() {
            path
        } else {
            path.parent().unwrap_or(path)
        };
        crate::android::view_path(dir)
    }
    #[cfg(windows)]
    {
        crate::windows::show_item_in_folder(path)
    }
    #[cfg(not(any(target_os = "android", windows)))]
    show_item_with_file_manager(path)
}

#[cfg(not(any(target_os = "macos", target_os = "android", windows)))]
fn show_item_with_file_manager(path: &Path) -> std::io::Result<()> {
    let shown = zbus::blocking::Connection::session().and_then(|bus| {
        bus.call_method(
            Some("org.freedesktop.FileManager1"),
            "/org/freedesktop/FileManager1",
            Some("org.freedesktop.FileManager1"),
            "ShowItems",
            &(vec![crate::file_url::file_uri(path)], ""),
        )
        .map(drop)
    });
    match shown {
        Ok(()) => Ok(()),
        Err(err) => {
            tracing::debug!(%err, "no FileManager1 service; opening the folder");
            let dir = if path.is_dir() {
                path
            } else {
                path.parent().unwrap_or(path)
            };
            spawn_detached("xdg-open", &[&dir.to_string_lossy()])
        }
    }
}

/// GHD `isApplicationBundleFromMetadata` (`lib/is-application-bundle.ts`):
/// whether Spotlight metadata (`mdls -name kMDItemContentType -name
/// kMDItemContentTypeTree` output) identifies an application bundle. Output
/// naming an application bundle, an application or an executable (quoted)
/// is one; a primary content type of `public.folder` is not; anything else
/// is inconclusive, an error (GHD throws).
pub fn is_application_bundle_from_metadata(metadata: &str) -> Result<bool, String> {
    const PROBABLE_BUNDLE_IDENTIFIERS: [&str; 3] = [
        "com.apple.application-bundle",
        "com.apple.application",
        "public.executable",
    ];
    if PROBABLE_BUNDLE_IDENTIFIERS
        .iter()
        .any(|id| metadata.contains(&format!("\"{id}\"")))
    {
        return Ok(true);
    }
    // `^[ \t]*kMDItemContentType\s*=\s*"([^"]+)"\s*$` on any line
    let primary = metadata.lines().find_map(|line| {
        let rest = line
            .trim_start_matches([' ', '\t'])
            .strip_prefix("kMDItemContentType")?;
        let value = rest.trim_start().strip_prefix('=')?.trim();
        let value = value.strip_prefix('"')?.strip_suffix('"')?;
        (!value.is_empty() && !value.contains('"')).then_some(value)
    });
    if primary == Some("public.folder") {
        return Ok(false);
    }
    Err("Metadata did not conclusively identify a directory".to_string())
}

/// GHD `isApplicationBundle(path)`: on macOS a directory can be an
/// application, which opening would run, so `mdls -name kMDItemContentType
/// -name kMDItemContentTypeTree <path>` is read by
/// [`is_application_bundle_from_metadata`]; always `false` elsewhere.
pub fn is_application_bundle(path: &Path) -> Result<bool, String> {
    if !cfg!(target_os = "macos") {
        return Ok(false);
    }
    let output = Command::new("/usr/bin/mdls")
        .args([
            "-name",
            "kMDItemContentType",
            "-name",
            "kMDItemContentTypeTree",
        ])
        .arg(path)
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    is_application_bundle_from_metadata(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn finds_finder_and_misses_nonsense() {
        let finder = app_path_for_bundle_id("com.apple.finder");
        assert!(finder.is_some_and(|p| p.ends_with("Finder.app")));
        assert_eq!(app_path_for_bundle_id("com.example.does-not-exist"), None);
    }
}
