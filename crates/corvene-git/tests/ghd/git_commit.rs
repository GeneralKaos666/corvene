//! Port of GitHub Desktop's `app/test/unit/git/commit-test.ts`.
//!
//! Corvene equivalents of what the tests call:
//!
//! - `createCommit(repository, message, files, options)` (`lib/git/commit.ts`)
//!   is not one function in Corvene: `Dispatcher::commit`
//!   (`corvene-core/src/dispatcher.rs`) runs `corvene_git::unstage_all`,
//!   `stage_files` and `stage_partial_files` (together GHD's `stageFiles`,
//!   `lib/git/update-index.ts`) and `commit` in that order.
//!   `corvene_test_support::try_create_commit` (imported as `create_commit`)
//!   makes the same calls, always unstaging first as GHD does (the
//!   dispatcher skips the reset when every file is fully selected and
//!   nothing is conflicted), and returns what `corvene_git::commit` returns.
//! - `createMergeCommit(repository, files, manualResolutions)` is
//!   `corvene_git::create_merge_commit` (resolutions in a `BTreeMap`).
//! - `getCommits(repository, 'HEAD', n)` and `getCommit(repository, 'HEAD')`
//!   are `corvene_git::get_commits` (`corvene_test_support::get_commits` /
//!   `get_commit`), `getChangedFiles` is
//!   `corvene_git::get_changed_files`.
//! - `getWorkingDirectoryDiff(repository, file)` is
//!   `corvene_git::working_directory_diff` with GHD's settings: whitespace
//!   shown, and the flags `743-renamed-diff-against-head` and
//!   `749-binary-diff-as-text` off (their github-desktop preset value).
//! - `new WorkingDirectoryFileChange(path, { kind }, selection)`: GHD's
//!   `{ kind }` status has no porcelain columns, Corvene's `FileStatus` has.
//!   [`file_change`] takes them from `corvene_git::map_status` for the code
//!   git reports for that file in the `repo-with-changes` fixture (`??
//!   new-file.md`, `.M modified-file.md`, `.D deleted-file.md`) and sets
//!   the kind GHD's test gives.
//! - `DiffSelection.fromInitialSelection(All / None)`, `withRangeSelection`,
//!   `withLineSelection`, `withSelectAll`, `withSelectNone` are
//!   `DiffSelection::all() / none()`, `with_range`, `with_line`,
//!   `select_all`, `select_none`; `file.withSelection(s)` /
//!   `withIncludeAll(true)` / `status.workingDirectory.withIncludeAllFiles(true)`
//!   replace the selection ([`with_selection`], [`with_include_all`]).
//! - GHD's `DiffHunk.unifiedDiffEnd` is `unifiedDiffStart + lines.length - 1`
//!   (`lib/diff-parser.ts`); Corvene's `DiffHunk` has only the start
//!   (`corvene_test_support::unified_diff_end`). GHD's `DiffLine.text`
//!   starts with the `+` / `-` / space marker, which Corvene keeps in
//!   `DiffLine::kind`.
//! - GHD's `parentSHAs` is `Commit::parents`, `shortSha` is
//!   `Commit::short_sha()`, `status.currentTip` is
//!   `WorkingDirectoryStatus::current_tip`.
//! - A rejected git call's message (`GitError.message`, `lib/git/core.ts`):
//!   the description of a recognised error, else git's output
//!   (`corvene_test_support::git_error_message`, from
//!   `GitFailure::description` / `GitFailure::output`).

use std::collections::BTreeMap;
use std::path::Path;

use corvene_git::{CommitOptions, GitError};
use corvene_models::{
    ChangesetData, Diff, DiffHunk, DiffLineKind, DiffSelection, FileStatus, FileStatusKind,
    GitStatusEntry, ManualConflictResolution, WorkingDirectoryFileChange, WorkingDirectoryStatus,
};
use corvene_test_support::try_create_commit as create_commit;
use corvene_test_support::{
    TestRepo, exec, get_commit, get_commits, get_status_or_throw, git, git_error_message,
    setup_conflicted_repo, setup_conflicted_repo_with_multiple_files, setup_empty_repository,
    setup_fixture_repository, unified_diff_end,
};

/// GitHub Desktop's `createMergeCommit(repository, files, manualResolutions)`.
fn create_merge_commit(
    repository: &TestRepo,
    files: &[WorkingDirectoryFileChange],
    manual_resolutions: &[(&str, ManualConflictResolution)],
) -> Result<String, GitError> {
    let resolutions: BTreeMap<String, ManualConflictResolution> = manual_resolutions
        .iter()
        .map(|(path, resolution)| (path.to_string(), *resolution))
        .collect();
    corvene_git::create_merge_commit(git(), repository.path(), files, &resolutions)
}

/// GitHub Desktop's `getChangedFiles(repository, sha)`.
fn get_changed_files(repository: &TestRepo, sha: &str) -> ChangesetData {
    corvene_git::get_changed_files(git(), repository.path(), sha).expect("getChangedFiles")
}

/// The test file's `getTextDiff(repo, file)`: `getWorkingDirectoryDiff`,
/// asserting that it is a text diff.
fn get_text_diff(repository: &TestRepo, file: &WorkingDirectoryFileChange) -> Vec<DiffHunk> {
    let diff =
        corvene_git::working_directory_diff(git(), repository.path(), file, false, false, false)
            .expect("getWorkingDirectoryDiff");
    match diff {
        Diff::Text { hunks, .. } => hunks,
        other => panic!("expected a text diff, got {other:?}"),
    }
}

/// `new WorkingDirectoryFileChange(path, { kind }, selection)`, with the
/// porcelain columns of `code` (see the module doc).
fn file_change(
    path: &str,
    kind: FileStatusKind,
    code: &str,
    selection: DiffSelection,
) -> WorkingDirectoryFileChange {
    let mut status = corvene_git::map_status(code, "N...", None).expect("a status code");
    status.kind = kind;
    WorkingDirectoryFileChange {
        path: path.to_string(),
        old_path: None,
        status,
        selection,
    }
}

/// GitHub Desktop's `file.withSelection(selection)`.
fn with_selection(
    file: &WorkingDirectoryFileChange,
    selection: DiffSelection,
) -> WorkingDirectoryFileChange {
    WorkingDirectoryFileChange {
        selection,
        ..file.clone()
    }
}

/// GitHub Desktop's `file.withIncludeAll(true)`.
fn with_include_all(file: &WorkingDirectoryFileChange) -> WorkingDirectoryFileChange {
    with_selection(file, file.selection.select_all())
}

/// GitHub Desktop's `status.workingDirectory.withIncludeAllFiles(true)`.
fn with_include_all_files(status: &WorkingDirectoryStatus) -> Vec<WorkingDirectoryFileChange> {
    status.files.iter().map(with_include_all).collect()
}

/// `/There are no changes to commit./.test(message)`: the text followed by
/// any character but a line terminator.
fn matches_no_changes_to_commit(message: &str) -> bool {
    const TEXT: &str = "There are no changes to commit";
    message.match_indices(TEXT).any(|(ix, _)| {
        message[ix + TEXT.len()..]
            .chars()
            .next()
            .is_some_and(|c| !matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}'))
    })
}

fn write(repository: &TestRepo, relative: &str, contents: &str) {
    std::fs::write(repository.join(relative), contents).unwrap();
}

fn path_exists(path: &Path) -> bool {
    path.exists()
}

fn tracked(status: &WorkingDirectoryStatus) -> Vec<WorkingDirectoryFileChange> {
    status
        .files
        .iter()
        .filter(|f| f.status.kind != FileStatusKind::Untracked)
        .cloned()
        .collect()
}

// ---- createCommit normal ----

// GHD: unit/git/commit-test.ts › git/commit › createCommit normal › commits the given files
#[test]
fn commits_the_given_files() {
    let repository = setup_fixture_repository("test-repo");
    write(&repository, "README.md", "Hi world\n");

    let status = get_status_or_throw(&repository);
    let files = status.files;
    assert_eq!(files.len(), 1);

    let sha = create_commit(
        &repository,
        "Special commit",
        &files,
        &CommitOptions::default(),
    )
    .unwrap();
    assert_eq!(sha.len(), 7);

    let status = get_status_or_throw(&repository);
    let files = status.files;
    assert_eq!(files.len(), 0);

    let commits = get_commits(&repository, "HEAD", 100);
    assert_eq!(commits.len(), 6);
    assert_eq!(commits[0].summary, "Special commit");
    assert_eq!(&commits[0].sha[..7], sha);
}

// GHD: unit/git/commit-test.ts › git/commit › createCommit normal › commit does not strip commentary by default
#[test]
fn commit_does_not_strip_commentary_by_default() {
    let repository = setup_fixture_repository("test-repo");

    write(&repository, "README.md", "Hi world\n");

    let status = get_status_or_throw(&repository);
    let files = status.files;
    assert_eq!(files.len(), 1);

    let message = "Special commit

# this is a comment";

    let sha = create_commit(&repository, message, &files, &CommitOptions::default()).unwrap();
    assert_eq!(sha.len(), 7);

    let commit = get_commit(&repository, "HEAD");
    assert!(commit.is_some());
    let commit = commit.unwrap();
    assert_eq!(commit.summary, "Special commit");
    assert_eq!(commit.body, "# this is a comment\n");
    assert_eq!(commit.short_sha(), sha);
}

// GHD: unit/git/commit-test.ts › git/commit › createCommit normal › can commit for empty repository
#[test]
fn can_commit_for_empty_repository() {
    let repo = setup_empty_repository();

    write(&repo, "foo", "foo\n");
    write(&repo, "bar", "bar\n");

    let status = get_status_or_throw(&repo);
    let files = status.files;

    assert_eq!(files.len(), 2);

    let all_changes = [with_include_all(&files[0]), with_include_all(&files[1])];

    let sha = create_commit(
        &repo,
        "added two files\n\nthis is a description",
        &all_changes,
        &CommitOptions::default(),
    )
    .unwrap();
    assert_eq!(sha, "(root-commit)");

    let status_after = get_status_or_throw(&repo);

    assert_eq!(status_after.files.len(), 0);

    let history = get_commits(&repo, "HEAD", 2);

    assert_eq!(history.len(), 1);
    assert_eq!(history[0].summary, "added two files");
    assert_eq!(history[0].body, "this is a description\n");
}

// GHD: unit/git/commit-test.ts › git/commit › createCommit normal › can commit renames
#[test]
fn can_commit_renames() {
    let repo = setup_empty_repository();

    write(&repo, "foo", "foo\n");

    exec(["add", "foo"], repo.path());
    exec(["commit", "-m", "Initial commit"], repo.path());
    exec(["mv", "foo", "bar"], repo.path());

    let status = get_status_or_throw(&repo);
    let files = status.files;

    assert_eq!(files.len(), 1);

    let sha = create_commit(
        &repo,
        "renamed a file",
        &[with_include_all(&files[0])],
        &CommitOptions::default(),
    )
    .unwrap();
    assert_eq!(sha.len(), 7);

    let status_after = get_status_or_throw(&repo);

    assert_eq!(status_after.files.len(), 0);
}

// ---- createCommit partials ----

// GHD: unit/git/commit-test.ts › git/commit › createCommit partials › can commit some lines from new file
#[test]
fn can_commit_some_lines_from_new_file() {
    let repository = setup_fixture_repository("repo-with-changes");

    let previous_tip = get_commits(&repository, "HEAD", 1).remove(0);

    let new_file_name = "new-file.md";

    // select first five lines of file
    let selection = DiffSelection::none().with_range(0, 5, true);

    let file = file_change(new_file_name, FileStatusKind::New, "??", selection);

    // commit just this change, ignore everything else
    let sha = create_commit(&repository, "title", &[file], &CommitOptions::default()).unwrap();
    assert_eq!(sha.len(), 7);

    // verify that the HEAD of the repository has moved
    let new_tip = get_commits(&repository, "HEAD", 1).remove(0);
    assert_ne!(new_tip.sha, previous_tip.sha);
    assert_eq!(new_tip.summary, "title");
    assert_eq!(new_tip.short_sha(), sha);

    // verify that the contents of this new commit are just the new file
    let changeset_data = get_changed_files(&repository, &new_tip.sha);
    assert_eq!(changeset_data.files.len(), 1);
    assert_eq!(changeset_data.files[0].path, new_file_name);

    // verify that changes remain for this new file
    let status = get_status_or_throw(&repository);
    assert_eq!(status.files.len(), 4);

    // verify that the file is now tracked
    let file_change = status.files.iter().find(|f| f.path == new_file_name);
    assert!(file_change.is_some());
    assert_eq!(file_change.unwrap().status.kind, FileStatusKind::Modified);
}

// GHD: unit/git/commit-test.ts › git/commit › createCommit partials › can commit second hunk from modified file
#[test]
fn can_commit_second_hunk_from_modified_file() {
    let repository = setup_fixture_repository("repo-with-changes");

    let previous_tip = get_commits(&repository, "HEAD", 1).remove(0);

    let modified_file = "modified-file.md";

    let unselected_file = DiffSelection::none();
    let file = file_change(
        modified_file,
        FileStatusKind::Modified,
        ".M",
        unselected_file,
    );

    let diff = get_text_diff(&repository, &file);

    let selection = DiffSelection::all().with_range(
        diff[0].unified_diff_start,
        unified_diff_end(&diff[0]) - diff[0].unified_diff_start,
        false,
    );

    let updated_file = with_selection(&file, selection);

    // commit just this change, ignore everything else
    let sha = create_commit(
        &repository,
        "title",
        &[updated_file],
        &CommitOptions::default(),
    )
    .unwrap();
    assert_eq!(sha.len(), 7);

    // verify that the HEAD of the repository has moved
    let new_tip = get_commits(&repository, "HEAD", 1).remove(0);
    assert_ne!(new_tip.sha, previous_tip.sha);
    assert_eq!(new_tip.summary, "title");

    // verify that the contents of this new commit are just the modified file
    let changeset_data = get_changed_files(&repository, &new_tip.sha);
    assert_eq!(changeset_data.files.len(), 1);
    assert_eq!(changeset_data.files[0].path, modified_file);

    // verify that changes remain for this modified file
    let status = get_status_or_throw(&repository);
    assert_eq!(status.files.len(), 4);

    // verify that the file is still marked as modified
    let file_change = status.files.iter().find(|f| f.path == modified_file);
    assert!(file_change.is_some());
    assert_eq!(file_change.unwrap().status.kind, FileStatusKind::Modified);
}

// GHD: unit/git/commit-test.ts › git/commit › createCommit partials › can commit single delete from modified file
#[test]
fn can_commit_single_delete_from_modified_file() {
    let repository = setup_fixture_repository("repo-with-changes");

    let previous_tip = get_commits(&repository, "HEAD", 1).remove(0);

    let file_name = "modified-file.md";

    let unselected_file = DiffSelection::none();
    let modified_file = file_change(file_name, FileStatusKind::Modified, ".M", unselected_file);

    let diff = get_text_diff(&repository, &modified_file);

    let second_removed_line = diff[0].unified_diff_start + 5;

    let selection = DiffSelection::none().with_range(second_removed_line, 1, true);

    let file = file_change(file_name, FileStatusKind::Modified, ".M", selection);

    // commit just this change, ignore everything else
    let sha = create_commit(&repository, "title", &[file], &CommitOptions::default()).unwrap();
    assert_eq!(sha.len(), 7);

    // verify that the HEAD of the repository has moved
    let new_tip = get_commits(&repository, "HEAD", 1).remove(0);
    assert_ne!(new_tip.sha, previous_tip.sha);
    assert_eq!(new_tip.summary, "title");
    assert_eq!(new_tip.short_sha(), sha);

    // verify that the contents of this new commit are just the modified file
    let changeset_data = get_changed_files(&repository, &new_tip.sha);
    assert_eq!(changeset_data.files.len(), 1);
    assert_eq!(changeset_data.files[0].path, file_name);
}

// GHD: unit/git/commit-test.ts › git/commit › createCommit partials › can commit multiple hunks from modified file
#[test]
fn can_commit_multiple_hunks_from_modified_file() {
    let repository = setup_fixture_repository("repo-with-changes");

    let previous_tip = get_commits(&repository, "HEAD", 1).remove(0);

    let modified_file = "modified-file.md";

    let unselected_file = DiffSelection::none();
    let file = file_change(
        modified_file,
        FileStatusKind::Modified,
        ".M",
        unselected_file,
    );

    let diff = get_text_diff(&repository, &file);

    let selection = DiffSelection::all().with_range(
        diff[1].unified_diff_start,
        unified_diff_end(&diff[1]) - diff[1].unified_diff_start,
        false,
    );

    let updated_file = file_change(modified_file, FileStatusKind::Modified, ".M", selection);

    // commit just this change, ignore everything else
    let sha = create_commit(
        &repository,
        "title",
        &[updated_file],
        &CommitOptions::default(),
    )
    .unwrap();
    assert_eq!(sha.len(), 7);

    // verify that the HEAD of the repository has moved
    let new_tip = get_commits(&repository, "HEAD", 1).remove(0);
    assert_ne!(new_tip.sha, previous_tip.sha);
    assert_eq!(new_tip.summary, "title");
    assert_eq!(new_tip.short_sha(), sha);

    // verify that the contents of this new commit are just the modified file
    let changeset_data = get_changed_files(&repository, &new_tip.sha);
    assert_eq!(changeset_data.files.len(), 1);
    assert_eq!(changeset_data.files[0].path, modified_file);

    // verify that changes remain for this modified file
    let status = get_status_or_throw(&repository);
    assert_eq!(status.files.len(), 4);

    // verify that the file is still marked as modified
    let file_change = status.files.iter().find(|f| f.path == modified_file);
    assert!(file_change.is_some());
    assert_eq!(file_change.unwrap().status.kind, FileStatusKind::Modified);
}

// GHD: unit/git/commit-test.ts › git/commit › createCommit partials › can commit some lines from deleted file
#[test]
fn can_commit_some_lines_from_deleted_file() {
    let repository = setup_fixture_repository("repo-with-changes");

    let previous_tip = get_commits(&repository, "HEAD", 1).remove(0);

    let deleted_file = "deleted-file.md";

    let selection = DiffSelection::none().with_range(0, 5, true);

    let file = file_change(deleted_file, FileStatusKind::Deleted, ".D", selection);

    // commit just this change, ignore everything else
    let sha = create_commit(&repository, "title", &[file], &CommitOptions::default()).unwrap();
    assert_eq!(sha.len(), 7);

    // verify that the HEAD of the repository has moved
    let new_tip = get_commits(&repository, "HEAD", 1).remove(0);
    assert_ne!(new_tip.sha, previous_tip.sha);
    assert_eq!(new_tip.summary, "title");
    assert_eq!(&new_tip.sha[..7], sha);

    // verify that the contents of this new commit are just the new file
    let changeset_data = get_changed_files(&repository, &new_tip.sha);
    assert_eq!(changeset_data.files.len(), 1);
    assert_eq!(changeset_data.files[0].path, deleted_file);

    // verify that changes remain for this new file
    let status = get_status_or_throw(&repository);
    assert_eq!(status.files.len(), 4);

    // verify that the file is now tracked
    let file_change = status.files.iter().find(|f| f.path == deleted_file);
    assert!(file_change.is_some());
    assert_eq!(file_change.unwrap().status.kind, FileStatusKind::Deleted);
}

// GHD: unit/git/commit-test.ts › git/commit › createCommit partials › can commit renames with modifications
#[test]
fn can_commit_renames_with_modifications() {
    let repo = setup_empty_repository();

    write(&repo, "foo", "foo\n");

    exec(["add", "foo"], repo.path());
    exec(["commit", "-m", "Initial commit"], repo.path());
    exec(["mv", "foo", "bar"], repo.path());

    write(&repo, "bar", "bar\n");

    let status = get_status_or_throw(&repo);
    let files = status.files;

    assert_eq!(files.len(), 1);

    let sha = create_commit(
        &repo,
        "renamed a file",
        &[with_include_all(&files[0])],
        &CommitOptions::default(),
    )
    .unwrap();
    assert_eq!(sha.len(), 7);

    let status_after = get_status_or_throw(&repo);
    assert!(status_after.current_tip.is_some());

    assert_eq!(status_after.files.len(), 0);
    assert_eq!(&status_after.current_tip.unwrap()[..7], sha);
}

// The scenario here is that the user has staged a rename (probably using git mv)
// and then added some lines to the newly renamed file and they only want to
// commit one of these lines.
// GHD: unit/git/commit-test.ts › git/commit › createCommit partials › can commit renames with partially selected modifications
#[test]
fn can_commit_renames_with_partially_selected_modifications() {
    let repo = setup_empty_repository();

    write(&repo, "foo", "line1\n");

    exec(["add", "foo"], repo.path());
    exec(["commit", "-m", "Initial commit"], repo.path());
    exec(["mv", "foo", "bar"], repo.path());

    write(&repo, "bar", "line1\nline2\nline3\n");

    let status = get_status_or_throw(&repo);
    let files = status.files;

    assert_eq!(files.len(), 1);
    assert!(files[0].path.contains("bar"));
    assert_eq!(files[0].status.kind, FileStatusKind::Renamed);

    let selection = files[0].selection.select_none().with_line(2, true);

    let partially_selected_file = with_selection(&files[0], selection);

    let sha = create_commit(
        &repo,
        "renamed a file",
        &[partially_selected_file],
        &CommitOptions::default(),
    )
    .unwrap();
    assert_eq!(sha.len(), 7);

    let status_after = get_status_or_throw(&repo);

    assert_eq!(status_after.files.len(), 1);

    let diff = get_text_diff(&repo, &status_after.files[0]);

    assert_eq!(diff.len(), 1);
    assert_eq!(diff[0].lines.len(), 4);
    // GHD: `lines[3].text === '+line3'` (the marker is Corvene's `kind`)
    assert_eq!(diff[0].lines[3].kind, DiffLineKind::Add);
    assert_eq!(diff[0].lines[3].text, "line3");
}

// ---- createCommit with a merge conflict ----

// GHD: unit/git/commit-test.ts › git/commit › createCommit with a merge conflict › creates a merge commit
#[test]
fn create_commit_with_a_merge_conflict_creates_a_merge_commit() {
    let repo = setup_conflicted_repo();
    let file_path = repo.join("foo");

    let in_merge = path_exists(&repo.join(".git").join("MERGE_HEAD"));
    assert!(in_merge);

    std::fs::write(&file_path, "b1b2").unwrap();

    let status = get_status_or_throw(&repo);
    let files = status.files;

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, "foo");

    // GHD: `{ kind: Conflicted, entry: { kind: 'conflicted', action:
    // BothModified, them: UpdatedButUnmerged, us: UpdatedButUnmerged,
    // submoduleStatus: undefined }, conflictMarkerCount: 0 }`; `UU` is
    // `BothModified`, `us` / `them` are Corvene's `index` / `working_tree`
    assert_eq!(
        files[0].status,
        FileStatus {
            kind: FileStatusKind::Conflicted,
            index: GitStatusEntry::Unmerged,
            working_tree: GitStatusEntry::Unmerged,
            score: None,
            code: "UU".to_string(),
            submodule: false,
            submodule_status: None,
            conflict_markers: Some(0),
        }
    );

    let selection = files[0].selection.select_all();
    let selected_file = with_selection(&files[0], selection);
    let sha = create_commit(
        &repo,
        "Merge commit!",
        &[selected_file],
        &CommitOptions::default(),
    )
    .unwrap();
    assert_eq!(sha.len(), 7);

    let commits = get_commits(&repo, "HEAD", 5);
    assert_eq!(commits[0].parents.len(), 2);
    assert_eq!(commits[0].short_sha(), sha);
}

// ---- createMergeCommit ----

// GHD: unit/git/commit-test.ts › git/commit › createMergeCommit › with a simple merge conflict › with a merge conflict › creates a merge commit
#[test]
fn create_merge_commit_with_a_simple_merge_conflict_creates_a_merge_commit() {
    let repository = setup_conflicted_repo();

    let status = get_status_or_throw(&repository);
    let tracked_files = tracked(&status);
    let sha = create_merge_commit(&repository, &tracked_files, &[]).unwrap();
    let new_status = get_status_or_throw(&repository);
    assert_eq!(sha.len(), 7);
    assert_eq!(new_status.files.len(), 0);
}

// GHD: unit/git/commit-test.ts › git/commit › createMergeCommit › with a merge conflict and manual resolutions › keeps files chosen to be added and commits
#[test]
fn keeps_files_chosen_to_be_added_and_commits() {
    let repository = setup_conflicted_repo_with_multiple_files();

    let status = get_status_or_throw(&repository);
    let tracked_files = tracked(&status);
    let manual_resolutions = [("bar", ManualConflictResolution::Ours)];
    let sha = create_merge_commit(&repository, &tracked_files, &manual_resolutions).unwrap();
    assert!(path_exists(&repository.join("bar")));
    let new_status = get_status_or_throw(&repository);
    assert_eq!(sha.len(), 7);
    assert_eq!(new_status.files.len(), 1);
}

// GHD: unit/git/commit-test.ts › git/commit › createMergeCommit › with a merge conflict and manual resolutions › deletes files chosen to be removed and commits
#[test]
fn deletes_files_chosen_to_be_removed_and_commits() {
    let repository = setup_conflicted_repo_with_multiple_files();

    let status = get_status_or_throw(&repository);
    let tracked_files = tracked(&status);
    let manual_resolutions = [("bar", ManualConflictResolution::Theirs)];
    let sha = create_merge_commit(&repository, &tracked_files, &manual_resolutions).unwrap();
    assert!(!path_exists(&repository.join("bar")));
    let new_status = get_status_or_throw(&repository);
    assert_eq!(sha.len(), 7);
    assert_eq!(new_status.files.len(), 1);
}

// GHD: unit/git/commit-test.ts › git/commit › createMergeCommit › with a merge conflict and manual resolutions › checks out our content for file added in both branches
#[test]
fn checks_out_our_content_for_file_added_in_both_branches() {
    let repository = setup_conflicted_repo_with_multiple_files();

    let status = get_status_or_throw(&repository);
    let tracked_files = tracked(&status);
    let manual_resolutions = [("baz", ManualConflictResolution::Ours)];
    let sha = create_merge_commit(&repository, &tracked_files, &manual_resolutions).unwrap();
    assert_eq!(
        std::fs::read_to_string(repository.join("baz")).unwrap(),
        "b2"
    );
    let new_status = get_status_or_throw(&repository);
    assert_eq!(sha.len(), 7);
    assert_eq!(new_status.files.len(), 1);
}

// GHD: unit/git/commit-test.ts › git/commit › createMergeCommit › with a merge conflict and manual resolutions › checks out their content for file added in both branches
#[test]
fn checks_out_their_content_for_file_added_in_both_branches() {
    let repository = setup_conflicted_repo_with_multiple_files();

    let status = get_status_or_throw(&repository);
    let tracked_files = tracked(&status);
    let manual_resolutions = [("baz", ManualConflictResolution::Theirs)];
    let sha = create_merge_commit(&repository, &tracked_files, &manual_resolutions).unwrap();
    assert_eq!(
        std::fs::read_to_string(repository.join("baz")).unwrap(),
        "b1"
    );
    let new_status = get_status_or_throw(&repository);
    assert_eq!(sha.len(), 7);
    assert_eq!(new_status.files.len(), 1);
}

/// The `binary file conflicts` describe's `setup`: the
/// `detect-conflict-in-binary-file` fixture on `make-a-change`, with
/// `my-cool-image.png` as on `master` (theirs) and as on `make-a-change`
/// (ours), read as UTF-8 like GHD's `readFile(…, 'utf8')`.
fn binary_conflict_setup() -> (TestRepo, String, String) {
    let repository = setup_fixture_repository("detect-conflict-in-binary-file");
    let file_name = "my-cool-image.png";

    exec(["checkout", "master"], repository.path());

    let file_contents_theirs = read_utf8(&repository.join(file_name));

    exec(["checkout", "make-a-change"], repository.path());

    let file_contents_ours = read_utf8(&repository.join(file_name));

    (repository, file_contents_theirs, file_contents_ours)
}

/// Node's `readFile(path, 'utf8')`: invalid sequences become U+FFFD.
fn read_utf8(path: &Path) -> String {
    String::from_utf8_lossy(&std::fs::read(path).unwrap()).into_owned()
}

// GHD: unit/git/commit-test.ts › git/commit › createMergeCommit › with a merge conflict and manual resolutions › binary file conflicts › chooses `their` version of a file and commits
#[test]
fn chooses_their_version_of_a_file_and_commits() {
    let (repository, file_contents_theirs, file_contents_ours) = binary_conflict_setup();

    exec(["merge", "master"], repository.path());

    let status = get_status_or_throw(&repository);
    let files = status.files.clone();
    assert_eq!(files.len(), 1);

    let file = &files[0];
    assert_eq!(file.status.kind, FileStatusKind::Conflicted);
    assert!(file.status.kind == FileStatusKind::Conflicted && file.status.is_manual_conflict());

    let tracked_files = tracked(&status);

    let manual_resolutions = [(file.path.as_str(), ManualConflictResolution::Theirs)];
    create_merge_commit(&repository, &tracked_files, &manual_resolutions).unwrap();

    let file_contents = read_utf8(&repository.join(&file.path));

    assert_ne!(file_contents, file_contents_ours);
    assert_eq!(file_contents, file_contents_theirs);
}

// GHD: unit/git/commit-test.ts › git/commit › createMergeCommit › with a merge conflict and manual resolutions › binary file conflicts › chooses `our` version of a file and commits
#[test]
fn chooses_our_version_of_a_file_and_commits() {
    let (repository, _, file_contents_ours) = binary_conflict_setup();

    exec(["merge", "master"], repository.path());

    let status = get_status_or_throw(&repository);
    let files = status.files.clone();
    assert_eq!(files.len(), 1);

    let file = &files[0];
    assert_eq!(file.status.kind, FileStatusKind::Conflicted);
    assert!(file.status.kind == FileStatusKind::Conflicted && file.status.is_manual_conflict());

    let tracked_files = tracked(&status);

    let manual_resolutions = [(file.path.as_str(), ManualConflictResolution::Ours)];
    create_merge_commit(&repository, &tracked_files, &manual_resolutions).unwrap();

    let file_contents = read_utf8(&repository.join(&file.path));

    assert_eq!(file_contents, file_contents_ours);
}

// GHD: unit/git/commit-test.ts › git/commit › createMergeCommit › with no changes › throws an error
#[test]
fn create_merge_commit_with_no_changes_throws_an_error() {
    let repository = setup_fixture_repository("test-repo");
    let status = get_status_or_throw(&repository);
    let err = create_merge_commit(&repository, &status.files, &[])
        .expect_err("createMergeCommit should reject");
    let message = git_error_message(&err);
    assert!(
        matches_no_changes_to_commit(&message),
        "{message:?} does not match /There are no changes to commit./"
    );
}

// ---- index corner cases ----

// GHD: unit/git/commit-test.ts › git/commit › index corner cases › can commit when staged new file is then deleted
#[test]
fn can_commit_when_staged_new_file_is_then_deleted() {
    let repo = setup_empty_repository();

    let first_path = repo.join("first");
    let second_path = repo.join("second");

    std::fs::write(&first_path, "line1\n").unwrap();
    std::fs::write(&second_path, "line2\n").unwrap();

    exec(["add", "."], repo.path());

    std::fs::remove_file(&first_path).unwrap();

    let status = get_status_or_throw(&repo);
    let files = &status.files;

    assert_eq!(files.len(), 1);
    assert!(files[0].path.contains("second"));
    assert_eq!(files[0].status.kind, FileStatusKind::New);

    let to_commit = with_include_all_files(&status);

    let sha = create_commit(
        &repo,
        "commit everything",
        &to_commit,
        &CommitOptions::default(),
    )
    .unwrap();
    assert_eq!(sha, "(root-commit)");

    let status = get_status_or_throw(&repo);
    let files = &status.files;
    assert_eq!(files.len(), 0);

    let commit = get_commit(&repo, "HEAD");
    assert!(commit.is_some());
    assert_eq!(commit.unwrap().summary, "commit everything");
}

// GHD: unit/git/commit-test.ts › git/commit › index corner cases › can commit when a delete is staged and the untracked file exists
#[test]
fn can_commit_when_a_delete_is_staged_and_the_untracked_file_exists() {
    let repo = setup_empty_repository();

    let first_path = repo.join("first");
    std::fs::write(&first_path, "line1\n").unwrap();

    exec(["add", "first"], repo.path());
    exec(["commit", "-am", "commit first file"], repo.path());
    exec(["rm", "--cached", "first"], repo.path());

    // if the text is now different, everything is fine
    std::fs::write(&first_path, "line2\n").unwrap();

    let status = get_status_or_throw(&repo);
    let files = &status.files;

    assert_eq!(files.len(), 1);
    assert!(files[0].path.contains("first"));
    assert_eq!(files[0].status.kind, FileStatusKind::Untracked);

    let to_commit = with_include_all_files(&status);

    let sha = create_commit(
        &repo,
        "commit again!",
        &to_commit,
        &CommitOptions::default(),
    )
    .unwrap();
    assert_eq!(sha.len(), 7);

    let status = get_status_or_throw(&repo);
    let files = &status.files;
    assert_eq!(files.len(), 0);

    let commit = get_commit(&repo, "HEAD");
    assert!(commit.is_some());
    let commit = commit.unwrap();
    assert_eq!(commit.summary, "commit again!");
    assert_eq!(commit.short_sha(), sha);
}

// GHD: unit/git/commit-test.ts › git/commit › index corner cases › file is deleted in index
#[test]
fn file_is_deleted_in_index() {
    let repo = setup_empty_repository();
    write(&repo, "secret", "contents\n");
    write(&repo, ".gitignore", "");

    // Setup repo to reproduce bug
    exec(["add", "."], repo.path());
    exec(["commit", "-m", "Initial commit"], repo.path());

    // Make changes that should remain secret
    write(&repo, "secret", "Somethign secret\n");

    // Ignore it
    write(&repo, ".gitignore", "secret");

    // Remove from index to mark as deleted
    exec(["rm", "--cached", "secret"], repo.path());

    // Make sure that file is marked as deleted
    let before_commit = get_status_or_throw(&repo);
    let files = &before_commit.files;
    assert_eq!(files.len(), 2);
    assert_eq!(files[1].status.kind, FileStatusKind::Deleted);

    // Commit changes
    create_commit(&repo, "FAIL commit", files, &CommitOptions::default()).unwrap();
    let after_commit = get_status_or_throw(&repo);
    assert!(after_commit.current_tip.is_some());
    assert_ne!(before_commit.current_tip, after_commit.current_tip);

    // Verify the file was delete in repo
    let changeset_data = get_changed_files(&repo, after_commit.current_tip.as_deref().unwrap());
    assert_eq!(changeset_data.files.len(), 2);
    assert_eq!(
        changeset_data.files[0].status.kind,
        FileStatusKind::Modified
    );
    assert_eq!(changeset_data.files[1].status.kind, FileStatusKind::Deleted);
}

// ---- createCommit allowEmpty ----

// GHD: unit/git/commit-test.ts › git/commit › createCommit allowEmpty › creates an empty commit when allowEmpty is true
#[test]
fn creates_an_empty_commit_when_allow_empty_is_true() {
    let repo = setup_empty_repository();

    // Create an initial commit so HEAD exists
    write(&repo, "file.txt", "content\n");
    let initial_status = get_status_or_throw(&repo);
    create_commit(
        &repo,
        "initial commit",
        &initial_status.files,
        &CommitOptions::default(),
    )
    .unwrap();

    // Now create an empty commit with no file changes
    let tip_before = get_status_or_throw(&repo).current_tip;
    let sha = create_commit(
        &repo,
        "empty commit",
        &[],
        &CommitOptions {
            allow_empty: true,
            ..CommitOptions::default()
        },
    )
    .unwrap();
    assert_eq!(sha.len(), 7);

    let tip_after = get_status_or_throw(&repo).current_tip;
    assert_ne!(tip_before, tip_after);

    let commit = get_commit(&repo, "HEAD");
    assert!(commit.is_some());
    assert_eq!(commit.unwrap().summary, "empty commit");
}

// GHD: unit/git/commit-test.ts › git/commit › createCommit allowEmpty › fails to create an empty commit when allowEmpty is not set
#[test]
fn fails_to_create_an_empty_commit_when_allow_empty_is_not_set() {
    let repo = setup_empty_repository();

    // Create an initial commit so HEAD exists
    write(&repo, "file.txt", "content\n");
    let initial_status = get_status_or_throw(&repo);
    create_commit(
        &repo,
        "initial commit",
        &initial_status.files,
        &CommitOptions::default(),
    )
    .unwrap();

    // Attempt to commit with no changes and no allowEmpty flag
    assert!(create_commit(&repo, "should fail", &[], &CommitOptions::default()).is_err());
}
