//! Port of GitHub Desktop's `app/test/unit/git/rebase/progress-test.ts`.
//!
//! - `rebase`, `continueRebase` (`lib/git/rebase.ts`) are
//!   `crate::rebase_support::{rebase, continue_rebase}` over
//!   `corvene_git::rebase` / `corvene_git::continue_rebase`.
//! - `getRebaseSnapshot` is `corvene_git::rebase_snapshot` (`None` is
//!   `null`); `getStatus` is `corvene_git::get_status` (an `Err` is `null`).
//! - `IMultiCommitOperationProgress` is `corvene_models::McoProgress`:
//!   `currentCommitSummary` is `current_summary`, `totalCommitCount` is
//!   `total`. Its `kind: 'multiCommitOperation'` only tells GitHub
//!   Desktop's `Progress` union apart; Corvene's progress of a multi-commit
//!   operation is its own type, so the expected objects below carry the
//!   other four fields.
//! - `isConflictedFile(file.status)` is `file.status.kind ==
//!   FileStatusKind::Conflicted` (GitHub Desktop's function compares the
//!   kind); `ManualConflictResolution.theirs` is
//!   `ManualConflictResolution::Theirs`.
//! - Each `describe`'s `setup` is a function every test of it calls.

use std::collections::BTreeMap;

use corvene_git::{RebaseResult, RebaseSnapshot};
use corvene_models::{
    FileStatusKind, ManualConflictResolution, McoProgress, WorkingDirectoryStatus,
};
use corvene_test_support::{
    TestRepo, get_branch_or_error, get_status_or_throw, git, repository_builder_long_rebase,
    repository_builder_rebase, setup_empty_directory,
};

use crate::rebase_support::{continue_rebase, rebase};

const BASE_BRANCH_NAME: &str = "base-branch";
const FEATURE_BRANCH_NAME: &str = "this-is-a-feature";

/// GitHub Desktop's `getRebaseSnapshot(repository)`.
fn get_rebase_snapshot(repository: &TestRepo) -> Option<RebaseSnapshot> {
    corvene_git::rebase_snapshot(git(), repository.path())
}

/// An `IMultiCommitOperationProgress` literal (see the module doc for
/// `kind`).
fn progress(
    current_commit_summary: &str,
    position: usize,
    total_commit_count: usize,
    value: f32,
) -> McoProgress {
    McoProgress {
        current_summary: current_commit_summary.into(),
        position,
        total: total_commit_count,
        value,
    }
}

struct Setup {
    repository: TestRepo,
    result: RebaseResult,
    snapshot: Option<RebaseSnapshot>,
    status: WorkingDirectoryStatus,
    progress: Vec<McoProgress>,
}

/// The `setup` of `can parse progress` (with `createShortRebaseTest`) and
/// of `can parse progress for long rebase` (with `createLongRebaseTest`).
fn setup(create_repository: fn(&str, &str) -> TestRepo) -> Setup {
    let repository = create_repository(BASE_BRANCH_NAME, FEATURE_BRANCH_NAME);

    let feature_branch = get_branch_or_error(&repository, FEATURE_BRANCH_NAME);

    let base_branch = get_branch_or_error(&repository, BASE_BRANCH_NAME);

    let mut progress = Vec::<McoProgress>::new();
    let result = rebase(
        &repository,
        &base_branch,
        &feature_branch,
        Some(&mut |p| progress.push(p)),
    );

    let snapshot = get_rebase_snapshot(&repository);

    let status = get_status_or_throw(&repository);

    Setup {
        repository,
        result,
        snapshot,
        status,
        progress,
    }
}

fn setup_short() -> Setup {
    setup(repository_builder_rebase::create_repository)
}

fn setup_long() -> Setup {
    setup(repository_builder_long_rebase::create_repository)
}

/// The test file's `resolveAndContinue(repository, strategy,
/// progressCallback)`.
fn resolve_and_continue(
    repository: &TestRepo,
    strategy: ManualConflictResolution,
    progress_callback: &mut dyn FnMut(McoProgress),
) -> RebaseResult {
    let status = corvene_git::get_status(git(), repository.path()).ok();
    if let Some(status) = &status {
        corvene_test_support::check_in_process_status(repository.path(), status);
    }
    let files = status.map(|s| s.files).unwrap_or_default();
    let mut resolutions = BTreeMap::<String, ManualConflictResolution>::new();

    for file in &files {
        if file.status.kind == FileStatusKind::Conflicted {
            resolutions.insert(file.path.clone(), strategy);
        }
    }

    continue_rebase(repository, &files, &resolutions, Some(progress_callback))
}

// GHD: unit/git/rebase/progress-test.ts › git/rebase › skips a normal repository › returns null for rebase progress
#[test]
fn returns_null_for_rebase_progress() {
    let repository = setup_empty_directory();
    let progress = get_rebase_snapshot(&repository);

    assert_eq!(progress, None);
}

// GHD: unit/git/rebase/progress-test.ts › git/rebase › can parse progress › returns a value indicating conflicts were encountered
#[test]
fn can_parse_progress_returns_a_value_indicating_conflicts_were_encountered() {
    let Setup { result, .. } = setup_short();
    assert_eq!(result, RebaseResult::ConflictsEncountered);
}

// GHD: unit/git/rebase/progress-test.ts › git/rebase › can parse progress › reported step-by-step progress before encountering conflicts
#[test]
fn can_parse_progress_reported_step_by_step_progress_before_encountering_conflicts() {
    let Setup { progress: p, .. } = setup_short();
    assert_eq!(p, vec![progress("Feature Branch!", 1, 1, 1.)]);
}

// GHD: unit/git/rebase/progress-test.ts › git/rebase › can parse progress › status detects REBASE_HEAD
#[test]
fn can_parse_progress_status_detects_rebase_head() {
    let Setup { snapshot, .. } = setup_short();

    assert!(snapshot.is_some());
    let s = snapshot.unwrap();
    assert_eq!(s.commits.len(), 1);
    assert_eq!(s.commits[0].summary, "Feature Branch!");

    assert_eq!(s.progress.position, 1);
    assert_eq!(s.progress.total, 1);
    assert_eq!(s.progress.current_summary, "Feature Branch!");
    assert_eq!(s.progress.value, 1.);
}

// GHD: unit/git/rebase/progress-test.ts › git/rebase › can parse progress › is a detached HEAD state
#[test]
fn can_parse_progress_is_a_detached_head_state() {
    let Setup { status, .. } = setup_short();
    assert!(status.branch.is_none());
}

// GHD: unit/git/rebase/progress-test.ts › git/rebase › can parse progress for long rebase › returns a value indicating conflicts were encountered
#[test]
fn long_rebase_returns_a_value_indicating_conflicts_were_encountered() {
    let Setup { result, .. } = setup_long();
    assert_eq!(result, RebaseResult::ConflictsEncountered);
}

// GHD: unit/git/rebase/progress-test.ts › git/rebase › can parse progress for long rebase › reported step-by-step progress before encountering conflicts
#[test]
fn long_rebase_reported_step_by_step_progress_before_encountering_conflicts() {
    let Setup { progress: p, .. } = setup_long();
    assert_eq!(
        p,
        vec![progress("Feature Branch First Commit!", 1, 10, 0.1)]
    );
}

// GHD: unit/git/rebase/progress-test.ts › git/rebase › can parse progress for long rebase › reports progress after resolving conflicts
#[test]
fn long_rebase_reports_progress_after_resolving_conflicts() {
    let Setup {
        progress: mut p,
        result,
        repository,
        ..
    } = setup_long();

    let strategy = ManualConflictResolution::Theirs;

    let mut r = result;
    while r == RebaseResult::ConflictsEncountered {
        r = resolve_and_continue(&repository, strategy, &mut |x| p.push(x));
    }

    assert_eq!(p.len(), 10);
    assert_eq!(p[9], progress("Feature Branch Tenth Commit!", 10, 10, 1.));
}

// GHD: unit/git/rebase/progress-test.ts › git/rebase › can parse progress for long rebase › status detects REBASE_HEAD
#[test]
fn long_rebase_status_detects_rebase_head() {
    let Setup { snapshot, .. } = setup_long();

    assert!(snapshot.is_some());
    let s = snapshot.unwrap();
    assert_eq!(s.commits.len(), 10);
    assert_eq!(s.commits[0].summary, "Feature Branch First Commit!");

    assert_eq!(s.progress.position, 1);
    assert_eq!(s.progress.total, 10);
    assert_eq!(s.progress.current_summary, "Feature Branch First Commit!");
    assert_eq!(s.progress.value, 0.1);
}

// GHD: unit/git/rebase/progress-test.ts › git/rebase › can parse progress for long rebase › is a detached HEAD state
#[test]
fn long_rebase_is_a_detached_head_state() {
    let Setup { status, .. } = setup_long();
    assert!(status.branch.is_none());
}
