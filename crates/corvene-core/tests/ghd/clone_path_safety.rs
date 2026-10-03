//! Port of GitHub Desktop's `app/test/unit/clone-path-safety-test.ts`.
//!
//! - GitHub Desktop's `parseRepositoryIdentifier` (`lib/remote-parsing.ts`)
//!   is `corvene_core::clone_info::parse_repository_identifier`.
//! - GitHub Desktop's `sanitizeCloneName` (`lib/remote-parsing.ts`) names the
//!   folder the Clone dialog appends to the chosen directory: the last
//!   non-empty `/`, `\` or `:` separated component without `.git`, `null` for
//!   `..`, `.` and an empty name. Corvene's Clone dialog
//!   (`crates/corvene-ui/src/dialogs/clone_repository.rs`, `derived_path`)
//!   names that folder with `corvene_git::repository_name_from_url` (its doc:
//!   GHD `getDefaultDir`) and does no other sanitizing, so that function
//!   stands for `sanitizeCloneName` here: the last `/` or `:` separated
//!   segment (`\` too, on Windows only) without `.git`, `None` when empty.
//! - Node's `Path.join` / `Path.resolve` (the platform's rules) are
//!   `std::path::Path::join` and [`resolve`] (`.` and `..` removed
//!   lexically, as `resolve` does for an absolute path); `Path.win32.join` /
//!   `Path.win32.resolve`, which the Windows case uses on every platform, are
//!   [`win32_join`] / [`win32_resolve`] (the same for a drive-absolute
//!   `C:\…` path). The containment check stays a string prefix check.
//!
//! `#[rustfmt::skip]` keeps the Windows-only ignores on one line, the form
//! `tools/ghd-tests/check.py` reads.

use std::path::{Component, Path, PathBuf};

use corvene_core::clone_info::parse_repository_identifier;
use corvene_git::repository_name_from_url as sanitize_clone_name;

/// Node's `Path.resolve(path)` for an absolute `path`: `.` and `..`
/// removed lexically (`..` at the root stays at the root).
fn resolve(path: &Path) -> String {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out.to_string_lossy().into_owned()
}

/// Node's `Path.win32.join(base, name)` (normalised by
/// [`win32_resolve`] right after, as the test does).
fn win32_join(base: &str, name: &str) -> String {
    format!("{base}\\{name}")
}

/// Node's `Path.win32.resolve(path)` for a drive-absolute `path`
/// (`C:\…`): `\` and `/` both separate, `.` and `..` are removed
/// lexically, the result is joined with `\`.
fn win32_resolve(path: &str) -> String {
    assert_eq!(
        path.as_bytes().get(1),
        Some(&b':'),
        "not a drive path: {path}"
    );
    let (device, rest) = path.split_at(2);
    let mut parts: Vec<&str> = Vec::new();
    for part in rest.split(['\\', '/']) {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    format!("{device}\\{}", parts.join("\\"))
}

// GHD: unit/clone-path-safety-test.ts › sanitizeCloneName › returns a simple name unchanged
#[test]
fn returns_a_simple_name_unchanged() {
    assert_eq!(
        sanitize_clone_name("Hello-World").as_deref(),
        Some("Hello-World")
    );
}

// GHD: unit/clone-path-safety-test.ts › sanitizeCloneName › extracts last component from backslash-separated traversal
#[test]
#[rustfmt::skip]
#[cfg_attr(not(windows), ignore = "ghd: bug: repository_name_from_url splits on backslash only on Windows: returns the whole input, GHD sanitizeCloneName gives 'foo'")]
fn extracts_last_component_from_backslash_separated_traversal() {
    assert_eq!(
        sanitize_clone_name("x..\\..\\..\\..\\foo").as_deref(),
        Some("foo")
    );
}

// GHD: unit/clone-path-safety-test.ts › sanitizeCloneName › rejects names that resolve to .. or empty
#[test]
#[ignore = "ghd: bug: repository_name_from_url(\"..\") is Some(\"..\") (clone folder <dir>/..), GHD sanitizeCloneName gives null"]
fn rejects_names_that_resolve_to_dot_dot_or_empty() {
    assert_eq!(sanitize_clone_name(".."), None);
    assert_eq!(sanitize_clone_name(""), None);
    assert_eq!(sanitize_clone_name(".git"), None);
}

// GHD: unit/clone-path-safety-test.ts › sanitizeCloneName › does not traverse from default basepath (#x\..\..\..\.ssh)
#[test]
fn does_not_traverse_from_default_basepath() {
    assert_eq!(
        sanitize_clone_name("x..\\..\\..\\../.ssh").as_deref(),
        Some(".ssh")
    );
}

// GHD: unit/clone-path-safety-test.ts › clone path derivation with sanitizeCloneName › normal URLs are unchanged after sanitization
#[test]
fn normal_urls_are_unchanged_after_sanitization() {
    let urls = [
        "https://github.com/octocat/Hello-World.git",
        "git@github.com:octocat/Hello-World.git",
        "octocat/Hello-World",
    ];
    for url in urls {
        let result = parse_repository_identifier(url);
        assert!(result.is_some(), "Failed to parse: {url}");
        let result = result.unwrap();
        assert_eq!(
            sanitize_clone_name(&result.name).as_deref(),
            Some("Hello-World")
        );
    }
}

// GHD: unit/clone-path-safety-test.ts › clone path derivation with sanitizeCloneName › traversal payload clone path stays contained (POSIX)
#[test]
fn traversal_payload_clone_path_stays_contained_posix() {
    let result = parse_repository_identifier("https://evil.com/owner/x..\\..\\..\\.\\.ssh.git");
    assert!(result.is_some());
    let result = result.unwrap();
    let safe_name = sanitize_clone_name(&result.name);
    assert!(safe_name.is_some());
    let safe_name = safe_name.unwrap();
    let base_dir = "/Users/victim/Documents/GitHub";
    let resolved = resolve(&Path::new(base_dir).join(&safe_name));
    assert!(
        resolved.starts_with(&resolve(Path::new(base_dir))),
        "Clone path \"{resolved}\" escapes base dir"
    );
}

// GHD: unit/clone-path-safety-test.ts › clone path derivation with sanitizeCloneName › traversal payload clone path stays contained (Windows)
#[test]
#[rustfmt::skip]
#[cfg_attr(not(windows), ignore = "ghd: bug: off Windows repository_name_from_url keeps the backslash parts of the name, which escape the base under Windows path rules; GHD sanitizeCloneName gives '.ssh'")]
fn traversal_payload_clone_path_stays_contained_windows() {
    let result = parse_repository_identifier("https://evil.com/owner/x..\\..\\..\\.\\.ssh.git");
    assert!(result.is_some());
    let result = result.unwrap();
    let safe_name = sanitize_clone_name(&result.name);
    assert!(safe_name.is_some());
    let safe_name = safe_name.unwrap();
    let base_dir = "C:\\Users\\victim\\Documents\\GitHub";
    let resolved = win32_resolve(&win32_join(base_dir, &safe_name));
    assert!(
        resolved.starts_with(&win32_resolve(base_dir)),
        "Clone path \"{resolved}\" escapes base dir on Windows"
    );
}
