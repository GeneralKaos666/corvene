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
//!   [`exec_with`] / [`ExecOptions::env`] or [`git_command`]`.env(..)`, and
//!   GitHub Desktop's `process.env` around a call with
//!   `corvene_git::process::with_env` (every git command of the test's
//!   thread until it returns).
//! - The global git configuration (`$HOME/.gitconfig`) is shared by every
//!   test of the binary. A test that writes it, or depends on it being
//!   empty, holds [`lock_global_config`] for its whole body.
//! - No proxy: the proxy variables are cleared and `NO_PROXY` covers
//!   `127.0.0.1`, `localhost` and `::1`, where the [`http`] stub servers
//!   listen.
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
//! | `helpers/status.ts` | [`get_status_or_throw`] (also checks the in-process status, [`check_in_process_status`]) |
//! | `helpers/random-data.ts` | [`generate_string`], [`DEFAULT_STRING_LENGTH`] |
//! | `helpers/local-config.ts` | [`setup_local_config`] |
//! | `helpers/repository-builder-rebase-test.ts` | [`repository_builder_rebase::create_repository`] |
//! | `helpers/repository-builder-cherry-pick-test.ts` | [`repository_builder_cherry_pick::create_repository`] |
//! | `helpers/repository-builder-pull-test.ts` | [`repository_builder_pull::create_repository`] |
//! | `helpers/repository-builder-long-rebase-test.ts` | [`repository_builder_long_rebase::create_repository`] |
//! | `helpers/repository-builder-branch-pruner.ts` | [`repository_builder_branch_pruner::create_repository`] |
//! | `helpers/github-repo-builder.ts` | [`git_hub_repo_fixture`] with [`GitHubRepoFixtureOptions`], [`DOT_COM_API_ENDPOINT`] |
//!
//! # Shared beyond `app/test/helpers`
//!
//! Functions of GitHub Desktop's app and of Node that many test files call
//! for setup or checks, ported once here instead of in each test module:
//!
//! | GitHub Desktop | here |
//! | --- | --- |
//! | `lib/git/for-each-ref.ts` `getBranches` | [`get_branches`] |
//! | `lib/git/commit.ts` `createCommit` | [`create_commit`], [`try_create_commit`] |
//! | `lib/git/log.ts` `getCommits`, `getCommit` | [`get_commits`], [`get_commit`] |
//! | `lib/git/log.ts` `getChangedFiles` | [`get_changed_files`] (also checks the in-process files, [`check_in_process_changed_files`]) |
//! | `lib/git/core.ts` `GitError.message` | [`git_error_message`] |
//! | `lib/git/diff.ts` `getWorkingDirectoryDiff` | [`get_working_directory_diff`] |
//! | `GitStore.tip` after `loadStatus()` | [`load_tip`] |
//! | `unit/git/push-test.ts` `createBareUpstream` | [`create_bare_upstream`] |
//! | `new Repository(path, id, gitHubRepository, false)` | [`new_repository`] |
//! | `new WorkingDirectoryFileChange(path, { kind }, selection)` | [`working_directory_file_change`] |
//! | `WorkingDirectoryStatus.fromFiles` | [`from_files`] |
//! | `files.filter(f => f.status.kind === Conflicted).length` | [`conflicted_count`] |
//! | `DiffLine.text`, `DiffHunk.unifiedDiffEnd` | [`ghd_text`], [`unified_diff_end`] |
//! | Node `fs.writeFile`, `fs.appendFile` | [`write_file`], [`append_file`] |
//! | Node `Buffer.toString('base64')` | [`base64`] |
//! | `Date.parse`, `Date.prototype.toISOString` | [`date_parse`], [`to_iso_string`] |
//! | a `Response` / stubbed `fetch` for API code | [`http`]: [`serve`], [`serve_with`], [`unreachable_endpoint`] |
//! | dugite's bundled Git LFS | [`has_git_lfs`] |
//!
//! # Not ported here
//!
//! These helpers drive GitHub Desktop's stores, API, Electron or React
//! layers, whose Corvene equivalents live in crates this one cannot depend
//! on (`corvene-core` would pull GPUI into every test binary). They are
//! ported next to the tests that need them (in the crate that has the
//! equivalent code; `tests/ghd/*_support.rs` when several modules share
//! them):
//!
//! - `helpers/repository-builder-branch-pruner.ts` `setupRepository` /
//!   `primeCaches` (`RepositoriesStore`, `RepositoryStateCache`, `GitStore`),
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
pub mod github_repo_builder;
pub mod http;
pub mod lib_git;
pub mod local_config;
pub mod models;
pub mod node;
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
pub use exec::{ExecOptions, ExecResult, exec, exec_ok, exec_with, git, git_command, has_git_lfs};
pub use fixture::{fixtures_dir, get_fixture_path};
pub use git_helpers::{get_branch_or_error, get_ref_or_error, get_tip_or_error};
pub use github_repo_builder::{
    DOT_COM_API_ENDPOINT, GitHubRepoFixtureOptions, git_hub_repo_fixture, new_repository,
};
pub use http::{StubRequest, StubResponse, StubServer, serve, serve_with, unreachable_endpoint};
pub use lib_git::{
    create_bare_upstream, create_commit, get_branches, get_commit, get_commits,
    get_working_directory_diff, git_error_message, load_tip, try_create_commit,
};
pub use local_config::setup_local_config;
pub use models::{
    FileChanges, conflicted_count, from_files, ghd_text, unified_diff_end,
    working_directory_file_change,
};
pub use node::{append_file, base64, date_parse, to_iso_string, write_file};
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
pub use status::{
    check_in_process_changed_files, check_in_process_status, get_changed_files, get_status_or_throw,
};
pub use temp::create_temp_directory;
