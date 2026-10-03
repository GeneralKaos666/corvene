//! Helpers shared by the `history` lane's `corvene-git` modules (stash,
//! tag, log, worktree, submodule).

use std::path::Path;

use corvene_git::{CommitOptions, GitError};
use corvene_models::{Branch, Commit, WorkingDirectoryFileChange};
use corvene_test_support::{TestRepo, git};

/// GitHub Desktop's `getBranches(repository, ...prefixes)`
/// (`lib/git/for-each-ref.ts`): the local and remote branches whose ref
/// matches one of the prefixes as a `for-each-ref` pattern does (the whole
/// ref name, or its leading components).
///
/// Corvene reads branches with `corvene_git::open_repository`
/// (`RepositoryInfo::branches`), as `corvene-test-support`'s
/// `get_branch_or_error` does.
pub fn get_branches(repository: &Path, prefixes: &[&str]) -> Vec<Branch> {
    let info = corvene_git::open_repository(repository)
        .unwrap_or_else(|err| panic!("getBranches in {}: {err}", repository.display()));
    info.branches
        .into_iter()
        .filter(|b| {
            prefixes.is_empty()
                || prefixes.iter().any(|p| {
                    let p = p.trim_end_matches('/');
                    b.full_name == p
                        || b.full_name
                            .strip_prefix(p)
                            .is_some_and(|rest| rest.starts_with('/'))
                })
        })
        .collect()
}

/// GitHub Desktop's `createCommit(repository, message, files)`
/// (`lib/git/commit.ts`): `unstageAll`, `stageFiles(files)`, `git commit -F
/// -`, returning the new commit's sha. Corvene's `Dispatcher::commit` makes
/// the same `corvene_git` calls (`unstage_all`, `stage_files`,
/// `stage_partial_files`, `commit`).
///
/// # Panics
///
/// When any of the git calls fails (GitHub Desktop's promise rejects).
pub fn create_commit(
    repository: &TestRepo,
    message: &str,
    files: &[WorkingDirectoryFileChange],
) -> String {
    let path = repository.path();
    let run = || -> Result<String, GitError> {
        corvene_git::unstage_all(git(), path)?;
        corvene_git::stage_files(git(), path, files)?;
        corvene_git::stage_partial_files(git(), path, files)?;
        corvene_git::commit(git(), path, message, &CommitOptions::default())
    };
    run().unwrap_or_else(|err| panic!("createCommit in {}: {err}", path.display()))
}

/// GitHub Desktop's `getCommit(repository, ref)` (`lib/git/log.ts`): the
/// first commit `getCommits(repository, ref, 1)` returns, Corvene's
/// `corvene_git::get_commits(path, ref, 0, 1)`.
pub fn get_commit(repository: &TestRepo, reference: &str) -> Option<Commit> {
    corvene_git::get_commits(repository.path(), reference, 0, 1)
        .unwrap_or_else(|err| panic!("getCommit {reference}: {err}"))
        .into_iter()
        .next()
}

/// What GitHub Desktop's `GitError.message` (`lib/git/core.ts`) holds for a
/// failed git call: the description of an error dugite recognises, else
/// git's output.
pub fn git_error_message(err: &GitError) -> String {
    match err.failure() {
        Some(failure) => failure
            .description("Settings")
            .unwrap_or_else(|| failure.output.clone()),
        None => err.to_string(),
    }
}
