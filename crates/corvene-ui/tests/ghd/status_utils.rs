//! Port of GitHub Desktop's `app/test/unit/status-utils-test.ts`.
//!
//! Corvene equivalents of GitHub Desktop's `lib/status.ts`:
//!
//! - `mapStatus(status)` is `corvene_ui::widgets::status_label(status.kind)`
//!   (the status text the file lists give VoiceOver). It takes the kind
//!   alone, which is all these cases vary.
//! - `hasConflictedFiles(workingDirectoryStatus)` is
//!   `WorkingDirectoryStatus::has_conflicts`.
//! - `isConflictedFile(status)` has no Corvene function (callers compare
//!   `status.kind` inline), so [`is_conflicted_file`] is a stand-in.
//! - `WorkingDirectoryStatus.fromFiles(files)` is a `WorkingDirectoryStatus`
//!   holding `files`; `makeFile(path, kind)` is [`make_file`].
//! - GitHub Desktop's status objects carry only the keys the tests set.
//!   Corvene's `FileStatus` has every field, so [`plain_status`] fills the
//!   ones GitHub Desktop leaves out with "nothing": no porcelain code,
//!   `Unchanged` on both sides, no score, no submodule and no conflict
//!   markers. A conflicted status (`entry: { action: 'both-modified', us:
//!   UpdatedButUnmerged, them: UpdatedButUnmerged }`) is the porcelain code
//!   `UU` with `Unmerged` on both sides.

use corvene_core::{
    DiffSelection, FileStatus, FileStatusKind, GitStatusEntry, WorkingDirectoryFileChange,
};
use corvene_test_support::from_files;
use corvene_ui::widgets::status_label;

/// GitHub Desktop's `mapStatus(status)`.
fn map_status(status: &FileStatus) -> &'static str {
    status_label(status.kind)
}

/// Stand-in for GitHub Desktop's `isConflictedFile(file)`
/// (`lib/status.ts`). Replace it with the Corvene function once there is
/// one and remove the `#[ignore]`s.
fn is_conflicted_file(_file: &FileStatus) -> bool {
    unimplemented!("Corvene has no isConflictedFile (callers compare status.kind inline)")
}

/// A status object with only `kind` set (`{ kind }`).
fn plain_status(kind: FileStatusKind) -> FileStatus {
    FileStatus {
        kind,
        index: GitStatusEntry::Unchanged,
        working_tree: GitStatusEntry::Unchanged,
        score: None,
        code: String::new(),
        submodule: false,
        submodule_status: None,
        conflict_markers: None,
    }
}

/// `{ kind: Conflicted, entry: { kind: 'conflicted', action:
/// 'both-modified', us: UpdatedButUnmerged, them: UpdatedButUnmerged },
/// conflictMarkerCount: 1 }`.
fn conflicted_status() -> FileStatus {
    FileStatus {
        kind: FileStatusKind::Conflicted,
        index: GitStatusEntry::Unmerged,
        working_tree: GitStatusEntry::Unmerged,
        score: None,
        code: "UU".to_string(),
        submodule: false,
        submodule_status: None,
        conflict_markers: Some(1),
    }
}

/// GitHub Desktop's `makeFile(path, kind)`.
fn make_file(path: &str, kind: FileStatusKind) -> WorkingDirectoryFileChange {
    let status = if kind == FileStatusKind::Conflicted {
        conflicted_status()
    } else {
        plain_status(kind)
    };

    WorkingDirectoryFileChange {
        path: path.to_string(),
        old_path: None,
        status,
        selection: DiffSelection::all(),
    }
}

// ---------------------------------------------------------------------------
// mapStatus
// ---------------------------------------------------------------------------

// GHD: unit/status-utils-test.ts › lib/status › mapStatus › returns "New" for new files
#[test]
fn returns_new_for_new_files() {
    assert_eq!(map_status(&plain_status(FileStatusKind::New)), "New");
}

// GHD: unit/status-utils-test.ts › lib/status › mapStatus › returns "New" for untracked files
#[test]
fn returns_new_for_untracked_files() {
    assert_eq!(map_status(&plain_status(FileStatusKind::Untracked)), "New");
}

// GHD: unit/status-utils-test.ts › lib/status › mapStatus › returns "Modified" for modified files
#[test]
fn returns_modified_for_modified_files() {
    assert_eq!(
        map_status(&plain_status(FileStatusKind::Modified)),
        "Modified"
    );
}

// GHD: unit/status-utils-test.ts › lib/status › mapStatus › returns "Deleted" for deleted files
#[test]
fn returns_deleted_for_deleted_files() {
    assert_eq!(
        map_status(&plain_status(FileStatusKind::Deleted)),
        "Deleted"
    );
}

// GHD: unit/status-utils-test.ts › lib/status › mapStatus › returns "Renamed" for renamed files
#[test]
fn returns_renamed_for_renamed_files() {
    // { kind: Renamed, oldPath: 'old.txt', renameIncludesModifications: false }
    // (Corvene keeps oldPath on the file change, not the status)
    assert_eq!(
        map_status(&plain_status(FileStatusKind::Renamed)),
        "Renamed"
    );
}

// GHD: unit/status-utils-test.ts › lib/status › mapStatus › returns "Copied" for copied files
#[test]
fn returns_copied_for_copied_files() {
    // { kind: Copied, oldPath: 'orig.txt', renameIncludesModifications: false }
    assert_eq!(map_status(&plain_status(FileStatusKind::Copied)), "Copied");
}

// ---------------------------------------------------------------------------
// isConflictedFile
// ---------------------------------------------------------------------------

// GHD: unit/status-utils-test.ts › lib/status › isConflictedFile › returns true for conflicted files
#[test]
#[ignore = "ghd: missing: Corvene has no isConflictedFile (lib/status.ts); callers compare status.kind inline"]
fn returns_true_for_conflicted_files() {
    let status = conflicted_status();
    assert!(is_conflicted_file(&status));
}

// GHD: unit/status-utils-test.ts › lib/status › isConflictedFile › returns false for non-conflicted files
#[test]
#[ignore = "ghd: missing: Corvene has no isConflictedFile (lib/status.ts); callers compare status.kind inline"]
fn returns_false_for_non_conflicted_files() {
    assert!(!is_conflicted_file(&plain_status(FileStatusKind::Modified)));
    assert!(!is_conflicted_file(&plain_status(FileStatusKind::New)));
    assert!(!is_conflicted_file(&plain_status(FileStatusKind::Deleted)));
}

// ---------------------------------------------------------------------------
// hasConflictedFiles
// ---------------------------------------------------------------------------

// GHD: unit/status-utils-test.ts › lib/status › hasConflictedFiles › returns false for an empty working directory
#[test]
fn returns_false_for_an_empty_working_directory() {
    let wd = from_files(vec![]);
    assert!(!wd.has_conflicts());
}

// GHD: unit/status-utils-test.ts › lib/status › hasConflictedFiles › returns false when no files are conflicted
#[test]
fn returns_false_when_no_files_are_conflicted() {
    let files = vec![
        make_file("a.txt", FileStatusKind::Modified),
        make_file("b.txt", FileStatusKind::New),
    ];
    let wd = from_files(files);
    assert!(!wd.has_conflicts());
}

// GHD: unit/status-utils-test.ts › lib/status › hasConflictedFiles › returns true when a file is conflicted
#[test]
fn returns_true_when_a_file_is_conflicted() {
    let files = vec![
        make_file("a.txt", FileStatusKind::Modified),
        make_file("b.txt", FileStatusKind::Conflicted),
    ];
    let wd = from_files(files);
    assert!(wd.has_conflicts());
}
