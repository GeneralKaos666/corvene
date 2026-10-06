//! Corvene (`117-file-icons`): the file type icon at the start of a file
//! row, and a tree's folder icons, from the built-in Octicons set or the
//! icon theme `corvene_core::file_icons` loaded. GHD's file rows have no
//! file icon (`ui/changes/changed-file.tsx`).

use corvene_core::AppState;
use corvene_core::file_icons::{BUILTIN, Icon, LoadedIconTheme, NONE};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::icons::{Octicon, octicon};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::zpx;

/// What draws the icons.
pub enum FileIcons {
    Builtin,
    Theme(std::sync::Arc<LoadedIconTheme>),
}

/// The icons to draw, `None` without the flag or with File icons set to
/// none.
pub fn file_icons(cx: &App) -> Option<FileIcons> {
    let s = AppState::try_global(cx)?.read(cx);
    if !s.flags.bool(corvene_core::flags::ids::FILE_ICONS) {
        return None;
    }
    match s.settings.file_icon_theme.as_str() {
        NONE => None,
        BUILTIN => Some(FileIcons::Builtin),
        // a theme still loading, or whose extension is gone or off: the
        // built-in set (Settings shows Octicons for it too)
        _ => Some(
            s.file_icon_theme
                .clone()
                .map_or(FileIcons::Builtin, FileIcons::Theme),
        ),
    }
}

fn size() -> Pixels {
    zpx(16.)
}

/// The icon of the file at `path` (repository-relative; a trailing `/` is
/// an untracked repository), tinted `color` where the icon takes a colour.
pub fn file_icon(icons: &FileIcons, path: &str, color: Hsla, cx: &App) -> AnyElement {
    let name = corvene_core::file_name_and_directory(path).0;
    match icons {
        FileIcons::Builtin => octicon(builtin_icon(path, name), color).into_any_element(),
        FileIcons::Theme(theme) => {
            let light = !cx.ghd().is_dark();
            match theme.file_icon(name, light) {
                Some(icon) => theme_icon(icon, color),
                None => blank(),
            }
        }
    }
}

/// The icon of a folder row named `name` (its last folder).
pub fn folder_icon(
    icons: Option<&FileIcons>,
    name: &str,
    expanded: bool,
    color: Hsla,
    cx: &App,
) -> AnyElement {
    let octicon_folder = || {
        octicon(
            if expanded {
                Octicon::FileDirectoryOpenFill
            } else {
                Octicon::FileDirectoryFill
            },
            color,
        )
        .into_any_element()
    };
    match icons {
        Some(FileIcons::Theme(theme)) => {
            let light = !cx.ghd().is_dark();
            match theme.folder_icon(name, expanded, light) {
                Some(icon) => theme_icon(icon, color),
                None => octicon_folder(),
            }
        }
        _ => octicon_folder(),
    }
}

fn blank() -> AnyElement {
    div().size(size()).flex_none().into_any_element()
}

fn theme_icon(icon: &Icon, color: Hsla) -> AnyElement {
    match icon {
        Icon::Svg(path) | Icon::Image(path) => img(path.clone())
            .size(size())
            .flex_none()
            .into_any_element(),
        Icon::SvgMask(path) => svg()
            .external_path(path.to_string_lossy().into_owned())
            .size(size())
            .flex_none()
            .text_color(color)
            .into_any_element(),
        Icon::Glyph {
            svg: data,
            color: glyph_color,
        } => svg()
            .data(data)
            .size(size())
            .flex_none()
            .text_color(glyph_color.as_deref().and_then(parse_hex).unwrap_or(color))
            .into_any_element(),
    }
}

/// `#rgb`, `#rrggbb` or `#rrggbbaa`.
fn parse_hex(text: &str) -> Option<Hsla> {
    let hex = text.strip_prefix('#')?;
    let digits: Vec<u8> = match hex.len() {
        3 => hex
            .chars()
            .map(|c| c.to_digit(16).map(|d| (d * 17) as u8))
            .collect::<Option<_>>()?,
        6 | 8 => (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok())
            .collect::<Option<_>>()?,
        _ => return None,
    };
    let a = digits.get(3).copied().unwrap_or(255);
    Some(
        Rgba {
            r: digits[0] as f32 / 255.,
            g: digits[1] as f32 / 255.,
            b: digits[2] as f32 / 255.,
            a: a as f32 / 255.,
        }
        .into(),
    )
}

/// The built-in set: an Octicon by file name and extension.
pub fn builtin_icon(path: &str, name: &str) -> Octicon {
    if path.ends_with('/') {
        return Octicon::FileSubmodule;
    }
    let lower = name.to_lowercase();
    let stem = lower.split('.').next().unwrap_or("");
    match stem {
        "readme" => return Octicon::Book,
        "license" | "licence" | "copying" | "notice" => return Octicon::Law,
        "dockerfile" | "containerfile" => return Octicon::Container,
        _ => {}
    }
    if lower.ends_with(".lock") || lower == "package-lock.json" || lower == "go.sum" {
        return Octicon::Lock;
    }
    let ext = lower.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    match ext {
        "md" | "markdown" | "mdx" | "mdown" | "mkd" => Octicon::Markdown,
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" | "icns" | "svg" | "tif"
        | "tiff" | "heic" | "avif" | "psd" | "mp4" | "mov" | "webm" | "mp3" | "wav" | "ogg"
        | "flac" => Octicon::Image,
        "zip" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "7z" | "rar" | "zst" | "jar" | "vsix"
        | "dmg" | "pkg" | "deb" | "rpm" => Octicon::FileZip,
        "exe" | "dll" | "so" | "dylib" | "a" | "o" | "lib" | "bin" | "wasm" | "class" | "pdf"
        | "ttf" | "otf" | "woff" | "woff2" => Octicon::FileBinary,
        "sql" | "sqlite" | "db" | "sqlite3" => Octicon::Database,
        "rs" | "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "py" | "go" | "c" | "h" | "cc"
        | "cpp" | "hpp" | "cxx" | "cs" | "java" | "kt" | "kts" | "swift" | "m" | "mm" | "rb"
        | "php" | "lua" | "sh" | "bash" | "zsh" | "fish" | "ps1" | "scala" | "hs" | "ex"
        | "exs" | "erl" | "ml" | "clj" | "dart" | "vue" | "svelte" | "astro" | "css" | "scss"
        | "less" | "html" | "htm" | "json" | "jsonc" | "yaml" | "yml" | "toml" | "xml" | "zig"
        | "nim" | "r" | "jl" | "pl" | "graphql" | "proto" | "tf" | "nix" | "gradle" | "groovy"
        | "elm" | "fs" | "vb" | "sol" => Octicon::FileCode,
        _ if lower.starts_with('.') => Octicon::Gear,
        _ => Octicon::File,
    }
}

#[cfg(test)]
mod tests {
    use super::{builtin_icon, parse_hex};
    use crate::icons::Octicon;

    #[::core::prelude::v1::test]
    fn builtin_icons_by_name_and_extension() {
        assert_eq!(builtin_icon("src/main.rs", "main.rs"), Octicon::FileCode);
        assert_eq!(builtin_icon("README.md", "README.md"), Octicon::Book);
        assert_eq!(builtin_icon("docs/a.md", "a.md"), Octicon::Markdown);
        assert_eq!(builtin_icon("Cargo.lock", "Cargo.lock"), Octicon::Lock);
        assert_eq!(builtin_icon(".gitignore", ".gitignore"), Octicon::Gear);
        assert_eq!(builtin_icon("vendor/x/", "x"), Octicon::FileSubmodule);
        assert_eq!(builtin_icon("notes.txt", "notes.txt"), Octicon::File);
    }

    #[::core::prelude::v1::test]
    fn hex_colours() {
        assert!(parse_hex("#fff").is_some());
        assert!(parse_hex("#a074c4").is_some());
        assert!(parse_hex("red").is_none());
    }
}
