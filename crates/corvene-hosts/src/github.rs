//! GitHub (GitHub.com and Enterprise) behind [`HostProvider`], over
//! `corvene_github::Client`. The app's GitHub stores keep calling the
//! client directly (incremental pull request refresh, check suites with
//! Actions steps and re-runs, rulesets…); this adapter exists so the trait
//! is held to the richest host and generic code can take any host.

use corvene_github::{ApiPullRequest, Client, Endpoint};
use corvene_models::{
    CheckConclusion, CheckStatus, GitHubRepository, HostAccount, HostAuthKind, HostEndpoint,
    PullRequest, PullRequestRef, RefCheck, check_duration_ms, check_short_description,
};
use serde::Serialize;

use crate::http::{Auth, Http, encode};
use crate::{CheckTarget, HostProvider, NewPullRequest, Result};

pub struct GitHub {
    endpoint: HostEndpoint,
    client: Client,
    http: Http,
}

impl GitHub {
    pub fn new(endpoint: HostEndpoint, token: &str) -> Self {
        let api = Endpoint::from_api_base(&endpoint.api_base);
        let auth = if token.is_empty() {
            Auth::Anonymous
        } else {
            Auth::Bearer(token.to_string())
        };
        Self {
            client: Client::new(api, token),
            http: Http::new(endpoint.clone(), auth),
            endpoint,
        }
    }

    fn convert_pull_request(&self, pr: ApiPullRequest) -> PullRequest {
        let convert_ref = |r: corvene_github::api::ApiPullRequestRef| PullRequestRef {
            ref_name: r.ref_name,
            sha: r.sha,
            repository: r.repo.map(|repo| self.client.convert(repo)),
        };
        PullRequest {
            number: pr.number,
            title: pr.title,
            created_at: pr.created_at,
            updated_at: pr.updated_at,
            head: convert_ref(pr.head),
            base: convert_ref(pr.base),
            author: pr.user.login,
            draft: pr.draft,
            body: pr.body.unwrap_or_default(),
            assignees: pr.assignees.into_iter().map(|u| u.login).collect(),
            requested_reviewers: pr
                .requested_reviewers
                .into_iter()
                .map(|u| u.login)
                .collect(),
        }
    }
}

#[derive(Serialize)]
struct CreatePullRequest<'a> {
    title: &'a str,
    body: &'a str,
    head: String,
    base: &'a str,
    draft: bool,
}

impl HostProvider for GitHub {
    fn endpoint(&self) -> &HostEndpoint {
        &self.endpoint
    }

    fn current_user(&self) -> Result<HostAccount> {
        let account = self.client.current_user(Vec::new())?;
        Ok(HostAccount {
            endpoint: self.endpoint.clone(),
            id: account.id.to_string(),
            login: account.login,
            name: account.name,
            avatar_url: account.avatar_url,
            emails: account.emails,
            auth: HostAuthKind::Token,
            basic_user: None,
            oauth_client_id: None,
            expires_at: None,
            needs_reauth: false,
        })
    }

    fn repository(&self, owner: &str, name: &str) -> Result<GitHubRepository> {
        Ok(self.client.repository(owner, name)?)
    }

    fn user_repositories(&self) -> Result<Vec<GitHubRepository>> {
        Ok(self.client.user_repositories()?)
    }

    fn open_pull_requests(&self, repo: &GitHubRepository) -> Result<Vec<PullRequest>> {
        let prs = self.client.open_pull_requests(&repo.owner, &repo.name)?;
        let mut prs: Vec<PullRequest> = prs
            .into_iter()
            .map(|pr| self.convert_pull_request(pr))
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
        let body = CreatePullRequest {
            title: &new.title,
            body: &new.body,
            head,
            base: &new.base,
            draft: new.draft,
        };
        let pr: ApiPullRequest = self.http.post(
            &format!("repos/{}/{}/pulls", encode(&repo.owner), encode(&repo.name)),
            &body,
        )?;
        Ok(self.convert_pull_request(pr))
    }

    fn ref_checks(
        &self,
        repo: &GitHubRepository,
        target: &CheckTarget,
    ) -> Result<Option<Vec<RefCheck>>> {
        let sha = target.sha.as_str();
        let statuses = self
            .client
            .combined_ref_status(&repo.owner, &repo.name, sha)?;
        let runs = self
            .client
            .ref_check_runs(&repo.owner, &repo.name, sha, false)?;
        if statuses.is_none() && runs.is_none() {
            return Ok(None);
        }
        let mut checks = Vec::new();
        for item in statuses.map(|s| s.statuses).unwrap_or_default() {
            let (status, conclusion) = match item.state.as_str() {
                "success" => (CheckStatus::Completed, Some(CheckConclusion::Success)),
                "pending" => (CheckStatus::InProgress, None),
                _ => (CheckStatus::Completed, Some(CheckConclusion::Failure)),
            };
            checks.push(RefCheck {
                id: item.id,
                name: item.context,
                description: check_short_description(status, conclusion, None),
                status,
                conclusion,
                app_name: String::new(),
                html_url: item.target_url,
                check_suite_id: None,
                actions_workflow: None,
                job_steps: None,
            });
        }
        for run in runs.map(|r| r.check_runs).unwrap_or_default() {
            let duration =
                check_duration_ms(run.started_at.as_deref(), run.completed_at.as_deref());
            checks.push(RefCheck {
                id: run.id,
                name: run.name,
                description: check_short_description(run.status, run.conclusion, duration),
                status: run.status,
                conclusion: run.conclusion,
                app_name: run.app.map(|a| a.name).unwrap_or_default(),
                html_url: run.html_url,
                check_suite_id: run.check_suite.map(|s| s.id),
                actions_workflow: None,
                job_steps: None,
            });
        }
        Ok(Some(checks))
    }
}
