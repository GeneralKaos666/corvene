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
    pub owner: Option<String>,
    pub fork: bool,
    pub private: bool,
    pub alias: Option<String>,
    /// The directory is gone ("Can't find").
    pub missing: bool,
    /// Flag `214`: the checked-out branch, once the indicators refreshed.
    pub branch: Option<String>,
    pub changed_files: u32,
    pub ahead: Option<u32>,
    pub behind: Option<u32>,
}

/// One group of GHD's `groupRepositories`: "Recent", one per GitHub owner,
/// then "Other" (ids into `repositories`).
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct RepoGroupVm {
    pub title: String,
    pub ids: Vec<u64>,
}

/// The repository list screen.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct RepoListVm {
    pub selected: Option<u64>,
    /// Most recent first (GHD's "Recent" group).
    pub recent: Vec<u64>,
    pub repositories: Vec<RepoVm>,
    /// Unfiltered grouping in display order (`209-recent-repositories-count`
    /// decides how many recent ones; the Recent group only shows with more
    /// than one repository).
    pub groups: Vec<RepoGroupVm>,
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
                owner: r.github.as_ref().map(|g| g.owner.clone()),
                fork: r.github.as_ref().is_some_and(|g| g.fork),
                private: r.github.as_ref().is_some_and(|g| g.private),
                alias: r.alias.clone(),
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
        groups: groups(s),
        repositories,
        signed_in: !s.accounts.is_empty(),
        welcome_completed: s.settings.welcome_completed,
    }
}

/// GHD `groupRepositories` without a filter (the Kotlin side filters).
fn groups(s: &AppState) -> Vec<RepoGroupVm> {
    let mut groups = Vec::new();
    let shown = usize::try_from(
        s.flags
            .number(corvene_core::flags::ids::RECENT_REPOSITORIES_COUNT),
    )
    .unwrap_or(3);
    let recent: Vec<u64> = s
        .recent
        .iter()
        .take(shown)
        .filter(|id| s.repository(**id).is_some())
        .copied()
        .collect();
    if !recent.is_empty() && s.repositories.len() > 1 {
        groups.push(RepoGroupVm {
            title: "Recent".into(),
            ids: recent,
        });
    }
    let mut owners: Vec<(String, Vec<u64>)> = Vec::new();
    let mut other = Vec::new();
    for repo in s.sorted_repositories() {
        match &repo.github {
            Some(gh) => match owners.iter_mut().find(|(o, _)| *o == gh.owner) {
                Some((_, ids)) => ids.push(repo.id),
                None => owners.push((gh.owner.clone(), vec![repo.id])),
            },
            None => other.push(repo.id),
        }
    }
    owners.sort_by_key(|(o, _)| o.to_lowercase());
    for (owner, ids) in owners {
        groups.push(RepoGroupVm { title: owner, ids });
    }
    if !other.is_empty() {
        groups.push(RepoGroupVm {
            title: "Other".into(),
            ids: other,
        });
    }
    groups
}
