//! Corvene `351-actions` (desktop/desktop#17498): a GitHub repository's
//! Actions in the app, cloned or not. GitHub Desktop has none of it: it
//! reads workflow runs only to re-run a pull request's failed checks
//! (`lib/stores/commit-status-store.ts`, `ui/check-runs/`), and everything
//! else happens on github.com.
//!
//! Repository › Actions… (a cloned repository) and View Actions… on a
//! repository of the clone dialog's lists open `Popup::Actions`: the
//! workflows on the left (All workflows first), the runs of the selection
//! in the middle (filtered by branch, status and event, 25 a page with
//! Load more), and the selected run's jobs and steps on the right, whose
//! logs open in the job log dialog (`347-actions-job-logs`). A run can be
//! re-run (all jobs or the failed ones), cancelled or have its logs
//! deleted; a job re-run on its own. Run workflow (`Popup::RunWorkflow`)
//! dispatches a workflow with `workflow_dispatch` on a branch or tag, with
//! a form built from the workflow file's `on.workflow_dispatch.inputs`
//! ([`parse_workflow_dispatch`]).
//!
//! While the view is open and something is queued or running it reads the
//! runs and the selected run's jobs again every [`POLL_INTERVAL`], with
//! `If-None-Match` so an unchanged answer costs no rate limit; closing the
//! view stops it. A run started or re-run from here is watched after the
//! view closes (every [`WATCH_INTERVAL`], at most [`WATCH_LIFETIME`]) and
//! posts a notification when it finishes, when Settings › Notifications
//! allows them; a click opens the run.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime};

use corvene_github::actions::{ApiWorkflow, Conditional, RunsQuery};
use corvene_github::api::{ApiWorkflowJob, ApiWorkflowRun};
use corvene_github::{Client, GitHubError};
use corvene_models::{
    CheckConclusion, CheckStatus, GitHubRepository, JobStep, RefCheck, RepositoryPermission,
    WorkflowRun,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};
use yaml_rust::{Yaml, YamlLoader};

use crate::dispatcher::Dispatcher;
use crate::host::{AsyncCtx, Host};
use crate::remote::spawn_bg;
use crate::state::{AppState, Popup};

/// The last view's generation (a poll of a closed view must not see a
/// reopened one as its own).
static GENERATION: AtomicU64 = AtomicU64::new(0);

fn next_generation() -> u64 {
    GENERATION.fetch_add(1, Ordering::Relaxed) + 1
}

/// Runs per page.
pub const RUNS_PER_PAGE: u32 = 25;
/// How often the open view reads again while a run is queued or running.
pub const POLL_INTERVAL: Duration = Duration::from_secs(12);
/// How often a watched run is read after the view closed.
pub const WATCH_INTERVAL: Duration = Duration::from_secs(30);
/// A watched run that has not finished after this is let go.
pub const WATCH_LIFETIME: Duration = Duration::from_secs(6 * 3600);
/// A dispatched run that has not shown up after this is not looked for.
const DISPATCH_LOOKUP: Duration = Duration::from_secs(120);
/// Pages of 100 branches / tags read for Run workflow's ref list.
const REF_PAGES: usize = 3;

/// The Status filter's choices: label and the API's `status` value.
pub const STATUS_FILTERS: &[(&str, &str)] = &[
    ("Queued", "queued"),
    ("In progress", "in_progress"),
    ("Waiting", "waiting"),
    ("Completed", "completed"),
    ("Success", "success"),
    ("Failure", "failure"),
    ("Cancelled", "cancelled"),
    ("Skipped", "skipped"),
    ("Timed out", "timed_out"),
    ("Action required", "action_required"),
];

/// The Event filter's usual choices (events seen in the runs are added).
pub const EVENT_FILTERS: &[&str] = &[
    "push",
    "pull_request",
    "pull_request_target",
    "workflow_dispatch",
    "schedule",
    "release",
    "merge_group",
    "workflow_run",
    "repository_dispatch",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowRow {
    pub id: u64,
    pub name: String,
    pub path: String,
    pub state: String,
    pub html_url: String,
}

impl WorkflowRow {
    fn from_api(w: ApiWorkflow) -> Self {
        Self {
            id: w.id,
            name: w.name,
            path: w.path,
            state: w.state,
            html_url: w.html_url,
        }
    }

    /// Disabled workflows are listed dimmed and cannot be dispatched.
    pub fn is_active(&self) -> bool {
        self.state.is_empty() || self.state == "active"
    }

    /// A workflow file in the repository (not a dynamic one such as
    /// Dependabot's or Pages').
    pub fn has_file(&self) -> bool {
        self.path.starts_with(".github/")
    }

    /// `ci.yml`
    pub fn file_name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunRow {
    pub id: u64,
    pub workflow_id: u64,
    /// The workflow's name.
    pub workflow_name: String,
    /// What GitHub lists the run as (the commit's or pull request's title).
    pub title: String,
    pub run_number: u64,
    pub run_attempt: u64,
    pub status: CheckStatus,
    pub conclusion: Option<CheckConclusion>,
    pub branch: Option<String>,
    pub head_sha: String,
    pub event: String,
    pub actor: String,
    pub html_url: String,
    pub created_at: String,
    pub updated_at: Option<String>,
    pub run_started_at: Option<String>,
}

impl RunRow {
    pub fn from_api(run: ApiWorkflowRun) -> Self {
        let actor = run
            .triggering_actor
            .or(run.actor)
            .map(|a| a.login)
            .unwrap_or_default();
        let title = run
            .display_title
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| run.name.clone());
        Self {
            id: run.id,
            workflow_id: run.workflow_id,
            workflow_name: run.name,
            title,
            run_number: run.run_number,
            run_attempt: run.run_attempt.max(1),
            status: run.status.unwrap_or(CheckStatus::Pending),
            conclusion: run.conclusion,
            branch: run.head_branch,
            head_sha: run.head_sha,
            event: run.event,
            actor,
            html_url: run.html_url,
            created_at: run.created_at,
            updated_at: run.updated_at,
            run_started_at: run.run_started_at,
        }
    }

    /// Queued, waiting or running.
    pub fn is_active(&self) -> bool {
        self.status != CheckStatus::Completed
    }

    /// Some job failed or was cancelled: Re-run failed jobs applies.
    pub fn has_failed_jobs(&self) -> bool {
        !self.is_active()
            && matches!(
                self.conclusion,
                Some(
                    CheckConclusion::Failure
                        | CheckConclusion::Cancelled
                        | CheckConclusion::TimedOut
                        | CheckConclusion::Unknown
                )
            )
    }

    /// How long the run took (or has been running, to `now`).
    pub fn duration_ms(&self, now: SystemTime) -> Option<i64> {
        let start = self
            .run_started_at
            .as_deref()
            .or(Some(self.created_at.as_str()))
            .and_then(corvene_models::parse_iso8601)?;
        let end = if self.is_active() {
            now
        } else {
            self.updated_at
                .as_deref()
                .and_then(corvene_models::parse_iso8601)?
        };
        let ms = end.duration_since(start).ok()?.as_millis();
        i64::try_from(ms).ok()
    }

    /// The run's status in words, as GitHub's run page heads it.
    pub fn status_text(&self) -> &'static str {
        match self.status {
            CheckStatus::Queued => "Queued",
            CheckStatus::InProgress => "In progress",
            CheckStatus::Pending => "Waiting",
            CheckStatus::Completed => match self.conclusion {
                Some(CheckConclusion::Success) => "Success",
                Some(CheckConclusion::Failure) => "Failure",
                Some(CheckConclusion::Cancelled) => "Cancelled",
                Some(CheckConclusion::Skipped) => "Skipped",
                Some(CheckConclusion::TimedOut) => "Timed out",
                Some(CheckConclusion::ActionRequired) => "Action required",
                Some(CheckConclusion::Neutral) => "Neutral",
                Some(CheckConclusion::Stale) => "Stale",
                Some(CheckConclusion::Unknown) => "Startup failure",
                None => "Completed",
            },
        }
    }
}

/// A job of the selected run: the check the job log dialog takes (its id
/// is the job id, its steps the job's) and its times.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobRow {
    pub check: RefCheck,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

impl JobRow {
    fn from_api(job: ApiWorkflowJob, run: Option<&RunRow>) -> Self {
        let steps = job
            .steps
            .into_iter()
            .map(|s| JobStep {
                name: s.name,
                number: s.number,
                status: s.status,
                conclusion: s.conclusion,
                started_at: s.started_at,
                completed_at: s.completed_at,
            })
            .collect();
        Self {
            check: RefCheck {
                id: job.id,
                name: job.name,
                description: String::new(),
                status: job.status,
                conclusion: job.conclusion,
                app_name: "GitHub Actions".into(),
                html_url: job.html_url,
                check_suite_id: None,
                actions_workflow: run.map(|r| WorkflowRun {
                    id: r.id,
                    workflow_id: r.workflow_id,
                    name: r.workflow_name.clone(),
                    event: r.event.clone(),
                    check_suite_id: None,
                    created_at: r.created_at.clone(),
                }),
                job_steps: Some(steps),
            },
            started_at: job.started_at,
            completed_at: job.completed_at,
        }
    }

    pub fn is_active(&self) -> bool {
        self.check.status != CheckStatus::Completed
    }

    pub fn duration_ms(&self) -> Option<i64> {
        corvene_models::check_duration_ms(self.started_at.as_deref(), self.completed_at.as_deref())
    }
}

/// The run list's filters.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunFilter {
    pub branch: Option<String>,
    /// An API `status` value ([`STATUS_FILTERS`]).
    pub status: Option<String>,
    pub event: Option<String>,
}

impl RunFilter {
    pub fn is_empty(&self) -> bool {
        self.branch.is_none() && self.status.is_none() && self.event.is_none()
    }
}

// ---- workflow_dispatch inputs ----

/// An input's type (`on.workflow_dispatch.inputs.<name>.type`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputKind {
    String,
    Boolean,
    Choice(Vec<String>),
    Number,
    /// A deployment environment of the repository.
    Environment,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowInput {
    pub name: String,
    pub description: Option<String>,
    pub required: bool,
    /// As a string (`true` / `false` for a boolean).
    pub default: Option<String>,
    pub kind: InputKind,
}

impl WorkflowInput {
    /// The label: the description, else the name.
    pub fn label(&self) -> &str {
        self.description
            .as_deref()
            .filter(|d| !d.trim().is_empty())
            .unwrap_or(&self.name)
    }

    /// The value the form starts with.
    pub fn initial_value(&self) -> String {
        match (&self.kind, &self.default) {
            (_, Some(default)) => default.clone(),
            (InputKind::Boolean, None) => "false".into(),
            (InputKind::Choice(options), None) => options.first().cloned().unwrap_or_default(),
            _ => String::new(),
        }
    }

    /// Why `value` cannot be sent, if it cannot.
    pub fn validate(&self, value: &str) -> Option<String> {
        let value = value.trim();
        if self.required && value.is_empty() && self.kind != InputKind::Boolean {
            return Some(format!("{} is required.", self.label()));
        }
        match &self.kind {
            InputKind::Number if !value.is_empty() && value.parse::<f64>().is_err() => {
                Some(format!("{} must be a number.", self.label()))
            }
            InputKind::Choice(options)
                if !value.is_empty() && !options.iter().any(|o| o == value) =>
            {
                Some(format!("{} must be one of its options.", self.label()))
            }
            _ => None,
        }
    }
}

/// What a workflow file says about `workflow_dispatch`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DispatchSpec {
    /// The workflow can be run by hand.
    pub dispatchable: bool,
    /// In the file's order.
    pub inputs: Vec<WorkflowInput>,
}

fn yaml_text(value: &Yaml) -> Option<String> {
    match value {
        Yaml::String(s) => Some(s.clone()),
        Yaml::Real(s) => Some(s.clone()),
        Yaml::Integer(i) => Some(i.to_string()),
        Yaml::Boolean(b) => Some(b.to_string()),
        _ => None,
    }
}

fn yaml_key(value: &Yaml) -> Option<String> {
    match value {
        // `on:` is a boolean to a YAML 1.1 reader; GitHub reads it as `on`
        Yaml::Boolean(true) => Some("on".into()),
        other => yaml_text(other),
    }
}

/// The `on` trigger list of a workflow document.
fn triggers(doc: &Yaml) -> Option<&Yaml> {
    let Yaml::Hash(map) = doc else { return None };
    map.iter()
        .find(|(k, _)| yaml_key(k).as_deref() == Some("on"))
        .map(|(_, v)| v)
}

/// Read a workflow file's `on.workflow_dispatch`: whether it is there (as a
/// string, an entry of a list or a key) and its inputs.
pub fn parse_workflow_dispatch(yaml: &str) -> Result<DispatchSpec, String> {
    let docs = YamlLoader::load_from_str(yaml)
        .map_err(|e| format!("The workflow file could not be read: {e}"))?;
    let Some(doc) = docs.first() else {
        return Ok(DispatchSpec::default());
    };
    let Some(on) = triggers(doc) else {
        return Ok(DispatchSpec::default());
    };
    let dispatch = match on {
        Yaml::String(event) => {
            return Ok(DispatchSpec {
                dispatchable: event == "workflow_dispatch",
                inputs: Vec::new(),
            });
        }
        Yaml::Array(events) => {
            return Ok(DispatchSpec {
                dispatchable: events
                    .iter()
                    .any(|e| yaml_text(e).as_deref() == Some("workflow_dispatch")),
                inputs: Vec::new(),
            });
        }
        Yaml::Hash(map) => match map
            .iter()
            .find(|(k, _)| yaml_text(k).as_deref() == Some("workflow_dispatch"))
        {
            Some((_, v)) => v,
            None => return Ok(DispatchSpec::default()),
        },
        _ => return Ok(DispatchSpec::default()),
    };
    let mut spec = DispatchSpec {
        dispatchable: true,
        inputs: Vec::new(),
    };
    let Yaml::Hash(inputs) = &dispatch["inputs"] else {
        return Ok(spec);
    };
    for (name, def) in inputs {
        let Some(name) = yaml_text(name) else {
            continue;
        };
        let options: Vec<String> = match &def["options"] {
            Yaml::Array(items) => items.iter().filter_map(yaml_text).collect(),
            _ => Vec::new(),
        };
        let kind = match yaml_text(&def["type"]).as_deref() {
            Some("boolean") => InputKind::Boolean,
            Some("choice") => InputKind::Choice(options),
            Some("number") => InputKind::Number,
            Some("environment") => InputKind::Environment,
            _ => InputKind::String,
        };
        let default = yaml_text(&def["default"]);
        let default = match (&kind, default) {
            // `default: True` and friends
            (InputKind::Boolean, Some(d)) => Some(d.eq_ignore_ascii_case("true").to_string()),
            (_, d) => d,
        };
        spec.inputs.push(WorkflowInput {
            name,
            description: yaml_text(&def["description"]),
            required: matches!(def["required"], Yaml::Boolean(true))
                || yaml_text(&def["required"]).is_some_and(|r| r.eq_ignore_ascii_case("true")),
            default,
            kind,
        });
    }
    Ok(spec)
}

/// A workflow file read at a ref, for Run workflow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DefinitionState {
    Loading,
    Loaded(DispatchSpec),
    /// The file is not on that branch or tag.
    Missing,
    Failed(String),
}

/// A branch or tag Run workflow can use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefChoice {
    pub name: String,
    pub tag: bool,
}

/// A dispatch whose run has not been seen in the list yet.
#[derive(Clone, Debug)]
pub struct PendingDispatch {
    pub workflow_id: u64,
    pub git_ref: String,
    pub since: SystemTime,
    pub started: Instant,
}

/// A message under the view's toolbar after an action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub text: String,
    pub error: bool,
}

/// The open Actions view (`AppState::actions`).
#[derive(Clone, Debug)]
pub struct ActionsViewState {
    pub github: GitHubRepository,
    /// The local repository, when it is cloned.
    pub repo: Option<u64>,
    /// No account for the repository's endpoint.
    pub signed_out: bool,
    /// The signed-in account's login and token scopes.
    pub login: Option<String>,
    pub scopes: Vec<String>,
    pub workflows: Vec<WorkflowRow>,
    pub workflows_loading: bool,
    pub workflows_loaded: bool,
    pub workflows_error: Option<String>,
    /// `None`: All workflows.
    pub selected_workflow: Option<u64>,
    pub filter: RunFilter,
    pub runs: Vec<RunRow>,
    pub total_runs: u64,
    /// Pages read of the current list.
    pub pages: u32,
    pub runs_loading: bool,
    pub runs_loaded: bool,
    pub runs_error: Option<String>,
    /// The first page's `ETag` and the query it answered.
    pub runs_etag: Option<(String, String)>,
    pub reload_pending: bool,
    pub loading_more: bool,
    pub selected_run: Option<u64>,
    pub jobs_run: Option<u64>,
    pub jobs: Vec<JobRow>,
    pub jobs_loading: bool,
    pub jobs_error: Option<String>,
    pub jobs_etag: Option<String>,
    /// The expanded job (its steps shown).
    pub expanded_job: Option<u64>,
    /// Workflow files read, by `(workflow id, ref)`.
    pub definitions: HashMap<(u64, String), DefinitionState>,
    pub refs: Option<Vec<RefChoice>>,
    pub refs_loading: bool,
    pub environments: Option<Vec<String>>,
    /// The action running on a run (`(run id, "Cancelling…")`).
    pub busy: Option<(u64, &'static str)>,
    pub dispatching: bool,
    pub dispatch_error: Option<String>,
    pub pending_dispatch: Option<PendingDispatch>,
    pub notice: Option<Notice>,
    /// Each opening gets a new one; the poll stops when it changes.
    pub generation: u64,
}

impl ActionsViewState {
    fn new(github: GitHubRepository, repo: Option<u64>, generation: u64) -> Self {
        Self {
            github,
            repo,
            signed_out: false,
            login: None,
            scopes: Vec::new(),
            workflows: Vec::new(),
            workflows_loading: false,
            workflows_loaded: false,
            workflows_error: None,
            selected_workflow: None,
            filter: RunFilter::default(),
            runs: Vec::new(),
            total_runs: 0,
            pages: 0,
            runs_loading: false,
            runs_loaded: false,
            runs_error: None,
            runs_etag: None,
            reload_pending: false,
            loading_more: false,
            selected_run: None,
            jobs_run: None,
            jobs: Vec::new(),
            jobs_loading: false,
            jobs_error: None,
            jobs_etag: None,
            expanded_job: None,
            definitions: HashMap::new(),
            refs: None,
            refs_loading: false,
            environments: None,
            busy: None,
            dispatching: false,
            dispatch_error: None,
            pending_dispatch: None,
            notice: None,
            generation,
        }
    }

    pub fn workflow(&self, id: u64) -> Option<&WorkflowRow> {
        self.workflows.iter().find(|w| w.id == id)
    }

    pub fn run(&self, id: u64) -> Option<&RunRow> {
        self.runs.iter().find(|r| r.id == id)
    }

    pub fn selected_run_row(&self) -> Option<&RunRow> {
        self.run(self.selected_run?)
    }

    pub fn job(&self, id: u64) -> Option<&JobRow> {
        self.jobs.iter().find(|j| j.check.id == id)
    }

    /// The workflow file at the default branch, once read.
    pub fn default_definition(&self, workflow: u64) -> Option<&DefinitionState> {
        let branch = self
            .github
            .default_branch
            .clone()
            .unwrap_or_else(|| "main".into());
        self.definitions.get(&(workflow, branch))
    }

    /// More runs than the pages read.
    pub fn has_more(&self) -> bool {
        (self.runs.len() as u64) < self.total_runs
    }

    /// Something shown is queued or running (or a dispatch is awaited):
    /// the poll reads again.
    pub fn is_live(&self) -> bool {
        self.runs.iter().any(RunRow::is_active)
            || self.jobs.iter().any(JobRow::is_active)
            || self.pending_dispatch.is_some()
    }

    /// The query of the first page (`page`: another one).
    pub fn query(&self, page: u32) -> RunsQuery {
        RunsQuery {
            workflow_id: self.selected_workflow,
            branch: self.filter.branch.clone(),
            status: self.filter.status.clone(),
            event: self.filter.event.clone(),
            per_page: RUNS_PER_PAGE,
            page,
        }
    }

    /// Why re-running, cancelling and dispatching are not possible, if they
    /// are not: no account, read access only, or a sign-in without the
    /// `repo` scope (`public_repo` is enough for a public repository).
    pub fn write_blocker(&self) -> Option<String> {
        if self.signed_out {
            return Some(format!(
                "Sign in to {} to run workflows.",
                host_name(&self.github)
            ));
        }
        if self.github.permissions == Some(RepositoryPermission::Read) {
            return Some(format!(
                "You can read {} but not write to it, so you cannot run its workflows.",
                self.github.full_name()
            ));
        }
        let scoped = !self.scopes.is_empty()
            && !self
                .scopes
                .iter()
                .any(|s| s == "repo" || (!self.github.private && s == "public_repo"));
        if scoped {
            return Some(
                "Your sign-in does not allow running workflows (it lacks the repo permission). Sign out and sign in again to allow it."
                    .into(),
            );
        }
        None
    }

    /// The branches for the Branch filter: the default one, those of the
    /// runs read, then the repository's.
    pub fn branch_choices(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut push = |b: &str| {
            if !b.is_empty() && !out.iter().any(|x| x == b) {
                out.push(b.to_string());
            }
        };
        if let Some(default) = &self.github.default_branch {
            push(default);
        }
        if let Some(branch) = &self.filter.branch {
            push(branch);
        }
        for run in &self.runs {
            if let Some(b) = &run.branch {
                push(b);
            }
        }
        for r in self.refs.iter().flatten().filter(|r| !r.tag) {
            push(&r.name);
        }
        out
    }

    /// The Event filter's choices: the usual ones, then any other seen.
    pub fn event_choices(&self) -> Vec<String> {
        let mut out: Vec<String> = EVENT_FILTERS.iter().map(|e| e.to_string()).collect();
        for run in &self.runs {
            if !run.event.is_empty() && !out.contains(&run.event) {
                out.push(run.event.clone());
            }
        }
        if let Some(event) = &self.filter.event
            && !out.contains(event)
        {
            out.push(event.clone());
        }
        out
    }
}

fn host_name(gh: &GitHubRepository) -> String {
    corvene_github::Endpoint::from_api_base(&gh.endpoint)
        .host()
        .to_string()
}

/// The open view, `None` while the flag is off.
pub fn actions_of(s: &AppState) -> Option<&ActionsViewState> {
    if !s.flags.bool(crate::flags::ids::ACTIONS) {
        return None;
    }
    s.actions.as_ref()
}

/// What an API error means here, in words (`what`: "load the runs").
pub fn describe_error(err: &GitHubError, what: &str, gh: &GitHubRepository) -> String {
    let host = host_name(gh);
    match err {
        GitHubError::Http(e) => format!("Could not {what}: {host} could not be reached ({e})."),
        GitHubError::Api {
            status, message, ..
        } => {
            let lower = message.to_lowercase();
            if lower.contains("rate limit") {
                format!(
                    "Could not {what}: the GitHub API rate limit is used up for now. Try again in a few minutes."
                )
            } else if *status == 401 {
                format!(
                    "Could not {what}: the sign-in to {host} is no longer valid. Sign in again."
                )
            } else if lower.contains("re-authorize sso") || lower.contains("saml") {
                format!("Could not {what}: {message}")
            } else if *status == 403
                || (*status == 404 && what.starts_with("re-run"))
                || (*status == 404 && what.starts_with("cancel"))
                || (*status == 404 && what.starts_with("run "))
            {
                format!(
                    "Could not {what}: your account is not allowed to. Running workflows needs write access to {}. ({message})",
                    gh.full_name()
                )
            } else if *status == 404 {
                format!(
                    "Could not {what}: {} has no Actions, or your account cannot see them.",
                    gh.full_name()
                )
            } else {
                format!("Could not {what}: {message}")
            }
        }
        other => format!("Could not {what}: {other}"),
    }
}

// ---- watched runs ----

/// A run started or re-run from the view, watched until it finishes.
#[derive(Clone, Debug)]
pub struct WatchedRun {
    pub github: GitHubRepository,
    pub repo: Option<u64>,
    pub run_id: u64,
    pub since: Instant,
    /// The attempt that was watched (a re-run makes a new one).
    pub attempt: u64,
}

/// `AppState::actions_watch`
#[derive(Clone, Debug, Default)]
pub struct ActionsWatch {
    pub runs: Vec<WatchedRun>,
    pub running: bool,
}

/// A run notification's payload: what a click opens.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionsRunNotification {
    pub actions_run_id: u64,
    pub github: GitHubRepository,
    pub repo: Option<u64>,
}

/// The notification's title and body for a finished run.
pub fn run_notification_text(run: &RunRow, gh: &GitHubRepository) -> (String, String) {
    let verb = match run.conclusion {
        Some(CheckConclusion::Success) => "succeeded",
        Some(CheckConclusion::Failure) => "failed",
        Some(CheckConclusion::Cancelled) => "was cancelled",
        Some(CheckConclusion::TimedOut) => "timed out",
        Some(CheckConclusion::Skipped) => "was skipped",
        Some(CheckConclusion::ActionRequired) => "needs action",
        _ => "finished",
    };
    let title = format!("{} {verb}", run.workflow_name);
    let mut body = format!("#{} {}", run.run_number, run.title);
    if let Some(branch) = &run.branch {
        body.push_str(&format!(" on {branch}"));
    }
    body.push_str(&format!(" in {}", gh.full_name()));
    (title, body)
}

fn is_open_in_any_window(s: &AppState) -> bool {
    s.workspaces.iter().any(|w| {
        w.popups
            .all_popups()
            .iter()
            .any(|p| matches!(p.popup, Popup::Actions { .. } | Popup::RunWorkflow { .. }))
    })
}

impl Dispatcher {
    fn actions_flags(cx: &dyn Host) -> (bool, bool) {
        let s = Self::state(cx).read(cx);
        (
            s.flags.bool(crate::flags::ids::API_SAML_SSO_HINT),
            s.flags.bool(crate::flags::ids::API_ERROR_DETAILS),
        )
    }

    /// The view's account was signed out by GitHub (a revoked token).
    fn actions_token_invalidated(api_base: &str, cx: &mut dyn Host) {
        let login = Self::state(cx)
            .read(cx)
            .actions
            .as_ref()
            .and_then(|v| v.login.clone());
        if let Some(login) = login {
            Self::token_invalidated(api_base, &login, cx);
        }
    }

    /// The client for the open view's repository, `None` when signed out.
    fn actions_client(cx: &dyn Host) -> Option<(Client, GitHubRepository, u64)> {
        let (gh, repo, generation) = {
            let s = Self::state(cx).read(cx);
            let view = actions_of(s)?;
            (view.github.clone(), view.repo, view.generation)
        };
        // `527-multiple-accounts`: a clone's own account
        let (endpoint, token, _) = match repo {
            Some(id) => Self::api_for_repository(id, &gh, cx),
            None => Self::api_for(&gh, cx),
        }?;
        let (sso, details) = Self::actions_flags(cx);
        Some((
            Client::new(endpoint, token)
                .with_sso_hint(sso)
                .with_error_details(details),
            gh,
            generation,
        ))
    }

    /// Update the open view when it is still the one of `generation`.
    fn update_actions<R>(
        generation: u64,
        cx: &mut dyn Host,
        f: impl FnOnce(&mut ActionsViewState) -> R,
    ) -> Option<R> {
        Self::state(cx).update(cx, |s, cx| {
            let view = s.actions.as_mut().filter(|v| v.generation == generation)?;
            let out = f(view);
            cx.notify();
            Some(out)
        })
    }

    /// Repository › Actions… for a local repository.
    pub fn show_repository_actions(id: u64, cx: &mut dyn Host) {
        let gh = Self::state(cx)
            .read(cx)
            .repository(id)
            .and_then(|r| r.github.clone());
        if let Some(gh) = gh {
            Self::show_actions(gh, Some(id), None, cx);
        }
    }

    /// Open the Actions view of `github` (`repo`: the local clone), `run`
    /// selected. The view of the same repository stays as it was.
    pub fn show_actions(
        github: GitHubRepository,
        repo: Option<u64>,
        run: Option<u64>,
        cx: &mut dyn Host,
    ) {
        if !Self::state(cx)
            .read(cx)
            .flags
            .bool(crate::flags::ids::ACTIONS)
        {
            return;
        }
        let repo = repo.or_else(|| {
            let s = Self::state(cx).read(cx);
            s.repositories
                .iter()
                .find(|r| {
                    r.github.as_ref().is_some_and(|g| {
                        g.endpoint == github.endpoint
                            && g.owner.eq_ignore_ascii_case(&github.owner)
                            && g.name.eq_ignore_ascii_case(&github.name)
                    })
                })
                .map(|r| r.id)
        });
        let account = {
            let s = Self::state(cx).read(cx);
            match repo {
                Some(id) => s.account_for_repository_on(id, &github.endpoint),
                None => s.account_for_github(&github),
            }
            .map(|a| (a.login.clone(), a.scopes.clone()))
        };
        let (fresh, generation) = Self::state(cx).update(cx, |s, cx| {
            let same = s.actions.as_ref().is_some_and(|v| {
                v.github.endpoint == github.endpoint
                    && v.github.owner.eq_ignore_ascii_case(&github.owner)
                    && v.github.name.eq_ignore_ascii_case(&github.name)
                    && v.signed_out == account.is_none()
            });
            let generation = if same {
                s.actions.as_ref().map(|v| v.generation).unwrap_or(0)
            } else {
                let generation = next_generation();
                let mut view = ActionsViewState::new(github.clone(), repo, generation);
                view.signed_out = account.is_none();
                if let Some((login, scopes)) = &account {
                    view.login = Some(login.clone());
                    view.scopes = scopes.clone();
                }
                s.actions = Some(view);
                generation
            };
            if let Some(view) = s.actions.as_mut() {
                // a newer copy of the repository (permissions)
                view.github = github.clone();
                if run.is_some() {
                    view.selected_run = run;
                }
            }
            cx.notify();
            (!same, generation)
        });
        Self::show_popup(Popup::Actions { github, repo }, cx);
        if fresh {
            Self::load_actions_workflows(cx);
            Self::load_actions_runs(false, cx);
            // the Branch filter's choices
            Self::load_actions_refs(cx);
            Self::start_actions_poll(generation, cx);
        } else {
            Self::refresh_actions(cx);
        }
        if run.is_some() {
            Self::load_actions_jobs(true, cx);
        }
    }

    /// The close button and Escape.
    pub fn close_actions(cx: &mut dyn Host) {
        Self::close_popup_if(|p| matches!(p, Popup::Actions { .. }), cx);
        Self::state(cx).update(cx, |s, cx| {
            if s.actions.take().is_some() {
                cx.notify();
            }
        });
    }

    /// The refresh button: everything again.
    pub fn refresh_actions(cx: &mut dyn Host) {
        Self::load_actions_workflows(cx);
        Self::load_actions_runs(false, cx);
        Self::load_actions_jobs(true, cx);
    }

    pub fn load_actions_workflows(cx: &mut dyn Host) {
        let Some((client, gh, generation)) = Self::actions_client(cx) else {
            Self::state(cx).update(cx, |s, cx| {
                if let Some(v) = s.actions.as_mut() {
                    v.signed_out = true;
                    v.workflows_loaded = true;
                    v.runs_loaded = true;
                    cx.notify();
                }
            });
            return;
        };
        let start = Self::update_actions(generation, cx, |v| {
            if v.workflows_loading {
                return false;
            }
            v.workflows_loading = true;
            true
        });
        if start != Some(true) {
            return;
        }
        spawn_bg(
            cx,
            move || {
                let result = client.workflows(&gh.owner, &gh.name);
                (
                    result.map_err(|e| {
                        (
                            describe_error(&e, "load the workflows", &gh),
                            e.is_token_invalidated(),
                        )
                    }),
                    gh,
                )
            },
            move |(result, gh), cx| {
                let invalid = matches!(&result, Err((_, true)));
                Self::update_actions(generation, cx, |v| {
                    v.workflows_loading = false;
                    v.workflows_loaded = true;
                    match result {
                        Ok(list) => {
                            v.workflows = list.into_iter().map(WorkflowRow::from_api).collect();
                            v.workflows_error = None;
                        }
                        Err((message, _)) => v.workflows_error = Some(message),
                    }
                });
                if invalid {
                    Self::actions_token_invalidated(&gh.endpoint, cx);
                }
            },
        );
    }

    /// Read the first page of runs again (conditionally when the query is
    /// the one the `ETag` answered), or the next page with `more`.
    pub fn load_actions_runs(more: bool, cx: &mut dyn Host) {
        let Some((client, gh, generation)) = Self::actions_client(cx) else {
            return;
        };
        let start = Self::update_actions(generation, cx, |v| {
            if v.runs_loading || v.loading_more {
                if !more {
                    v.reload_pending = true;
                }
                return None;
            }
            if more {
                if !v.has_more() {
                    return None;
                }
                v.loading_more = true;
                return Some((v.query(v.pages + 1), None));
            }
            v.runs_loading = true;
            let query = v.query(1);
            let path = query.path(&gh.owner, &gh.name);
            let etag = v
                .runs_etag
                .as_ref()
                .filter(|(p, _)| *p == path && v.runs_loaded && v.pages <= 1)
                .map(|(_, e)| e.clone());
            Some((query, etag))
        })
        .flatten();
        let Some((query, etag)) = start else {
            return;
        };
        let path = query.path(&gh.owner, &gh.name);
        spawn_bg(
            cx,
            move || {
                let result =
                    client.workflow_runs_page(&gh.owner, &gh.name, &query, etag.as_deref());
                (
                    result.map_err(|e| {
                        (
                            describe_error(&e, "load the runs", &gh),
                            e.is_token_invalidated(),
                        )
                    }),
                    gh,
                )
            },
            move |(result, gh), cx| {
                let invalid = matches!(&result, Err((_, true)));
                let outcome = Self::update_actions(generation, cx, |v| {
                    let reload = std::mem::take(&mut v.reload_pending);
                    if more {
                        v.loading_more = false;
                    } else {
                        v.runs_loading = false;
                        v.runs_loaded = true;
                    }
                    let mut changed_selected = false;
                    match result {
                        Ok(Conditional::NotModified) => debug!("runs not modified"),
                        Ok(Conditional::Fresh { value, etag }) => {
                            v.runs_error = None;
                            v.total_runs = value.total_count;
                            let rows: Vec<RunRow> = value
                                .workflow_runs
                                .into_iter()
                                .map(RunRow::from_api)
                                .collect();
                            if more {
                                v.pages += 1;
                                for row in rows {
                                    if !v.runs.iter().any(|r| r.id == row.id) {
                                        v.runs.push(row);
                                    }
                                }
                            } else {
                                let before = v.selected_run_row().cloned();
                                v.runs = rows;
                                v.pages = 1;
                                v.runs_etag = etag.map(|e| (path.clone(), e));
                                let after = v.selected_run_row().cloned();
                                changed_selected = before != after;
                            }
                            if v.selected_run.is_some_and(|r| v.run(r).is_none()) && !more {
                                v.selected_run = None;
                            }
                            if v.selected_run.is_none() {
                                v.selected_run = v.runs.first().map(|r| r.id);
                                changed_selected = true;
                            }
                        }
                        Err((message, _)) => v.runs_error = Some(message),
                    }
                    let found = v.pending_dispatch.clone().and_then(|p| {
                        let found = v.runs.iter().find(|r| {
                            r.workflow_id == p.workflow_id
                                && r.event == "workflow_dispatch"
                                && r.branch.as_deref() == Some(p.git_ref.as_str())
                                && corvene_models::parse_iso8601(&r.created_at)
                                    .is_some_and(|t| t + Duration::from_secs(60) >= p.since)
                        });
                        found
                            .map(|r| r.id)
                            .or_else(|| (p.started.elapsed() > DISPATCH_LOOKUP).then_some(0))
                    });
                    if let Some(found) = found {
                        v.pending_dispatch = None;
                        if found != 0 {
                            v.selected_run = Some(found);
                            changed_selected = true;
                        }
                    }
                    let finished: Vec<RunRow> =
                        v.runs.iter().filter(|r| !r.is_active()).cloned().collect();
                    (
                        reload,
                        changed_selected,
                        found.filter(|f| *f != 0),
                        finished,
                    )
                });
                if invalid {
                    Self::actions_token_invalidated(&gh.endpoint, cx);
                }
                let Some((reload, changed, dispatched, finished)) = outcome else {
                    return;
                };
                if let Some(run) = dispatched {
                    Self::watch_actions_run(gh.clone(), run, 1, cx);
                }
                Self::finish_watched_runs(&gh, &finished, cx);
                if changed {
                    Self::load_actions_jobs(true, cx);
                }
                if reload {
                    Self::load_actions_runs(false, cx);
                }
            },
        );
    }

    /// Pick a workflow on the left (`None`: All workflows).
    pub fn select_actions_workflow(workflow: Option<u64>, cx: &mut dyn Host) {
        let changed = Self::state(cx).update(cx, |s, cx| {
            let v = s.actions.as_mut()?;
            if v.selected_workflow == workflow {
                return None;
            }
            v.selected_workflow = workflow;
            v.runs.clear();
            v.total_runs = 0;
            v.pages = 0;
            v.runs_loaded = false;
            v.runs_error = None;
            v.selected_run = None;
            v.jobs.clear();
            v.jobs_run = None;
            v.jobs_etag = None;
            cx.notify();
            Some(())
        });
        if changed.is_some() {
            Self::load_actions_runs(false, cx);
            // whether Run workflow applies
            if let Some(workflow) = workflow {
                let default_branch = Self::state(cx)
                    .read(cx)
                    .actions
                    .as_ref()
                    .and_then(|v| v.github.default_branch.clone())
                    .unwrap_or_else(|| "main".into());
                Self::load_workflow_definition(workflow, default_branch, cx);
            }
        }
    }

    /// Change the run filters.
    pub fn set_actions_filter(filter: RunFilter, cx: &mut dyn Host) {
        let changed = Self::state(cx).update(cx, |s, cx| {
            let v = s.actions.as_mut()?;
            if v.filter == filter {
                return None;
            }
            v.filter = filter;
            v.runs.clear();
            v.total_runs = 0;
            v.pages = 0;
            v.runs_loaded = false;
            v.runs_error = None;
            v.selected_run = None;
            v.jobs.clear();
            v.jobs_run = None;
            v.jobs_etag = None;
            cx.notify();
            Some(())
        });
        if changed.is_some() {
            Self::load_actions_runs(false, cx);
        }
    }

    pub fn select_actions_run(run: u64, cx: &mut dyn Host) {
        let changed = Self::state(cx).update(cx, |s, cx| {
            let v = s.actions.as_mut()?;
            if v.selected_run == Some(run) {
                return None;
            }
            v.selected_run = Some(run);
            v.expanded_job = None;
            cx.notify();
            Some(())
        });
        if changed.is_some() {
            Self::load_actions_jobs(true, cx);
        }
    }

    /// Expand or collapse a job's steps.
    pub fn toggle_actions_job(job: u64, cx: &mut dyn Host) {
        Self::state(cx).update(cx, |s, cx| {
            if let Some(v) = s.actions.as_mut() {
                v.expanded_job = if v.expanded_job == Some(job) {
                    None
                } else {
                    Some(job)
                };
                cx.notify();
            }
        });
    }

    /// Read the selected run's jobs (`force`: even when the run is the one
    /// read last, conditionally).
    pub fn load_actions_jobs(force: bool, cx: &mut dyn Host) {
        let Some((client, gh, generation)) = Self::actions_client(cx) else {
            return;
        };
        let start = Self::update_actions(generation, cx, |v| {
            let run = v.selected_run?;
            if v.jobs_loading {
                return None;
            }
            let same = v.jobs_run == Some(run);
            if same && !force {
                return None;
            }
            if !same {
                v.jobs.clear();
                v.jobs_etag = None;
                v.jobs_error = None;
                v.jobs_run = Some(run);
                v.expanded_job = None;
            }
            v.jobs_loading = true;
            Some((run, v.jobs_etag.clone().filter(|_| same)))
        })
        .flatten();
        let Some((run, etag)) = start else {
            return;
        };
        spawn_bg(
            cx,
            move || {
                let result =
                    client.workflow_run_jobs_all(&gh.owner, &gh.name, run, etag.as_deref());
                (
                    result.map_err(|e| {
                        (
                            describe_error(&e, "load the jobs", &gh),
                            e.is_token_invalidated(),
                        )
                    }),
                    gh,
                )
            },
            move |(result, gh), cx| {
                let invalid = matches!(&result, Err((_, true)));
                let reload = Self::update_actions(generation, cx, |v| {
                    v.jobs_loading = false;
                    if v.jobs_run != Some(run) || v.selected_run != Some(run) {
                        // the selection moved meanwhile: read the new one's
                        return true;
                    }
                    match result {
                        Ok(Conditional::NotModified) => {}
                        Ok(Conditional::Fresh { value, etag }) => {
                            let row = v.run(run).cloned();
                            v.jobs = value
                                .into_iter()
                                .map(|j| JobRow::from_api(j, row.as_ref()))
                                .collect();
                            v.jobs_etag = etag;
                            v.jobs_error = None;
                            if v.expanded_job.is_none() {
                                // the first failed job opens, as on github.com
                                v.expanded_job = v
                                    .jobs
                                    .iter()
                                    .find(|j| j.check.is_failure())
                                    .map(|j| j.check.id);
                            }
                        }
                        Err((message, _)) => v.jobs_error = Some(message),
                    }
                    false
                });
                if invalid {
                    Self::actions_token_invalidated(&gh.endpoint, cx);
                }
                if reload == Some(true) {
                    Self::load_actions_jobs(true, cx);
                }
            },
        );
    }

    /// The poll of an open view: every [`POLL_INTERVAL`] while something is
    /// live, until the view closes.
    fn start_actions_poll(generation: u64, cx: &mut dyn Host) {
        cx.spawn(async move |cx: &mut AsyncCtx| {
            loop {
                cx.background_executor().timer(POLL_INTERVAL).await;
                if !cx.update(|cx| Self::poll_actions(generation, cx)) {
                    debug!(generation, "actions poll stopped");
                    break;
                }
            }
        })
        .detach();
    }

    /// One poll: `false` once the view closed.
    fn poll_actions(generation: u64, cx: &mut dyn Host) -> bool {
        let (open, live) = {
            let s = Self::state(cx).read(cx);
            match s.actions.as_ref().filter(|v| v.generation == generation) {
                Some(v) => (is_open_in_any_window(s), v.is_live()),
                None => return false,
            }
        };
        if !open {
            // closed by Escape or the popup stack: drop the view
            Self::state(cx).update(cx, |s, cx| {
                if s.actions
                    .as_ref()
                    .is_some_and(|v| v.generation == generation)
                {
                    s.actions = None;
                    cx.notify();
                }
            });
            return false;
        }
        if live && !crate::pull_requests::android_in_background() {
            Self::load_actions_runs(false, cx);
            Self::load_actions_jobs(true, cx);
        }
        true
    }

    /// Run `call` for `run` with the "busy" mark, then report and refresh.
    fn run_action(
        run: u64,
        busy: &'static str,
        what: &'static str,
        done: &'static str,
        watch: bool,
        call: impl FnOnce(&Client, &GitHubRepository) -> Result<(), GitHubError> + Send + 'static,
        cx: &mut dyn Host,
    ) {
        let Some((client, gh, generation)) = Self::actions_client(cx) else {
            return;
        };
        let attempt = Self::state(cx)
            .read(cx)
            .actions
            .as_ref()
            .and_then(|v| v.run(run))
            .map(|r| r.run_attempt)
            .unwrap_or(1);
        let start = Self::update_actions(generation, cx, |v| {
            if v.busy.is_some() {
                return false;
            }
            v.busy = Some((run, busy));
            v.notice = None;
            true
        });
        if start != Some(true) {
            return;
        }
        spawn_bg(
            cx,
            move || {
                let result = call(&client, &gh);
                (
                    result.map_err(|e| (describe_error(&e, what, &gh), e.is_token_invalidated())),
                    gh,
                )
            },
            move |(result, gh), cx| {
                let invalid = matches!(&result, Err((_, true)));
                let ok = result.is_ok();
                Self::update_actions(generation, cx, |v| {
                    v.busy = None;
                    v.notice = Some(match result {
                        Ok(()) => Notice {
                            text: done.into(),
                            error: false,
                        },
                        Err((message, _)) => Notice {
                            text: message,
                            error: true,
                        },
                    });
                });
                if invalid {
                    Self::actions_token_invalidated(&gh.endpoint, cx);
                }
                if ok {
                    info!(run, what, "actions run updated");
                    if watch {
                        Self::watch_actions_run(gh, run, attempt + 1, cx);
                    }
                    Self::load_actions_runs(false, cx);
                    Self::load_actions_jobs(true, cx);
                }
            },
        );
    }

    /// Re-run all jobs (or the failed ones) of `run`.
    pub fn rerun_actions_run(run: u64, failed_only: bool, cx: &mut dyn Host) {
        Self::run_action(
            run,
            "Re-running…",
            if failed_only {
                "re-run the failed jobs"
            } else {
                "re-run the jobs"
            },
            if failed_only {
                "Re-running the failed jobs."
            } else {
                "Re-running all jobs."
            },
            true,
            move |client, gh| client.rerun_workflow_run(&gh.owner, &gh.name, run, failed_only),
            cx,
        );
    }

    /// Re-run one job of `run`.
    pub fn rerun_actions_job(run: u64, job: u64, cx: &mut dyn Host) {
        Self::run_action(
            run,
            "Re-running…",
            "re-run the job",
            "Re-running the job.",
            true,
            move |client, gh| client.rerun_workflow_job(&gh.owner, &gh.name, job),
            cx,
        );
    }

    pub fn cancel_actions_run(run: u64, cx: &mut dyn Host) {
        Self::run_action(
            run,
            "Cancelling…",
            "cancel the run",
            "Cancelling the run.",
            false,
            move |client, gh| client.cancel_workflow_run(&gh.owner, &gh.name, run),
            cx,
        );
    }

    pub fn delete_actions_run_logs(run: u64, cx: &mut dyn Host) {
        Self::run_action(
            run,
            "Deleting logs…",
            "delete the logs",
            "The run's logs were deleted.",
            false,
            move |client, gh| client.delete_workflow_run_logs(&gh.owner, &gh.name, run),
            cx,
        );
    }

    pub fn dismiss_actions_notice(cx: &mut dyn Host) {
        Self::state(cx).update(cx, |s, cx| {
            if let Some(v) = s.actions.as_mut()
                && v.notice.take().is_some()
            {
                cx.notify();
            }
        });
    }

    /// Open a job's log (`step`: scroll to that step).
    pub fn open_actions_job_log(job: u64, step: Option<String>, cx: &mut dyn Host) {
        let found = {
            let s = Self::state(cx).read(cx);
            s.actions
                .as_ref()
                .and_then(|v| Some((v.repo, v.github.clone(), v.job(job)?.check.clone())))
        };
        if let Some((repo, github, check)) = found {
            Self::show_job_log(repo, github, check, step, cx);
        }
    }

    pub fn open_actions_url(url: String, cx: &mut dyn Host) {
        if !url.is_empty() {
            Self::open_url(&url, cx);
        }
    }

    // ---- Run workflow ----

    /// Run workflow… for `workflow`: the form, the refs and the file at
    /// the default branch.
    pub fn show_run_workflow(workflow: u64, cx: &mut dyn Host) {
        let found = {
            let s = Self::state(cx).read(cx);
            actions_of(s).map(|v| {
                (
                    v.github.clone(),
                    v.github
                        .default_branch
                        .clone()
                        .unwrap_or_else(|| "main".into()),
                )
            })
        };
        let Some((github, default_branch)) = found else {
            return;
        };
        Self::state(cx).update(cx, |s, cx| {
            if let Some(v) = s.actions.as_mut() {
                v.dispatch_error = None;
                cx.notify();
            }
        });
        Self::show_popup(Popup::RunWorkflow { github, workflow }, cx);
        Self::load_actions_refs(cx);
        Self::load_workflow_definition(workflow, default_branch, cx);
    }

    /// The branches and tags (once), and the environments.
    pub fn load_actions_refs(cx: &mut dyn Host) {
        let Some((client, gh, generation)) = Self::actions_client(cx) else {
            return;
        };
        let start = Self::update_actions(generation, cx, |v| {
            if v.refs.is_some() || v.refs_loading {
                return false;
            }
            v.refs_loading = true;
            true
        });
        if start != Some(true) {
            return;
        }
        spawn_bg(
            cx,
            move || {
                let branches = client.branch_names(&gh.owner, &gh.name, REF_PAGES);
                let tags = client
                    .tag_names(&gh.owner, &gh.name, REF_PAGES)
                    .unwrap_or_default();
                let environments = client.environment_names(&gh.owner, &gh.name).ok().flatten();
                (branches.map_err(|e| e.to_string()), tags, environments, gh)
            },
            move |(branches, tags, environments, gh), cx| {
                Self::update_actions(generation, cx, |v| {
                    v.refs_loading = false;
                    let mut branches = match branches {
                        Ok(b) => b,
                        Err(err) => {
                            warn!(%err, "could not list the branches");
                            Vec::new()
                        }
                    };
                    // the default branch first
                    if let Some(default) = &gh.default_branch {
                        branches.retain(|b| b != default);
                        branches.insert(0, default.clone());
                    }
                    let mut refs: Vec<RefChoice> = branches
                        .into_iter()
                        .map(|name| RefChoice { name, tag: false })
                        .collect();
                    refs.extend(tags.into_iter().map(|name| RefChoice { name, tag: true }));
                    v.refs = Some(refs);
                    v.environments = environments;
                });
            },
        );
    }

    /// Read `workflow`'s file at `git_ref` (once per pair).
    pub fn load_workflow_definition(workflow: u64, git_ref: String, cx: &mut dyn Host) {
        let Some((client, gh, generation)) = Self::actions_client(cx) else {
            return;
        };
        let key = (workflow, git_ref.clone());
        let path = Self::update_actions(generation, cx, |v| {
            if v.definitions.contains_key(&key) {
                return None;
            }
            let path = v.workflow(workflow)?.path.clone();
            v.definitions.insert(key.clone(), DefinitionState::Loading);
            Some(path)
        })
        .flatten();
        let Some(path) = path else {
            return;
        };
        spawn_bg(
            cx,
            move || match client.file_text(&gh.owner, &gh.name, &path, Some(&git_ref)) {
                Ok(Some(text)) => match parse_workflow_dispatch(&text) {
                    Ok(spec) => DefinitionState::Loaded(spec),
                    Err(err) => DefinitionState::Failed(err),
                },
                Ok(None) => DefinitionState::Missing,
                Err(err) => {
                    DefinitionState::Failed(describe_error(&err, "read the workflow file", &gh))
                }
            },
            move |state, cx| {
                Self::update_actions(generation, cx, |v| {
                    v.definitions.insert(key, state);
                });
            },
        );
    }

    /// Run workflow › Run: dispatch `workflow` on `git_ref` with `inputs`;
    /// the run is looked for in the list, selected and watched.
    pub fn dispatch_actions_workflow(
        workflow: u64,
        git_ref: String,
        inputs: Vec<(String, String)>,
        cx: &mut dyn Host,
    ) {
        let Some((client, gh, generation)) = Self::actions_client(cx) else {
            return;
        };
        let start = Self::update_actions(generation, cx, |v| {
            if v.dispatching {
                return false;
            }
            v.dispatching = true;
            v.dispatch_error = None;
            true
        });
        if start != Some(true) {
            return;
        }
        let since = SystemTime::now();
        let target = git_ref.clone();
        spawn_bg(
            cx,
            move || {
                let result =
                    client.dispatch_workflow(&gh.owner, &gh.name, workflow, &git_ref, &inputs);
                (
                    result.map_err(|e| {
                        (
                            describe_error(&e, "run the workflow", &gh),
                            e.is_token_invalidated(),
                        )
                    }),
                    gh,
                )
            },
            move |(result, gh), cx| {
                let invalid = matches!(&result, Err((_, true)));
                let ok = result.is_ok();
                Self::update_actions(generation, cx, |v| {
                    v.dispatching = false;
                    match result {
                        Ok(()) => {
                            info!(workflow, git_ref = %target, "workflow dispatched");
                            let name = v
                                .workflow(workflow)
                                .map(|w| w.name.clone())
                                .unwrap_or_else(|| "The workflow".into());
                            v.notice = Some(Notice {
                                text: format!("{name} was started on {target}."),
                                error: false,
                            });
                            v.pending_dispatch = Some(PendingDispatch {
                                workflow_id: workflow,
                                git_ref: target.clone(),
                                since,
                                started: Instant::now(),
                            });
                            // show the workflow's runs, unfiltered
                            if v.selected_workflow != Some(workflow) || !v.filter.is_empty() {
                                v.selected_workflow = Some(workflow);
                                v.filter = RunFilter::default();
                                v.runs.clear();
                                v.total_runs = 0;
                                v.pages = 0;
                                v.runs_loaded = false;
                                v.selected_run = None;
                                v.jobs.clear();
                                v.jobs_run = None;
                            }
                        }
                        Err((message, _)) => v.dispatch_error = Some(message),
                    }
                });
                if invalid {
                    Self::actions_token_invalidated(&gh.endpoint, cx);
                }
                if ok {
                    Self::close_popup_if(|p| matches!(p, Popup::RunWorkflow { .. }), cx);
                    // GitHub takes a moment to create the run
                    cx.spawn(async move |cx: &mut AsyncCtx| {
                        cx.background_executor().timer(Duration::from_secs(3)).await;
                        cx.update(|cx| Self::load_actions_runs(false, cx));
                    })
                    .detach();
                }
            },
        );
    }

    // ---- watching ----

    /// Watch `run` until it finishes (its `attempt` or a later one).
    fn watch_actions_run(github: GitHubRepository, run: u64, attempt: u64, cx: &mut dyn Host) {
        let start = Self::state(cx).update(cx, |s, _| {
            let repo = s.actions.as_ref().and_then(|v| v.repo);
            let w = &mut s.actions_watch;
            w.runs
                .retain(|r| !(r.run_id == run && r.github.endpoint == github.endpoint));
            w.runs.push(WatchedRun {
                github,
                repo,
                run_id: run,
                since: Instant::now(),
                attempt,
            });
            !std::mem::replace(&mut w.running, true)
        });
        if start {
            cx.spawn(async move |cx: &mut AsyncCtx| {
                loop {
                    cx.background_executor().timer(WATCH_INTERVAL).await;
                    if !cx.update(Self::check_watched_runs) {
                        break;
                    }
                }
            })
            .detach();
        }
    }

    /// One pass over the watched runs; `false` (and the loop ends) when
    /// none is left.
    fn check_watched_runs(cx: &mut dyn Host) -> bool {
        let runs = Self::state(cx).update(cx, |s, _| {
            let w = &mut s.actions_watch;
            w.runs.retain(|r| r.since.elapsed() < WATCH_LIFETIME);
            if w.runs.is_empty() {
                w.running = false;
            }
            w.runs.clone()
        });
        if runs.is_empty() {
            return false;
        }
        // the open view reads its runs itself
        let open_gh = Self::state(cx)
            .read(cx)
            .actions
            .as_ref()
            .map(|v| v.github.clone());
        for watched in runs {
            if open_gh
                .as_ref()
                .is_some_and(|g| same_repo(g, &watched.github))
            {
                continue;
            }
            let Some((endpoint, token, _)) = Self::api_for(&watched.github, cx) else {
                continue;
            };
            let gh = watched.github.clone();
            spawn_bg(
                cx,
                move || {
                    Client::new(endpoint, token)
                        .workflow_run(&gh.owner, &gh.name, watched.run_id)
                        .map(RunRow::from_api)
                },
                move |result, cx| match result {
                    Ok(row) => Self::finish_watched_runs(&watched.github, &[row], cx),
                    Err(err) => debug!(%err, "watched run not read"),
                },
            );
        }
        true
    }

    /// Notify about and stop watching the runs of `github` among `rows`
    /// that finished.
    fn finish_watched_runs(github: &GitHubRepository, rows: &[RunRow], cx: &mut dyn Host) {
        let done: Vec<(WatchedRun, RunRow)> = Self::state(cx).update(cx, |s, _| {
            let mut done = Vec::new();
            s.actions_watch.runs.retain(|w| {
                let finished = same_repo(&w.github, github).then(|| {
                    rows.iter()
                        .find(|r| r.id == w.run_id && !r.is_active() && r.run_attempt >= w.attempt)
                });
                match finished.flatten() {
                    Some(row) => {
                        done.push((w.clone(), row.clone()));
                        false
                    }
                    None => true,
                }
            });
            done
        });
        for (watched, row) in done {
            Self::notify_actions_run(&watched, &row, cx);
        }
    }

    fn notify_actions_run(watched: &WatchedRun, row: &RunRow, cx: &mut dyn Host) {
        if !Self::state(cx).read(cx).settings.notifications_enabled {
            debug!("notifications are disabled");
            return;
        }
        let (title, body) = run_notification_text(row, &watched.github);
        let payload = ActionsRunNotification {
            actions_run_id: row.id,
            github: watched.github.clone(),
            repo: watched.repo,
        };
        let Ok(payload) = serde_json::to_string(&payload) else {
            return;
        };
        let identifier = format!("corvene-actions-run-{}-{}", row.id, row.run_attempt);
        info!(%identifier, %title, "showing run notification");
        corvene_platform::notifications::show(
            &identifier,
            &title,
            &body,
            Some(&payload),
            |result| match result {
                Ok(()) => debug!("notification posted"),
                Err(err) => warn!(%err, "notification not shown"),
            },
        );
    }

    /// A click on a run notification: the view at the run. `false` when
    /// `payload` is not one.
    pub fn actions_notification_clicked(payload: &str, cx: &mut dyn Host) -> bool {
        let Ok(n) = serde_json::from_str::<ActionsRunNotification>(payload) else {
            return false;
        };
        Self::show_actions(n.github, n.repo, Some(n.actions_run_id), cx);
        true
    }
}

fn same_repo(a: &GitHubRepository, b: &GitHubRepository) -> bool {
    a.endpoint == b.endpoint
        && a.owner.eq_ignore_ascii_case(&b.owner)
        && a.name.eq_ignore_ascii_case(&b.name)
}

/// `CORVENE_POPUP=actions`: the view with sample workflows, runs and jobs,
/// no API behind it.
pub fn install_samples(id: Option<u64>, cx: &mut dyn Host) -> GitHubRepository {
    let gh = match id {
        Some(id) => crate::samples::github_repository(id, cx),
        None => crate::samples::stand_in_github_repository(),
    };
    let ago = crate::samples::iso_ago;
    let workflows = vec![
        WorkflowRow {
            id: 3,
            name: "CI".into(),
            path: ".github/workflows/ci.yml".into(),
            state: "active".into(),
            html_url: format!("{}/actions/workflows/ci.yml", gh.html_url),
        },
        WorkflowRow {
            id: 4,
            name: "Release".into(),
            path: ".github/workflows/release.yml".into(),
            state: "active".into(),
            html_url: format!("{}/actions/workflows/release.yml", gh.html_url),
        },
        WorkflowRow {
            id: 5,
            name: "Nightly".into(),
            path: ".github/workflows/nightly.yml".into(),
            state: "disabled_manually".into(),
            html_url: format!("{}/actions/workflows/nightly.yml", gh.html_url),
        },
    ];
    let run = |id: u64,
               workflow: (u64, &str),
               title: &str,
               number: u64,
               status: CheckStatus,
               conclusion: Option<CheckConclusion>,
               branch: &str,
               event: &str,
               started: u64,
               took: u64| RunRow {
        id,
        workflow_id: workflow.0,
        workflow_name: workflow.1.into(),
        title: title.into(),
        run_number: number,
        run_attempt: 1,
        status,
        conclusion,
        branch: Some(branch.into()),
        head_sha: "8f2a1c0d5e7b9a3f1c2d4e6f8a0b1c3d5e7f9a1b".into(),
        event: event.into(),
        actor: "octocat".into(),
        html_url: format!("{}/actions/runs/{id}", gh.html_url),
        created_at: ago(started),
        updated_at: Some(ago(started.saturating_sub(took))),
        run_started_at: Some(ago(started)),
    };
    let runs = vec![
        run(
            31,
            (3, "CI"),
            "Show workflow runs in the app",
            48,
            CheckStatus::InProgress,
            None,
            "actions-view",
            "push",
            95,
            0,
        ),
        run(
            30,
            (3, "CI"),
            "Fix the flaky fetch test",
            47,
            CheckStatus::Completed,
            Some(CheckConclusion::Failure),
            "main",
            "push",
            3_600,
            310,
        ),
        run(
            29,
            (4, "Release"),
            "Release",
            12,
            CheckStatus::Completed,
            Some(CheckConclusion::Success),
            "v0.2.0",
            "workflow_dispatch",
            86_400,
            1_254,
        ),
        run(
            28,
            (3, "CI"),
            "Render pull request bodies as Markdown",
            46,
            CheckStatus::Completed,
            Some(CheckConclusion::Cancelled),
            "markdown-bodies",
            "pull_request",
            2 * 86_400,
            75,
        ),
    ];
    let step = |number: u64, name: &str, conclusion: CheckConclusion, from: u64, to: u64| JobStep {
        name: name.into(),
        number,
        status: CheckStatus::Completed,
        conclusion: Some(conclusion),
        started_at: Some(ago(3_600 - from)),
        completed_at: Some(ago(3_600 - to)),
    };
    let job =
        |id: u64, name: &str, conclusion: CheckConclusion, took: u64, steps: Vec<JobStep>| JobRow {
            check: RefCheck {
                id,
                name: name.into(),
                description: String::new(),
                status: CheckStatus::Completed,
                conclusion: Some(conclusion),
                app_name: "GitHub Actions".into(),
                html_url: Some(format!("{}/actions/runs/30/job/{id}", gh.html_url)),
                check_suite_id: None,
                actions_workflow: Some(WorkflowRun {
                    id: 30,
                    workflow_id: 3,
                    name: "CI".into(),
                    event: "push".into(),
                    check_suite_id: None,
                    created_at: ago(3_600),
                }),
                job_steps: Some(steps),
            },
            started_at: Some(ago(3_600)),
            completed_at: Some(ago(3_600 - took)),
        };
    let jobs = vec![
        job(
            101,
            "test (macos-15)",
            CheckConclusion::Failure,
            310,
            vec![
                step(1, "Set up job", CheckConclusion::Success, 0, 2),
                step(2, "Checkout", CheckConclusion::Success, 2, 5),
                step(3, "cargo clippy", CheckConclusion::Success, 5, 99),
                step(
                    4,
                    "cargo test --workspace",
                    CheckConclusion::Failure,
                    99,
                    310,
                ),
                step(5, "Post Checkout", CheckConclusion::Skipped, 310, 310),
            ],
        ),
        job(
            102,
            "lint",
            CheckConclusion::Success,
            94,
            vec![
                step(1, "Set up job", CheckConclusion::Success, 0, 2),
                step(2, "cargo fmt --check", CheckConclusion::Success, 2, 94),
            ],
        ),
        job(
            103,
            "build (ubuntu-latest)",
            CheckConclusion::Success,
            182,
            vec![
                step(1, "Set up job", CheckConclusion::Success, 0, 2),
                step(2, "cargo build", CheckConclusion::Success, 2, 182),
            ],
        ),
    ];
    let mut definitions = HashMap::new();
    let default_branch = gh.default_branch.clone().unwrap_or_else(|| "main".into());
    if let Ok(spec) = parse_workflow_dispatch(SAMPLE_RELEASE_WORKFLOW) {
        definitions.insert((4, default_branch.clone()), DefinitionState::Loaded(spec));
    }
    let refs = vec![
        RefChoice {
            name: default_branch,
            tag: false,
        },
        RefChoice {
            name: "actions-view".into(),
            tag: false,
        },
        RefChoice {
            name: "v0.2.0".into(),
            tag: true,
        },
    ];
    let github = gh.clone();
    Dispatcher::state(cx).update(cx, |s, cx| {
        let mut view = ActionsViewState::new(github, id, next_generation());
        view.workflows = workflows;
        view.workflows_loaded = true;
        view.total_runs = runs.len() as u64;
        view.runs = runs;
        view.pages = 1;
        view.runs_loaded = true;
        view.selected_run = Some(30);
        view.jobs_run = Some(30);
        view.jobs = jobs;
        view.expanded_job = Some(101);
        view.definitions = definitions;
        view.refs = Some(refs);
        view.environments = Some(vec!["staging".into(), "production".into()]);
        view.login = Some("octocat".into());
        s.actions = Some(view);
        cx.notify();
    });
    gh
}

/// The sample Release workflow's file (`CORVENE_POPUP=run-workflow`).
pub const SAMPLE_RELEASE_WORKFLOW: &str = r#"name: Release
on:
  workflow_dispatch:
    inputs:
      version:
        description: Version to release
        required: true
        type: string
      channel:
        description: Channel
        type: choice
        options: [stable, beta, nightly]
        default: beta
      draft:
        description: Create the release as a draft
        type: boolean
        default: true
      retries:
        type: number
        default: 2
      target:
        description: Deploy to
        type: environment
  push:
    tags: ["v*"]
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_inputs_are_read_in_order() {
        let spec = parse_workflow_dispatch(SAMPLE_RELEASE_WORKFLOW).unwrap();
        assert!(spec.dispatchable);
        let names: Vec<&str> = spec.inputs.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["version", "channel", "draft", "retries", "target"]);
        let version = &spec.inputs[0];
        assert!(version.required);
        assert_eq!(version.kind, InputKind::String);
        assert_eq!(version.label(), "Version to release");
        assert_eq!(
            spec.inputs[1].kind,
            InputKind::Choice(vec!["stable".into(), "beta".into(), "nightly".into()])
        );
        assert_eq!(spec.inputs[1].initial_value(), "beta");
        assert_eq!(spec.inputs[2].kind, InputKind::Boolean);
        assert_eq!(spec.inputs[2].initial_value(), "true");
        assert_eq!(spec.inputs[3].kind, InputKind::Number);
        assert_eq!(spec.inputs[3].initial_value(), "2");
        assert_eq!(spec.inputs[3].label(), "retries");
        assert_eq!(spec.inputs[4].kind, InputKind::Environment);
        assert!(!spec.inputs[4].required);
    }

    #[test]
    fn dispatch_triggers_in_every_shape() {
        let read = |y: &str| parse_workflow_dispatch(y).unwrap().dispatchable;
        assert!(read("on: workflow_dispatch\n"));
        assert!(read("on: [push, workflow_dispatch]\n"));
        assert!(read("on:\n  workflow_dispatch:\n  push:\n"));
        assert!(read("'on':\n  workflow_dispatch: {}\n"));
        assert!(!read("on: push\n"));
        assert!(!read("on: [push, pull_request]\n"));
        assert!(!read("on:\n  push:\n    branches: [main]\n"));
        assert!(!read("name: no triggers\n"));
        assert!(!read(""));
        assert!(parse_workflow_dispatch("on: [unclosed\n").is_err());
        let plain = parse_workflow_dispatch("on:\n  workflow_dispatch:\n").unwrap();
        assert!(plain.inputs.is_empty());
    }

    #[test]
    fn input_defaults_and_validation() {
        let spec = parse_workflow_dispatch(
            "on:\n  workflow_dispatch:\n    inputs:\n      dry:\n        type: boolean\n        default: True\n      count:\n        type: number\n        required: true\n      env:\n        type: choice\n        options: [a, b]\n      free:\n        default: 3\n",
        )
        .unwrap();
        let dry = &spec.inputs[0];
        assert_eq!(dry.initial_value(), "true");
        assert_eq!(dry.validate(""), None);
        let count = &spec.inputs[1];
        assert_eq!(count.validate(""), Some("count is required.".into()));
        assert_eq!(count.validate("x"), Some("count must be a number.".into()));
        assert_eq!(count.validate("1.5"), None);
        let env = &spec.inputs[2];
        assert_eq!(env.initial_value(), "a");
        assert!(env.validate("c").is_some());
        assert_eq!(spec.inputs[3].kind, InputKind::String);
        assert_eq!(spec.inputs[3].initial_value(), "3");
    }

    fn row(status: CheckStatus, conclusion: Option<CheckConclusion>) -> RunRow {
        RunRow {
            id: 1,
            workflow_id: 1,
            workflow_name: "CI".into(),
            title: "Fix it".into(),
            run_number: 7,
            run_attempt: 1,
            status,
            conclusion,
            branch: Some("main".into()),
            head_sha: String::new(),
            event: "push".into(),
            actor: "octocat".into(),
            html_url: String::new(),
            created_at: "2024-05-01T10:00:00Z".into(),
            updated_at: Some("2024-05-01T10:05:10Z".into()),
            run_started_at: Some("2024-05-01T10:00:00Z".into()),
        }
    }

    #[test]
    fn runs_read_well() {
        let done = row(CheckStatus::Completed, Some(CheckConclusion::Failure));
        assert!(!done.is_active());
        assert!(done.has_failed_jobs());
        assert_eq!(done.status_text(), "Failure");
        assert_eq!(done.duration_ms(SystemTime::now()), Some(310_000));
        let ok = row(CheckStatus::Completed, Some(CheckConclusion::Success));
        assert!(!ok.has_failed_jobs());
        let running = row(CheckStatus::InProgress, None);
        assert!(running.is_active());
        assert!(!running.has_failed_jobs());
        let gh = crate::samples::stand_in_github_repository();
        let (title, body) = run_notification_text(&done, &gh);
        assert_eq!(title, "CI failed");
        assert_eq!(body, format!("#7 Fix it on main in {}", gh.full_name()));
    }

    #[test]
    fn write_blockers() {
        let mut gh = crate::samples::stand_in_github_repository();
        let mut view = ActionsViewState::new(gh.clone(), None, 1);
        assert_eq!(view.write_blocker(), None);
        view.scopes = vec!["read:user".into()];
        assert!(view.write_blocker().is_some());
        view.scopes = vec!["public_repo".into()];
        assert_eq!(view.write_blocker(), None);
        gh.permissions = Some(RepositoryPermission::Read);
        view.github = gh;
        assert!(view.write_blocker().is_some());
        view.signed_out = true;
        assert!(view.write_blocker().unwrap().starts_with("Sign in"));
    }

    #[test]
    fn errors_read_well() {
        let gh = crate::samples::stand_in_github_repository();
        let limited = GitHubError::api(403, "API rate limit exceeded for user ID 1.");
        assert!(describe_error(&limited, "load the runs", &gh).contains("rate limit is used up"));
        let refused = GitHubError::api(403, "Resource not accessible by integration");
        assert!(describe_error(&refused, "re-run the jobs", &gh).contains("write access"));
        let missing = GitHubError::api(404, "Not Found");
        assert!(describe_error(&missing, "load the runs", &gh).contains("has no Actions"));
        assert!(describe_error(&missing, "run the workflow", &gh).contains("write access"));
        let invalid = GitHubError::api(422, "Unexpected inputs provided: [\"x\"]");
        assert_eq!(
            describe_error(&invalid, "run the workflow", &gh),
            "Could not run the workflow: Unexpected inputs provided: [\"x\"]"
        );
    }

    #[test]
    fn notification_payload_round_trips() {
        let n = ActionsRunNotification {
            actions_run_id: 30,
            github: crate::samples::stand_in_github_repository(),
            repo: Some(4),
        };
        let text = serde_json::to_string(&n).unwrap();
        assert_eq!(
            serde_json::from_str::<ActionsRunNotification>(&text).unwrap(),
            n
        );
        // a pull request notification is not one
        assert!(
            serde_json::from_str::<ActionsRunNotification>(r#"{"repo":1,"owner":"o"}"#).is_err()
        );
    }
}
