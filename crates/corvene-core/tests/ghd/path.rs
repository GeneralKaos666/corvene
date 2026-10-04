//! Port of GitHub Desktop's `app/test/unit/path-test.ts` (`lib/path.ts`).
//!
//! - `resolveWithin(rootPath, ...pathSegments)` is
//!   `corvene_core::app_url::resolve_within(root, relative)` (the
//!   `openRepo` URL's `filepath`, as in GitHub Desktop); the cases pass one
//!   segment, joined as Node's `path.join` does.
//! - `encodePathAsUrl(...pathSegments)` is `pathToFileURL(Path.resolve(…))`,
//!   the `file:` URL of a bundled image for GitHub Desktop's renderer:
//!   `corvene_platform::file_url::encode_path_as_url(&segments)` (Corvene
//!   embeds its images; its `file_uri` serves the Linux FileManager1
//!   reveal).
//!
//! `process.cwd()` is the test binary's working directory (the crate
//! directory under cargo).

use std::path::{Path, PathBuf};

use corvene_core::app_url::resolve_within;
#[cfg(any(windows, target_os = "macos", target_os = "linux"))]
use corvene_platform::file_url::encode_path_as_url;

/// Node's `path.join` of the segments (no `.` or `..` collapsing needed for
/// the cases' segments).
fn join(segments: &[&str]) -> String {
    segments
        .iter()
        .collect::<PathBuf>()
        .to_string_lossy()
        .into_owned()
}

fn basename(path: &Path) -> &str {
    path.file_name()
        .and_then(|n| n.to_str())
        .expect("a file name")
}

/// `const root = process.cwd()`
fn root() -> PathBuf {
    std::env::current_dir().expect("the working directory")
}

// GHD: unit/path-test.ts › path › encodePathAsUrl › normalizes path separators on Windows
#[cfg(windows)]
#[test]
fn normalizes_path_separators_on_windows() {
    let dir_name = "C:/Users/shiftkey\\AppData\\Local\\GitHubDesktop\\app-1.0.4\\resources\\app";
    let uri = encode_path_as_url(&[dir_name, "folder/file.html"]);
    assert!(uri.starts_with("file:///C:/Users/shiftkey/AppData/Local/"));
}

// GHD: unit/path-test.ts › path › encodePathAsUrl › encodes spaces and hashes
#[cfg(windows)]
#[test]
fn encodes_spaces_and_hashes_windows() {
    let dir_name = "C:/Users/The Kong #2\\AppData\\Local\\GitHubDesktop\\app-1.0.4\\resources\\app";
    let uri = encode_path_as_url(&[dir_name, "index.html"]);
    assert!(uri.starts_with("file:///C:/Users/The%20Kong%20%232/"));
}

// GHD: unit/path-test.ts › path › encodePathAsUrl › encodes spaces and hashes #2
#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn encodes_spaces_and_hashes() {
    let dir_name = "/Users/The Kong #2/AppData/Local/GitHubDesktop/app-1.0.4/resources/app";
    let uri = encode_path_as_url(&[dir_name, "index.html"]);
    assert!(uri.starts_with("file:///Users/The%20Kong%20%232/"));
}

// GHD: unit/path-test.ts › path › resolveWithin › fails for paths outside of the root
#[test]
fn fails_for_paths_outside_of_the_root() {
    let root = root();
    assert!(resolve_within(&root, &join(&[".."])).is_none());
    assert!(resolve_within(&root, &join(&["..", ".."])).is_none());
}

// GHD: unit/path-test.ts › path › resolveWithin › succeeds for paths that traverse out, and then back into, the root
#[test]
fn succeeds_for_paths_that_traverse_out_and_then_back_into_the_root() {
    let root = root();
    assert_eq!(
        resolve_within(&root, &join(&["..", basename(&root)])),
        Some(root.clone())
    );
}

// GHD: unit/path-test.ts › path › resolveWithin › fails for paths containing null bytes
#[test]
fn fails_for_paths_containing_null_bytes() {
    let root = root();
    assert!(resolve_within(&root, "foo\0bar").is_none());
}

// GHD: unit/path-test.ts › path › resolveWithin › succeeds for absolute relative paths as long as they stay within the root
#[test]
fn succeeds_for_absolute_relative_paths_as_long_as_they_stay_within_the_root() {
    let root = root();
    // `resolve(root, '..')`
    let parent = root.parent().expect("a parent directory");
    assert_eq!(
        resolve_within(parent, &root.to_string_lossy()),
        Some(root.clone())
    );
}

/// `mkdtemp(join(tmpdir(), 'path-test'))`
#[cfg(not(windows))]
fn mkdtemp_path_test() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("path-test")
        .tempdir_in(std::env::temp_dir())
        .expect("create a temporary directory")
}

/// The test's `finally`: `unlink(symlinkPath)` before the directory goes.
#[cfg(not(windows))]
struct UnlinkOnDrop(PathBuf);

#[cfg(not(windows))]
impl Drop for UnlinkOnDrop {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

// GHD: unit/path-test.ts › path › resolveWithin › fails for paths that use a symlink to traverse outside of the root
#[cfg(not(windows))]
#[test]
fn fails_for_paths_that_use_a_symlink_to_traverse_outside_of_the_root() {
    let temp = mkdtemp_path_test();
    let temp_dir = temp.path();
    let symlink_name = "dangerzone";
    let symlink_path = temp_dir.join(symlink_name);

    // `resolve(tempDir, '..', '..')`
    let target = temp_dir
        .parent()
        .and_then(Path::parent)
        .expect("two parent directories");
    std::os::unix::fs::symlink(target, &symlink_path).expect("create the symlink");
    let _unlink = UnlinkOnDrop(symlink_path.clone());

    assert!(resolve_within(temp_dir, symlink_name).is_none());
}

// GHD: unit/path-test.ts › path › resolveWithin › succeeds for paths that use a symlink to traverse outside of the root and then back again
#[cfg(not(windows))]
#[test]
fn succeeds_for_paths_that_use_a_symlink_to_traverse_outside_of_the_root_and_then_back_again() {
    let temp = mkdtemp_path_test();
    let temp_dir = temp.path();
    let symlink_name = "dangerzone";
    let symlink_path = temp_dir.join(symlink_name);

    // `resolve(tempDir, '..', '..')`
    let target = temp_dir
        .parent()
        .and_then(Path::parent)
        .expect("two parent directories");
    std::os::unix::fs::symlink(target, &symlink_path).expect("create the symlink");
    let _unlink = UnlinkOnDrop(symlink_path.clone());

    let through_symlink_path = join(&[
        symlink_name,
        basename(temp_dir.parent().expect("a parent directory")),
        basename(temp_dir),
    ]);
    assert_eq!(
        resolve_within(temp_dir, &through_symlink_path),
        // `resolve(tempDir, throughSymlinkPath)`
        Some(temp_dir.join(&through_symlink_path))
    );
}
