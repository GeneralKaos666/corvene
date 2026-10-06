//! Corvene `351-actions`: the GitHub Actions REST calls of the Actions view
//! (workflows, their runs, a run's jobs, re-run / cancel / dispatch and the
//! workflow file). GHD (`lib/api.ts`) only reads the runs and jobs behind a
//! pull request's checks; see [`crate::api`] for those.

use corvene_models::CheckStatus;
use serde::Deserialize;
use tracing::debug;

use crate::api::{ApiWorkflowJob, ApiWorkflowJobs, ApiWorkflowRun, ApiWorkflowRuns, Client};
use crate::api::{encode_path_component, get_next_page_path_from_link, header_pairs};
use crate::error::{GitHubError, Result};

/// The most jobs read for one run (pages of 100).
const MAX_JOB_PAGES: u32 = 5;

/// `GET repos/{owner}/{name}/actions/workflows`: one workflow file.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ApiWorkflow {
    pub id: u64,
    pub name: String,
    /// `.github/workflows/ci.yml`; dynamic workflows (Dependabot, Pages)
    /// have a `dynamic/…` path and no file.
    #[serde(default)]
    pub path: String,
    /// `active`, `disabled_manually`, `disabled_inactivity`, `deleted`…
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub html_url: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ApiWorkflows {
    #[serde(default)]
    total_count: u64,
    #[serde(default)]
    workflows: Vec<ApiWorkflow>,
}

/// Which runs `GET …/actions/runs` lists: one workflow's or all, filtered by
/// branch, status (a status or a conclusion) and event, a page of them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunsQuery {
    pub workflow_id: Option<u64>,
    pub branch: Option<String>,
    pub status: Option<String>,
    pub event: Option<String>,
    pub per_page: u32,
    /// From 1.
    pub page: u32,
}

impl RunsQuery {
    /// The request path for `owner/name`.
    pub fn path(&self, owner: &str, name: &str) -> String {
        let base = format!(
            "repos/{}/{}/actions",
            encode_path_component(owner),
            encode_path_component(name)
        );
        let mut path = match self.workflow_id {
            Some(id) => format!("{base}/workflows/{id}/runs"),
            None => format!("{base}/runs"),
        };
        path.push_str(&format!(
            "?per_page={}&page={}",
            self.per_page.clamp(1, 100),
            self.page.max(1)
        ));
        for (key, value) in [
            ("branch", &self.branch),
            ("status", &self.status),
            ("event", &self.event),
        ] {
            if let Some(value) = value.as_deref().filter(|v| !v.is_empty()) {
                path.push_str(&format!("&{key}={}", encode_path_component(value)));
            }
        }
        path
    }
}

/// A conditional read: unchanged since the `ETag` sent, or the new value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Conditional<T> {
    NotModified,
    Fresh { value: T, etag: Option<String> },
}

impl<T> Conditional<T> {
    fn of(answer: Option<(T, Option<String>)>) -> Self {
        match answer {
            None => Self::NotModified,
            Some((value, etag)) => Self::Fresh { value, etag },
        }
    }
}

/// `GET repos/{owner}/{name}/environments`
#[derive(Debug, Clone, Deserialize)]
struct ApiEnvironments {
    #[serde(default)]
    environments: Vec<ApiNamed>,
}

#[derive(Debug, Clone, Deserialize)]
struct ApiNamed {
    name: String,
}

impl Client {
    fn repo_path(owner: &str, name: &str) -> String {
        format!(
            "repos/{}/{}",
            encode_path_component(owner),
            encode_path_component(name)
        )
    }

    /// The repository's workflows, by name (pages of 100, at most 500).
    pub fn workflows(&self, owner: &str, name: &str) -> Result<Vec<ApiWorkflow>> {
        let base = format!("{}/actions/workflows", Self::repo_path(owner, name));
        let mut out: Vec<ApiWorkflow> = Vec::new();
        for page in 1..=5u32 {
            let batch: ApiWorkflows = self.get_json(&format!("{base}?per_page=100&page={page}"))?;
            let short = batch.workflows.len() < 100;
            out.extend(batch.workflows);
            if short || out.len() as u64 >= batch.total_count {
                break;
            }
        }
        out.sort_by_key(|w| w.name.to_lowercase());
        Ok(out)
    }

    /// One page of runs (`query`), newest first; `etag` makes it
    /// conditional.
    pub fn workflow_runs_page(
        &self,
        owner: &str,
        name: &str,
        query: &RunsQuery,
        etag: Option<&str>,
    ) -> Result<Conditional<ApiWorkflowRuns>> {
        let answer = self.get_json_if_none_match(&query.path(owner, name), etag)?;
        Ok(Conditional::of(answer))
    }

    /// `GET …/actions/runs/{run_id}`
    pub fn workflow_run(&self, owner: &str, name: &str, run_id: u64) -> Result<ApiWorkflowRun> {
        self.get_json(&format!(
            "{}/actions/runs/{run_id}",
            Self::repo_path(owner, name)
        ))
    }

    /// Every job of the run's latest attempt (pages of 100); `etag` makes
    /// the first page conditional, and an unchanged first page means an
    /// unchanged run.
    pub fn workflow_run_jobs_all(
        &self,
        owner: &str,
        name: &str,
        run_id: u64,
        etag: Option<&str>,
    ) -> Result<Conditional<Vec<ApiWorkflowJob>>> {
        let base = format!(
            "{}/actions/runs/{run_id}/jobs?filter=latest&per_page=100",
            Self::repo_path(owner, name)
        );
        let Some((first, etag)) = self.get_json_if_none_match::<ApiWorkflowJobs>(&base, etag)?
        else {
            return Ok(Conditional::NotModified);
        };
        let total = first.total_count;
        let mut jobs = first.jobs;
        let mut page = 1;
        while (jobs.len() as u64) < total && page < MAX_JOB_PAGES {
            page += 1;
            let batch: ApiWorkflowJobs = self.get_json(&format!("{base}&page={page}"))?;
            if batch.jobs.is_empty() {
                break;
            }
            jobs.extend(batch.jobs);
        }
        Ok(Conditional::Fresh { value: jobs, etag })
    }

    /// `POST …/actions/runs/{id}/rerun` (every job), or
    /// `…/rerun-failed-jobs` with `failed_only`.
    pub fn rerun_workflow_run(
        &self,
        owner: &str,
        name: &str,
        run_id: u64,
        failed_only: bool,
    ) -> Result<()> {
        let action = if failed_only {
            "rerun-failed-jobs"
        } else {
            "rerun"
        };
        self.send_no_content(
            false,
            &format!(
                "{}/actions/runs/{run_id}/{action}",
                Self::repo_path(owner, name)
            ),
            None,
        )
    }

    /// `POST …/actions/jobs/{id}/rerun`: one job (and the jobs that need it).
    pub fn rerun_workflow_job(&self, owner: &str, name: &str, job_id: u64) -> Result<()> {
        self.send_no_content(
            false,
            &format!(
                "{}/actions/jobs/{job_id}/rerun",
                Self::repo_path(owner, name)
            ),
            None,
        )
    }

    /// `POST …/actions/runs/{id}/cancel` (202).
    pub fn cancel_workflow_run(&self, owner: &str, name: &str, run_id: u64) -> Result<()> {
        self.send_no_content(
            false,
            &format!(
                "{}/actions/runs/{run_id}/cancel",
                Self::repo_path(owner, name)
            ),
            None,
        )
    }

    /// `DELETE …/actions/runs/{id}/logs` (204).
    pub fn delete_workflow_run_logs(&self, owner: &str, name: &str, run_id: u64) -> Result<()> {
        self.send_no_content(
            true,
            &format!(
                "{}/actions/runs/{run_id}/logs",
                Self::repo_path(owner, name)
            ),
            None,
        )
    }

    /// `POST …/actions/workflows/{id}/dispatches` (204): run the workflow
    /// on `git_ref` (a branch or tag name) with `inputs` (GitHub takes every
    /// value as a string).
    pub fn dispatch_workflow(
        &self,
        owner: &str,
        name: &str,
        workflow_id: u64,
        git_ref: &str,
        inputs: &[(String, String)],
    ) -> Result<()> {
        let inputs: serde_json::Map<String, serde_json::Value> = inputs
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect();
        self.send_no_content(
            false,
            &format!(
                "{}/actions/workflows/{workflow_id}/dispatches",
                Self::repo_path(owner, name)
            ),
            Some(&serde_json::json!({ "ref": git_ref, "inputs": inputs })),
        )
    }

    /// `GET …/contents/{path}?ref=` as raw text (`application/vnd.github.raw`):
    /// a workflow file at a branch or tag. `None` when it is not there.
    pub fn file_text(
        &self,
        owner: &str,
        name: &str,
        path: &str,
        git_ref: Option<&str>,
    ) -> Result<Option<String>> {
        let encoded: Vec<String> = path.split('/').map(encode_path_component).collect();
        let mut url = format!(
            "{}/contents/{}",
            Self::repo_path(owner, name),
            encoded.join("/")
        );
        if let Some(r) = git_ref.filter(|r| !r.is_empty()) {
            url.push_str(&format!("?ref={}", encode_path_component(r)));
        }
        match self.get_response(&url, "application/vnd.github.raw") {
            Ok(mut response) => Ok(Some(
                response
                    .body_mut()
                    .with_config()
                    .limit(4 * 1024 * 1024)
                    .read_to_string()?,
            )),
            Err(GitHubError::Api { status: 404, .. }) => {
                debug!(%path, "no file at the ref");
                Ok(None)
            }
            Err(err) => Err(err),
        }
    }

    /// The names of a `GET` list of `{ name }` objects, following `Link`
    /// pages (at most `max_pages` of 100).
    fn names(&self, path: &str, max_pages: usize) -> Result<Vec<String>> {
        let mut out = Vec::new();
        let mut next = Some(path.to_string());
        let mut pages = 0;
        while let Some(path) = next.take() {
            let mut response = self.get_response(&path, "application/vnd.github+json")?;
            let headers = header_pairs(response.headers());
            let page: Vec<ApiNamed> = response.body_mut().read_json()?;
            out.extend(page.into_iter().map(|n| n.name));
            pages += 1;
            next = get_next_page_path_from_link(&headers).filter(|_| pages < max_pages);
        }
        Ok(out)
    }

    /// The repository's branch names (what Run workflow can run on).
    pub fn branch_names(&self, owner: &str, name: &str, max_pages: usize) -> Result<Vec<String>> {
        self.names(
            &format!("{}/branches?per_page=100", Self::repo_path(owner, name)),
            max_pages,
        )
    }

    /// The repository's tag names, newest first.
    pub fn tag_names(&self, owner: &str, name: &str, max_pages: usize) -> Result<Vec<String>> {
        self.names(
            &format!("{}/tags?per_page=100", Self::repo_path(owner, name)),
            max_pages,
        )
    }

    /// The deployment environments (an `environment` input's choices).
    /// `None` when the account may not read them.
    pub fn environment_names(&self, owner: &str, name: &str) -> Result<Option<Vec<String>>> {
        match self.get_json::<ApiEnvironments>(&format!(
            "{}/environments?per_page=100",
            Self::repo_path(owner, name)
        )) {
            Ok(list) => Ok(Some(
                list.environments.into_iter().map(|e| e.name).collect(),
            )),
            Err(err) if err.is_token_invalidated() => Err(err),
            Err(GitHubError::Api { status, .. }) => {
                debug!(status, "environments not readable");
                Ok(None)
            }
            Err(err) => Err(err),
        }
    }
}

/// Whether a run's or job's status means it has not finished.
pub fn is_active(status: Option<CheckStatus>) -> bool {
    !matches!(status, Some(CheckStatus::Completed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_query_paths() {
        let query = RunsQuery {
            per_page: 25,
            page: 2,
            ..Default::default()
        };
        assert_eq!(
            query.path("octo", "cat"),
            "repos/octo/cat/actions/runs?per_page=25&page=2"
        );
        let query = RunsQuery {
            workflow_id: Some(7),
            branch: Some("feature/x y".into()),
            status: Some("in_progress".into()),
            event: Some("push".into()),
            per_page: 500,
            page: 0,
        };
        assert_eq!(
            query.path("octo", "cat"),
            "repos/octo/cat/actions/workflows/7/runs?per_page=100&page=1&branch=feature%2Fx%20y&status=in_progress&event=push"
        );
    }

    #[test]
    fn parses_runs_and_workflows() {
        let runs: ApiWorkflowRuns = serde_json::from_str(
            r#"{"total_count": 2, "workflow_runs": [
                {"id": 30, "workflow_id": 3, "name": "CI", "created_at": "2024-05-01T10:00:00Z",
                 "event": "push", "status": "in_progress", "conclusion": null,
                 "head_branch": "main", "head_sha": "abc", "run_number": 12, "run_attempt": 2,
                 "actor": {"login": "octocat"}, "html_url": "https://github.com/o/r/actions/runs/30",
                 "display_title": "Fix the build", "path": ".github/workflows/ci.yml",
                 "run_started_at": "2024-05-01T10:00:05Z", "updated_at": "2024-05-01T10:02:00Z"},
                {"id": 29, "workflow_id": 3, "created_at": "2024-04-30T10:00:00Z",
                 "status": "completed", "conclusion": "startup_failure"}
            ]}"#,
        )
        .unwrap();
        assert_eq!(runs.total_count, 2);
        let run = &runs.workflow_runs[0];
        assert_eq!(run.status, Some(CheckStatus::InProgress));
        assert_eq!(run.conclusion, None);
        assert_eq!(
            run.actor.as_ref().map(|a| a.login.as_str()),
            Some("octocat")
        );
        assert_eq!(run.display_title.as_deref(), Some("Fix the build"));
        assert_eq!(run.run_attempt, 2);
        assert!(is_active(run.status));
        let old = &runs.workflow_runs[1];
        assert_eq!(
            old.conclusion,
            Some(corvene_models::CheckConclusion::Unknown)
        );
        assert!(!is_active(old.status));

        let workflows: ApiWorkflows = serde_json::from_str(
            r#"{"total_count": 1, "workflows": [{"id": 3, "name": "CI",
                "path": ".github/workflows/ci.yml", "state": "active",
                "html_url": "https://github.com/o/r/blob/main/.github/workflows/ci.yml"}]}"#,
        )
        .unwrap();
        assert_eq!(workflows.workflows[0].state, "active");
        let waiting: ApiWorkflowRun = serde_json::from_str(
            r#"{"id": 1, "workflow_id": 1, "created_at": "x", "status": "waiting"}"#,
        )
        .unwrap();
        assert_eq!(waiting.status, Some(CheckStatus::Pending));
    }
}
