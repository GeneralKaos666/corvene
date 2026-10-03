//! Port of GitHub Desktop's
//! `app/test/unit/stores/updates/update-changed-files-test.ts`.
//!
//! GitHub Desktop's `updateChangedFiles(state, status, clearPartialState)`
//! (`lib/stores/updates/changes-state.ts`) merges a new status into the
//! changes state: each file keeps its previous selection (a partial one is
//! cleared to none when `clearPartialState`), the files are sorted by
//! `caseInsensitiveCompare`, the selected files that are gone are dropped
//! (the first file is selected when none is left), and the diff is kept
//! only when the same single file stays selected. Corvene does these steps
//! in two places and has no function taking a previous state and a status:
//! `corvene_git::get_status(git, path, previous)` carries the previous
//! selections over by path and sorts, on the output of `git status` it runs
//! itself; `Dispatcher::refresh_repository` (needs a gpui `App`) keeps or
//! replaces `RepositoryState::selected_file(s)` and drops the diff. Nothing
//! clears partial selections (a commit refreshes with the previous status).
//! [`update_changed_files`] is a stand-in returning the updated
//! `RepositoryState`.
//!
//! - `IChangesState` is Corvene's `RepositoryState`: `workingDirectory` →
//!   `status`, `selection.selectedFileIDs` → `selected_files` (with the
//!   first in `selected_file`), `selection.diff` → `diff`, `selection.kind`
//!   `WorkingDirectory` → `showing_stash == false`.
//! - `IStatusResult` (`createStatus`, `helpers/changes-state-helper.ts`) is
//!   a `WorkingDirectoryStatus` with those files.
//! - A file's `id` (`<kind>+<path>`) is its path: Corvene identifies changed
//!   files by path. `findFileWithID(id)` finds the file with that path.
//! - `DiffSelection.fromInitialSelection(All / None)` is
//!   `DiffSelection::all()` / `none()`, `withSelectableLines` /
//!   `withLineSelection` are `with_selectable_lines` / `with_line`,
//!   `getSelectionType()` is `kind()`.
//! - The `IBinaryDiff` `{ kind: DiffType.Binary }` is `Diff::Binary` behind
//!   an `Arc`; "the same diff" is the same `Arc`.

use std::collections::BTreeSet;
use std::sync::Arc;

use corvene_core::state::RepositoryState;
use corvene_core::{
    Diff, DiffSelection, DiffSelectionType, FileStatus, FileStatusKind, GitStatusEntry,
    WorkingDirectoryFileChange, WorkingDirectoryStatus,
};

/// Stand-in for GitHub Desktop's `updateChangedFiles(state, status,
/// clearPartialState)`: the changes state after merging `status` into
/// `state`. Replace it with a gpui-free Corvene function once there is one
/// and remove the `#[ignore]`s.
fn update_changed_files(
    _state: &RepositoryState,
    _status: &WorkingDirectoryStatus,
    _clear_partial_state: bool,
) -> RepositoryState {
    unimplemented!(
        "no gpui-free updateChangedFiles (corvene_git::get_status + Dispatcher::refresh_repository)"
    )
}

/// `new WorkingDirectoryFileChange(path, { kind: Modified | New }, selection)`.
fn file_change(
    path: &str,
    kind: FileStatusKind,
    selection: DiffSelection,
) -> WorkingDirectoryFileChange {
    let (index, working_tree, code) = match kind {
        FileStatusKind::New => (GitStatusEntry::Added, GitStatusEntry::Unchanged, "A."),
        _ => (GitStatusEntry::Unchanged, GitStatusEntry::Modified, ".M"),
    };
    WorkingDirectoryFileChange {
        path: path.to_string(),
        old_path: None,
        status: FileStatus {
            kind,
            index,
            working_tree,
            score: None,
            code: code.to_string(),
            submodule: false,
            submodule_status: None,
            conflict_markers: None,
        },
        selection,
    }
}

/// `allSelected`.
fn all_selected() -> DiffSelection {
    DiffSelection::all()
}

/// `noneSelected`.
fn none_selected() -> DiffSelection {
    DiffSelection::none()
}

/// The module's `files`: `README.md` (all selected) and `app/package.json`
/// (none selected), both modified.
fn files() -> Vec<WorkingDirectoryFileChange> {
    vec![
        file_change("README.md", FileStatusKind::Modified, all_selected()),
        file_change(
            "app/package.json",
            FileStatusKind::Modified,
            none_selected(),
        ),
    ]
}

/// `WorkingDirectoryStatus.fromFiles(files)`.
fn from_files(files: Vec<WorkingDirectoryFileChange>) -> WorkingDirectoryStatus {
    WorkingDirectoryStatus {
        files,
        ..Default::default()
    }
}

/// `selection: { kind: WorkingDirectory, selectedFileIDs, diff }`.
struct Selection {
    selected_file_ids: Vec<String>,
    diff: Option<Arc<Diff>>,
}

/// `createState({ workingDirectory?, selection? })`: the defaults are an
/// empty working directory and a working-directory selection of nothing.
fn create_state(
    working_directory: Option<WorkingDirectoryStatus>,
    selection: Option<Selection>,
) -> RepositoryState {
    let selection = selection.unwrap_or(Selection {
        selected_file_ids: Vec::new(),
        diff: None,
    });
    RepositoryState {
        status: Some(working_directory.unwrap_or_default()),
        selected_file: selection.selected_file_ids.first().cloned(),
        selected_files: selection.selected_file_ids,
        diff: selection.diff,
        showing_stash: false,
        ..Default::default()
    }
}

/// `createStatus({ workingDirectory? })`.
fn create_status(working_directory: Option<WorkingDirectoryStatus>) -> WorkingDirectoryStatus {
    working_directory.unwrap_or_default()
}

/// `status.findFileWithID(id)`.
fn find_file_with_id<'a>(
    working_directory: &'a WorkingDirectoryStatus,
    id: &str,
) -> Option<&'a WorkingDirectoryFileChange> {
    working_directory.files.iter().find(|f| f.path == id)
}

/// The `workingDirectory` describe's `beforeEach`: `app/index.ts`, new,
/// with lines 1-3 of 6 selected, added to `files`.
fn partially_selected_setup() -> (WorkingDirectoryFileChange, WorkingDirectoryStatus) {
    let partial_file_selection = none_selected()
        .with_selectable_lines(BTreeSet::from([1, 2, 3, 4, 5, 6]))
        .with_line(1, true)
        .with_line(2, true)
        .with_line(3, true);

    let partially_selected_file =
        file_change("app/index.ts", FileStatusKind::New, partial_file_selection);

    let mut files_with_partial_change = files();
    files_with_partial_change.push(partially_selected_file.clone());

    let old_working_directory = from_files(files_with_partial_change);
    (partially_selected_file, old_working_directory)
}

// GHD: unit/stores/updates/update-changed-files-test.ts › updateChangedFiles › workingDirectory › clears partial selection on file when clearPartialState is true
#[test]
#[ignore = "ghd: missing: no gpui-free updateChangedFiles (selection carry-over is inside corvene_git::get_status, selection in Dispatcher::refresh_repository) and nothing clears partial selections (no clearPartialState)"]
fn clears_partial_selection_on_file_when_clear_partial_state_is_true() {
    let (partially_selected_file, old_working_directory) = partially_selected_setup();
    let prev_state = create_state(Some(old_working_directory.clone()), None);

    let status = create_status(Some(old_working_directory));

    let state = update_changed_files(&prev_state, &status, true);
    let working_directory = state.status.as_ref().expect("workingDirectory");

    let partial_file = find_file_with_id(working_directory, &partially_selected_file.path);

    let partial_file = partial_file.expect("partialFile !== null");
    assert_eq!(partial_file.selection.kind(), DiffSelectionType::None);
}

// GHD: unit/stores/updates/update-changed-files-test.ts › updateChangedFiles › workingDirectory › preserves partial selection on file when clearPartialState is false
#[test]
#[ignore = "ghd: missing: no gpui-free updateChangedFiles (selection carry-over is inside corvene_git::get_status, selection in Dispatcher::refresh_repository)"]
fn preserves_partial_selection_on_file_when_clear_partial_state_is_false() {
    let (partially_selected_file, old_working_directory) = partially_selected_setup();
    let prev_state = create_state(Some(old_working_directory.clone()), None);

    let status = create_status(Some(old_working_directory));

    let state = update_changed_files(&prev_state, &status, false);
    let working_directory = state.status.as_ref().expect("workingDirectory");

    let partial_file = find_file_with_id(working_directory, &partially_selected_file.path);

    let partial_file = partial_file.expect("partialFile !== null");
    assert_eq!(partial_file.selection.kind(), DiffSelectionType::Partial);
}

// GHD: unit/stores/updates/update-changed-files-test.ts › updateChangedFiles › workingDirectory › does not return same working directory object
#[test]
#[ignore = "ghd: missing: no gpui-free updateChangedFiles (selection carry-over is inside corvene_git::get_status, selection in Dispatcher::refresh_repository)"]
fn does_not_return_same_working_directory_object() {
    let old_working_directory = from_files(files());
    let prev_state = create_state(Some(old_working_directory.clone()), None);

    let status = create_status(Some(old_working_directory));

    let state = update_changed_files(&prev_state, &status, false);
    let working_directory = state.status.as_ref().expect("workingDirectory");

    let old_working_directory = prev_state.status.as_ref().expect("oldWorkingDirectory");
    assert!(!std::ptr::eq(working_directory, old_working_directory));
}

// GHD: unit/stores/updates/update-changed-files-test.ts › updateChangedFiles › selectedFileIDs › selects the first file if none found in state
#[test]
#[ignore = "ghd: missing: no gpui-free updateChangedFiles (the first-file selection is inline in Dispatcher::refresh_repository, which needs a gpui App)"]
fn selects_the_first_file_if_none_found_in_state() {
    let files = files();
    let prev_state = create_state(None, None);

    let status = create_status(Some(from_files(files.clone())));
    let selection = update_changed_files(&prev_state, &status, false);

    assert!(!selection.showing_stash);
    let selected_file_ids = &selection.selected_files;
    assert_eq!(selected_file_ids.len(), 1);
    // NOTE: `updateChangedFiles` sorts the paths and `app/package.json` will
    // appear in list before `README.md`
    assert_eq!(selected_file_ids[0], files[1].path);
}

// GHD: unit/stores/updates/update-changed-files-test.ts › updateChangedFiles › selectedFileIDs › remembers previous selection if file is found in status
#[test]
#[ignore = "ghd: missing: no gpui-free updateChangedFiles (keeping the selection is inline in Dispatcher::refresh_repository, which needs a gpui App)"]
fn remembers_previous_selection_if_file_is_found_in_status() {
    let files = files();
    let first_file = files[0].path.clone();
    let prev_state = create_state(
        None,
        Some(Selection {
            selected_file_ids: vec![first_file.clone()],
            diff: None,
        }),
    );

    let status = create_status(Some(from_files(files)));
    let selection = update_changed_files(&prev_state, &status, false);

    assert!(!selection.showing_stash);
    let selected_file_ids = &selection.selected_files;
    assert_eq!(selected_file_ids.len(), 1);
    assert_eq!(selected_file_ids[0], first_file);
}

// GHD: unit/stores/updates/update-changed-files-test.ts › updateChangedFiles › selectedFileIDs › clears selection if no files found in status
#[test]
#[ignore = "ghd: missing: no gpui-free updateChangedFiles (dropping gone files from the selection is inline in Dispatcher::refresh_repository, which needs a gpui App)"]
fn clears_selection_if_no_files_found_in_status() {
    let files = files();
    let first_file = files[0].path.clone();
    let prev_state = create_state(
        None,
        Some(Selection {
            selected_file_ids: vec![first_file],
            diff: None,
        }),
    );

    let status = create_status(None);
    let selection = update_changed_files(&prev_state, &status, false);

    assert!(!selection.showing_stash);
    let selected_file_ids = &selection.selected_files;
    assert_eq!(selected_file_ids.len(), 0);
}

// GHD: unit/stores/updates/update-changed-files-test.ts › updateChangedFiles › diff › clears diff if selected file is not in previous state
#[test]
#[ignore = "ghd: missing: no gpui-free updateChangedFiles (the diff is dropped inline in Dispatcher::refresh_repository, which needs a gpui App)"]
fn clears_diff_if_selected_file_is_not_in_previous_state() {
    let working_directory = from_files(files());

    let prev_state = create_state(
        Some(working_directory.clone()),
        Some(Selection {
            // an unknown file was set as selected last time
            selected_file_ids: vec!["id-from-file-not-in-status".to_string()],
            diff: Some(Arc::new(Diff::Binary)),
        }),
    );

    let status = create_status(Some(working_directory));
    let selection = update_changed_files(&prev_state, &status, false);

    assert!(!selection.showing_stash);

    assert!(selection.diff.is_none());
}

// GHD: unit/stores/updates/update-changed-files-test.ts › updateChangedFiles › diff › returns same diff if selected file from previous state is found
#[test]
#[ignore = "ghd: missing: no gpui-free updateChangedFiles (the diff is kept inline in Dispatcher::refresh_repository, which needs a gpui App)"]
fn returns_same_diff_if_selected_file_from_previous_state_is_found() {
    let files = files();
    let working_directory = from_files(files.clone());

    // first file was selected the last time we updated state
    let selected_file_ids = vec![files[0].path.clone()];
    let diff = Arc::new(Diff::Binary);

    let prev_state = create_state(
        Some(working_directory.clone()),
        Some(Selection {
            selected_file_ids,
            diff: Some(diff.clone()),
        }),
    );

    // same working directory is provided as last time
    let status = create_status(Some(working_directory));

    let selection = update_changed_files(&prev_state, &status, false);
    assert!(!selection.showing_stash);
    let kept = selection.diff.as_ref().expect("the same diff");
    assert!(Arc::ptr_eq(kept, &diff));
}
