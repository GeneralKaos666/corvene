//! Helpers shared by the `refs` lane's `corvene-git` modules.

use std::path::Path;

use corvene_git::GitError;
use corvene_models::Branch;

/// GitHub Desktop's `getBranches(repository, ...prefixes)`
/// (`lib/git/for-each-ref.ts`): `git for-each-ref` over `refs/heads` and
/// `refs/remotes` (or the given prefixes), symbolic refs left out, and `[]`
/// when git reports `NotAGitRepository`.
///
/// Corvene reads the same branches with `corvene_git::open_repository`
/// (`RepositoryInfo::branches`, which leaves out `<remote>/HEAD`), as
/// `corvene-test-support`'s `get_branch_or_error` does; its
/// `GitError::NotARepository` is GitHub Desktop's `NotAGitRepository`. A
/// prefix matches as a `for-each-ref` pattern does: the whole ref name, or
/// its leading components.
pub fn get_branches(repository: &Path, prefixes: &[&str]) -> Vec<Branch> {
    let branches = match corvene_git::open_repository(repository) {
        Ok(info) => info.branches,
        Err(GitError::NotARepository(_)) => return Vec::new(),
        Err(err) => panic!("getBranches in {}: {err}", repository.display()),
    };
    branches
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
