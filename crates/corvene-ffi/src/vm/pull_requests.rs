//! The open pull requests of a GitHub repository (the branch picker's
//! Pull Requests tab).

use corvene_core::AppState;

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct PullRequestVm {
    pub number: u64,
    pub title: String,
    pub author: String,
    pub draft: bool,
    pub head_ref: String,
    pub head_sha: String,
    pub base_ref: String,
    /// `owner/name` of the head repository when it is a fork.
    pub head_repository: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    /// The current branch is this pull request's head.
    pub checked_out: bool,
}

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct PullRequestsVm {
    pub repo: u64,
    pub pull_requests: Vec<PullRequestVm>,
}

pub fn pull_requests(s: &AppState, repo: u64) -> Option<PullRequestsVm> {
    let rs = s.repo_states.get(&repo)?;
    let current = rs
        .info
        .as_ref()
        .and_then(|i| i.tip.branch_name().map(str::to_string));
    let github = s.repository(repo).and_then(|r| r.github.as_ref());
    let pull_requests = s
        .pull_requests_for(repo)
        .iter()
        .map(|pr| {
            let fork = pr
                .head
                .repository
                .as_ref()
                .filter(|head| github.is_none_or(|g| g.owner != head.owner || g.name != head.name))
                .map(|head| format!("{}/{}", head.owner, head.name));
            let checked_out =
                fork.is_none() && current.as_deref() == Some(pr.head.ref_name.as_str());
            PullRequestVm {
                number: pr.number,
                title: pr.title.clone(),
                author: pr.author.clone(),
                draft: pr.draft,
                head_ref: pr.head.ref_name.clone(),
                head_sha: pr.head.sha.clone(),
                base_ref: pr.base.ref_name.clone(),
                head_repository: fork,
                created_at: pr.created_at.clone(),
                updated_at: pr.updated_at.clone(),
                checked_out,
            }
        })
        .collect();
    Some(PullRequestsVm {
        repo,
        pull_requests,
    })
}
