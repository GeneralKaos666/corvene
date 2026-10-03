//! Port of GitHub Desktop's `app/test/unit/repositories-store-test.ts`.
//!
//! GitHub Desktop's `RepositoriesStore` (`lib/stores/repositories-store.ts`)
//! adds, reads and changes repository rows in IndexedDB
//! (`TestRepositoriesDatabase`). Corvene keeps the repository list in
//! `AppState::repositories`, saved whole by `corvene_store::Store`'s
//! `StoreExt::save_repositories` and read back by `StoreExt::repositories`;
//! `corvene_core::repositories_store::RepositoriesStore` borrows the list and
//! the store and makes every change (the dispatcher reaches it through
//! `AppState::repositories_store`).
//!
//! [`RepositoriesStore`] maps each method: `getAll` is
//! `StoreExt::repositories` on a store in a temporary directory (GitHub
//! Desktop's `new TestRepositoriesDatabase()` + `reset()`), the changes go
//! through Corvene's `RepositoriesStore` over the list read back from that
//! store, and `upsertGitHubRepository(endpoint, apiRepo)` is the API
//! conversion `corvene_github::Client::convert` (Corvene keeps no table of
//! GitHub repositories to upsert into). Corvene records no `gitDir` (its
//! missing-worktree recovery reads the main worktree path only), so the
//! `gitDir` arguments are not passed on. The case that compares GitHub
//! repository database ids is skipped (`tools/ghd-tests/skips/stores.tsv`):
//! Corvene embeds the GitHub repository in each `Repository` and has no such
//! id.
//!
//! The `apiRepo` object is deserialised from the same JSON fields as
//! GitHub Desktop's `IAPIFullRepository` literal.

use std::path::{Path, PathBuf};

use corvene_core::persistence::StoreExt;
use corvene_core::repositories_store::RepositoriesStore as CoreRepositoriesStore;
use corvene_core::{GitHubRepository, Repository};
use corvene_github::api::ApiRepository;
use corvene_github::{Client, Endpoint};
use corvene_store::Store;
use tempfile::TempDir;

/// GitHub Desktop's `RepositoriesStore` over an empty
/// `TestRepositoriesDatabase` (see the module docs).
struct RepositoriesStore {
    store: Store,
    _dir: TempDir,
}

impl RepositoriesStore {
    /// `beforeEach`: `repoDb = new TestRepositoriesDatabase(); await
    /// repoDb.reset(); repositoriesStore = new RepositoriesStore(repoDb)`.
    fn new() -> Self {
        let dir = corvene_test_support::create_temp_directory();
        let store = Store::open_in(dir.path()).expect("open the store");
        Self { store, _dir: dir }
    }

    /// Corvene's `RepositoriesStore` over the stored list.
    fn changes<T>(&self, change: impl FnOnce(&mut CoreRepositoriesStore<'_>) -> T) -> T {
        let mut repositories = self.get_all();
        change(&mut CoreRepositoriesStore::new(
            &self.store,
            &mut repositories,
        ))
    }

    /// `addRepository(path, gitDir)`.
    fn add_repository(&self, path: &str, _git_dir: &Path) -> Repository {
        self.changes(|store| store.add_repository(Path::new(path)))
    }

    /// `getAll()`.
    fn get_all(&self) -> Vec<Repository> {
        self.store.repositories().expect("read the repositories")
    }

    /// `upsertGitHubRepository(endpoint, apiRepo)`.
    fn upsert_github_repository(
        &self,
        endpoint: &str,
        api_repo: &ApiRepository,
    ) -> GitHubRepository {
        Client::new(Endpoint::from_api_base(endpoint), "").convert(api_repo.clone())
    }

    /// `setGitHubRepository(repository, gitHubRepository)`.
    fn set_github_repository(
        &self,
        repository: Repository,
        github_repository: GitHubRepository,
    ) -> Repository {
        self.changes(|store| store.set_github_repository(&repository, github_repository))
    }

    /// `switchWorktree(repository, worktreePath, isMissing, worktreeGitDir,
    /// mainWorktreePath)`: the main worktree path is kept when `None`.
    fn switch_worktree(
        &self,
        repository: &Repository,
        worktree_path: &str,
        is_missing: bool,
        _worktree_git_dir: &Path,
        main_worktree_path: Option<&str>,
    ) -> Repository {
        self.changes(|store| {
            store
                .switch_worktree(
                    repository,
                    Path::new(worktree_path),
                    is_missing,
                    main_worktree_path.map(Path::new),
                )
                .repository
        })
    }

    /// `updateRepositoryPath(repository, path, gitDir, mainWorktreePath,
    /// missing)`. (The last argument is GitHub Desktop's `missing`; its test
    /// passes `true` where the main worktree cannot be resolved, which
    /// `mainWorktreePath: undefined` already clears.)
    fn update_repository_path(
        &self,
        repository: &Repository,
        path: &str,
        _git_dir: Option<&Path>,
        main_worktree_path: Option<&str>,
        missing: bool,
    ) -> Repository {
        self.changes(|store| {
            store.update_repository_path(
                repository,
                Path::new(path),
                main_worktree_path.map(Path::new),
                missing,
            )
        })
    }
}

/// `join(path, …)`.
fn join(path: &str, rest: &str) -> PathBuf {
    Path::new(path).join(rest)
}

/// The `updating a GitHub repository` describe's `apiRepo`.
fn api_repo() -> ApiRepository {
    serde_json::from_value(serde_json::json!({
        "clone_url": "https://github.com/my-user/my-repo",
        "ssh_url": "git@github.com:my-user/my-repo.git",
        "html_url": "https://github.com/my-user/my-repo",
        "name": "my-repo",
        "owner": {
            "id": 42,
            "html_url": "https://github.com/my-user",
            "login": "my-user",
            "avatar_url": "https://github.com/my-user.png",
            "type": "User",
        },
        "private": true,
        "fork": false,
        "default_branch": "master",
        "pushed_at": "1995-12-17T03:24:00",
        "has_issues": true,
        "archived": false,
        "permissions": {
            "pull": true,
            "push": true,
            "admin": false,
        },
        "parent": null,
    }))
    .expect("IAPIFullRepository")
}

/// GitHub Desktop's `getDotComAPIEndpoint()`.
const ENDPOINT: &str = "https://api.github.com";

// GHD: unit/repositories-store-test.ts › RepositoriesStore › adding a new repository › contains the added repository
#[test]
fn contains_the_added_repository() {
    let repositories_store = RepositoriesStore::new();
    let repo_path = "/some/cool/path";
    repositories_store.add_repository(repo_path, &join(repo_path, ".git"));

    let repositories = repositories_store.get_all();
    assert_eq!(repositories[0].path, Path::new(repo_path));
}

// GHD: unit/repositories-store-test.ts › RepositoriesStore › getting all repositories › returns multiple repositories
#[test]
fn returns_multiple_repositories() {
    let repositories_store = RepositoriesStore::new();
    repositories_store.add_repository("/some/cool/path", Path::new("/some/cool/path/.git"));
    repositories_store.add_repository("/some/other/path", Path::new("/some/other/path/.git"));

    let repositories = repositories_store.get_all();
    assert_eq!(repositories.len(), 2);
}

// GHD: unit/repositories-store-test.ts › RepositoriesStore › updating a GitHub repository › adds a new GitHub repository
#[test]
fn adds_a_new_github_repository() {
    let repositories_store = RepositoriesStore::new();
    let api_repo = api_repo();
    repositories_store.set_github_repository(
        repositories_store.add_repository("/some/cool/path", Path::new("/some/cool/path/.git")),
        repositories_store.upsert_github_repository(ENDPOINT, &api_repo),
    );

    let repositories = repositories_store.get_all();
    let repo = &repositories[0];
    // assertIsRepositoryWithGitHubRepository(repo)
    let github_repository = repo.github.as_ref().expect("a GitHub repository");
    assert!(github_repository.private);
    assert!(!github_repository.fork);
    assert_eq!(
        github_repository.html_url,
        "https://github.com/my-user/my-repo"
    );
}

/// The `switching worktrees` describe's paths.
const MAIN_PATH: &str = "/some/cool/path";
const WORKTREE_PATH: &str = "/some/cool/path-wt-a";

fn worktree_git_dir() -> PathBuf {
    join(MAIN_PATH, ".git/worktrees/path-wt-a")
}

// GHD: unit/repositories-store-test.ts › RepositoriesStore › switching worktrees › persists the main worktree path
#[test]
fn persists_the_main_worktree_path() {
    let repositories_store = RepositoriesStore::new();
    let repository = repositories_store.add_repository(MAIN_PATH, &join(MAIN_PATH, ".git"));

    repositories_store.switch_worktree(
        &repository,
        WORKTREE_PATH,
        false,
        &worktree_git_dir(),
        Some(MAIN_PATH),
    );

    let reloaded = repositories_store.get_all().remove(0);
    assert_eq!(reloaded.path, Path::new(WORKTREE_PATH));
    assert_eq!(
        reloaded.main_worktree_path.as_deref(),
        Some(Path::new(MAIN_PATH))
    );
}

// GHD: unit/repositories-store-test.ts › RepositoriesStore › switching worktrees › keeps the main worktree path when switching between worktrees
#[test]
fn keeps_the_main_worktree_path_when_switching_between_worktrees() {
    let repositories_store = RepositoriesStore::new();
    let repository = repositories_store.add_repository(MAIN_PATH, &join(MAIN_PATH, ".git"));

    let on_worktree = repositories_store.switch_worktree(
        &repository,
        WORKTREE_PATH,
        false,
        &worktree_git_dir(),
        Some(MAIN_PATH),
    );

    // Switching on to a second worktree doesn't re-resolve the main worktree,
    // so it has to survive without being passed again.
    repositories_store.switch_worktree(
        &on_worktree,
        "/some/cool/path-wt-b",
        false,
        &join(MAIN_PATH, ".git/worktrees/path-wt-b"),
        None,
    );

    let reloaded = repositories_store.get_all().remove(0);
    assert_eq!(
        reloaded.main_worktree_path.as_deref(),
        Some(Path::new(MAIN_PATH))
    );
}

/// The `relocating a repository` describe's `onWorktree()`.
fn on_worktree(repositories_store: &RepositoriesStore) -> Repository {
    let repository = repositories_store.add_repository(MAIN_PATH, &join(MAIN_PATH, ".git"));

    repositories_store.switch_worktree(
        &repository,
        WORKTREE_PATH,
        false,
        &worktree_git_dir(),
        Some(MAIN_PATH),
    )
}

// GHD: unit/repositories-store-test.ts › RepositoriesStore › relocating a repository › updates the main worktree path
#[test]
fn updates_the_main_worktree_path() {
    let repositories_store = RepositoriesStore::new();
    // Relocating moves the whole repository, so the previously recorded main
    // worktree no longer exists where it used to.
    let moved_main = "/moved/path";
    let moved_worktree = "/moved/path-wt-a";

    repositories_store.update_repository_path(
        &on_worktree(&repositories_store),
        moved_worktree,
        Some(&join(moved_main, ".git/worktrees/path-wt-a")),
        Some(moved_main),
        false,
    );

    let reloaded = repositories_store.get_all().remove(0);
    assert_eq!(reloaded.path, Path::new(moved_worktree));
    assert_eq!(
        reloaded.main_worktree_path.as_deref(),
        Some(Path::new(moved_main))
    );
}

// GHD: unit/repositories-store-test.ts › RepositoriesStore › relocating a repository › clears the main worktree path when it cannot be resolved
#[test]
fn clears_the_main_worktree_path_when_it_cannot_be_resolved() {
    let repositories_store = RepositoriesStore::new();
    // Better to fall back to the git dir lookup than to keep pointing at a
    // location the repository has moved away from.
    repositories_store.update_repository_path(
        &on_worktree(&repositories_store),
        "/moved/path-wt-a",
        None,
        None,
        true,
    );

    let reloaded = repositories_store.get_all().remove(0);
    assert_eq!(reloaded.main_worktree_path, None);
}
