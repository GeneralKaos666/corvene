//! GitLab (gitlab.com and self-managed, REST API v4). Projects are
//! addressed by their URL-encoded full path (`group%2Fsub%2Fproject`);
//! merge requests by their per-project `iid`; CI comes from the newest
//! pipeline of a commit and its jobs, grouped by stage.

use std::collections::HashMap;

use corvene_models::{
    CheckConclusion, CheckStatus, GitHubRepository, HostAccount, HostAuthKind, HostEndpoint,
    PullRequest, PullRequestRef, RefCheck, RepositoryPermission, WorkflowRun, check_duration_ms,
    check_short_description,
};
use serde::{Deserialize, Serialize};

use crate::http::{Auth, Http, encode};
use crate::{CheckTarget, HostProvider, NewPullRequest, Result, blank_repository, split_path};

/// Pages read of a list (100 items each).
const MAX_PAGES: usize = 10;
/// Fork projects read for one merge request list.
const MAX_FORK_LOOKUPS: usize = 30;

pub struct GitLab {
    http: Http,
}

impl GitLab {
    pub fn new(endpoint: HostEndpoint, token: &str) -> Self {
        let auth = if token.is_empty() {
            Auth::Anonymous
        } else {
            Auth::Bearer(token.to_string())
        };
        Self {
            http: Http::new(endpoint, auth),
        }
    }

    fn project_path(repo: &GitHubRepository) -> String {
        format!("projects/{}", encode(&repo.full_name()))
    }

    fn convert(&self, project: ApiProject) -> GitHubRepository {
        convert_project(&self.http.endpoint, project)
    }
}

#[derive(Deserialize)]
pub struct ApiUser {
    pub id: u64,
    pub username: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub public_email: Option<String>,
    #[serde(default)]
    pub commit_email: Option<String>,
}

#[derive(Deserialize)]
struct ApiEmail {
    email: String,
    #[serde(default)]
    confirmed_at: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct ApiAccess {
    #[serde(default)]
    pub access_level: u32,
}

#[derive(Deserialize, Default)]
pub struct ApiPermissions {
    #[serde(default)]
    pub project_access: Option<ApiAccess>,
    #[serde(default)]
    pub group_access: Option<ApiAccess>,
}

#[derive(Deserialize)]
pub struct ApiForkedFrom {
    pub path_with_namespace: String,
    #[serde(default)]
    pub web_url: Option<String>,
    #[serde(default)]
    pub http_url_to_repo: Option<String>,
    #[serde(default)]
    pub default_branch: Option<String>,
}

#[derive(Deserialize)]
pub struct ApiProject {
    pub id: u64,
    pub path_with_namespace: String,
    #[serde(default)]
    pub web_url: Option<String>,
    #[serde(default)]
    pub http_url_to_repo: Option<String>,
    #[serde(default)]
    pub default_branch: Option<String>,
    #[serde(default)]
    pub visibility: Option<String>,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub forked_from_project: Option<ApiForkedFrom>,
    #[serde(default)]
    pub permissions: Option<ApiPermissions>,
}

#[derive(Deserialize)]
pub struct ApiAuthor {
    pub username: String,
}

#[derive(Deserialize)]
pub struct ApiMergeRequest {
    pub iid: u64,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub author: ApiAuthor,
    pub source_branch: String,
    pub target_branch: String,
    pub source_project_id: u64,
    pub target_project_id: u64,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub work_in_progress: bool,
    #[serde(default)]
    pub sha: Option<String>,
    #[serde(default)]
    pub assignees: Vec<ApiAuthor>,
    #[serde(default)]
    pub reviewers: Vec<ApiAuthor>,
}

#[derive(Deserialize)]
pub struct ApiPipeline {
    pub id: u64,
    #[serde(default)]
    pub created_at: Option<String>,
}

#[derive(Deserialize)]
pub struct ApiJob {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub stage: String,
    pub status: String,
    #[serde(default)]
    pub web_url: Option<String>,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub finished_at: Option<String>,
    #[serde(default)]
    pub allow_failure: bool,
}

#[derive(Deserialize)]
pub struct ApiCommitStatus {
    pub id: u64,
    pub name: String,
    pub status: String,
    #[serde(default)]
    pub target_url: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub finished_at: Option<String>,
    #[serde(default)]
    pub allow_failure: bool,
}

/// GitLab access levels: 30 developer can push, 40 maintainer and 50 owner
/// administer.
fn permission(permissions: Option<&ApiPermissions>) -> Option<RepositoryPermission> {
    let p = permissions?;
    let level = [p.project_access.as_ref(), p.group_access.as_ref()]
        .into_iter()
        .flatten()
        .map(|a| a.access_level)
        .max()?;
    Some(match level {
        40.. => RepositoryPermission::Admin,
        30.. => RepositoryPermission::Write,
        _ => RepositoryPermission::Read,
    })
}

pub fn convert_project(endpoint: &HostEndpoint, project: ApiProject) -> GitHubRepository {
    let (owner, name) = split_path(&project.path_with_namespace);
    let html_url = project
        .web_url
        .unwrap_or_else(|| endpoint.web(&project.path_with_namespace));
    let parent = project.forked_from_project.map(|p| {
        let (owner, name) = split_path(&p.path_with_namespace);
        let html_url = p
            .web_url
            .unwrap_or_else(|| endpoint.web(&p.path_with_namespace));
        Box::new(GitHubRepository {
            owner,
            name,
            clone_url: p
                .http_url_to_repo
                .unwrap_or_else(|| format!("{html_url}.git")),
            html_url,
            default_branch: p.default_branch,
            ..blank_repository(endpoint)
        })
    });
    GitHubRepository {
        owner,
        name,
        clone_url: project
            .http_url_to_repo
            .unwrap_or_else(|| format!("{html_url}.git")),
        html_url,
        default_branch: project.default_branch,
        private: project.visibility.as_deref().is_some_and(|v| v != "public"),
        fork: parent.is_some(),
        parent,
        archived: project.archived,
        permissions: permission(project.permissions.as_ref()),
        ..blank_repository(endpoint)
    }
}

/// A job's or commit status's state as a check: `manual` jobs that never
/// ran and failures that are allowed do not fail the pipeline, as on
/// GitLab ("passed with warnings").
pub fn job_state(status: &str, allow_failure: bool) -> (CheckStatus, Option<CheckConclusion>) {
    match status {
        "success" => (CheckStatus::Completed, Some(CheckConclusion::Success)),
        "failed" if allow_failure => (CheckStatus::Completed, Some(CheckConclusion::Neutral)),
        "failed" => (CheckStatus::Completed, Some(CheckConclusion::Failure)),
        "canceled" | "canceling" => (CheckStatus::Completed, Some(CheckConclusion::Cancelled)),
        "skipped" => (CheckStatus::Completed, Some(CheckConclusion::Skipped)),
        "manual" => (CheckStatus::Completed, Some(CheckConclusion::Neutral)),
        "running" => (CheckStatus::InProgress, None),
        // created, waiting_for_resource, preparing, pending, scheduled,
        // waiting_for_callback
        _ => (CheckStatus::Queued, None),
    }
}

pub fn convert_job(job: ApiJob, pipeline: &ApiPipeline) -> RefCheck {
    let (status, conclusion) = job_state(&job.status, job.allow_failure);
    let duration = check_duration_ms(job.started_at.as_deref(), job.finished_at.as_deref());
    RefCheck {
        id: job.id,
        name: job.name,
        description: check_short_description(status, conclusion, duration),
        status,
        conclusion,
        app_name: "GitLab CI/CD".into(),
        html_url: job.web_url,
        check_suite_id: None,
        // the popover groups checks by workflow name: one group per stage
        actions_workflow: (!job.stage.is_empty()).then(|| WorkflowRun {
            id: pipeline.id,
            workflow_id: 0,
            name: job.stage,
            event: String::new(),
            check_suite_id: None,
            created_at: pipeline.created_at.clone().unwrap_or_default(),
        }),
        job_steps: None,
    }
}

pub fn convert_status(status: ApiCommitStatus) -> RefCheck {
    let (state, conclusion) = job_state(&status.status, status.allow_failure);
    let duration = check_duration_ms(status.started_at.as_deref(), status.finished_at.as_deref());
    RefCheck {
        id: status.id,
        name: status.name,
        description: status
            .description
            .filter(|d| !d.trim().is_empty())
            .unwrap_or_else(|| check_short_description(state, conclusion, duration)),
        status: state,
        conclusion,
        app_name: "GitLab".into(),
        html_url: status.target_url,
        check_suite_id: None,
        actions_workflow: None,
        job_steps: None,
    }
}

/// A merge request as a pull request: `head` on `source` (the target
/// project unless the request comes from a fork; `None` when the fork is
/// gone or private), `base` on `target`.
pub fn convert_merge_request(
    mr: ApiMergeRequest,
    target: &GitHubRepository,
    source: Option<GitHubRepository>,
) -> PullRequest {
    let draft = mr.draft || mr.work_in_progress;
    PullRequest {
        number: mr.iid,
        title: mr.title,
        created_at: mr.created_at,
        updated_at: mr.updated_at,
        head: PullRequestRef {
            ref_name: mr.source_branch,
            sha: mr.sha.unwrap_or_default(),
            repository: source,
        },
        base: PullRequestRef {
            ref_name: mr.target_branch,
            sha: String::new(),
            repository: Some(target.clone()),
        },
        author: mr.author.username,
        draft,
        body: mr.description.unwrap_or_default(),
        assignees: mr.assignees.into_iter().map(|u| u.username).collect(),
        requested_reviewers: mr.reviewers.into_iter().map(|u| u.username).collect(),
    }
}

#[derive(Serialize)]
struct CreateMergeRequest<'a> {
    source_branch: &'a str,
    target_branch: &'a str,
    title: String,
    description: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_project_id: Option<u64>,
}

impl HostProvider for GitLab {
    fn endpoint(&self) -> &HostEndpoint {
        &self.http.endpoint
    }

    fn current_user(&self) -> Result<HostAccount> {
        let (user, _): (ApiUser, _) = self.http.get("user")?;
        let mut emails: Vec<String> = Vec::new();
        for email in [&user.commit_email, &user.email, &user.public_email]
            .into_iter()
            .flatten()
        {
            if !email.is_empty() && !emails.contains(email) {
                emails.push(email.clone());
            }
        }
        // needs `read_user`; older servers list only the secondary ones
        if let Ok((more, _)) = self.http.get::<Vec<ApiEmail>>("user/emails") {
            for email in more {
                if email.confirmed_at.is_some() && !emails.contains(&email.email) {
                    emails.push(email.email);
                }
            }
        }
        Ok(HostAccount {
            endpoint: self.http.endpoint.clone(),
            id: user.id.to_string(),
            login: user.username,
            name: user.name,
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
        let path = format!("projects/{}", encode(&format!("{owner}/{name}")));
        let (project, _): (ApiProject, _) = self.http.get(&path)?;
        Ok(self.convert(project))
    }

    fn user_repositories(&self) -> Result<Vec<GitHubRepository>> {
        let projects: Vec<ApiProject> = self.http.get_all(
            "projects?membership=true&archived=false&order_by=last_activity_at&per_page=100",
            MAX_PAGES,
        )?;
        Ok(projects.into_iter().map(|p| self.convert(p)).collect())
    }

    fn open_pull_requests(&self, repo: &GitHubRepository) -> Result<Vec<PullRequest>> {
        let path = format!(
            "{}/merge_requests?state=opened&order_by=created_at&sort=desc&per_page=100",
            Self::project_path(repo)
        );
        let requests: Vec<ApiMergeRequest> = self.http.get_all(&path, MAX_PAGES)?;
        // the target project's id, and the fork projects requests come from
        let target_id = requests
            .first()
            .map(|mr| mr.target_project_id)
            .unwrap_or_default();
        let mut sources: HashMap<u64, Option<GitHubRepository>> = HashMap::new();
        sources.insert(target_id, Some(repo.clone()));
        // the fork projects requests come from, read side by side (one
        // request each would take seconds in a row)
        let mut forks: Vec<u64> = requests
            .iter()
            .map(|mr| mr.source_project_id)
            .filter(|id| *id != target_id)
            .collect();
        forks.sort_unstable();
        forks.dedup();
        forks.truncate(MAX_FORK_LOOKUPS);
        let found: Vec<(u64, Option<GitHubRepository>)> = std::thread::scope(|scope| {
            let handles: Vec<_> = forks
                .iter()
                .map(|&id| {
                    scope.spawn(move || {
                        let project = self
                            .http
                            .get_optional::<ApiProject>(&format!("projects/{id}"))
                            .ok()
                            .flatten()
                            .map(|p| self.convert(p));
                        (id, project)
                    })
                })
                .collect();
            handles.into_iter().filter_map(|h| h.join().ok()).collect()
        });
        sources.extend(found);
        let mut prs: Vec<PullRequest> = requests
            .into_iter()
            .map(|mr| {
                let source = sources.get(&mr.source_project_id).cloned().flatten();
                convert_merge_request(mr, repo, source)
            })
            .collect();
        prs.sort_by_key(|p| std::cmp::Reverse(p.number));
        Ok(prs)
    }

    fn create_pull_request(
        &self,
        repo: &GitHubRepository,
        new: &NewPullRequest,
    ) -> Result<PullRequest> {
        let title = if new.draft && !new.title.starts_with("Draft:") {
            format!("Draft: {}", new.title)
        } else {
            new.title.clone()
        };
        // from a fork: the request is opened on the fork, aimed at the target
        let (post_on, target_project_id, source) = match &new.head_repository {
            Some(fork) => {
                let (target, _): (ApiProject, _) = self.http.get(&Self::project_path(repo))?;
                (
                    Self::project_path(fork),
                    Some(target.id),
                    Some(fork.clone()),
                )
            }
            None => (Self::project_path(repo), None, Some(repo.clone())),
        };
        let body = CreateMergeRequest {
            source_branch: &new.head,
            target_branch: &new.base,
            title,
            description: &new.body,
            target_project_id,
        };
        let mr: ApiMergeRequest = self
            .http
            .post(&format!("{post_on}/merge_requests"), &body)?;
        Ok(convert_merge_request(mr, repo, source))
    }

    fn ref_checks(
        &self,
        repo: &GitHubRepository,
        target: &CheckTarget,
    ) -> Result<Option<Vec<RefCheck>>> {
        let anonymous = self.http.auth == Auth::Anonymous;
        // a project can keep its pipelines to members: an anonymous 401
        // or 403 means none to show, an authenticated one a bad token
        let hidden_ok = |result: Result<Vec<ApiPipeline>>| match result {
            Err(err) if anonymous && err.is_hidden() => Ok(Vec::new()),
            other => other,
        };
        let project = Self::project_path(repo);
        let latest = |path: String| -> Result<Option<ApiPipeline>> {
            Ok(hidden_ok(self.http.get(&path).map(|(p, _)| p))?
                .into_iter()
                .next())
        };
        // a merge request's pipelines run on its merged result, a commit of
        // their own: ask the merge request
        let mut pipeline = match target.pull_request {
            Some(number) => latest(format!(
                "{project}/merge_requests/{number}/pipelines?per_page=1"
            ))?,
            None => None,
        };
        if pipeline.is_none() {
            pipeline = latest(format!(
                "{project}/pipelines?sha={}&order_by=id&sort=desc&per_page=1",
                encode(&target.sha)
            ))?;
        }
        if pipeline.is_none()
            && let Some(branch) = &target.branch
        {
            pipeline = latest(format!(
                "{project}/pipelines?ref={}&sha={}&order_by=id&sort=desc&per_page=1",
                encode(branch),
                encode(&target.sha)
            ))?;
        }
        if let Some(pipeline) = pipeline {
            let jobs: Vec<ApiJob> = self.http.get_all(
                &format!("{project}/pipelines/{}/jobs?per_page=100", pipeline.id),
                MAX_PAGES,
            )?;
            if !jobs.is_empty() {
                return Ok(Some(
                    jobs.into_iter()
                        .map(|job| convert_job(job, &pipeline))
                        .collect(),
                ));
            }
        }
        // no pipeline: statuses reported by other CI services
        let statuses: Vec<ApiCommitStatus> = match self.http.get(&format!(
            "{project}/repository/commits/{}/statuses?per_page=100",
            encode(&target.sha)
        )) {
            Ok((statuses, _)) => statuses,
            Err(err) if anonymous && err.is_hidden() => Vec::new(),
            Err(err) => return Err(err),
        };
        if statuses.is_empty() {
            return Ok(None);
        }
        // the API lists every run of a name, newest first
        let mut seen = std::collections::HashSet::new();
        Ok(Some(
            statuses
                .into_iter()
                .filter(|s| seen.insert(s.name.clone()))
                .map(convert_status)
                .collect(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use corvene_models::HostKind;

    fn endpoint() -> HostEndpoint {
        HostEndpoint::public(HostKind::GitLab)
    }

    #[test]
    fn converts_projects() {
        let project: ApiProject = serde_json::from_str(
            r#"{"id": 7, "path_with_namespace": "group/sub/proj",
                "web_url": "https://gitlab.com/group/sub/proj",
                "http_url_to_repo": "https://gitlab.com/group/sub/proj.git",
                "default_branch": "main", "visibility": "internal", "archived": false,
                "forked_from_project": {"id": 3, "path_with_namespace": "up/proj",
                    "web_url": "https://gitlab.com/up/proj",
                    "http_url_to_repo": "https://gitlab.com/up/proj.git"},
                "permissions": {"project_access": {"access_level": 30},
                                "group_access": {"access_level": 40}}}"#,
        )
        .unwrap();
        let repo = convert_project(&endpoint(), project);
        assert_eq!(repo.owner, "group/sub");
        assert_eq!(repo.name, "proj");
        assert_eq!(repo.endpoint, "https://gitlab.com/api/v4");
        assert!(repo.private);
        assert!(repo.fork);
        assert_eq!(repo.permissions, Some(RepositoryPermission::Admin));
        let parent = repo.parent.unwrap();
        assert_eq!(parent.full_name(), "up/proj");
        assert_eq!(parent.clone_url, "https://gitlab.com/up/proj.git");
        let bare: ApiProject =
            serde_json::from_str(r#"{"id": 1, "path_with_namespace": "a/b", "permissions": null}"#)
                .unwrap();
        let bare = convert_project(&endpoint(), bare);
        assert_eq!(bare.html_url, "https://gitlab.com/a/b");
        assert_eq!(bare.permissions, None);
        assert!(!bare.private);
    }

    #[test]
    fn converts_merge_requests() {
        let mr: ApiMergeRequest = serde_json::from_str(
            r#"{"iid": 12, "title": "Fix it", "description": null,
                "created_at": "2026-01-02T03:04:05.000Z", "updated_at": "2026-01-03T03:04:05.000Z",
                "author": {"username": "mona"}, "source_branch": "fix", "target_branch": "main",
                "source_project_id": 9, "target_project_id": 7, "work_in_progress": true,
                "sha": "abc", "reviewers": [{"username": "hubot"}]}"#,
        )
        .unwrap();
        let target = GitHubRepository {
            owner: "group".into(),
            name: "proj".into(),
            ..blank_repository(&endpoint())
        };
        let pr = convert_merge_request(mr, &target, None);
        assert_eq!(pr.number, 12);
        assert!(pr.draft);
        assert_eq!(pr.head.ref_name, "fix");
        assert_eq!(pr.head.sha, "abc");
        assert!(pr.head.repository.is_none());
        assert_eq!(pr.base.repository.as_ref().unwrap().name, "proj");
        assert_eq!(pr.requested_reviewers, ["hubot"]);
    }

    #[test]
    fn maps_job_states() {
        assert_eq!(
            job_state("failed", true),
            (CheckStatus::Completed, Some(CheckConclusion::Neutral))
        );
        assert_eq!(
            job_state("failed", false),
            (CheckStatus::Completed, Some(CheckConclusion::Failure))
        );
        assert_eq!(job_state("running", false), (CheckStatus::InProgress, None));
        assert_eq!(job_state("pending", false), (CheckStatus::Queued, None));
        let pipeline = ApiPipeline {
            id: 5,
            created_at: None,
        };
        let job: ApiJob = serde_json::from_str(
            r#"{"id": 1, "name": "rspec", "stage": "test", "status": "success",
                "web_url": "https://gitlab.com/a/b/-/jobs/1",
                "started_at": "2026-01-01T00:00:00Z", "finished_at": "2026-01-01T00:01:30Z"}"#,
        )
        .unwrap();
        let check = convert_job(job, &pipeline);
        assert_eq!(check.actions_workflow.as_ref().unwrap().name, "test");
        assert_eq!(check.description, "Successful in 1m 30s");
    }
}
