//! Port of `app/test/helpers/repository-builder-rebase-test.ts`.

use crate::repositories::{TestRepo, setup_empty_repository};
use crate::repository_scaffolding::{Tree, TreeEntry, create_branch, make_commit, switch_to};

/// GitHub Desktop's `createRepository(t, firstBranchName, secondBranchName)`
/// for rebase tests: two commits on `master` (`First!`, `Second!`), one
/// commit `Base Branch!` on `first_branch_name` and one commit
/// `Feature Branch!` on `second_branch_name`, both based on `master`, which
/// is checked out. The two branch commits conflict in `THING.md` and
/// `OTHER.md`.
pub fn create_repository(first_branch_name: &str, second_branch_name: &str) -> TestRepo {
    let repository = setup_empty_repository();

    // make two commits on `master` to setup the README
    make_commit(
        &repository,
        &Tree::with_message("First!", [TreeEntry::new("README.md", "# HELLO WORLD! \n")]),
    );
    make_commit(
        &repository,
        &Tree::with_message(
            "Second!",
            [
                TreeEntry::new("THING.md", "# HELLO WORLD! \nTHINGS GO HERE\n"),
                TreeEntry::new("OTHER.md", "# HELLO WORLD! \nTHINGS GO HERE\n"),
                TreeEntry::new("THIRD.md", "nothing goes here"),
            ],
        ),
    );

    create_branch(&repository, first_branch_name, "HEAD");
    create_branch(&repository, second_branch_name, "HEAD");

    switch_to(&repository, first_branch_name);
    make_commit(
        &repository,
        &Tree::with_message(
            "Base Branch!",
            [
                TreeEntry::new(
                    "THING.md",
                    "# HELLO WORLD! \nTHINGS GO HERE\nBASE BRANCH UNDERWAY\n",
                ),
                TreeEntry::new(
                    "OTHER.md",
                    "# HELLO WORLD! \nTHINGS GO HERE\nALSO BASE BRANCH UNDERWAY\n",
                ),
            ],
        ),
    );

    switch_to(&repository, second_branch_name);
    make_commit(
        &repository,
        &Tree::with_message(
            "Feature Branch!",
            [
                TreeEntry::new(
                    "THING.md",
                    "# HELLO WORLD! \nTHINGS GO HERE\nFEATURE BRANCH UNDERWAY\n",
                ),
                TreeEntry::new(
                    "OTHER.md",
                    "# HELLO WORLD! \nTHINGS GO HERE\nALSO FEATURE BRANCH UNDERWAY\n",
                ),
            ],
        ),
    );

    // put the repository back on the default branch
    switch_to(&repository, "master");
    repository
}
