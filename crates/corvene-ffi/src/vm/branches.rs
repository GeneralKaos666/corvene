//! The branch picker: the current branch, the default one, the recent
//! ones and the rest (GHD's `BranchList` groups), with the sync button's
//! state next to them.

use corvene_core::AppState;
use corvene_models::{BranchKind, Tip};

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct BranchVm {
    pub name: String,
    pub remote: bool,
    pub current: bool,
    pub upstream: Option<String>,
    /// Unix seconds of the tip commit, when known.
    pub tip_time: Option<i64>,
}

/// GHD `PushPullButton` states.
#[derive(uniffi::Enum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncActionVm {
    /// No remote: "Publish repository".
    PublishRepository,
    /// No upstream: "Publish branch".
    PublishBranch,
    Fetch,
    Pull,
    Push,
    /// Something runs: `progress` says what.
    Busy,
}

#[derive(uniffi::Record, Clone, Debug, PartialEq)]
pub struct BranchesVm {
    pub repo: u64,
    /// `None` on an unborn or detached HEAD.
    pub current: Option<String>,
    pub detached_sha: Option<String>,
    pub default_branch: Option<String>,
    pub recent: Vec<String>,
    pub branches: Vec<BranchVm>,
    pub ahead: Option<u32>,
    pub behind: Option<u32>,
    pub sync: SyncActionVm,
    pub sync_progress_title: Option<String>,
    pub sync_progress: Option<f32>,
    pub last_fetched_at: Option<i64>,
    pub stash_count: u32,
}

pub fn branches(s: &AppState, repo: u64) -> Option<BranchesVm> {
    let rs = s.repo_states.get(&repo)?;
    let info = rs.info.as_ref();
    let (current, detached_sha) = match info.map(|i| &i.tip) {
        Some(Tip::Valid { branch }) => (Some(branch.name.clone()), None),
        Some(Tip::Unborn { name }) => (Some(name.clone()), None),
        Some(Tip::Detached { sha }) => (None, Some(sha.clone())),
        Some(Tip::Unknown) | None => (None, None),
    };
    let branches = info
        .map(|i| {
            i.branches
                .iter()
                .map(|b| BranchVm {
                    name: b.name.clone(),
                    remote: b.kind == BranchKind::Remote,
                    current: current.as_deref() == Some(&b.name) && b.kind == BranchKind::Local,
                    upstream: b.upstream.clone(),
                    tip_time: b.tip_time,
                })
                .collect()
        })
        .unwrap_or_default();
    let ahead_behind = rs
        .ahead_behind
        .as_ref()
        .or_else(|| info.and_then(|i| i.ahead_behind.as_ref()));
    let has_remote = info.is_some_and(|i| !i.remotes.is_empty());
    let has_upstream = match info.map(|i| &i.tip) {
        Some(Tip::Valid { branch }) => branch.upstream.is_some(),
        _ => false,
    };
    let progress = rs.push_pull_progress.as_ref();
    let sync = if rs.push_pull_in_progress || progress.is_some() {
        SyncActionVm::Busy
    } else if !has_remote {
        SyncActionVm::PublishRepository
    } else if !has_upstream {
        SyncActionVm::PublishBranch
    } else {
        match ahead_behind {
            Some(ab) if ab.behind > 0 => SyncActionVm::Pull,
            Some(ab) if ab.ahead > 0 => SyncActionVm::Push,
            _ => SyncActionVm::Fetch,
        }
    };
    Some(BranchesVm {
        repo,
        current,
        detached_sha,
        default_branch: rs.default_branch.clone(),
        recent: rs.recent_branches.clone(),
        branches,
        ahead: ahead_behind.map(|ab| ab.ahead),
        behind: ahead_behind.map(|ab| ab.behind),
        sync,
        sync_progress_title: progress.map(|p| p.title.clone()),
        sync_progress: progress.map(|p| p.value),
        last_fetched_at: rs.last_fetched.map(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
                .unwrap_or(0)
        }),
        stash_count: u32::try_from(rs.stash_count).unwrap_or(u32::MAX),
    })
}
