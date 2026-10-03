//! Port of GitHub Desktop's
//! `app/test/unit/ui/path-text-and-link-button-test.tsx` (`PathText` and
//! its helpers; the React update-depth case and `LinkButton` are skipped
//! in `tools/ghd-tests/skips/ui2.tsv`).
//!
//! Corvene equivalents of `ui/lib/path-text.tsx`:
//!
//! - `extract(normalizedPath)` (file name and directory, the directory
//!   keeping its trailing separator) is how Corvene's file rows split a
//!   path: `WorkingDirectoryFileChange::file_name` and `::directory`, the
//!   directory shown with the platform's separators through
//!   `corvene_ui::format::display_path` (`changes.rs`). Corvene keeps
//!   repository paths in git's form (`/`), so GitHub Desktop's Windows
//!   input `src\components\file.tsx` (a `Path.normalize`d path) is
//!   `src/components/file.tsx` here and the separators come back in the
//!   displayed directory.
//! - `PathText`'s `.dirname` / `.filename` spans are that directory and
//!   file name (the row's two text elements).
//! - `truncateMid(value, length)` and `truncatePath(path, length)` are
//!   `corvene_ui::path_text::truncate_mid` and `truncate_path`. The first
//!   case is split into a truncation test and an `extract` test.
//!
//! Not ported (React DOM only): "without a tooltip" (`[role="tooltip"]`
//! absent when the path fits; Corvene's rows have no truncation tooltip
//! element to look for) and `availableWidth` (a width in which the path
//! fits, so nothing is truncated).

use corvene_core::{DiffSelection, FileStatusKind, GitStatusEntry, WorkingDirectoryFileChange};
use corvene_test_support::working_directory_file_change;
use corvene_ui::format::display_path;
use corvene_ui::path_text::{truncate_mid, truncate_path};

/// A changed file at `path` (the changes list row's model), status
/// Modified with nothing else set.
fn file_change(path: &str) -> WorkingDirectoryFileChange {
    let mut file =
        working_directory_file_change(path, FileStatusKind::Modified, DiffSelection::all());
    file.status.working_tree = GitStatusEntry::Modified;
    file.status.code = ".M".to_string();
    file
}

/// GitHub Desktop's `extract(path)`: `(normalizedFileName,
/// normalizedDirectory)` as Corvene's file rows compute them.
fn extract(path: &str) -> (String, String) {
    let file = file_change(path);
    (file.file_name().to_string(), display_path(file.directory()))
}

// GHD: unit/ui/path-text-and-link-button-test.tsx › path text and link button surfaces › truncates text and paths using the exported helpers
#[test]
fn truncates_text_and_paths_using_the_exported_helpers() {
    assert_eq!(truncate_mid("abcdef", 4), "a…ef");
    assert_eq!(truncate_mid("abcdef", 1), "…");
    assert_eq!(
        truncate_path(
            if cfg!(windows) {
                "src\\components\\file.tsx"
            } else {
                "src/components/file.tsx"
            },
            12
        ),
        if cfg!(windows) {
            "sr…\\file.tsx"
        } else {
            "sr…/file.tsx"
        }
    );
}

// GHD: unit/ui/path-text-and-link-button-test.tsx › path text and link button surfaces › truncates text and paths using the exported helpers
#[test]
fn truncates_text_and_paths_using_the_exported_helpers_extract() {
    assert_eq!(
        extract("src/components/file.tsx"),
        (
            "file.tsx".to_string(),
            if cfg!(windows) {
                "src\\components\\"
            } else {
                "src/components/"
            }
            .to_string()
        )
    );
}

// GHD: unit/ui/path-text-and-link-button-test.tsx › path text and link button surfaces › renders path text without a tooltip when the full path fits
#[test]
fn renders_path_text_without_a_tooltip_when_the_full_path_fits() {
    let (filename, dirname) = extract("src/components/file.tsx");

    assert_eq!(
        dirname,
        if cfg!(windows) {
            "src\\components\\"
        } else {
            "src/components/"
        }
    );
    assert_eq!(filename, "file.tsx");
}
