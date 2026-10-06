//! Where a pull request review thread sits in the diff Corvene shows
//! (`348-pull-request-review`, `349-review-thread-remapping`).
//!
//! GitHub keeps a thread's `line` relative to the pull request's current
//! head (`RIGHT`) or the base side of its three-dot diff (`LEFT`), and
//! drops it (`isOutdated`) once a push changed those lines. Corvene may
//! show another head: the local branch tip with commits not pushed yet, or
//! a tip behind a force-push. [`LineMap`] carries a line number from one
//! version of a file to another over the lines both versions share (an
//! LCS diff of the two blobs, `similar`); [`anchor_threads`] applies the
//! maps to every thread of a file and says, for each, which row of the
//! diff it belongs under, or that it has no place in it (outdated). An
//! outdated thread whose original lines survived unchanged in the shown
//! head is anchored again from its `originalLine` (flag 349), which
//! github.com does not do.

use std::collections::HashMap;

use corvene_github::review::{DiffSide, ReviewThread};
use serde::{Deserialize, Serialize};

/// Lines of one version of a file to the same lines in another: a line
/// changed or deleted in between has no image.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineMap {
    /// `None`: the identity (the two versions are the same blob).
    map: Option<HashMap<u32, u32>>,
}

impl LineMap {
    /// Every line maps to itself.
    pub fn identity() -> Self {
        Self { map: None }
    }

    /// Nothing maps: the file does not exist on one side.
    pub fn empty() -> Self {
        Self {
            map: Some(HashMap::new()),
        }
    }

    /// The map from `from`'s lines to `to`'s (both 1-based).
    pub fn between(from: &[String], to: &[String]) -> Self {
        if from == to {
            return Self::identity();
        }
        let mut map = HashMap::new();
        for op in similar::capture_diff_slices(similar::Algorithm::Myers, from, to) {
            let (tag, old, new) = op.as_tag_tuple();
            if tag == similar::DiffTag::Equal {
                for (o, n) in old.zip(new) {
                    map.insert(o as u32 + 1, n as u32 + 1);
                }
            }
        }
        Self { map: Some(map) }
    }

    pub fn is_identity(&self) -> bool {
        self.map.is_none()
    }

    /// `line` of the source version in the target one.
    pub fn map(&self, line: u32) -> Option<u32> {
        match &self.map {
            None => Some(line),
            Some(map) => map.get(&line).copied(),
        }
    }
}

/// The maps a file's threads are carried through to the shown diff.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileLineMaps {
    /// GitHub's head (`RIGHT` lines) to the shown head.
    pub right: LineMap,
    /// GitHub's base side (`LEFT` lines) to the shown diff's old side.
    pub left: LineMap,
    /// Each thread's original commit to the shown head, for outdated
    /// threads (flag 349); a commit missing here could not be read.
    pub originals: HashMap<String, LineMap>,
}

impl FileLineMaps {
    /// Both heads are the same commit: GitHub's lines are the diff's.
    pub fn identity() -> Self {
        Self {
            right: LineMap::identity(),
            left: LineMap::identity(),
            originals: HashMap::new(),
        }
    }
}

/// Where one thread goes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThreadAnchor {
    /// Under the row whose line number (new side for `Right`, old side
    /// for `Left`) is `line`; `start` is the first line of a range.
    Line {
        side: DiffSide,
        line: u32,
        start: Option<u32>,
        /// GitHub had dropped the position; Corvene found it again from
        /// the original lines.
        remapped: bool,
    },
    /// On the file as a whole.
    File,
    /// Nowhere in the shown diff.
    Outdated,
}

/// A thread of the file (by its index in the pull request's thread list)
/// and its place.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnchoredThread {
    pub thread: usize,
    pub anchor: ThreadAnchor,
}

/// Anchor every thread of one file. `threads` pairs each thread with its
/// index in the full list; `maps` is `None` when the local remapping is
/// off (flag 349), which anchors only what GitHub positioned, and only when
/// the shown head is GitHub's (`same_head`).
pub fn anchor_threads(
    threads: &[(usize, &ReviewThread)],
    maps: Option<&FileLineMaps>,
    same_head: bool,
) -> Vec<AnchoredThread> {
    threads
        .iter()
        .map(|(index, thread)| AnchoredThread {
            thread: *index,
            anchor: anchor_thread(thread, maps, same_head),
        })
        .collect()
}

fn anchor_thread(
    thread: &ReviewThread,
    maps: Option<&FileLineMaps>,
    same_head: bool,
) -> ThreadAnchor {
    if thread.is_file_level() {
        return ThreadAnchor::File;
    }
    let side = thread.diff_side.unwrap_or(DiffSide::Right);
    if let Some(line) = thread.line {
        let map = match (maps, side) {
            (Some(maps), DiffSide::Right) => Some(&maps.right),
            (Some(maps), DiffSide::Left) => Some(&maps.left),
            (None, _) => None,
        };
        let mapped = match map {
            Some(map) => map.map(line),
            None if same_head => Some(line),
            None => None,
        };
        if let Some(mapped) = mapped {
            let start = thread
                .start_line
                .filter(|s| *s < line && thread.start_diff_side.unwrap_or(side) == side)
                .and_then(|s| match map {
                    Some(map) => map.map(s),
                    None => Some(s),
                })
                .filter(|s| *s < mapped);
            return ThreadAnchor::Line {
                side,
                line: mapped,
                start,
                remapped: false,
            };
        }
        return ThreadAnchor::Outdated;
    }
    // outdated on GitHub: back to the lines of the commit it was made on
    if side == DiffSide::Right
        && let (Some(maps), Some(original), Some(commit)) =
            (maps, thread.original_line, thread.original_commit())
        && let Some(map) = maps.originals.get(commit)
        && let Some(line) = map.map(original)
    {
        let start = thread
            .original_start_line
            .filter(|s| *s < original)
            .and_then(|s| map.map(s))
            .filter(|s| *s < line);
        return ThreadAnchor::Line {
            side,
            line,
            start,
            remapped: true,
        };
    }
    ThreadAnchor::Outdated
}

#[cfg(test)]
mod tests {
    use super::*;
    use corvene_github::review::ReviewComment;

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(str::to_string).collect()
    }

    fn thread(line: Option<u32>, side: DiffSide) -> ReviewThread {
        ReviewThread {
            id: "T".into(),
            path: "a.rs".into(),
            line,
            diff_side: Some(side),
            original_line: Some(3),
            is_outdated: line.is_none(),
            comments: vec![ReviewComment {
                original_commit: Some("orig".into()),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn line_map_follows_unchanged_lines_and_drops_changed_ones() {
        let from = lines("a\nb\nc\nd");
        let to = lines("x\na\nb\nD\nd");
        let map = LineMap::between(&from, &to);
        assert_eq!(map.map(1), Some(2));
        assert_eq!(map.map(2), Some(3));
        assert_eq!(map.map(3), None);
        assert_eq!(map.map(4), Some(5));
        assert!(LineMap::between(&from, &from).is_identity());
        assert_eq!(LineMap::empty().map(1), None);
    }

    #[test]
    fn same_head_without_maps_keeps_githubs_lines() {
        let t = thread(Some(7), DiffSide::Right);
        let out = anchor_threads(&[(0, &t)], None, true);
        assert_eq!(
            out[0].anchor,
            ThreadAnchor::Line {
                side: DiffSide::Right,
                line: 7,
                start: None,
                remapped: false
            }
        );
        // another head and no maps: nothing can be placed
        let out = anchor_threads(&[(0, &t)], None, false);
        assert_eq!(out[0].anchor, ThreadAnchor::Outdated);
    }

    #[test]
    fn maps_carry_lines_and_ranges_to_the_shown_head() {
        let maps = FileLineMaps {
            right: LineMap::between(&lines("a\nb\nc"), &lines("n\na\nb\nc")),
            left: LineMap::identity(),
            originals: HashMap::new(),
        };
        let mut t = thread(Some(3), DiffSide::Right);
        t.start_line = Some(1);
        let out = anchor_threads(&[(4, &t)], Some(&maps), false);
        assert_eq!(out[0].thread, 4);
        assert_eq!(
            out[0].anchor,
            ThreadAnchor::Line {
                side: DiffSide::Right,
                line: 4,
                start: Some(2),
                remapped: false
            }
        );
        let left = thread(Some(2), DiffSide::Left);
        let out = anchor_threads(&[(0, &left)], Some(&maps), false);
        assert!(matches!(
            out[0].anchor,
            ThreadAnchor::Line {
                side: DiffSide::Left,
                line: 2,
                ..
            }
        ));
    }

    #[test]
    fn outdated_threads_come_back_from_their_original_commit() {
        let mut maps = FileLineMaps::identity();
        maps.originals.insert(
            "orig".into(),
            LineMap::between(&lines("a\nb\nc"), &lines("a\nb\nz\nc")),
        );
        let t = thread(None, DiffSide::Right);
        let out = anchor_threads(&[(0, &t)], Some(&maps), true);
        assert_eq!(
            out[0].anchor,
            ThreadAnchor::Line {
                side: DiffSide::Right,
                line: 4,
                start: None,
                remapped: true
            }
        );
        // the original line itself changed: still outdated
        maps.originals.insert(
            "orig".into(),
            LineMap::between(&lines("a\nb\nc"), &lines("a\nb\nC")),
        );
        let out = anchor_threads(&[(0, &t)], Some(&maps), true);
        assert_eq!(out[0].anchor, ThreadAnchor::Outdated);
        // without the maps GitHub's verdict stands
        let out = anchor_threads(&[(0, &t)], None, true);
        assert_eq!(out[0].anchor, ThreadAnchor::Outdated);
    }

    #[test]
    fn file_level_threads_anchor_to_the_file() {
        let mut t = thread(None, DiffSide::Right);
        t.original_line = None;
        t.is_outdated = false;
        let out = anchor_threads(&[(0, &t)], None, true);
        assert_eq!(out[0].anchor, ThreadAnchor::File);
    }
}
