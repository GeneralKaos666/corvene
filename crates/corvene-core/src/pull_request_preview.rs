//! Preview Pull Request - GHD app-store `_startPullRequest`,
//! `_initializePullRequestPreview`, `_updatePullRequestBaseBranch`,
//! `_changePullRequestFileSelection` and `setupPRMergeTreePromise`: the
//! commits and changed files of the current branch since it diverged from
//! the base branch, one file's merge-base diff, and the mergeability.
//!
//! Deviation (`1203-compare-branch-files`): the compare view opens the same
//! dialog against the compared branch, without the pull request button
//! ("Compare Branches"), for any repository.
//!
//! Corvene `1218-compare-refs`: the same loaders fill the Compare view's
//! combined diff ([`PreviewSlot::RefCompare`]), between any two refs, from
//! their merge base or straight from one to the other.

use std::sync::Arc;

use crate::host::Host;
use corvene_models::{ChangesetData, Diff, Mergeability};
use tracing::warn;

use crate::dispatcher::Dispatcher;
use crate::remote::spawn_bg;
use crate::state::{Popup, RepositoryState};

/// Where a [`PullRequestPreview`] lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewSlot {
    /// The Preview Pull Request (or `1203` Compare Branches) dialog.
    PullRequest,
    /// `1218-compare-refs`: the Compare view's combined diff.
    RefCompare,
}

impl PreviewSlot {
    pub fn get(self, rs: &RepositoryState) -> Option<&PullRequestPreview> {
        match self {
            Self::PullRequest => rs.pull_request_preview.as_ref(),
            Self::RefCompare => rs.ref_compare_changes.as_ref(),
        }
    }

    fn slot(self, rs: &mut RepositoryState) -> &mut Option<PullRequestPreview> {
        match self {
            Self::PullRequest => &mut rs.pull_request_preview,
            Self::RefCompare => &mut rs.ref_compare_changes,
        }
    }
}

/// `MergeTreeResult` as the dialog footer shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MergeStatus {
    Loading,
    Clean,
    Conflicts,
    /// Unrelated histories.
    Invalid,
}

/// `IPullRequestState` (+ the commit selection folded in).
#[derive(Clone, Debug, Default)]
pub struct PullRequestPreview {
    /// `None`: no default branch to compare against.
    pub base_branch: Option<String>,
    pub current_branch: String,
    /// `None` while the commits are being listed; oldest first.
    pub commit_shas: Option<Vec<String>>,
    pub changeset: Option<ChangesetData>,
    /// The selected file (`commitSelection.file`).
    pub file: Option<String>,
    pub diff: Option<Arc<Diff>>,
    pub diff_generation: u64,
    pub diff_contents: Option<Arc<Vec<String>>>,
    pub merge_status: Option<MergeStatus>,
    /// Bumped by every (re)initialisation; late results for an older
    /// generation are dropped.
    pub generation: u64,
    /// `1203-compare-branch-files`: opened from the compare view to show the
    /// changed files only (no pull request button).
    pub compare_only: bool,
    /// `1218-compare-refs`: the files changed straight from the base to the
    /// compared ref (`git diff base..head`), not since their merge base.
    pub direct: bool,
}

impl PullRequestPreview {
    /// `nonLocalCommitSHA` stand-in: the comparison branch's tip.
    pub fn newest_sha(&self) -> Option<&str> {
        self.commit_shas.as_ref()?.last().map(String::as_str)
    }
}

impl Dispatcher {
    /// Branch › Preview Pull Request (`_startPullRequest`).
    pub fn start_pull_request(id: u64, cx: &mut dyn Host) {
        let (current, default_branch) = {
            let s = Self::state(cx).read(cx);
            let Some(rs) = s.repo_states.get(&id) else {
                return;
            };
            let Some(current) = rs
                .info
                .as_ref()
                .and_then(|i| i.current_branch())
                .map(|b| b.name.clone())
            else {
                return;
            };
            (current, rs.default_branch.clone())
        };
        // `333-pr-base-from-branch-origin`, `352-remember-pr-base`
        let default_branch = Self::proposed_pull_request_base(id, cx).or(default_branch);
        Self::close_foldout(cx);
        Self::initialize_pull_request_preview(id, default_branch, current, false, cx);
    }

    /// `1203-compare-branch-files`: the compare view's Show Changed Files:
    /// the preview dialog with `base` (the compared branch) as its base.
    pub fn compare_branch_files(id: u64, base: String, cx: &mut dyn Host) {
        let current = Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|rs| rs.info.as_ref())
            .and_then(|i| i.current_branch())
            .map(|b| b.name.clone());
        if let Some(current) = current {
            Self::initialize_pull_request_preview(id, Some(base), current, true, cx);
        }
    }

    /// `_updatePullRequestBaseBranch`
    pub fn update_pull_request_base_branch(id: u64, base: String, cx: &mut dyn Host) {
        let current = Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|rs| rs.pull_request_preview.as_ref())
            .map(|p| (p.current_branch.clone(), p.compare_only));
        if let Some((current, compare_only)) = current {
            Self::initialize_pull_request_preview(id, Some(base), current, compare_only, cx);
        }
    }

    /// Drop the preview state when the dialog closes.
    pub fn close_pull_request_preview(id: u64, cx: &mut dyn Host) {
        Self::state(cx).update(cx, |s, cx| {
            if let Some(rs) = s.repo_states.get_mut(&id)
                && rs.pull_request_preview.take().is_some()
            {
                cx.notify();
            }
        });
    }

    /// `_initializePullRequestPreview`: show the dialog, then list the
    /// commits between the branches, the changed files, the mergeability
    /// and the first file's diff.
    fn initialize_pull_request_preview(
        id: u64,
        base: Option<String>,
        current: String,
        compare_only: bool,
        cx: &mut dyn Host,
    ) {
        Self::load_range_preview(
            id,
            PreviewSlot::PullRequest,
            PullRequestPreview {
                base_branch: base,
                current_branch: current,
                compare_only,
                ..PullRequestPreview::default()
            },
            cx,
        );
    }

    /// Put `preview` (its refs and options) in `slot` and load its files:
    /// for the pull request, the commits between the branches, the files
    /// changed since they diverged and the mergeability; for the Compare
    /// view (`1217`) the files changed between the two refs. Then the first
    /// file's diff.
    pub(crate) fn load_range_preview(
        id: u64,
        slot: PreviewSlot,
        mut preview: PullRequestPreview,
        cx: &mut dyn Host,
    ) {
        let generation = Self::state(cx).update(cx, |s, cx| {
            let target = slot.slot(s.repo_state_mut(id));
            let generation = target.as_ref().map(|p| p.generation + 1).unwrap_or(1);
            preview.generation = generation;
            *target = Some(preview.clone());
            cx.notify();
            generation
        });
        if slot == PreviewSlot::PullRequest
            && Self::state(cx).read(cx).popup() != Some(&Popup::StartPullRequest { repo: id })
        {
            Self::show_popup(Popup::StartPullRequest { repo: id }, cx);
        }
        let Some(base) = preview.base_branch.clone() else {
            // `showPullRequestPopupNoBaseBranch`
            return;
        };
        let current = preview.current_branch.clone();
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let (base_for_load, current_for_load) = (base.clone(), current.clone());
        let merge_base = !preview.direct;
        // a reload keeps the selected file while it is still there
        let wanted = preview.file.clone();
        spawn_bg(
            cx,
            move || {
                if slot == PreviewSlot::RefCompare {
                    // the compared ref's commit is the files' commitish
                    let Some(newest) = corvene_git::resolve_commit(&workdir, &current_for_load)
                        .ok()
                        .flatten()
                    else {
                        return (Vec::new(), Some(ChangesetData::default()));
                    };
                    let changeset = corvene_git::range_changed_files(
                        git,
                        &workdir,
                        &base_for_load,
                        &newest,
                        merge_base,
                        &newest,
                    )
                    .unwrap_or_else(|err| {
                        warn!(%err, "could not list the compared files");
                        Some(ChangesetData::default())
                    });
                    return (vec![newest], changeset);
                }
                let commits = corvene_git::commits_between(
                    git.clone(),
                    &workdir,
                    &base_for_load,
                    &current_for_load,
                )
                .map_err(|err| warn!(%err, "could not list the pull request commits"))
                .ok()
                .flatten()
                .unwrap_or_default();
                let shas: Vec<String> = commits.into_iter().map(|c| c.sha).collect();
                let changeset = match shas.last() {
                    Some(newest) => corvene_git::merge_base_changed_files(
                        git,
                        &workdir,
                        &base_for_load,
                        &current_for_load,
                        newest,
                    )
                    .unwrap_or_else(|err| {
                        warn!(%err, "could not list the pull request files");
                        Some(ChangesetData::default())
                    }),
                    None => Some(ChangesetData::default()),
                };
                (shas, changeset)
            },
            move |(shas, changeset), cx| {
                let has_merge_base = changeset.is_some();
                let first_file = changeset.as_ref().and_then(|c| {
                    wanted
                        .as_ref()
                        .filter(|w| c.files.iter().any(|f| &f.path == *w))
                        .or(c.files.first().map(|f| &f.path))
                        .cloned()
                });
                let applied = Self::state(cx).update(cx, |s, cx| {
                    let Some(preview) = slot
                        .slot(s.repo_state_mut(id))
                        .as_mut()
                        .filter(|p| p.generation == generation)
                    else {
                        return false;
                    };
                    preview.commit_shas = Some(if has_merge_base {
                        shas.clone()
                    } else {
                        Vec::new()
                    });
                    preview.changeset = Some(changeset.unwrap_or_default());
                    preview.merge_status = if !has_merge_base {
                        Some(MergeStatus::Invalid)
                    } else if !shas.is_empty() && slot == PreviewSlot::PullRequest {
                        Some(MergeStatus::Loading)
                    } else {
                        None
                    };
                    cx.notify();
                    true
                });
                if !applied {
                    return;
                }
                if slot == PreviewSlot::PullRequest && has_merge_base && !shas.is_empty() {
                    Self::load_pull_request_mergeability(id, generation, base, current, cx);
                }
                if let Some(path) = first_file {
                    Self::select_preview_file(id, slot, path, cx);
                }
            },
        );
    }

    /// `setupPRMergeTreePromise`
    fn load_pull_request_mergeability(
        id: u64,
        generation: u64,
        base: String,
        current: String,
        cx: &mut dyn Host,
    ) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        spawn_bg(
            cx,
            move || corvene_git::determine_mergeability(git, &workdir, &base, &current),
            move |result, cx| {
                Self::state(cx).update(cx, |s, cx| {
                    let Some(preview) = s
                        .repo_states
                        .get_mut(&id)
                        .and_then(|rs| rs.pull_request_preview.as_mut())
                        .filter(|p| p.generation == generation)
                    else {
                        return;
                    };
                    preview.merge_status = match result {
                        Ok(Mergeability::Clean) => Some(MergeStatus::Clean),
                        Ok(Mergeability::Conflicts(_)) => Some(MergeStatus::Conflicts),
                        Ok(Mergeability::Invalid) => Some(MergeStatus::Invalid),
                        Err(err) => {
                            warn!(%err, "could not determine the pull request mergeability");
                            None
                        }
                    };
                    cx.notify();
                });
            },
        );
    }

    /// `_changePullRequestFileSelection`: select a file and load its
    /// merge-base diff.
    pub fn select_pull_request_file(id: u64, path: String, cx: &mut dyn Host) {
        Self::select_preview_file(id, PreviewSlot::PullRequest, path, cx);
    }

    /// Select a file of `slot`'s files and load its diff.
    pub fn select_preview_file(id: u64, slot: PreviewSlot, path: String, cx: &mut dyn Host) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let hide_whitespace = Self::state(cx)
            .read(cx)
            .settings
            .hide_whitespace_in_pull_request_diff;
        let Some((generation, base, current, newest, file, direct)) =
            Self::state(cx).update(cx, |s, cx| {
                let preview = slot.slot(s.repo_states.get_mut(&id)?).as_mut()?;
                preview.file = Some(path.clone());
                preview.diff = None;
                cx.notify();
                let base = preview.base_branch.clone()?;
                let newest = preview.newest_sha()?.to_string();
                let file = preview
                    .changeset
                    .as_ref()?
                    .files
                    .iter()
                    .find(|f| f.path == path)?
                    .clone();
                Some((
                    preview.generation,
                    base,
                    preview.current_branch.clone(),
                    newest,
                    file,
                    preview.direct,
                ))
            })
        else {
            return;
        };
        spawn_bg(
            cx,
            move || {
                let diff = corvene_git::range_file_diff(
                    git.clone(),
                    &workdir,
                    &file,
                    &base,
                    &current,
                    !direct,
                    hide_whitespace,
                    &newest,
                );
                let contents = (file.status.kind != corvene_models::FileStatusKind::Deleted)
                    .then(|| corvene_git::blob_lines(git, &workdir, &newest, &file.path))
                    .flatten();
                (diff, contents)
            },
            move |(diff, contents), cx| {
                Self::state(cx).update(cx, |s, cx| {
                    let Some(preview) = s
                        .repo_states
                        .get_mut(&id)
                        .and_then(|rs| slot.slot(rs).as_mut())
                        .filter(|p| p.generation == generation)
                    else {
                        return;
                    };
                    if preview.file.as_deref() != Some(path.as_str()) {
                        return;
                    }
                    let diff = diff.unwrap_or_else(|err| {
                        warn!(id, %err, "pull request diff failed");
                        Diff::Empty
                    });
                    let mut no_old = None;
                    if crate::dispatcher::replace_diff(
                        (&mut preview.diff, &mut preview.diff_contents, &mut no_old),
                        (Arc::new(diff), contents.map(Arc::new), None),
                    ) {
                        preview.diff_generation += 1;
                    }
                    cx.notify();
                });
            },
        );
    }

    /// Diff Settings › Hide Whitespace Changes inside the dialog
    /// (`onHideWhitespaceInPullRequestDiffChanged`).
    pub fn set_hide_whitespace_in_pull_request_diff(hide: bool, cx: &mut dyn Host) {
        Self::update_settings(cx, |s| s.hide_whitespace_in_pull_request_diff = hide);
        let Some(id) = Self::state(cx).read(cx).selected else {
            return;
        };
        // `1218-compare-refs`: the Compare view's diff follows the setting
        for slot in [PreviewSlot::PullRequest, PreviewSlot::RefCompare] {
            let file = Self::state(cx)
                .read(cx)
                .repo_states
                .get(&id)
                .and_then(|rs| slot.get(rs))
                .and_then(|p| p.file.clone());
            if let Some(file) = file {
                Self::select_preview_file(id, slot, file, cx);
            }
        }
    }
}
