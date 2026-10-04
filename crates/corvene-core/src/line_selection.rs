//! Corvene `790-selection-follows-lines`: a file's partial line selection
//! follows its lines when the file's diff changes.
//!
//! A [`DiffSelection`] names lines by their position in the diff. GHD
//! (`updateChangesWorkingDirectoryDiff`, `lib/stores/updates/changes-state.ts`)
//! keeps those positions when the diff is read again and only drops the
//! ones that no longer exist (`withSelectableLines`), so after an edit above
//! the selected lines (#4720) or after discarding some lines (#17614) the
//! ticks land on other rows. Here the changed lines of the diff the
//! selection was made on are matched with the new diff's by kind and text
//! (a line diff over the changed lines): matched lines keep their state,
//! new or unmatched ones get the file's default.
//!
//! Corvene `1208-undo-restores-line-selection` ([`selection_from_commit`]):
//! after Undo, the lines the undone commit made are the ones selected.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use corvene_models::{DiffHunk, DiffLineKind, DiffSelection};

/// A line diff of huge change sets stops trying for a minimal match after
/// this long (the match it has is still valid).
const MATCH_BUDGET: Duration = Duration::from_millis(50);

/// The added and deleted lines of `hunks`: absolute index, whether it is
/// an addition, text.
fn changed_lines(hunks: &[DiffHunk]) -> Vec<(u32, bool, &str)> {
    hunks
        .iter()
        .flat_map(|h| {
            h.lines
                .iter()
                .enumerate()
                .filter_map(move |(i, l)| match l.kind {
                    DiffLineKind::Add | DiffLineKind::Delete => Some((
                        h.unified_diff_start + i as u32,
                        l.kind == DiffLineKind::Add,
                        l.text.as_str(),
                    )),
                    _ => None,
                })
        })
        .collect()
}

/// `selection`, made on the diff `old`, moved to the diff `new`: each
/// changed line of `new` that matches one of `old` (same kind and text, in
/// order) keeps that line's state; the others take the selection's default.
pub fn carry_over(selection: &DiffSelection, old: &[DiffHunk], new: &[DiffHunk]) -> DiffSelection {
    let old_lines = changed_lines(old);
    let new_lines = changed_lines(new);
    let old_keys: Vec<(bool, &str)> = old_lines.iter().map(|l| (l.1, l.2)).collect();
    let new_keys: Vec<(bool, &str)> = new_lines.iter().map(|l| (l.1, l.2)).collect();
    let default_all = selection.defaults_to_selected();
    let mut diverging = BTreeSet::new();
    let ops = similar::capture_diff_slices_deadline(
        similar::Algorithm::Myers,
        &old_keys,
        &new_keys,
        Instant::now().checked_add(MATCH_BUDGET),
    );
    for op in ops {
        if let similar::DiffOp::Equal {
            old_index,
            new_index,
            len,
        } = op
        {
            for k in 0..len {
                if selection.is_selected(old_lines[old_index + k].0) != default_all {
                    diverging.insert(new_lines[new_index + k].0);
                }
            }
        }
    }
    let selectable = new_lines.iter().map(|l| l.0).collect();
    DiffSelection::from_parts(default_all, diverging, selectable)
}

/// Corvene `1208-undo-restores-line-selection`: the selection of a
/// working-directory diff's lines that an undone commit had made: its
/// deletions by the parent's line number, its additions by the working
/// copy's.
pub fn selection_from_commit(hunks: &[DiffHunk], made: &corvene_git::CommitLines) -> DiffSelection {
    let mut selectable = BTreeSet::new();
    let mut unselected = BTreeSet::new();
    for h in hunks {
        for (i, l) in h.lines.iter().enumerate() {
            let made_by_commit = match l.kind {
                DiffLineKind::Add => l.new_line.is_some_and(|n| made.added.contains(&n)),
                DiffLineKind::Delete => l.old_line.is_some_and(|n| made.deleted.contains(&n)),
                _ => continue,
            };
            let ix = h.unified_diff_start + i as u32;
            selectable.insert(ix);
            if !made_by_commit {
                unselected.insert(ix);
            }
        }
    }
    if unselected.is_empty() {
        DiffSelection::all().with_selectable_lines(selectable)
    } else {
        DiffSelection::from_parts(true, unselected, selectable)
    }
}

#[cfg(test)]
mod tests {
    use corvene_models::{Diff, DiffSelectionType};

    use super::*;

    fn hunks(patch: &str) -> Vec<DiffHunk> {
        match corvene_git::parse_unified(patch) {
            Diff::Text { hunks, .. } => hunks,
            other => panic!("text diff expected, got {other:?}"),
        }
    }

    fn selected(selection: &DiffSelection, hunks: &[DiffHunk]) -> Vec<String> {
        changed_lines(hunks)
            .into_iter()
            .filter(|(ix, _, _)| selection.is_selected(*ix))
            .map(|(_, add, text)| format!("{}{text}", if add { '+' } else { '-' }))
            .collect()
    }

    #[test]
    fn selection_follows_lines_after_an_edit_above() {
        let before = hunks("@@ -1,3 +1,3 @@\n a\n-b\n+B\n c\n@@ -10,2 +10,3 @@\n x\n+y\n z\n");
        // lines: 0 hunk, 1 a, 2 -b, 3 +B, 4 c | 5 hunk, 6 x, 7 +y, 8 z
        let selection = DiffSelection::all()
            .with_selectable_lines(BTreeSet::from([2, 3, 7]))
            .with_line(7, false);
        assert_eq!(selected(&selection, &before), ["-b", "+B"]);
        // a new change above: every line moves down by four rows
        let after = hunks(
            "@@ -1,1 +1,2 @@\n top\n+new\n@@ -1,3 +2,3 @@\n a\n-b\n+B\n c\n@@ -10,2 +11,3 @@\n x\n+y\n z\n",
        );
        // GHD: the old positions, now other lines
        let ghd =
            selection.with_selectable_lines(changed_lines(&after).iter().map(|l| l.0).collect());
        assert_ne!(selected(&ghd, &after), ["+new", "-b", "+B"]);
        let carried = carry_over(&selection, &before, &after);
        assert_eq!(selected(&carried, &after), ["+new", "-b", "+B"]);
        assert_eq!(carried.kind(), DiffSelectionType::Partial);
    }

    #[test]
    fn selection_follows_lines_after_a_discard() {
        let before = hunks("@@ -1,4 +1,4 @@\n a\n-b\n-c\n+B\n+C\n d\n");
        // lines: 0 hunk, 1 a, 2 -b, 3 -c, 4 +B, 5 +C, 6 d; only +C excluded
        let selection = DiffSelection::all()
            .with_selectable_lines(BTreeSet::from([2, 3, 4, 5]))
            .with_line(5, false);
        // -b and +B discarded
        let after = hunks("@@ -1,3 +1,3 @@\n a\n B\n-c\n+C\n d\n");
        let carried = carry_over(&selection, &before, &after);
        assert_eq!(selected(&carried, &after), ["-c"]);
    }

    #[test]
    fn selection_from_commit_picks_the_committed_lines() {
        // parent: a b c d; commit: a B c d (b → B); working copy: a B c D x
        let working = hunks("@@ -1,4 +1,5 @@\n a\n-b\n+B\n c\n-d\n+D\n+x\n");
        let made = corvene_git::CommitLines {
            deleted: BTreeSet::from([2]),
            added: BTreeSet::from([2]),
        };
        let selection = selection_from_commit(&working, &made);
        assert_eq!(selected(&selection, &working), ["-b", "+B"]);
        assert_eq!(selection.kind(), DiffSelectionType::Partial);
        let everything = corvene_git::CommitLines {
            deleted: BTreeSet::from([2, 4]),
            added: BTreeSet::from([2, 4, 5]),
        };
        assert_eq!(
            selection_from_commit(&working, &everything).kind(),
            DiffSelectionType::All
        );
    }

    #[test]
    fn unmatched_lines_take_the_default() {
        let before = hunks("@@ -1,2 +1,2 @@\n a\n-b\n+B\n");
        let selection = DiffSelection::none()
            .with_selectable_lines(BTreeSet::from([2, 3]))
            .with_line(3, true);
        let after = hunks("@@ -1,2 +1,3 @@\n a\n-b\n+B\n+more\n");
        let carried = carry_over(&selection, &before, &after);
        assert_eq!(selected(&carried, &after), ["+B"]);
    }
}
