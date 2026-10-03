//! Port of `app/test/helpers/repository-builder-long-rebase-test.ts`.

use crate::repositories::{TestRepo, setup_empty_repository};
use crate::repository_scaffolding::{Tree, TreeEntry, create_branch, make_commit, switch_to};

/// GitHub Desktop's `createRepository(t, firstBranchName, secondBranchName)`
/// for long rebase tests: two commits on `master` (`First!`, `Second!`),
/// three commits on `first_branch_name` and ten on `second_branch_name`,
/// both based on `master`, which is checked out. The seventh feature commit
/// is (as in GitHub Desktop) also titled `Feature Branch Third Commit!`.
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
            "Base Branch First Commit!",
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
    make_commit(
        &repository,
        &Tree::with_message(
            "Base Branch Second Commit!",
            [
                TreeEntry::new(
                    "THING.md",
                    "# HELLO WORLD! \nTHINGS GO HERE\nMAKING BRANCH CHANGES\n",
                ),
                TreeEntry::new(
                    "OTHER.md",
                    "# HELLO WORLD! \nTHINGS GO HERE\nMORE BASE BRANCH CHANGES HAPPENING\n",
                ),
            ],
        ),
    );
    make_commit(
        &repository,
        &Tree::with_message(
            "Base Branch Third Commit!",
            [
                TreeEntry::new(
                    "THING.md",
                    "# HELLO WORLD! \nWORDS IN HERE\nMAKING BRANCH CHANGES\n",
                ),
                TreeEntry::new(
                    "OTHER.md",
                    "# HELLO WORLD! \nALSO WORDS GO IN HERE\nMORE BASE BRANCH CHANGES HAPPENING\n",
                ),
            ],
        ),
    );

    switch_to(&repository, second_branch_name);
    make_commit(
        &repository,
        &Tree::with_message(
            "Feature Branch First Commit!",
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
    make_commit(
        &repository,
        &Tree::with_message(
            "Feature Branch Second Commit!",
            [
                TreeEntry::new(
                    "THING.md",
                    "# HELLO WORLD! \nTHINGS GO HERE\nALSO FEATURE BRANCH CHANGES\n",
                ),
                TreeEntry::new(
                    "OTHER.md",
                    "# HELLO WORLD! \nTHINGS GO HERE\nYES FEATURE BRANCH HERE TOO\n",
                ),
            ],
        ),
    );
    make_commit(
        &repository,
        &Tree::with_message(
            "Feature Branch Third Commit!",
            [
                TreeEntry::new(
                    "THING.md",
                    "# HELLO WORLD! \nCHAAAAANGE HERE\nALSO FEATURE BRANCH CHANGES\n",
                ),
                TreeEntry::new(
                    "OTHER.md",
                    "# HELLO WORLD! \nCHAAAAANGE GO HERE\nYES FEATURE BRANCH HERE TOO\n",
                ),
            ],
        ),
    );
    make_commit(
        &repository,
        &Tree::with_message(
            "Feature Branch Fourth Commit!",
            [
                TreeEntry::new(
                    "THING.md",
                    "# HELLO WORLD! \nCHANGES GO HERE\nALSO FEATURE BRANCH CHANGES\n",
                ),
                TreeEntry::new(
                    "OTHER.md",
                    "# HELLO WORLD! \nCHANGE ALSO GO HERE TOO\nYES FEATURE BRANCH HERE TOO\n",
                ),
            ],
        ),
    );
    make_commit(
        &repository,
        &Tree::with_message(
            "Feature Branch Fifth Commit!",
            [
                TreeEntry::new(
                    "THING.md",
                    "# SO LONG AND THANKS FOR THE FISH! \nTHINGS GO HERE\nALSO FEATURE BRANCH CHANGES\n",
                ),
                TreeEntry::new(
                    "OTHER.md",
                    "# IS IT FRIDAY YET? \nTHINGS GO HERE\nYES FEATURE BRANCH HERE TOO\n",
                ),
            ],
        ),
    );
    make_commit(
        &repository,
        &Tree::with_message(
            "Feature Branch Sixth Commit!",
            [
                TreeEntry::new(
                    "THING.md",
                    "# HELLO WORLD! \nWORDS WORDS WORDS\nALSO FEATURE BRANCH CHANGES\n",
                ),
                TreeEntry::new(
                    "OTHER.md",
                    "# HELLO WORLD! WORDS WORDS WORDS\nYES FEATURE BRANCH HERE TOO\n",
                ),
            ],
        ),
    );
    make_commit(
        &repository,
        &Tree::with_message(
            "Feature Branch Third Commit!",
            [
                TreeEntry::new(
                    "THING.md",
                    "# HELLO WORLD! \nTHINGS GO HERE\nWORDS WORDS WORDS\n",
                ),
                TreeEntry::new(
                    "OTHER.md",
                    "# HELLO WORLD! \nTHINGS GO HERE\nWORDS WORDS WORDS\n",
                ),
            ],
        ),
    );
    make_commit(
        &repository,
        &Tree::with_message(
            "Feature Branch Eighth Commit!",
            [
                TreeEntry::new(
                    "THING.md",
                    "# WORDS WORDS WORDS! \nTHINGS GO HERE\nALSO FEATURE BRANCH CHANGES\n",
                ),
                TreeEntry::new(
                    "OTHER.md",
                    "# WORDS WORDS WORDS! \nTHINGS GO HERE\nYES FEATURE BRANCH HERE TOO\n",
                ),
            ],
        ),
    );
    make_commit(
        &repository,
        &Tree::with_message(
            "Feature Branch Ninth Commit!",
            [
                TreeEntry::new(
                    "THING.md",
                    "# HELLO WORLD! \nTHINGS GO HERE\nARE WE THERE YET?\n",
                ),
                TreeEntry::new(
                    "OTHER.md",
                    "# HELLO WORLD! \nTHINGS GO HERE\nARE WE THERE YET?\n",
                ),
            ],
        ),
    );
    make_commit(
        &repository,
        &Tree::with_message(
            "Feature Branch Tenth Commit!",
            [
                TreeEntry::new(
                    "THING.md",
                    "# HELLO WORLD! \nDOOT DOOT DOOT\nALSO FEATURE BRANCH CHANGES\n",
                ),
                TreeEntry::new(
                    "OTHER.md",
                    "# DOOT DOOT DOOT! \nTHINGS GO HERE\nYES FEATURE BRANCH HERE TOO\n",
                ),
            ],
        ),
    );

    // put the repository back on the default branch
    switch_to(&repository, "master");
    repository
}
