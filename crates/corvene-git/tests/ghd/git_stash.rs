//! Port of GitHub Desktop's `app/test/unit/git/stash-test.ts`.
//!
//! Corvene equivalents of GitHub Desktop's `lib/git/stash.ts`:
//!
//! - `getStashes(repository)` is `corvene_git::get_stashes`, which returns
//!   every `refs/stash` entry (and their count) with `branch: Some(..)` for
//!   the ones whose message is a Desktop one (`!!GitHub_Desktop<branch>`).
//!   GitHub Desktop's `desktopEntries` are those entries ([`get_stashes`]
//!   keeps them, as the dispatcher does with `s.branch.is_some()`); its
//!   `branchName` is `StashEntry::branch`, `stashSha` is `StashEntry::sha`.
//! - `createDesktopStashEntry(repository, branch, untrackedFilesToStage)` is
//!   `corvene_git::create_desktop_stash(git, path, branch, false)`, which
//!   stages every untracked file itself instead of the ones it is given (the
//!   tests pass every untracked file, or none when there are none); `false`
//!   is the `github-desktop` value of `869-stash-protects-assume-unchanged`.
//! - `createDesktopStashMessage(branch)` is
//!   `corvene_git::desktop_stash_message`.
//! - `dropDesktopStashEntry(repository, stashSha)` and
//!   `popStashEntry(repository, stashSha)` are
//!   `corvene_git::drop_desktop_stash_entry` and
//!   `corvene_git::pop_stash_entry`: both look the Desktop entry up by its
//!   sha and run `git stash drop|pop <name>`.
//! - `getLastDesktopStashEntryForBranch(repository, branch)` is
//!   `corvene_git::get_last_desktop_stash_entry_for_branch`
//!   ([`get_last_desktop_stash_entry_for_branch`]).

use corvene_models::{FileStatusKind, StashEntry};
use corvene_test_support::{
    DEFAULT_STRING_LENGTH, TestRepo, append_file, exec, generate_string, get_status_or_throw, git,
    setup_empty_repository,
};

/// GitHub Desktop's `StashResult` as the tests read it.
struct StashResult {
    /// The stash entries created by Desktop.
    desktop_entries: Vec<StashEntry>,
}

/// GitHub Desktop's `getStashes(repository)`: `corvene_git::get_stashes`,
/// keeping the Desktop entries (`branch: Some(..)`).
fn get_stashes(repository: &TestRepo) -> StashResult {
    let (entries, _count) = corvene_git::get_stashes(git(), repository.path())
        .unwrap_or_else(|err| panic!("getStashes: {err}"));
    StashResult {
        desktop_entries: entries.into_iter().filter(|e| e.branch.is_some()).collect(),
    }
}

/// `getLastDesktopStashEntryForBranch(repository, branch)`.
fn get_last_desktop_stash_entry_for_branch(
    repository: &TestRepo,
    branch: &str,
) -> Option<StashEntry> {
    corvene_git::get_last_desktop_stash_entry_for_branch(git(), repository.path(), branch)
        .expect("getLastDesktopStashEntryForBranch")
}

/// The `setup` of every `describe` but `getStash`'s first two cases: an
/// empty repository with an empty `README.md` committed.
fn setup() -> TestRepo {
    let repository = setup_empty_repository();
    std::fs::write(repository.join("README.md"), "").unwrap();
    exec(["add", "README.md"], repository.path());
    exec(["commit", "-m", "initial commit"], repository.path());
    repository
}

/// The test file's `stash(repository, branchName, message)`: `git stash
/// push -m <message>`, a Desktop message when `message` is `None`.
fn stash(repository: &TestRepo, branch_name: &str, message: Option<&str>) {
    let message = message
        .map(str::to_string)
        .unwrap_or_else(|| corvene_git::desktop_stash_message(branch_name));
    let result = exec(["stash", "push", "-m", message.as_str()], repository.path());
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
}

/// The test file's `generateTestStashEntry(repository, branchName,
/// simulateDesktopEntry)`.
fn generate_test_stash_entry(
    repository: &TestRepo,
    branch_name: &str,
    simulate_desktop_entry: bool,
) {
    let message = if simulate_desktop_entry {
        None
    } else {
        Some("Should get filtered")
    };
    append_file(
        repository.join("README.md"),
        generate_string(DEFAULT_STRING_LENGTH),
    );
    stash(repository, branch_name, message);
}

// GHD: unit/git/stash-test.ts › git/stash › getStash › handles unborn repo by returning empty list
#[test]
fn handles_unborn_repo_by_returning_empty_list() {
    let repo = setup_empty_repository();
    let stash = get_stashes(&repo);

    assert_eq!(stash.desktop_entries.len(), 0);
}

// GHD: unit/git/stash-test.ts › git/stash › getStash › returns an empty list when no stash entries have been created
#[test]
fn returns_an_empty_list_when_no_stash_entries_have_been_created() {
    let stash = get_stashes(&setup_empty_repository());

    assert_eq!(stash.desktop_entries.len(), 0);
}

// GHD: unit/git/stash-test.ts › git/stash › getStash › returns all stash entries created by Desktop
#[test]
fn returns_all_stash_entries_created_by_desktop() {
    let repository = setup_empty_repository();
    let readme = repository.join("README.md");
    std::fs::write(&readme, "").unwrap();
    exec(["add", "README.md"], repository.path());
    exec(["commit", "-m", "initial commit"], repository.path());

    generate_test_stash_entry(&repository, "master", false);
    generate_test_stash_entry(&repository, "master", false);
    generate_test_stash_entry(&repository, "master", true);

    let stash = get_stashes(&repository);
    let entries = stash.desktop_entries;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].branch.as_deref(), Some("master"));
    assert_eq!(entries[0].name, "refs/stash@{0}");
}

// GHD: unit/git/stash-test.ts › git/stash › createDesktopStashEntry › creates a stash entry when repo is not unborn or in any kind of conflict or rebase state
#[test]
fn creates_a_stash_entry_when_repo_is_not_unborn_or_in_any_kind_of_conflict_or_rebase_state() {
    let repository = setup();
    append_file(repository.join("README.md"), "just testing stuff");

    // `869-stash-protects-assume-unchanged` off, as in GitHub Desktop
    corvene_git::create_desktop_stash(git(), repository.path(), "master", false)
        .expect("createDesktopStashEntry");

    let stash = get_stashes(&repository);
    let entries = stash.desktop_entries;

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].branch.as_deref(), Some("master"));
}

// GHD: unit/git/stash-test.ts › git/stash › createDesktopStashEntry › stashes untracked files and removes them from the working directory
#[test]
fn stashes_untracked_files_and_removes_them_from_the_working_directory() {
    let repository = setup();
    let untracked_file = repository.join("not-tracked.txt");
    std::fs::write(&untracked_file, "some untracked file").unwrap();

    let status = get_status_or_throw(&repository);
    let files = &status.files;

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].status.kind, FileStatusKind::Untracked);

    // GitHub Desktop passes these to `createDesktopStashEntry`; Corvene's
    // stages every untracked file itself
    let _untracked_files: Vec<_> = status
        .files
        .iter()
        .filter(|f| f.status.kind == FileStatusKind::Untracked)
        .collect();

    // `869-stash-protects-assume-unchanged` off, as in GitHub Desktop
    corvene_git::create_desktop_stash(git(), repository.path(), "master", false)
        .expect("createDesktopStashEntry");

    let status = get_status_or_throw(&repository);
    let files = status.files;

    assert_eq!(files.len(), 0);
}

// GHD: unit/git/stash-test.ts › git/stash › getLastDesktopStashEntryForBranch › returns null when no stash entries exist for branch
#[test]
fn returns_null_when_no_stash_entries_exist_for_branch() {
    let repository = setup();
    generate_test_stash_entry(&repository, "some-other-branch", true);

    let entry = get_last_desktop_stash_entry_for_branch(&repository, "master");

    assert!(entry.is_none());
}

// GHD: unit/git/stash-test.ts › git/stash › getLastDesktopStashEntryForBranch › returns last entry made for branch
#[test]
fn returns_last_entry_made_for_branch() {
    let repository = setup();
    let branch_name = "master";
    generate_test_stash_entry(&repository, branch_name, true);
    generate_test_stash_entry(&repository, branch_name, true);

    let stash = get_stashes(&repository);
    // entries are returned in LIFO order
    let last_entry = &stash.desktop_entries[0];

    let actual = get_last_desktop_stash_entry_for_branch(&repository, branch_name);

    let actual = actual.expect("an entry");
    assert_eq!(actual.sha, last_entry.sha);
}

// GHD: unit/git/stash-test.ts › git/stash › createDesktopStashMessage › creates message that matches Desktop stash entry format
#[test]
fn creates_message_that_matches_desktop_stash_entry_format() {
    let branch_name = "master";

    let message = corvene_git::desktop_stash_message(branch_name);

    assert_eq!(message, "!!GitHub_Desktop<master>");
}

// GHD: unit/git/stash-test.ts › git/stash › dropDesktopStashEntry › removes the entry identified by `stashSha`
#[test]
fn removes_the_entry_identified_by_stash_sha() {
    let repository = setup();

    generate_test_stash_entry(&repository, "master", true);
    generate_test_stash_entry(&repository, "master", true);

    let stash = get_stashes(&repository);
    let entries = stash.desktop_entries;
    assert_eq!(entries.len(), 2);

    let stash_to_delete = entries[1].clone();
    corvene_git::drop_desktop_stash_entry(git(), repository.path(), &stash_to_delete.sha)
        .expect("dropDesktopStashEntry");

    // using this function to get stashSha since it parses
    // the output from git into easy to use objects
    let stash = get_stashes(&repository);
    let entries = stash.desktop_entries;
    assert_eq!(entries.len(), 1);
    // GitHub Desktop compares the sha with the entry object (never equal);
    // the sha of the entry is what the test means
    assert_ne!(entries[0].sha, stash_to_delete.sha);
}

/// GitHub Desktop's made-up `IStashEntry` of the two cases below (`files:
/// NotLoaded` has no Corvene field; `IStashEntry` has no message).
fn does_not_exist(name: &str) -> StashEntry {
    StashEntry {
        name: name.to_string(),
        sha: "xyz".to_string(),
        branch: Some("master".to_string()),
        message: String::new(),
        tree: "xyz".to_string(),
        parents: vec!["abc".to_string()],
        date: 0,
    }
}

// GHD: unit/git/stash-test.ts › git/stash › dropDesktopStashEntry › does not fail when attempting to delete when stash is empty
#[test]
fn does_not_fail_when_attempting_to_delete_when_stash_is_empty() {
    let repository = setup();

    let does_not_exist = does_not_exist("refs/stash@{0}");

    let result =
        corvene_git::drop_desktop_stash_entry(git(), repository.path(), &does_not_exist.sha);
    assert!(result.is_ok(), "{result:?}");
}

// GHD: unit/git/stash-test.ts › git/stash › dropDesktopStashEntry › does not fail when attempting to delete stash entry that doesn't exist
#[test]
fn does_not_fail_when_attempting_to_delete_stash_entry_that_doesnt_exist() {
    let repository = setup();
    let does_not_exist = does_not_exist("refs/stash@{4}");
    generate_test_stash_entry(&repository, "master", true);
    generate_test_stash_entry(&repository, "master", true);
    generate_test_stash_entry(&repository, "master", true);

    let result =
        corvene_git::drop_desktop_stash_entry(git(), repository.path(), &does_not_exist.sha);
    assert!(result.is_ok(), "{result:?}");
}

// GHD: unit/git/stash-test.ts › git/stash › popStashEntry › without any conflicts › restores changes back to the working directory
#[test]
fn restores_changes_back_to_the_working_directory() {
    let repository = setup();

    generate_test_stash_entry(&repository, "master", true);
    let stash = get_stashes(&repository);
    let desktop_entries = stash.desktop_entries;
    assert_eq!(desktop_entries.len(), 1);

    let status = get_status_or_throw(&repository);
    let files = status.files;
    assert_eq!(files.len(), 0);

    let entry_to_apply = &desktop_entries[0];
    corvene_git::pop_stash_entry(git(), repository.path(), &entry_to_apply.sha)
        .expect("popStashEntry");

    let status = get_status_or_throw(&repository);
    let files = status.files;
    assert_eq!(files.len(), 1);
}

// GHD: unit/git/stash-test.ts › git/stash › popStashEntry › when there are (resolvable) conflicts › restores changes and drops stash
#[test]
fn restores_changes_and_drops_stash() {
    let repository = setup();

    generate_test_stash_entry(&repository, "master", true);
    let stash = get_stashes(&repository);
    let desktop_entries = stash.desktop_entries;
    assert_eq!(desktop_entries.len(), 1);

    let readme = repository.join("README.md");
    append_file(&readme, generate_string(DEFAULT_STRING_LENGTH));
    exec(["commit", "-am", "later commit"], repository.path());

    let status = get_status_or_throw(&repository);
    let files = status.files;
    assert_eq!(files.len(), 0);

    let entry_to_apply = desktop_entries[0].clone();
    corvene_git::pop_stash_entry(git(), repository.path(), &entry_to_apply.sha)
        .expect("popStashEntry");

    let status = get_status_or_throw(&repository);
    let files = status.files;
    assert_eq!(files.len(), 1);

    let stash_after = get_stashes(&repository);
    // GitHub Desktop's `includes` compares objects by reference; the entry
    // (by value) being gone is what the test means
    assert!(!stash_after.desktop_entries.contains(&entry_to_apply));
}

// GHD: unit/git/stash-test.ts › git/stash › popStashEntry › when there are unresolvable conflicts › throws an error
#[test]
fn throws_an_error() {
    let repository = setup();

    generate_test_stash_entry(&repository, "master", true);
    let stash = get_stashes(&repository);
    let desktop_entries = stash.desktop_entries;
    assert_eq!(desktop_entries.len(), 1);

    let readme = repository.join("README.md");
    std::fs::write(&readme, generate_string(DEFAULT_STRING_LENGTH)).unwrap();

    let entry_to_apply = &desktop_entries[0];
    let result = corvene_git::pop_stash_entry(git(), repository.path(), &entry_to_apply.sha);
    assert!(result.is_err());
}
