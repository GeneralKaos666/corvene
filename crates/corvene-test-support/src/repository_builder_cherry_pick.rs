//! Port of `app/test/helpers/repository-builder-cherry-pick-test.ts`.

use crate::repositories::{TestRepo, setup_empty_repository_default_main};
use crate::repository_scaffolding::{Tree, TreeEntry, create_branch, make_commit, switch_to};

/// GitHub Desktop's `createRepository(t, featureBranchName,
/// targetBranchName)` for cherry-pick tests: one commit `First!` on `main`,
/// one commit `Cherry-picked Feature!` (adding `THING.md`) on
/// `feature_branch_name`, and `target_branch_name` created from `main` and
/// checked out.
pub fn create_repository(feature_branch_name: &str, target_branch_name: &str) -> TestRepo {
    let repository = setup_empty_repository_default_main();

    make_commit(
        &repository,
        &Tree::with_message("First!", [TreeEntry::new("README.md", "# HELLO WORLD! \n")]),
    );

    create_branch(&repository, feature_branch_name, "HEAD");
    create_branch(&repository, target_branch_name, "HEAD");

    switch_to(&repository, feature_branch_name);
    make_commit(
        &repository,
        &Tree::with_message(
            "Cherry-picked Feature!",
            [TreeEntry::new(
                "THING.md",
                "# HELLO WORLD! \nTHINGS GO HERE\n",
            )],
        ),
    );

    switch_to(&repository, target_branch_name);
    repository
}
