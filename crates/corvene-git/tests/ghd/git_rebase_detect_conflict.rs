//! Port of GitHub Desktop's `app/test/unit/git/rebase/detect-conflict-test.ts`.
//!
//! - `rebase`, `abortRebase`, `continueRebase` (`lib/git/rebase.ts`) are
//!   `crate::rebase_support::{rebase, abort_rebase, continue_rebase}` over
//!   `corvene_git::rebase` / `abort_rebase` / `continue_rebase`.
//! - `getChangedFiles(repository, sha)` is `corvene_git::get_changed_files`.
//! - GitHub Desktop's status fields are `WorkingDirectoryStatus`'s:
//!   `rebaseInternalState` is `rebase_internal_state` (`None` is `null`),
//!   `workingDirectory.files` is `files`, `currentBranch` is `branch`
//!   (`None` is `undefined`), `currentTip` is `current_tip`;
//!   `AppFileStatusKind.Conflicted` is `FileStatusKind::Conflicted`.
//! - Each `describe`'s `setup` is a function every test of it calls.

use std::collections::BTreeMap;

use corvene_git::RebaseResult;
use corvene_models::{FileStatusKind, RebaseInternalState, WorkingDirectoryStatus};
use corvene_test_support::{
    exec, get_branch_or_error, get_status_or_throw, git, repository_builder_rebase,
};

use crate::rebase_support::{abort_rebase, continue_rebase, rebase};

const BASE_BRANCH_NAME: &str = "base-branch";
const FEATURE_BRANCH_NAME: &str = "this-is-a-feature";

fn write_file(path: std::path::PathBuf, contents: &str) {
    std::fs::write(&path, contents).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
}

fn conflicted_count(status: &WorkingDirectoryStatus) -> usize {
    status
        .files
        .iter()
        .filter(|f| f.status.kind == FileStatusKind::Conflicted)
        .count()
}

// ---------------------------------------------------------------------------
// detect conflicts
// ---------------------------------------------------------------------------

struct DetectConflicts {
    result: RebaseResult,
    status: WorkingDirectoryStatus,
    original_branch_tip: String,
    base_branch_tip: String,
}

fn setup_detect_conflicts() -> DetectConflicts {
    let repository =
        repository_builder_rebase::create_repository(BASE_BRANCH_NAME, FEATURE_BRANCH_NAME);

    let feature_branch = get_branch_or_error(&repository, FEATURE_BRANCH_NAME);
    let original_branch_tip = feature_branch.tip.clone().expect("feature branch tip");

    let base_branch = get_branch_or_error(&repository, BASE_BRANCH_NAME);
    let base_branch_tip = base_branch.tip.clone().expect("base branch tip");

    let result = rebase(&repository, &base_branch, &feature_branch, None);

    let status = get_status_or_throw(&repository);

    DetectConflicts {
        result,
        status,
        original_branch_tip,
        base_branch_tip,
    }
}

// GHD: unit/git/rebase/detect-conflict-test.ts › git/rebase › detect conflicts › returns a value indicating conflicts were encountered
#[test]
fn detect_conflicts_returns_a_value_indicating_conflicts_were_encountered() {
    let DetectConflicts { result, .. } = setup_detect_conflicts();
    assert_eq!(result, RebaseResult::ConflictsEncountered);
}

// GHD: unit/git/rebase/detect-conflict-test.ts › git/rebase › detect conflicts › status detects REBASE_HEAD
#[test]
fn detect_conflicts_status_detects_rebase_head() {
    let DetectConflicts {
        original_branch_tip,
        base_branch_tip,
        status,
        ..
    } = setup_detect_conflicts();

    assert_eq!(
        status.rebase_internal_state,
        Some(RebaseInternalState {
            original_branch_tip,
            base_branch_tip,
            target_branch: "this-is-a-feature".into(),
        })
    );
}

// GHD: unit/git/rebase/detect-conflict-test.ts › git/rebase › detect conflicts › has conflicted files in working directory
#[test]
fn detect_conflicts_has_conflicted_files_in_working_directory() {
    let DetectConflicts { status, .. } = setup_detect_conflicts();

    assert_eq!(conflicted_count(&status), 2);
}

// GHD: unit/git/rebase/detect-conflict-test.ts › git/rebase › detect conflicts › is a detached HEAD state
#[test]
fn detect_conflicts_is_a_detached_head_state() {
    let DetectConflicts { status, .. } = setup_detect_conflicts();
    assert!(status.branch.is_none());
}

// ---------------------------------------------------------------------------
// abort after conflicts found
// ---------------------------------------------------------------------------

fn setup_abort_after_conflicts_found() -> WorkingDirectoryStatus {
    let repository =
        repository_builder_rebase::create_repository(BASE_BRANCH_NAME, FEATURE_BRANCH_NAME);

    let feature_branch = get_branch_or_error(&repository, FEATURE_BRANCH_NAME);

    let base_branch = get_branch_or_error(&repository, BASE_BRANCH_NAME);

    rebase(&repository, &base_branch, &feature_branch, None);

    abort_rebase(&repository);

    get_status_or_throw(&repository)
}

// GHD: unit/git/rebase/detect-conflict-test.ts › git/rebase › abort after conflicts found › REBASE_HEAD is no longer found
#[test]
fn abort_after_conflicts_found_rebase_head_is_no_longer_found() {
    let status = setup_abort_after_conflicts_found();
    assert!(status.rebase_internal_state.is_none());
}

// GHD: unit/git/rebase/detect-conflict-test.ts › git/rebase › abort after conflicts found › no longer has working directory changes
#[test]
fn abort_after_conflicts_found_no_longer_has_working_directory_changes() {
    let status = setup_abort_after_conflicts_found();
    assert_eq!(status.files.len(), 0);
}

// GHD: unit/git/rebase/detect-conflict-test.ts › git/rebase › abort after conflicts found › returns to the feature branch
#[test]
fn abort_after_conflicts_found_returns_to_the_feature_branch() {
    let status = setup_abort_after_conflicts_found();
    assert_eq!(status.branch.as_deref(), Some(FEATURE_BRANCH_NAME));
}

// ---------------------------------------------------------------------------
// attempt to continue without resolving conflicts
// ---------------------------------------------------------------------------

fn setup_continue_without_resolving() -> DetectConflicts {
    let repository =
        repository_builder_rebase::create_repository(BASE_BRANCH_NAME, FEATURE_BRANCH_NAME);

    let feature_branch = get_branch_or_error(&repository, FEATURE_BRANCH_NAME);
    let original_branch_tip = feature_branch.tip.clone().expect("feature branch tip");

    let base_branch = get_branch_or_error(&repository, BASE_BRANCH_NAME);
    let base_branch_tip = base_branch.tip.clone().expect("base branch tip");

    rebase(&repository, &base_branch, &feature_branch, None);

    // the second parameter here represents files that the UI indicates have
    // no conflict markers, so can be safely staged before continuing the
    // rebase
    let result = continue_rebase(&repository, &[], &BTreeMap::new(), None);

    let status = get_status_or_throw(&repository);

    DetectConflicts {
        result,
        status,
        original_branch_tip,
        base_branch_tip,
    }
}

// GHD: unit/git/rebase/detect-conflict-test.ts › git/rebase › attempt to continue without resolving conflicts › indicates that the rebase was not complete
#[test]
fn continue_without_resolving_indicates_that_the_rebase_was_not_complete() {
    let DetectConflicts { result, .. } = setup_continue_without_resolving();
    assert_eq!(result, RebaseResult::OutstandingFilesNotStaged);
}

// GHD: unit/git/rebase/detect-conflict-test.ts › git/rebase › attempt to continue without resolving conflicts › REBASE_HEAD is still found
#[test]
fn continue_without_resolving_rebase_head_is_still_found() {
    let DetectConflicts {
        status,
        original_branch_tip,
        base_branch_tip,
        ..
    } = setup_continue_without_resolving();
    assert_eq!(
        status.rebase_internal_state,
        Some(RebaseInternalState {
            original_branch_tip,
            base_branch_tip,
            target_branch: "this-is-a-feature".into(),
        })
    );
}

// GHD: unit/git/rebase/detect-conflict-test.ts › git/rebase › attempt to continue without resolving conflicts › still has conflicted files in working directory
#[test]
fn continue_without_resolving_still_has_conflicted_files_in_working_directory() {
    let DetectConflicts { status, .. } = setup_continue_without_resolving();
    assert_eq!(conflicted_count(&status), 2);
}

// ---------------------------------------------------------------------------
// continue after resolving conflicts
// ---------------------------------------------------------------------------

struct ContinueAfterResolving {
    result: RebaseResult,
    status: WorkingDirectoryStatus,
    before_rebase_tip: String,
}

fn setup_continue_after_resolving() -> ContinueAfterResolving {
    let repository =
        repository_builder_rebase::create_repository(BASE_BRANCH_NAME, FEATURE_BRANCH_NAME);

    let feature_branch = get_branch_or_error(&repository, FEATURE_BRANCH_NAME);
    let before_rebase_tip = feature_branch.tip.clone().expect("feature branch tip");

    let base_branch = get_branch_or_error(&repository, BASE_BRANCH_NAME);

    rebase(&repository, &base_branch, &feature_branch, None);

    let after_rebase = get_status_or_throw(&repository);

    let files = after_rebase.files;

    let diff_check_before = exec(["diff", "--check"], repository.path());

    assert!(diff_check_before.exit_code > 0);

    // resolve conflicts by writing files to disk
    write_file(
        repository.join("THING.md"),
        "# HELLO WORLD! \nTHINGS GO HERE\nFEATURE BRANCH UNDERWAY\n",
    );

    write_file(
        repository.join("OTHER.md"),
        "# HELLO WORLD! \nTHINGS GO HERE\nALSO FEATURE BRANCH UNDERWAY\n",
    );

    let diff_check_after = exec(["diff", "--check"], repository.path());

    assert_eq!(diff_check_after.exit_code, 0);

    let result = continue_rebase(&repository, &files, &BTreeMap::new(), None);

    let status = get_status_or_throw(&repository);

    ContinueAfterResolving {
        result,
        status,
        before_rebase_tip,
    }
}

// GHD: unit/git/rebase/detect-conflict-test.ts › git/rebase › continue after resolving conflicts › returns success
#[test]
fn continue_after_resolving_returns_success() {
    let ContinueAfterResolving { result, .. } = setup_continue_after_resolving();
    assert_eq!(result, RebaseResult::CompletedWithoutError);
}

// GHD: unit/git/rebase/detect-conflict-test.ts › git/rebase › continue after resolving conflicts › REBASE_HEAD is no longer found
#[test]
fn continue_after_resolving_rebase_head_is_no_longer_found() {
    let ContinueAfterResolving { status, .. } = setup_continue_after_resolving();
    assert!(status.rebase_internal_state.is_none());
}

// GHD: unit/git/rebase/detect-conflict-test.ts › git/rebase › continue after resolving conflicts › no longer has working directory changes
#[test]
fn continue_after_resolving_no_longer_has_working_directory_changes() {
    let ContinueAfterResolving { status, .. } = setup_continue_after_resolving();
    assert_eq!(status.files.len(), 0);
}

// GHD: unit/git/rebase/detect-conflict-test.ts › git/rebase › continue after resolving conflicts › returns to the feature branch
#[test]
fn continue_after_resolving_returns_to_the_feature_branch() {
    let ContinueAfterResolving { status, .. } = setup_continue_after_resolving();

    assert_eq!(status.branch.as_deref(), Some(FEATURE_BRANCH_NAME));
}

// GHD: unit/git/rebase/detect-conflict-test.ts › git/rebase › continue after resolving conflicts › branch is now a different ref
#[test]
fn continue_after_resolving_branch_is_now_a_different_ref() {
    let ContinueAfterResolving {
        status,
        before_rebase_tip,
        ..
    } = setup_continue_after_resolving();
    assert_ne!(
        status.current_tip.as_deref(),
        Some(before_rebase_tip.as_str())
    );
}

// ---------------------------------------------------------------------------
// continue with additional changes unrelated to conflicted files
// ---------------------------------------------------------------------------

struct ContinueWithAdditionalChanges {
    result: RebaseResult,
    status: WorkingDirectoryStatus,
    before_rebase_tip: String,
    files_in_rebased_commit: Vec<corvene_models::CommittedFileChange>,
}

fn setup_continue_with_additional_changes() -> ContinueWithAdditionalChanges {
    let repository =
        repository_builder_rebase::create_repository(BASE_BRANCH_NAME, FEATURE_BRANCH_NAME);

    let feature_branch = get_branch_or_error(&repository, FEATURE_BRANCH_NAME);
    let before_rebase_tip = feature_branch.tip.clone().expect("feature branch tip");

    let base_branch = get_branch_or_error(&repository, BASE_BRANCH_NAME);

    rebase(&repository, &base_branch, &feature_branch, None);

    // resolve conflicts by writing files to disk
    write_file(
        repository.join("THING.md"),
        "# HELLO WORLD! \nTHINGS GO HERE\nFEATURE BRANCH UNDERWAY\n",
    );

    write_file(
        repository.join("OTHER.md"),
        "# HELLO WORLD! \nTHINGS GO HERE\nALSO FEATURE BRANCH UNDERWAY\n",
    );

    // change unrelated tracked while rebasing changes
    write_file(
        repository.join("THIRD.md"),
        "this change should be included in the latest commit",
    );

    // add untracked file before continuing rebase
    write_file(
        repository.join("UNTRACKED-FILE.md"),
        "this file should remain in the working directory",
    );

    let after_rebase = get_status_or_throw(&repository);

    let files = after_rebase.files;

    let result = continue_rebase(&repository, &files, &BTreeMap::new(), None);

    let status = get_status_or_throw(&repository);

    assert!(status.current_tip.is_some());

    let changeset_data = corvene_git::get_changed_files(
        git(),
        repository.path(),
        status.current_tip.as_deref().expect("current tip"),
    )
    .expect("getChangedFiles");

    let files_in_rebased_commit = changeset_data.files;

    ContinueWithAdditionalChanges {
        result,
        status,
        before_rebase_tip,
        files_in_rebased_commit,
    }
}

// GHD: unit/git/rebase/detect-conflict-test.ts › git/rebase › continue with additional changes unrelated to conflicted files › returns success
#[test]
fn continue_with_additional_changes_returns_success() {
    let ContinueWithAdditionalChanges { result, .. } = setup_continue_with_additional_changes();
    assert_eq!(result, RebaseResult::CompletedWithoutError);
}

// GHD: unit/git/rebase/detect-conflict-test.ts › git/rebase › continue with additional changes unrelated to conflicted files › keeps untracked working directory file out of rebase
#[test]
fn continue_with_additional_changes_keeps_untracked_working_directory_file_out_of_rebase() {
    let ContinueWithAdditionalChanges { status, .. } = setup_continue_with_additional_changes();
    assert_eq!(status.files.len(), 1);
}

// GHD: unit/git/rebase/detect-conflict-test.ts › git/rebase › continue with additional changes unrelated to conflicted files › has modified but unconflicted file in commit contents
#[test]
fn continue_with_additional_changes_has_modified_but_unconflicted_file_in_commit_contents() {
    let ContinueWithAdditionalChanges {
        files_in_rebased_commit,
        ..
    } = setup_continue_with_additional_changes();

    // `find(..) !== undefined`
    assert!(files_in_rebased_commit.iter().any(|f| f.path == "THIRD.md"));
}

// GHD: unit/git/rebase/detect-conflict-test.ts › git/rebase › continue with additional changes unrelated to conflicted files › returns to the feature branch
#[test]
fn continue_with_additional_changes_returns_to_the_feature_branch() {
    let ContinueWithAdditionalChanges { status, .. } = setup_continue_with_additional_changes();

    assert_eq!(status.branch.as_deref(), Some(FEATURE_BRANCH_NAME));
}

// GHD: unit/git/rebase/detect-conflict-test.ts › git/rebase › continue with additional changes unrelated to conflicted files › branch is now a different ref
#[test]
fn continue_with_additional_changes_branch_is_now_a_different_ref() {
    let ContinueWithAdditionalChanges {
        status,
        before_rebase_tip,
        ..
    } = setup_continue_with_additional_changes();

    assert_ne!(
        status.current_tip.as_deref(),
        Some(before_rebase_tip.as_str())
    );
}

// ---------------------------------------------------------------------------
// continue with tracked change omitted from list
// ---------------------------------------------------------------------------

// GHD: unit/git/rebase/detect-conflict-test.ts › git/rebase › continue with tracked change omitted from list › returns error code indicating that required files were missing
#[test]
fn returns_error_code_indicating_that_required_files_were_missing() {
    let repository =
        repository_builder_rebase::create_repository(BASE_BRANCH_NAME, FEATURE_BRANCH_NAME);

    let feature_branch = get_branch_or_error(&repository, FEATURE_BRANCH_NAME);

    let base_branch = get_branch_or_error(&repository, BASE_BRANCH_NAME);

    rebase(&repository, &base_branch, &feature_branch, None);

    // resolve conflicts by writing files to disk
    write_file(
        repository.join("THING.md"),
        "# HELLO WORLD! \nTHINGS GO HERE\nFEATURE BRANCH UNDERWAY\n",
    );

    write_file(
        repository.join("OTHER.md"),
        "# HELLO WORLD! \nTHINGS GO HERE\nALSO FEATURE BRANCH UNDERWAY\n",
    );

    // change unrelated tracked while rebasing changes
    write_file(
        repository.join("THIRD.md"),
        "this change should be included in the latest commit",
    );

    let after_rebase = get_status_or_throw(&repository);

    let files = after_rebase.files;

    // omit the last change should cause Git to error because it requires
    // all tracked changes to be staged as a prerequisite for rebasing
    let only_conflicted_files: Vec<_> =
        files.into_iter().filter(|f| f.path != "THIRD.md").collect();

    let result = continue_rebase(&repository, &only_conflicted_files, &BTreeMap::new(), None);

    assert_eq!(result, RebaseResult::OutstandingFilesNotStaged);
}
