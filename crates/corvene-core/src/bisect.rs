//! Corvene `1212-bisect`: a guided `git bisect` (GitHub Desktop has none).
//!
//! The bisect itself lives in git: the status refresh reads it
//! (`corvene_git::bisect_state`, with the commits still in question) into
//! [`corvene_models::WorkingDirectoryStatus::bisect`], so a bisect started
//! on the command line, or left running when Corvene quit, shows the same
//! way. Marks, start and stop run `git bisect`
//! ([`Dispatcher::request_start_bisect`], [`Dispatcher::bisect_mark`],
//! [`Dispatcher::stop_bisect`]). Starting with uncommitted changes asks to
//! stash them first (`Popup::StartBisect`), as a branch switch does.
//!
//! While a bad and a good commit are known, History lists the commits from
//! the bad one down ([`history_tip`]) rather than from the detached HEAD
//! git leaves at the commit under test, so the whole range stays in view;
//! the first bad commit is selected once git names it.

use corvene_git::BisectVerdict;
use corvene_models::{BisectPhase, BisectState, Section, Tip};

use crate::dispatcher::Dispatcher;
use crate::host::Host;
use crate::state::{AppState, Popup, RepositoryState};

/// How a History row is marked while bisecting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BisectMark {
    Bad,
    Good,
    Skipped,
    /// HEAD, the commit being tested.
    Testing,
    /// The bisect's answer.
    FirstBad,
}

/// The selected bisect, `None` while the flag is off.
pub fn bisect_of(s: &AppState, rs: &RepositoryState) -> Option<BisectState> {
    if !s.flags.bool(crate::flags::ids::BISECT) {
        return None;
    }
    rs.status.as_ref()?.bisect.clone()
}

/// [`bisect_of`] for the selected repository.
pub fn selected_bisect(s: &AppState) -> Option<BisectState> {
    bisect_of(s, s.selected_state()?)
}

/// HEAD's commit while it is detached (the commit under test).
pub fn detached_head(rs: &RepositoryState) -> Option<&str> {
    match &rs.info.as_ref()?.tip {
        Tip::Detached { sha } => Some(sha),
        _ => None,
    }
}

/// The steps left, counting the commit being tested: git's "roughly N
/// steps" after this one, plus this one.
pub fn steps_left(state: &BisectState) -> u32 {
    corvene_git::estimate_steps(state.candidates.len()) + 1
}

/// The mark of the row for `sha`.
pub fn row_mark(state: &BisectState, head: Option<&str>, sha: &str) -> Option<BisectMark> {
    match state.phase() {
        BisectPhase::Found { sha: first } if first == sha => return Some(BisectMark::FirstBad),
        BisectPhase::Testing { .. } if head == Some(sha) => return Some(BisectMark::Testing),
        _ => {}
    }
    if state.bad.as_deref() == Some(sha) {
        Some(BisectMark::Bad)
    } else if state.good.iter().any(|g| g == sha) {
        Some(BisectMark::Good)
    } else if state.skipped.iter().any(|g| g == sha) {
        Some(BisectMark::Skipped)
    } else {
        None
    }
}

/// The word git uses for `verdict` in this bisect ("good", or the term the
/// bisect was started with), capitalized for a label.
pub fn term_label(state: Option<&BisectState>, verdict: BisectVerdict) -> String {
    let term = match (state, verdict) {
        (_, BisectVerdict::Skip) => "skip",
        (Some(state), BisectVerdict::Good) => state.term_good.as_str(),
        (Some(state), BisectVerdict::Bad) => state.term_bad.as_str(),
        (None, BisectVerdict::Good) => "good",
        (None, BisectVerdict::Bad) => "bad",
    };
    let mut chars = term.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// What History lists instead of HEAD while bisecting: the bad commit, once
/// a good one is known too (HEAD is then one of its ancestors).
pub fn history_tip(s: &AppState, rs: &RepositoryState) -> Option<String> {
    let state = bisect_of(s, rs)?;
    if state.good.is_empty() {
        return None;
    }
    state.bad
}

impl Dispatcher {
    /// Repository › Start Bisect (`mark` `None`: the current commit is bad)
    /// or History's Bisect ▸ items while no bisect runs: start, asking to
    /// stash uncommitted changes first.
    pub fn request_start_bisect(id: u64, mark: Option<(BisectVerdict, String)>, cx: &mut dyn Host) {
        let (branch, dirty) = {
            let s = Self::state(cx).read(cx);
            if !s.flags.bool(crate::flags::ids::BISECT) {
                return;
            }
            let Some(rs) = s.repo_states.get(&id) else {
                return;
            };
            (
                rs.info
                    .as_ref()
                    .and_then(|i| i.current_branch())
                    .map(|b| b.name.clone()),
                rs.changed_files() > 0,
            )
        };
        if !dirty {
            Self::start_bisect(id, mark, false, cx);
            return;
        }
        match branch {
            Some(branch) => Self::show_popup(
                Popup::StartBisect {
                    repo: id,
                    mark,
                    branch,
                },
                cx,
            ),
            // a stash needs a branch to belong to
            None => Self::show_error(
                "Could not start bisecting",
                "Bisecting checks out other commits. Commit or discard your changes first.",
                cx,
            ),
        }
    }

    /// `git bisect start` (stashing the changes on the current branch first
    /// when `stash`), then `mark`, or the current commit as bad.
    pub fn start_bisect(
        id: u64,
        mark: Option<(BisectVerdict, String)>,
        stash: bool,
        cx: &mut dyn Host,
    ) {
        let (branch, guard) = {
            let s = Self::state(cx).read(cx);
            (
                s.repo_states
                    .get(&id)
                    .and_then(|rs| rs.info.as_ref())
                    .and_then(|i| i.current_branch())
                    .map(|b| b.name.clone()),
                s.flags
                    .bool(crate::flags::ids::STASH_PROTECTS_ASSUME_UNCHANGED),
            )
        };
        Self::run_history_op_then(
            id,
            "Could not start bisecting",
            move |git, workdir| {
                if stash && let Some(branch) = branch.as_deref() {
                    corvene_git::create_desktop_stash(git.clone(), &workdir, branch, guard)?;
                }
                corvene_git::bisect_start(git.clone(), &workdir)?;
                match mark {
                    Some((verdict, sha)) => {
                        corvene_git::bisect_mark(git, &workdir, verdict, Some(&sha))
                    }
                    None => corvene_git::bisect_mark(git, &workdir, BisectVerdict::Bad, None),
                }
            },
            move |cx| Self::show_section(id, Section::History, cx),
            cx,
        );
    }

    /// Mark `sha` (the commit being tested when `None`); git checks out the
    /// next one. History selects the first bad commit once git names it.
    pub fn bisect_mark(id: u64, verdict: BisectVerdict, sha: Option<String>, cx: &mut dyn Host) {
        Self::run_history_op_then(
            id,
            "Could not mark the commit",
            move |git, workdir| corvene_git::bisect_mark(git, &workdir, verdict, sha.as_deref()),
            move |cx| {
                Self::state(cx).update(cx, |s, _| s.repo_state_mut(id).bisect_reveal = true);
            },
            cx,
        );
    }

    /// `git bisect reset`: back to the branch the bisect started from.
    pub fn stop_bisect(id: u64, cx: &mut dyn Host) {
        Self::run_history_op(
            id,
            "Could not stop bisecting",
            |git, workdir| corvene_git::bisect_reset(git, &workdir),
            cx,
        );
    }

    /// The bisect's answer: History shows and selects it (a History load
    /// calls this once the list is in).
    pub(crate) fn reveal_first_bad_commit(id: u64, cx: &mut dyn Host) {
        let first = Self::state(cx).update(cx, |s, _| {
            let rs = s.repo_states.get_mut(&id)?;
            if !std::mem::take(&mut rs.bisect_reveal) {
                return None;
            }
            match rs.status.as_ref()?.bisect.as_ref()?.phase() {
                BisectPhase::Found { sha } if rs.commits.iter().any(|c| c.sha == sha) => Some(sha),
                _ => None,
            }
        });
        if let Some(sha) = first {
            Self::show_section(id, Section::History, cx);
            Self::select_commit(id, sha, cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> BisectState {
        BisectState {
            start: "main".into(),
            term_bad: "bad".into(),
            term_good: "good".into(),
            bad: Some("d".into()),
            good: vec!["a".into()],
            skipped: vec!["c".into()],
            candidates: vec!["d".into(), "c".into(), "b".into()],
        }
    }

    #[test]
    fn marks_rows() {
        let s = state();
        assert_eq!(s.phase(), BisectPhase::Testing { left: 3 });
        assert_eq!(row_mark(&s, Some("b"), "b"), Some(BisectMark::Testing));
        assert_eq!(row_mark(&s, Some("b"), "d"), Some(BisectMark::Bad));
        assert_eq!(row_mark(&s, Some("b"), "a"), Some(BisectMark::Good));
        assert_eq!(row_mark(&s, Some("b"), "c"), Some(BisectMark::Skipped));
        assert_eq!(row_mark(&s, Some("b"), "e"), None);
        let found = BisectState {
            candidates: vec!["d".into()],
            ..state()
        };
        assert_eq!(found.phase(), BisectPhase::Found { sha: "d".into() });
        assert_eq!(row_mark(&found, Some("b"), "d"), Some(BisectMark::FirstBad));
        let skipped = BisectState {
            candidates: vec!["d".into(), "c".into()],
            ..state()
        };
        assert_eq!(skipped.phase(), BisectPhase::OnlySkipped { count: 2 });
        let waiting = BisectState {
            good: Vec::new(),
            candidates: Vec::new(),
            ..state()
        };
        assert_eq!(
            waiting.phase(),
            BisectPhase::Waiting {
                has_bad: true,
                has_good: false
            }
        );
    }

    #[test]
    fn labels_follow_the_terms() {
        let custom = BisectState {
            term_bad: "fixed".into(),
            term_good: "broken".into(),
            ..state()
        };
        assert_eq!(term_label(Some(&custom), BisectVerdict::Bad), "Fixed");
        assert_eq!(term_label(Some(&custom), BisectVerdict::Good), "Broken");
        assert_eq!(term_label(None, BisectVerdict::Good), "Good");
        assert_eq!(term_label(None, BisectVerdict::Skip), "Skip");
    }

    #[test]
    fn steps_count_the_current_test() {
        let s = BisectState {
            candidates: (0..99).map(|i| i.to_string()).collect(),
            ..state()
        };
        assert_eq!(steps_left(&s), 7);
    }
}
