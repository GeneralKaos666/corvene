//! Bitbucket Cloud, REST API 2.0. Repositories are `workspace/slug`;
//! lists come in `{values, next}` pages; pull requests have no git refs of
//! their own; CI is the commit's build statuses (Pipelines reports there
//! too). API tokens authenticate as `email:token` (Basic); OAuth and
//! access tokens as Bearer.

use corvene_models::{
    CheckConclusion, CheckStatus, GitHubRepository, HostAccount, HostAuthKind, HostEndpoint,
    PullRequest, PullRequestRef, RefCheck, RepositoryPermission, check_short_description,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::http::{Auth, Http, encode};
use crate::{CheckTarget, HostProvider, NewPullRequest, Result, blank_repository};

/// Pages read of a list.
const MAX_PAGES: usize = 10;

pub struct Bitbucket {
    http: Http,
}

impl Bitbucket {
    /// `basic_user` (the Atlassian account email) makes `token` an API
    /// token sent with Basic authentication; without it the token is a
    /// Bearer one.
    pub fn new(endpoint: HostEndpoint, token: &str, basic_user: Option<&str>) -> Self {
        let auth = match (token.is_empty(), basic_user.filter(|u| !u.is_empty())) {
            (true, _) => Auth::Anonymous,
            (false, Some(user)) => Auth::Basic {
                user: user.to_string(),
                token: token.to_string(),
            },
            (false, None) => Auth::Bearer(token.to_string()),
        };
        Self {
            http: Http::new(endpoint, auth),
        }
    }

    fn repo_path(repo: &GitHubRepository) -> String {
        format!(
            "repositories/{}/{}",
            encode(&repo.owner),
            encode(&repo.name)
        )
    }

    /// Every page of a `{values, next}` list.
    fn get_values<T: DeserializeOwned>(&self, path: &str) -> Result<Vec<T>> {
        let mut items = Vec::new();
        let mut next = Some(path.to_string());
        let mut pages = 0;
        while let Some(path) = next.take() {
            let (page, _): (Page<T>, _) = self.http.get(&path)?;
            items.extend(page.values);
            pages += 1;
            next = page.next.filter(|_| pages < MAX_PAGES);
        }
        Ok(items)
    }
}

/// Page sizes for the open pull requests: a page that fails is read again
/// in pages of the next size.
const PULL_REQUEST_PAGES: [usize; 3] = [50, 10, 1];

impl Bitbucket {
    /// The open pull requests. One pull request whose commit Bitbucket
    /// lost fails its whole page with a 400 ("Either a cset or a repo and
    /// commit hash must be provided"); such a page is read again in
    /// smaller pages and the broken pull request left out.
    fn pull_request_pages(&self, path: &str) -> Result<Vec<ApiPullRequest>> {
        let mut items = Vec::new();
        for page in 1..=MAX_PAGES {
            if self.pull_request_page(path, 0, page, &mut items)? {
                break;
            }
        }
        Ok(items)
    }

    /// Page `page` at size `PULL_REQUEST_PAGES[level]` into `items`; true
    /// at the end of the list.
    fn pull_request_page(
        &self,
        path: &str,
        level: usize,
        page: usize,
        items: &mut Vec<ApiPullRequest>,
    ) -> Result<bool> {
        let size = PULL_REQUEST_PAGES[level];
        let url = format!("{path}&pagelen={size}&page={page}");
        match self.http.get::<Page<ApiPullRequest>>(&url) {
            Ok((batch, _)) => {
                let end = batch.next.is_none() || batch.values.is_empty();
                items.extend(batch.values);
                Ok(end)
            }
            Err(err) if err.status() == Some(400) => {
                let Some(&smaller) = PULL_REQUEST_PAGES.get(level + 1) else {
                    tracing::warn!(page, %err, "left out a pull request Bitbucket cannot list");
                    return Ok(false);
                };
                let per = size / smaller;
                let first = (page - 1) * per + 1;
                for sub in first..first + per {
                    if self.pull_request_page(path, level + 1, sub, items)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            Err(err) => Err(err),
        }
    }
}

#[derive(Deserialize)]
struct Page<T> {
    #[serde(default = "Vec::new")]
    values: Vec<T>,
    #[serde(default)]
    next: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct ApiHref {
    #[serde(default)]
    pub href: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct ApiNamedHref {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub href: String,
}

#[derive(Deserialize, Default)]
pub struct ApiLinks {
    #[serde(default)]
    pub html: Option<ApiHref>,
    #[serde(default)]
    pub avatar: Option<ApiHref>,
    #[serde(default)]
    pub clone: Vec<ApiNamedHref>,
}

#[derive(Deserialize)]
pub struct ApiUser {
    #[serde(default)]
    pub uuid: Option<String>,
    #[serde(default)]
    pub account_id: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub nickname: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub links: ApiLinks,
}

impl ApiUser {
    fn login(&self) -> String {
        self.username
            .clone()
            .or_else(|| self.nickname.clone())
            .or_else(|| self.display_name.clone())
            .unwrap_or_default()
    }
}

#[derive(Deserialize)]
struct ApiEmail {
    email: String,
    #[serde(default)]
    is_primary: bool,
    #[serde(default)]
    is_confirmed: bool,
}

#[derive(Deserialize)]
pub struct ApiBranchName {
    pub name: String,
}

#[derive(Deserialize)]
pub struct ApiRepository {
    pub full_name: String,
    #[serde(default)]
    pub links: ApiLinks,
    #[serde(default)]
    pub mainbranch: Option<ApiBranchName>,
    #[serde(default)]
    pub is_private: bool,
    #[serde(default)]
    pub parent: Option<Box<ApiRepository>>,
}

#[derive(Deserialize)]
pub struct ApiCommit {
    #[serde(default)]
    pub hash: Option<String>,
}

#[derive(Deserialize)]
pub struct ApiEnd {
    pub branch: ApiBranchName,
    #[serde(default)]
    pub commit: Option<ApiCommit>,
    #[serde(default)]
    pub repository: Option<ApiRepository>,
}

#[derive(Deserialize)]
pub struct ApiPullRequest {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub created_on: Option<String>,
    #[serde(default)]
    pub updated_on: Option<String>,
    #[serde(default)]
    pub author: Option<ApiUser>,
    pub source: ApiEnd,
    pub destination: ApiEnd,
    #[serde(default)]
    pub reviewers: Vec<ApiUser>,
}

#[derive(Deserialize)]
pub struct ApiStatus {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub name: Option<String>,
    pub state: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Deserialize)]
struct ApiWorkspaceAccess {
    workspace: ApiWorkspace,
}

#[derive(Deserialize)]
struct ApiWorkspace {
    slug: String,
}

#[derive(Deserialize)]
struct ApiRepositoryPermission {
    permission: String,
}

/// A clone URL without the `user@` Bitbucket puts in HTTPS ones.
fn without_user(url: &str) -> String {
    match url.split_once("://") {
        Some((scheme, rest)) => {
            let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
            let host = authority.rsplit('@').next().unwrap_or(authority);
            format!("{scheme}://{host}/{path}")
        }
        None => url.to_string(),
    }
}

pub fn convert_repository(endpoint: &HostEndpoint, repo: ApiRepository) -> GitHubRepository {
    let (owner, name) = crate::split_path(&repo.full_name);
    let html_url = repo
        .links
        .html
        .and_then(|h| h.href)
        .unwrap_or_else(|| endpoint.web(&repo.full_name));
    let clone_url = repo
        .links
        .clone
        .iter()
        .find(|c| c.name == "https")
        .map(|c| without_user(&c.href))
        .unwrap_or_else(|| format!("{html_url}.git"));
    GitHubRepository {
        owner,
        name,
        html_url,
        clone_url,
        default_branch: repo.mainbranch.map(|b| b.name),
        private: repo.is_private,
        fork: repo.parent.is_some(),
        parent: repo
            .parent
            .map(|p| Box::new(convert_repository(endpoint, *p))),
        ..blank_repository(endpoint)
    }
}

pub fn convert_pull_request(
    endpoint: &HostEndpoint,
    pr: ApiPullRequest,
    target: &GitHubRepository,
) -> PullRequest {
    let convert_end = |end: ApiEnd| {
        let repository = match end.repository {
            Some(repo) if !repo.full_name.eq_ignore_ascii_case(&target.full_name()) => {
                Some(convert_repository(endpoint, repo))
            }
            _ => Some(target.clone()),
        };
        PullRequestRef {
            ref_name: end.branch.name,
            sha: end.commit.and_then(|c| c.hash).unwrap_or_default(),
            repository,
        }
    };
    PullRequest {
        number: pr.id,
        title: pr.title,
        created_at: pr.created_on.unwrap_or_default(),
        updated_at: pr.updated_on.unwrap_or_default(),
        head: convert_end(pr.source),
        base: convert_end(pr.destination),
        author: pr.author.map(|a| a.login()).unwrap_or_default(),
        draft: pr.draft,
        body: pr.description.unwrap_or_default(),
        assignees: Vec::new(),
        requested_reviewers: pr.reviewers.iter().map(ApiUser::login).collect(),
    }
}

/// A stable id for a status (Bitbucket keys them by name).
fn key_id(key: &str) -> u64 {
    // FNV-1a
    key.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, b| {
        (hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

pub fn convert_status(status: ApiStatus) -> RefCheck {
    let (state, conclusion) = match status.state.as_str() {
        "SUCCESSFUL" => (CheckStatus::Completed, Some(CheckConclusion::Success)),
        "FAILED" => (CheckStatus::Completed, Some(CheckConclusion::Failure)),
        "STOPPED" => (CheckStatus::Completed, Some(CheckConclusion::Cancelled)),
        _ => (CheckStatus::InProgress, None),
    };
    RefCheck {
        id: key_id(&status.key),
        name: status
            .name
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| status.key.clone()),
        description: status
            .description
            .filter(|d| !d.trim().is_empty())
            .unwrap_or_else(|| check_short_description(state, conclusion, None)),
        status: state,
        conclusion,
        app_name: String::new(),
        html_url: status.url.filter(|u| !u.is_empty()),
        check_suite_id: None,
        actions_workflow: None,
        job_steps: None,
    }
}

#[derive(Serialize)]
struct BranchName<'a> {
    name: &'a str,
}

#[derive(Serialize)]
struct FullName {
    full_name: String,
}

#[derive(Serialize)]
struct Source<'a> {
    branch: BranchName<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    repository: Option<FullName>,
}

#[derive(Serialize)]
struct Destination<'a> {
    branch: BranchName<'a>,
}

#[derive(Serialize)]
struct CreatePullRequest<'a> {
    title: &'a str,
    description: &'a str,
    source: Source<'a>,
    destination: Destination<'a>,
    draft: bool,
}

impl HostProvider for Bitbucket {
    fn endpoint(&self) -> &HostEndpoint {
        &self.http.endpoint
    }

    fn current_user(&self) -> Result<HostAccount> {
        let (user, _): (ApiUser, _) = self.http.get("user")?;
        let mut emails: Vec<String> = Vec::new();
        if let Ok(mut list) = self.get_values::<ApiEmail>("user/emails") {
            list.retain(|e| e.is_confirmed);
            list.sort_by_key(|e| !e.is_primary);
            emails.extend(list.into_iter().map(|e| e.email));
        }
        let login = user.login();
        Ok(HostAccount {
            endpoint: self.http.endpoint.clone(),
            id: user
                .uuid
                .clone()
                .or_else(|| user.account_id.clone())
                .unwrap_or_else(|| login.clone()),
            name: user.display_name.clone().filter(|n| n != &login),
            avatar_url: user.links.avatar.and_then(|a| a.href),
            login,
            emails,
            auth: HostAuthKind::Token,
            basic_user: None,
            oauth_client_id: None,
            expires_at: None,
            needs_reauth: false,
        })
    }

    fn repository(&self, owner: &str, name: &str) -> Result<GitHubRepository> {
        let path = format!("repositories/{}/{}", encode(owner), encode(name));
        let (repo, _): (ApiRepository, _) = self.http.get(&path)?;
        let mut repo = convert_repository(&self.http.endpoint, repo);
        // the user's own permission is a separate (workspace) call
        let query = encode(&format!("repository.full_name=\"{owner}/{name}\""));
        if let Ok(permissions) = self.get_values::<ApiRepositoryPermission>(&format!(
            "user/workspaces/{}/permissions/repositories?q={query}",
            encode(owner)
        )) {
            repo.permissions = permissions
                .first()
                .and_then(|p| match p.permission.as_str() {
                    "admin" => Some(RepositoryPermission::Admin),
                    "write" => Some(RepositoryPermission::Write),
                    "read" => Some(RepositoryPermission::Read),
                    _ => None,
                });
        }
        Ok(repo)
    }

    fn user_repositories(&self) -> Result<Vec<GitHubRepository>> {
        let workspaces: Vec<ApiWorkspaceAccess> = self.get_values("user/workspaces?pagelen=100")?;
        let mut repos = Vec::new();
        for access in workspaces.into_iter().take(20) {
            let list: Vec<ApiRepository> = self.get_values(&format!(
                "repositories/{}?role=member&pagelen=100",
                encode(&access.workspace.slug)
            ))?;
            repos.extend(
                list.into_iter()
                    .map(|r| convert_repository(&self.http.endpoint, r)),
            );
        }
        Ok(repos)
    }

    fn open_pull_requests(&self, repo: &GitHubRepository) -> Result<Vec<PullRequest>> {
        let path = format!("{}/pullrequests?state=OPEN", Self::repo_path(repo));
        let prs = self.pull_request_pages(&path)?;
        let mut prs: Vec<PullRequest> = prs
            .into_iter()
            .map(|pr| convert_pull_request(&self.http.endpoint, pr, repo))
            .collect();
        prs.sort_by_key(|p| std::cmp::Reverse(p.number));
        Ok(prs)
    }

    fn create_pull_request(
        &self,
        repo: &GitHubRepository,
        new: &NewPullRequest,
    ) -> Result<PullRequest> {
        let body = CreatePullRequest {
            title: &new.title,
            description: &new.body,
            source: Source {
                branch: BranchName { name: &new.head },
                repository: new.head_repository.as_ref().map(|r| FullName {
                    full_name: r.full_name(),
                }),
            },
            destination: Destination {
                branch: BranchName { name: &new.base },
            },
            draft: new.draft,
        };
        let pr: ApiPullRequest = self
            .http
            .post(&format!("{}/pullrequests", Self::repo_path(repo)), &body)?;
        Ok(convert_pull_request(&self.http.endpoint, pr, repo))
    }

    fn ref_checks(
        &self,
        repo: &GitHubRepository,
        target: &CheckTarget,
    ) -> Result<Option<Vec<RefCheck>>> {
        let statuses: Vec<ApiStatus> = match self.get_values(&format!(
            "{}/commit/{}/statuses?pagelen=100",
            Self::repo_path(repo),
            encode(&target.sha)
        )) {
            Ok(statuses) => statuses,
            Err(err) if self.http.auth == Auth::Anonymous && err.is_hidden() => return Ok(None),
            Err(err) => return Err(err),
        };
        if statuses.is_empty() {
            return Ok(None);
        }
        Ok(Some(statuses.into_iter().map(convert_status).collect()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use corvene_models::HostKind;

    fn endpoint() -> HostEndpoint {
        HostEndpoint::public(HostKind::Bitbucket)
    }

    #[test]
    fn converts_repositories() {
        let repo: ApiRepository = serde_json::from_str(
            r#"{"full_name": "ws/repo", "is_private": true, "mainbranch": {"name": "main"},
                "links": {"html": {"href": "https://bitbucket.org/ws/repo"},
                          "clone": [{"name": "https", "href": "https://me@bitbucket.org/ws/repo.git"},
                                    {"name": "ssh", "href": "git@bitbucket.org:ws/repo.git"}]}}"#,
        )
        .unwrap();
        let repo = convert_repository(&endpoint(), repo);
        assert_eq!(repo.owner, "ws");
        assert_eq!(repo.name, "repo");
        assert_eq!(repo.clone_url, "https://bitbucket.org/ws/repo.git");
        assert_eq!(repo.default_branch.as_deref(), Some("main"));
        assert!(repo.private);
        assert_eq!(repo.endpoint, "https://api.bitbucket.org/2.0");
    }

    #[test]
    fn converts_pull_requests_and_statuses() {
        let target = GitHubRepository {
            owner: "ws".into(),
            name: "repo".into(),
            html_url: "https://bitbucket.org/ws/repo".into(),
            ..blank_repository(&endpoint())
        };
        let pr: ApiPullRequest = serde_json::from_str(
            r#"{"id": 3, "title": "T", "draft": false, "created_on": "2026-01-02T03:04:05.123456+00:00",
                "author": {"nickname": "mona", "display_name": "Mona"},
                "source": {"branch": {"name": "feat"}, "commit": {"hash": "0123456789ab"},
                           "repository": {"full_name": "fork/repo"}},
                "destination": {"branch": {"name": "main"}, "repository": {"full_name": "WS/repo"}}}"#,
        )
        .unwrap();
        let pr = convert_pull_request(&endpoint(), pr, &target);
        assert_eq!(pr.author, "mona");
        assert_eq!(pr.head.repository.unwrap().full_name(), "fork/repo");
        assert_eq!(pr.base.repository.unwrap().html_url, target.html_url);
        let status: ApiStatus = serde_json::from_str(
            r#"{"key": "build-1", "name": "Pipeline #1", "state": "STOPPED", "url": "https://x"}"#,
        )
        .unwrap();
        let check = convert_status(status);
        assert_eq!(check.conclusion, Some(CheckConclusion::Cancelled));
        assert_eq!(check.id, key_id("build-1"));
        let page: Page<ApiStatus> = serde_json::from_str(r#"{"pagelen": 10}"#).unwrap();
        assert!(page.values.is_empty() && page.next.is_none());
    }
}
