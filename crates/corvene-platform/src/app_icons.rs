//! The icon of an installed application, for the editor and shell menus of
//! Settings › Integrations (flag `513-integration-app-icons`; Android's
//! launcher icons come from `editors::app_icon`).
//!
//! macOS: the Finder icon of the `.app` bundle the editor or shell lives in.
//! Linux: the `Icon=` of the desktop entry that starts the executable,
//! looked up in the icon themes like the desktop does (hicolor first).
//! Windows: the shell's large icon of the executable.
//!
//! The freedesktop lookup is plain path and file logic that compiles (and is
//! tested) on every OS.
#![allow(unexpected_cfgs)] // `objc` macros probe a `cargo-clippy` feature

use std::path::Path;

/// Encoded picture data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppIcon {
    pub bytes: Vec<u8>,
    pub format: IconFormat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconFormat {
    Png,
    Svg,
}

/// Pixels the icon is wanted at: a 16 pt menu picture on a 2× display.
#[cfg_attr(any(target_os = "android", windows), allow(dead_code))]
const SIZE: u32 = 32;

/// The icon of the application `path` (an editor's or shell's `path`)
/// belongs to. Blocking (file system, LaunchServices); run it off the main
/// thread. `None` when there is none to show.
pub fn icon(path: &Path) -> Option<AppIcon> {
    if path.as_os_str().is_empty() {
        return None;
    }
    platform_icon(path)
}

#[cfg(target_os = "macos")]
fn platform_icon(path: &Path) -> Option<AppIcon> {
    use std::ffi::{CString, c_void};

    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};

    #[link(name = "AppKit", kind = "framework")]
    unsafe extern "C" {}

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Rect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    }

    /// `NSBitmapImageFileTypePNG`
    const PNG: u64 = 4;

    // a shell launched through its executable (`kitty.app/Contents/MacOS/
    // kitty`) still shows its bundle's icon
    let bundle = path
        .ancestors()
        .find(|p| p.extension().is_some_and(|ext| ext == "app"))
        .unwrap_or(path);
    let c_path = CString::new(bundle.to_str()?).ok()?;
    let nil: *mut Object = std::ptr::null_mut();
    // SAFETY: Objective-C message sends on AppKit classes; the bitmap rep is
    // released here and everything else is autoreleased into the pool.
    unsafe {
        let pool: *mut Object = msg_send![class!(NSAutoreleasePool), new];
        let ns_path: *mut Object =
            msg_send![class!(NSString), stringWithUTF8String: c_path.as_ptr()];
        let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        let image: *mut Object = msg_send![workspace, iconForFile: ns_path];
        let mut bytes = None;
        if !image.is_null() {
            // the representation closest to SIZE × SIZE pixels
            let mut rect = Rect {
                x: 0.,
                y: 0.,
                w: f64::from(SIZE),
                h: f64::from(SIZE),
            };
            let cg: *mut c_void = msg_send![image, CGImageForProposedRect: &mut rect as *mut Rect context: nil hints: nil];
            if !cg.is_null() {
                let rep: *mut Object = msg_send![class!(NSBitmapImageRep), alloc];
                let rep: *mut Object = msg_send![rep, initWithCGImage: cg];
                if !rep.is_null() {
                    let properties: *mut Object = msg_send![class!(NSDictionary), dictionary];
                    let data: *mut Object =
                        msg_send![rep, representationUsingType: PNG properties: properties];
                    if !data.is_null() {
                        let len: usize = msg_send![data, length];
                        let ptr: *const u8 = msg_send![data, bytes];
                        if !ptr.is_null() && len > 0 {
                            bytes = Some(std::slice::from_raw_parts(ptr, len).to_vec());
                        }
                    }
                    let _: () = msg_send![rep, release];
                }
            }
        }
        let _: () = msg_send![pool, drain];
        bytes.map(|bytes| AppIcon {
            bytes,
            format: IconFormat::Png,
        })
    }
}

#[cfg(windows)]
fn platform_icon(path: &Path) -> Option<AppIcon> {
    use std::os::windows::ffi::OsStrExt;

    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, DeleteObject, GetDC,
        GetDIBits, GetObjectW, HBITMAP, HDC, ReleaseDC,
    };
    use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;
    use windows::Win32::UI::Shell::{SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON, SHGetFileInfoW};
    use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO};
    use windows::core::PCWSTR;

    /// Top-down 32-bit BGRA rows of `bitmap`, with its size.
    unsafe fn pixels(dc: HDC, bitmap: HBITMAP) -> Option<(u32, u32, Vec<u8>)> {
        let mut info = BITMAP::default();
        let read = unsafe {
            GetObjectW(
                bitmap.into(),
                std::mem::size_of::<BITMAP>() as i32,
                Some(&mut info as *mut BITMAP as *mut _),
            )
        };
        if read == 0 || info.bmWidth <= 0 || info.bmHeight <= 0 {
            return None;
        }
        let (width, height) = (info.bmWidth as u32, info.bmHeight as u32);
        let mut header = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                // negative: top-down rows
                biHeight: -(height as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut buffer = vec![0u8; (width * height * 4) as usize];
        let lines = unsafe {
            GetDIBits(
                dc,
                bitmap,
                0,
                height,
                Some(buffer.as_mut_ptr().cast()),
                &mut header,
                DIB_RGB_COLORS,
            )
        };
        (lines == height as i32).then_some((width, height, buffer))
    }

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut file_info = SHFILEINFOW::default();
    // SAFETY: Win32 calls with buffers sized as they ask; every handle made
    // here is released before returning.
    unsafe {
        let found = SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut file_info as *mut SHFILEINFOW),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        );
        let icon = file_info.hIcon;
        if found == 0 || icon.is_invalid() {
            return None;
        }
        let mut icon_info = ICONINFO::default();
        let got_info = GetIconInfo(icon, &mut icon_info).is_ok();
        let _ = DestroyIcon(icon);
        if !got_info {
            return None;
        }
        let dc = GetDC(None);
        let color = (!icon_info.hbmColor.is_invalid())
            .then(|| pixels(dc, icon_info.hbmColor))
            .flatten();
        let mask = (!icon_info.hbmMask.is_invalid())
            .then(|| pixels(dc, icon_info.hbmMask))
            .flatten();
        let _ = ReleaseDC(None, dc);
        if !icon_info.hbmColor.is_invalid() {
            let _ = DeleteObject(icon_info.hbmColor.into());
        }
        if !icon_info.hbmMask.is_invalid() {
            let _ = DeleteObject(icon_info.hbmMask.into());
        }
        let (width, height, mut bgra) = color?;
        // an icon without alpha takes its transparency from the AND mask
        // (set bits are transparent)
        if bgra.as_chunks::<4>().0.iter().all(|px| px[3] == 0) {
            let mask = mask.filter(|(w, h, _)| *w == width && *h >= height);
            for (ix, px) in bgra.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                let transparent = mask
                    .as_ref()
                    .is_some_and(|(_, _, m)| m.get(ix * 4).is_some_and(|&b| b != 0));
                px[3] = if transparent { 0 } else { 255 };
            }
        }
        for px in bgra.as_chunks_mut::<4>().0 {
            px.swap(0, 2);
        }
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(&bgra).ok()?;
        writer.finish().ok()?;
        Some(AppIcon {
            bytes,
            format: IconFormat::Png,
        })
    }
}

#[cfg(not(any(target_os = "macos", target_os = "android", windows)))]
fn platform_icon(path: &Path) -> Option<AppIcon> {
    use std::path::PathBuf;

    let home = dirs::home_dir();
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute());
    let data_dirs = std::env::var("XDG_DATA_DIRS").ok();
    let dirs = freedesktop::data_dirs(home.as_deref(), data_home, data_dirs.as_deref());
    let file = freedesktop::icon_file(path, &dirs, home.as_deref())?;
    let format = match file.extension().and_then(|ext| ext.to_str()) {
        Some("png") => IconFormat::Png,
        Some("svg") => IconFormat::Svg,
        _ => return None,
    };
    let bytes = std::fs::read(&file).ok()?;
    Some(AppIcon { bytes, format })
}

#[cfg(target_os = "android")]
fn platform_icon(_path: &Path) -> Option<AppIcon> {
    None
}

/// Desktop entries and icon themes (the Desktop Entry and Icon Theme
/// specifications, simplified: hicolor, then any other installed theme).
#[cfg_attr(
    any(target_os = "macos", target_os = "android", windows),
    allow(dead_code)
)]
mod freedesktop {
    use std::path::{Path, PathBuf};

    use super::SIZE;

    /// The `share` directories to search, most important first:
    /// `$XDG_DATA_HOME` (`~/.local/share`), `$XDG_DATA_DIRS` (the spec's
    /// default when unset or empty), then the Flatpak and Snap export
    /// directories a session not started through them may be missing.
    pub fn data_dirs(
        home: Option<&Path>,
        data_home: Option<PathBuf>,
        data_dirs: Option<&str>,
    ) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = Vec::new();
        let mut push = |dir: PathBuf| {
            if dir.is_absolute() && !out.contains(&dir) {
                out.push(dir);
            }
        };
        if let Some(dir) = data_home.or_else(|| home.map(|h| h.join(".local/share"))) {
            push(dir);
        }
        for dir in data_dirs
            .filter(|dirs| !dirs.is_empty())
            .unwrap_or("/usr/local/share:/usr/share")
            .split(':')
            .filter(|dir| !dir.is_empty())
        {
            push(PathBuf::from(dir));
        }
        if let Some(home) = home {
            push(home.join(".local/share/flatpak/exports/share"));
        }
        push(PathBuf::from("/var/lib/flatpak/exports/share"));
        push(PathBuf::from("/var/lib/snapd/desktop"));
        out
    }

    /// The keys of a desktop entry's `[Desktop Entry]` group this module
    /// reads.
    #[derive(Debug, Default, PartialEq, Eq)]
    pub struct Entry {
        pub exec: Option<String>,
        pub try_exec: Option<String>,
        pub icon: Option<String>,
    }

    pub fn parse_entry(text: &str) -> Entry {
        let mut entry = Entry::default();
        let mut in_group = false;
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                in_group = line == "[Desktop Entry]";
                continue;
            }
            if !in_group {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = Some(value.trim().to_string()).filter(|v| !v.is_empty());
            match key.trim() {
                "Exec" => entry.exec = value,
                "TryExec" => entry.try_exec = value,
                "Icon" => entry.icon = value,
                _ => {}
            }
        }
        entry
    }

    /// The program an `Exec=` line starts: its first word, quotes removed,
    /// past an `env VAR=value …` prefix (Snap's entries).
    pub fn exec_program(exec: &str) -> Option<String> {
        let mut words = Vec::new();
        let mut rest = exec.trim_start();
        while !rest.is_empty() && words.len() < 16 {
            let (word, tail) = if let Some(quoted) = rest.strip_prefix('"') {
                match quoted.find('"') {
                    Some(end) => (quoted[..end].replace("\\\\", "\\"), &quoted[end + 1..]),
                    None => (quoted.to_string(), ""),
                }
            } else {
                match rest.find(char::is_whitespace) {
                    Some(end) => (rest[..end].to_string(), &rest[end..]),
                    None => (rest.to_string(), ""),
                }
            };
            words.push(word);
            rest = tail.trim_start();
        }
        let mut words = words.into_iter();
        let mut word = words.next()?;
        if Path::new(&word)
            .file_name()
            .is_some_and(|name| name == "env")
        {
            word = words.find(|w| !w.contains('=') && !w.starts_with('-'))?;
        }
        Some(word)
    }

    /// How well the entry named `stem` matches the executable `exe`: 3 when
    /// it starts that very path, 2 a program of the same name, 1 when the
    /// entry is named after it (a Flatpak export's application id).
    pub fn score(entry: &Entry, stem: &str, exe: &Path) -> u8 {
        let Some(exe_name) = exe.file_name().and_then(|n| n.to_str()) else {
            return 0;
        };
        let programs = [
            entry.try_exec.clone(),
            entry.exec.as_deref().and_then(exec_program),
        ];
        let mut best = 0;
        for program in programs.iter().flatten() {
            let program = Path::new(program);
            if program == exe {
                best = best.max(3);
            } else if program.file_name().and_then(|n| n.to_str()) == Some(exe_name) {
                best = best.max(2);
            }
        }
        if stem == exe_name {
            best = best.max(1);
        }
        best
    }

    /// Every `.desktop` file under `<dir>/applications` (one level of
    /// subfolders, KDE's `kde4/`), in directory order.
    fn desktop_files(dirs: &[PathBuf]) -> Vec<PathBuf> {
        fn list(dir: &Path, depth: u8, out: &mut Vec<PathBuf>) {
            let Ok(read) = std::fs::read_dir(dir) else {
                return;
            };
            let mut paths: Vec<PathBuf> = read.flatten().map(|e| e.path()).collect();
            paths.sort();
            for path in paths {
                if path.extension().is_some_and(|ext| ext == "desktop") {
                    out.push(path);
                } else if depth > 0 && path.is_dir() {
                    list(&path, depth - 1, out);
                }
            }
        }
        let mut out = Vec::new();
        for dir in dirs {
            list(&dir.join("applications"), 1, &mut out);
        }
        out
    }

    /// The `Icon=` of the best-matching entry for `exe`; an earlier
    /// directory wins a tie.
    pub fn entry_icon(exe: &Path, dirs: &[PathBuf]) -> Option<String> {
        let mut best: Option<(u8, String)> = None;
        for file in desktop_files(dirs) {
            let Some(stem) = file.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let Ok(text) = std::fs::read_to_string(&file) else {
                continue;
            };
            let entry = parse_entry(&text);
            let Some(icon) = entry.icon.clone() else {
                continue;
            };
            let score = score(&entry, stem, exe);
            if score > 0 && best.as_ref().is_none_or(|(b, _)| score > *b) {
                best = Some((score, icon));
                if score == 3 {
                    break;
                }
            }
        }
        best.map(|(_, icon)| icon)
    }

    /// The file of the icon named `name` (or the path an `Icon=` gives):
    /// hicolor's bitmaps nearest [`SIZE`], its scalable icon, then other
    /// themes the same way, then `pixmaps`.
    pub fn find_icon(name: &str, dirs: &[PathBuf], home: Option<&Path>) -> Option<PathBuf> {
        let usable = |path: &Path| {
            path.extension()
                .is_some_and(|ext| ext == "png" || ext == "svg")
                && path.is_file()
        };
        let given = Path::new(name);
        if given.is_absolute() {
            return usable(given).then(|| given.to_path_buf());
        }
        let name = name
            .strip_suffix(".png")
            .or_else(|| name.strip_suffix(".svg"))
            .or_else(|| name.strip_suffix(".xpm"))
            .unwrap_or(name);
        let mut bases: Vec<PathBuf> = home.map(|h| h.join(".icons")).into_iter().collect();
        bases.extend(dirs.iter().map(|dir| dir.join("icons")));
        // nearest the wanted size first, larger before smaller
        let sizes = [SIZE, 48, 64, 128, 256, 512, 24, 22, 16];
        let in_theme = |theme: &Path| -> Option<PathBuf> {
            sizes
                .iter()
                .flat_map(|s| {
                    [
                        theme.join(format!("{s}x{s}/apps/{name}.png")),
                        // Breeze's layout
                        theme.join(format!("apps/{s}/{name}.svg")),
                    ]
                })
                .chain([
                    theme.join(format!("scalable/apps/{name}.svg")),
                    theme.join(format!("{SIZE}x{SIZE}/apps/{name}.svg")),
                ])
                .find(|path| usable(path))
        };
        if let Some(found) = bases
            .iter()
            .find_map(|base| in_theme(&base.join("hicolor")))
        {
            return Some(found);
        }
        for base in &bases {
            let Ok(read) = std::fs::read_dir(base) else {
                continue;
            };
            let mut themes: Vec<PathBuf> = read
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir() && !p.ends_with("hicolor"))
                .collect();
            themes.sort();
            if let Some(found) = themes.iter().find_map(|theme| in_theme(theme)) {
                return Some(found);
            }
        }
        dirs.iter()
            .flat_map(|dir| {
                [
                    dir.join(format!("pixmaps/{name}.png")),
                    dir.join(format!("pixmaps/{name}.svg")),
                ]
            })
            .find(|path| usable(path))
    }

    /// The icon file of the application whose executable is `exe`.
    pub fn icon_file(exe: &Path, dirs: &[PathBuf], home: Option<&Path>) -> Option<PathBuf> {
        find_icon(&entry_icon(exe, dirs)?, dirs, home)
    }
}

#[cfg(test)]
mod tests {
    use super::freedesktop::*;
    use std::path::{Path, PathBuf};

    #[test]
    fn exec_program_skips_env_and_quotes() {
        assert_eq!(
            exec_program("/usr/share/code/code %F").as_deref(),
            Some("/usr/share/code/code")
        );
        assert_eq!(
            exec_program(
                "env BAMF_DESKTOP_FILE_HINT=/x.desktop /snap/bin/code --force-user-env %F"
            )
            .as_deref(),
            Some("/snap/bin/code")
        );
        assert_eq!(
            exec_program("\"/opt/Sublime Text/sublime_text\" %F").as_deref(),
            Some("/opt/Sublime Text/sublime_text")
        );
        assert_eq!(exec_program("   ").as_deref(), None);
    }

    #[test]
    fn parse_entry_reads_only_the_main_group() {
        let entry = parse_entry(
            "[Desktop Entry]\nName=Code\nExec=code %F\nIcon=vscode\n\n\
             [Desktop Action new-empty-window]\nExec=code --new-window\nIcon=other\n",
        );
        assert_eq!(entry.exec.as_deref(), Some("code %F"));
        assert_eq!(entry.icon.as_deref(), Some("vscode"));
        assert_eq!(entry.try_exec, None);
    }

    #[test]
    fn score_prefers_the_exact_path() {
        let entry = parse_entry("[Desktop Entry]\nExec=/usr/bin/kitty\nIcon=kitty\n");
        assert_eq!(score(&entry, "kitty", Path::new("/usr/bin/kitty")), 3);
        assert_eq!(score(&entry, "kitty", Path::new("/opt/bin/kitty")), 2);
        let flatpak = parse_entry(
            "[Desktop Entry]\nExec=/usr/bin/flatpak run com.visualstudio.code\nIcon=com.visualstudio.code\n",
        );
        let exe = Path::new(
            "/var/lib/flatpak/app/com.visualstudio.code/current/active/export/bin/com.visualstudio.code",
        );
        assert_eq!(score(&flatpak, "com.visualstudio.code", exe), 1);
        assert_eq!(score(&flatpak, "other", exe), 0);
    }

    #[test]
    fn data_dirs_defaults_and_dedupes() {
        let dirs = data_dirs(Some(Path::new("/home/u")), None, Some(""));
        assert_eq!(dirs[0], PathBuf::from("/home/u/.local/share"));
        assert_eq!(dirs[1], PathBuf::from("/usr/local/share"));
        assert_eq!(dirs[2], PathBuf::from("/usr/share"));
        let dirs = data_dirs(None, None, Some("/usr/share:/usr/share:relative"));
        assert_eq!(
            dirs.iter()
                .filter(|d| **d == Path::new("/usr/share"))
                .count(),
            1
        );
        assert!(!dirs.iter().any(|d| d.is_relative()));
    }

    fn touch(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"x").unwrap();
    }

    #[test]
    fn icon_file_follows_the_entry_into_the_theme() {
        let tmp = tempfile::tempdir().unwrap();
        let share = tmp.path().join("share");
        let home = tmp.path().join("home");
        std::fs::create_dir_all(share.join("applications")).unwrap();
        std::fs::write(
            share.join("applications/code.desktop"),
            "[Desktop Entry]\nExec=/usr/share/code/code %F\nIcon=vscode\n",
        )
        .unwrap();
        std::fs::write(
            share.join("applications/org.gnome.Terminal.desktop"),
            "[Desktop Entry]\nExec=gnome-terminal --window\nIcon=org.gnome.Terminal\n",
        )
        .unwrap();
        touch(&share.join("icons/hicolor/256x256/apps/vscode.png"));
        touch(&share.join("icons/hicolor/32x32/apps/vscode.png"));
        touch(&share.join("icons/hicolor/scalable/apps/org.gnome.Terminal.svg"));
        let dirs = vec![share.clone()];
        assert_eq!(
            icon_file(Path::new("/usr/bin/code"), &dirs, Some(&home)),
            Some(share.join("icons/hicolor/32x32/apps/vscode.png"))
        );
        assert_eq!(
            icon_file(Path::new("/usr/bin/gnome-terminal"), &dirs, Some(&home)),
            Some(share.join("icons/hicolor/scalable/apps/org.gnome.Terminal.svg"))
        );
        assert_eq!(
            icon_file(Path::new("/usr/bin/xterm"), &dirs, Some(&home)),
            None
        );
    }

    #[test]
    fn find_icon_falls_back_to_other_themes_and_pixmaps() {
        let tmp = tempfile::tempdir().unwrap();
        let share = tmp.path().join("share");
        touch(&share.join("icons/breeze/apps/48/utilities-terminal.svg"));
        touch(&share.join("pixmaps/xterm-color.png"));
        let dirs = vec![share.clone()];
        assert_eq!(
            find_icon("utilities-terminal", &dirs, None),
            Some(share.join("icons/breeze/apps/48/utilities-terminal.svg"))
        );
        assert_eq!(
            find_icon("xterm-color.png", &dirs, None),
            Some(share.join("pixmaps/xterm-color.png"))
        );
        let absolute = share.join("pixmaps/xterm-color.png");
        assert_eq!(
            find_icon(absolute.to_str().unwrap(), &dirs, None),
            Some(absolute.clone())
        );
        assert_eq!(find_icon("missing", &dirs, None), None);
    }
}
