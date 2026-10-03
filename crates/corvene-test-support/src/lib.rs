//! Test-only support for Corvene's port of GitHub Desktop's unit tests
//! (`app/test` of desktop/desktop 3.6.6): the fixtures, the environment
//! and the helpers of `app/test/helpers`, under the same names in snake
//! case. Use it as a dev-dependency only. Helpers panic (with a message)
//! instead of returning errors: a failed setup is a failed test.
//!
//! How the ported tests are laid out, marked and checked is in
//! `tools/ghd-tests/README.md`.
//!
//! # Environment
//!
//! Every test binary that links this crate runs with an isolated git
//! environment, set up before `main` ([`env`]): `HOME` in an empty
//! temporary directory (no `XDG_CONFIG_HOME`), no system configuration, no
//! inherited `GIT_*` variables, `TERM=dumb`, and the commit identity `Joe
//! Bloggs <joe.bloggs@somewhere.com>` ([`AUTHOR_NAME`], [`AUTHOR_EMAIL`]). The
//! developer's own `~/.gitconfig` (signing, hooks, aliases, templates) is
//! never read.
//!
//! - Call [`init`] at the top of a test that runs git (or `gix`) without
//!   using any helper first. The helpers call it themselves.
//! - Never call `std::env::set_var` / `remove_var` in a test: tests run on
//!   parallel threads. Pass per-invocation variables with
//!   [`exec_with`] / [`ExecOptions::env`] or [`git_command`]`.env(..)`.
//! - The global git configuration (`$HOME/.gitconfig`) is shared by every
//!   test of the binary. A test that writes it, or depends on it being
//!   empty, holds [`lock_global_config`] for its whole body.
//!
//! # Map of GitHub Desktop's helpers
//!
//! | GitHub Desktop (`app/test/…`) | here |
//! | --- | --- |
//! | `unit-test-env.ts` | [`env`]: [`init`], [`home_dir`], [`lock_global_config`] |
//! | dugite `exec` | [`exec`], [`exec_with`], [`ExecResult`], [`ExecOptions`] |
//! | `lib/git/core.ts` `git()` in helpers | [`exec_ok`] |
//! | the bundled git | [`git()`], [`git_command`] |
//! | `helpers/temp.ts` | [`create_temp_directory`] |
//! | `helpers/fixture.ts` | [`get_fixture_path`], [`fixtures_dir`] |
//! | `helpers/repositories.ts` | [`setup_fixture_repository`], [`setup_empty_repository`], [`setup_empty_repository_with_default_branch`], [`setup_empty_repository_default_main`], [`setup_empty_directory`], [`setup_conflicted_repo`], [`setup_conflicted_repo_with_unrelated_committed_change`], [`setup_conflicted_repo_with_multiple_files`], [`setup_two_commit_repo`], [`setup_local_fork_of_repository`], [`setup_repository_with_uninitialized_submodule`] |
//! | `Repository` model returned by them | [`TestRepo`] |
//! | `lib/git/description.ts` `DefaultGitDescription` | [`DEFAULT_GIT_DESCRIPTION`] |
//! | `helpers/repository-scaffolding.ts` | [`make_commit`] with [`Tree`] / [`TreeEntry`], [`create_branch`], [`switch_to`], [`clone_repository`], [`clone_local_repository`] |
//! | `helpers/git.ts` | [`get_tip_or_error`], [`get_ref_or_error`], [`get_branch_or_error`] |
//! | `helpers/status.ts` | [`get_status_or_throw`] |
//! | `helpers/random-data.ts` | [`generate_string`], [`DEFAULT_STRING_LENGTH`] |
//! | `helpers/local-config.ts` | [`setup_local_config`] |
//! | `helpers/repository-builder-rebase-test.ts` | [`repository_builder_rebase::create_repository`] |
//! | `helpers/repository-builder-cherry-pick-test.ts` | [`repository_builder_cherry_pick::create_repository`] |
//! | `helpers/repository-builder-pull-test.ts` | [`repository_builder_pull::create_repository`] |
//! | `helpers/repository-builder-long-rebase-test.ts` | [`repository_builder_long_rebase::create_repository`] |
//! | `helpers/repository-builder-branch-pruner.ts` | [`repository_builder_branch_pruner::create_repository`] |
//!
//! # Not ported here
//!
//! These helpers drive GitHub Desktop's stores, API, Electron or React
//! layers. The lane that ports the tests needing them ports them next to
//! those tests (in the crate that has the equivalent code):
//!
//! - `helpers/repository-builder-branch-pruner.ts` `setupRepository` /
//!   `primeCaches` (`RepositoriesStore`, `RepositoryStateCache`, `GitStore`),
//! - `helpers/github-repo-builder.ts` (`GitHubRepository` models),
//! - `helpers/mock-api.ts`, `helpers/app-store-test-harness.ts`,
//!   `helpers/in-memory-dispatcher.ts`, `helpers/changes-state-helper.ts`,
//! - `helpers/stores/*` (in-memory stores), `helpers/databases/*` (Dexie
//!   test databases), `helpers/test-stats-store.ts`,
//!   `helpers/test-activity-monitor.ts`, `helpers/test-app-shell.ts`,
//! - `helpers/mock-ipc.ts`, `helpers/menus/*`, `helpers/ui/*` (Electron and
//!   React rendering).

pub mod env;
pub mod exec;
pub mod fixture;
pub mod git_helpers;
pub mod local_config;
pub mod random_data;
pub mod repositories;
pub mod repository_builder_branch_pruner;
pub mod repository_builder_cherry_pick;
pub mod repository_builder_long_rebase;
pub mod repository_builder_pull;
pub mod repository_builder_rebase;
pub mod repository_scaffolding;
pub mod status;
pub mod temp;

pub use env::{
    AUTHOR_EMAIL, AUTHOR_NAME, GlobalConfigGuard, home_dir, init, isolated_before_main,
    lock_global_config,
};
pub use exec::{ExecOptions, ExecResult, exec, exec_ok, exec_with, git, git_command};
pub use fixture::{fixtures_dir, get_fixture_path};
pub use git_helpers::{get_branch_or_error, get_ref_or_error, get_tip_or_error};
pub use local_config::setup_local_config;
pub use random_data::{DEFAULT_STRING_LENGTH, generate_string};
pub use repositories::{
    DEFAULT_GIT_DESCRIPTION, TestRepo, setup_conflicted_repo,
    setup_conflicted_repo_with_multiple_files,
    setup_conflicted_repo_with_unrelated_committed_change, setup_empty_directory,
    setup_empty_repository, setup_empty_repository_default_main,
    setup_empty_repository_with_default_branch, setup_fixture_repository,
    setup_local_fork_of_repository, setup_repository_with_uninitialized_submodule,
    setup_two_commit_repo,
};
pub use repository_scaffolding::{
    Tree, TreeEntry, clone_local_repository, clone_repository, create_branch, make_commit,
    switch_to,
};
pub use status::get_status_or_throw;
pub use temp::create_temp_directory;
