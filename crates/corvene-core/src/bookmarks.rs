//! Corvene (`294-follow-moved-repositories`): Corvene keeps a file bookmark
//! of each repository's folder (`corvene_platform::bookmarks`, macOS), made
//! after a refresh finds it, and when the folder is gone it follows the
//! bookmark to where the folder was moved or renamed: the entry gets the new
//! path and a banner says so. A folder moved to the Trash, deleted, or
//! somewhere that is no longer a repository is not followed; the missing
//! repository view (with Locate…) shows as before. GHD only offers Locate…
//! (`ui/missing-repository.tsx`).

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::dispatcher::Dispatcher;
use crate::host::Host;
use crate::remote::spawn_bg;

/// The store key: repository id → [`Saved`].
const KEY: &str = "repositories.bookmarks";

/// A repository's bookmark and the path it was made for.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Saved {
    path: PathBuf,
    /// The bookmark data, hex.
    bookmark: String,
}

thread_local! {
    /// The stored bookmarks, read on first use.
    static SAVED: RefCell<Option<HashMap<u64, Saved>>> = const { RefCell::new(None) };
    /// Repositories whose bookmark is being made.
    static PENDING: RefCell<HashSet<u64>> = RefCell::new(HashSet::new());
}

fn with_saved<T>(store: &corvene_store::Store, f: impl FnOnce(&mut HashMap<u64, Saved>) -> T) -> T {
    SAVED.with(|saved| {
        let mut saved = saved.borrow_mut();
        let map = saved.get_or_insert_with(|| store.get(KEY).ok().flatten().unwrap_or_default());
        f(map)
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|ix| u8::from_str_radix(text.get(ix..ix + 2)?, 16).ok())
        .collect()
}

/// A path in a Trash folder (`~/.Trash`, a volume's `.Trashes`).
fn in_trash(path: &Path) -> bool {
    path.components()
        .any(|c| matches!(c.as_os_str().to_str(), Some(".Trash" | ".Trashes")))
}

impl Dispatcher {
    fn follows_moved_repositories(cx: &dyn Host) -> bool {
        cfg!(target_os = "macos")
            && Self::state(cx)
                .read(cx)
                .flags
                .bool(crate::flags::ids::FOLLOW_MOVED_REPOSITORIES)
    }

    /// The repository is where its entry says: keep a bookmark of the
    /// folder (made again when the path changed).
    pub(crate) fn remember_repository_location(id: u64, cx: &mut dyn Host) {
        if !Self::follows_moved_repositories(cx) {
            return;
        }
        let state = Self::state(cx);
        let Some(path) = state
            .read(cx)
            .repository(id)
            .filter(|r| !r.missing)
            .map(|r| r.path.clone())
        else {
            return;
        };
        let known = with_saved(&state.read(cx).store, |saved| {
            saved.get(&id).is_some_and(|s| s.path == path)
        });
        if known || !PENDING.with(|p| p.borrow_mut().insert(id)) {
            return;
        }
        let at = path.clone();
        spawn_bg(
            cx,
            move || corvene_platform::bookmarks::bookmark(&at).map(|data| hex(&data)),
            move |bookmark, cx| {
                PENDING.with(|p| p.borrow_mut().remove(&id));
                let Some(bookmark) = bookmark else {
                    return;
                };
                let s = Self::state(cx).read(cx);
                with_saved(&s.store, |saved| {
                    saved.insert(id, Saved { path, bookmark });
                    if let Err(err) = s.store.set(KEY, saved) {
                        warn!(%err, "could not save the repository bookmarks");
                    }
                });
            },
        );
    }

    /// The repository's folder at `missing_path` is gone: follow its
    /// bookmark, else go on to GHD's missing-worktree recovery. `false`
    /// when there is nothing to follow (the caller recovers at once).
    pub(crate) fn follow_moved_repository(
        id: u64,
        missing_path: PathBuf,
        cx: &mut dyn Host,
    ) -> bool {
        if !Self::follows_moved_repositories(cx) {
            return false;
        }
        let bookmark = with_saved(&Self::state(cx).read(cx).store, |saved| {
            saved.get(&id).and_then(|s| unhex(&s.bookmark))
        });
        let Some(bookmark) = bookmark else {
            return false;
        };
        let gone = missing_path.clone();
        spawn_bg(
            cx,
            move || {
                let moved = corvene_platform::bookmarks::resolve_bookmark(&bookmark)?;
                if moved == gone || in_trash(&moved) || !moved.is_dir() {
                    return None;
                }
                let workdir = corvene_git::top_level_working_directory(&moved)?;
                let main = corvene_git::main_worktree_path(&workdir);
                Some((workdir, main))
            },
            move |found, cx| {
                let Some((workdir, main)) = found else {
                    return Self::recover_missing_worktree(id, missing_path, cx);
                };
                let moved = Self::state(cx).update(cx, |s, cx| {
                    let repo = s.repository(id).cloned()?;
                    // another entry has it already, or it moved meanwhile
                    if repo.path != missing_path
                        || s.repositories
                            .iter()
                            .any(|r| r.id != id && crate::dispatcher::same_path(&r.path, &workdir))
                    {
                        return None;
                    }
                    info!(id, from = %missing_path.display(), to = %workdir.display(), "following a moved repository");
                    s.repositories_store()
                        .update_repository_path(&repo, &workdir, main.as_deref(), false);
                    let rs = s.repo_state_mut(id);
                    rs.unsafe_path = None;
                    rs.error = None;
                    rs.worktrees.clear();
                    s.watchers.remove(&id);
                    cx.notify();
                    Some(repo.name())
                });
                match moved {
                    Some(name) => {
                        Self::set_banner(
                            crate::mco::Banner::RepositoryMoved {
                                name,
                                path: workdir,
                            },
                            cx,
                        );
                        Self::refresh_repository(id, cx);
                        Self::start_watching(id, cx);
                    }
                    None => Self::recover_missing_worktree(id, missing_path, cx),
                }
            },
        );
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bookmarks_round_trip_as_hex() {
        let bytes = vec![0u8, 1, 0xab, 0xff];
        assert_eq!(hex(&bytes), "0001abff");
        assert_eq!(unhex("0001abff"), Some(bytes));
        assert_eq!(unhex("abc"), None);
        assert_eq!(unhex("zz"), None);
    }

    #[test]
    fn trash_folders_are_not_followed() {
        assert!(in_trash(Path::new("/Users/me/.Trash/project")));
        assert!(in_trash(Path::new("/Volumes/Disk/.Trashes/501/project")));
        assert!(!in_trash(Path::new("/Users/me/Code/project")));
    }
}
