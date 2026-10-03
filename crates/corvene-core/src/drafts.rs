//! Corvene additions that keep the Changes tab's work across restarts (GHD
//! keeps it in memory only, `RepositoryStateCache`):
//! - `766-persist-commit-drafts`: each repository's commit summary and
//!   description, saved a moment after the last edit and cleared by a commit.
//! - `767-persist-file-selection`: each repository's unticked files (whole
//!   files only), unticked again by the repository's first status of the
//!   next session.

use std::time::Duration;

use corvene_models::{DiffSelectionType, WorkingDirectoryStatus};
use gpui_kit::{App, AsyncApp};
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::dispatcher::Dispatcher;
use crate::persistence::StoreExt;
use crate::state::AppState;

/// How long after the last edit a draft is written.
const DRAFT_SAVE_DELAY: Duration = Duration::from_secs(1);

/// A repository's unfinished commit message.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitDraft {
    pub summary: String,
    pub description: String,
}

impl CommitDraft {
    /// The draft worth keeping: `None` when nothing was typed (an empty
    /// summary with no description, or only the commit template's text).
    pub fn normalized(
        summary: &str,
        description: &str,
        template: Option<&str>,
    ) -> Option<CommitDraft> {
        let untouched_description = description.trim().is_empty()
            || template.is_some_and(|t| t.trim() == description.trim());
        if summary.trim().is_empty() && untouched_description {
            return None;
        }
        Some(CommitDraft {
            summary: summary.to_string(),
            description: description.to_string(),
        })
    }
}

/// The paths of `status` whose files are wholly left out of the commit.
pub fn excluded_paths(status: &WorkingDirectoryStatus) -> Vec<String> {
    let mut paths: Vec<String> = status
        .files
        .iter()
        .filter(|f| f.selection.kind() == DiffSelectionType::None)
        .map(|f| f.path.clone())
        .collect();
    paths.sort();
    paths
}

/// Untick the files of `status` named in `excluded` (a fresh status ticks
/// every file).
pub fn apply_excluded(status: &mut WorkingDirectoryStatus, excluded: &[String]) {
    for file in &mut status.files {
        if excluded.contains(&file.path) {
            file.selection = file.selection.select_none();
        }
    }
}

/// `767-persist-file-selection`: record `id`'s unticked files and save them
/// when they changed. Call after anything that ticks or unticks files.
pub(crate) fn note_excluded(s: &mut AppState, id: u64, cx: &mut App) {
    if !s.flags.bool(crate::flags::ids::PERSIST_FILE_SELECTION) {
        return;
    }
    let Some(status) = s.repo_states.get(&id).and_then(|rs| rs.status.as_ref()) else {
        return;
    };
    let paths = excluded_paths(status);
    let unchanged = match s.excluded_files.get(&id) {
        Some(saved) => *saved == paths,
        None => paths.is_empty(),
    };
    if unchanged {
        return;
    }
    if paths.is_empty() {
        s.excluded_files.remove(&id);
    } else {
        s.excluded_files.insert(id, paths);
    }
    let store = s.store.clone();
    let excluded = s.excluded_files.clone();
    cx.background_executor()
        .spawn(async move {
            if let Err(err) = store.save_excluded_files(&excluded) {
                warn!(?err, "could not save the unticked files");
            }
        })
        .detach();
}

impl Dispatcher {
    /// `766-persist-commit-drafts`: the commit form of `id` now holds
    /// `draft` (`None`: nothing typed); written after [`DRAFT_SAVE_DELAY`]
    /// without further edits.
    pub fn set_commit_draft(id: u64, draft: Option<CommitDraft>, cx: &mut App) {
        let state = Self::state(cx);
        let nonce = state.update(cx, |s, _| {
            if !s.flags.bool(crate::flags::ids::PERSIST_COMMIT_DRAFTS)
                || s.commit_drafts.get(&id) == draft.as_ref()
            {
                return None;
            }
            match draft {
                Some(draft) => s.commit_drafts.insert(id, draft),
                None => s.commit_drafts.remove(&id),
            };
            s.commit_drafts_nonce += 1;
            Some(s.commit_drafts_nonce)
        });
        let Some(nonce) = nonce else { return };
        cx.spawn(async move |cx: &mut AsyncApp| {
            cx.background_executor().timer(DRAFT_SAVE_DELAY).await;
            let pending = state.read_with(cx, |s, _| {
                (s.commit_drafts_nonce == nonce).then(|| (s.store.clone(), s.commit_drafts.clone()))
            });
            if let Some((store, drafts)) = pending {
                cx.background_executor()
                    .spawn(async move {
                        if let Err(err) = store.save_commit_drafts(&drafts) {
                            warn!(?err, "could not save the commit drafts");
                        }
                    })
                    .await;
            }
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use corvene_models::{
        DiffSelection, FileStatus, FileStatusKind, GitStatusEntry, WorkingDirectoryFileChange,
    };

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
                code: ".M".into(),
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
    fn drafts_skip_untouched_forms() {
        assert_eq!(CommitDraft::normalized("", "  \n", None), None);
        assert_eq!(
            CommitDraft::normalized("", "Why:\n", Some("Why:\n")),
            None,
            "the template alone is no draft"
        );
        assert_eq!(
            CommitDraft::normalized("Fix", "Why:\n", Some("Why:\n")),
            Some(CommitDraft {
                summary: "Fix".into(),
                description: "Why:\n".into()
            })
        );
        assert!(CommitDraft::normalized("", "details", None).is_some());
    }

    #[test]
    fn excluded_paths_are_whole_unticked_files() {
        let st = status(vec![
            file("b.txt", DiffSelection::none()),
            file("a.txt", DiffSelection::none()),
            file("c.txt", DiffSelection::all()),
        ]);
        assert_eq!(excluded_paths(&st), vec!["a.txt", "b.txt"]);
    }

    #[test]
    fn apply_excluded_unticks_named_files_only() {
        let mut st = status(vec![
            file("a.txt", DiffSelection::all()),
            file("b.txt", DiffSelection::all()),
        ]);
        apply_excluded(&mut st, &["b.txt".to_string(), "gone.txt".to_string()]);
        assert_eq!(excluded_paths(&st), vec!["b.txt"]);
    }
}
