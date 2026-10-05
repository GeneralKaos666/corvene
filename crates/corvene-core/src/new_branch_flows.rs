//! Corvene `787-commit-to-new-branch` (no GHD counterpart): work that turns
//! out to belong on its own branch gets there in one go.
//!
//! - Commit form gear › "Commit to New Branch…": a name, then a branch at
//!   `HEAD` that carries the uncommitted changes (`git checkout -b`), the
//!   commit (through [`Dispatcher::commit_with`]'s checks), the branch
//!   published and the Create Pull Request page opened (GitHub only).
//! - History › "Create Branch from Commits…" for the current branch's newest
//!   commits: a branch at `HEAD` checked out, then published with the pull
//!   request page. When the commits were never pushed, the dialog's
//!   (unticked) "Remove them from <branch>" also moves the old branch back
//!   to before them ([`corvene_git::move_branch_back`]: compare-and-swap,
//!   refused for a branch checked out elsewhere or commits a remote has;
//!   the new branch keeps the commits).
//!
//! Any failure stops the chain at that step with the usual error; the
//! repository is left on the new branch.

use crate::dispatcher::Dispatcher;
use crate::host::Host;
use crate::state::RepositoryState;

/// What happens once a commit landed (`CommitChecks::after`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AfterCommit {
    #[default]
    Nothing,
    /// Publish the branch, then open its Create Pull Request page.
    PublishAndOpenPullRequest,
}

/// "Remove them from <branch>": move `branch` from `expected_tip` back to
/// `target` (the parent of the oldest of `commits`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveBack {
    pub branch: String,
    pub target: String,
    pub expected_tip: String,
}

/// History › Create Branch from Commits… for a selection: the commits,
/// newest first, and whether the old branch can give them up.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FromCommitsPlan {
    pub commits: Vec<String>,
    pub move_back: Option<MoveBack>,
}

/// The plan for `selection` (History's selected shas): `None` unless they
/// are the current branch's newest commits (the first ones of its history,
/// in any click order). The old branch can give them up when they form one
/// first-parent line with a parent before them and look unpushed (ahead of
/// the upstream, or no remote has them as far as the state knows); git
/// checks again before moving anything.
pub fn from_commits_plan(rs: &RepositoryState, selection: &[String]) -> Option<FromCommitsPlan> {
    let info = rs.info.as_ref()?;
    let branch = info.current_branch()?;
    let n = selection.len();
    if n == 0 || rs.commits.len() < n || rs.compare.is_comparing() {
        return None;
    }
    let newest = &rs.commits[..n];
    if !newest.iter().all(|c| selection.contains(&c.sha)) {
        return None;
    }
    let commits: Vec<String> = newest.iter().map(|c| c.sha.clone()).collect();
    let linear = newest.iter().all(|c| c.parents.len() == 1)
        && newest
            .windows(2)
            .all(|pair| pair[0].parents.first() == Some(&pair[1].sha));
    let unpushed = if branch.upstream.is_some() {
        rs.ahead_behind.is_some_and(|ab| ab.ahead as usize >= n)
    } else {
        rs.unpublished_commits
            .as_ref()
            .is_none_or(|shas| commits.iter().all(|sha| shas.contains(sha)))
    };
    let move_back = (linear && unpushed)
        .then(|| {
            Some(MoveBack {
                branch: branch.name.clone(),
                target: newest.last()?.parents.first()?.clone(),
                expected_tip: newest.first()?.sha.clone(),
            })
        })
        .flatten();
    Some(FromCommitsPlan { commits, move_back })
}

impl Dispatcher {
    /// "Commit to New Branch…" › Create Branch and Commit.
    pub fn commit_to_new_branch(
        id: u64,
        name: String,
        summary: String,
        description: String,
        cx: &mut dyn Host,
    ) {
        Self::new_branch_at_head(id, name, None, cx, move |_, cx| {
            let checks = crate::commit_checks::CommitChecks {
                after: AfterCommit::PublishAndOpenPullRequest,
                ..Default::default()
            };
            Self::commit_with(id, summary, description, checks, cx);
        });
    }

    /// History › "Create Branch from Commits…" › Create Branch.
    pub fn create_branch_from_commits(
        id: u64,
        name: String,
        commits: Vec<String>,
        move_back: Option<MoveBack>,
        cx: &mut dyn Host,
    ) {
        let moved = move_back.map(|m| (m, commits, name.clone()));
        Self::new_branch_at_head(id, name.clone(), moved, cx, move |_, cx| {
            Self::publish_and_open_pull_request(id, name, cx);
        });
    }

    /// `git checkout -b <name>` (the changes come along; an unborn branch is
    /// renamed), optionally the old branch moved back, then the new
    /// repository info applied (so the next step sees the new branch) and
    /// `then`. Errors stop here.
    fn new_branch_at_head(
        id: u64,
        name: String,
        move_back: Option<(MoveBack, Vec<String>, String)>,
        cx: &mut dyn Host,
        then: impl FnOnce(u64, &mut dyn Host) + 'static,
    ) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        Self::state(cx).update(cx, |s, cx| {
            s.repo_state_mut(id).checkout_target = Some(name.clone());
            cx.notify();
        });
        crate::remote::spawn_bg(
            cx,
            move || {
                corvene_git::checkout_new_branch(git.clone(), &workdir, &name)?;
                // the branch exists and is checked out: a refused move
                // keeps it and stops the chain
                let moved = match &move_back {
                    Some((m, commits, new_branch)) => corvene_git::move_branch_back(
                        git.clone(),
                        &workdir,
                        &m.branch,
                        &m.target,
                        &m.expected_tip,
                        commits,
                        &format!("Corvene: commits moved to {new_branch}"),
                    )
                    .map_err(|err| (m.branch.clone(), err)),
                    None => Ok(()),
                };
                let info = corvene_git::open_repository(&workdir)?;
                Ok::<_, corvene_git::GitError>((info, moved))
            },
            move |result, cx| {
                Self::state(cx).update(cx, |s, cx| {
                    s.repo_state_mut(id).checkout_target = None;
                    cx.notify();
                });
                match result {
                    Ok((info, moved)) => {
                        Self::state(cx).update(cx, |s, cx| {
                            s.repo_state_mut(id).info = Some(info);
                            cx.notify();
                        });
                        Self::show_section(id, corvene_models::Section::Changes, cx);
                        Self::refresh_repository(id, cx);
                        match moved {
                            Ok(()) => then(id, cx),
                            // the chain stops here, on the new branch
                            Err((branch, err)) => Self::show_error(
                                format!("Could not remove the commits from {branch}"),
                                &err,
                                cx,
                            ),
                        }
                    }
                    Err(err) => {
                        Self::show_error("Could not create branch", &err, cx);
                        Self::refresh_repository(id, cx);
                    }
                }
            },
        );
    }

    /// Publish (or push) `branch`, then open its Create Pull Request page
    /// when the repository is on GitHub.
    pub(crate) fn publish_and_open_pull_request(id: u64, branch: String, cx: &mut dyn Host) {
        Self::push_then(
            id,
            false,
            Some(branch),
            move |outcome, cx| {
                let github = Self::state(cx)
                    .read(cx)
                    .repository(id)
                    .is_some_and(|r| r.github.is_some());
                if outcome == crate::remote::PushOutcome::Pushed && github {
                    let base = Self::pull_request_base_from_origin(id, cx);
                    Self::open_create_pull_request_in_browser(id, base, cx);
                }
            },
            cx,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use corvene_models::{Branch, BranchKind, Commit, CommitIdentity, RepositoryInfo, Tip};

    fn commit(sha: &str, parents: &[&str]) -> Commit {
        let who = CommitIdentity {
            name: "T".into(),
            email: "t@example.com".into(),
            seconds: 0,
            offset: 0,
        };
        Commit {
            sha: sha.into(),
            summary: sha.into(),
            body: String::new(),
            author: who.clone(),
            committer: who,
            parents: parents.iter().map(|p| p.to_string()).collect(),
            trailers: Vec::new(),
            tags: Vec::new(),
            signature: None,
        }
    }

    fn state(upstream: Option<&str>, ahead: u32) -> RepositoryState {
        let branch = Branch {
            name: "main".into(),
            kind: BranchKind::Local,
            full_name: "refs/heads/main".into(),
            tip: Some("c".into()),
            upstream: upstream.map(String::from),
            tip_time: None,
            tip_author: None,
            remote_name: None,
        };
        RepositoryState {
            info: Some(RepositoryInfo {
                workdir: std::path::PathBuf::new(),
                tip: Tip::Valid { branch },
                branches: Vec::new(),
                remotes: Vec::new(),
                identity: Default::default(),
                ahead_behind: None,
                commit_template: None,
                diff_order: Vec::new(),
            }),
            commits: vec![commit("c", &["b"]), commit("b", &["a"]), commit("a", &[])],
            ahead_behind: Some(corvene_models::AheadBehind { ahead, behind: 0 }),
            ..Default::default()
        }
    }

    #[test]
    fn plans_only_the_newest_commits_and_moves_back_unpushed_ones() {
        let rs = state(Some("refs/remotes/origin/main"), 2);
        let sel = |shas: &[&str]| shas.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(from_commits_plan(&rs, &sel(&["b"])).is_none());
        let plan = from_commits_plan(&rs, &sel(&["b", "c"])).unwrap();
        assert_eq!(plan.commits, sel(&["c", "b"]));
        assert_eq!(
            plan.move_back,
            Some(MoveBack {
                branch: "main".into(),
                target: "a".into(),
                expected_tip: "c".into(),
            })
        );
        // pushed: a branch, no move
        let pushed = state(Some("refs/remotes/origin/main"), 1);
        assert_eq!(
            from_commits_plan(&pushed, &sel(&["c", "b"]))
                .unwrap()
                .move_back,
            None
        );
        // the root commit has nothing to go back to
        let all = from_commits_plan(&state(None, 0), &sel(&["a", "b", "c"])).unwrap();
        assert_eq!(all.move_back, None);
    }
}
