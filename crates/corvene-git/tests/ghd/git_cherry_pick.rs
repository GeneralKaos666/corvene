//! Port of GitHub Desktop's `app/test/unit/git/cherry-pick-test.ts`.
//!
//! - `cherryPick`, `continueCherryPick`, `abortCherryPick`,
//!   `getCherryPickSnapshot` (`lib/git/cherry-pick.ts`) are
//!   `corvene_git::{cherry_pick, continue_cherry_pick, abort_cherry_pick,
//!   cherry_pick_snapshot}`, run with GitHub Desktop's value of flag
//!   `836-cherry-pick-keeps-messages` (off). GitHub Desktop's `cherryPick`
//!   and `continueCherryPick` never return `CherryPickResult.Error`: they
//!   throw when git fails with an error they do not expect (`git()`, or
//!   `parseCherryPickResult`'s `Unhandled result found`), which fails a test
//!   even where it ignores the result. Corvene returns
//!   `CherryPickResult::Error(stderr)` for it, so [`try_cherry_pick`] turns
//!   that into an `Err` (the rejected promise, caught by the one test that
//!   expects it) and [`cherry_pick`] / [`continue_cherry_pick`] panic on it.
//!   An `Err` of `continue_cherry_pick` / `abort_cherry_pick` is a throw too.
//! - `getCommit(repository, sha)` and `getCommits(repository, ref, n)` are
//!   `corvene_git::get_commits(path, ref, 0, 1 | n)`;
//!   `getCommitsInRange(repository, range)` is `corvene_git::commits_in_range`
//!   (`None` is `null`).
//! - `revRangeInclusive(from, to)` (`lib/git/rev-list.ts`) only spells the
//!   range `<from>^..<to>`; Corvene has no helper for it, so the test spells
//!   it the same way.
//! - `merge(repository, ref)` is `corvene_git::merge_branch(git, path, ref,
//!   false)`; `MergeResult.Success` is `MergeOutcome::Success`.
//! - A GitHub Desktop `Branch`'s `tip.sha` is `Branch::tip` and its `ref` is
//!   `Branch::full_name`.
//! - `IMultiCommitOperationProgress` is `corvene_models::McoProgress` (its
//!   `kind` only tells GitHub Desktop's `Progress` union apart, see
//!   `git_rebase_progress.rs`).
//! - `isConflictedFile(file.status)` is `file.status.kind ==
//!   FileStatusKind::Conflicted`.

use std::collections::BTreeMap;

use corvene_git::{CherryPickResult, CherryPickSnapshot, MergeOutcome};
use corvene_models::{
    Branch, Commit, CommitOneLine, FileStatusKind, ManualConflictResolution, McoProgress,
    WorkingDirectoryFileChange,
};
use corvene_test_support::{
    TestRepo, Tree, TreeEntry, create_branch, exec, get_branch_or_error, get_status_or_throw, git,
    make_commit, repository_builder_cherry_pick, switch_to,
};

const FEATURE_BRANCH_NAME: &str = "this-is-a-feature";
const TARGET_BRANCH_NAME: &str = "target-branch";

struct Setup {
    repository: TestRepo,
    feature_branch: Branch,
    target_branch: Branch,
}

fn setup() -> Setup {
    // This will create a repository with a feature branch with one commit to
    // cherry pick and will check out the target branch.
    let repository =
        repository_builder_cherry_pick::create_repository(FEATURE_BRANCH_NAME, TARGET_BRANCH_NAME);

    // branch with tip as commit to cherry pick
    let feature_branch = get_branch_or_error(&repository, FEATURE_BRANCH_NAME);

    // branch with to cherry pick to
    let target_branch = get_branch_or_error(&repository, TARGET_BRANCH_NAME);

    Setup {
        repository,
        feature_branch,
        target_branch,
    }
}

fn tip(branch: &Branch) -> &str {
    branch.tip.as_deref().expect("branch has a tip")
}

fn write_file(path: std::path::PathBuf, contents: &str) {
    std::fs::write(&path, contents).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
}

/// GitHub Desktop's `cherryPick(repository, commits, progressCallback?)`
/// as a promise: `Err` is its throw (see the module doc).
fn try_cherry_pick(
    repository: &TestRepo,
    commits: &[CommitOneLine],
    progress_callback: Option<&mut dyn FnMut(McoProgress)>,
) -> Result<CherryPickResult, String> {
    let mut progress_callback = progress_callback;
    match corvene_git::cherry_pick(git(), repository.path(), commits, false, |p| {
        if let Some(callback) = progress_callback.as_mut() {
            callback(p);
        }
    }) {
        CherryPickResult::Error(error) => Err(error),
        result => Ok(result),
    }
}

/// GitHub Desktop's `await cherryPick(repository, commits,
/// progressCallback?)` outside a `try`: its throw fails the test.
fn cherry_pick(
    repository: &TestRepo,
    commits: &[CommitOneLine],
    progress_callback: Option<&mut dyn FnMut(McoProgress)>,
) -> CherryPickResult {
    try_cherry_pick(repository, commits, progress_callback)
        .unwrap_or_else(|error| panic!("cherryPick threw: {error}"))
}

/// GitHub Desktop's `continueCherryPick(repository, files,
/// manualResolutions, progressCallback?)`.
fn continue_cherry_pick(
    repository: &TestRepo,
    files: &[WorkingDirectoryFileChange],
    manual_resolutions: &BTreeMap<String, ManualConflictResolution>,
    progress_callback: Option<&mut dyn FnMut(McoProgress)>,
) -> CherryPickResult {
    let mut progress_callback = progress_callback;
    let result = corvene_git::continue_cherry_pick(
        git(),
        repository.path(),
        files,
        manual_resolutions,
        false,
        |p| {
            if let Some(callback) = progress_callback.as_mut() {
                callback(p);
            }
        },
    )
    .expect("continueCherryPick");
    if let CherryPickResult::Error(error) = &result {
        panic!("continueCherryPick threw: Unhandled result found: {error}");
    }
    result
}

/// GitHub Desktop's `abortCherryPick(repository)`.
fn abort_cherry_pick(repository: &TestRepo) {
    corvene_git::abort_cherry_pick(git(), repository.path()).expect("abortCherryPick");
}

/// GitHub Desktop's `getCherryPickSnapshot(repository)`.
fn get_cherry_pick_snapshot(repository: &TestRepo) -> Option<CherryPickSnapshot> {
    corvene_git::cherry_pick_snapshot(git(), repository.path())
}

/// GitHub Desktop's `getCommit(repository, ref)`.
fn get_commit(repository: &TestRepo, reference: &str) -> Option<Commit> {
    corvene_git::get_commits(repository.path(), reference, 0, 1)
        .expect("getCommit")
        .into_iter()
        .next()
}

/// GitHub Desktop's `getCommits(repository, revisionRange, limit)`.
fn get_commits(repository: &TestRepo, revision_range: &str, limit: usize) -> Vec<Commit> {
    corvene_git::get_commits(repository.path(), revision_range, 0, limit).expect("getCommits")
}

/// GitHub Desktop's `getCommitsInRange(repository, range)`.
fn get_commits_in_range(repository: &TestRepo, range: &str) -> Option<Vec<CommitOneLine>> {
    corvene_git::commits_in_range(git(), repository.path(), range).expect("getCommitsInRange")
}

/// GitHub Desktop's `revRangeInclusive(from, to)` (see the module doc).
fn rev_range_inclusive(from: &str, to: &str) -> String {
    format!("{from}^..{to}")
}

/// The test file's `getCommitOneLine(repository, commitSha)`.
fn get_commit_one_line(repository: &TestRepo, commit_sha: &str) -> CommitOneLine {
    let commit = get_commit(repository, commit_sha);
    assert!(commit.is_some());
    let commit = commit.unwrap();
    CommitOneLine {
        sha: commit.sha,
        summary: commit.summary,
    }
}

/// The test file's `addThreeMoreCommitsOntoFeatureBranch(repository)`.
fn add_three_more_commits_onto_feature_branch(repository: &TestRepo) {
    switch_to(repository, FEATURE_BRANCH_NAME);

    let feature_branch_commit_two = Tree::with_message(
        "Cherry-picked Feature! Number Two",
        [TreeEntry::new(
            "THING_TWO.md",
            "# HELLO WORLD! \nTHINGS GO HERE\n",
        )],
    );
    make_commit(repository, &feature_branch_commit_two);

    let feature_branch_commit_three = Tree::with_message(
        "Cherry-picked Feature! Number Three",
        [TreeEntry::new(
            "THING_THREE.md",
            "# HELLO WORLD! \nTHINGS GO HERE\n",
        )],
    );
    make_commit(repository, &feature_branch_commit_three);

    let feature_branch_commit_four = Tree::with_message(
        "Cherry-picked Feature! Number Four",
        [TreeEntry::new(
            "THING_FOUR.md",
            "# HELLO WORLD! \nTHINGS GO HERE\n",
        )],
    );
    make_commit(repository, &feature_branch_commit_four);
}

/// The `cherry-picking with conflicts` describe's `makeConflictCommit`.
fn make_conflict_commit(repository: &TestRepo) {
    // In the 'git/cherry-pick' `beforeEach`, we call `createRepository` which
    // adds a commit to the feature branch with a file called THING.md. In
    // order to make a conflict, we will add the same file to the target
    // branch.
    let conflicting_commit = Tree::with_message(
        "Conflicting Commit!",
        [TreeEntry::new(
            "THING.md",
            "# HELLO WORLD! \n CREATING CONFLICT! FUN TIMES!\n",
        )],
    );
    make_commit(repository, &conflicting_commit);
}

fn conflicted_count(files: &[WorkingDirectoryFileChange]) -> usize {
    files
        .iter()
        .filter(|f| f.status.kind == FileStatusKind::Conflicted)
        .count()
}

// GHD: unit/git/cherry-pick-test.ts › git/cherry-pick › successfully cherry-picked one commit without conflicts
#[test]
fn successfully_cherry_picked_one_commit_without_conflicts() {
    let Setup {
        repository,
        feature_branch,
        target_branch,
    } = setup();

    let feature_tip = get_commit_one_line(&repository, tip(&feature_branch));
    let result = cherry_pick(&repository, &[feature_tip], None);
    let cherry_picked_commit = get_commit(&repository, tip(&feature_branch));

    let commits = get_commits(&repository, &target_branch.full_name, 3);
    assert!(cherry_picked_commit.is_some());
    let cherry_picked_commit = cherry_picked_commit.unwrap();

    assert_eq!(commits.len(), 2);
    assert_eq!(commits[0].summary, cherry_picked_commit.summary);
    assert_eq!(result, CherryPickResult::CompletedWithoutError);
}

// GHD: unit/git/cherry-pick-test.ts › git/cherry-pick › successfully cherry-picked a commit with empty message
#[test]
fn successfully_cherry_picked_a_commit_with_empty_message() {
    let Setup {
        repository,
        target_branch,
        ..
    } = setup();

    // add a commit with no message
    switch_to(&repository, FEATURE_BRANCH_NAME);
    let file_path = repository.join("EMPTY_MESSAGE.md");
    write_file(file_path.clone(), "# HELLO WORLD! \nTHINGS GO HERE\n");
    exec(
        [std::ffi::OsStr::new("add"), file_path.as_os_str()],
        repository.path(),
    );
    exec(
        ["commit", "--allow-empty-message", "-m", ""],
        repository.path(),
    );

    let feature_branch = get_branch_or_error(&repository, FEATURE_BRANCH_NAME);
    switch_to(&repository, TARGET_BRANCH_NAME);

    // confirm feature branch tip has an empty message
    let empty_message_commit = get_commit(&repository, tip(&feature_branch));
    assert_eq!(
        empty_message_commit.as_ref().map(|c| c.summary.as_str()),
        Some("")
    );

    let feature_tip = get_commit_one_line(&repository, tip(&feature_branch));
    let result = cherry_pick(&repository, &[feature_tip], None);

    let commits = get_commits(&repository, &target_branch.full_name, 5);
    assert_eq!(commits.len(), 2);
    assert_eq!(commits[0].summary, "");
    assert_eq!(result, CherryPickResult::CompletedWithoutError);
}

// GHD: unit/git/cherry-pick-test.ts › git/cherry-pick › successfully cherry-picks a redundant commit
#[test]
fn successfully_cherry_picks_a_redundant_commit() {
    let Setup {
        repository,
        feature_branch,
        target_branch,
    } = setup();

    let mut feature_tip = get_commit_one_line(&repository, tip(&feature_branch));
    let mut result = cherry_pick(&repository, &[feature_tip], None);

    let commits = get_commits(&repository, &target_branch.full_name, 5);
    assert_eq!(commits.len(), 2);
    assert_eq!(result, CherryPickResult::CompletedWithoutError);

    feature_tip = get_commit_one_line(&repository, tip(&feature_branch));
    result = cherry_pick(&repository, &[feature_tip], None);

    let commits_after_redundant = get_commits(&repository, &target_branch.full_name, 5);
    assert_eq!(commits_after_redundant.len(), 3);
    assert_eq!(result, CherryPickResult::CompletedWithoutError);
}

// GHD: unit/git/cherry-pick-test.ts › git/cherry-pick › successfully cherry-picks an empty commit
#[test]
fn successfully_cherry_picks_an_empty_commit() {
    let Setup {
        repository,
        target_branch,
        ..
    } = setup();

    // add empty commit to feature branch
    switch_to(&repository, FEATURE_BRANCH_NAME);
    exec(
        ["commit", "--allow-empty", "-m", "Empty Commit"],
        repository.path(),
    );

    let feature_branch = get_branch_or_error(&repository, FEATURE_BRANCH_NAME);
    switch_to(&repository, TARGET_BRANCH_NAME);

    let feature_tip = get_commit_one_line(&repository, tip(&feature_branch));
    let result = cherry_pick(&repository, &[feature_tip], None);

    let commits = get_commits(&repository, &target_branch.full_name, 5);
    assert_eq!(commits.len(), 2);
    assert_eq!(result, CherryPickResult::CompletedWithoutError);
}

// GHD: unit/git/cherry-pick-test.ts › git/cherry-pick › successfully cherry-picks an empty commit inside a range
#[test]
fn successfully_cherry_picks_an_empty_commit_inside_a_range() {
    let Setup {
        repository,
        feature_branch,
        target_branch,
    } = setup();
    let first_commit_sha = tip(&feature_branch).to_string();

    // add empty commit to feature branch
    switch_to(&repository, FEATURE_BRANCH_NAME);
    exec(
        ["commit", "--allow-empty", "-m", "Empty Commit"],
        repository.path(),
    );

    // add another commit so empty commit will be inside a range
    let feature_branch_commit_two = Tree::with_message(
        "Cherry-picked Feature! Number Two",
        [TreeEntry::new(
            "THING_TWO.md",
            "# HELLO WORLD! \nTHINGS GO HERE\n",
        )],
    );
    make_commit(&repository, &feature_branch_commit_two);

    let feature_branch_updated = get_branch_or_error(&repository, FEATURE_BRANCH_NAME);
    switch_to(&repository, TARGET_BRANCH_NAME);

    // cherry picking 3 (on added in setup, empty, featureBranchCommitTwo)
    let commit_range = rev_range_inclusive(&first_commit_sha, tip(&feature_branch_updated));
    let commits_in_range = get_commits_in_range(&repository, &commit_range);
    assert!(commits_in_range.is_some());
    let result = cherry_pick(&repository, &commits_in_range.unwrap(), None);

    let commits = get_commits(&repository, &target_branch.full_name, 5);
    assert_eq!(commits.len(), 4); // original commit + 4 cherry picked
    assert_eq!(result, CherryPickResult::CompletedWithoutError);
}

// GHD: unit/git/cherry-pick-test.ts › git/cherry-pick › successfully cherry-picked multiple commits without conflicts
#[test]
fn successfully_cherry_picked_multiple_commits_without_conflicts() {
    let Setup {
        repository,
        feature_branch,
        target_branch,
    } = setup();
    // keep reference to the first commit in cherry pick range
    let first_commit_sha = tip(&feature_branch).to_string();

    add_three_more_commits_onto_feature_branch(&repository);
    let feature_branch_updated = get_branch_or_error(&repository, FEATURE_BRANCH_NAME);
    switch_to(&repository, TARGET_BRANCH_NAME);

    let commit_range = rev_range_inclusive(&first_commit_sha, tip(&feature_branch_updated));
    let commits_in_range = get_commits_in_range(&repository, &commit_range);
    assert!(commits_in_range.is_some());

    cherry_pick(&repository, &commits_in_range.unwrap(), None);

    let commits = get_commits(&repository, &target_branch.full_name, 5);
    assert_eq!(commits.len(), 5);
    assert_eq!(commits[1].summary, "Cherry-picked Feature! Number Three");
    assert_eq!(commits[2].summary, "Cherry-picked Feature! Number Two");
}

// GHD: unit/git/cherry-pick-test.ts › git/cherry-pick › fails to cherry-pick array of no commits
#[test]
fn fails_to_cherry_pick_array_of_no_commits() {
    let Setup { repository, .. } = setup();

    let result = cherry_pick(&repository, &[], None);
    assert_eq!(result, CherryPickResult::UnableToStart);
}

// GHD: unit/git/cherry-pick-test.ts › git/cherry-pick › fails to cherry-pick when working tree is not clean
#[test]
fn fails_to_cherry_pick_when_working_tree_is_not_clean() {
    let Setup {
        repository,
        feature_branch,
        ..
    } = setup();

    write_file(
        repository.join("THING.md"),
        "# HELLO WORLD! \nTHINGS GO HERE\nFEATURE BRANCH UNDERWAY\n",
    );

    // This error should not occur in the wild due to the nature of Desktop's UI
    // starting on source branch and having to checkout the target branch.
    // During target branch checkout, it will fail before we even get to cherry
    // picking. Thus, this scenario from a UI's perspective is already handled.
    // No need to add dugite errors to handle it.
    let mut result: Option<CherryPickResult> = None;
    let feature_tip = get_commit_one_line(&repository, tip(&feature_branch));
    match try_cherry_pick(&repository, &[feature_tip], None) {
        Ok(r) => result = Some(r),
        Err(error) => assert!(
            error.contains(
                "The following untracked working tree files would be overwritten by merge"
            ),
            "{error}"
        ),
    }
    assert_eq!(result, None);
}

// GHD: unit/git/cherry-pick-test.ts › git/cherry-pick › successfully cherry-picks a merge commit
#[test]
fn successfully_cherry_picks_a_merge_commit() {
    let Setup {
        repository,
        feature_branch,
        ..
    } = setup();

    //create new branch off of default to merge into feature branch
    switch_to(&repository, "main");
    let merge_branch_name = "branch-to-merge";
    create_branch(&repository, merge_branch_name, "HEAD");
    switch_to(&repository, merge_branch_name);
    let merge_commit = Tree::with_message(
        "Commit To Merge",
        [TreeEntry::new(
            "merging.md",
            "# HELLO WORLD! \nMERGED THINGS GO HERE\n",
        )],
    );
    make_commit(&repository, &merge_commit);
    let merge_branch = get_branch_or_error(&repository, merge_branch_name);
    switch_to(&repository, FEATURE_BRANCH_NAME);
    assert_eq!(
        corvene_git::merge_branch(git(), repository.path(), &merge_branch.full_name, false)
            .expect("merge"),
        MergeOutcome::Success
    );

    // top commit is a merge commit
    let commits = get_commits(&repository, &feature_branch.full_name, 7);
    assert!(commits[0].summary.contains("Merge"));

    let feature_branch_now = get_branch_or_error(&repository, FEATURE_BRANCH_NAME);
    switch_to(&repository, TARGET_BRANCH_NAME);

    let feature_tip = get_commit_one_line(&repository, tip(&feature_branch_now));
    let result = cherry_pick(&repository, &[feature_tip], None);
    assert_eq!(result, CherryPickResult::CompletedWithoutError);
}

// GHD: unit/git/cherry-pick-test.ts › git/cherry-pick › successfully cherry-picks a merge commit after a conflict
#[test]
fn successfully_cherry_picks_a_merge_commit_after_a_conflict() {
    let Setup {
        repository,
        feature_branch,
        ..
    } = setup();
    let first_sha = tip(&feature_branch).to_string();

    // In the 'git/cherry-pick' `beforeEach`, we call `createRepository` which
    // adds a commit to the feature branch with a file called THING.md. In
    // order to make a conflict, we will add the same file to the target
    // branch.
    let conflicting_commit = Tree::with_message(
        "Conflicting Commit!",
        [TreeEntry::new(
            "THING.md",
            "# HELLO WORLD! \n CREATING CONFLICT! FUN TIMES!\n",
        )],
    );
    make_commit(&repository, &conflicting_commit);

    //create new branch off of default to merge into feature branch
    switch_to(&repository, "main");
    let merge_branch_name = "branch-to-merge";
    create_branch(&repository, merge_branch_name, "HEAD");
    switch_to(&repository, merge_branch_name);
    let merge_commit = Tree::with_message(
        "Commit To Merge",
        [TreeEntry::new(
            "merging.md",
            "# HELLO WORLD! \nMERGED THINGS GO HERE\n",
        )],
    );
    make_commit(&repository, &merge_commit);
    let merge_branch = get_branch_or_error(&repository, merge_branch_name);
    switch_to(&repository, FEATURE_BRANCH_NAME);
    assert_eq!(
        corvene_git::merge_branch(git(), repository.path(), &merge_branch.full_name, false)
            .expect("merge"),
        MergeOutcome::Success
    );

    // top commit is a merge commit
    let commits = get_commits(&repository, &feature_branch.full_name, 7);
    assert!(commits[0].summary.contains("Merge"));

    let feature_branch_now = get_branch_or_error(&repository, FEATURE_BRANCH_NAME);
    switch_to(&repository, TARGET_BRANCH_NAME);

    let commit_range = rev_range_inclusive(&first_sha, tip(&feature_branch_now));
    let commits_in_range = get_commits_in_range(&repository, &commit_range);
    assert!(commits_in_range.is_some());
    let mut result = cherry_pick(&repository, &commits_in_range.unwrap(), None);
    assert_eq!(result, CherryPickResult::ConflictsEncountered);

    // resolve conflicts by writing files to disk
    write_file(
        repository.join("THING.md"),
        "# HELLO WORLD! \nTHINGS GO HERE\nFEATURE BRANCH UNDERWAY\n",
    );

    let status_after_cherry_pick = get_status_or_throw(&repository);
    let files = status_after_cherry_pick.files;

    result = continue_cherry_pick(&repository, &files, &BTreeMap::new(), None);

    assert_eq!(result, CherryPickResult::CompletedWithoutError);
}

// GHD: unit/git/cherry-pick-test.ts › git/cherry-pick › cherry-picking with conflicts › successfully detects cherry-pick with conflicts
#[test]
fn successfully_detects_cherry_pick_with_conflicts() {
    let Setup {
        repository,
        feature_branch,
        ..
    } = setup();
    make_conflict_commit(&repository);

    let feature_tip = get_commit_one_line(&repository, tip(&feature_branch));
    let result = cherry_pick(&repository, &[feature_tip], None);
    assert_eq!(result, CherryPickResult::ConflictsEncountered);

    let status = get_status_or_throw(&repository);
    assert_eq!(conflicted_count(&status.files), 1);
}

// GHD: unit/git/cherry-pick-test.ts › git/cherry-pick › cherry-picking with conflicts › successfully continues cherry-picking with conflicts after resolving them by overwriting
#[test]
fn successfully_continues_cherry_picking_with_conflicts_after_resolving_them_by_overwriting() {
    let Setup {
        repository,
        feature_branch,
        ..
    } = setup();
    make_conflict_commit(&repository);

    let feature_tip = get_commit_one_line(&repository, tip(&feature_branch));
    let mut result = cherry_pick(&repository, &[feature_tip], None);
    assert_eq!(result, CherryPickResult::ConflictsEncountered);

    let status_after_cherry_pick = get_status_or_throw(&repository);
    let files = status_after_cherry_pick.files;

    // git diff --check warns if conflict markers exist and will exit with
    // non-zero status if conflicts found
    let diff_check_before = exec(["diff", "--check"], repository.path());
    assert!(diff_check_before.exit_code > 0);

    // resolve conflicts by writing files to disk
    write_file(
        repository.join("THING.md"),
        "# HELLO WORLD! \nTHINGS GO HERE\nFEATURE BRANCH UNDERWAY\n",
    );

    // diff --check to verify no conflicts exist (exitCode should be 0)
    let diff_check_after = exec(["diff", "--check"], repository.path());
    assert_eq!(diff_check_after.exit_code, 0);

    result = continue_cherry_pick(&repository, &files, &BTreeMap::new(), None);

    assert_eq!(result, CherryPickResult::CompletedWithoutError);
}

// GHD: unit/git/cherry-pick-test.ts › git/cherry-pick › cherry-picking with conflicts › successfully continues cherry-picking with conflicts after resolving them manually
#[test]
fn successfully_continues_cherry_picking_with_conflicts_after_resolving_them_manually() {
    let Setup {
        repository,
        feature_branch,
        ..
    } = setup();
    make_conflict_commit(&repository);

    let feature_tip = get_commit_one_line(&repository, tip(&feature_branch));
    let mut result = cherry_pick(&repository, &[feature_tip], None);
    assert_eq!(result, CherryPickResult::ConflictsEncountered);

    let status_after_cherry_pick = get_status_or_throw(&repository);
    let files = status_after_cherry_pick.files;

    // git diff --check warns if conflict markers exist and will exit with
    // non-zero status if conflicts found
    let diff_check_before = exec(["diff", "--check"], repository.path());
    assert!(diff_check_before.exit_code > 0);

    let mut manual_resolutions = BTreeMap::<String, ManualConflictResolution>::new();

    for file in &files {
        if file.status.kind == FileStatusKind::Conflicted {
            manual_resolutions.insert(file.path.clone(), ManualConflictResolution::Theirs);
        }
    }

    result = continue_cherry_pick(&repository, &files, &manual_resolutions, None);

    assert_eq!(result, CherryPickResult::CompletedWithoutError);
}

// GHD: unit/git/cherry-pick-test.ts › git/cherry-pick › cherry-picking with conflicts › successfully continues cherry-picking with conflicts after resolving them manually and no changes to commit
#[test]
fn successfully_continues_cherry_picking_after_resolving_them_manually_and_no_changes_to_commit() {
    let Setup {
        repository,
        feature_branch,
        ..
    } = setup();
    make_conflict_commit(&repository);

    let feature_tip = get_commit_one_line(&repository, tip(&feature_branch));
    let mut result = cherry_pick(&repository, &[feature_tip], None);
    assert_eq!(result, CherryPickResult::ConflictsEncountered);

    let status_after_cherry_pick = get_status_or_throw(&repository);
    let files = status_after_cherry_pick.files;

    // git diff --check warns if conflict markers exist and will exit with
    // non-zero status if conflicts found
    let diff_check_before = exec(["diff", "--check"], repository.path());
    assert!(diff_check_before.exit_code > 0);

    let mut manual_resolutions = BTreeMap::<String, ManualConflictResolution>::new();

    for file in &files {
        if file.status.kind == FileStatusKind::Conflicted {
            manual_resolutions.insert(file.path.clone(), ManualConflictResolution::Ours);
        }
    }

    result = continue_cherry_pick(&repository, &files, &manual_resolutions, None);

    assert_eq!(result, CherryPickResult::CompletedWithoutError);
}

// GHD: unit/git/cherry-pick-test.ts › git/cherry-pick › cherry-picking with conflicts › successfully detects cherry-picking with outstanding files not staged
#[test]
fn successfully_detects_cherry_picking_with_outstanding_files_not_staged() {
    let Setup {
        repository,
        feature_branch,
        ..
    } = setup();
    make_conflict_commit(&repository);

    let feature_tip = get_commit_one_line(&repository, tip(&feature_branch));
    let mut result = cherry_pick(&repository, &[feature_tip], None);
    assert_eq!(result, CherryPickResult::ConflictsEncountered);

    result = continue_cherry_pick(&repository, &[], &BTreeMap::new(), None);
    assert_eq!(result, CherryPickResult::OutstandingFilesNotStaged);

    let status = get_status_or_throw(&repository);
    assert_eq!(conflicted_count(&status.files), 1);
}

// GHD: unit/git/cherry-pick-test.ts › git/cherry-pick › cherry-picking with conflicts › successfully continues cherry-picking with additional changes to untracked files
#[test]
fn successfully_continues_cherry_picking_with_additional_changes_to_untracked_files() {
    let Setup {
        repository,
        feature_branch,
        ..
    } = setup();
    make_conflict_commit(&repository);

    let feature_tip = get_commit_one_line(&repository, tip(&feature_branch));
    let mut result = cherry_pick(&repository, &[feature_tip], None);
    assert_eq!(result, CherryPickResult::ConflictsEncountered);

    // resolve conflicts by writing files to disk
    write_file(
        repository.join("THING.md"),
        "# HELLO WORLD! \nTHINGS GO HERE\nFEATURE BRANCH UNDERWAY\n",
    );

    // changes to untracked file
    write_file(
        repository.join("UNTRACKED_FILE.md"),
        "# HELLO WORLD! \nUNTRACKED FILE STUFF IN HERE\n",
    );

    let status_after_cherry_pick = get_status_or_throw(&repository);
    let files = status_after_cherry_pick.files;

    // THING.MD and UNTRACKED_FILE.md should be in working directory
    assert_eq!(files.len(), 2);

    result = continue_cherry_pick(&repository, &files, &BTreeMap::new(), None);
    assert_eq!(result, CherryPickResult::CompletedWithoutError);

    // Only UNTRACKED_FILE.md should be in working directory
    // THING.md committed with cherry pick
    let status = get_status_or_throw(&repository);
    assert_eq!(status.files[0].path, "UNTRACKED_FILE.md");
}

// GHD: unit/git/cherry-pick-test.ts › git/cherry-pick › cherry-picking with conflicts › successfully aborts cherry-pick after conflict
#[test]
fn successfully_aborts_cherry_pick_after_conflict() {
    let Setup {
        repository,
        feature_branch,
        ..
    } = setup();
    make_conflict_commit(&repository);

    let feature_tip = get_commit_one_line(&repository, tip(&feature_branch));
    let result = cherry_pick(&repository, &[feature_tip], None);
    assert_eq!(result, CherryPickResult::ConflictsEncountered);

    // files from cherry pick exist in conflicted state
    let status_after_conflict = get_status_or_throw(&repository);
    assert_eq!(status_after_conflict.files.len(), 1);

    abort_cherry_pick(&repository);

    // file from cherry pick removed after abort
    let status_after_abort = get_status_or_throw(&repository);
    assert_eq!(status_after_abort.files.len(), 0);
}

// GHD: unit/git/cherry-pick-test.ts › git/cherry-pick › cherry-picking progress › successfully parses progress for a single commit
#[test]
fn successfully_parses_progress_for_a_single_commit() {
    let Setup {
        repository,
        feature_branch,
        ..
    } = setup();
    let mut progress = Vec::<McoProgress>::new();

    let feature_tip = get_commit_one_line(&repository, tip(&feature_branch));
    cherry_pick(
        &repository,
        std::slice::from_ref(&feature_tip),
        Some(&mut |p| progress.push(p)),
    );

    // commit summary set up in before each is "Cherry-picked Feature"
    assert_eq!(
        progress,
        vec![McoProgress {
            current_summary: feature_tip.summary.clone(),
            position: 1,
            total: 1,
            value: 1.,
        }]
    );
}

// GHD: unit/git/cherry-pick-test.ts › git/cherry-pick › cherry-picking progress › successfully parses progress for multiple commits
#[test]
fn successfully_parses_progress_for_multiple_commits() {
    let Setup {
        repository,
        feature_branch,
        ..
    } = setup();
    let mut progress = Vec::<McoProgress>::new();

    let first_commit_sha = tip(&feature_branch).to_string();

    add_three_more_commits_onto_feature_branch(&repository);
    let feature_branch_now = get_branch_or_error(&repository, FEATURE_BRANCH_NAME);
    switch_to(&repository, TARGET_BRANCH_NAME);

    let commit_range = rev_range_inclusive(&first_commit_sha, tip(&feature_branch_now));
    let commits_in_range = get_commits_in_range(&repository, &commit_range);
    assert!(commits_in_range.is_some());
    let result = cherry_pick(
        &repository,
        &commits_in_range.unwrap(),
        Some(&mut |p| progress.push(p)),
    );

    assert_eq!(result, CherryPickResult::CompletedWithoutError);
    assert_eq!(progress.len(), 4);
}

// GHD: unit/git/cherry-pick-test.ts › git/cherry-pick › cherry-picking progress › successfully parses progress for multiple commits including a conflict
#[test]
fn successfully_parses_progress_for_multiple_commits_including_a_conflict() {
    let Setup {
        repository,
        feature_branch,
        ..
    } = setup();
    let mut progress = Vec::<McoProgress>::new();

    let first_commit_sha = tip(&feature_branch).to_string();

    add_three_more_commits_onto_feature_branch(&repository);
    let feature_branch_now = get_branch_or_error(&repository, FEATURE_BRANCH_NAME);
    switch_to(&repository, TARGET_BRANCH_NAME);

    // Add a commit to the target branch to conflict with the third commit on
    // the target branch.
    let target_branch_conflicting_commit_two = Tree::with_message(
        "Conflicting with 2nd commit on feature branch",
        [TreeEntry::new(
            "THING_THREE.md",
            "# Conflict with feature branch here",
        )],
    );
    make_commit(&repository, &target_branch_conflicting_commit_two);

    let commit_range = rev_range_inclusive(&first_commit_sha, tip(&feature_branch_now));
    let commits_in_range = get_commits_in_range(&repository, &commit_range);
    assert!(commits_in_range.is_some());
    let mut result = cherry_pick(
        &repository,
        &commits_in_range.unwrap(),
        Some(&mut |p| progress.push(p)),
    );
    assert_eq!(result, CherryPickResult::ConflictsEncountered);
    // First commit and second cherry picked and rest are waiting on conflict
    // resolution.
    assert_eq!(progress.len(), 2);

    // snapshot prepares the progress for the commit after what has
    // already happened.
    let snapshot = get_cherry_pick_snapshot(&repository);
    assert_eq!(
        snapshot.map(|s| s.progress.position),
        Some(progress[1].position + 1)
    );

    // resolve conflicts and continue
    let status_after_conflicted_cherry_pick = get_status_or_throw(&repository);
    let files = status_after_conflicted_cherry_pick.files;
    write_file(repository.join("THING_THREE.md"), "# Resolve conflicts!");
    result = continue_cherry_pick(
        &repository,
        &files,
        &BTreeMap::new(),
        Some(&mut |p| progress.push(p)),
    );
    assert_eq!(result, CherryPickResult::CompletedWithoutError);
    // After 3rd commit resolved, 3rd and 4th were cherry picked
    assert_eq!(progress.len(), 4);
    assert_eq!(progress[0].current_summary, "Cherry-picked Feature!");
    assert_eq!(
        progress[1].current_summary,
        "Cherry-picked Feature! Number Two"
    );
    assert_eq!(
        progress[2].current_summary,
        "Cherry-picked Feature! Number Three"
    );
    assert_eq!(
        progress[3].current_summary,
        "Cherry-picked Feature! Number Four"
    );
}
