//! Corvene (`1310-file-list-tree`, desktop/desktop#20624 and #20529): the
//! changed files of a list as a folder tree. GHD lists every file flat
//! (`app/src/ui/changes/filter-changes-list.tsx`).
//!
//! Folders come before files, both by name (ignoring case first); a folder
//! whose only child is a folder shares its row (`src/ui/views`, as on
//! github.com). A collapsed folder keeps its files in [`FileTree::order`] but
//! not in [`FileTree::rows`].

use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::ops::Range;
use std::sync::Arc;

use tracing::warn;

use crate::dispatcher::Dispatcher;
use crate::host::Host;
use crate::persistence::StoreExt;

/// One row of the tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TreeRow {
    Folder {
        /// The full path of the row's last folder (`src/ui/views`), the key
        /// of its collapse state.
        path: String,
        /// What the row shows: the folders it stands for (`ui/views`).
        label: String,
        depth: usize,
        /// The folder's files as a range of [`FileTree::order`].
        items: Range<usize>,
        expanded: bool,
    },
    File {
        /// Index into the paths the tree was built from.
        item: usize,
        depth: usize,
    },
}

impl TreeRow {
    pub fn depth(&self) -> usize {
        match self {
            TreeRow::Folder { depth, .. } | TreeRow::File { depth, .. } => *depth,
        }
    }
}

/// The rows of a tree over a list of paths.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileTree {
    pub rows: Vec<TreeRow>,
    /// Every item in tree order, the files of collapsed folders too.
    pub order: Vec<usize>,
    /// Each row's folder row.
    parents: Vec<Option<usize>>,
    /// Each item's row, or the row of the collapsed folder hiding it.
    item_rows: Vec<usize>,
}

impl FileTree {
    /// The tree over `paths` (repository-relative, `/`-separated; a trailing
    /// `/`, an untracked repository, is dropped). `collapsed` says which
    /// folders, by full path, show without their contents.
    pub fn build(paths: &[&str], collapsed: impl Fn(&str) -> bool) -> Self {
        let mut entries: Vec<(usize, Vec<&str>)> = paths
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let p = p.strip_suffix('/').unwrap_or(p);
                (i, p.split('/').collect())
            })
            .collect();
        entries.sort_by(|(_, a), (_, b)| compare(a, b));
        let mut tree = FileTree {
            rows: Vec::new(),
            order: Vec::with_capacity(paths.len()),
            parents: Vec::new(),
            item_rows: vec![0; paths.len()],
        };
        tree.emit(&entries, 0, 0, None, &collapsed);
        tree
    }

    fn emit(
        &mut self,
        entries: &[(usize, Vec<&str>)],
        component: usize,
        depth: usize,
        parent: Option<usize>,
        collapsed: &impl Fn(&str) -> bool,
    ) {
        let mut i = 0;
        while i < entries.len() {
            let (item, parts) = &entries[i];
            if parts.len() <= component + 1 {
                self.item_rows[*item] = self.rows.len();
                self.parents.push(parent);
                self.rows.push(TreeRow::File { item: *item, depth });
                self.order.push(*item);
                i += 1;
                continue;
            }
            let name = parts[component];
            let len = entries[i..]
                .iter()
                .take_while(|(_, p)| p.len() > component + 1 && p[component] == name)
                .count();
            let group = &entries[i..i + len];
            // a folder holding one folder and no files shares its row
            let mut end = component + 1;
            while group
                .iter()
                .all(|(_, p)| p.len() > end + 1 && p[end] == group[0].1[end])
            {
                end += 1;
            }
            let path = parts[..end].join("/");
            let expanded = !collapsed(&path);
            let row = self.rows.len();
            let start = self.order.len();
            self.parents.push(parent);
            self.rows.push(TreeRow::Folder {
                label: parts[component..end].join("/"),
                path,
                depth,
                items: start..start,
                expanded,
            });
            if expanded {
                self.emit(group, end, depth + 1, Some(row), collapsed);
            } else {
                for (item, _) in group {
                    self.item_rows[*item] = row;
                    self.order.push(*item);
                }
            }
            let stop = self.order.len();
            if let TreeRow::Folder { items, .. } = &mut self.rows[row] {
                items.end = stop;
            }
            i += len;
        }
    }

    /// The row showing `item`, or the collapsed folder's row hiding it.
    pub fn row_of_item(&self, item: usize) -> Option<usize> {
        self.item_rows.get(item).copied()
    }

    /// The folder row `row` sits in.
    pub fn parent(&self, row: usize) -> Option<usize> {
        self.parents.get(row).copied().flatten()
    }

    /// The items of `row`: its file, or every file of its folder.
    pub fn row_items(&self, row: usize) -> &[usize] {
        match self.rows.get(row) {
            Some(TreeRow::File { item, .. }) => std::slice::from_ref(item),
            Some(TreeRow::Folder { items, .. }) => &self.order[items.clone()],
            None => &[],
        }
    }

    /// The items of the expanded tree's file rows, top to bottom (what
    /// ⇧↑ / ⇧↓ step through).
    pub fn visible_items(&self) -> Vec<usize> {
        self.rows
            .iter()
            .filter_map(|r| match r {
                TreeRow::File { item, .. } => Some(*item),
                TreeRow::Folder { .. } => None,
            })
            .collect()
    }

    /// Every folder path in the tree, collapsed or not (Expand All /
    /// Collapse All), including the folders a shared row stands for.
    pub fn folder_paths(paths: &[&str]) -> Vec<String> {
        let mut folders = std::collections::BTreeSet::new();
        for p in paths {
            let p = p.strip_suffix('/').unwrap_or(p);
            let mut at = 0;
            while let Some(slash) = p[at..].find('/') {
                folders.insert(p[..at + slash].to_string());
                at += slash + 1;
            }
        }
        folders.into_iter().collect()
    }
}

/// The snapshot each collapsed-folders save takes, and the newest one
/// written.
static SAVE_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static SAVED_GENERATION: std::sync::Mutex<u64> = std::sync::Mutex::new(0);

impl Dispatcher {
    /// View › Show Changes as Tree / as List.
    pub fn set_file_list_tree(tree: bool, cx: &mut dyn Host) {
        Self::update_settings(cx, |s| s.file_list_tree = tree);
    }

    /// Collapse or expand `folders` in `id`'s changes tree and save the
    /// collapsed ones. Folders without changed files any more are
    /// forgotten then.
    pub fn set_folders_collapsed(
        id: u64,
        folders: Vec<String>,
        collapsed: bool,
        cx: &mut dyn Host,
    ) {
        Self::state(cx).update(cx, |s, cx| {
            let current = s.collapsed_folders.get(&id).cloned().unwrap_or_default();
            let mut next: BTreeSet<String> = (*current).clone();
            for folder in folders {
                if collapsed {
                    next.insert(folder);
                } else {
                    next.remove(&folder);
                }
            }
            if let Some(status) = s.repo_states.get(&id).and_then(|rs| rs.status.as_ref()) {
                let paths: Vec<&str> = status.files.iter().map(|f| f.path.as_str()).collect();
                let live: std::collections::HashSet<String> =
                    FileTree::folder_paths(&paths).into_iter().collect();
                next.retain(|folder| live.contains(folder));
            }
            if next == *current {
                return;
            }
            if next.is_empty() {
                s.collapsed_folders.remove(&id);
            } else {
                s.collapsed_folders.insert(id, Arc::new(next));
            }
            cx.notify();
            let store = s.store.clone();
            let saved: std::collections::HashMap<u64, Vec<String>> = s
                .collapsed_folders
                .iter()
                .map(|(id, set)| (*id, set.iter().cloned().collect()))
                .collect();
            // the saves run on threads of their own: a later snapshot that
            // got written first must not be overwritten by an older one
            let generation = SAVE_GENERATION.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            std::thread::Builder::new()
                .name("collapsed-folders".into())
                .spawn(move || {
                    let Ok(mut written) = SAVED_GENERATION.lock() else {
                        return;
                    };
                    if *written > generation {
                        return;
                    }
                    *written = generation;
                    if let Err(err) = store.save_collapsed_folders(&saved) {
                        warn!(?err, "could not save the collapsed folders");
                    }
                })
                .map(drop)
                .unwrap_or_else(|err| warn!(?err, "could not save the collapsed folders"));
        });
    }

    /// ⌘-click on a folder row: add its files to the selection, or take
    /// them out when they are all selected.
    pub fn toggle_files_selection(id: u64, paths: Vec<String>, cx: &mut dyn Host) {
        if paths.is_empty() {
            return;
        }
        let changed = Self::state(cx).update(cx, |s, cx| {
            let rs = s.repo_state_mut(id);
            let before = rs.selected_file.clone();
            let selected: std::collections::HashSet<&String> = rs.selected_files.iter().collect();
            let all = paths.iter().all(|p| selected.contains(p));
            if all {
                let gone: std::collections::HashSet<&String> = paths.iter().collect();
                rs.selected_files.retain(|p| !gone.contains(p));
                if rs.selected_file.as_ref().is_some_and(|p| gone.contains(p)) {
                    rs.selected_file = rs.selected_files.last().cloned();
                }
            } else {
                let missing: Vec<String> = paths
                    .iter()
                    .filter(|p| !selected.contains(p))
                    .cloned()
                    .collect();
                rs.selected_files.extend(missing);
                rs.selected_file = paths.first().cloned();
            }
            if rs.selected_file != before {
                rs.diff = None;
            }
            cx.notify();
            rs.selected_file != before
        });
        if changed {
            Self::load_diff(id, cx);
        }
    }
}

/// Folders first, then by name ignoring case, then exactly.
fn compare(a: &[&str], b: &[&str]) -> Ordering {
    for i in 0.. {
        let (a_file, b_file) = (i + 1 >= a.len(), i + 1 >= b.len());
        if a_file != b_file {
            return if a_file {
                Ordering::Greater
            } else {
                Ordering::Less
            };
        }
        let (x, y) = (a.get(i).copied(), b.get(i).copied());
        let (Some(x), Some(y)) = (x, y) else {
            return a.len().cmp(&b.len());
        };
        if x != y {
            return compare_names(x, y);
        }
        if a_file {
            return Ordering::Equal;
        }
    }
    Ordering::Equal
}

fn compare_names(x: &str, y: &str) -> Ordering {
    fn lower(s: &str) -> impl Iterator<Item = char> + '_ {
        s.chars().flat_map(char::to_lowercase)
    }
    lower(x).cmp(lower(y)).then_with(|| x.cmp(y))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(tree: &FileTree, paths: &[&str]) -> Vec<String> {
        tree.rows
            .iter()
            .map(|r| match r {
                TreeRow::Folder {
                    label,
                    depth,
                    expanded,
                    items,
                    ..
                } => format!(
                    "{}{}{label}/ ({})",
                    "  ".repeat(*depth),
                    if *expanded { "v " } else { "> " },
                    items.len()
                ),
                TreeRow::File { item, depth } => {
                    format!("{}{}", "  ".repeat(*depth), paths[*item])
                }
            })
            .collect()
    }

    #[test]
    fn folders_come_first_and_single_child_chains_share_a_row() {
        let paths = [
            "README.md",
            "src/ui/views/a.rs",
            "src/ui/views/b.rs",
            "src/main.rs",
            "Cargo.toml",
            "docs/x/y/z.md",
        ];
        let tree = FileTree::build(&paths, |_| false);
        assert_eq!(
            labels(&tree, &paths),
            [
                "v docs/x/y/ (1)",
                "  docs/x/y/z.md",
                "v src/ (3)",
                "  v ui/views/ (2)",
                "    src/ui/views/a.rs",
                "    src/ui/views/b.rs",
                "  src/main.rs",
                "Cargo.toml",
                "README.md",
            ]
        );
        assert_eq!(tree.order, [5, 1, 2, 3, 4, 0]);
        let TreeRow::Folder { path, .. } = &tree.rows[3] else {
            panic!()
        };
        assert_eq!(path, "src/ui/views");
        assert_eq!(tree.parent(4), Some(3));
        assert_eq!(tree.parent(3), Some(2));
        assert_eq!(tree.parent(7), None);
        assert_eq!(tree.row_items(2), [1, 2, 3]);
        assert_eq!(tree.row_items(6), [3]);
        assert_eq!(tree.visible_items(), [5, 1, 2, 3, 4, 0]);
    }

    #[test]
    fn collapsed_folders_hide_their_rows_but_keep_their_items() {
        let paths = ["src/a.rs", "src/b/c.rs", "src/b/d.rs", "z.txt"];
        let tree = FileTree::build(&paths, |p| p == "src/b");
        assert_eq!(
            labels(&tree, &paths),
            ["v src/ (3)", "  > b/ (2)", "  src/a.rs", "z.txt"]
        );
        assert_eq!(tree.order, [1, 2, 0, 3]);
        assert_eq!(tree.row_of_item(1), Some(1));
        assert_eq!(tree.row_of_item(2), Some(1));
        assert_eq!(tree.row_of_item(0), Some(2));
        assert_eq!(tree.row_items(1), [1, 2]);
        assert_eq!(tree.visible_items(), [0, 3]);
    }

    #[test]
    fn names_sort_ignoring_case_and_untracked_repositories_are_files() {
        let paths = ["b.txt", "A.txt", "Lib/x", "lib/y", "sub/"];
        let tree = FileTree::build(&paths, |_| false);
        assert_eq!(
            labels(&tree, &paths),
            [
                "v Lib/ (1)",
                "  Lib/x",
                "v lib/ (1)",
                "  lib/y",
                "A.txt",
                "b.txt",
                "sub/",
            ]
        );
        assert_eq!(
            FileTree::folder_paths(&paths),
            ["Lib".to_string(), "lib".to_string()]
        );
    }

    #[test]
    fn a_folder_with_a_file_and_a_folder_does_not_share_its_row() {
        let paths = ["a/b/c.rs", "a/d.rs"];
        let tree = FileTree::build(&paths, |_| false);
        assert_eq!(
            labels(&tree, &paths),
            ["v a/ (2)", "  v b/ (1)", "    a/b/c.rs", "  a/d.rs"]
        );
    }
}
