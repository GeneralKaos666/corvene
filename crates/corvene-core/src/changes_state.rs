//! GHD `updateChangedFiles` (`app/src/lib/stores/updates/changes-state.ts`):
//! merge a freshly read working directory status into a repository's
//! changes state.
//!
//! - Every file keeps the line selection it had in the previous status;
//!   with `clear_partial_state` (after a commit, GHD's
//!   `refreshChangesSection({ clearPartialState: true })`) a partial
//!   selection becomes "nothing selected" instead.
//! - The files are sorted by their paths, case-insensitively
//!   (`WorkingDirectoryStatus::sort_files`, GHD `caseInsensitiveCompare`).
//! - Selected files that are gone are dropped from the selection; the first
//!   file is selected when nothing is left.
//! - The diff is kept only when the same single file stays selected.
//!
//! `Dispatcher::refresh_repository` applies every status it reads with
//! [`apply_changed_files`].
//!
//! Corvene identifies a changed file by its path (GHD's id is
//! `<status kind>+<path>`, so a file whose kind changes counts as a new
//! one). Corvene keeps the working-directory selection while a stash is
//! shown (GHD's `ChangesSelectionKind.Stash` replaces it), so it is updated
//! whether or not `RepositoryState::showing_stash` is set. Corvene's
//! selection also has an anchor, `RepositoryState::selected_file` (GHD
//! shows the diff of `selectedFileIDs[0]`): it stays when it is still
//! selected and moves to the first selected file otherwise.

use std::collections::{HashMap, HashSet};

use corvene_models::{DiffSelection, DiffSelectionType, WorkingDirectoryStatus};

use crate::state::RepositoryState;

/// GHD `updateChangedFiles(state, status, clearPartialState)`: the changes
/// state after merging `status` into `state` (see the module docs).
pub fn update_changed_files(
    state: &RepositoryState,
    status: &WorkingDirectoryStatus,
    clear_partial_state: bool,
) -> RepositoryState {
    let mut next = state.clone();
    apply_changed_files(&mut next, status.clone(), clear_partial_state);
    next
}

/// [`update_changed_files`] in place, taking the status; returns whether
/// the status, the selection or the diff changed.
pub fn apply_changed_files(
    state: &mut RepositoryState,
    mut status: WorkingDirectoryStatus,
    clear_partial_state: bool,
) -> bool {
    // Attempt to preserve the selection state for each file in the new
    // working directory state by looking at the current files
    {
        let previous: HashMap<&str, &DiffSelection> = state
            .status
            .iter()
            .flat_map(|s| s.files.iter())
            .map(|f| (f.path.as_str(), &f.selection))
            .collect();
        for file in &mut status.files {
            let Some(existing) = previous.get(file.path.as_str()) else {
                continue;
            };
            file.selection = if clear_partial_state && existing.kind() == DiffSelectionType::Partial
            {
                file.selection.select_none()
            } else {
                (*existing).clone()
            };
        }
    }
    status.sort_files();

    // The previously selected files might not be available in the working
    // directory any more due to having been committed or discarded
    let (selected_files, keep_diff) = {
        let present: HashSet<&str> = status.files.iter().map(|f| f.path.as_str()).collect();
        let previous = &state.selected_files;
        let mut selected: Vec<String> = previous
            .iter()
            .filter(|p| present.contains(p.as_str()))
            .cloned()
            .collect();
        // Select the first file if we don't have anything selected and we
        // have something to select.
        if selected.is_empty()
            && let Some(first) = status.files.first()
        {
            selected.push(first.path.clone());
        }
        // We only render a diff when a single file is selected: keep it when
        // that file was the single selected file before.
        let keep_diff = selected.len() == 1 && previous.len() == 1 && previous[0] == selected[0];
        (selected, keep_diff)
    };
    let selected_file = state
        .selected_file
        .clone()
        .filter(|p| selected_files.contains(p))
        .or_else(|| selected_files.first().cloned());

    let mut changed = set(&mut state.selected_files, selected_files);
    changed |= set(&mut state.selected_file, selected_file);
    if !keep_diff && state.diff.is_some() {
        state.diff = None;
        changed = true;
    }
    changed |= set(&mut state.status, Some(status));
    changed
}

fn set<T: PartialEq>(slot: &mut T, value: T) -> bool {
    if *slot == value {
        return false;
    }
    *slot = value;
    true
}

#[cfg(test)]
mod tests {
    use corvene_models::{FileStatus, FileStatusKind, GitStatusEntry, WorkingDirectoryFileChange};

    use super::*;

    fn file(path: &str, selection: DiffSelection) -> WorkingDirectoryFileChange {
        WorkingDirectoryFileChange {
            path: path.to_string(),
            old_path: None,
            status: FileStatus {
                kind: FileStatusKind::Modified,
                index: GitStatusEntry::Unchanged,
                working_tree: GitStatusEntry::Modified,
                score: None,
                code: ".M".to_string(),
                submodule: false,
                submodule_status: None,
                conflict_markers: None,
            },
            selection,
        }
    }

    fn status(files: Vec<WorkingDirectoryFileChange>) -> WorkingDirectoryStatus {
        WorkingDirectoryStatus {
            files,
            ..Default::default()
        }
    }

    #[test]
    fn the_anchor_moves_to_a_file_still_selected() {
        let mut state = RepositoryState {
            status: Some(status(vec![
                file("a", DiffSelection::all()),
                file("b", DiffSelection::all()),
                file("c", DiffSelection::all()),
            ])),
            selected_files: vec!["a".into(), "c".into()],
            selected_file: Some("c".into()),
            ..Default::default()
        };
        let changed = apply_changed_files(
            &mut state,
            status(vec![
                file("b", DiffSelection::all()),
                file("a", DiffSelection::none()),
            ]),
            false,
        );
        assert!(changed);
        assert_eq!(state.selected_files, vec!["a".to_string()]);
        assert_eq!(state.selected_file.as_deref(), Some("a"));
        let files = &state
            .status
            .as_ref()
            .map(|s| s.files.clone())
            .unwrap_or_default();
        // sorted, and `a` keeps the selection it had
        assert_eq!(files[0].path, "a");
        assert_eq!(files[0].selection.kind(), DiffSelectionType::All);
    }

    #[test]
    fn an_unchanged_status_changes_nothing() {
        let current = status(vec![file("a", DiffSelection::all())]);
        let mut state = RepositoryState {
            status: Some(current.clone()),
            selected_files: vec!["a".into()],
            selected_file: Some("a".into()),
            ..Default::default()
        };
        assert!(!apply_changed_files(&mut state, current, false));
    }
}
