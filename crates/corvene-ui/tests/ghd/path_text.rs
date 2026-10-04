//! Port of GitHub Desktop's `app/test/unit/path-text-test.ts`
//! (`ui/lib/path-text.tsx`).
//!
//! - `extract(normalizedPath)` splits a changed file's path into the file
//!   name and the directory `PathText` draws dimmed. Corvene's changes,
//!   commit and stash rows (`corvene_ui::changes`, `selected_commit`,
//!   `stash_view`) draw `WorkingDirectoryFileChange::file_name` and
//!   `corvene_ui::format::display_path(…::directory())`; [`extract`] calls
//!   those on a file change with that path.
//! - `truncateMid` / `truncatePath` shorten the path to a number of
//!   characters, keeping the file name: `corvene_ui::path_text::truncate_mid`
//!   and `truncate_path`, which the rows' `PathText` uses.
//!
//! `PathText` normalizes git's `/`-separated path (`Path.normalize`, `\` on
//! Windows) before `extract`, so the Windows cases pass `\`-separated paths.
//! Corvene keeps git's path in the file change and only turns the directory
//! into the platform's separators (`display_path`), so [`extract`] hands it
//! the path in git's form: `\` back to `/` on Windows, a change of
//! representation only.

use corvene_core::{
    DiffSelection, FileStatus, FileStatusKind, GitStatusEntry, WorkingDirectoryFileChange,
};
use corvene_ui::format::display_path;
use corvene_ui::path_text::{truncate_mid, truncate_path};

/// The result of GitHub Desktop's `extract`.
struct Extracted {
    normalized_file_name: String,
    normalized_directory: String,
}

/// GitHub Desktop's `extract(normalizedPath)`: the file name and directory
/// halves Corvene's rows draw for a changed file at that path.
fn extract(normalized_path: &str) -> Extracted {
    // the file change holds git's `/`-separated path
    let git_path = if cfg!(windows) {
        normalized_path.replace('\\', "/")
    } else {
        normalized_path.to_string()
    };
    let change = WorkingDirectoryFileChange {
        path: git_path,
        old_path: None,
        status: FileStatus {
            kind: FileStatusKind::Untracked,
            index: GitStatusEntry::Untracked,
            working_tree: GitStatusEntry::Untracked,
            score: None,
            code: "??".to_string(),
            submodule: false,
            submodule_status: None,
            conflict_markers: None,
        },
        selection: DiffSelection::all(),
    };
    Extracted {
        normalized_file_name: change.file_name().to_string(),
        normalized_directory: display_path(change.directory()),
    }
}

// GHD: unit/path-text-test.ts › PathText › truncateMid › doesn't truncate if the string already fits
#[test]
fn truncate_mid_doesnt_truncate_if_the_string_already_fits() {
    assert_eq!(truncate_mid("foo", 3), "foo");
    assert_eq!(truncate_mid("foo", 10), "foo");
}

// GHD: unit/path-text-test.ts › PathText › truncateMid › returns an empty string if length is zero or less
#[test]
fn truncate_mid_returns_an_empty_string_if_length_is_zero_or_less() {
    assert_eq!(truncate_mid("foo", 0), "");
    assert_eq!(truncate_mid("foo", -10), "");
}

// GHD: unit/path-text-test.ts › PathText › truncateMid › returns an ellipsis if length is one
#[test]
fn truncate_mid_returns_an_ellipsis_if_length_is_one() {
    assert_eq!(truncate_mid("foo", 1), "…");
}

// GHD: unit/path-text-test.ts › PathText › truncateMid › truncates to the exact length given
#[test]
fn truncate_mid_truncates_to_the_exact_length_given() {
    assert_eq!(truncate_mid("foo bar", 6), "fo…bar");
    assert_eq!(truncate_mid("foo bar", 5), "fo…ar");
    assert_eq!(truncate_mid("foo bar", 3), "f…r");
}

// GHD: unit/path-text-test.ts › PathText › truncatePath › doesn't truncate if the string already fits
#[test]
fn truncate_path_doesnt_truncate_if_the_string_already_fits() {
    assert_eq!(truncate_path("foo", 3), "foo");
    assert_eq!(truncate_path("foo", 10), "foo");
}

// GHD: unit/path-text-test.ts › PathText › truncatePath › returns an empty string if length is zero or less
#[test]
fn truncate_path_returns_an_empty_string_if_length_is_zero_or_less() {
    assert_eq!(truncate_path("foo", 0), "");
    assert_eq!(truncate_path("foo", -10), "");
}

// GHD: unit/path-text-test.ts › PathText › truncatePath › returns an ellipsis if length is one
#[test]
fn truncate_path_returns_an_ellipsis_if_length_is_one() {
    assert_eq!(truncate_path("foo", 1), "…");
}

// GHD: unit/path-text-test.ts › PathText › truncatePath › truncates to the exact length given
#[test]
fn truncate_path_truncates_to_the_exact_length_given() {
    assert_eq!(truncate_path("foo bar", 6), "fo…bar");
    assert_eq!(truncate_path("foo bar", 5), "fo…ar");
    assert_eq!(truncate_path("foo bar", 3), "f…r");

    if cfg!(windows) {
        assert_eq!(truncate_path("foo\\foo bar", 6), "fo…bar");
        assert_eq!(truncate_path("foo\\foo bar", 9), "…\\foo bar");
    } else {
        assert_eq!(truncate_path("foo/foo bar", 6), "fo…bar");
        assert_eq!(truncate_path("foo/foo bar", 9), "…/foo bar");
    }
}

// GHD: unit/path-text-test.ts › PathText › truncatePath › favors truncation of directory components over file names
#[test]
fn favors_truncation_of_directory_components_over_file_names() {
    if cfg!(windows) {
        assert_eq!(
            truncate_path("alfa\\bravo\\charlie\\delta.txt", 25),
            "alfa\\bravo\\cha…\\delta.txt"
        );
        assert_eq!(
            truncate_path("alfa\\bravo\\charlie\\delta.txt", 22),
            "alfa\\bravo\\…\\delta.txt"
        );
        assert_eq!(
            truncate_path("alfa\\bravo\\charlie\\delta.txt", 17),
            "alfa\\b…\\delta.txt"
        );
    } else {
        assert_eq!(
            truncate_path("alfa/bravo/charlie/delta.txt", 25),
            "alfa/bravo/cha…/delta.txt"
        );
        assert_eq!(
            truncate_path("alfa/bravo/charlie/delta.txt", 22),
            "alfa/bravo/…/delta.txt"
        );
        assert_eq!(
            truncate_path("alfa/bravo/charlie/delta.txt", 17),
            "alfa/b…/delta.txt"
        );
    }
}

// GHD: unit/path-text-test.ts › PathText › extract › converts untracked submodule correctly
#[test]
fn converts_untracked_submodule_correctly() {
    let Extracted {
        normalized_file_name,
        normalized_directory,
    } = extract(if cfg!(windows) {
        "some\\submodule\\path\\"
    } else {
        "some/submodule/path/"
    });
    assert_eq!(normalized_file_name, "path");
    assert_eq!(
        normalized_directory,
        if cfg!(windows) {
            "some\\submodule\\"
        } else {
            "some/submodule/"
        }
    );
}

// GHD: unit/path-text-test.ts › PathText › extract › converts tracked submodule correctly
#[test]
fn converts_tracked_submodule_correctly() {
    let Extracted {
        normalized_file_name,
        normalized_directory,
    } = extract(if cfg!(windows) {
        "some\\submodule\\path"
    } else {
        "some/submodule/path"
    });
    assert_eq!(normalized_file_name, "path");
    assert_eq!(
        normalized_directory,
        if cfg!(windows) {
            "some\\submodule\\"
        } else {
            "some/submodule/"
        }
    );
}

// GHD: unit/path-text-test.ts › PathText › extract › converts file path correctly
#[test]
fn converts_file_path_correctly() {
    let Extracted {
        normalized_file_name,
        normalized_directory,
    } = extract(if cfg!(windows) {
        "some\\repository\\path.tsx"
    } else {
        "some/repository/path.tsx"
    });
    assert_eq!(normalized_file_name, "path.tsx");
    assert_eq!(
        normalized_directory,
        if cfg!(windows) {
            "some\\repository\\"
        } else {
            "some/repository/"
        }
    );
}
