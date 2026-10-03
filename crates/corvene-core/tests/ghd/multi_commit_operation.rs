//! Port of GitHub Desktop's `app/test/unit/multi-commit-operation-test.ts`.
//!
//! - `conflictSteps.includes(step)` (`models/multi-commit-operation.ts`) is
//!   `corvene_core::MultiCommitOperation::in_conflict_step` of an operation
//!   at that step (Corvene's `McoStep` is GitHub Desktop's
//!   `MultiCommitOperationStepKind`, `MultiCommitOperation` its
//!   `IMultiCommitOperationState`). It also counts `HideConflicts`, which
//!   GitHub Desktop's list leaves out (it holds `ShowConflicts`,
//!   `ConfirmAbort` and the Copilot steps); no upstream case checks that
//!   step.
//! - `isConflictsFlow` (`lib/multi-commit-operation.ts`) has no Corvene
//!   function: the dispatcher's conflict handling (`mco.rs`, the status
//!   update that reopens the conflicts step) inlines its check, so the cases
//!   call the stand-in [`is_conflicts_flow`].
//! - `getMultiCommitOperationChooseBranchStep` has no Corvene function
//!   either: Corvene's `McoStep::ChooseBranch` carries no branches (the
//!   dialog reads them from `RepositoryState`), so the cases call the
//!   stand-in [`get_multi_commit_operation_choose_branch_step`] with GitHub
//!   Desktop's `IRepositoryState.branchesState` as Corvene's
//!   `RepositoryState` (`info.tip`, `info.branches`, `recent_branches`,
//!   `default_branch`).
//! - `isIdMultiCommitOperation` is skipped (`tools/ghd-tests/skips/mco.tsv`).

use std::path::PathBuf;

use corvene_core::state::RepositoryState;
use corvene_core::{
    Branch, BranchKind, Identity, McoConflicts, McoDetail, McoProgress, McoStep,
    MultiCommitOperation, RepositoryInfo, Tip,
};

/// Stand-in for GitHub Desktop's `isConflictsFlow(
/// isMultiCommitOperationPopupOpen, multiCommitOperationState)`
/// (`lib/multi-commit-operation.ts`): the popup is open and the operation is
/// at one of `conflictSteps`. Call the Corvene function once there is one and
/// remove the `#[ignore]`s.
fn is_conflicts_flow(
    _is_multi_commit_operation_popup_open: bool,
    _multi_commit_operation_state: Option<&MultiCommitOperation>,
) -> bool {
    unimplemented!("corvene_core has no isConflictsFlow")
}

/// GitHub Desktop's `ChooseBranchStep` (`models/multi-commit-operation.ts`),
/// what [`get_multi_commit_operation_choose_branch_step`] returns.
#[derive(Debug)]
struct ChooseBranchStep {
    kind: McoStep,
    default_branch: Option<Branch>,
    current_branch: Branch,
    all_branches: Vec<Branch>,
}

/// Stand-in for GitHub Desktop's
/// `getMultiCommitOperationChooseBranchStep(state, initialBranch?)`
/// (`lib/multi-commit-operation.ts`): the ChooseBranch step with the
/// repository's current, default and other branches, or an error (GitHub
/// Desktop throws) when the tip is not a valid branch. Call the Corvene
/// function once there is one and remove the `#[ignore]`s.
fn get_multi_commit_operation_choose_branch_step(
    _state: &RepositoryState,
) -> Result<ChooseBranchStep, String> {
    unimplemented!("corvene_core has no getMultiCommitOperationChooseBranchStep")
}

/// An operation at `step`, the rest as in the tests' state objects.
fn operation(
    step: McoStep,
    detail: McoDetail,
    user_has_resolved_conflicts: bool,
) -> MultiCommitOperation {
    MultiCommitOperation {
        step,
        detail,
        progress: McoProgress {
            value: 0.0,
            ..Default::default()
        },
        user_has_resolved_conflicts,
        original_branch_tip: None,
        target_branch: None,
        conflicts: McoConflicts::default(),
    }
}

fn rebase_detail() -> McoDetail {
    McoDetail::Rebase {
        base_branch: None,
        commits: Vec::new(),
    }
}

fn cherry_pick_detail() -> McoDetail {
    McoDetail::CherryPick {
        source_branch: None,
        branch_created: false,
        commits: Vec::new(),
    }
}

/// `conflictSteps.includes(step)`.
fn conflict_steps_include(step: McoStep) -> bool {
    operation(step, rebase_detail(), false).in_conflict_step()
}

/// `{ name, tip: { sha }, type: 0 }` (a local branch).
fn local_branch(name: &str, sha: &str) -> Branch {
    Branch {
        name: name.into(),
        kind: BranchKind::Local,
        full_name: format!("refs/heads/{name}"),
        tip: Some(sha.into()),
        upstream: None,
        tip_time: None,
        remote_name: None,
    }
}

/// A `RepositoryState` whose branches are GitHub Desktop's `branchesState`.
fn repository_state(
    tip: Tip,
    default_branch: Option<&Branch>,
    all_branches: Vec<Branch>,
    recent_branches: Vec<&Branch>,
) -> RepositoryState {
    RepositoryState {
        info: Some(RepositoryInfo {
            workdir: PathBuf::from("/repository"),
            tip,
            branches: all_branches,
            remotes: Vec::new(),
            identity: Identity {
                name: None,
                email: None,
            },
            ahead_behind: None,
            commit_template: None,
        }),
        recent_branches: recent_branches.iter().map(|b| b.name.clone()).collect(),
        default_branch: default_branch.map(|b| b.name.clone()),
        ..Default::default()
    }
}

// GHD: unit/multi-commit-operation-test.ts › multi-commit-operation › conflictSteps › includes ShowConflicts
#[test]
fn includes_show_conflicts() {
    assert!(conflict_steps_include(McoStep::ShowConflicts));
}

// GHD: unit/multi-commit-operation-test.ts › multi-commit-operation › conflictSteps › includes ConfirmAbort
#[test]
fn includes_confirm_abort() {
    assert!(conflict_steps_include(McoStep::ConfirmAbort));
}

// GHD: unit/multi-commit-operation-test.ts › multi-commit-operation › conflictSteps › does not include ChooseBranch
#[test]
fn does_not_include_choose_branch() {
    assert!(!conflict_steps_include(McoStep::ChooseBranch));
}

// GHD: unit/multi-commit-operation-test.ts › multi-commit-operation › isConflictsFlow › returns false when popup is not open
#[test]
#[ignore = "ghd: missing: corvene_core has no isConflictsFlow (lib/multi-commit-operation.ts); the dispatcher inlines the check in mco.rs"]
fn returns_false_when_popup_is_not_open() {
    assert!(!is_conflicts_flow(false, None));
}

// GHD: unit/multi-commit-operation-test.ts › multi-commit-operation › isConflictsFlow › returns false when state is null
#[test]
#[ignore = "ghd: missing: corvene_core has no isConflictsFlow (lib/multi-commit-operation.ts); the dispatcher inlines the check in mco.rs"]
fn returns_false_when_state_is_null() {
    assert!(!is_conflicts_flow(true, None));
}

// GHD: unit/multi-commit-operation-test.ts › multi-commit-operation › isConflictsFlow › returns false when step is not a conflict step
#[test]
#[ignore = "ghd: missing: corvene_core has no isConflictsFlow (lib/multi-commit-operation.ts); the dispatcher inlines the check in mco.rs"]
fn returns_false_when_step_is_not_a_conflict_step() {
    let state = operation(McoStep::ShowProgress, rebase_detail(), false);

    assert!(!is_conflicts_flow(true, Some(&state)));
}

// GHD: unit/multi-commit-operation-test.ts › multi-commit-operation › isConflictsFlow › returns true when in ShowConflicts step
#[test]
#[ignore = "ghd: missing: corvene_core has no isConflictsFlow (lib/multi-commit-operation.ts); the dispatcher inlines the check in mco.rs"]
fn returns_true_when_in_show_conflicts_step() {
    let state = operation(McoStep::ShowConflicts, rebase_detail(), false);

    assert!(is_conflicts_flow(true, Some(&state)));
}

// GHD: unit/multi-commit-operation-test.ts › multi-commit-operation › isConflictsFlow › returns true when in ConfirmAbort step
#[test]
#[ignore = "ghd: missing: corvene_core has no isConflictsFlow (lib/multi-commit-operation.ts); the dispatcher inlines the check in mco.rs"]
fn returns_true_when_in_confirm_abort_step() {
    let state = operation(McoStep::ConfirmAbort, cherry_pick_detail(), true);

    assert!(is_conflicts_flow(true, Some(&state)));
}

// GHD: unit/multi-commit-operation-test.ts › multi-commit-operation › getMultiCommitOperationChooseBranchStep › throws when tip is not valid
#[test]
#[ignore = "ghd: missing: corvene_core has no getMultiCommitOperationChooseBranchStep (lib/multi-commit-operation.ts); McoStep::ChooseBranch carries no branches"]
fn throws_when_tip_is_not_valid() {
    let state = repository_state(Tip::Unknown, None, Vec::new(), Vec::new());

    assert!(get_multi_commit_operation_choose_branch_step(&state).is_err());
}

// GHD: unit/multi-commit-operation-test.ts › multi-commit-operation › getMultiCommitOperationChooseBranchStep › returns ChooseBranch step with branch info when tip is valid
#[test]
#[ignore = "ghd: missing: corvene_core has no getMultiCommitOperationChooseBranchStep (lib/multi-commit-operation.ts); McoStep::ChooseBranch carries no branches"]
fn returns_choose_branch_step_with_branch_info_when_tip_is_valid() {
    let current_branch = local_branch("feature", "abc123");
    let default_branch = local_branch("main", "def456");

    let state = repository_state(
        Tip::Valid {
            branch: current_branch.clone(),
        },
        Some(&default_branch),
        vec![current_branch.clone(), default_branch.clone()],
        vec![&current_branch],
    );

    let step = get_multi_commit_operation_choose_branch_step(&state)
        .expect("getMultiCommitOperationChooseBranchStep threw");

    assert_eq!(step.kind, McoStep::ChooseBranch);
    assert_eq!(step.current_branch, current_branch);
    assert_eq!(step.default_branch, Some(default_branch));
    assert_eq!(step.all_branches.len(), 2);
}
