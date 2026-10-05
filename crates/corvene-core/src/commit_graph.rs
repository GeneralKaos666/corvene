//! The lanes of History's commit graph (Corvene `1213-commit-graph`; GHD
//! `ui/history/commit-list.tsx` lists the commits without one).
//!
//! Rows come in list order (children before their parents, as `git log`
//! walks) and are laid out once: [`CommitGraph::push`] continues from the
//! lanes the previous row left open, so loading another page of History
//! only lays out the new rows, and every row keeps its colours while the
//! list scrolls. Each row records what its own segment of the graph shows
//! ([`Edge`]s from the top of the row to its commit, from the commit to the
//! bottom, and lanes passing straight through), so a virtualised list paints
//! a row without looking at its neighbours.
//!
//! Lanes keep their column while they live (a free column is reused from
//! the left); a commit continues in its own lane towards its first parent,
//! its other parents get a new lane each (or join the lane already waiting
//! for that parent), and lanes waiting for the same commit meet at it.
//! The newest commit's first-parent chain (or HEAD's, when the list holds
//! other branches too) is the emphasised lane, always colour 0.

use std::collections::HashSet;
use std::ops::Range;

use corvene_models::Commit;

/// Colours a graph cycles through: 0 is the emphasised (current branch)
/// lane's, the others go round `1..PALETTE_LEN`.
pub const PALETTE_LEN: u8 = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeKind {
    /// From the top of the row to the bottom, in one column.
    Through,
    /// From the top of the row in column `from` to the commit (`to`).
    In,
    /// From the commit (`from`) to the bottom of the row in column `to`.
    Out,
}

/// One line of a row's segment of the graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edge {
    pub kind: EdgeKind,
    pub from: u16,
    pub to: u16,
    pub color: u8,
    /// Part of the current branch's first-parent chain.
    pub emphasized: bool,
}

/// One commit's row of the graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphRow {
    /// The column of the commit's dot.
    pub lane: u16,
    pub color: u8,
    pub emphasized: bool,
    /// More than one parent.
    pub merge: bool,
    edges: Range<u32>,
}

#[derive(Clone, Copy, Debug)]
struct Lane {
    /// The commit this lane runs down to.
    awaiting: u64,
    color: u8,
    emphasized: bool,
}

/// The graph of a commit list, laid out row by row.
#[derive(Default)]
pub struct CommitGraph {
    lanes: Vec<Option<Lane>>,
    rows: Vec<GraphRow>,
    edges: Vec<Edge>,
    /// Commits laid out so far. A parent that is already among them (only
    /// with clock skew: `git log` lists by commit date) gets no edge rather
    /// than a lane that would never end.
    done: HashSet<u64>,
    /// The next commit of the emphasised first-parent chain.
    head_next: Option<u64>,
    /// The commit the emphasised chain starts at; `None`: the first row.
    head: Option<u64>,
    first_parent: bool,
    next_color: u8,
    width: usize,
    /// The first and last rows' commits, to tell an extended list from a
    /// different one ([`Self::sync`]).
    first: Option<u64>,
    last: Option<u64>,
}

/// History's All branches list: the commits of HEAD and of every local and
/// remote-tracking branch, newest first, in pages.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AllBranchesHistory {
    /// Where the walk starts (HEAD's commit first), read with the first
    /// page; the next pages walk from the same tips.
    pub tips: Vec<String>,
    pub commits: Vec<Commit>,
    /// The first page is in (until then History shows HEAD's commits).
    pub loaded: bool,
    pub loading: bool,
    /// A first-page reload was asked for while a page was loading.
    pub reload_pending: bool,
    pub exhausted: bool,
}

/// A short key for a SHA: its first 16 hex digits.
fn key(sha: &str) -> u64 {
    sha.get(..16)
        .and_then(|s| u64::from_str_radix(s, 16).ok())
        .unwrap_or_else(|| {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            sha.hash(&mut h);
            h.finish()
        })
}

impl CommitGraph {
    /// An empty graph whose emphasised chain starts at `head` (the first
    /// row when `None`); with `first_parent`, merges' other parents are
    /// left out (`git log --first-parent`).
    pub fn new(head: Option<&str>, first_parent: bool) -> Self {
        let head = head.map(key);
        Self {
            head,
            head_next: head,
            first_parent,
            next_color: 1,
            ..Self::default()
        }
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The most columns any row so far uses.
    pub fn width(&self) -> usize {
        self.width
    }

    /// Row `ix` and its edges.
    pub fn row(&self, ix: usize) -> Option<(&GraphRow, &[Edge])> {
        let row = self.rows.get(ix)?;
        let edges = &self.edges[row.edges.start as usize..row.edges.end as usize];
        Some((row, edges))
    }

    /// Lays out `commits`, the whole list: when it starts with the rows
    /// already laid out (a further page), only the new ones are added;
    /// another list (a refresh that changed it, another branch) starts over.
    pub fn sync(&mut self, commits: &[Commit], head: Option<&str>, first_parent: bool) {
        let n = self.rows.len();
        let same = self.head == head.map(key)
            && self.first_parent == first_parent
            && n <= commits.len()
            && (n == 0
                || (self.first == Some(key(&commits[0].sha))
                    && self.last == Some(key(&commits[n - 1].sha))));
        if !same {
            *self = Self::new(head, first_parent);
        }
        for commit in &commits[self.rows.len()..] {
            self.push(&commit.sha, &commit.parents);
        }
    }

    fn free_column(&mut self) -> usize {
        match self.lanes.iter().position(Option::is_none) {
            Some(ix) => ix,
            None => {
                self.lanes.push(None);
                self.lanes.len() - 1
            }
        }
    }

    fn take_color(&mut self) -> u8 {
        let color = self.next_color.max(1);
        self.next_color = if color + 1 >= PALETTE_LEN {
            1
        } else {
            color + 1
        };
        color
    }

    /// Lays out the next row: commit `sha` with `parents`.
    pub fn push(&mut self, sha: &str, parents: &[String]) {
        let k = key(sha);
        if self.rows.is_empty() {
            self.first = Some(k);
            if self.head.is_none() {
                self.head_next = Some(k);
            }
        }
        self.last = Some(k);
        let emphasized = self.head_next == Some(k);
        let start = self.edges.len() as u32;

        // the lanes running down to this commit meet at the leftmost
        let awaited = self
            .lanes
            .iter()
            .position(|l| l.is_some_and(|l| l.awaiting == k));
        let (node, lane_color) = match awaited {
            Some(ix) => (ix, self.lanes[ix].map_or(0, |l| l.color)),
            None => {
                let ix = self.free_column();
                let color = if emphasized { 0 } else { self.take_color() };
                (ix, color)
            }
        };
        let color = if emphasized { 0 } else { lane_color };
        for (ix, lane) in self.lanes.iter_mut().enumerate() {
            let Some(l) = *lane else { continue };
            let kind = if l.awaiting == k {
                *lane = None;
                EdgeKind::In
            } else {
                EdgeKind::Through
            };
            self.edges.push(Edge {
                kind,
                from: ix as u16,
                to: if kind == EdgeKind::In { node } else { ix } as u16,
                color: l.color,
                emphasized: l.emphasized,
            });
        }

        let parents = if self.first_parent {
            &parents[..parents.len().min(1)]
        } else {
            parents
        };
        let mut seen: Vec<u64> = Vec::with_capacity(parents.len());
        for (pi, parent) in parents.iter().enumerate() {
            let pk = key(parent);
            if self.done.contains(&pk) || seen.contains(&pk) {
                continue;
            }
            seen.push(pk);
            if pi == 0 {
                // the commit's own lane goes on to its first parent
                self.lanes[node] = Some(Lane {
                    awaiting: pk,
                    color,
                    emphasized,
                });
                self.edges.push(Edge {
                    kind: EdgeKind::Out,
                    from: node as u16,
                    to: node as u16,
                    color,
                    emphasized,
                });
            } else {
                let existing = self
                    .lanes
                    .iter()
                    .position(|l| l.is_some_and(|l| l.awaiting == pk));
                let (ix, lane_color) = match existing {
                    Some(ix) => (ix, self.lanes[ix].map_or(0, |l| l.color)),
                    None => {
                        let ix = self.free_column();
                        let c = self.take_color();
                        self.lanes[ix] = Some(Lane {
                            awaiting: pk,
                            color: c,
                            emphasized: false,
                        });
                        (ix, c)
                    }
                };
                self.edges.push(Edge {
                    kind: EdgeKind::Out,
                    from: node as u16,
                    to: ix as u16,
                    color: lane_color,
                    emphasized: false,
                });
            }
        }
        if emphasized {
            self.head_next = parents
                .first()
                .map(|p| key(p))
                .filter(|p| !self.done.contains(p));
        }
        self.done.insert(k);

        // columns this row draws in
        let used = self.edges[start as usize..]
            .iter()
            .map(|e| e.from.max(e.to) as usize + 1)
            .max()
            .unwrap_or(0)
            .max(node + 1);
        self.width = self.width.max(used);
        while self.lanes.last().is_some_and(Option::is_none) {
            self.lanes.pop();
        }
        self.rows.push(GraphRow {
            lane: node as u16,
            color,
            emphasized,
            merge: parents.len() > 1,
            edges: start..self.edges.len() as u32,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 40-digit SHA from a short name ("a" → "a000…").
    fn sha(name: &str) -> String {
        let hex: String = name
            .bytes()
            .map(|b| format!("{:02x}", b))
            .collect::<String>();
        format!("{hex:0<40}")
    }

    fn build(rows: &[(&str, &[&str])]) -> CommitGraph {
        let mut g = CommitGraph::new(None, false);
        for (name, parents) in rows {
            let parents: Vec<String> = parents.iter().map(|p| sha(p)).collect();
            g.push(&sha(name), &parents);
        }
        g
    }

    /// Row `ix` as text: the dot's column, then each edge as `|c` (through),
    /// `a\b` / `a/b` (into / out of the dot, from column a to b).
    fn draw(g: &CommitGraph, ix: usize) -> String {
        let (row, edges) = g.row(ix).unwrap();
        let mut out = format!("*{}", row.lane);
        for e in edges {
            out.push(' ');
            out.push_str(&match e.kind {
                EdgeKind::Through => format!("|{}", e.from),
                EdgeKind::In => format!("{}v{}", e.from, e.to),
                EdgeKind::Out => format!("{}>{}", e.from, e.to),
            });
        }
        out
    }

    fn drawing(g: &CommitGraph) -> Vec<String> {
        (0..g.len()).map(|ix| draw(g, ix)).collect()
    }

    #[test]
    fn linear_history_is_one_lane() {
        let g = build(&[("c", &["b"]), ("b", &["a"]), ("a", &[])]);
        assert_eq!(drawing(&g), ["*0 0>0", "*0 0v0 0>0", "*0 0v0"]);
        assert_eq!(g.width(), 1);
        assert!((0..3).all(|ix| g.row(ix).unwrap().0.emphasized));
    }

    #[test]
    fn merge_opens_a_lane_that_meets_the_fork_point() {
        // m merges x (branch) into b; both come from a
        let g = build(&[("m", &["b", "x"]), ("x", &["a"]), ("b", &["a"]), ("a", &[])]);
        assert_eq!(
            drawing(&g),
            ["*0 0>0 0>1", "*1 |0 1v1 1>1", "*0 0v0 |1 0>0", "*0 0v0 1v0",]
        );
        assert!(g.row(0).unwrap().0.merge);
        assert_eq!(g.width(), 2);
        // the merged branch is not the emphasised chain
        assert!(!g.row(1).unwrap().0.emphasized);
        assert!(g.row(2).unwrap().0.emphasized);
        assert_ne!(g.row(1).unwrap().0.color, 0);
        assert_eq!(g.row(2).unwrap().0.color, 0);
    }

    #[test]
    fn octopus_merge_opens_a_lane_per_parent() {
        let g = build(&[
            ("m", &["a", "b", "c"]),
            ("c", &["r"]),
            ("b", &["r"]),
            ("a", &["r"]),
            ("r", &[]),
        ]);
        assert_eq!(
            drawing(&g),
            [
                "*0 0>0 0>1 0>2",
                "*2 |0 |1 2v2 2>2",
                "*1 |0 1v1 |2 1>1",
                "*0 0v0 |1 |2 0>0",
                "*0 0v0 1v0 2v0",
            ]
        );
        let colors: HashSet<u8> = (1..4).map(|ix| g.row(ix).unwrap().0.color).collect();
        assert_eq!(colors.len(), 3, "each lane its own colour");
        assert_eq!(g.width(), 3);
    }

    #[test]
    fn criss_cross_merges_reuse_waiting_lanes() {
        // m1 = merge(a1, b1), m2 = merge(b1, a1)
        let g = build(&[
            ("t", &["m1", "m2"]),
            ("m2", &["b1", "a1"]),
            ("m1", &["a1", "b1"]),
            ("b1", &["r"]),
            ("a1", &["r"]),
            ("r", &[]),
        ]);
        assert_eq!(
            drawing(&g),
            [
                "*0 0>0 0>1",
                "*1 |0 1v1 1>1 1>2",
                "*0 0v0 |1 |2 0>0 0>1",
                "*1 |0 1v1 |2 1>1",
                "*0 0v0 |1 2v0 0>0",
                "*0 0v0 1v0",
            ]
        );
    }

    #[test]
    fn root_commits_end_their_lane_and_free_the_column() {
        // two unrelated histories merged: o is a second root
        let g = build(&[("m", &["a", "o"]), ("o", &[]), ("a", &[]), ("n", &[])]);
        assert_eq!(drawing(&g), ["*0 0>0 0>1", "*1 |0 1v1", "*0 0v0", "*0"]);
        // a commit no lane waits for (another branch's tip) takes a free column
        assert!(!g.row(3).unwrap().0.emphasized);
    }

    #[test]
    fn a_parent_listed_before_its_child_gets_no_lane() {
        // clock skew: p (parent of b) is listed before b
        let g = build(&[("m", &["a", "b"]), ("a", &["p"]), ("p", &[]), ("b", &["p"])]);
        let (_, edges) = g.row(3).unwrap();
        assert!(edges.iter().all(|e| e.kind != EdgeKind::Out));
        assert!(g.lanes.is_empty(), "no lane left waiting");
    }

    #[test]
    fn first_parent_leaves_out_merged_parents() {
        let mut g = CommitGraph::new(None, true);
        g.push(&sha("m"), &[sha("b"), sha("x")]);
        g.push(&sha("b"), &[sha("a")]);
        assert_eq!(drawing(&g), ["*0 0>0", "*0 0v0 0>0"]);
        assert!(!g.row(0).unwrap().0.merge);
    }

    #[test]
    fn head_chain_is_emphasised_among_other_branches() {
        // all branches: `f` (another branch's tip) is listed above HEAD `h`
        let mut g = CommitGraph::new(Some(&sha("h")), false);
        g.push(&sha("f"), &[sha("h")]);
        g.push(&sha("h"), &[sha("a")]);
        g.push(&sha("a"), &[]);
        let rows: Vec<_> = (0..3).map(|ix| g.row(ix).unwrap().0.clone()).collect();
        assert!(!rows[0].emphasized);
        assert_ne!(rows[0].color, 0);
        assert!(rows[1].emphasized && rows[2].emphasized);
        assert_eq!((rows[1].color, rows[2].color), (0, 0));
        let (_, edges) = g.row(2).unwrap();
        assert!(edges.iter().all(|e| e.emphasized));
    }

    fn commit(name: &str, parents: &[&str]) -> Commit {
        let who = corvene_models::CommitIdentity {
            name: String::new(),
            email: String::new(),
            seconds: 0,
            offset: 0,
        };
        Commit {
            sha: sha(name),
            summary: String::new(),
            body: String::new(),
            author: who.clone(),
            committer: who,
            parents: parents.iter().map(|p| sha(p)).collect(),
            trailers: Vec::new(),
            tags: Vec::new(),
            signature: None,
        }
    }

    #[test]
    fn sync_extends_pages_and_restarts_for_another_list() {
        let all = vec![
            commit("m", &["b", "x"]),
            commit("x", &["a"]),
            commit("b", &["a"]),
            commit("a", &[]),
        ];
        let mut paged = CommitGraph::default();
        paged.sync(&all[..2], None, false);
        assert_eq!(paged.len(), 2);
        paged.sync(&all, None, false);
        let full = build(&[("m", &["b", "x"]), ("x", &["a"]), ("b", &["a"]), ("a", &[])]);
        assert_eq!(drawing(&paged), drawing(&full));
        let colors = |g: &CommitGraph| -> Vec<u8> {
            (0..g.len()).map(|ix| g.row(ix).unwrap().0.color).collect()
        };
        assert_eq!(colors(&paged), colors(&full));

        // a new commit on top: laid out again from the start
        let mut moved = vec![commit("n", &["m"])];
        moved.extend(all.iter().cloned());
        paged.sync(&moved, None, false);
        assert_eq!(paged.len(), 5);
        assert_eq!(draw(&paged, 0), "*0 0>0");
        assert!(paged.row(0).unwrap().0.emphasized);

        // the first-parent switch starts over too
        paged.sync(&moved, None, true);
        assert!(!paged.row(1).unwrap().0.merge);
    }

    #[test]
    fn many_branches_reuse_freed_columns() {
        // a long history of short-lived branches never widens past 2
        let mut g = CommitGraph::new(None, false);
        for i in 0..1000 {
            let m = format!("m{i}");
            let x = format!("x{i}");
            let next = format!("m{}", i + 1);
            g.push(&sha(&m), &[sha(&next), sha(&x)]);
            g.push(&sha(&x), &[sha(&next)]);
        }
        assert_eq!(g.width(), 2);
        assert_eq!(g.len(), 2000);
    }
}
