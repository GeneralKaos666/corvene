//! The repository list.

use corvene_core::AppState;

/// One row of the repository list.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct RepoVm {
    pub id: u64,
    /// The alias, else the directory name.
    pub name: String,
    pub path: String,
    /// `owner/name` on GitHub, when the `origin` remote points there.
    pub github: Option<String>,
    /// The directory is gone ("Can't find").
    pub missing: bool,
    /// Flag `214`: the checked-out branch, once the indicators refreshed.
    pub branch: Option<String>,
    pub changed_files: u32,
    pub ahead: Option<u32>,
    pub behind: Option<u32>,
}

/// The repository list screen.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct RepoListVm {
    pub selected: Option<u64>,
    /// Most recent first (GHD's "Recent" group).
    pub recent: Vec<u64>,
    pub repositories: Vec<RepoVm>,
    /// The sign-in state matters for the empty list's blank slate.
    pub signed_in: bool,
    pub welcome_completed: bool,
}

pub fn repo_list(s: &AppState) -> RepoListVm {
    let repositories = s
        .repositories
        .iter()
        .map(|r| {
            let indicator = s.indicators.get(&r.id);
            RepoVm {
                id: r.id,
                name: r.name(),
                path: r.path.to_string_lossy().into_owned(),
                github: r.github.as_ref().map(|g| format!("{}/{}", g.owner, g.name)),
                missing: r.missing,
                branch: indicator.and_then(|i| i.branch.clone()),
                changed_files: indicator
                    .map(|i| u32::try_from(i.changed_files).unwrap_or(u32::MAX))
                    .unwrap_or(0),
                ahead: indicator
                    .and_then(|i| i.ahead_behind.as_ref())
                    .map(|ab| ab.ahead),
                behind: indicator
                    .and_then(|i| i.ahead_behind.as_ref())
                    .map(|ab| ab.behind),
            }
        })
        .collect();
    RepoListVm {
        selected: s.selected,
        recent: s.recent.clone(),
        repositories,
        signed_in: !s.accounts.is_empty(),
        welcome_completed: s.settings.welcome_completed,
    }
}
