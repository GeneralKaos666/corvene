//! Port of GitHub Desktop's `app/test/unit/git/reorder-test.ts`.
//!
//! - `reorder(repository, toMove, beforeCommit, lastRetainedCommitRef)`
//!   (`lib/git/reorder.ts`) is `corvene_git::reorder`, run with
//!   `RebaseOptions::default()`: GitHub Desktop's values of flags
//!   `834-rebase-keeps-hash-messages` and `829-squash-autostash` (both off).
//!   GitHub Desktop's `RebaseResult.Error` is `RebaseResult::Error(_)`
//!   (Corvene's carries the reason).
//! - `getCommit(repository, 'HEAD')` / `getCommits(repository, 'HEAD', 5)`
//!   are `corvene_git::get_commits(path, "HEAD", 0, 1 | 5)`.
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

/// GitHub Desktop's `reorder(repository, toMove, beforeCommit,
/// lastRetainedCommitRef)`.
fn reorder(
    repository: &TestRepo,
    to_move: &[Commit],
    before_commit: Option<&Commit>,
    last_retained_commit_ref: Option<&str>,
) -> RebaseResult {
    corvene_git::reorder(
        git(),
        repository.path(),
        to_move,
        before_commit,
        last_retained_commit_ref,
        RebaseOptions::default(),
        |_| {},
    )
}

fn summaries(log: &[Commit]) -> Vec<&str> {
    log.iter().map(|c| c.summary.as_str()).collect()
}

/// The test file's `makeSampleCommit(repository, desc, file?)`.
fn make_sample_commit(repository: &TestRepo, desc: &str, file: Option<&str>) -> Commit {
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

// GHD: unit/git/reorder-test.ts › git/reorder › moves second commit before the first one
#[test]
fn moves_second_commit_before_the_first_one() {
    let repository = setup_empty_repository_default_main();
    let initial_commit = make_sample_commit(&repository, "initialize", None);

    let first_commit = make_sample_commit(&repository, "first", None);
    let second_commit = make_sample_commit(&repository, "second", None);

    let result = reorder(
        &repository,
        &[second_commit],
        Some(&first_commit),
        Some(&initial_commit.sha),
    );

    assert_eq!(result, RebaseResult::CompletedWithoutError);

    let log = get_commits(&repository, "HEAD", 5);
    assert_eq!(log.len(), 3);
    assert_eq!(log[2].summary, "initialize");
    assert_eq!(log[1].summary, "second");
    assert_eq!(log[0].summary, "first");
}

// GHD: unit/git/reorder-test.ts › git/reorder › moves first and fourth commits after the second one respecting their order in the log
#[test]
fn moves_first_and_fourth_commits_after_the_second_one_respecting_their_order_in_the_log() {
    let repository = setup_empty_repository_default_main();
    let initial_commit = make_sample_commit(&repository, "initialize", None);

    let first_commit = make_sample_commit(&repository, "first", None);
    make_sample_commit(&repository, "second", None);
    let third_commit = make_sample_commit(&repository, "third", None);
    let fourth_commit = make_sample_commit(&repository, "fourth", None);

    let result = reorder(
        &repository,
        &[fourth_commit, first_commit], // provided in opposite log order
        Some(&third_commit),
        Some(&initial_commit.sha),
    );

    assert_eq!(result, RebaseResult::CompletedWithoutError);

    let log = get_commits(&repository, "HEAD", 5);
    assert_eq!(log.len(), 5);

    assert_eq!(
        summaries(&log),
        ["third", "fourth", "first", "second", "initialize"]
    );
}

// GHD: unit/git/reorder-test.ts › git/reorder › moves first commit after the last one
#[test]
fn moves_first_commit_after_the_last_one() {
    let repository = setup_empty_repository_default_main();
    let initial_commit = make_sample_commit(&repository, "initialize", None);

    let first_commit = make_sample_commit(&repository, "first", None);
    make_sample_commit(&repository, "second", None);
    make_sample_commit(&repository, "third", None);
    make_sample_commit(&repository, "last", None);

    let result = reorder(
        &repository,
        &[first_commit],
        None,
        Some(&initial_commit.sha),
    );

    assert_eq!(result, RebaseResult::CompletedWithoutError);

    let log = get_commits(&repository, "HEAD", 5);
    assert_eq!(
        summaries(&log),
        ["first", "last", "third", "second", "initialize"]
    );
}

// GHD: unit/git/reorder-test.ts › git/reorder › reorders using the root of the branch if last retained commit is null
#[test]
fn reorders_using_the_root_of_the_branch_if_last_retained_commit_is_null() {
    let repository = setup_empty_repository_default_main();
    let initial_commit = make_sample_commit(&repository, "initialize", None);

    let first_commit = make_sample_commit(&repository, "first", None);
    make_sample_commit(&repository, "second", None);

    let result = reorder(&repository, &[first_commit], Some(&initial_commit), None);

    assert_eq!(result, RebaseResult::CompletedWithoutError);

    let log = get_commits(&repository, "HEAD", 5);
    assert_eq!(log.len(), 3);

    assert_eq!(summaries(&log), ["second", "initialize", "first"]);
}

// GHD: unit/git/reorder-test.ts › git/reorder › handles reordering a conflicting commit
#[test]
fn handles_reordering_a_conflicting_commit() {
    let repository = setup_empty_repository_default_main();
    let initial_commit = make_sample_commit(&repository, "initialize", None);

    make_sample_commit(&repository, "first", None);

    // make a commit with a commit message 'second' and adding file 'second.md'
    let second_commit = make_sample_commit(&repository, "second", None);

    // make a third commit modifying 'second.md' from secondCommit
    let third_commit = make_sample_commit(&repository, "third", Some("second"));

    // move third commit before second commit
    // Will cause a conflict due to modifications to 'second.md'  - a file that
    // does not exist in the first commit.
    let result = reorder(
        &repository,
        &[third_commit],
        Some(&second_commit),
        Some(&initial_commit.sha),
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
    let (_third_dir, third_message_path) = get_temp_file_path("reorderCommitMessage-third");
    std::fs::write(&third_message_path, "third - fixed").unwrap();

    // continue rebase
    let mut continue_result = continue_rebase(
        &repository,
        &files,
        Some(&format!("cat \"{}\" >", third_message_path.display())),
    );

    // This will now conflict with the 'third' commit since it is going to now
    // apply the 'second' commit which now modifies the same lines in the
    // 'second.md' that the previous commit does.
    assert_eq!(continue_result, RebaseResult::ConflictsEncountered);

    let status = get_status_or_throw(&repository);
    files = status.files;

    std::fs::write(
        repository.join("second.md"),
        "# resolve conflict from putting \"third\" before \"second\"",
    )
    .unwrap();

    let (_second_dir, second_message_path) = get_temp_file_path("reorderCommitMessage-second");
    std::fs::write(&second_message_path, "second - fixed").unwrap();

    continue_result = continue_rebase(
        &repository,
        &files,
        Some(&format!("cat \"{}\" >", second_message_path.display())),
    );
    assert_eq!(continue_result, RebaseResult::CompletedWithoutError);

    let log = get_commits(&repository, "HEAD", 5);
    assert_eq!(
        summaries(&log),
        ["second - fixed", "third - fixed", "first", "initialize"]
    );
}

// GHD: unit/git/reorder-test.ts › git/reorder › returns error on invalid lastRetainedCommitRef
#[test]
fn returns_error_on_invalid_last_retained_commit_ref() {
    let repository = setup_empty_repository_default_main();
    make_sample_commit(&repository, "initialize", None);

    let first_commit = make_sample_commit(&repository, "first", None);
    let second_commit = make_sample_commit(&repository, "second", None);

    let result = reorder(
        &repository,
        &[second_commit],
        Some(&first_commit),
        Some("INVALID INVALID"),
    );

    assert!(matches!(result, RebaseResult::Error(_)), "{result:?}");

    // Rebase will not start - As it won't be able retrieve a commits to build a
    // todo and then interactive rebase would fail for bad revision. Added logic
    // to short circuit to prevent unnecessary attempt at an interactive rebase.
    let is_rebase_still_ongoing = corvene_git::rebase_internal_state(repository.path());
    assert!(is_rebase_still_ongoing.is_none());
}

// GHD: unit/git/reorder-test.ts › git/reorder › returns error on invalid base commit
#[test]
fn returns_error_on_invalid_base_commit() {
    let repository = setup_empty_repository_default_main();
    let initial_commit = make_sample_commit(&repository, "initialize", None);

    make_sample_commit(&repository, "first", None);
    let second_commit = make_sample_commit(&repository, "second", None);

    let bad_commit = Commit {
        sha: "INVALID".into(),
        summary: "INVALID".into(),
        ..second_commit.clone()
    };
    let result = reorder(
        &repository,
        &[second_commit],
        Some(&bad_commit),
        Some(&initial_commit.sha),
    );

    assert!(matches!(result, RebaseResult::Error(_)), "{result:?}");

    // Rebase should not start - if we did attempt this, it could result in
    // dropping commits.
    let is_rebase_still_ongoing = corvene_git::rebase_internal_state(repository.path());
    assert!(is_rebase_still_ongoing.is_none());
}

// GHD: unit/git/reorder-test.ts › git/reorder › returns error when no commits are reordered
#[test]
fn returns_error_when_no_commits_are_reordered() {
    let repository = setup_empty_repository_default_main();
    let initial_commit = make_sample_commit(&repository, "initialize", None);

    let first = make_sample_commit(&repository, "first", None);
    make_sample_commit(&repository, "second", None);

    let result = reorder(&repository, &[], Some(&first), Some(&initial_commit.sha));

    assert!(matches!(result, RebaseResult::Error(_)), "{result:?}");

    // Rebase should not start - technically there would be no harm in this
    // rebase as it would just replay history, but we should not use reorder to
    // replay history.
    let is_rebase_still_ongoing = corvene_git::rebase_internal_state(repository.path());
    assert!(is_rebase_still_ongoing.is_none());
}
