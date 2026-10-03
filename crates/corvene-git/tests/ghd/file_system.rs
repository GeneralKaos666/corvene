//! Port of GitHub Desktop's `app/test/unit/file-system-test.ts`
//! (`lib/file-system.ts`).
//!
//! - `getTempFilePath(name)` is Corvene's
//!   `corvene_git::rebase_ops::temp_file(prefix, contents)`: the temporary
//!   files squash and reorder hand to git (GitHub Desktop's `squashTodo`,
//!   `squashCommitMessage` and `reorderTodo`), named
//!   `corvene-<prefix>-<pid>-<nanoseconds>` in `std::env::temp_dir()` (Node's
//!   `os.tmpdir()`). Unlike GitHub Desktop's function it also creates the
//!   file, holding `contents`; [`get_temp_file_path`] passes no contents and
//!   removes the file again, since GitHub Desktop's never exists.
//! - `readPartialFile(path, start, end)` (the inclusive byte range syntax
//!   highlighting reads from the working copy, up to
//!   `MaxHighlightContentLength`) is `corvene_git::read_partial_file`
//!   ([`read_partial_file`]).

use std::path::{Path, PathBuf};

use corvene_test_support::create_temp_directory;

/// GitHub Desktop's `getTempFilePath(name)` through
/// `corvene_git::rebase_ops::temp_file`, which also creates the file.
fn get_temp_file_path(name: &str) -> PathBuf {
    let path = corvene_git::rebase_ops::temp_file(name, "").expect("create a temporary file");
    let _ = std::fs::remove_file(&path);
    path
}

/// `readPartialFile(path, start, end)`.
fn read_partial_file(path: &Path, start: u64, end: u64) -> Vec<u8> {
    corvene_git::read_partial_file(path, start, end).expect("readPartialFile")
}

// GHD: unit/file-system-test.ts › file-system › getTempFilePath › returns a path in the temp directory
#[test]
fn returns_a_path_in_the_temp_directory() {
    let result = get_temp_file_path("test-file");
    assert!(
        result
            .to_string_lossy()
            .starts_with(&*std::env::temp_dir().to_string_lossy())
    );
}

// GHD: unit/file-system-test.ts › file-system › getTempFilePath › includes the given name in the path
#[test]
fn includes_the_given_name_in_the_path() {
    let result = get_temp_file_path("my-temp-file");
    assert!(result.to_string_lossy().contains("my-temp-file"));
}

// GHD: unit/file-system-test.ts › file-system › getTempFilePath › generates unique paths on each call
#[test]
fn generates_unique_paths_on_each_call() {
    let a = get_temp_file_path("test");
    let b = get_temp_file_path("test");
    assert_ne!(a, b);
}

// GHD: unit/file-system-test.ts › file-system › readPartialFile › reads a specific range from a file
#[test]
fn reads_a_specific_range_from_a_file() {
    let temp_dir = create_temp_directory();
    let file_path = temp_dir.path().join("partial-read-test");
    std::fs::write(&file_path, "Hello, World!").unwrap();

    let result = read_partial_file(&file_path, 0, 4);
    assert_eq!(String::from_utf8_lossy(&result), "Hello");
}

// GHD: unit/file-system-test.ts › file-system › readPartialFile › reads from the middle of a file
#[test]
fn reads_from_the_middle_of_a_file() {
    let temp_dir = create_temp_directory();
    let file_path = temp_dir.path().join("partial-read-test-mid");
    std::fs::write(&file_path, "abcdefghij").unwrap();

    let result = read_partial_file(&file_path, 3, 6);
    assert_eq!(String::from_utf8_lossy(&result), "defg");
}

// GHD: unit/file-system-test.ts › file-system › readPartialFile › reads a single byte
#[test]
fn reads_a_single_byte() {
    let temp_dir = create_temp_directory();
    let file_path = temp_dir.path().join("partial-read-single");
    std::fs::write(&file_path, "X").unwrap();

    let result = read_partial_file(&file_path, 0, 0);
    assert_eq!(String::from_utf8_lossy(&result), "X");
}
