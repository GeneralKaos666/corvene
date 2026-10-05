//! The multi-commit operation in flight (merge, rebase, cherry-pick,
//! squash, reorder): its step, progress and the conflicted files with
//! the user's manual resolutions.

use corvene_core::AppState;
use corvene_core::mco::{ConflictKind, McoDetail, McoStep};
use corvene_models::{ManualConflictResolution, MultiCommitOperationKind};

#[derive(uniffi::Enum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum McoKindVm {
    Merge,
    Rebase,
    CherryPick,
    Squash,
    Reorder,
}

#[derive(uniffi::Enum, Clone, Debug, PartialEq, Eq)]
pub enum McoStepVm {
    ChooseBranch,
    WarnForcePush,
    ShowProgress,
    ShowConflicts,
    HideConflicts,
    ConfirmAbort,
    CreateBranch { initial_name: String },
}

#[derive(uniffi::Enum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolutionVm {
    Ours,
    Theirs,
}

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct ConflictedFileVm {
    pub path: String,
    pub resolution: Option<ResolutionVm>,
    /// git lists it as unmerged with no manual resolution yet.
    pub unresolved: bool,
}

#[derive(uniffi::Record, Clone, Debug, PartialEq)]
pub struct McoVm {
    pub repo: u64,
    pub kind: McoKindVm,
    pub step: McoStepVm,
    pub target_branch: Option<String>,
    pub source_branch: Option<String>,
    pub our_branch: Option<String>,
    pub their_branch: Option<String>,
    /// 0..1 while `ShowProgress`.
    pub progress: f32,
    pub progress_position: u32,
    pub progress_total: u32,
    pub progress_summary: String,
    pub commit_count: u32,
    pub user_has_resolved_conflicts: bool,
}

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct ConflictsVm {
    pub repo: u64,
    /// Merge, Rebase or CherryPick.
    pub kind: McoKindVm,
    pub current_branch: Option<String>,
    pub target_branch: Option<String>,
    pub files: Vec<ConflictedFileVm>,
    pub unresolved_count: u32,
}

fn kind_vm(kind: MultiCommitOperationKind) -> McoKindVm {
    match kind {
        MultiCommitOperationKind::Merge => McoKindVm::Merge,
        MultiCommitOperationKind::Rebase => McoKindVm::Rebase,
        MultiCommitOperationKind::CherryPick => McoKindVm::CherryPick,
        MultiCommitOperationKind::Squash => McoKindVm::Squash,
        MultiCommitOperationKind::Reorder => McoKindVm::Reorder,
    }
}

pub fn mco(s: &AppState, repo: u64) -> Option<McoVm> {
    let rs = s.repo_states.get(&repo)?;
    let op = rs.mco.as_ref()?;
    let (source_branch, commit_count) = match &op.detail {
        McoDetail::Rebase {
            base_branch,
            commits,
        } => (base_branch.clone(), commits.len()),
        McoDetail::CherryPick {
            source_branch,
            commits,
            ..
        } => (source_branch.clone(), commits.len()),
        McoDetail::Squash { commits, .. }
        | McoDetail::Reorder { commits, .. }
        | McoDetail::Autosquash { commits, .. } => (None, commits.len()),
        McoDetail::Merge { source_branch, .. } => (source_branch.clone(), 0),
    };
    Some(McoVm {
        repo,
        kind: kind_vm(op.detail.kind()),
        step: match &op.step {
            McoStep::ChooseBranch => McoStepVm::ChooseBranch,
            McoStep::WarnForcePush => McoStepVm::WarnForcePush,
            McoStep::ShowProgress => McoStepVm::ShowProgress,
            McoStep::ShowConflicts => McoStepVm::ShowConflicts,
            McoStep::HideConflicts => McoStepVm::HideConflicts,
            McoStep::ConfirmAbort => McoStepVm::ConfirmAbort,
            McoStep::CreateBranch { initial_name } => McoStepVm::CreateBranch {
                initial_name: initial_name.clone(),
            },
        },
        target_branch: op.target_branch.clone(),
        source_branch,
        our_branch: op.conflicts.our_branch.clone(),
        their_branch: op.conflicts.their_branch.clone(),
        progress: op.progress.value,
        progress_position: u32::try_from(op.progress.position).unwrap_or(u32::MAX),
        progress_total: u32::try_from(op.progress.total).unwrap_or(u32::MAX),
        progress_summary: op.progress.current_summary.clone(),
        commit_count: u32::try_from(commit_count).unwrap_or(u32::MAX),
        user_has_resolved_conflicts: op.user_has_resolved_conflicts,
    })
}

pub fn conflicts(s: &AppState, repo: u64) -> Option<ConflictsVm> {
    let rs = s.repo_states.get(&repo)?;
    let conflict = rs.conflict_state.as_ref()?;
    let (kind, current_branch, target_branch) = match &conflict.kind {
        ConflictKind::Merge { current_branch, .. } => {
            (McoKindVm::Merge, Some(current_branch.clone()), None)
        }
        ConflictKind::Rebase { target_branch, .. } => {
            (McoKindVm::Rebase, None, Some(target_branch.clone()))
        }
        ConflictKind::CherryPick { target_branch } => {
            (McoKindVm::CherryPick, None, Some(target_branch.clone()))
        }
    };
    let files: Vec<ConflictedFileVm> = rs
        .status
        .as_ref()
        .map(|status| {
            corvene_core::mco::unmerged_files(status)
                .into_iter()
                .map(|f| {
                    let resolution = conflict.manual_resolutions.get(&f.path).copied();
                    ConflictedFileVm {
                        path: f.path.clone(),
                        resolution: resolution.map(|r| match r {
                            ManualConflictResolution::Ours => ResolutionVm::Ours,
                            ManualConflictResolution::Theirs => ResolutionVm::Theirs,
                        }),
                        unresolved: f.status.has_unresolved_conflicts(resolution),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    let unresolved_count = files.iter().filter(|f| f.unresolved).count();
    Some(ConflictsVm {
        repo,
        kind,
        current_branch,
        target_branch,
        files,
        unresolved_count: u32::try_from(unresolved_count).unwrap_or(u32::MAX),
    })
}
