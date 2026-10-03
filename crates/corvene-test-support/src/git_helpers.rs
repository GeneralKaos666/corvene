//! Port of `app/test/helpers/git.ts`.

use corvene_models::{Branch, BranchKind, Commit};

use crate::repositories::TestRepo;

/// GitHub Desktop's `getTipOrError(repository)`: the commit `HEAD` points
/// at (`getCommit(repository, 'HEAD')`, Corvene's `corvene_git::get_commits`).
///
/// # Panics
///
/// When the repository is unborn.
pub fn get_tip_or_error(repository: &TestRepo) -> Commit {
    first_commit(repository, "HEAD").unwrap_or_else(|| {
        panic!(
            "Unable to find commit for HEAD - check that you are not working with an unborn repository"
        )
    })
}

/// GitHub Desktop's `getRefOrError(repository, ref)`: the commit a ref, a
/// branch name or a commit id resolves to.
///
/// # Panics
///
/// When `reference` resolves to no commit.
pub fn get_ref_or_error(repository: &TestRepo, reference: &str) -> Commit {
    first_commit(repository, reference).unwrap_or_else(|| {
        panic!("Unable to find commit for {reference} - check that this exists in the repository")
    })
}

/// GitHub Desktop's `getBranchOrError(repository, name)`: the local branch
/// `refs/heads/<name>` (`getBranches(repository, ref)`, Corvene's
/// `corvene_git::open_repository(..).branches`).
///
/// # Panics
///
/// When there is no such local branch.
pub fn get_branch_or_error(repository: &TestRepo, name: &str) -> Branch {
    let reference = format!("refs/heads/{name}");
    corvene_git::open_repository(repository.path())
        .unwrap_or_else(|err| panic!("open {}: {err}", repository.path().display()))
        .branches
        .into_iter()
        .find(|b| b.kind == BranchKind::Local && b.full_name == reference)
        .unwrap_or_else(|| {
            panic!(
                "Unable to find branch matching {reference} - check that this exists in the repository"
            )
        })
}

fn first_commit(repository: &TestRepo, revision: &str) -> Option<Commit> {
    corvene_git::get_commits(repository.path(), revision, 0, 1)
        .unwrap_or_else(|err| panic!("read {revision} in {}: {err}", repository.path().display()))
        .into_iter()
        .next()
}
