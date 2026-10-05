//! Gitea and Forgejo (Codeberg), REST API v1. Close to GitHub's v3 in
//! shape: `owner/name` repositories, numbered pulls, combined commit
//! statuses (Gitea / Forgejo Actions post their jobs as statuses).

use corvene_models::{
    CheckConclusion, CheckStatus, GitHubRepository, HostAccount, HostAuthKind, HostEndpoint,
    PullRequest, PullRequestRef, RefCheck, RepositoryPermission, check_short_description,
};
use serde::{Deserialize, Serialize};

use crate::http::{Auth, Http, encode};
use crate::{CheckTarget, HostProvider, NewPullRequest, Result, blank_repository};

/// Pages read of a list (50 items each, Gitea's default maximum).
const MAX_PAGES: usize = 10;

pub struct Gitea {
    http: Http,
}

impl Gitea {
    pub fn new(endpoint: HostEndpoint, token: &str) -> Self {
        let auth = if token.is_empty() {
            Auth::Anonymous
        } else {
            Auth::Token(token.to_string())
        };
        Self {
            http: Http::new(endpoint, auth),
        }
    }

    /// For an OAuth token (sent as Bearer; access tokens go as `token`).
    pub fn new_bearer(endpoint: HostEndpoint, token: &str) -> Self {
        Self {
            http: Http::new(endpoint, Auth::Bearer(token.to_string())),
        }
    }

    fn repo_path(repo: &GitHubRepository) -> String {
        format!("repos/{}/{}", encode(&repo.owner), encode(&repo.name))
    }
}

#[derive(Deserialize)]
pub struct ApiUser {
    pub id: u64,
    pub login: String,
    #[serde(default)]
    pub full_name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
}

#[derive(Deserialize)]
struct ApiEmail {
    email: String,
    #[serde(default)]
    verified: bool,
    #[serde(default)]
    primary: bool,
}

#[derive(Deserialize, Default)]
pub struct ApiPermissions {
    #[serde(default)]
    pub admin: bool,
    #[serde(default)]
    pub push: bool,
    #[serde(default)]
    pub pull: bool,
}

#[derive(Deserialize)]
pub struct ApiOwner {
    pub login: String,
}

#[derive(Deserialize)]
pub struct ApiRepository {
    pub name: String,
    pub owner: ApiOwner,
    #[serde(default)]
    pub html_url: Option<String>,
    #[serde(default)]
    pub clone_url: Option<String>,
    #[serde(default)]
    pub default_branch: Option<String>,
    #[serde(default)]
    pub private: bool,
    #[serde(default)]
    pub fork: bool,
    #[serde(default)]
    pub parent: Option<Box<ApiRepository>>,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub permissions: Option<ApiPermissions>,
}

#[derive(Deserialize)]
pub struct ApiRef {
    #[serde(rename = "ref")]
    pub ref_name: String,
    #[serde(default)]
    pub sha: Option<String>,
    #[serde(default)]
    pub repo: Option<ApiRepository>,
}

#[derive(Deserialize)]
pub struct ApiLogin {
    pub login: String,
}

#[derive(Deserialize)]
pub struct ApiPullRequest {
    pub number: u64,
    pub title: String,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub user: Option<ApiLogin>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    /// Missing before Gitea 1.22: the title's `WIP:` prefix says it then.
    #[serde(default)]
    pub draft: Option<bool>,
    pub head: ApiRef,
    pub base: ApiRef,
    #[serde(default)]
    pub assignees: Option<Vec<ApiLogin>>,
    #[serde(default)]
    pub requested_reviewers: Option<Vec<ApiLogin>>,
}

#[derive(Deserialize)]
pub struct ApiStatus {
    #[serde(default)]
    pub id: u64,
    #[serde(default)]
    pub context: String,
    /// `status` in a status, `state` in older servers' output.
    #[serde(default, alias = "state")]
    pub status: String,
    #[serde(default)]
    pub target_url: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Deserialize)]
pub struct ApiCombinedStatus {
    #[serde(default)]
    pub statuses: Option<Vec<ApiStatus>>,
}

pub fn convert_repository(endpoint: &HostEndpoint, repo: ApiRepository) -> GitHubRepository {
    let full_name = format!("{}/{}", repo.owner.login, repo.name);
    let html_url = repo.html_url.unwrap_or_else(|| endpoint.web(&full_name));
    let permissions = repo.permissions.map(|p| {
        if p.admin {
            RepositoryPermission::Admin
        } else if p.push {
            RepositoryPermission::Write
        } else {
            RepositoryPermission::Read
        }
    });
    GitHubRepository {
        owner: repo.owner.login,
        name: repo.name,
        clone_url: repo.clone_url.unwrap_or_else(|| format!("{html_url}.git")),
        html_url,
        default_branch: repo.default_branch,
        private: repo.private,
        fork: repo.fork,
        parent: repo
            .parent
            .map(|p| Box::new(convert_repository(endpoint, *p))),
        archived: repo.archived,
        permissions,
        ..blank_repository(endpoint)
    }
}

/// Gitea's `WORK_IN_PROGRESS_PREFIXES` defaults.
fn has_wip_prefix(title: &str) -> bool {
    let lower = title.trim_start().to_ascii_lowercase();
    lower.starts_with("wip:") || lower.starts_with("[wip]")
}

/// A pull request as Corvene's. One opened with AGit (`git push origin
/// HEAD:refs/for/main`) has no head branch: its head is `refs/pull/N/head`
/// on the base repository, so it is given no head repository and checks
/// out from that ref like a pull request from a deleted fork.
pub fn convert_pull_request(endpoint: &HostEndpoint, pr: ApiPullRequest) -> PullRequest {
    let convert_ref = |r: ApiRef| PullRequestRef {
        repository: r
            .repo
            .filter(|_| !r.ref_name.starts_with("refs/"))
            .map(|repo| convert_repository(endpoint, repo)),
        ref_name: r.ref_name,
        sha: r.sha.unwrap_or_default(),
    };
    let draft = pr.draft.unwrap_or_else(|| has_wip_prefix(&pr.title));
    let logins = |users: Option<Vec<ApiLogin>>| {
        users
            .unwrap_or_default()
            .into_iter()
            .map(|u| u.login)
            .collect()
    };
    PullRequest {
        number: pr.number,
        title: pr.title,
        created_at: pr.created_at.unwrap_or_default(),
        updated_at: pr.updated_at.unwrap_or_default(),
        head: convert_ref(pr.head),
        base: convert_ref(pr.base),
        author: pr.user.map(|u| u.login).unwrap_or_default(),
        draft,
        body: pr.body.unwrap_or_default(),
        assignees: logins(pr.assignees),
        requested_reviewers: logins(pr.requested_reviewers),
    }
}

/// A commit status as a check. Gitea has no durations on statuses; its
/// Actions put "Successful in 12s" in the description, which is kept.
/// Forgejo Actions' target URLs are server-relative.
pub fn convert_status(endpoint: &HostEndpoint, status: ApiStatus) -> RefCheck {
    let (state, conclusion) = match status.status.as_str() {
        "success" => (CheckStatus::Completed, Some(CheckConclusion::Success)),
        "failure" | "error" => (CheckStatus::Completed, Some(CheckConclusion::Failure)),
        "warning" => (CheckStatus::Completed, Some(CheckConclusion::Neutral)),
        "skipped" => (CheckStatus::Completed, Some(CheckConclusion::Skipped)),
        _ => (CheckStatus::InProgress, None),
    };
    RefCheck {
        id: status.id,
        name: status.context,
        description: status
            .description
            .filter(|d| !d.trim().is_empty())
            .unwrap_or_else(|| check_short_description(state, conclusion, None)),
        status: state,
        conclusion,
        app_name: String::new(),
        html_url: status
            .target_url
            .filter(|u| !u.is_empty())
            .map(|u| match u.strip_prefix('/') {
                Some(path) if !path.starts_with('/') => format!("{}/{path}", endpoint.origin()),
                _ => u,
            }),
        check_suite_id: None,
        actions_workflow: None,
        job_steps: None,
    }
}

#[derive(Serialize)]
struct CreatePullRequest<'a> {
    head: String,
    base: &'a str,
    title: String,
    body: &'a str,
}

impl HostProvider for Gitea {
    fn endpoint(&self) -> &HostEndpoint {
        &self.http.endpoint
    }

    fn current_user(&self) -> Result<HostAccount> {
        let (user, _): (ApiUser, _) = self.http.get("user")?;
        let mut emails: Vec<String> = Vec::new();
        if let Ok((list, _)) = self.http.get::<Vec<ApiEmail>>("user/emails") {
            let mut list: Vec<ApiEmail> = list.into_iter().filter(|e| e.verified).collect();
            list.sort_by_key(|e| !e.primary);
            emails.extend(list.into_iter().map(|e| e.email));
        }
        if let Some(email) = user.email.filter(|e| !e.is_empty())
            && !emails.contains(&email)
        {
            emails.insert(0, email);
        }
        Ok(HostAccount {
            endpoint: self.http.endpoint.clone(),
            id: user.id.to_string(),
            login: user.login,
            name: user.full_name.filter(|n| !n.is_empty()),
            avatar_url: user.avatar_url,
            emails,
            auth: HostAuthKind::Token,
            basic_user: None,
            oauth_client_id: None,
            expires_at: None,
            needs_reauth: false,
        })
    }

    fn repository(&self, owner: &str, name: &str) -> Result<GitHubRepository> {
        let (repo, _): (ApiRepository, _) =
            self.http
                .get(&format!("repos/{}/{}", encode(owner), encode(name)))?;
        Ok(convert_repository(&self.http.endpoint, repo))
    }

    fn user_repositories(&self) -> Result<Vec<GitHubRepository>> {
        let repos: Vec<ApiRepository> = self.http.get_all("user/repos?limit=50", MAX_PAGES)?;
        Ok(repos
            .into_iter()
            .filter(|r| !r.archived)
            .map(|r| convert_repository(&self.http.endpoint, r))
            .collect())
    }

    fn open_pull_requests(&self, repo: &GitHubRepository) -> Result<Vec<PullRequest>> {
        let path = format!("{}/pulls?state=open&limit=50", Self::repo_path(repo));
        let prs: Vec<ApiPullRequest> = self.http.get_all_parallel(&path, 50, MAX_PAGES)?;
        let mut prs: Vec<PullRequest> = prs
            .into_iter()
            .map(|pr| convert_pull_request(&self.http.endpoint, pr))
            .collect();
        prs.sort_by_key(|p| std::cmp::Reverse(p.number));
        Ok(prs)
    }

    fn create_pull_request(
        &self,
        repo: &GitHubRepository,
        new: &NewPullRequest,
    ) -> Result<PullRequest> {
        let head = match &new.head_repository {
            Some(fork) => format!("{}:{}", fork.owner, new.head),
            None => new.head.clone(),
        };
        let title = if new.draft && !has_wip_prefix(&new.title) {
            format!("WIP: {}", new.title)
        } else {
            new.title.clone()
        };
        let body = CreatePullRequest {
            head,
            base: &new.base,
            title,
            body: &new.body,
        };
        let pr: ApiPullRequest = self
            .http
            .post(&format!("{}/pulls", Self::repo_path(repo)), &body)?;
        Ok(convert_pull_request(&self.http.endpoint, pr))
    }

    fn ref_checks(
        &self,
        repo: &GitHubRepository,
        target: &CheckTarget,
    ) -> Result<Option<Vec<RefCheck>>> {
        let combined: ApiCombinedStatus = match self.http.get(&format!(
            "{}/commits/{}/status",
            Self::repo_path(repo),
            encode(&target.sha)
        )) {
            Ok((combined, _)) => combined,
            Err(err) if self.http.auth == Auth::Anonymous && err.is_hidden() => return Ok(None),
            Err(err) => return Err(err),
        };
        let statuses = combined.statuses.unwrap_or_default();
        if statuses.is_empty() {
            return Ok(None);
        }
        Ok(Some(
            statuses
                .into_iter()
                .map(|s| convert_status(&self.http.endpoint, s))
                .collect(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use corvene_models::HostKind;

    #[test]
    fn converts_pull_requests() {
        let endpoint = HostEndpoint::public(HostKind::Gitea);
        let pr: ApiPullRequest = serde_json::from_str(
            r#"{"number": 4, "title": "WIP: try", "body": "x", "user": {"login": "mona"},
                "created_at": "2026-01-02T03:04:05Z", "updated_at": "2026-01-02T03:04:05Z",
                "head": {"ref": "try", "sha": "abc", "repo": {"name": "n", "owner": {"login": "fork"},
                    "html_url": "https://codeberg.org/fork/n", "clone_url": "https://codeberg.org/fork/n.git",
                    "fork": true, "permissions": {"admin": false, "push": true, "pull": true}}},
                "base": {"ref": "main", "sha": "def", "repo": {"name": "n", "owner": {"login": "up"}}},
                "requested_reviewers": null}"#,
        )
        .unwrap();
        let pr = convert_pull_request(&endpoint, pr);
        assert!(pr.draft);
        assert_eq!(pr.author, "mona");
        let head = pr.head.repository.unwrap();
        assert_eq!(head.full_name(), "fork/n");
        assert_eq!(head.permissions, Some(RepositoryPermission::Write));
        let base = pr.base.repository.unwrap();
        assert_eq!(base.html_url, "https://codeberg.org/up/n");
        assert_eq!(base.endpoint, "https://codeberg.org/api/v1");
    }

    #[test]
    fn converts_statuses() {
        let combined: ApiCombinedStatus = serde_json::from_str(
            r#"{"state": "failure", "statuses": [
                {"id": 1, "context": "ci / test (push)", "status": "failure",
                 "target_url": "/a/b/actions/runs/1/jobs/0",
                 "description": "Failing after 12s"},
                {"id": 2, "context": "lint", "status": "pending", "target_url": ""}]}"#,
        )
        .unwrap();
        let endpoint = HostEndpoint::public(HostKind::Gitea);
        let checks: Vec<RefCheck> = combined
            .statuses
            .unwrap()
            .into_iter()
            .map(|s| convert_status(&endpoint, s))
            .collect();
        assert!(checks[0].is_failure());
        assert_eq!(checks[0].description, "Failing after 12s");
        assert_eq!(
            checks[0].html_url.as_deref(),
            Some("https://codeberg.org/a/b/actions/runs/1/jobs/0")
        );
        assert_eq!(checks[1].status, CheckStatus::InProgress);
        assert_eq!(checks[1].html_url, None);
        let empty: ApiCombinedStatus =
            serde_json::from_str(r#"{"state": "", "statuses": null}"#).unwrap();
        assert!(empty.statuses.is_none());
    }
}
