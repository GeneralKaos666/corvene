//! Corvene (`1112-sparse-checkout`): Repository › Sparse Checkout… opens a
//! dialog (`Popup::SparseCheckout`) with `HEAD`'s folders as a tree of
//! checkboxes; Apply runs `git sparse-checkout set --cone` with the ticked
//! folders, Turn Off runs `git sparse-checkout disable`. While sparse
//! checkout is on, the Changes tab shows a banner
//! (`RepositoryState::sparse_checkout`, read with every refresh). GHD has
//! no sparse checkout.

use crate::dispatcher::Dispatcher;
use crate::host::Host;
use crate::remote::spawn_bg;
use crate::state::Popup;

/// Sparse checkout is on: what the Changes banner says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SparseSummary {
    /// Cone mode: the patterns are folders.
    pub cone: bool,
    /// How many folders (cone mode) or patterns are listed.
    pub patterns: usize,
}

/// What the dialog shows (`RepositoryState::sparse_editor`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SparseEditor {
    pub loading: bool,
    /// Every folder of `HEAD`, parents first.
    pub directories: Vec<String>,
    pub checkout: corvene_git::SparseCheckout,
    pub error: Option<String>,
}

impl Dispatcher {
    /// Repository › Sparse Checkout… and the banner's Edit….
    pub fn show_sparse_checkout(id: u64, cx: &mut dyn Host) {
        if !Self::state(cx)
            .read(cx)
            .flags
            .bool(crate::flags::ids::SPARSE_CHECKOUT)
        {
            return;
        }
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        Self::state(cx).update(cx, |s, _| {
            s.repo_state_mut(id).sparse_editor = Some(SparseEditor {
                loading: true,
                ..Default::default()
            })
        });
        Self::show_popup(Popup::SparseCheckout { repo: id }, cx);
        spawn_bg(
            cx,
            move || {
                let directories = corvene_git::tree_directories(git.clone(), &workdir)?;
                let checkout = corvene_git::sparse_checkout(git, &workdir)?;
                Ok::<_, corvene_git::GitError>((directories, checkout))
            },
            move |result, cx| {
                Self::state(cx).update(cx, |s, cx| {
                    let Some(editor) = s.repo_state_mut(id).sparse_editor.as_mut() else {
                        return;
                    };
                    editor.loading = false;
                    match result {
                        Ok((directories, checkout)) => {
                            editor.directories = directories;
                            editor.checkout = checkout;
                        }
                        Err(err) => editor.error = Some(err.to_string()),
                    }
                    cx.notify();
                });
            },
        );
    }

    /// Apply: check out the files at the top and `folders`.
    pub fn set_sparse_checkout(id: u64, folders: Vec<String>, cx: &mut dyn Host) {
        Self::close_sparse_checkout(id, cx);
        Self::run_history_op(
            id,
            "Could not change the sparse checkout",
            move |git, workdir| corvene_git::sparse_checkout_set(git, &workdir, &folders),
            cx,
        );
    }

    /// Turn Off: every file is checked out again.
    pub fn disable_sparse_checkout(id: u64, cx: &mut dyn Host) {
        Self::close_sparse_checkout(id, cx);
        Self::run_history_op(
            id,
            "Could not turn off sparse checkout",
            move |git, workdir| corvene_git::sparse_checkout_disable(git, &workdir),
            cx,
        );
    }

    pub fn close_sparse_checkout(id: u64, cx: &mut dyn Host) {
        Self::close_popup_if(|p| matches!(p, Popup::SparseCheckout { .. }), cx);
        Self::state(cx).update(cx, |s, _| s.repo_state_mut(id).sparse_editor = None);
    }
}

/// The dialog's ticks: `selected` is the smallest set of folders, each
/// checked out with everything in it (cone mode).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SparseSelection {
    pub selected: std::collections::BTreeSet<String>,
}

/// A folder's tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tick {
    /// The folder or a parent is ticked.
    On,
    /// A folder inside it is ticked.
    Partly,
    Off,
}

fn parent_of(dir: &str) -> Option<&str> {
    dir.rfind('/').map(|i| &dir[..i])
}

fn is_inside(dir: &str, ancestor: &str) -> bool {
    dir.len() > ancestor.len()
        && dir.starts_with(ancestor)
        && dir.as_bytes()[ancestor.len()] == b'/'
}

impl SparseSelection {
    /// The cone patterns `git sparse-checkout list` gave, reduced to the
    /// topmost ones.
    pub fn from_patterns(patterns: &[String]) -> Self {
        let mut selection = Self::default();
        for p in patterns {
            let p = p.trim_matches('/');
            if !p.is_empty() {
                selection.tick(p);
            }
        }
        selection
    }

    pub fn tick_of(&self, dir: &str) -> Tick {
        if self.selected.iter().any(|s| s == dir || is_inside(dir, s)) {
            Tick::On
        } else if self.selected.iter().any(|s| is_inside(s, dir)) {
            Tick::Partly
        } else {
            Tick::Off
        }
    }

    /// Tick `dir` (with everything in it) or untick it. Unticking a folder
    /// whose parent is ticked ticks the parent's other folders instead;
    /// `directories` is the whole tree, to find them.
    pub fn toggle(&mut self, dir: &str, directories: &[String]) {
        if self.tick_of(dir) == Tick::On {
            self.untick(dir, directories);
        } else {
            self.tick(dir);
        }
    }

    fn tick(&mut self, dir: &str) {
        // already in through a ticked parent
        if self.selected.iter().any(|s| s == dir || is_inside(dir, s)) {
            return;
        }
        self.selected.retain(|s| !is_inside(s, dir));
        self.selected.insert(dir.to_string());
    }

    fn untick(&mut self, dir: &str, directories: &[String]) {
        self.selected.retain(|s| s != dir && !is_inside(s, dir));
        // a ticked parent: tick its other children down to `dir`
        let Some(ancestor) = self.selected.iter().find(|s| is_inside(dir, s)).cloned() else {
            return;
        };
        self.selected.remove(&ancestor);
        let mut path = Vec::new();
        let mut cursor = dir;
        while cursor != ancestor {
            path.push(cursor);
            match parent_of(cursor) {
                Some(parent) => cursor = parent,
                None => break,
            }
        }
        // every folder on the way down: its children but the next step
        for step in path {
            let Some(parent) = parent_of(step) else {
                continue;
            };
            for sibling in directories {
                if sibling != step && parent_of(sibling) == Some(parent) {
                    self.selected.insert(sibling.clone());
                }
            }
        }
    }

    /// The folders for `git sparse-checkout set --cone`.
    pub fn folders(&self) -> Vec<String> {
        self.selected.iter().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> Vec<String> {
        ["docs", "src", "src/a", "src/a/x", "src/b", "src/c", "srcs"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    #[test]
    fn ticks_follow_the_cone() {
        let dirs = tree();
        let mut sel = SparseSelection::default();
        sel.toggle("src/a/x", &dirs);
        assert_eq!(sel.tick_of("src"), Tick::Partly);
        assert_eq!(sel.tick_of("src/a"), Tick::Partly);
        assert_eq!(sel.tick_of("src/a/x"), Tick::On);
        assert_eq!(sel.tick_of("srcs"), Tick::Off);
        // ticking the parent swallows the child
        sel.toggle("src", &dirs);
        assert_eq!(sel.folders(), vec!["src"]);
        assert_eq!(sel.tick_of("src/b"), Tick::On);
        // unticking a child of a ticked parent ticks its siblings
        sel.toggle("src/a/x", &dirs);
        assert_eq!(sel.folders(), vec!["src/b", "src/c"]);
        assert_eq!(sel.tick_of("src/a"), Tick::Off);
        sel.toggle("src/b", &dirs);
        assert_eq!(sel.folders(), vec!["src/c"]);
        assert_eq!(
            SparseSelection::from_patterns(&["src".into(), "src/a".into(), "docs/".into()])
                .folders(),
            vec!["docs", "src"]
        );
    }
}
