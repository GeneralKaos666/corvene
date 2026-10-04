//! GHD `BranchPruner` (`app/src/lib/stores/helpers/branch-pruner.ts`):
//! for a repository on GitHub, at most once a day, delete the local
//! branches that are merged into the default branch and whose upstream
//! branch is gone (deleted on the remote and pruned by a fetch). Spared:
//! the current branch, GHD's reserved names (`main`, `master`, `gh-pages`,
//! `develop`, …), branches checked out in the last two weeks (HEAD's
//! reflog), branches checked out in a linked worktree, and branches without
//! an upstream. Nothing is pruned without a default branch.
//!
//! [`BranchPruner::run_once`] is blocking (git runs on the calling thread).
//! `Dispatcher::start_background_pruner` prunes when a repository is
//! selected and every four hours while it stays selected
//! (`Dispatcher::prune_branches`): it stores the new last prune date first
//! (`crate::repositories_store::update_last_prune_date`), as GHD does, runs
//! the pruner on the background executor and refreshes the repository
//! afterwards. GHD reads the default branch and the branches from whatever
//! the repository's state holds when it is selected (nothing yet for a
//! repository not loaded this session); Corvene waits for the refresh that
//! selecting starts. The remote branches the upstreams are checked against
//! come from that same refresh (GHD lists `refs/remotes/` again).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use corvene_git::{GitBinary, format_as_local_ref};
use corvene_models::{Branch, BranchKind, Repository};
use tracing::{info, warn};

/// GHD `BackgroundPruneMinimumInterval`: how often a selected repository is
/// checked.
pub const BACKGROUND_PRUNE_MINIMUM_INTERVAL: Duration = Duration::from_secs(4 * 60 * 60);

/// Only prune if it's been at least this long since the last time.
const PRUNE_THRESHOLD: Duration = Duration::from_secs(24 * 60 * 60);

/// Branches checked out this recently are kept.
const RECENT_CHECKOUT_WINDOW: Duration = Duration::from_secs(14 * 24 * 60 * 60);

/// GHD `ReservedRefs`: never pruned.
pub const RESERVED_REFS: [&str; 10] = [
    "HEAD",
    "refs/heads/main",
    "refs/heads/master",
    "refs/heads/gh-pages",
    "refs/heads/develop",
    "refs/heads/dev",
    "refs/heads/development",
    "refs/heads/trunk",
    "refs/heads/devel",
    "refs/heads/release",
];

/// GHD `PruneRuntimeOptions`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PruneOptions {
    /// Skip when the last prune is less than a day old.
    pub enforce_prune_threshold: bool,
    /// Delete the branches found (otherwise only log them).
    pub delete_branch: bool,
}

impl PruneOptions {
    /// GHD `DefaultPruneOptions`.
    pub const DEFAULT: Self = Self {
        enforce_prune_threshold: true,
        delete_branch: true,
    };
}

/// What a prune did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PruneRun {
    /// GHD `updateLastPruneDate(repository, Date.now())`, done first thing
    /// once the threshold check passes: the date to store, `None` when the
    /// prune was skipped.
    pub last_prune_date: Option<SystemTime>,
    /// The local branches deleted (short names).
    pub pruned: Vec<String>,
    /// The run got to its end (GHD then calls `onPruneCompleted`, which
    /// refreshes the repository).
    pub completed: bool,
}

/// GHD `BranchPruner` for one repository, with what it reads from the
/// repository's state.
pub struct BranchPruner<'a> {
    git: Arc<GitBinary>,
    repository: &'a Repository,
    /// GHD `branchesState.allBranches` (local and remote).
    branches: &'a [Branch],
    /// GHD `branchesState.defaultBranch`'s name.
    default_branch: Option<&'a str>,
    /// GHD `RepositoriesStore.getLastPruneDate(repository)`.
    last_prune_date: Option<SystemTime>,
}

impl<'a> BranchPruner<'a> {
    pub fn new(
        git: Arc<GitBinary>,
        repository: &'a Repository,
        branches: &'a [Branch],
        default_branch: Option<&'a str>,
        last_prune_date: Option<SystemTime>,
    ) -> Self {
        Self {
            git,
            repository,
            branches,
            default_branch,
            last_prune_date,
        }
    }

    /// GHD `runOnce`.
    pub fn run_once(&self) -> PruneRun {
        self.prune_local_branches(PruneOptions::DEFAULT, SystemTime::now())
    }

    /// GHD `pruneLocalBranches(options)`, at `now`.
    pub fn prune_local_branches(&self, options: PruneOptions, now: SystemTime) -> PruneRun {
        let mut run = PruneRun::default();
        if self.repository.github.is_none() {
            return run;
        }
        let path = self.repository.path.as_path();
        if options.enforce_prune_threshold && !is_due(self.last_prune_date, now) {
            info!(
                path = %path.display(),
                last = ?self.last_prune_date,
                "[BranchPruner] last prune is recent, skipping"
            );
            return run;
        }
        // update the last prune date first thing after we check it
        run.last_prune_date = Some(now);

        let Some(default_branch) = self.default_branch else {
            return run;
        };
        let merged = self.find_branches_merged_into_default_branch(default_branch);
        if merged.is_empty() {
            info!(path = %path.display(), "[BranchPruner] No branches to prune.");
            return run;
        }

        // all branches checked out within the past two weeks
        let two_weeks_ago = now.checked_sub(RECENT_CHECKOUT_WINDOW).unwrap_or(now);
        let recently_checked_out: HashSet<String> =
            match corvene_git::get_branch_checkouts(self.git.clone(), path, two_weeks_ago) {
                Ok(checkouts) => checkouts.keys().map(|b| format_as_local_ref(b)).collect(),
                Err(err) => {
                    warn!(%err, "[BranchPruner] could not read the recent checkouts");
                    return run;
                }
            };
        // branches checked out in linked worktrees
        let worktree_branches: HashSet<String> =
            match corvene_git::list_worktrees(self.git.clone(), path) {
                Ok(worktrees) => worktrees.into_iter().filter_map(|w| w.branch).collect(),
                Err(err) => {
                    warn!(%err, "[BranchPruner] could not list the worktrees");
                    return run;
                }
            };

        let mut ready: Vec<&String> = merged
            .keys()
            .filter(|reference| {
                !RESERVED_REFS.contains(&reference.as_str())
                    && !recently_checked_out.contains(*reference)
                    && !worktree_branches.contains(*reference)
                    && upstream_is_gone(reference, self.branches)
            })
            .collect();
        ready.sort();
        info!(
            path = %path.display(),
            count = ready.len(),
            default_branch,
            "[BranchPruner] pruning branches merged into the default branch"
        );

        for reference in ready {
            let Some(name) = reference.strip_prefix("refs/heads/") else {
                continue;
            };
            if !options.delete_branch {
                info!(branch = name, "[BranchPruner] branch marked for deletion");
                continue;
            }
            match corvene_git::delete_local_branch(self.git.clone(), path, name) {
                Ok(()) => {
                    info!(branch = name, sha = ?merged.get(reference), "[BranchPruner] pruned branch");
                    run.pruned.push(name.to_string());
                }
                Err(err) => warn!(branch = name, %err, "[BranchPruner] could not delete branch"),
            }
        }
        run.completed = true;
        run
    }

    /// GHD `findBranchesMergedIntoDefaultBranch`: canonical ref → sha of
    /// the local branches merged into the default branch, without the
    /// current branch.
    fn find_branches_merged_into_default_branch(
        &self,
        default_branch: &str,
    ) -> HashMap<String, String> {
        let path = self.repository.path.as_path();
        let mut merged = corvene_git::get_merged_branches(self.git.clone(), path, default_branch)
            .unwrap_or_else(|err| {
                warn!(%err, "[BranchPruner] could not list the merged branches");
                HashMap::new()
            });
        // remove the current branch
        if let Ok(Some(current)) = corvene_git::get_symbolic_ref(self.git.clone(), path, "HEAD") {
            merged.remove(&current);
        }
        merged
    }
}

/// GHD's threshold check: a repository is pruned when it never was or the
/// last prune is at least 24 hours old.
pub fn is_due(last_prune_date: Option<SystemTime>, now: SystemTime) -> bool {
    let threshold = now.checked_sub(PRUNE_THRESHOLD).unwrap_or(now);
    last_prune_date.is_none_or(|last| last <= threshold)
}

/// GHD `getUpstreamRefForLocalBranchRef` checked against the remote
/// branches: the branch named by the canonical `reference` has an upstream,
/// and no remote-tracking branch of that name exists any more. A branch
/// whose upstream cannot be determined is never pruned.
fn upstream_is_gone(reference: &str, branches: &[Branch]) -> bool {
    let Some(branch) = branches
        .iter()
        .find(|b| format_as_local_ref(&b.name) == reference)
    else {
        return false;
    };
    let Some(upstream) = branch.upstream.as_deref() else {
        return false;
    };
    !branches
        .iter()
        .any(|b| b.kind == BranchKind::Remote && b.full_name == upstream)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn branch(name: &str, kind: BranchKind, upstream: Option<&str>) -> Branch {
        Branch {
            name: name.to_string(),
            kind,
            full_name: match kind {
                BranchKind::Local => format!("refs/heads/{name}"),
                BranchKind::Remote => format!("refs/remotes/{name}"),
            },
            tip: None,
            upstream: upstream.map(str::to_string),
            tip_time: None,
            tip_author: None,
            remote_name: None,
        }
    }

    #[test]
    fn only_a_branch_whose_upstream_is_gone_qualifies() {
        let branches = [
            branch("gone", BranchKind::Local, Some("refs/remotes/origin/gone")),
            branch("kept", BranchKind::Local, Some("refs/remotes/origin/kept")),
            branch("local", BranchKind::Local, None),
            branch("origin/kept", BranchKind::Remote, None),
        ];
        assert!(upstream_is_gone("refs/heads/gone", &branches));
        assert!(!upstream_is_gone("refs/heads/kept", &branches));
        assert!(!upstream_is_gone("refs/heads/local", &branches));
        assert!(!upstream_is_gone("refs/heads/unknown", &branches));
    }
}
