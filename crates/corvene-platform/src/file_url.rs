//! `file:` URLs of local paths: GHD `encodePathAsUrl` (`app/src/lib/path.ts`,
//! Node's `pathToFileURL(Path.resolve(...))`) and the item URI Linux's
//! `org.freedesktop.FileManager1.ShowItems` takes
//! (`apps::show_item_in_folder`).
//!
//! GitHub Desktop builds such URLs for the images it bundles; Corvene embeds
//! its images, so only the file manager reveal uses [`file_uri`].

use std::path::{Component, Path, PathBuf};

/// GHD `encodePathAsUrl(...pathSegments)`: the segments resolved like
/// Node's `Path.resolve` (joined onto the working directory unless one is
/// absolute, `.` and `..` folded) as a `file:` URL ([`file_uri`]).
pub fn encode_path_as_url<P: AsRef<Path>>(path_segments: &[P]) -> String {
    file_uri(&resolve(path_segments))
}

/// Node's `Path.resolve`: from the right, segments are prepended until an
/// absolute path forms, then the working directory; the result is folded
/// lexically (no symlinks are read).
fn resolve<P: AsRef<Path>>(path_segments: &[P]) -> PathBuf {
    let mut joined = PathBuf::new();
    for segment in path_segments {
        // `PathBuf::push` of an absolute segment replaces what is there
        joined.push(segment.as_ref());
    }
    if !joined.is_absolute() {
        let cwd = std::env::current_dir().unwrap_or_default();
        joined = cwd.join(joined);
    }
    let mut out = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// The `file:` URL of an absolute path, as Node's `pathToFileURL` writes it:
/// `%`, controls, space, `"`, `#`, `<`, `>`, `?`, `` ` ``, `{`, `}` and every
/// non-ASCII byte (UTF-8) percent-encoded, plus `\` outside Windows. On
/// Windows the separators become `/` and a drive path gets a leading `/`
/// (`file:///C:/Users/…`); a UNC path keeps its server as the host.
pub fn file_uri(path: &Path) -> String {
    // a POSIX name is bytes, not necessarily UTF-8
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        format!("file://{}", encode(path.as_os_str().as_bytes(), true))
    }
    #[cfg(not(unix))]
    {
        file_uri_of_text(&path.to_string_lossy())
    }
}

/// [`file_uri`] of a path's text (Windows separators handled on Windows).
#[cfg(not(unix))]
fn file_uri_of_text(text: &str) -> String {
    #[cfg(windows)]
    let text = {
        let text = text.replace('\\', "/");
        // `\\?\C:\…` and `\\?\UNC\server\share\…` (verbatim paths)
        let text = match text.strip_prefix("//?/") {
            Some(rest) => match rest.strip_prefix("UNC/") {
                Some(unc) => format!("//{unc}"),
                None => rest.to_string(),
            },
            None => text,
        };
        match text.strip_prefix("//") {
            // UNC: `file://server/share/…`
            Some(unc) => {
                let (host, rest) = unc.split_once('/').unwrap_or((unc, ""));
                return format!(
                    "file://{}/{}",
                    encode(host.as_bytes(), false),
                    encode(rest.as_bytes(), false)
                );
            }
            None if text.starts_with('/') => text,
            None => format!("/{text}"),
        }
    };
    format!("file://{}", encode(text.as_bytes(), !cfg!(windows)))
}

/// Percent-encode a path for a `file:` URL (see [`file_uri`]);
/// `backslash`: also `\`, a plain character in POSIX names.
fn encode(path: &[u8], backslash: bool) -> String {
    let mut out = String::with_capacity(path.len());
    for &b in path {
        let escape = b <= b' '
            || b >= 0x7f
            || matches!(
                b,
                b'"' | b'#' | b'%' | b'<' | b'>' | b'?' | b'`' | b'{' | b'}'
            )
            || (backslash && b == b'\\');
        if escape {
            out.push_str(&format!("%{b:02X}"));
        } else {
            out.push(b as char);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn file_uris_are_escaped() {
        assert_eq!(
            file_uri(Path::new("/home/a b/ü#.txt")),
            "file:///home/a%20b/%C3%BC%23.txt"
        );
        assert_eq!(
            file_uri(Path::new("/a/100%/x?y\\z")),
            "file:///a/100%25/x%3Fy%5Cz"
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn segments_resolve_like_node() {
        assert_eq!(
            encode_path_as_url(&["/a/b", "../c", "./d.html"]),
            "file:///a/c/d.html"
        );
        assert_eq!(encode_path_as_url(&["/a", "/b", "c"]), "file:///b/c");
        let cwd = std::env::current_dir().unwrap();
        assert_eq!(encode_path_as_url(&["x"]), file_uri(&cwd.join("x")));
    }

    #[cfg(windows)]
    #[test]
    fn windows_paths_become_drive_urls() {
        assert_eq!(
            file_uri(Path::new(r"C:\Users\The Kong #2\a.txt")),
            "file:///C:/Users/The%20Kong%20%232/a.txt"
        );
        assert_eq!(
            file_uri(Path::new(r"\\server\share\a b")),
            "file://server/share/a%20b"
        );
        assert_eq!(file_uri(Path::new(r"\\?\C:\a")), "file:///C:/a");
    }
}
