//! Corvene `1218-compare-refs`: compare any two refs. GitHub Desktop only
//! compares the current branch to another branch (`compare.rs`), and shows
//! the files that changed only in Preview Pull Request.
//!
//! Branch › Compare… (and Compare with… in the History commit menu and the
//! branch list menu) opens `Popup::CompareRefs`, whose two pickers take a
//! branch, a tag or a commit. History then lists the commits of the range
//! ([`crate::compare::CompareForm::Refs`]): `base..head` (the commits only
//! `head` has) or `base...head` (the commits only one of them has, each
//! marked with its side). Its "Changed files" row shows the combined diff
//! in the commit view's place, with the file list of the `1203` Compare
//! Branches dialog (`crate::pull_request_preview`, [`PreviewSlot::RefCompare`]):
//! as `git diff` reads the same range, straight from `base` to `head` for
//! `..` and from their merge base for `...`.

use std::collections::HashSet;

use corvene_models::{Commit, Section};
use tracing::warn;

use crate::compare::CompareForm;
use crate::dispatcher::Dispatcher;
use crate::host::Host;
use crate::pull_request_preview::{PreviewSlot, PullRequestPreview};
use crate::remote::spawn_bg;
use crate::state::{AppState, Popup, RepositoryState};

/// How the two refs make a range, as `git log` and `git diff` read it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RefRange {
    /// `base..head`: the commits `head` has and `base` has not; the diff
    /// from `base` to `head`.
    #[default]
    Range,
    /// `base...head`: the commits only one of them has; the diff from
    /// their merge base to `head`.
    Symmetric,
}

impl RefRange {
    pub fn separator(self) -> &'static str {
        match self {
            Self::Range => "..",
            Self::Symmetric => "...",
        }
    }
}

/// At most this many commits of a range are listed.
pub const MAX_COMPARE_COMMITS: usize = 10_000;

/// The flag is on.
pub fn enabled(s: &AppState) -> bool {
    s.flags.bool(crate::flags::ids::COMPARE_REFS)
}

/// `base`'s and `head`'s commits merged newest first (by committer date,
/// as History lists them); the shas only `base` has.
fn merge_sides(head_side: Vec<Commit>, base_side: Vec<Commit>) -> (Vec<Commit>, HashSet<String>) {
    let base_only: HashSet<String> = base_side.iter().map(|c| c.sha.clone()).collect();
    let mut commits = head_side;
    commits.extend(base_side);
    // stable: each side keeps its own order between equal dates
    commits.sort_by(|a, b| b.committer.seconds.cmp(&a.committer.seconds));
    (commits, base_only)
}

/// What the background load hands back.
struct Loaded {
    commits: Vec<Commit>,
    base_only_shas: HashSet<String>,
    base_only: u32,
    head_only: u32,
}

impl Dispatcher {
    /// Branch › Compare… and the Compare with… menu items: the dialog with
    /// its pickers filled in.
    pub fn show_compare_refs(
        id: u64,
        base: Option<String>,
        head: Option<String>,
        cx: &mut dyn Host,
    ) {
        if !enabled(Self::state(cx).read(cx)) {
            return;
        }
        Self::close_foldout(cx);
        Self::show_popup(
            Popup::CompareRefs {
                repo: id,
                base,
                head,
            },
            cx,
        );
    }

    /// Compare `base` with `head`: History lists the range's commits and
    /// the combined diff loads. `refresh` keeps the selected commit (when
    /// still listed) and file.
    pub fn compare_refs(
        id: u64,
        base: String,
        head: String,
        range: RefRange,
        refresh: bool,
        cx: &mut dyn Host,
    ) {
        if !enabled(Self::state(cx).read(cx)) {
            return;
        }
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        if !refresh {
            Self::show_section(id, Section::History, cx);
            // they replace History's list too
            Self::close_recent_activity(id, cx);
            Self::close_issues(id, cx);
            Self::close_releases(id, cx);
        }
        Self::state(cx).update(cx, |s, cx| {
            let rs = s.repo_state_mut(id);
            rs.compare.loading = true;
            rs.compare.show_branch_list = false;
            cx.notify();
        });
        let (base_for_load, head_for_load) = (base.clone(), head.clone());
        spawn_bg(
            cx,
            move || -> Result<Loaded, String> {
                for name in [&base_for_load, &head_for_load] {
                    if !corvene_git::resolve_commit(&workdir, name)
                        .map_err(|err| err.to_string())?
                        .is_some()
                    {
                        return Err(format!("{name} is not a branch, tag or commit."));
                    }
                }
                let counts = corvene_git::symmetric_ahead_behind(
                    git,
                    &workdir,
                    &base_for_load,
                    &head_for_load,
                )
                .map_err(|err| err.to_string())?
                .unwrap_or_default();
                let limit = |n: u32| (n as usize).min(MAX_COMPARE_COMMITS);
                let head_side = corvene_git::get_commits_in_range(
                    &workdir,
                    &base_for_load,
                    &head_for_load,
                    limit(counts.behind),
                )
                .map_err(|err| err.to_string())?;
                let (commits, base_only_shas) = match range {
                    RefRange::Range => (head_side, HashSet::new()),
                    RefRange::Symmetric => {
                        let base_side = corvene_git::get_commits_in_range(
                            &workdir,
                            &head_for_load,
                            &base_for_load,
                            limit(counts.ahead),
                        )
                        .map_err(|err| err.to_string())?;
                        merge_sides(head_side, base_side)
                    }
                };
                Ok(Loaded {
                    commits,
                    base_only_shas,
                    base_only: counts.ahead,
                    head_only: counts.behind,
                })
            },
            move |result, cx| {
                let loaded = match result {
                    Ok(loaded) => loaded,
                    Err(message) => {
                        warn!(id, %message, "compare refs failed");
                        Self::state(cx).update(cx, |s, cx| {
                            s.repo_state_mut(id).compare.loading = false;
                            cx.notify();
                        });
                        if !refresh {
                            Self::show_error("Could not compare", message, cx);
                        } else {
                            // a ref went away: back to the history
                            Self::exit_compare(id, cx);
                        }
                        return;
                    }
                };
                let (keep_selection, file) = Self::state(cx).update(cx, |s, cx| {
                    let rs = s.repo_state_mut(id);
                    rs.compare.loading = false;
                    rs.compare.form = CompareForm::Refs {
                        base: base.clone(),
                        head: head.clone(),
                        range,
                        base_only: loaded.base_only,
                        head_only: loaded.head_only,
                    };
                    rs.compare.filter_text = format!("{base}{}{head}", range.separator());
                    rs.compare.commits = loaded.commits;
                    rs.compare.base_only_shas = loaded.base_only_shas;
                    rs.compare.merge_status = None;
                    rs.compare.conflicted_files.clear();
                    let keep = refresh
                        && rs
                            .selected_commits
                            .iter()
                            .all(|sha| rs.compare.commits.iter().any(|c| &c.sha == sha));
                    let file = rs
                        .ref_compare_changes
                        .as_ref()
                        .filter(|_| refresh)
                        .and_then(|p| p.file.clone());
                    cx.notify();
                    (keep, file)
                });
                if !keep_selection {
                    // the combined diff shows first
                    Self::select_commits(id, Vec::new(), cx);
                }
                Self::load_range_preview(
                    id,
                    PreviewSlot::RefCompare,
                    PullRequestPreview {
                        base_branch: Some(base),
                        current_branch: head,
                        compare_only: true,
                        direct: range == RefRange::Range,
                        file,
                        ..PullRequestPreview::default()
                    },
                    cx,
                );
            },
        );
    }

    /// The `..` / `...` toggle.
    pub fn set_ref_compare_range(id: u64, range: RefRange, cx: &mut dyn Host) {
        if let Some((base, head, current)) = Self::compared_refs(id, cx)
            && current != range
        {
            Self::compare_refs(id, base, head, range, false, cx);
        }
    }

    /// The swap button: `head` becomes the base.
    pub fn swap_compare_refs(id: u64, cx: &mut dyn Host) {
        if let Some((base, head, range)) = Self::compared_refs(id, cx) {
            Self::compare_refs(id, head, base, range, false, cx);
        }
    }

    /// The "Changed files" row: the combined diff in the commit view's place.
    pub fn show_ref_compare_changes(id: u64, cx: &mut dyn Host) {
        Self::select_commits(id, Vec::new(), cx);
    }

    fn compared_refs(id: u64, cx: &dyn Host) -> Option<(String, String, RefRange)> {
        let s = Self::state(cx).read(cx);
        let (base, head, range) = s.repo_states.get(&id)?.compare.refs()?;
        Some((base.to_string(), head.to_string(), range))
    }
}

/// The combined diff shows: two refs are compared and no commit is selected.
pub fn showing_changes(rs: &RepositoryState) -> bool {
    rs.compare.refs().is_some() && rs.selected_commits.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use corvene_models::CommitIdentity;

    fn commit(sha: &str, seconds: i64) -> Commit {
        let who = CommitIdentity {
            name: "A".into(),
            email: "a@example.com".into(),
            seconds,
            offset: 0,
        };
        Commit {
            sha: sha.into(),
            summary: String::new(),
            body: String::new(),
            author: who.clone(),
            committer: who,
            parents: Vec::new(),
            trailers: Vec::new(),
            tags: Vec::new(),
            signature: None,
        }
    }

    #[::core::prelude::v1::test]
    fn sides_merge_newest_first() {
        let (commits, base_only) = merge_sides(
            vec![commit("h2", 30), commit("h1", 10)],
            vec![commit("b2", 40), commit("b1", 20)],
        );
        let order: Vec<&str> = commits.iter().map(|c| c.sha.as_str()).collect();
        assert_eq!(order, ["b2", "h2", "b1", "h1"]);
        assert!(base_only.contains("b1") && base_only.contains("b2"));
        assert!(!base_only.contains("h1"));
    }

    #[::core::prelude::v1::test]
    fn separators_read_like_git() {
        assert_eq!(RefRange::Range.separator(), "..");
        assert_eq!(RefRange::Symmetric.separator(), "...");
    }
}
