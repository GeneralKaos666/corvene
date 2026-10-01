//! Desktop integration for an AppImage (GHD ships no Linux build; Electron's
//! AppImages leave this to AppImageLauncher / appimaged).
//!
//! The `.deb` installs `com.wasimaster.corvane.desktop` and the icons under
//! `/usr/share`. An AppImage, downloaded by hand or moved to
//! `~/Applications` by the Homebrew cask, installs nothing, so there is no
//! launcher entry and [`crate::url_schemes::register`] has no entry to make
//! the `x-corvane://` / `x-corvane-auth://` handler. Every launch from an
//! AppImage (`$APPIMAGE`) therefore writes
//! `$XDG_DATA_HOME/applications/com.wasimaster.corvane.desktop` from
//! `packaging/linux/com.wasimaster.corvane.desktop` with `Exec` / `TryExec`
//! naming the image, and copies the icons from the mounted AppDir
//! (`$APPDIR/usr/share/icons`) to `$XDG_DATA_HOME/icons/hicolor`.
//!
//! Rules ([`plan`]):
//! - an entry this module wrote ([`MARKER`]) is rewritten when its contents
//!   differ (the image moved, the template changed);
//! - a user entry without the marker is somebody else's and is left alone;
//! - with no user entry, an entry in a system data directory (the `.deb`'s)
//!   wins and nothing is written. Nothing outside `$XDG_DATA_HOME` is ever
//!   touched;
//! - when another user entry already starts this image (AppImageLauncher's
//!   and appimaged's `appimagekit_*.desktop`, [`integrated_entry`]), that
//!   tool owns the launcher: nothing is written and an entry of ours is
//!   removed, so the menu never shows Corvane twice.
//!
//! A launch that is not from an AppImage removes an entry this module wrote
//! once its image is gone ([`remove_stale`]): it would shadow the `.deb`'s.
//!
//! Everything but [`ensure`] is plain path and file logic that compiles (and
//! is tested) on every OS.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The `.desktop` file's name (the GPUI `app_id` / `WM_CLASS`).
pub const DESKTOP_ID: &str = "com.wasimaster.corvane.desktop";

/// The packaged entry (`Exec=/usr/lib/corvane/corvane %U`, the `.deb`'s).
pub const TEMPLATE: &str = include_str!("../../../packaging/linux/com.wasimaster.corvane.desktop");

/// The line that marks an entry as written by this module.
pub const MARKER: &str = "X-Corvane-Installer=appimage";

/// The icons of `packaging/linux/package.sh`'s `stage`, relative to a
/// `share` directory (the AppDir's `usr/share`, `$XDG_DATA_HOME`).
pub const ICONS: [&str; 2] = [
    "icons/hicolor/256x256/apps/com.wasimaster.corvane.png",
    "icons/hicolor/scalable/apps/com.wasimaster.corvane.svg",
];

/// The image a `$APPIMAGE` value names: an absolute path. Unlike
/// `updater::running_appimage` it need not be replaceable (an image in a
/// read-only folder still gets a launcher) and symlinks are kept.
pub fn appimage_path(value: Option<OsString>) -> Option<PathBuf> {
    let path = PathBuf::from(value.filter(|v| !v.is_empty())?);
    path.is_absolute().then_some(path)
}

/// `<data_home>/applications/com.wasimaster.corvane.desktop`
pub fn user_entry_path(data_home: &Path) -> PathBuf {
    data_home.join("applications").join(DESKTOP_ID)
}

/// The entry in a system data directory (`$XDG_DATA_DIRS`, the spec's
/// default when unset or empty), if any.
pub fn system_entry(data_dirs: Option<&str>) -> Option<PathBuf> {
    data_dirs
        .filter(|dirs| !dirs.is_empty())
        .unwrap_or("/usr/local/share:/usr/share")
        .split(':')
        .filter(|dir| !dir.is_empty())
        .map(|dir| Path::new(dir).join("applications").join(DESKTOP_ID))
        .find(|path| path.is_file())
}

/// A desktop entry string value: backslashes doubled. `None` for a value a
/// one-line key cannot hold.
fn string_value(text: &str) -> Option<String> {
    (!text.contains(['\n', '\r'])).then(|| text.replace('\\', "\\\\"))
}

/// `path` as the program of an `Exec` key (Desktop Entry Specification, "The
/// Exec key"): quoted when it has a reserved character, with `"`, `` ` ``,
/// `$` and `\` escaped inside the quotes, `%` doubled, and the result
/// escaped once more as a string value.
pub fn exec_arg(path: &str) -> Option<String> {
    const RESERVED: &[char] = &[
        ' ', '\t', '"', '\'', '\\', '>', '<', '~', '|', '&', ';', '$', '*', '?', '#', '(', ')', '`',
    ];
    let arg = if path.contains(RESERVED) {
        let mut quoted = String::with_capacity(path.len() + 2);
        quoted.push('"');
        for c in path.chars() {
            if matches!(c, '"' | '`' | '$' | '\\') {
                quoted.push('\\');
            }
            quoted.push(c);
        }
        quoted.push('"');
        quoted
    } else {
        path.to_string()
    };
    string_value(&arg.replace('%', "%%"))
}

/// `template` with `Exec=<image> %U`, `TryExec=<image>` (the launcher goes
/// away with the image) and [`MARKER`]. `None` when the path cannot be
/// written into an entry (not UTF-8, a line break).
pub fn entry_contents(template: &str, image: &Path) -> Option<String> {
    let image = image.to_str()?;
    let exec = exec_arg(image)?;
    let try_exec = string_value(image)?;
    let mut out = String::with_capacity(template.len() + 2 * image.len());
    for line in template.lines() {
        if line.starts_with("Exec=") {
            out.push_str(&format!("Exec={exec} %U"));
        } else if line.starts_with("TryExec=") {
            out.push_str(&format!("TryExec={try_exec}"));
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out.push_str(MARKER);
    out.push('\n');
    Some(out)
}

/// Whether `contents` is an entry this module wrote.
pub fn is_ours(contents: &str) -> bool {
    contents.lines().any(|line| line.trim_end() == MARKER)
}

/// The image an entry of ours starts (its `TryExec`).
pub fn entry_image(contents: &str) -> Option<PathBuf> {
    let value = contents
        .lines()
        .find_map(|line| line.strip_prefix("TryExec="))?;
    Some(PathBuf::from(value.replace("\\\\", "\\")))
}

/// Another entry in `<data_home>/applications` whose `Exec` or `TryExec`
/// names `image`: the one AppImageLauncher or appimaged wrote for it.
pub fn integrated_entry(data_home: &Path, image: &Path) -> Option<PathBuf> {
    let image = image.to_str()?;
    let quoted = exec_arg(image)?;
    std::fs::read_dir(data_home.join("applications"))
        .ok()?
        .filter_map(|entry| Some(entry.ok()?.path()))
        .filter(|path| {
            path.extension().is_some_and(|ext| ext == "desktop")
                && path.file_name().is_some_and(|name| name != DESKTOP_ID)
        })
        .find(|path| {
            std::fs::read_to_string(path).is_ok_and(|contents| {
                contents
                    .lines()
                    .filter_map(|line| {
                        line.strip_prefix("Exec=")
                            .or_else(|| line.strip_prefix("TryExec="))
                    })
                    .any(|value| value.contains(image) || value.contains(&quoted))
            })
        })
}

/// What to do about the user's entry ([`plan`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plan {
    /// Write (or rewrite) the entry.
    Write,
    /// Our entry is already what it should be.
    UpToDate,
    /// The user's entry was not written by Corvane.
    Foreign,
    /// A system data directory has the entry (the `.deb`).
    System,
}

/// Decide from the user's entry (`None`: there is none), whether a system
/// data directory has one, and the entry [`entry_contents`] wants.
pub fn plan(user_entry: Option<&str>, system_entry: bool, wanted: &str) -> Plan {
    match user_entry {
        Some(existing) if !is_ours(existing) => Plan::Foreign,
        Some(existing) if existing == wanted => Plan::UpToDate,
        Some(_) => Plan::Write,
        None if system_entry => Plan::System,
        None => Plan::Write,
    }
}

/// Write `contents` to `path` through a temporary file and a rename.
fn write_atomic(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    std::fs::create_dir_all(dir)?;
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let temp = dir.join(format!(".{name}.{}", std::process::id()));
    std::fs::write(&temp, contents)?;
    std::fs::rename(&temp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&temp);
    })
}

/// Install the entry for `image` and the icons from `app_dir` (the mounted
/// AppDir) under `data_home`, per [`plan`]. `Ok(true)` when a file changed;
/// the icons are only touched while the entry is ours. With an
/// [`integrated_entry`] for the image, an entry of ours is removed instead.
pub fn install(
    data_home: &Path,
    system_entry: bool,
    image: &Path,
    app_dir: Option<&Path>,
) -> std::io::Result<bool> {
    let Some(wanted) = entry_contents(TEMPLATE, image) else {
        return Ok(false);
    };
    let path = user_entry_path(data_home);
    let existing = match std::fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        // unreadable or not text: not ours to replace
        Err(_) => return Ok(false),
    };
    if integrated_entry(data_home, image).is_some() {
        let ours = existing.as_deref().is_some_and(is_ours);
        if ours {
            std::fs::remove_file(&path)?;
        }
        return Ok(ours);
    }
    let mut changed = false;
    match plan(existing.as_deref(), system_entry, &wanted) {
        Plan::Foreign | Plan::System => return Ok(false),
        Plan::UpToDate => {}
        Plan::Write => {
            write_atomic(&path, wanted.as_bytes())?;
            changed = true;
        }
    }
    for icon in ICONS {
        let Some(source) = app_dir.map(|dir| dir.join("usr/share").join(icon)) else {
            break;
        };
        let Ok(bytes) = std::fs::read(&source) else {
            continue;
        };
        let target = data_home.join(icon);
        if std::fs::read(&target).is_ok_and(|current| current == bytes) {
            continue;
        }
        write_atomic(&target, &bytes)?;
        changed = true;
    }
    Ok(changed)
}

/// Remove the user's entry when it is ours and its image no longer exists
/// (the AppImage was deleted; the entry would shadow the `.deb`'s).
/// `Ok(true)` when it was removed.
pub fn remove_stale(data_home: &Path) -> std::io::Result<bool> {
    let path = user_entry_path(data_home);
    let Ok(contents) = std::fs::read_to_string(&path) else {
        return Ok(false);
    };
    if !is_ours(&contents) || entry_image(&contents).is_none_or(|image| image.exists()) {
        return Ok(false);
    }
    std::fs::remove_file(&path)?;
    Ok(true)
}

/// Keep the user's desktop entry in step with how this process was started
/// (blocking; run off the main thread): [`install`] for an AppImage,
/// [`remove_stale`] otherwise, then refresh the desktop's caches.
#[cfg(not(target_os = "macos"))]
pub fn ensure() {
    let Some(data_home) = dirs::data_dir() else {
        return;
    };
    let image = appimage_path(std::env::var_os("APPIMAGE")).filter(|image| image.is_file());
    let result = match &image {
        Some(image) => {
            let data_dirs = std::env::var("XDG_DATA_DIRS").ok();
            let app_dir = std::env::var_os("APPDIR").map(PathBuf::from);
            install(
                &data_home,
                system_entry(data_dirs.as_deref()).is_some(),
                image,
                app_dir.as_deref(),
            )
        }
        None => remove_stale(&data_home),
    };
    match result {
        Ok(false) => {}
        Ok(true) => {
            tracing::info!(entry = %user_entry_path(&data_home).display(), "desktop entry updated");
            // both are optional tools; the files alone are enough for most desktops
            let _ = std::process::Command::new("update-desktop-database")
                .arg("-q")
                .arg(data_home.join("applications"))
                .status();
            let _ = std::process::Command::new("gtk-update-icon-cache")
                .args(["-q", "-t", "-f"])
                .arg(data_home.join("icons/hicolor"))
                .status();
        }
        Err(err) => tracing::warn!(%err, "could not update the desktop entry"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const IMAGE: &str = "/home/me/Applications/Corvane.AppImage";

    fn wanted(image: &str) -> String {
        entry_contents(TEMPLATE, Path::new(image)).unwrap()
    }

    #[test]
    fn appimage_path_needs_an_absolute_path() {
        assert_eq!(appimage_path(Some(IMAGE.into())), Some(IMAGE.into()));
        assert_eq!(appimage_path(None), None);
        assert_eq!(appimage_path(Some("".into())), None);
        assert_eq!(appimage_path(Some("Corvane.AppImage".into())), None);
    }

    #[test]
    fn exec_arg_quotes_only_when_needed() {
        assert_eq!(exec_arg(IMAGE).unwrap(), IMAGE);
        assert_eq!(
            exec_arg("/home/me/My Apps/Corvane.AppImage").unwrap(),
            "\"/home/me/My Apps/Corvane.AppImage\""
        );
        // `$` and `\` are escaped in the quotes, then once more as a string
        assert_eq!(exec_arg("/a/$b").unwrap(), r#""/a/\\$b""#);
        assert_eq!(exec_arg(r"/a/b\c").unwrap(), r#""/a/b\\\\c""#);
        assert_eq!(exec_arg(r#"/a/"b"#).unwrap(), r#""/a/\\"b""#);
        // a field code look-alike
        assert_eq!(exec_arg("/a/100%U/c").unwrap(), "/a/100%%U/c");
        assert_eq!(exec_arg("/a/b\nc"), None);
    }

    #[test]
    fn entry_points_at_the_image_and_keeps_the_rest_of_the_template() {
        let entry = wanted(IMAGE);
        let lines: Vec<&str> = entry.lines().collect();
        assert_eq!(lines[0], "[Desktop Entry]");
        assert!(lines.contains(&format!("Exec={IMAGE} %U").as_str()));
        assert!(lines.contains(&format!("TryExec={IMAGE}").as_str()));
        assert!(!entry.contains("/usr/lib/corvane"));
        assert!(
            lines.contains(&"MimeType=x-scheme-handler/x-corvane;x-scheme-handler/x-corvane-auth;")
        );
        assert!(lines.contains(&"Icon=com.wasimaster.corvane"));
        assert_eq!(lines.len(), TEMPLATE.lines().count() + 1);
        assert!(is_ours(&entry));
        assert!(!is_ours(TEMPLATE));
        assert_eq!(entry_image(&entry), Some(PathBuf::from(IMAGE)));

        let spaced = wanted("/home/me/My Apps/Corvane.AppImage");
        assert!(spaced.contains("Exec=\"/home/me/My Apps/Corvane.AppImage\" %U\n"));
        assert!(spaced.contains("TryExec=/home/me/My Apps/Corvane.AppImage\n"));
        assert_eq!(
            entry_image(&wanted(r"/a/b\c")),
            Some(PathBuf::from(r"/a/b\c"))
        );
        assert_eq!(entry_contents(TEMPLATE, Path::new("/a/b\nc")), None);
    }

    #[test]
    fn plan_only_writes_what_is_ours_or_missing() {
        let entry = wanted(IMAGE);
        assert_eq!(plan(None, false, &entry), Plan::Write);
        assert_eq!(plan(None, true, &entry), Plan::System);
        assert_eq!(plan(Some(&entry), false, &entry), Plan::UpToDate);
        assert_eq!(plan(Some(&entry), true, &entry), Plan::UpToDate);
        let moved = wanted("/opt/Corvane.AppImage");
        assert_eq!(plan(Some(&moved), false, &entry), Plan::Write);
        assert_eq!(plan(Some(TEMPLATE), false, &entry), Plan::Foreign);
    }

    /// `<tmp>/share` (data home), `<tmp>/Corvane.AppImage` and a mounted
    /// AppDir with both icons.
    fn setup() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let data_home = tmp.path().join("share");
        let image = tmp.path().join("Corvane.AppImage");
        std::fs::write(&image, b"image").unwrap();
        let app_dir = tmp.path().join("mount");
        for icon in ICONS {
            let path = app_dir.join("usr/share").join(icon);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, icon.as_bytes()).unwrap();
        }
        (tmp, data_home, image, app_dir)
    }

    #[test]
    fn install_writes_the_entry_and_icons_once() {
        let (tmp, data_home, image, app_dir) = setup();
        assert!(install(&data_home, false, &image, Some(&app_dir)).unwrap());
        let entry = std::fs::read_to_string(user_entry_path(&data_home)).unwrap();
        assert_eq!(entry, entry_contents(TEMPLATE, &image).unwrap());
        for icon in ICONS {
            assert_eq!(
                std::fs::read(data_home.join(icon)).unwrap(),
                icon.as_bytes()
            );
        }
        // nothing to do the second time, and no temporary files are left
        assert!(!install(&data_home, false, &image, Some(&app_dir)).unwrap());
        let names: Vec<_> = std::fs::read_dir(data_home.join("applications"))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, [DESKTOP_ID]);

        // the image moved: the entry follows it
        let moved = tmp.path().join("Corvane-0.2.0-x86_64.AppImage");
        std::fs::rename(&image, &moved).unwrap();
        assert!(install(&data_home, false, &moved, Some(&app_dir)).unwrap());
        let entry = std::fs::read_to_string(user_entry_path(&data_home)).unwrap();
        assert_eq!(entry_image(&entry), Some(moved.clone()));

        // a new icon in the image replaces the installed one
        std::fs::write(app_dir.join("usr/share").join(ICONS[1]), b"new").unwrap();
        assert!(install(&data_home, false, &moved, Some(&app_dir)).unwrap());
        assert_eq!(std::fs::read(data_home.join(ICONS[1])).unwrap(), b"new");
        // without a mounted AppDir the entry is still kept up to date
        assert!(!install(&data_home, false, &moved, None).unwrap());
    }

    #[test]
    fn install_leaves_foreign_and_system_entries_alone() {
        let (_tmp, data_home, image, app_dir) = setup();
        // the .deb's entry exists: nothing is written
        assert!(!install(&data_home, true, &image, Some(&app_dir)).unwrap());
        assert!(!data_home.exists());

        // an entry somebody else put in the user's folder stays as it is
        let path = user_entry_path(&data_home);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, TEMPLATE).unwrap();
        assert!(!install(&data_home, false, &image, Some(&app_dir)).unwrap());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), TEMPLATE);
        assert!(!data_home.join("icons").exists());
        assert!(!remove_stale(&data_home).unwrap());
        assert!(path.exists());
    }

    #[test]
    fn another_tools_entry_for_the_image_replaces_ours() {
        let (tmp, data_home, image, app_dir) = setup();
        let applications = data_home.join("applications");
        std::fs::create_dir_all(&applications).unwrap();
        // an entry for some other image changes nothing
        let other = applications.join("appimagekit_0123-Other.desktop");
        std::fs::write(&other, "[Desktop Entry]\nExec=/opt/Other.AppImage %U\n").unwrap();
        assert_eq!(integrated_entry(&data_home, &image), None);
        assert!(install(&data_home, false, &image, Some(&app_dir)).unwrap());
        assert!(user_entry_path(&data_home).exists());
        // our own entry does not count as another tool's
        assert_eq!(integrated_entry(&data_home, &image), None);

        // AppImageLauncher integrates the image: ours goes away and stays away
        let theirs = applications.join("appimagekit_4567-Corvane.desktop");
        let contents = format!(
            "[Desktop Entry]\nExec={} %U\nTryExec={}\n",
            exec_arg(image.to_str().unwrap()).unwrap(),
            image.display()
        );
        std::fs::write(&theirs, contents).unwrap();
        assert_eq!(integrated_entry(&data_home, &image), Some(theirs.clone()));
        assert!(install(&data_home, false, &image, Some(&app_dir)).unwrap());
        assert!(!user_entry_path(&data_home).exists());
        assert!(!install(&data_home, false, &image, Some(&app_dir)).unwrap());
        assert!(!user_entry_path(&data_home).exists());
        assert!(theirs.exists());

        // a path with a space is quoted in `Exec`
        let spaced = tmp.path().join("My Apps/Corvane.AppImage");
        std::fs::write(
            &theirs,
            format!(
                "[Desktop Entry]\nExec={} %U\n",
                exec_arg(spaced.to_str().unwrap()).unwrap()
            ),
        )
        .unwrap();
        assert_eq!(integrated_entry(&data_home, &spaced), Some(theirs));
    }

    #[test]
    fn system_entry_searches_the_data_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let share = tmp.path().join("usr/share");
        let dirs = format!("{}:{}", tmp.path().join("none").display(), share.display());
        assert_eq!(system_entry(Some(&dirs)), None);
        let entry = share.join("applications").join(DESKTOP_ID);
        std::fs::create_dir_all(entry.parent().unwrap()).unwrap();
        std::fs::write(&entry, TEMPLATE).unwrap();
        assert_eq!(system_entry(Some(&dirs)), Some(entry));
    }

    #[test]
    fn a_stale_entry_of_ours_is_removed() {
        let (_tmp, data_home, image, _) = setup();
        assert!(install(&data_home, false, &image, None).unwrap());
        // the image is still there
        assert!(!remove_stale(&data_home).unwrap());
        std::fs::remove_file(&image).unwrap();
        assert!(remove_stale(&data_home).unwrap());
        assert!(!user_entry_path(&data_home).exists());
        assert!(!remove_stale(&data_home).unwrap());
    }
}
