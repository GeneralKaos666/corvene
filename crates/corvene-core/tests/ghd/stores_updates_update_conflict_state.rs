//! Port of GitHub Desktop's
//! `app/test/unit/stores/updates/update-conflict-state-test.ts`.
//!
//! GitHub Desktop's `updateConflictState(state, status, statsStore)`
//! (`lib/stores/updates/changes-state.ts`) is
//! `corvene_core::mco::derive_conflict_state(status, previous)`, which the
//! dispatcher calls with the repository's previous
//! `RepositoryState::conflict_state` after every status read. The stats
//! store (`TestStatsStore`) is left out: Corvene has no usage stats, and the
//! upstream assertions on its counters are commented out anyway, so the
//! "increments … counter" cases only run the update.
//!
//! - `createState({ conflictState })` (`helpers/changes-state-helper.ts`) is
//!   a default `RepositoryState` with that `conflict_state`.
//! - `createStatus(…)` is a default `WorkingDirectoryStatus` with the given
//!   fields (`currentBranch` → `branch`, `currentTip` → `current_tip`,
//!   `mergeHeadFound` → `merge_head_found`, `rebaseInternalState` →
//!   `rebase_internal_state`, with `rebase_in_progress` set alongside it as
//!   `corvene_git::get_status` does). Corvene has no
//!   `doConflictedFilesExist`: it is `WorkingDirectoryStatus::has_conflicts`
//!   over the files, so `doConflictedFilesExist: true` adds one conflicted
//!   file.
//! - GitHub Desktop's `ConflictState` objects are
//!   `corvene_core::ConflictState` (`kind: 'merge'` → `ConflictKind::Merge`,
//!   `kind: 'rebase'` → `ConflictKind::Rebase`, whose `current_tip` is
//!   `RebaseConflictState.currentTip`) and `ManualConflictResolution` maps
//!   are `BTreeMap`s; [`rebase_conflict_current_tip`] reads `currentTip`.

use std::collections::BTreeMap;

use corvene_core::mco::derive_conflict_state;
use corvene_core::state::RepositoryState;
use corvene_core::{
    ConflictKind, ConflictState, DiffSelection, FileStatus, FileStatusKind, GitStatusEntry,
    ManualConflictResolution, RebaseInternalState, WorkingDirectoryFileChange,
    WorkingDirectoryStatus,
};

/// GitHub Desktop's `updateConflictState(state, status, statsStore)`.
fn update_conflict_state(
    state: &RepositoryState,
    status: &WorkingDirectoryStatus,
) -> Option<ConflictState> {
    derive_conflict_state(status, state.conflict_state.as_ref())
}

/// GitHub Desktop's `RebaseConflictState.currentTip` (the tip while the
/// rebase is conflicted, `getConflictState` in
/// `lib/stores/updates/changes-state.ts`).
fn rebase_conflict_current_tip(conflict_state: &ConflictState) -> Option<String> {
    match &conflict_state.kind {
        ConflictKind::Rebase { current_tip, .. } => Some(current_tip.clone()),
        _ => None,
    }
}

/// The test's `manualResolutions`: `foo` resolved with theirs.
fn manual_resolutions() -> BTreeMap<String, ManualConflictResolution> {
    BTreeMap::from([("foo".to_string(), ManualConflictResolution::Theirs)])
}

/// `createState({ conflictState })`.
fn create_state(conflict_state: Option<ConflictState>) -> RepositoryState {
    RepositoryState {
        conflict_state,
        ..Default::default()
    }
}

/// The fields of `createStatus(…)` the tests set.
#[derive(Default)]
struct StatusPick {
    merge_head_found: bool,
    rebase_internal_state: Option<RebaseInternalState>,
    current_branch: Option<&'static str>,
    current_tip: Option<&'static str>,
    do_conflicted_files_exist: bool,
}

/// `createStatus(pick)`.
fn create_status(pick: StatusPick) -> WorkingDirectoryStatus {
    let files = if pick.do_conflicted_files_exist {
        vec![WorkingDirectoryFileChange {
            path: "conflicted".into(),
            old_path: None,
            status: FileStatus {
                kind: FileStatusKind::Conflicted,
                index: GitStatusEntry::Unmerged,
                working_tree: GitStatusEntry::Unmerged,
                score: None,
                code: "UU".into(),
                submodule: false,
                submodule_status: None,
                conflict_markers: Some(3),
            },
            selection: DiffSelection::all(),
        }]
    } else {
        Vec::new()
    };
    WorkingDirectoryStatus {
        files,
        branch: pick.current_branch.map(Into::into),
        merge_head_found: pick.merge_head_found,
        rebase_in_progress: pick.rebase_internal_state.is_some(),
        current_tip: pick.current_tip.map(Into::into),
        rebase_internal_state: pick.rebase_internal_state,
        ..Default::default()
    }
}

fn merge_state(
    current_branch: &str,
    current_tip: &str,
    manual_resolutions: BTreeMap<String, ManualConflictResolution>,
) -> ConflictState {
    ConflictState {
        kind: ConflictKind::Merge {
            current_branch: current_branch.into(),
            current_tip: current_tip.into(),
        },
        manual_resolutions,
    }
}

/// GitHub Desktop's `{ kind: 'rebase', currentTip, manualResolutions,
/// targetBranch, baseBranchTip, originalBranchTip }`.
fn rebase_state(
    current_tip: &str,
    manual_resolutions: BTreeMap<String, ManualConflictResolution>,
    target_branch: &str,
    base_branch_tip: &str,
    original_branch_tip: &str,
) -> ConflictState {
    ConflictState {
        kind: ConflictKind::Rebase {
            current_tip: current_tip.into(),
            target_branch: target_branch.into(),
            base_branch_tip: base_branch_tip.into(),
            original_branch_tip: original_branch_tip.into(),
        },
        manual_resolutions,
    }
}

fn rebase_internal_state(
    target_branch: &str,
    base_branch_tip: &str,
    original_branch_tip: &str,
) -> RebaseInternalState {
    RebaseInternalState {
        target_branch: target_branch.into(),
        base_branch_tip: base_branch_tip.into(),
        original_branch_tip: original_branch_tip.into(),
    }
}

// GHD: unit/stores/updates/update-conflict-state-test.ts › updateConflictState › merge conflicts › returns null when no MERGE_HEAD file found
#[test]
fn merge_returns_null_when_no_merge_head_file_found() {
    let prev_state = create_state(Some(merge_state(
        "old-branch",
        "old-sha",
        manual_resolutions(),
    )));
    let status = create_status(StatusPick {
        merge_head_found: false,
        ..Default::default()
    });
    let conflict_state = update_conflict_state(&prev_state, &status);
    assert!(conflict_state.is_none());
}

// GHD: unit/stores/updates/update-conflict-state-test.ts › updateConflictState › merge conflicts › preserves manual resolutions between updates in the same merge
#[test]
fn preserves_manual_resolutions_between_updates_in_the_same_merge() {
    let prev_state = create_state(Some(merge_state(
        "old-branch",
        "old-sha",
        manual_resolutions(),
    )));
    let status = create_status(StatusPick {
        merge_head_found: true,
        current_branch: Some("master"),
        current_tip: Some("first-sha"),
        do_conflicted_files_exist: true,
        ..Default::default()
    });

    let conflict_state = update_conflict_state(&prev_state, &status);

    assert_eq!(
        conflict_state,
        Some(merge_state("master", "first-sha", manual_resolutions()))
    );
}

// GHD: unit/stores/updates/update-conflict-state-test.ts › updateConflictState › merge conflicts › returns null when MERGE_HEAD set but not branch or tip defined
#[test]
fn returns_null_when_merge_head_set_but_not_branch_or_tip_defined() {
    let prev_state = create_state(Some(merge_state(
        "old-branch",
        "old-sha",
        manual_resolutions(),
    )));
    let status = create_status(StatusPick {
        merge_head_found: true,
        current_branch: None,
        current_tip: None,
        ..Default::default()
    });

    let conflict_state = update_conflict_state(&prev_state, &status);
    assert!(conflict_state.is_none());
}

// GHD: unit/stores/updates/update-conflict-state-test.ts › updateConflictState › merge conflicts › returns a value when status has MERGE_HEAD set and in conflicted state
#[test]
fn returns_a_value_when_status_has_merge_head_set_and_in_conflicted_state() {
    let prev_state = create_state(None);
    let status = create_status(StatusPick {
        merge_head_found: true,
        current_branch: Some("master"),
        current_tip: Some("first-sha"),
        do_conflicted_files_exist: true,
        ..Default::default()
    });

    let conflict_state = update_conflict_state(&prev_state, &status);

    assert_eq!(
        conflict_state,
        Some(merge_state("master", "first-sha", BTreeMap::new()))
    );
}

// GHD: unit/stores/updates/update-conflict-state-test.ts › updateConflictState › merge conflicts › increments abort counter when branch has changed
#[test]
fn merge_increments_abort_counter_when_branch_has_changed() {
    let prev_state = create_state(Some(merge_state("old-branch", "old-sha", BTreeMap::new())));
    let status = create_status(StatusPick {
        merge_head_found: true,
        current_branch: Some("master"),
        current_tip: Some("first-sha"),
        do_conflicted_files_exist: true,
        ..Default::default()
    });

    update_conflict_state(&prev_state, &status);

    // upstream: `expect(statsStore.increment).toHaveBeenCalledWith(
    // 'mergeAbortedAfterConflictsCount')`, commented out; Corvene has no stats
}

// GHD: unit/stores/updates/update-conflict-state-test.ts › updateConflictState › merge conflicts › increments abort counter when conflict resolved and tip has not changed
#[test]
fn merge_increments_abort_counter_when_conflict_resolved_and_tip_has_not_changed() {
    let prev_state = create_state(Some(merge_state("master", "old-sha", BTreeMap::new())));
    let status = create_status(StatusPick {
        merge_head_found: false,
        current_branch: Some("master"),
        current_tip: Some("old-sha"),
        ..Default::default()
    });

    update_conflict_state(&prev_state, &status);

    // upstream: `expect(statsStore.increment).toHaveBeenCalledWith(
    // 'mergeAbortedAfterConflictsCount')`, commented out; Corvene has no stats
}

// GHD: unit/stores/updates/update-conflict-state-test.ts › updateConflictState › merge conflicts › increments success counter when conflict resolved and tip has changed
#[test]
fn merge_increments_success_counter_when_conflict_resolved_and_tip_has_changed() {
    let prev_state = create_state(Some(merge_state("master", "old-sha", BTreeMap::new())));
    let status = create_status(StatusPick {
        merge_head_found: false,
        current_branch: Some("master"),
        current_tip: Some("new-sha"),
        ..Default::default()
    });

    update_conflict_state(&prev_state, &status);

    // upstream: `expect(statsStore.increment).toHaveBeenCalledWith(
    // 'mergeSuccessAfterConflictsCount')`, commented out; Corvene has no stats
}

// GHD: unit/stores/updates/update-conflict-state-test.ts › updateConflictState › rebase conflicts › returns null when no REBASE_HEAD file found
#[test]
fn rebase_returns_null_when_no_rebase_head_file_found() {
    let prev_state = create_state(Some(rebase_state(
        "old-sha",
        manual_resolutions(),
        "my-feature-branch",
        "another-sha",
        "some-other-sha",
    )));
    let status = create_status(StatusPick {
        rebase_internal_state: None,
        ..Default::default()
    });
    let conflict_state = update_conflict_state(&prev_state, &status);
    assert!(conflict_state.is_none());
}

// GHD: unit/stores/updates/update-conflict-state-test.ts › updateConflictState › rebase conflicts › returns a value when status has REBASE_HEAD set and conflict present
#[test]
fn returns_a_value_when_status_has_rebase_head_set_and_conflict_present() {
    let prev_state = create_state(None);
    let status = create_status(StatusPick {
        rebase_internal_state: Some(rebase_internal_state(
            "my-feature-branch",
            "another-sha",
            "some-other-sha",
        )),
        current_branch: Some("master"),
        current_tip: Some("first-sha"),
        do_conflicted_files_exist: true,
        ..Default::default()
    });

    let conflict_state = update_conflict_state(&prev_state, &status);

    assert_eq!(
        conflict_state,
        Some(rebase_state(
            "first-sha",
            BTreeMap::new(),
            "my-feature-branch",
            "another-sha",
            "some-other-sha",
        ))
    );
    assert_eq!(
        rebase_conflict_current_tip(conflict_state.as_ref().unwrap()).as_deref(),
        Some("first-sha")
    );
}

// GHD: unit/stores/updates/update-conflict-state-test.ts › updateConflictState › rebase conflicts › preserves manual resolutions when a rebase is detected
#[test]
fn preserves_manual_resolutions_when_a_rebase_is_detected() {
    let prev_state = create_state(Some(rebase_state(
        "old-sha",
        manual_resolutions(),
        "my-feature-branch",
        "another-sha",
        "some-other-sha",
    )));
    let status = create_status(StatusPick {
        rebase_internal_state: Some(rebase_internal_state(
            "my-feature-branch",
            "another-sha",
            "some-other-sha",
        )),
        current_branch: Some("master"),
        current_tip: Some("first-sha"),
        do_conflicted_files_exist: true,
        ..Default::default()
    });

    let conflict_state = update_conflict_state(&prev_state, &status);

    assert_eq!(
        conflict_state,
        Some(rebase_state(
            "first-sha",
            manual_resolutions(),
            "my-feature-branch",
            "another-sha",
            "some-other-sha",
        ))
    );
    assert_eq!(
        rebase_conflict_current_tip(conflict_state.as_ref().unwrap()).as_deref(),
        Some("first-sha")
    );
}

// GHD: unit/stores/updates/update-conflict-state-test.ts › updateConflictState › rebase conflicts › increments abort counter when conflict remains but branch has changed
#[test]
fn rebase_increments_abort_counter_when_conflict_remains_but_branch_has_changed() {
    let prev_state = create_state(Some(rebase_state(
        "current-sha",
        manual_resolutions(),
        "my-feature-branch",
        "another-sha",
        "old-sha",
    )));
    let status = create_status(StatusPick {
        rebase_internal_state: Some(rebase_internal_state(
            "a-different-feature-branch",
            "an-even-older-sha",
            "some-old-sha",
        )),
        current_tip: Some("current-sha"),
        do_conflicted_files_exist: true,
        ..Default::default()
    });

    update_conflict_state(&prev_state, &status);

    // upstream: `expect(statsStore.increment).toHaveBeenCalledWith(
    // 'rebaseAbortedAfterConflictsCount')`, commented out; Corvene has no stats
}

// GHD: unit/stores/updates/update-conflict-state-test.ts › updateConflictState › rebase conflicts › increments abort counter when conflict resolved but tip has not changed
#[test]
fn rebase_increments_abort_counter_when_conflict_resolved_but_tip_has_not_changed() {
    let prev_state = create_state(Some(rebase_state(
        "current-sha",
        manual_resolutions(),
        "my-feature-branch",
        "another-sha",
        "old-sha",
    )));
    let status = create_status(StatusPick {
        rebase_internal_state: None,
        current_branch: Some("my-feature-branch"),
        current_tip: Some("old-sha"),
        ..Default::default()
    });

    update_conflict_state(&prev_state, &status);

    // upstream: `expect(statsStore.increment).toHaveBeenCalledWith(
    // 'rebaseAbortedAfterConflictsCount')`, commented out; Corvene has no stats
}

// GHD: unit/stores/updates/update-conflict-state-test.ts › updateConflictState › rebase conflicts › does not increment aborted counter when conflict resolved and tip has changed
#[test]
fn does_not_increment_aborted_counter_when_conflict_resolved_and_tip_has_changed() {
    let prev_state = create_state(Some(rebase_state(
        "current-sha",
        manual_resolutions(),
        "my-feature-branch",
        "even-older-sha",
        "old-sha",
    )));
    let status = create_status(StatusPick {
        rebase_internal_state: None,
        current_branch: Some("my-feature-branch"),
        current_tip: Some("new-sha"),
        ..Default::default()
    });

    update_conflict_state(&prev_state, &status);

    // upstream: `expect(statsStore.increment).not.toHaveBeenCalledWith(
    // 'rebaseAbortedAfterConflictsCount')`, commented out; Corvene has no stats
}
