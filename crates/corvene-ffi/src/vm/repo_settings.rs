//! Repository settings (Remote, Ignored files, Git config) and the global
//! git config, loaded by the engine when the dialog opens.

use corvene_core::AppState;

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct RepositorySettingsVm {
    pub repo: u64,
    pub remote_name: Option<String>,
    pub remote_url: Option<String>,
    /// The `.gitignore` text, `None` when the file is missing.
    pub gitignore: Option<String>,
    /// Local (`.git/config`) identity, when set.
    pub local_name: Option<String>,
    pub local_email: Option<String>,
    pub global_name: Option<String>,
    pub global_email: Option<String>,
    /// `core.autocrlf` in effect / the local value when set.
    pub autocrlf: bool,
    pub local_autocrlf: Option<String>,
}

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct GlobalGitConfigVm {
    pub name: Option<String>,
    pub email: Option<String>,
    pub default_branch: String,
}

pub fn repository_settings(s: &AppState) -> Option<RepositorySettingsVm> {
    let d = s.repo_settings.as_ref()?;
    Some(RepositorySettingsVm {
        repo: d.repo,
        remote_name: d.remote.as_ref().map(|r| r.name.clone()),
        remote_url: d.remote.as_ref().map(|r| r.url.clone()),
        gitignore: d.gitignore.clone(),
        local_name: d.local_name.clone(),
        local_email: d.local_email.clone(),
        global_name: d.global.name.clone(),
        global_email: d.global.email.clone(),
        autocrlf: d.autocrlf,
        local_autocrlf: d.local_autocrlf.clone(),
    })
}

pub fn global_git_config(s: &AppState) -> Option<GlobalGitConfigVm> {
    let g = s.global_git.as_ref()?;
    Some(GlobalGitConfigVm {
        name: g.name.clone(),
        email: g.email.clone(),
        default_branch: g.default_branch.clone(),
    })
}
