//! Port of GitHub Desktop's `app/test/unit/git/squash-test.ts` (its
//! `describe` is `git/cherry-pick`).
//!
//! - `squash(repository, toSquash, squashOnto, lastRetainedCommitRef,
//!   commitMessage)` (`lib/git/squash.ts`) is `corvene_git::squash`, run
//!   with `RebaseOptions::default()`: GitHub Desktop's values of flags
//!   `834-rebase-keeps-hash-messages` and `829-squash-autostash` (both off).
//!   GitHub Desktop's `RebaseResult.Error` is `RebaseResult::Error(_)`
//!   (Corvene's carries the reason).
//! - `getCommit(repository, 'HEAD')` / `getCommits(repository, 'HEAD', 5)`
//!   are `corvene_git::get_commits(path, "HEAD", 0, 1 | 5)`.
//! - `getChangedFiles(repository, sha)` is `corvene_git::get_changed_files`.
//! - `getRebaseInternalState` is `corvene_git::rebase_internal_state`.
//! - `continueRebase(repository, files, undefined, { gitEditor })` is
//!   `corvene_git::continue_rebase` with `git_editor`
//!   (`crate::mco_support::continue_rebase`).

use corvene_git::{RebaseOptions, RebaseResult};
use corvene_models::Commit;
use corvene_test_support::{
    TestRepo, Tree, TreeEntry, exec, get_commits, get_status_or_throw, git, make_commit,
    setup_empty_repository_default_main,
};

use crate::mco_support::{continue_rebase, get_temp_file_path};

/// GitHub Desktop's `squash(repository, toSquash, squashOnto,
/// lastRetainedCommitRef, commitMessage)`.
fn squash(
    repository: &TestRepo,
    to_squash: &[Commit],
    squash_onto: &Commit,
    last_retained_commit_ref: Option<&str>,
    commit_message: &str,
) -> RebaseResult {
    corvene_git::squash(
        git(),
        repository.path(),
        to_squash,
        squash_onto,
        last_retained_commit_ref,
        commit_message,
        RebaseOptions::default(),
        |_| {},
    )
}

/// `getChangedFiles(repository, sha).files.map(f => f.path).join(' ')`.
fn changed_file_paths(repository: &TestRepo, sha: &str) -> String {
    let squashed_changeset_data =
        corvene_git::get_changed_files(git(), repository.path(), sha).expect("getChangedFiles");
    squashed_changeset_data
        .files
        .iter()
        .map(|f| f.path.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The test file's `makeSquashCommit(repository, desc, file?)`.
fn make_squash_commit(repository: &TestRepo, desc: &str, file: Option<&str>) -> Commit {
    let file = file.unwrap_or(desc);
    let commit_tree = Tree::with_message(
        desc,
        [TreeEntry::new(format!("{file}.md"), format!("# {desc} \n"))],
    );
    make_commit(repository, &commit_tree);

    let commit = get_commits(repository, "HEAD", 1).into_iter().next();
    assert!(commit.is_some(), "Couldn't find HEAD after committing!");
    commit.unwrap()
}

// GHD: unit/git/squash-test.ts › git/cherry-pick › squashes one commit onto the next (non-conflicting)
#[test]
fn squashes_one_commit_onto_the_next_non_conflicting() {
    let repository = setup_empty_repository_default_main();
    let initial_commit = make_squash_commit(&repository, "initialize", None);

    let first_commit = make_squash_commit(&repository, "first", None);
    let second_commit = make_squash_commit(&repository, "second", None);

    let result = squash(
        &repository,
        &[second_commit],
        &first_commit,
        Some(&initial_commit.sha),
        "Test Summary\n\nTest Body",
    );

    assert_eq!(result, RebaseResult::CompletedWithoutError);

    let log = get_commits(&repository, "HEAD", 5);
    let squashed = &log[0];
    assert_eq!(squashed.summary, "Test Summary");
    assert_eq!(squashed.body, "Test Body\n");
    assert_eq!(log.len(), 2);

    // verify squashed commit contains changes from squashed commits
    let squashed_file_paths = changed_file_paths(&repository, &squashed.sha);
    assert!(squashed_file_paths.contains("first.md"));
    assert!(squashed_file_paths.contains("second.md"));
}

// GHD: unit/git/squash-test.ts › git/cherry-pick › returns error when squashOnto is in the toSquash array
#[test]
fn returns_error_when_squash_onto_is_in_the_to_squash_array() {
    let repository = setup_empty_repository_default_main();
    let initial_commit = make_squash_commit(&repository, "initialize", None);

    let first_commit = make_squash_commit(&repository, "first", None);
    let second_commit = make_squash_commit(&repository, "second", None);

    let result = squash(
        &repository,
        &[first_commit.clone(), second_commit],
        &first_commit,
        Some(&initial_commit.sha),
        "Test Summary\n\nTest Body",
    );

    assert!(matches!(result, RebaseResult::Error(_)), "{result:?}");
}

// GHD: unit/git/squash-test.ts › git/cherry-pick › squashes multiple commit onto one (non-conflicting)
#[test]
fn squashes_multiple_commit_onto_one_non_conflicting() {
    let repository = setup_empty_repository_default_main();
    let initial_commit = make_squash_commit(&repository, "initialize", None);

    let first_commit = make_squash_commit(&repository, "first", None);
    let second_commit = make_squash_commit(&repository, "second", None);
    let third_commit = make_squash_commit(&repository, "third", None);
    let fourth_commit = make_squash_commit(&repository, "fourth", None);

    let result = squash(
        &repository,
        &[second_commit, third_commit, fourth_commit],
        &first_commit,
        Some(&initial_commit.sha),
        "Test Summary\n\nTest Body",
    );

    assert_eq!(result, RebaseResult::CompletedWithoutError);

    let log = get_commits(&repository, "HEAD", 5);
    let squashed = &log[0];
    assert_eq!(squashed.summary, "Test Summary");
    assert_eq!(squashed.body, "Test Body\n");
    assert_eq!(log.len(), 2);

    // verify squashed commit contains changes from squashed commits
    let squashed_file_paths = changed_file_paths(&repository, &squashed.sha);
    assert!(squashed_file_paths.contains("first.md"));
    assert!(squashed_file_paths.contains("second.md"));
    assert!(squashed_file_paths.contains("third.md"));
    assert!(squashed_file_paths.contains("fourth.md"));
}

// GHD: unit/git/squash-test.ts › git/cherry-pick › squashes using the root of the branch if last retained commit is null
#[test]
fn squashes_using_the_root_of_the_branch_if_last_retained_commit_is_null() {
    let repository = setup_empty_repository_default_main();
    let initial_commit = make_squash_commit(&repository, "initialize", None);

    let first_commit = make_squash_commit(&repository, "first", None);
    let second_commit = make_squash_commit(&repository, "second", None);

    let log = get_commits(&repository, "HEAD", 5);
    assert_eq!(log.len(), 3);

    let result = squash(
        &repository,
        &[first_commit, second_commit],
        &initial_commit, // first in branch (root) commit.
        None,
        "Test Summary\n\nTest Body",
    );

    assert_eq!(result, RebaseResult::CompletedWithoutError);

    let log = get_commits(&repository, "HEAD", 5);
    let squashed = &log[0];
    assert_eq!(squashed.summary, "Test Summary");
    assert_eq!(squashed.body, "Test Body\n");
    assert_eq!(log.len(), 1);

    // verify squashed commit contains changes from squashed commits
    let squashed_file_paths = changed_file_paths(&repository, &squashed.sha);
    assert!(squashed_file_paths.contains("initialize"));
    assert!(squashed_file_paths.contains("first.md"));
    assert!(squashed_file_paths.contains("second.md"));
}

// GHD: unit/git/squash-test.ts › git/cherry-pick › squashes multiple commit non-sequential commits (reorders, non-conflicting)
#[test]
fn squashes_multiple_commit_non_sequential_commits_reorders_non_conflicting() {
    let repository = setup_empty_repository_default_main();
    let initial_commit = make_squash_commit(&repository, "initialize", None);

    let first_commit = make_squash_commit(&repository, "first", None);
    make_squash_commit(&repository, "second", None);
    let third_commit = make_squash_commit(&repository, "third", None);
    make_squash_commit(&repository, "fourth", None);
    let fifth_commit = make_squash_commit(&repository, "fifth", None);

    // From oldest to newest, log looks like:
    // - initial commit
    // - 'first' commit
    // - 'second' commit
    // - 'third' commit
    // - 'fourth' commit
    // - 'fifth' commit

    // Squashing 'first' and 'fifth' onto 'third'
    // Thus, reordering to 'second', 'first - third - fifth', 'fourth'
    let result = squash(
        &repository,
        &[fifth_commit, first_commit], // provided in opposite log order
        &third_commit,
        Some(&initial_commit.sha),
        "",
    );

    assert_eq!(result, RebaseResult::CompletedWithoutError);

    // From oldest to newest, log should look like:
    // - initial commit - log[2]
    // - the squashed commit 'first third fifth` - order by log history
    // - 'fourth' commit - log[0]
    let log = get_commits(&repository, "HEAD", 5);
    let squashed = &log[1];
    assert_eq!(squashed.summary, "first");
    assert_eq!(squashed.body, "third\n\nfifth\n");
    assert_eq!(log[0].summary, "fourth");
    assert_eq!(log.len(), 4);

    // verify squashed commit contains changes from squashed commits
    let squashed_file_paths = changed_file_paths(&repository, &squashed.sha);
    assert!(squashed_file_paths.contains("first.md"));
    assert!(squashed_file_paths.contains("third.md"));
    assert!(squashed_file_paths.contains("fifth.md"));
    assert!(!squashed_file_paths.contains("second.md"));
    assert!(!squashed_file_paths.contains("fourth.md"));
}

// GHD: unit/git/squash-test.ts › git/cherry-pick › handles squashing a conflicting commit
#[test]
fn handles_squashing_a_conflicting_commit() {
    let repository = setup_empty_repository_default_main();
    let initial_commit = make_squash_commit(&repository, "initialize", None);

    let first_commit = make_squash_commit(&repository, "first", None);

    // make a commit with a commit message 'second' and adding file 'second.md'
    make_squash_commit(&repository, "second", None);

    // make a third commit modifying 'second.md' from secondCommit
    let third_commit = make_squash_commit(&repository, "third", Some("second"));

    // squash third commit onto first commit
    // Will cause a conflict due to modifications to 'second.md'  - a file that
    // does not exist in the first commit.
    let result = squash(
        &repository,
        &[third_commit],
        &first_commit,
        Some(&initial_commit.sha),
        "Test Summary\n\nTest Body",
    );

    assert_eq!(result, RebaseResult::ConflictsEncountered);

    let status = get_status_or_throw(&repository);
    let mut files = status.files;

    // resolve conflicts by adding the conflicting file
    exec(
        [
            std::ffi::OsStr::new("add"),
            repository.join("second.md").as_os_str(),
        ],
        repository.path(),
    );

    // If there are conflicts, we need to resend in git editor for changing the
    // git message on continue
    let (_message_dir, message_path) = get_temp_file_path("squashCommitMessage");
    std::fs::write(&message_path, "Test Summary\n\nTest Body").unwrap();

    // continue rebase
    let mut continue_result = continue_rebase(
        &repository,
        &files,
        Some(&format!("cat \"{}\" >", message_path.display())),
    );

    // This will now conflict with the 'second' commit since it is going to now
    // apply the second commit which now modifies the same lines in the
    // 'second.md' that the squashed first commit does.
    assert_eq!(continue_result, RebaseResult::ConflictsEncountered);

    let status = get_status_or_throw(&repository);
    files = status.files;

    std::fs::write(
        repository.join("second.md"),
        "# resolve conflict from adding add after resolving squash",
    )
    .unwrap();

    continue_result = continue_rebase(
        &repository,
        &files,
        // Only reason I did this here is to show it does not cause harm.
        // In case of multiple commits being squashed/reordered before the squash
        // completes, we may not be able to tell which conflict the squash
        // message will need to go after so we will be sending it on all
        // continues.
        Some(&format!("cat \"{}\" >", message_path.display())),
    );
    assert_eq!(continue_result, RebaseResult::CompletedWithoutError);

    let log = get_commits(&repository, "HEAD", 5);
    assert_eq!(log.len(), 3);
    let squashed = &log[1];
    assert_eq!(squashed.summary, "Test Summary");
    assert_eq!(squashed.body, "Test Body\n");

    // verify squashed commit contains changes from squashed commits
    let squashed_file_paths = changed_file_paths(&repository, &squashed.sha);
    assert!(squashed_file_paths.contains("first.md"));
    assert!(squashed_file_paths.contains("second.md"));
}

// GHD: unit/git/squash-test.ts › git/cherry-pick › squashes with default merged commit message/description if commit message not provided
#[test]
fn squashes_with_default_merged_commit_message_description_if_commit_message_not_provided() {
    let repository = setup_empty_repository_default_main();
    let initial_commit = make_squash_commit(&repository, "initialize", None);

    let first_commit = make_squash_commit(&repository, "first", None);
    let second_commit = make_squash_commit(&repository, "second", None);

    let result = squash(
        &repository,
        &[second_commit],
        &first_commit,
        Some(&initial_commit.sha),
        "",
    );
    assert_eq!(result, RebaseResult::CompletedWithoutError);

    let log = get_commits(&repository, "HEAD", 5);
    let squashed = &log[0];
    assert_eq!(squashed.summary, "first");
    assert_eq!(squashed.body, "second\n");
    assert_eq!(log.len(), 2);
}

// GHD: unit/git/squash-test.ts › git/cherry-pick › returns error on invalid lastRetainedCommitRef
#[test]
fn returns_error_on_invalid_last_retained_commit_ref() {
    let repository = setup_empty_repository_default_main();
    make_squash_commit(&repository, "initialize", None);

    let first_commit = make_squash_commit(&repository, "first", None);
    let second_commit = make_squash_commit(&repository, "second", None);

    let result = squash(
        &repository,
        &[second_commit],
        &first_commit,
        Some("INVALID INVALID"),
        "Test Summary\n\nTest Body",
    );

    assert!(matches!(result, RebaseResult::Error(_)), "{result:?}");

    // Rebase will not start - As it won't be able retrieve a commits to build a
    // todo and then interactive rebase would fail for bad revision. Added logic
    // to short circuit to prevent unnecessary attempt at an interactive rebase.
    assert_eq!(corvene_git::rebase_internal_state(repository.path()), None);
}

// GHD: unit/git/squash-test.ts › git/cherry-pick › returns error on invalid commit to squashOnto
#[test]
fn returns_error_on_invalid_commit_to_squash_onto() {
    let repository = setup_empty_repository_default_main();
    let initial_commit = make_squash_commit(&repository, "initialize", None);

    make_squash_commit(&repository, "first", None);
    let second_commit = make_squash_commit(&repository, "second", None);

    let bad_commit = Commit {
        sha: "INVALID".into(),
        summary: "INVALID".into(),
        ..second_commit.clone()
    };
    let result = squash(
        &repository,
        &[second_commit],
        &bad_commit,
        Some(&initial_commit.sha),
        "Test Summary\n\nTest Body",
    );

    assert!(matches!(result, RebaseResult::Error(_)), "{result:?}");

    // Rebase should not start - if we did attempt this, it could result in
    // dropping commits.
    assert_eq!(corvene_git::rebase_internal_state(repository.path()), None);
}

// GHD: unit/git/squash-test.ts › git/cherry-pick › returns error on empty toSquash
#[test]
fn returns_error_on_empty_to_squash() {
    let repository = setup_empty_repository_default_main();
    let initial_commit = make_squash_commit(&repository, "initialize", None);

    let first = make_squash_commit(&repository, "first", None);
    make_squash_commit(&repository, "second", None);

    let result = squash(
        &repository,
        &[],
        &first,
        Some(&initial_commit.sha),
        "Test Summary\n\nTest Body",
    );

    assert!(matches!(result, RebaseResult::Error(_)), "{result:?}");

    // Rebase should not start - technically there would be no harm in this
    // rebase as it would just replay history, but we should not use squash to
    // replay history.
    assert_eq!(corvene_git::rebase_internal_state(repository.path()), None);
}
