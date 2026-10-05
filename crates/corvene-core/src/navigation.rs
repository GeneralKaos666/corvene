//! Corvene (`427-back-forward-navigation`): View › Back / Forward step
//! through the repositories and Changes / History sections shown before,
//! like a browser's history. GitHub Desktop has no navigation history.
//!
//! A step is recorded when another repository is selected or the selected
//! repository's section changes (`Dispatcher::select_repository`,
//! `Dispatcher::show_section`), except while Back / Forward itself moves.
//! Entries of repositories removed since are skipped.

use std::collections::VecDeque;

use corvene_models::Section;

/// How many steps each direction keeps.
pub const NAVIGATION_HISTORY_LENGTH: usize = 50;

/// A place Back / Forward returns to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NavigationEntry {
    pub repository: u64,
    pub section: Section,
}

#[derive(Clone, Debug, Default)]
pub struct NavigationHistory {
    back: VecDeque<NavigationEntry>,
    forward: Vec<NavigationEntry>,
    /// Back / Forward is moving: the selection changes it makes are not
    /// new steps.
    pub replaying: bool,
}

impl NavigationHistory {
    /// The user left `from`: it becomes the Back step and the Forward
    /// steps are dropped.
    pub fn record(&mut self, from: NavigationEntry) {
        if self.replaying {
            return;
        }
        if self.back.back() != Some(&from) {
            self.back.push_back(from);
            if self.back.len() > NAVIGATION_HISTORY_LENGTH {
                self.back.pop_front();
            }
        }
        self.forward.clear();
    }

    /// The step Back (`back`) or Forward goes to from `current`, skipping
    /// entries `exists` rejects and entries equal to `current`; `current`
    /// becomes a step the other way.
    pub fn step(
        &mut self,
        back: bool,
        current: Option<NavigationEntry>,
        exists: impl Fn(u64) -> bool,
    ) -> Option<NavigationEntry> {
        let target = loop {
            let entry = if back {
                self.back.pop_back()
            } else {
                self.forward.pop()
            }?;
            if exists(entry.repository) && Some(entry) != current {
                break entry;
            }
        };
        if let Some(current) = current {
            if back {
                self.forward.push(current);
            } else {
                self.back.push_back(current);
            }
        }
        Some(target)
    }

    /// Whether Back / Forward has a step (removed repositories included:
    /// the menu item is enabled until a step finds none).
    pub fn can_go(&self, back: bool) -> bool {
        if back {
            !self.back.is_empty()
        } else {
            !self.forward.is_empty()
        }
    }

    /// A repository was removed: its steps go.
    pub fn forget(&mut self, repository: u64) {
        self.back.retain(|e| e.repository != repository);
        self.forward.retain(|e| e.repository != repository);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(repository: u64, section: Section) -> NavigationEntry {
        NavigationEntry {
            repository,
            section,
        }
    }

    #[test]
    fn back_and_forward_retrace_the_steps() {
        let mut nav = NavigationHistory::default();
        nav.record(at(1, Section::Changes));
        nav.record(at(1, Section::History));
        let current = at(2, Section::Changes);
        let back = nav.step(true, Some(current), |_| true);
        assert_eq!(back, Some(at(1, Section::History)));
        assert!(nav.can_go(false));
        let back = nav.step(true, back, |_| true);
        assert_eq!(back, Some(at(1, Section::Changes)));
        assert!(!nav.can_go(true));
        let forward = nav.step(false, back, |_| true);
        assert_eq!(forward, Some(at(1, Section::History)));
        assert_eq!(nav.step(false, forward, |_| true), Some(current));
        assert!(!nav.can_go(false));
    }

    #[test]
    fn a_new_step_drops_forward_and_replays_are_not_steps() {
        let mut nav = NavigationHistory::default();
        nav.record(at(1, Section::Changes));
        nav.step(true, Some(at(2, Section::Changes)), |_| true);
        assert!(nav.can_go(false));
        nav.replaying = true;
        nav.record(at(3, Section::Changes));
        assert!(nav.can_go(false));
        nav.replaying = false;
        nav.record(at(1, Section::Changes));
        assert!(!nav.can_go(false));
    }

    #[test]
    fn removed_repositories_are_skipped() {
        let mut nav = NavigationHistory::default();
        nav.record(at(1, Section::Changes));
        nav.record(at(2, Section::Changes));
        nav.record(at(3, Section::Changes));
        nav.forget(3);
        let back = nav.step(true, Some(at(4, Section::Changes)), |id| id != 2);
        assert_eq!(back, Some(at(1, Section::Changes)));
        assert_eq!(nav.step(true, back, |_| true), None);
    }

    #[test]
    fn keeps_a_bounded_history() {
        let mut nav = NavigationHistory::default();
        for id in 0..(NAVIGATION_HISTORY_LENGTH as u64 + 10) {
            nav.record(at(id, Section::Changes));
        }
        let mut steps = 0;
        let mut current = Some(at(1000, Section::Changes));
        while let Some(entry) = nav.step(true, current, |_| true) {
            current = Some(entry);
            steps += 1;
        }
        assert_eq!(steps, NAVIGATION_HISTORY_LENGTH);
    }
}
