//! Corvene (`895-bulk-delete-branches`): deleting several local branches at
//! once from the branch list's multi-selection. GHD
//! (`ui/delete-branch/delete-branch-dialog.tsx`, `_deleteBranch` in
//! `lib/stores/app-store.ts`) deletes one branch per confirmation.
//!
//! The confirmation lists the branches with what deleting them would lose
//! ([`DeleteBranchesPreview`]); the deletions run one after another, the
//! failures are reported in one error and, with `861-undo-delete-branch`,
//! one Undo recreates every deleted local branch.

use std::collections::HashMap;

use crate::host::{AsyncCtx, Host};
use corvene_models::{Branch, BranchKind};

use crate::Dispatcher;
use crate::mco::Banner;

/// What deleting each selected branch would lose, by branch name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeleteBranchesPreview {
    /// The branches the preview was computed for.
    pub branches: Vec<String>,
    /// Commits the default branch (local or its upstream) lacks.
    pub unmerged: HashMap<String, u32>,
    /// Commits the branch's upstream lacks (published branches only).
    pub unpushed: HashMap<String, u32>,
    /// The default branch the commits were compared with.
    pub default_branch: Option<String>,
}

/// A branch older than this is marked as old in the confirmation.
pub const OLD_BRANCH_DAYS: i64 = 30;

/// Whether `branch`'s upstream is configured but its remote-tracking branch
/// is gone (deleted on the remote and pruned).
pub fn upstream_gone(branch: &Branch, branches: &[Branch]) -> bool {
    branch.upstream.as_deref().is_some_and(|upstream| {
        !branches
            .iter()
            .any(|b| b.kind == BranchKind::Remote && b.full_name == upstream)
    })
}

/// Whether the tip of `branch` is older than [`OLD_BRANCH_DAYS`] at `now`
/// (seconds since the epoch).
pub fn is_old(branch: &Branch, now: i64) -> bool {
    branch
        .tip_time
        .is_some_and(|t| t > 0 && now - t > OLD_BRANCH_DAYS * 24 * 60 * 60)
}

/// The summary of the deletions that failed, one line per branch.
pub fn failure_summary(failures: &[(String, String)]) -> String {
    failures
        .iter()
        .map(|(name, err)| format!("{name}: {}", err.trim()))
        .collect::<Vec<_>>()
        .join("\n")
}

impl Dispatcher {
    /// Fills `RepositoryState::delete_branches_preview` for `names`.
    pub fn preview_delete_branches(id: u64, names: Vec<String>, cx: &mut dyn Host) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        // (branch, its upstream if fetched), and the default branch's refs
        let (branches, default_refs, default_name) = {
            let s = Self::state(cx).read(cx);
            let rs = s.repo_states.get(&id);
            let all = rs
                .and_then(|r| r.info.as_ref())
                .map(|i| i.branches.clone())
                .unwrap_or_default();
            let default = rs.and_then(|r| r.default_branch.clone()).and_then(|d| {
                all.iter()
                    .find(|b| b.name == d && b.kind == BranchKind::Local)
                    .cloned()
            });
            let mut default_refs: Vec<String> = Vec::new();
            if let Some(default) = &default {
                default_refs.push(default.full_name.clone());
                if let Some(upstream) = &default.upstream
                    && !upstream_gone(default, &all)
                {
                    default_refs.push(upstream.clone());
                }
            }
            let branches: Vec<(Branch, Option<String>)> = names
                .iter()
                .filter_map(|n| {
                    all.iter()
                        .find(|b| &b.name == n && b.kind == BranchKind::Local)
                })
                .map(|b| {
                    let upstream = b.upstream.clone().filter(|_| !upstream_gone(b, &all));
                    (b.clone(), upstream)
                })
                .collect();
            (branches, default_refs, default.map(|d| d.name))
        };
        let task = cx.background_executor().spawn(async move {
            let mut unmerged = HashMap::new();
            let mut unpushed = HashMap::new();
            for (branch, upstream) in &branches {
                if !default_refs.is_empty() && Some(&branch.name) != default_name.as_ref() {
                    let n = corvene_git::commits_not_in(
                        git.clone(),
                        &workdir,
                        &branch.full_name,
                        &default_refs,
                    )
                    .unwrap_or(0);
                    unmerged.insert(branch.name.clone(), n);
                }
                if let Some(upstream) = upstream {
                    let n = corvene_git::commits_not_in(
                        git.clone(),
                        &workdir,
                        &branch.full_name,
                        std::slice::from_ref(upstream),
                    )
                    .unwrap_or(0);
                    unpushed.insert(branch.name.clone(), n);
                }
            }
            DeleteBranchesPreview {
                branches: branches.into_iter().map(|(b, _)| b.name).collect(),
                unmerged,
                unpushed,
                default_branch: default_name,
            }
        });
        Self::state(cx).update(cx, |s, _| {
            s.repo_state_mut(id).delete_branches_preview = None
        });
        cx.spawn(async move |cx: &mut AsyncCtx| {
            let preview = task.await;
            cx.update(|cx| {
                Self::state(cx).update(cx, |s, cx| {
                    s.repo_state_mut(id).delete_branches_preview = Some(preview);
                    cx.notify();
                });
            });
        })
        .detach();
    }

    /// Deletes the local branches `branches` names one after another, each
    /// with its remote branch when its flag is set; failures are reported
    /// together and the deleted local branches get one Undo banner
    /// (`861-undo-delete-branch`). The current branch is skipped.
    pub fn delete_branches(id: u64, branches: Vec<(String, bool)>, cx: &mut dyn Host) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let (targets, undo, qualified) = {
            let s = Self::state(cx).read(cx);
            let info = s.repo_states.get(&id).and_then(|r| r.info.as_ref());
            let current = info
                .and_then(|i| i.current_branch())
                .map(|b| b.name.clone());
            let targets: Vec<(Branch, bool)> = branches
                .into_iter()
                .filter(|(n, _)| Some(n) != current.as_ref())
                .filter_map(|(n, remote)| {
                    info.and_then(|i| {
                        i.branches
                            .iter()
                            .find(|b| b.name == n && b.kind == BranchKind::Local)
                    })
                    .map(|b| (b.clone(), remote))
                })
                .collect();
            (
                targets,
                s.flags.bool(crate::flags::ids::UNDO_DELETE_BRANCH),
                s.flags.bool(crate::flags::ids::QUALIFIED_PUSH_REFSPECS),
            )
        };
        if targets.is_empty() {
            return;
        }
        // the signed-in accounts' credentials for every remote involved
        let mut askpass = None;
        for (branch, _) in targets.iter().filter(|(_, remote)| *remote) {
            let url = branch.upstream_remote_name().and_then(|name| {
                Self::state(cx)
                    .read(cx)
                    .repo_states
                    .get(&id)
                    .and_then(|r| r.info.as_ref())
                    .and_then(|i| i.remotes.iter().find(|r| r.name == name))
                    .map(|r| r.url.clone())
            });
            if let Some(url) = url {
                Self::arm_credential_helper_for(id, &url, cx);
                askpass = Self::askpass_env_for(id, &url, cx);
            }
        }
        let task = cx.background_executor().spawn(async move {
            let mut deleted: Vec<(String, String)> = Vec::new();
            let mut failures: Vec<(String, String)> = Vec::new();
            for (branch, remote) in targets {
                if let Err(err) =
                    corvene_git::delete_local_branch(git.clone(), &workdir, &branch.name)
                {
                    failures.push((branch.name.clone(), err.to_string()));
                    continue;
                }
                if let Some(sha) = &branch.tip {
                    deleted.push((branch.name.clone(), sha.clone()));
                }
                if remote
                    && let (Some(remote_name), Some(remote_branch)) = (
                        branch.upstream_remote_name(),
                        branch.upstream_without_remote(),
                    )
                {
                    // `867-qualified-push-refspecs`
                    let name = if qualified {
                        format!("refs/heads/{remote_branch}")
                    } else {
                        remote_branch.to_string()
                    };
                    if let Err(err) = corvene_git::delete_remote_branch_with(
                        git.clone(),
                        &workdir,
                        remote_name,
                        &name,
                        askpass.as_ref(),
                    ) {
                        failures.push((format!("{remote_name}/{remote_branch}"), err.to_string()));
                    }
                }
            }
            (deleted, failures)
        });
        cx.spawn(async move |cx: &mut AsyncCtx| {
            let (deleted, failures) = task.await;
            cx.update(|cx| {
                if !failures.is_empty() {
                    Self::show_error(
                        if failures.len() == 1 {
                            "Could not delete a branch"
                        } else {
                            "Could not delete some branches"
                        },
                        failure_summary(&failures),
                        cx,
                    );
                }
                Self::refresh_repository(id, cx);
                if undo && !deleted.is_empty() {
                    Self::set_banner(
                        match <[_; 1]>::try_from(deleted) {
                            Ok([(branch, sha)]) => Banner::BranchDeleted {
                                repo: id,
                                branch,
                                sha,
                            },
                            Err(branches) => Banner::BranchesDeleted { repo: id, branches },
                        },
                        cx,
                    );
                }
            });
        })
        .detach();
    }

    /// The "Deleted N branches" banner's Undo: recreate every branch at the
    /// commit it pointed at.
    pub fn restore_deleted_branches(id: u64, branches: Vec<(String, String)>, cx: &mut dyn Host) {
        let count = branches.len();
        Self::run_history_op_then(
            id,
            "Could not restore branches",
            move |git, workdir| {
                for (branch, sha) in &branches {
                    corvene_git::create_branch(
                        git.clone(),
                        &workdir,
                        branch,
                        Some(sha.as_str()),
                        true,
                    )?;
                }
                Ok(())
            },
            move |cx| Self::set_banner(Banner::BranchesRestored { count }, cx),
            cx,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn branch(
        name: &str,
        kind: BranchKind,
        upstream: Option<&str>,
        tip_time: Option<i64>,
    ) -> Branch {
        Branch {
            name: name.into(),
            kind,
            full_name: match kind {
                BranchKind::Local => format!("refs/heads/{name}"),
                BranchKind::Remote => format!("refs/remotes/{name}"),
            },
            tip: None,
            upstream: upstream.map(str::to_string),
            tip_time,
            tip_author: None,
            remote_name: None,
        }
    }

    #[test]
    fn upstream_gone_needs_a_configured_upstream_without_remote_branch() {
        let all = vec![
            branch("a", BranchKind::Local, Some("refs/remotes/origin/a"), None),
            branch("b", BranchKind::Local, Some("refs/remotes/origin/b"), None),
            branch("c", BranchKind::Local, None, None),
            branch("origin/a", BranchKind::Remote, None, None),
        ];
        assert!(!upstream_gone(&all[0], &all));
        assert!(upstream_gone(&all[1], &all));
        assert!(!upstream_gone(&all[2], &all));
    }

    #[test]
    fn old_means_a_tip_more_than_thirty_days_back() {
        let day = 24 * 60 * 60;
        let now = 100 * day;
        assert!(is_old(
            &branch("a", BranchKind::Local, None, Some(now - 31 * day)),
            now
        ));
        assert!(!is_old(
            &branch("a", BranchKind::Local, None, Some(now - 29 * day)),
            now
        ));
        assert!(!is_old(&branch("a", BranchKind::Local, None, None), now));
    }

    #[test]
    fn failures_are_one_line_each() {
        let summary = failure_summary(&[
            ("a".into(), "error: boom\n".into()),
            ("origin/b".into(), "denied".into()),
        ]);
        assert_eq!(summary, "a: error: boom\norigin/b: denied");
    }
}
