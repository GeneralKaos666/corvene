//! The Changes tab: the file list, what is selected, the commit form.

use corvene_core::AppState;
use corvene_models::{DiffSelectionType, FileStatusKind};

#[derive(uniffi::Enum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileStatusVm {
    New,
    Modified,
    Deleted,
    Copied,
    Renamed,
    Conflicted,
    Untracked,
}

impl From<FileStatusKind> for FileStatusVm {
    fn from(kind: FileStatusKind) -> Self {
        match kind {
            FileStatusKind::New => FileStatusVm::New,
            FileStatusKind::Modified => FileStatusVm::Modified,
            FileStatusKind::Deleted => FileStatusVm::Deleted,
            FileStatusKind::Copied => FileStatusVm::Copied,
            FileStatusKind::Renamed => FileStatusVm::Renamed,
            FileStatusKind::Conflicted => FileStatusVm::Conflicted,
            FileStatusKind::Untracked => FileStatusVm::Untracked,
        }
    }
}

/// The include checkbox: all, some (partial staging) or none of the file.
#[derive(uniffi::Enum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum IncludeVm {
    All,
    Partial,
    None,
}

impl From<DiffSelectionType> for IncludeVm {
    fn from(kind: DiffSelectionType) -> Self {
        match kind {
            DiffSelectionType::All => IncludeVm::All,
            DiffSelectionType::Partial => IncludeVm::Partial,
            DiffSelectionType::None => IncludeVm::None,
        }
    }
}

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct ChangedFileVm {
    pub path: String,
    pub old_path: Option<String>,
    pub status: FileStatusVm,
    pub include: IncludeVm,
    pub selected: bool,
    pub lines_added: Option<u32>,
    pub lines_deleted: Option<u32>,
}

/// Flag `1307-commit-progress`: how far a commit that takes a while got.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct CommitProgressVm {
    /// Git writes the commit (and runs its hooks); every file is staged.
    pub writing: bool,
    /// Files in the index so far.
    pub staged: u32,
    /// Files going into the commit.
    pub total: u32,
}

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct CommitFormVm {
    /// The author shown next to the form, "Name <email>" when known.
    pub author: Option<String>,
    pub branch: Option<String>,
    pub committing: bool,
    pub amending: bool,
    pub co_authors: Vec<String>,
    /// GHD "Undo" bar: the last commit of this session.
    pub last_commit_sha: Option<String>,
    pub last_commit_summary: Option<String>,
    /// Flag `1307-commit-progress`: set once a commit has run for a moment.
    #[uniffi(default)]
    pub progress: Option<CommitProgressVm>,
}

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct ChangesVm {
    pub repo: u64,
    pub loading: bool,
    pub error: Option<String>,
    pub files: Vec<ChangedFileVm>,
    pub included_count: u32,
    pub selected_file: Option<String>,
    /// Bumps when the diff of the selected file changed.
    pub diff_generation: u64,
    /// Bumps after every commit (clears the form).
    pub commit_nonce: u64,
    pub form: CommitFormVm,
    pub conflicts: u32,
    pub stash_count: u32,
    /// GHD `desktop_stash`: the current branch has a stash (the banner).
    pub stash_on_current_branch: bool,
    /// The filter bar: which kinds are hidden (GHD `FilterOptions`).
    pub filter_included: bool,
    pub filter_excluded: bool,
    pub filter_new: bool,
    pub filter_modified: bool,
    pub filter_deleted: bool,
    pub filter_renamed: bool,
    /// Flag `705-renamed-files-filter`: whether the Renamed filter exists.
    pub renamed_filter_available: bool,
}

pub fn changes(s: &AppState, repo: u64) -> Option<ChangesVm> {
    let rs = s.repo_states.get(&repo)?;
    let files: Vec<ChangedFileVm> = rs
        .status
        .as_ref()
        .map(|status| {
            status
                .files
                .iter()
                .map(|f| {
                    let stats = rs.line_stats.get(&f.path);
                    ChangedFileVm {
                        path: f.path.clone(),
                        old_path: f.old_path.clone(),
                        status: f.status.kind.into(),
                        include: f.selection.kind().into(),
                        selected: rs.selected_files.iter().any(|p| p == &f.path)
                            || rs.selected_file.as_deref() == Some(&f.path),
                        lines_added: stats.map(|s| u32::try_from(s.added).unwrap_or(u32::MAX)),
                        lines_deleted: stats.map(|s| u32::try_from(s.deleted).unwrap_or(u32::MAX)),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    let included_count = files
        .iter()
        .filter(|f| f.include != IncludeVm::None)
        .count();
    let conflicts = files
        .iter()
        .filter(|f| f.status == FileStatusVm::Conflicted)
        .count();
    let branch = rs
        .info
        .as_ref()
        .and_then(|i| i.current_branch().map(|b| b.name.clone()));
    // the repository's account (`527-multiple-accounts`), else the first
    let author = s
        .account_for_repository(repo)
        .or(s.accounts.first())
        .map(|a| a.login.clone());
    Some(ChangesVm {
        repo,
        loading: rs.loading,
        error: rs.error.clone(),
        included_count: u32::try_from(included_count).unwrap_or(u32::MAX),
        selected_file: rs.selected_file.clone(),
        diff_generation: rs.diff_generation,
        commit_nonce: rs.commit_nonce,
        form: CommitFormVm {
            author,
            branch,
            committing: rs.committing,
            amending: rs.commit_to_amend.is_some(),
            co_authors: rs.co_authors.iter().map(|a| a.display_text()).collect(),
            last_commit_sha: rs.last_commit.as_ref().map(|c| c.sha.clone()),
            last_commit_summary: rs.last_commit.as_ref().map(|c| c.summary.clone()),
            progress: rs
                .commit_progress
                .filter(|_| rs.committing)
                .map(|p| CommitProgressVm {
                    writing: p.phase == corvene_core::commit_progress::CommitPhase::Writing,
                    staged: u32::try_from(p.staged).unwrap_or(u32::MAX),
                    total: u32::try_from(p.total).unwrap_or(u32::MAX),
                }),
        },
        conflicts: u32::try_from(conflicts).unwrap_or(u32::MAX),
        stash_count: u32::try_from(rs.stash_count).unwrap_or(u32::MAX),
        stash_on_current_branch: rs.stash.is_some(),
        filter_included: rs.file_list_filter.included,
        filter_excluded: rs.file_list_filter.excluded,
        filter_new: rs.file_list_filter.new_files,
        filter_modified: rs.file_list_filter.modified,
        filter_deleted: rs.file_list_filter.deleted,
        filter_renamed: rs.file_list_filter.renamed,
        renamed_filter_available: s.flags.bool(corvene_core::flags::ids::RENAMED_FILES_FILTER),
        files,
    })
}
