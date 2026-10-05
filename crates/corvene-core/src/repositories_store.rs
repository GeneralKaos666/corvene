//! GHD `RepositoriesStore` (`app/src/lib/stores/repositories-store.ts`):
//! the changes to the repository list, each saved at once.
//!
//! Corvene keeps the list in `AppState::repositories` and saves it whole
//! through `StoreExt::save_repositories`; [`RepositoriesStore`] borrows the
//! list and the store (`AppState::repositories_store`) so the dispatcher
//! (`add_repository_then`, `switch_worktree`, `relocate_repository`, the
//! GitHub repository refreshes) changes it in one place, without gpui.
//!
//! Differences from GHD:
//!
//! - There is no `gitDir` column: GHD records a repository's git directory
//!   as the anchor `resolveMainWorktreePath` falls back to; Corvene's
//!   missing-worktree recovery reads only `Repository::main_worktree_path`,
//!   which it also records on every refresh (`crate::worktrees`).
//! - There is no table of GitHub repositories: each `Repository` embeds its
//!   `GitHubRepository`, so [`RepositoriesStore::set_github_repository`]
//!   stores the value itself. The branch pruner's last prune date, which GHD
//!   keeps in that table, is stored per GitHub repository (endpoint, owner
//!   and name) under its own key ([`last_prune_date`]).
//! - A path matches an existing entry when both name the same directory
//!   (`dispatcher::same_path`); GHD compares the strings.

use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use corvene_models::{GitHubRepository, Repository};
use corvene_store::Store;
use tracing::{error, warn};

use crate::dispatcher::same_path;
use crate::persistence::StoreExt;

/// GHD `switchWorktree`'s result.
#[derive(Clone, Debug, PartialEq)]
pub struct SwitchWorktreeResult {
    pub repository: Repository,
    /// Another entry already has the worktree's path; it is returned and
    /// nothing was changed.
    pub existing_repository: bool,
}

/// GHD `RepositoriesStore` over `AppState::repositories` and its store.
pub struct RepositoriesStore<'a> {
    store: &'a Store,
    repositories: &'a mut Vec<Repository>,
}

impl<'a> RepositoriesStore<'a> {
    pub fn new(store: &'a Store, repositories: &'a mut Vec<Repository>) -> Self {
        Self {
            store,
            repositories,
        }
    }

    /// GHD `getAll`.
    pub fn get_all(&self) -> &[Repository] {
        self.repositories
    }

    /// The entry for the repository at `path`, if any.
    pub fn find_by_path(&self, path: &Path) -> Option<&Repository> {
        self.repositories.iter().find(|r| same_path(&r.path, path))
    }

    /// GHD `addRepository(path, gitDir)`: a new entry at `path` with the
    /// next repository id, or the existing entry for that path.
    pub fn add_repository(&mut self, path: &Path) -> Repository {
        if let Some(existing) = self.find_by_path(path) {
            return existing.clone();
        }
        let id = self.store.next_repository_id().unwrap_or_else(|err| {
            warn!(%err, "could not read the next repository id");
            self.repositories.iter().map(|r| r.id).max().unwrap_or(0) + 1
        });
        let repository = Repository::new(id, path);
        self.repositories.push(repository.clone());
        self.persist();
        repository
    }

    /// GHD `setGitHubRepository(repository, gitHubRepository)`: associate the
    /// GitHub repository (nothing is written when it is already the one) and
    /// return the updated entry.
    pub fn set_github_repository(
        &mut self,
        repository: &Repository,
        github: GitHubRepository,
    ) -> Repository {
        self.update(repository, |repo| {
            if repo.github.as_ref() == Some(&github) {
                return false;
            }
            repo.github = Some(github);
            true
        })
    }

    /// GHD `switchWorktree(repository, worktreePath, missing, gitDir,
    /// mainWorktreePath)`: point the entry at another worktree of the same
    /// repository. `main_worktree_path` `None` keeps the recorded one (a
    /// switch between linked worktrees does not resolve it again). When
    /// another entry already has `worktree_path` it is returned unchanged.
    pub fn switch_worktree(
        &mut self,
        repository: &Repository,
        worktree_path: &Path,
        missing: bool,
        main_worktree_path: Option<&Path>,
    ) -> SwitchWorktreeResult {
        if let Some(existing) = self
            .find_by_path(worktree_path)
            .filter(|r| r.id != repository.id)
        {
            return SwitchWorktreeResult {
                repository: existing.clone(),
                existing_repository: true,
            };
        }
        let repository = self.update(repository, |repo| {
            repo.path = worktree_path.to_path_buf();
            repo.missing = missing;
            if let Some(main) = main_worktree_path {
                repo.main_worktree_path = Some(main.to_path_buf());
            }
            true
        });
        SwitchWorktreeResult {
            repository,
            existing_repository: false,
        }
    }

    /// GHD `updateRepositoryPath(repository, path, gitDir, mainWorktreePath,
    /// missing)`: the repository moved. Unlike [`Self::switch_worktree`] the
    /// main worktree is always replaced: moving invalidates the recorded
    /// one, so `None` (it cannot be resolved) clears it.
    pub fn update_repository_path(
        &mut self,
        repository: &Repository,
        path: &Path,
        main_worktree_path: Option<&Path>,
        missing: bool,
    ) -> Repository {
        self.update(repository, |repo| {
            repo.path = path.to_path_buf();
            repo.missing = missing;
            repo.main_worktree_path = main_worktree_path.map(Path::to_path_buf);
            true
        })
    }

    /// Apply `edit` to the entry with `repository`'s id and save the list
    /// when it reports a change; the updated entry (or `repository` itself
    /// when it is not in the list).
    fn update(
        &mut self,
        repository: &Repository,
        edit: impl FnOnce(&mut Repository) -> bool,
    ) -> Repository {
        let Some(repo) = self.repositories.iter_mut().find(|r| r.id == repository.id) else {
            let mut detached = repository.clone();
            edit(&mut detached);
            return detached;
        };
        let changed = edit(repo);
        let updated = repo.clone();
        if changed {
            self.persist();
        }
        updated
    }

    fn persist(&self) {
        if let Err(err) = self.store.save_repositories(self.repositories) {
            error!(?err, "could not save repositories");
        }
    }
}

/// The store key of the last prune dates (GHD `IDatabaseGitHubRepository
/// .lastPruneDate`): GitHub repository key to milliseconds since the epoch.
const LAST_PRUNE_DATES_KEY: &str = "repositories.lastPruneDates";

/// What identifies a GitHub repository across local clones of it (GHD's
/// `gitHubRepositories` row): its API endpoint, owner and name.
fn github_key(github: &GitHubRepository) -> String {
    format!(
        "{}/{}/{}",
        github.endpoint.trim_end_matches('/'),
        github.owner,
        github.name
    )
    .to_lowercase()
}

fn last_prune_dates(store: &Store) -> HashMap<String, u64> {
    store
        .get(LAST_PRUNE_DATES_KEY)
        .unwrap_or_else(|err| {
            warn!(%err, "could not read the last prune dates");
            None
        })
        .unwrap_or_default()
}

/// GHD `getLastPruneDate(repository)`: when the branch pruner last ran for
/// this GitHub repository.
pub fn last_prune_date(store: &Store, github: &GitHubRepository) -> Option<SystemTime> {
    last_prune_dates(store)
        .get(&github_key(github))
        .map(|ms| UNIX_EPOCH + Duration::from_millis(*ms))
}

/// GHD `updateLastPruneDate(repository, date)`.
pub fn update_last_prune_date(store: &Store, github: &GitHubRepository, date: SystemTime) {
    let mut dates = last_prune_dates(store);
    let ms = date
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0);
    dates.insert(github_key(github), ms);
    if let Err(err) = store.set(LAST_PRUNE_DATES_KEY, &dates) {
        error!(?err, "could not save the last prune date");
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn github(name: &str) -> GitHubRepository {
        GitHubRepository {
            endpoint: "https://api.github.com".into(),
            owner: "desktop".into(),
            name: name.into(),
            html_url: String::new(),
            clone_url: String::new(),
            default_branch: None,
            private: false,
            fork: false,
            parent: None,
            archived: false,
            permissions: None,
            allow_forking: None,
            node_id: None,
        }
    }

    #[test]
    fn adding_an_existing_path_returns_the_entry() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open_in(dir.path()).unwrap();
        let mut repositories = Vec::new();
        let mut repos = RepositoriesStore::new(&store, &mut repositories);
        let first = repos.add_repository(Path::new("/some/path"));
        let again = repos.add_repository(Path::new("/some/path"));
        assert_eq!(first, again);
        assert_eq!(store.repositories().unwrap().len(), 1);
    }

    #[test]
    fn switching_onto_another_entry_returns_it() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open_in(dir.path()).unwrap();
        let mut repositories = Vec::new();
        let mut repos = RepositoriesStore::new(&store, &mut repositories);
        let main = repos.add_repository(Path::new("/r"));
        let other = repos.add_repository(Path::new("/r-wt"));
        let result = repos.switch_worktree(&main, Path::new("/r-wt"), false, None);
        assert!(result.existing_repository);
        assert_eq!(result.repository, other);
        assert_eq!(repos.get_all()[0].path, PathBuf::from("/r"));
    }

    #[test]
    fn prune_dates_are_kept_per_github_repository() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open_in(dir.path()).unwrap();
        assert_eq!(last_prune_date(&store, &github("a")), None);
        let at = UNIX_EPOCH + Duration::from_millis(1_700_000_000_123);
        update_last_prune_date(&store, &github("a"), at);
        assert_eq!(last_prune_date(&store, &github("a")), Some(at));
        assert_eq!(last_prune_date(&store, &github("b")), None);
    }
}
