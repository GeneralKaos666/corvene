//! Port of `app/test/helpers/repository-builder-pull-test.ts`.

use crate::repositories::{TestRepo, setup_empty_repository};
use crate::repository_scaffolding::{Tree, TreeEntry, make_commit, switch_to};

/// GitHub Desktop's `createRepository(t, branchName)` for pull tests: two
/// commits on `master` (`First!`, `Second!`), then two commits on
/// `branch_name` (`Added a new file`, `Updated README`), which stays checked
/// out.
pub fn create_repository(branch_name: &str) -> TestRepo {
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
            ],
        ),
    );

    switch_to(&repository, branch_name);

    make_commit(
        &repository,
        &Tree::with_message("Added a new file", [TreeEntry::new("CONTRIBUTING.md", "")]),
    );
    make_commit(
        &repository,
        &Tree::with_message(
            "Updated README",
            [TreeEntry::new("README.md", "things go here")],
        ),
    );

    repository
}
