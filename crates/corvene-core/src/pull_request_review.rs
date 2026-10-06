//! Corvene `348-pull-request-review`: reviewing a pull request in the app.
//! GitHub Desktop has no review surface at all: its pull request list
//! checks a pull request out, `ui/notifications/*` show one review or
//! comment from a notification, and every write happens on github.com.
//!
//! Branch › Review Pull Request… (for the checked-out branch's pull
//! request) or Review Pull Request in the Pull Requests tab's row menu
//! swaps History for the review ([`PullRequestReviewState`] on
//! `RepositoryState.pull_request_review`): the sidebar lists the pull
//! request's changed files with their thread counts under an Overview row,
//! the content area shows the overview pane (description, reviewers,
//! checks, timeline) or the selected file's diff with its review threads
//! under the lines they belong to, and the footer's Review changes… opens
//! the submit dialog (comment, approve, request changes).
//!
//! The diff shown is `merge-base(base, head)..head` where head is the
//! local branch tip when the pull request's branch is checked out
//! (commits not pushed yet included) and otherwise the head GitHub knows,
//! fetched into `refs/corvene/pull/<n>/head` when the repository lacks it.
//! Threads come from GraphQL ([`corvene_github::review`]) and are placed
//! by [`crate::review_anchor`]: GitHub's line numbers refer to its own head
//! and base, so when the shown head differs they are carried over with a
//! line map of the two blobs (`349-review-thread-remapping`), and a new
//! comment's line is carried back the same way before it is posted (a
//! line only the local commits have cannot be commented on until it is
//! pushed). Every write (reply, resolve, a comment into the pending review
//! or posted at once, submit, discard, edit, delete) is one mutation
//! followed by a reload of the threads, so the state on screen is always
//! GitHub's.

use std::collections::HashMap;
use std::sync::Arc;

use corvene_github::Client;
use corvene_github::review::NewThread;
pub use corvene_github::review::{
    DiffSide, OverviewLabel, OverviewReview, PendingReview, PullRequestOverview, ReviewActor,
    ReviewComment, ReviewEvent, ReviewRequest, ReviewThread, RollupState, TimelineItem,
};
use corvene_models::{
    ChangesetData, CommittedFileChange, Diff, FileStatusKind, GitHubRepository, PullRequest,
    Section, url_matches_remote,
};
use tracing::{info, warn};

use crate::dispatcher::Dispatcher;
use crate::host::Host;
use crate::markdown::Block;
use crate::mco::Banner;
use crate::remote::spawn_bg;
use crate::review_anchor::{AnchoredThread, FileLineMaps, LineMap, ThreadAnchor, anchor_threads};
use crate::state::{AppState, Popup, RepositoryState};

/// Original commits read for outdated threads of one file, at most.
const MAX_ORIGINAL_BLOBS: usize = 12;

/// Which pane the content area shows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ReviewSelection {
    #[default]
    Overview,
    File(String),
}

/// A comment being written on a line or a range of the shown diff (its
/// line numbers: new side for `Right`, old side for `Left`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComposeTarget {
    pub path: String,
    pub side: DiffSide,
    pub line: u32,
    /// The first line of a range (below `line`).
    pub start: Option<u32>,
}

impl ComposeTarget {
    pub fn range(&self) -> (u32, u32) {
        (self.start.unwrap_or(self.line).min(self.line), self.line)
    }

    pub fn covers(&self, side: DiffSide, line: u32) -> bool {
        let (start, end) = self.range();
        self.side == side && (start..=end).contains(&line)
    }
}

/// The threads of one file, counted for the sidebar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FileThreadCounts {
    pub total: usize,
    pub unresolved: usize,
    pub pending: usize,
}

/// The selected file's diff and where its threads go in it.
#[derive(Clone, Debug, Default)]
pub struct ReviewFile {
    pub path: String,
    pub change: Option<CommittedFileChange>,
    pub diff: Option<Arc<Diff>>,
    pub diff_generation: u64,
    pub diff_contents: Option<Arc<Vec<String>>>,
    pub diff_old_contents: Option<Arc<Vec<String>>>,
    pub anchors: Vec<AnchoredThread>,
    /// The maps the anchors came from (`None` while the diff loads, or
    /// with flag 349 off).
    pub maps: Option<FileLineMaps>,
    /// The maps for a new comment's line back to GitHub's coordinates.
    pub maps_back: Option<(LineMap, LineMap)>,
    /// The shown head is GitHub's and so is the base side.
    pub same_as_github: bool,
}

impl ReviewFile {
    /// The threads under the row with `line` on `side`, in list order.
    pub fn anchored_at(&self, side: DiffSide, line: u32) -> impl Iterator<Item = &AnchoredThread> {
        self.anchors.iter().filter(move |a| {
            matches!(a.anchor, ThreadAnchor::Line { side: s, line: l, .. } if s == side && l == line)
        })
    }

    /// Threads without a place in the diff (outdated) and file-level ones.
    pub fn unplaced(&self) -> impl Iterator<Item = &AnchoredThread> {
        self.anchors
            .iter()
            .filter(|a| matches!(a.anchor, ThreadAnchor::Outdated | ThreadAnchor::File))
    }
}

#[derive(Clone, Debug, Default)]
pub struct PullRequestReviewState {
    pub pull_request: PullRequest,
    /// Bumped by every (re)preparation; late results of an older one are
    /// dropped.
    pub generation: u64,
    /// The head GitHub knows (`headRefOid`).
    pub api_head: String,
    /// The head whose diff is shown (`None` while preparing).
    pub display_head: Option<String>,
    /// The diff's old side: the merge base of the base branch and the
    /// shown head.
    pub base_sha: Option<String>,
    /// The merge base GitHub's diff uses (its base side), when it could
    /// be computed locally.
    pub github_base: Option<String>,
    /// The checked-out branch, when it is the pull request's.
    pub local_branch: Option<String>,
    pub preparing: bool,
    pub prepare_error: Option<String>,
    pub changeset: Option<ChangesetData>,
    pub selection: ReviewSelection,
    pub file: Option<ReviewFile>,
    pub threads: Vec<ReviewThread>,
    pub threads_loaded: bool,
    pub threads_loading: bool,
    pub threads_error: Option<String>,
    pub threads_reload_pending: bool,
    /// Bumped by every thread reload (the anchors follow).
    pub threads_version: u64,
    /// The pull request's node id, once the threads or the overview came.
    pub pull_request_id: Option<String>,
    pub overview: Option<PullRequestOverview>,
    pub overview_loading: bool,
    pub overview_error: Option<String>,
    /// The viewer's unsubmitted review.
    pub pending_review: Option<PendingReview>,
    pub composer: Option<ComposeTarget>,
    /// A write in flight, named.
    pub busy: Option<String>,
    /// The last write's failure.
    pub error: Option<String>,
    /// Resolved threads shown expanded.
    pub show_resolved: bool,
    /// No account for the repository's host: the diff shows, GitHub's
    /// threads and the writes do not.
    pub signed_out: bool,
    /// History's selection when the review opened, restored on close.
    pub saved_selection: Vec<String>,
    /// Comment and description bodies parsed once, by comment id (the
    /// pull request's body under its node id).
    pub bodies: HashMap<String, Arc<Vec<Block>>>,
}

impl PullRequestReviewState {
    pub fn number(&self) -> u64 {
        self.pull_request.number
    }

    /// The shown head is the one GitHub has.
    pub fn same_head(&self) -> bool {
        self.display_head.as_deref() == Some(self.api_head.as_str())
    }

    pub fn thread(&self, id: &str) -> Option<&ReviewThread> {
        self.threads.iter().find(|t| t.id == id)
    }

    /// The threads of `path`, each with its index in [`Self::threads`].
    pub fn threads_for(&self, path: &str) -> Vec<(usize, &ReviewThread)> {
        self.threads
            .iter()
            .enumerate()
            .filter(|(_, t)| t.path == path)
            .collect()
    }

    pub fn counts_by_file(&self) -> HashMap<&str, FileThreadCounts> {
        let mut out: HashMap<&str, FileThreadCounts> = HashMap::new();
        for thread in &self.threads {
            let c = out.entry(thread.path.as_str()).or_default();
            c.total += 1;
            if !thread.is_resolved {
                c.unresolved += 1;
            }
            if thread.is_pending() {
                c.pending += 1;
            }
        }
        out
    }

    pub fn unresolved_count(&self) -> usize {
        self.threads.iter().filter(|t| !t.is_resolved).count()
    }

    /// Comments of the pending review (its own count, else the threads').
    pub fn pending_count(&self) -> u64 {
        match &self.pending_review {
            Some(p) => p.comment_count,
            None => self
                .threads
                .iter()
                .flat_map(|t| t.comments.iter())
                .filter(|c| c.pending)
                .count() as u64,
        }
    }

    /// The parsed body of a comment (or of the description, by the pull
    /// request's id), parsed now when the load did not.
    pub fn body(&self, id: &str, text: &str) -> Arc<Vec<Block>> {
        self.bodies
            .get(id)
            .cloned()
            .unwrap_or_else(|| Arc::new(crate::markdown::parse(text)))
    }

    pub fn selected_file(&self) -> Option<&str> {
        match &self.selection {
            ReviewSelection::File(path) => Some(path),
            ReviewSelection::Overview => None,
        }
    }

    /// The file whose diff is loaded and shown.
    pub fn shown_file(&self) -> Option<&ReviewFile> {
        let path = self.selected_file()?;
        self.file.as_ref().filter(|f| f.path == path)
    }

    pub fn html_url(&self) -> Option<String> {
        self.overview
            .as_ref()
            .map(|o| o.url.clone())
            .filter(|u| !u.is_empty())
            .or_else(|| self.pull_request.html_url())
    }
}

/// The review of the selected repository, `None` while the flag is off.
pub fn review_of<'a>(s: &AppState, rs: &'a RepositoryState) -> Option<&'a PullRequestReviewState> {
    if !s.flags.bool(crate::flags::ids::PULL_REQUEST_REVIEW) {
        return None;
    }
    rs.pull_request_review.as_ref()
}

/// The ref the pull request's head is fetched into when the repository
/// does not have it.
pub fn review_head_ref(number: u64) -> String {
    format!("refs/corvene/pull/{number}/head")
}

/// What preparing the review found.
struct Prepared {
    display_head: String,
    base_sha: String,
    github_base: Option<String>,
    changeset: ChangesetData,
}

/// What loading a file found.
struct LoadedFile {
    diff: Diff,
    contents: Option<Vec<String>>,
    old_contents: Option<Vec<String>>,
    maps: Option<FileLineMaps>,
    maps_back: Option<(LineMap, LineMap)>,
    same_as_github: bool,
}

fn parse_bodies(threads: &[ReviewThread], into: &mut HashMap<String, Arc<Vec<Block>>>) {
    for comment in threads.iter().flat_map(|t| t.comments.iter()) {
        into.entry(comment.id.clone())
            .or_insert_with(|| Arc::new(crate::markdown::parse(&comment.body)));
    }
}

impl Dispatcher {
    /// Branch › Review Pull Request…: the checked-out branch's pull request.
    pub fn review_current_pull_request(id: u64, cx: &mut dyn Host) {
        let pr = Self::state(cx).read(cx).current_pull_request(id).cloned();
        match pr {
            Some(pr) => Self::review_pull_request(id, pr, cx),
            None => Self::show_error(
                "No pull request",
                "The current branch has no open pull request.",
                cx,
            ),
        }
    }

    /// Open the review of `pr` in place of History.
    pub fn review_pull_request(id: u64, pr: PullRequest, cx: &mut dyn Host) {
        if !Self::state(cx)
            .read(cx)
            .flags
            .bool(crate::flags::ids::PULL_REQUEST_REVIEW)
        {
            return;
        }
        Self::close_foldout(cx);
        Self::show_section(id, Section::History, cx);
        let generation = Self::state(cx).update(cx, |s, cx| {
            let rs = s.repo_state_mut(id);
            let same = rs
                .pull_request_review
                .as_ref()
                .is_some_and(|r| r.pull_request.number == pr.number);
            if same {
                return None;
            }
            let saved = rs
                .pull_request_review
                .take()
                .map(|r| r.saved_selection)
                .unwrap_or_else(|| rs.selected_commits.clone());
            let generation = 1;
            rs.pull_request_review = Some(PullRequestReviewState {
                api_head: pr.head.sha.clone(),
                pull_request: pr.clone(),
                generation,
                saved_selection: saved,
                ..Default::default()
            });
            rs.selected_commits.clear();
            rs.selected_commit = None;
            rs.changeset = None;
            rs.commit_selected_file = None;
            rs.commit_diff = None;
            cx.notify();
            Some(generation)
        });
        let Some(generation) = generation else {
            return;
        };
        Self::close_blame(id, cx);
        Self::close_recent_activity(id, cx);
        Self::close_issues(id, cx);
        Self::close_releases(id, cx);
        Self::prepare_pull_request_review(id, generation, cx);
        Self::load_review_threads(id, cx);
        Self::load_review_overview(id, cx);
    }

    /// The header's close button and Escape: back to History, its
    /// selection as it was.
    pub fn close_pull_request_review(id: u64, cx: &mut dyn Host) {
        let saved = Self::state(cx).update(cx, |s, cx| {
            let rs = s.repo_states.get_mut(&id)?;
            let review = rs.pull_request_review.take()?;
            rs.selected_commits.clear();
            rs.selected_commit = None;
            rs.changeset = None;
            rs.commit_selected_file = None;
            rs.commit_diff = None;
            cx.notify();
            Some(review.saved_selection)
        });
        if let Some(saved) = saved
            && !saved.is_empty()
        {
            Self::select_commits(id, saved, cx);
        }
    }

    /// The GitHub repository the pull request lives in (its base).
    fn review_github(id: u64, cx: &dyn Host) -> Option<GitHubRepository> {
        let s = Self::state(cx).read(cx);
        let rs = s.repo_states.get(&id)?;
        rs.pull_request_review
            .as_ref()
            .and_then(|r| r.pull_request.base.repository.clone())
            .or_else(|| s.repository(id).and_then(|r| r.non_fork_github().cloned()))
    }

    /// The API client for the review's repository, or `None` signed out.
    fn review_client(id: u64, cx: &dyn Host) -> Option<(GitHubRepository, Client)> {
        let gh = Self::review_github(id, cx)?;
        let (endpoint, token, _) = Self::api_for(&gh, cx)?;
        let s = Self::state(cx).read(cx);
        let client = Client::new(endpoint, token)
            .with_sso_hint(s.flags.bool(crate::flags::ids::API_SAML_SSO_HINT))
            .with_error_details(s.flags.bool(crate::flags::ids::API_ERROR_DETAILS));
        Some((gh, client))
    }

    /// Find the shown head (the local branch tip or GitHub's, fetched when
    /// missing), the merge base and the changed files.
    fn prepare_pull_request_review(id: u64, generation: u64, cx: &mut dyn Host) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let Some((pr, local_tip, local_branch, remote)) = Self::state(cx).update(cx, |s, cx| {
            let current = s.current_pull_request(id).map(|p| p.number);
            let info = s.repo_states.get(&id)?.info.clone()?;
            let branch = info.current_branch().cloned();
            let review = s.repo_state_mut(id).pull_request_review.as_mut()?;
            if review.generation != generation {
                return None;
            }
            review.preparing = true;
            review.prepare_error = None;
            let pr = review.pull_request.clone();
            let (local_tip, local_branch) = match branch {
                Some(b) if current == Some(pr.number) => (b.tip.clone(), Some(b.name.clone())),
                _ => (None, None),
            };
            review.local_branch = local_branch.clone();
            cx.notify();
            let base_url = pr.base.repository.as_ref().map(|r| r.clone_url.clone());
            let remote = info
                .remotes
                .iter()
                .find(|r| {
                    base_url
                        .as_deref()
                        .is_some_and(|u| url_matches_remote(u, &r.url))
                })
                .or_else(|| info.remotes.iter().find(|r| r.name == "origin"))
                .or_else(|| info.remotes.first())
                .map(|r| r.name.clone());
            Some((pr, local_tip, local_branch, remote))
        }) else {
            return;
        };
        let askpass = Self::askpass_env(cx);
        let _ = local_branch;
        spawn_bg(
            cx,
            move || -> Result<Prepared, String> {
                let api_head = pr.head.sha.clone();
                let display_head = local_tip.clone().unwrap_or_else(|| api_head.clone());
                let have = |c: &str| corvene_git::commit_exists(git.clone(), &workdir, c);
                if !have(&api_head)
                    && let Some(remote) = &remote
                {
                    let refspec = format!(
                        "+refs/pull/{}/head:{}",
                        pr.number,
                        review_head_ref(pr.number)
                    );
                    if let Err(err) = corvene_git::fetch_refspec(
                        git.clone(),
                        &workdir,
                        remote,
                        &refspec,
                        askpass.as_ref(),
                    ) {
                        warn!(%err, "could not fetch the pull request head");
                    }
                }
                if !have(&display_head) {
                    return Err(format!(
                        "The pull request's head commit ({}) is not in this repository. \
                         Fetch the branch and try again.",
                        &display_head[..display_head.len().min(7)]
                    ));
                }
                // the base: GitHub's tip of the base branch when it is
                // here, else the remote-tracking branch, else the local one
                let base_candidates = [
                    pr.base.sha.clone(),
                    remote
                        .as_ref()
                        .map(|r| format!("refs/remotes/{r}/{}", pr.base.ref_name))
                        .unwrap_or_default(),
                    format!("refs/heads/{}", pr.base.ref_name),
                ];
                let base = base_candidates
                    .iter()
                    .filter(|c| !c.is_empty())
                    .find(|c| have(c))
                    .cloned()
                    .ok_or_else(|| {
                        format!(
                            "The base branch {} is not in this repository. Fetch it and try again.",
                            pr.base.ref_name
                        )
                    })?;
                let base_sha = corvene_git::merge_base(git.clone(), &workdir, &base, &display_head)
                    .map_err(|e| e.to_string())?
                    .ok_or_else(|| {
                        format!(
                            "{} and the pull request have no common history.",
                            pr.base.ref_name
                        )
                    })?;
                let github_base = if display_head == api_head {
                    Some(base_sha.clone())
                } else if have(&api_head) {
                    corvene_git::merge_base(git.clone(), &workdir, &base, &api_head)
                        .ok()
                        .flatten()
                } else {
                    None
                };
                let changeset = corvene_git::merge_base_changed_files(
                    git,
                    &workdir,
                    &base_sha,
                    &display_head,
                    &display_head,
                )
                .map_err(|e| e.to_string())?
                .unwrap_or_default();
                Ok(Prepared {
                    display_head,
                    base_sha,
                    github_base,
                    changeset,
                })
            },
            move |result, cx| {
                let reselect = Self::state(cx).update(cx, |s, cx| {
                    let review = s
                        .repo_states
                        .get_mut(&id)
                        .and_then(|rs| rs.pull_request_review.as_mut())
                        .filter(|r| r.generation == generation)?;
                    review.preparing = false;
                    match result {
                        Ok(p) => {
                            review.display_head = Some(p.display_head);
                            review.base_sha = Some(p.base_sha);
                            review.github_base = p.github_base;
                            review.changeset = Some(p.changeset);
                            review.prepare_error = None;
                        }
                        Err(err) => {
                            warn!(id, %err, "pull request review: prepare");
                            review.prepare_error = Some(err);
                            review.changeset = Some(ChangesetData::default());
                        }
                    }
                    cx.notify();
                    review.selected_file().map(str::to_string)
                });
                if let Some(path) = reselect {
                    Self::select_review_file(id, path, cx);
                }
            },
        );
    }

    /// The Overview row: the overview pane in the content area.
    pub fn select_review_overview(id: u64, cx: &mut dyn Host) {
        Self::state(cx).update(cx, |s, cx| {
            if let Some(review) = s.repo_state_mut(id).pull_request_review.as_mut()
                && review.selection != ReviewSelection::Overview
            {
                review.selection = ReviewSelection::Overview;
                review.composer = None;
                cx.notify();
            }
        });
    }

    /// A file row: its merge-base diff with the threads placed in it.
    pub fn select_review_file(id: u64, path: String, cx: &mut dyn Host) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let (hide_whitespace, remap) = {
            let s = Self::state(cx).read(cx);
            (
                s.settings.hide_whitespace_in_history_diff,
                s.flags.bool(crate::flags::ids::REVIEW_THREAD_REMAPPING),
            )
        };
        let Some(input) = Self::state(cx).update(cx, |s, cx| {
            let review = s.repo_state_mut(id).pull_request_review.as_mut()?;
            if review.selection != ReviewSelection::File(path.clone()) {
                review.composer = None;
            }
            review.selection = ReviewSelection::File(path.clone());
            if review.file.as_ref().is_none_or(|f| f.path != path) {
                review.file = Some(ReviewFile {
                    path: path.clone(),
                    ..Default::default()
                });
            }
            cx.notify();
            let display_head = review.display_head.clone()?;
            let base_sha = review.base_sha.clone()?;
            let file = review
                .changeset
                .as_ref()?
                .files
                .iter()
                .find(|f| f.path == path)?
                .clone();
            // the commits the outdated threads were made on
            let originals: Vec<String> = review
                .threads_for(&path)
                .iter()
                .filter(|(_, t)| t.line.is_none() && t.original_line.is_some())
                .filter_map(|(_, t)| t.original_commit().map(str::to_string))
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .take(MAX_ORIGINAL_BLOBS)
                .collect();
            Some((
                review.generation,
                display_head,
                base_sha,
                review.api_head.clone(),
                review.github_base.clone(),
                file,
                originals,
            ))
        }) else {
            return;
        };
        let (generation, display_head, base_sha, api_head, github_base, file, originals) = input;
        let path_for_apply = path.clone();
        spawn_bg(
            cx,
            move || {
                let diff = corvene_git::merge_base_file_diff(
                    git.clone(),
                    &workdir,
                    &file,
                    &base_sha,
                    &display_head,
                    hide_whitespace,
                    &display_head,
                )
                .unwrap_or_else(|err| {
                    warn!(id, %err, "pull request review: diff");
                    Diff::Empty
                });
                let lines = |commit: &str, path: &str| {
                    corvene_git::blob_lines(git.clone(), &workdir, commit, path)
                };
                let old_path = file.old_path.clone().unwrap_or_else(|| file.path.clone());
                let contents = (file.status.kind != FileStatusKind::Deleted)
                    .then(|| lines(&display_head, &file.path))
                    .flatten();
                let old_contents = (!matches!(
                    file.status.kind,
                    FileStatusKind::New | FileStatusKind::Untracked
                ))
                .then(|| lines(&base_sha, &old_path))
                .flatten();
                let same_head = display_head == api_head;
                let same_base = github_base.as_deref() == Some(base_sha.as_str());
                let same_as_github = same_head && same_base;
                let (maps, maps_back) = if !remap {
                    (None, None)
                } else {
                    let empty: Vec<String> = Vec::new();
                    let (right, right_back) = if same_head {
                        (LineMap::identity(), LineMap::identity())
                    } else {
                        let api = lines(&api_head, &file.path).unwrap_or_default();
                        let shown = contents.clone().unwrap_or_default();
                        (
                            LineMap::between(&api, &shown),
                            LineMap::between(&shown, &api),
                        )
                    };
                    let (left, left_back) = match &github_base {
                        _ if same_base => (LineMap::identity(), LineMap::identity()),
                        Some(gb) => {
                            let theirs = lines(gb, &old_path).unwrap_or_default();
                            let ours = old_contents.clone().unwrap_or_default();
                            (
                                LineMap::between(&theirs, &ours),
                                LineMap::between(&ours, &theirs),
                            )
                        }
                        None => (LineMap::between(&empty, &empty), LineMap::empty()),
                    };
                    let shown = contents.clone().unwrap_or_default();
                    let originals = originals
                        .into_iter()
                        .filter_map(|commit| {
                            let original = lines(&commit, &file.path)?;
                            Some((commit, LineMap::between(&original, &shown)))
                        })
                        .collect();
                    (
                        Some(FileLineMaps {
                            right,
                            left,
                            originals,
                        }),
                        Some((right_back, left_back)),
                    )
                };
                LoadedFile {
                    diff,
                    contents,
                    old_contents,
                    maps,
                    maps_back,
                    same_as_github,
                }
            },
            move |loaded, cx| {
                Self::state(cx).update(cx, |s, cx| {
                    let Some(review) = s
                        .repo_states
                        .get_mut(&id)
                        .and_then(|rs| rs.pull_request_review.as_mut())
                        .filter(|r| r.generation == generation)
                    else {
                        return;
                    };
                    let anchors = {
                        let threads = review.threads_for(&path_for_apply);
                        anchor_threads(&threads, loaded.maps.as_ref(), loaded.same_as_github)
                    };
                    let Some(file) = review.file.as_mut().filter(|f| f.path == path_for_apply)
                    else {
                        return;
                    };
                    if crate::dispatcher::replace_diff(
                        (
                            &mut file.diff,
                            &mut file.diff_contents,
                            &mut file.diff_old_contents,
                        ),
                        (
                            Arc::new(loaded.diff),
                            loaded.contents.map(Arc::new),
                            loaded.old_contents.map(Arc::new),
                        ),
                    ) {
                        file.diff_generation += 1;
                    }
                    file.maps = loaded.maps;
                    file.maps_back = loaded.maps_back;
                    file.same_as_github = loaded.same_as_github;
                    file.anchors = anchors;
                    cx.notify();
                });
            },
        );
    }

    /// Place the loaded file's threads again (after a reload).
    fn reanchor_review_file(review: &mut PullRequestReviewState) {
        let Some(path) = review.file.as_ref().map(|f| f.path.clone()) else {
            return;
        };
        let anchors = {
            let threads = review.threads_for(&path);
            let file = review.file.as_ref().expect("checked above");
            anchor_threads(&threads, file.maps.as_ref(), file.same_as_github)
        };
        if let Some(file) = review.file.as_mut() {
            file.anchors = anchors;
        }
    }

    /// Read every review thread (opened, after a write, refreshed). One
    /// load at a time; one asked for meanwhile runs after it.
    pub fn load_review_threads(id: u64, cx: &mut dyn Host) {
        let api = Self::review_client(id, cx);
        let Some((generation, number)) = Self::state(cx).update(cx, |s, cx| {
            let review = s.repo_state_mut(id).pull_request_review.as_mut()?;
            review.signed_out = api.is_none();
            if api.is_none() {
                review.threads_loaded = true;
                cx.notify();
                return None;
            }
            if review.threads_loading {
                review.threads_reload_pending = true;
                return None;
            }
            review.threads_loading = true;
            review.threads_error = None;
            cx.notify();
            Some((review.generation, review.pull_request.number))
        }) else {
            return;
        };
        let Some((gh, client)) = api else {
            return;
        };
        spawn_bg(
            cx,
            move || {
                client
                    .review_threads(&gh.owner, &gh.name, number)
                    .map(|threads| {
                        let mut bodies = HashMap::new();
                        parse_bodies(&threads.threads, &mut bodies);
                        (threads, bodies)
                    })
            },
            move |result, cx| {
                let reload = Self::state(cx).update(cx, |s, cx| {
                    let Some(review) = s
                        .repo_states
                        .get_mut(&id)
                        .and_then(|rs| rs.pull_request_review.as_mut())
                    else {
                        return false;
                    };
                    review.threads_loading = false;
                    review.threads_loaded = true;
                    let reload = std::mem::take(&mut review.threads_reload_pending);
                    match result {
                        Ok((threads, bodies)) => {
                            review.pull_request_id = Some(threads.pull_request_id);
                            review.threads = threads.threads;
                            review.threads_version += 1;
                            review.threads_error = None;
                            review.bodies.extend(bodies);
                            // a pending review that was submitted or
                            // discarded elsewhere
                            if review.pending_review.is_some()
                                && !review
                                    .threads
                                    .iter()
                                    .any(|t| t.comments.iter().any(|c| c.pending))
                                && review
                                    .pending_review
                                    .as_ref()
                                    .is_some_and(|p| p.comment_count > 0)
                            {
                                review.pending_review = None;
                            }
                            Self::reanchor_review_file(review);
                        }
                        Err(err) => {
                            if err.is_token_invalidated() {
                                warn!(id, %err, "pull request review: token invalidated");
                            }
                            review.threads_error = Some(err.to_string());
                        }
                    }
                    cx.notify();
                    reload && review.generation == generation
                });
                if reload {
                    Self::load_review_threads(id, cx);
                }
            },
        );
    }

    /// Read the overview (description, reviewers, checks, timeline). A head
    /// GitHub reports that differs from the cached one prepares the review
    /// again (the branch was pushed meanwhile).
    pub fn load_review_overview(id: u64, cx: &mut dyn Host) {
        let api = Self::review_client(id, cx);
        let Some(number) = Self::state(cx).update(cx, |s, cx| {
            let review = s.repo_state_mut(id).pull_request_review.as_mut()?;
            if api.is_none() {
                review.signed_out = true;
                cx.notify();
                return None;
            }
            if review.overview_loading {
                return None;
            }
            review.overview_loading = true;
            review.overview_error = None;
            cx.notify();
            Some(review.pull_request.number)
        }) else {
            return;
        };
        let Some((gh, client)) = api else {
            return;
        };
        spawn_bg(
            cx,
            move || {
                client
                    .pull_request_overview(&gh.owner, &gh.name, number)
                    .map(|o| {
                        let body = Arc::new(crate::markdown::parse(&o.body));
                        let mut bodies = HashMap::new();
                        bodies.insert(o.id.clone(), body);
                        for item in &o.timeline {
                            match item {
                                TimelineItem::Comment { id, body, .. } => {
                                    bodies
                                        .insert(id.clone(), Arc::new(crate::markdown::parse(body)));
                                }
                                TimelineItem::Review(r) if !r.body.is_empty() => {
                                    bodies.insert(
                                        r.id.clone(),
                                        Arc::new(crate::markdown::parse(&r.body)),
                                    );
                                }
                                _ => {}
                            }
                        }
                        (o, bodies)
                    })
            },
            move |result, cx| {
                let reprepare = Self::state(cx).update(cx, |s, cx| {
                    let review = s
                        .repo_states
                        .get_mut(&id)
                        .and_then(|rs| rs.pull_request_review.as_mut())?;
                    review.overview_loading = false;
                    let mut reprepare = None;
                    match result {
                        Ok((overview, bodies)) => {
                            review.pull_request_id = Some(overview.id.clone());
                            review.pending_review = overview.pending_review.clone();
                            review.bodies.extend(bodies);
                            if !overview.head_ref_oid.is_empty()
                                && overview.head_ref_oid != review.api_head
                            {
                                info!(id, "pull request review: head moved on GitHub");
                                review.api_head = overview.head_ref_oid.clone();
                                review.pull_request.head.sha = overview.head_ref_oid.clone();
                                review.generation += 1;
                                reprepare = Some(review.generation);
                            }
                            review.pull_request.title = overview.title.clone();
                            review.overview = Some(overview);
                            review.overview_error = None;
                        }
                        Err(err) => review.overview_error = Some(err.to_string()),
                    }
                    cx.notify();
                    reprepare
                });
                if let Some(generation) = reprepare {
                    Self::prepare_pull_request_review(id, generation, cx);
                    Self::load_review_threads(id, cx);
                }
            },
        );
    }

    /// The header's refresh: threads and overview again.
    pub fn refresh_pull_request_review(id: u64, cx: &mut dyn Host) {
        Self::state(cx).update(cx, |s, cx| {
            if let Some(review) = s.repo_state_mut(id).pull_request_review.as_mut() {
                review.error = None;
                cx.notify();
            }
        });
        Self::load_review_threads(id, cx);
        Self::load_review_overview(id, cx);
    }

    /// Where a new comment is being written (`None` closes the box).
    pub fn set_review_composer(id: u64, target: Option<ComposeTarget>, cx: &mut dyn Host) {
        Self::state(cx).update(cx, |s, cx| {
            if let Some(review) = s.repo_state_mut(id).pull_request_review.as_mut()
                && review.composer != target
            {
                review.composer = target;
                review.error = None;
                cx.notify();
            }
        });
    }

    /// Show resolved threads expanded (or collapsed to a line).
    pub fn set_show_resolved_threads(id: u64, show: bool, cx: &mut dyn Host) {
        Self::state(cx).update(cx, |s, cx| {
            if let Some(review) = s.repo_state_mut(id).pull_request_review.as_mut()
                && review.show_resolved != show
            {
                review.show_resolved = show;
                cx.notify();
            }
        });
    }

    /// Mark a write in flight; `None` when another is running.
    fn review_begin(id: u64, what: &str, cx: &mut dyn Host) -> Option<(GitHubRepository, Client)> {
        let api = Self::review_client(id, cx);
        if api.is_none() {
            Self::show_error(
                "Not signed in",
                "Sign in to the repository's GitHub account to take part in the review.",
                cx,
            );
            return None;
        }
        let started = Self::state(cx).update(cx, |s, cx| {
            let Some(review) = s.repo_state_mut(id).pull_request_review.as_mut() else {
                return false;
            };
            if review.busy.is_some() {
                return false;
            }
            review.busy = Some(what.to_string());
            review.error = None;
            cx.notify();
            true
        });
        if started { api } else { None }
    }

    /// The write finished: clear the flag, keep the error, reload.
    fn review_end(id: u64, result: Result<(), String>, reload_overview: bool, cx: &mut dyn Host) {
        Self::state(cx).update(cx, |s, cx| {
            if let Some(review) = s.repo_state_mut(id).pull_request_review.as_mut() {
                review.busy = None;
                if let Err(err) = &result {
                    warn!(id, %err, "pull request review: write");
                    review.error = Some(err.clone());
                }
                cx.notify();
            }
        });
        if result.is_ok() {
            Self::load_review_threads(id, cx);
            if reload_overview {
                Self::load_review_overview(id, cx);
            }
        }
    }

    /// The composer's comment: posted at once (`single`, GitHub's "Add
    /// single comment") or into the pending review, started when there
    /// is none ("Start a review" / "Add review comment").
    pub fn add_review_comment(id: u64, body: String, single: bool, cx: &mut dyn Host) {
        let body = body.trim().to_string();
        if body.is_empty() {
            return;
        }
        let Some((target, pr_id, commit, maps_back, pending)) = Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|rs| {
                let review = rs.pull_request_review.as_ref()?;
                let target = review.composer.clone()?;
                let file = review.shown_file().filter(|f| f.path == target.path)?;
                Some((
                    target,
                    review.pull_request_id.clone(),
                    review.api_head.clone(),
                    (file.maps_back.clone(), file.same_as_github),
                    review.pending_review.clone(),
                ))
            })
        else {
            return;
        };
        let Some(pr_id) = pr_id else {
            Self::show_error(
                "Could not comment",
                "The pull request has not been read from GitHub yet.",
                cx,
            );
            return;
        };
        // GitHub's line numbers: carry the shown diff's back
        let (maps_back, same) = maps_back;
        let back = |line: u32| -> Option<u32> {
            match (&maps_back, target.side) {
                (Some((right, _)), DiffSide::Right) => right.map(line),
                (Some((_, left)), DiffSide::Left) => left.map(line),
                (None, _) if same => Some(line),
                (None, _) => None,
            }
        };
        let Some(line) = back(target.line) else {
            Self::state(cx).update(cx, |s, cx| {
                if let Some(review) = s.repo_state_mut(id).pull_request_review.as_mut() {
                    review.error = Some(
                        "This line is not on GitHub yet: push the branch before commenting on it."
                            .to_string(),
                    );
                    cx.notify();
                }
            });
            return;
        };
        let start_line = target.start.and_then(back).filter(|s| *s < line);
        let thread = NewThread {
            path: target.path.clone(),
            line,
            side: target.side,
            start_line,
            start_side: start_line.map(|_| target.side),
            body,
        };
        let Some((_, client)) = Self::review_begin(id, "Posting the comment…", cx) else {
            return;
        };
        spawn_bg(
            cx,
            move || -> Result<Option<PendingReview>, String> {
                if single {
                    client
                        .add_single_review_comment(&pr_id, &commit, &thread)
                        .map_err(|e| e.to_string())?;
                    return Ok(None);
                }
                let mut pending = match pending {
                    Some(p) => p,
                    None => PendingReview {
                        id: client
                            .start_pending_review(&pr_id, &commit)
                            .map_err(|e| e.to_string())?,
                        body: String::new(),
                        comment_count: 0,
                    },
                };
                client
                    .add_review_thread(&pending.id, &thread)
                    .map_err(|e| e.to_string())?;
                pending.comment_count += 1;
                Ok(Some(pending))
            },
            move |result, cx| {
                let result = result.map(|pending| {
                    Self::state(cx).update(cx, |s, _| {
                        if let Some(review) = s.repo_state_mut(id).pull_request_review.as_mut() {
                            review.composer = None;
                            if pending.is_some() {
                                review.pending_review = pending;
                            }
                        }
                    });
                });
                Self::review_end(id, result, false, cx);
            },
        );
    }

    /// A reply under a thread: posted at once, or into the pending review
    /// (`pending`, started when there is none).
    pub fn reply_to_review_thread(
        id: u64,
        thread_id: String,
        body: String,
        pending: bool,
        cx: &mut dyn Host,
    ) {
        let body = body.trim().to_string();
        if body.is_empty() {
            return;
        }
        let Some((pr_id, commit, review_pending)) = Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|rs| {
                let review = rs.pull_request_review.as_ref()?;
                Some((
                    review.pull_request_id.clone(),
                    review.api_head.clone(),
                    review.pending_review.clone(),
                ))
            })
        else {
            return;
        };
        let Some((_, client)) = Self::review_begin(id, "Posting the reply…", cx) else {
            return;
        };
        spawn_bg(
            cx,
            move || -> Result<Option<PendingReview>, String> {
                if !pending {
                    client
                        .reply_to_review_thread(&thread_id, &body, None)
                        .map_err(|e| e.to_string())?;
                    return Ok(None);
                }
                let mut review = match review_pending {
                    Some(p) => p,
                    None => {
                        let pr_id =
                            pr_id.ok_or("The pull request has not been read from GitHub yet.")?;
                        PendingReview {
                            id: client
                                .start_pending_review(&pr_id, &commit)
                                .map_err(|e| e.to_string())?,
                            body: String::new(),
                            comment_count: 0,
                        }
                    }
                };
                client
                    .reply_to_review_thread(&thread_id, &body, Some(&review.id))
                    .map_err(|e| e.to_string())?;
                review.comment_count += 1;
                Ok(Some(review))
            },
            move |result, cx| {
                let result = result.map(|pending| {
                    if pending.is_some() {
                        Self::state(cx).update(cx, |s, _| {
                            if let Some(review) = s.repo_state_mut(id).pull_request_review.as_mut()
                            {
                                review.pending_review = pending;
                            }
                        });
                    }
                });
                Self::review_end(id, result, false, cx);
            },
        );
    }

    /// Resolve / unresolve a thread: shown at once, undone when GitHub
    /// refuses.
    pub fn set_review_thread_resolved(
        id: u64,
        thread_id: String,
        resolved: bool,
        cx: &mut dyn Host,
    ) {
        let Some((_, client)) = Self::review_begin(
            id,
            if resolved {
                "Resolving the thread…"
            } else {
                "Unresolving the thread…"
            },
            cx,
        ) else {
            return;
        };
        let flip = {
            let thread_id = thread_id.clone();
            move |to: bool, cx: &mut dyn Host| {
                Self::state(cx).update(cx, |s, cx| {
                    if let Some(review) = s.repo_state_mut(id).pull_request_review.as_mut() {
                        if let Some(t) = review.threads.iter_mut().find(|t| t.id == thread_id) {
                            t.is_resolved = to;
                        }
                        cx.notify();
                    }
                });
            }
        };
        flip(resolved, cx);
        spawn_bg(
            cx,
            move || {
                client
                    .set_review_thread_resolved(&thread_id, resolved)
                    .map_err(|e| e.to_string())
            },
            move |result, cx| {
                let result = match result {
                    Ok(now) => {
                        flip(now, cx);
                        Ok(())
                    }
                    Err(err) => {
                        flip(!resolved, cx);
                        Err(err)
                    }
                };
                Self::review_end(id, result, false, cx);
            },
        );
    }

    /// Review changes… › Submit: the pending review goes out with the
    /// verdict, or a review that is only the verdict is added.
    pub fn submit_pull_request_review(
        id: u64,
        event: ReviewEvent,
        body: String,
        cx: &mut dyn Host,
    ) {
        let Some((pr_id, commit, pending, number, url)) = Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|rs| {
                let review = rs.pull_request_review.as_ref()?;
                Some((
                    review.pull_request_id.clone(),
                    review.api_head.clone(),
                    review.pending_review.clone(),
                    review.pull_request.number,
                    review.html_url(),
                ))
            })
        else {
            return;
        };
        let Some((_, client)) = Self::review_begin(id, "Submitting the review…", cx) else {
            return;
        };
        Self::close_popup_if(|p| matches!(p, Popup::SubmitPullRequestReview { .. }), cx);
        let body = body.trim().to_string();
        spawn_bg(
            cx,
            move || -> Result<(), String> {
                match pending {
                    Some(p) => client
                        .submit_review(&p.id, event, &body)
                        .map_err(|e| e.to_string()),
                    None => {
                        let pr_id =
                            pr_id.ok_or("The pull request has not been read from GitHub yet.")?;
                        client
                            .add_review(&pr_id, &commit, event, &body)
                            .map(|_| ())
                            .map_err(|e| e.to_string())
                    }
                }
            },
            move |result, cx| {
                if result.is_ok() {
                    Self::state(cx).update(cx, |s, _| {
                        if let Some(review) = s.repo_state_mut(id).pull_request_review.as_mut() {
                            review.pending_review = None;
                        }
                    });
                    Self::set_banner(
                        Banner::ReviewSubmitted {
                            number,
                            event,
                            html_url: url.unwrap_or_default(),
                        },
                        cx,
                    );
                } else {
                    // the dialog closed: say why
                    if let Err(err) = &result {
                        Self::show_error("Could not submit the review", err.clone(), cx);
                    }
                }
                Self::review_end(id, result, true, cx);
            },
        );
    }

    /// Discard the pending review and its comments.
    pub fn discard_pending_review(id: u64, cx: &mut dyn Host) {
        let Some(pending) = Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|rs| rs.pull_request_review.as_ref())
            .and_then(|r| r.pending_review.clone())
        else {
            return;
        };
        let Some((_, client)) = Self::review_begin(id, "Discarding the review…", cx) else {
            return;
        };
        Self::close_popup_if(|p| matches!(p, Popup::SubmitPullRequestReview { .. }), cx);
        spawn_bg(
            cx,
            move || {
                client
                    .delete_pending_review(&pending.id)
                    .map_err(|e| e.to_string())
            },
            move |result, cx| {
                if result.is_ok() {
                    Self::state(cx).update(cx, |s, _| {
                        if let Some(review) = s.repo_state_mut(id).pull_request_review.as_mut() {
                            review.pending_review = None;
                        }
                    });
                }
                Self::review_end(id, result, true, cx);
            },
        );
    }

    /// Edit one of the viewer's comments.
    pub fn update_review_comment(id: u64, comment_id: String, body: String, cx: &mut dyn Host) {
        let body = body.trim().to_string();
        if body.is_empty() {
            return;
        }
        let Some((_, client)) = Self::review_begin(id, "Saving the comment…", cx) else {
            return;
        };
        spawn_bg(
            cx,
            move || {
                client
                    .update_review_comment(&comment_id, &body)
                    .map_err(|e| e.to_string())
            },
            move |result, cx| Self::review_end(id, result, false, cx),
        );
    }

    /// Delete one of the viewer's comments.
    pub fn delete_review_comment(id: u64, comment_id: String, cx: &mut dyn Host) {
        let Some((_, client)) = Self::review_begin(id, "Deleting the comment…", cx) else {
            return;
        };
        spawn_bg(
            cx,
            move || {
                client
                    .delete_review_comment(&comment_id)
                    .map_err(|e| e.to_string())
            },
            move |result, cx| {
                if result.is_ok() {
                    Self::state(cx).update(cx, |s, _| {
                        if let Some(review) = s.repo_state_mut(id).pull_request_review.as_mut()
                            && let Some(p) = review.pending_review.as_mut()
                        {
                            p.comment_count = p.comment_count.saturating_sub(1);
                        }
                    });
                }
                Self::review_end(id, result, false, cx);
            },
        );
    }

    /// The overview pane's comment box: a comment on the conversation.
    pub fn add_pull_request_conversation_comment(id: u64, body: String, cx: &mut dyn Host) {
        let body = body.trim().to_string();
        if body.is_empty() {
            return;
        }
        let Some(pr_id) = Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|rs| rs.pull_request_review.as_ref())
            .and_then(|r| r.pull_request_id.clone())
        else {
            return;
        };
        let Some((_, client)) = Self::review_begin(id, "Posting the comment…", cx) else {
            return;
        };
        spawn_bg(
            cx,
            move || {
                client
                    .add_pull_request_comment(&pr_id, &body)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            },
            move |result, cx| Self::review_end(id, result, true, cx),
        );
    }

    /// The pull request's page in the browser.
    pub fn open_pull_request_review_on_github(id: u64, cx: &mut dyn Host) {
        let url = Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|rs| rs.pull_request_review.as_ref())
            .and_then(|r| r.html_url());
        if let Some(url) = url {
            Self::open_url(&url, cx);
        }
    }

    /// Review changes…: the submit dialog.
    pub fn show_submit_pull_request_review(id: u64, cx: &mut dyn Host) {
        if Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|rs| rs.pull_request_review.as_ref())
            .is_some()
        {
            Self::show_popup(Popup::SubmitPullRequestReview { repo: id }, cx);
        }
    }
}

/// `CORVENE_POPUP=pull-request-review`: the review of a sample pull
/// request, with sample threads, without GitHub (the writes fail).
pub fn install_samples(id: u64, cx: &mut dyn Host) {
    let pr = crate::samples::pull_request(id, cx);
    Dispatcher::review_pull_request(id, pr, cx);
    Dispatcher::state(cx).update(cx, |s, cx| {
        let Some(review) = s.repo_state_mut(id).pull_request_review.as_mut() else {
            return;
        };
        review.threads = crate::samples::review_threads();
        review.threads_loaded = true;
        review.threads_version += 1;
        parse_bodies(&review.threads, &mut review.bodies);
        review.overview = Some(crate::samples::pull_request_overview(&review.pull_request));
        review.pull_request_id = Some("PR_sample".to_string());
        cx.notify();
    });
}
