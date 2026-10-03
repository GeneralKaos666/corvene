//! Port of `createRepository` from
//! `app/test/helpers/repository-builder-branch-pruner.ts`. Its
//! `setupRepository` (and the private `primeCaches`) fill GitHub Desktop's
//! `RepositoriesStore` / `RepositoryStateCache` and belong to the
//! store-level lane that ports the branch pruner tests.

use crate::exec::exec;
use crate::repositories::{TestRepo, setup_empty_repository};
use crate::repository_scaffolding::{Tree, TreeEntry, make_commit, switch_to};

/// GitHub Desktop's `createRepository(t)` for branch pruner tests (it
/// returns the path; use [`TestRepo::path`]): `master` with a `--no-ff`
/// merge of `other-branch` (two commits adding `bar` and `baz`, after `foo`
/// changed on `master`), and an empty reflog so every branch is a pruning
/// candidate.
pub fn create_repository() -> TestRepo {
    let repo = setup_empty_repository();

    make_commit(
        &repo,
        &Tree::new([
            TreeEntry::new("foo", ""),
            TreeEntry::new("perlin", "perlin"),
        ]),
    );

    // creating the new branch before switching so that we have distinct
    // changes on both branches and also to ensure a merge commit is needed
    exec(["branch", "other-branch"], repo.path());

    make_commit(&repo, &Tree::new([TreeEntry::new("foo", "b1")]));

    switch_to(&repo, "other-branch");

    make_commit(&repo, &Tree::new([TreeEntry::new("bar", "b2")]));
    make_commit(
        &repo,
        &Tree::new([TreeEntry::new("baz", "very much more words")]),
    );

    switch_to(&repo, "master");

    // ensure the merge operation always creates a merge commit
    exec(["merge", "other-branch", "--no-ff"], repo.path());

    // clear reflog of all entries, so any branches are considered
    // candidates for pruning
    exec(
        [
            "reflog",
            "expire",
            "--expire=now",
            "--expire-unreachable=now",
            "--all",
        ],
        repo.path(),
    );

    repo
}
