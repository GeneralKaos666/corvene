//! Port of GitHub Desktop's `app/test/unit/git/revert-test.ts`.
//!
//! GitHub Desktop's `revertCommit(repository, commit, currentRemote)`
//! (`lib/git/revert.ts`) runs `git revert [-m 1] <sha>`, passing `-m 1` when
//! the commit has more than one parent. Corvene's
//! `corvene_git::revert_commit(git, path, sha, is_merge)` takes that decision
//! as `is_merge`, which the dispatcher fills with `Commit::is_merge`; the
//! remote (only used for progress) has no counterpart.
//! `getCommits(repository, 'HEAD', 3)` is `corvene_git::get_commits(path,
//! "HEAD", 0, 3)`.

use corvene_test_support::{
    Tree, TreeEntry, get_tip_or_error, git, make_commit, setup_empty_repository,
};

// GHD: unit/git/revert-test.ts › git/revert › revertCommit › reverts a simple commit
#[test]
fn reverts_a_simple_commit() {
    let repo = setup_empty_repository();

    // Create an initial commit with a file
    make_commit(
        &repo,
        &Tree::with_message(
            "initial commit",
            [TreeEntry::new("file.txt", "initial content")],
        ),
    );

    // Create a second commit that modifies the file
    make_commit(
        &repo,
        &Tree::with_message(
            "modify file",
            [TreeEntry::new("file.txt", "modified content")],
        ),
    );

    // Get the tip commit (the one to revert)
    let tip = get_tip_or_error(&repo);

    // Revert the second commit
    corvene_git::revert_commit(git(), repo.path(), &tip.sha, tip.is_merge()).expect("revert");

    // Verify a new revert commit was created
    let commits = corvene_git::get_commits(repo.path(), "HEAD", 0, 3).expect("getCommits");
    assert_eq!(commits.len(), 3);
    assert!(commits[0].summary.starts_with("Revert"));
    assert_eq!(
        std::fs::read_to_string(repo.join("file.txt")).unwrap(),
        "initial content"
    );
}

// GHD: unit/git/revert-test.ts › git/revert › revertCommit › reverts a commit that adds a new file
#[test]
fn reverts_a_commit_that_adds_a_new_file() {
    let repo = setup_empty_repository();

    // Create initial commit
    make_commit(
        &repo,
        &Tree::with_message("initial commit", [TreeEntry::new("initial.txt", "initial")]),
    );

    // Create commit that adds a new file
    make_commit(
        &repo,
        &Tree::with_message(
            "add new file",
            [
                TreeEntry::new("initial.txt", "initial"),
                TreeEntry::new("new-file.txt", "new content"),
            ],
        ),
    );

    let tip = get_tip_or_error(&repo);
    corvene_git::revert_commit(git(), repo.path(), &tip.sha, tip.is_merge()).expect("revert");

    // Verify the revert commit exists
    let commits = corvene_git::get_commits(repo.path(), "HEAD", 0, 3).expect("getCommits");
    assert_eq!(commits.len(), 3);
    assert!(commits[0].summary.starts_with("Revert"));
    assert!(!repo.join("new-file.txt").exists());
}
