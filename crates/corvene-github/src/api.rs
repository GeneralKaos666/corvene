//! Minimal REST client: the calls core parity needs (`lib/api.ts`).
//!
//! Deviation (flag `api-saml-sso-hint`, `Client::with_sso_hint`): a 403
//! with `X-GitHub-SSO: required; url=…` adds "Re-authorize SSO for <org>"
//! and the URL to the error; GHD's `lib/api.ts` keeps GitHub's message only.

use std::time::Duration;

use corvene_models::{
    Account, BypassReason, CheckConclusion, CheckStatus, GitHubRepository, RepoRuleEnforced,
    RepositoryPermission, RuleOperator,
};
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::USER_AGENT;
use crate::endpoint::Endpoint;
use crate::error::{ApiErrorBody, GitHubError, Result};

/// The response headers the paging helpers read (GHD `Response.headers`):
/// `(name, value)` pairs, names matched without regard to case.
pub type ResponseHeaders = [(String, String)];

pub struct Client {
    agent: ureq::Agent,
    endpoint: Endpoint,
    token: String,
    /// Append the body's `errors[].message` to an error's message.
    error_details: bool,
    /// Flag `api-saml-sso-hint`: a 403 carrying `X-GitHub-SSO` says which
    /// organization needs its SSO authorization renewed, and where.
    sso_hint: bool,
}

/// Flag `api-saml-sso-hint`: the hint for an `X-GitHub-SSO: required;
/// url=<authorize url>` header, naming the organization when the URL is
/// `…/orgs/<org>/sso…`. Other values (`partial-results; organizations=…`)
/// give no hint.
pub fn sso_hint(header: &str) -> Option<String> {
    let mut parts = header.split(';').map(str::trim);
    if !parts.next()?.eq_ignore_ascii_case("required") {
        return None;
    }
    let url = parts.find_map(|p| p.strip_prefix("url="))?.trim();
    if url.is_empty() {
        return None;
    }
    let org = url
        .split_once("/orgs/")
        .and_then(|(_, rest)| rest.split(['/', '?']).next())
        .filter(|org| !org.is_empty());
    Some(match org {
        Some(org) => format!("Re-authorize SSO for {org}: {url}"),
        None => format!("Re-authorize SSO: {url}"),
    })
}

#[derive(Debug, Deserialize)]
struct ApiUser {
    id: u64,
    login: String,
    name: Option<String>,
    avatar_url: Option<String>,
    #[serde(default)]
    plan: Option<ApiPlan>,
}

#[derive(Debug, Deserialize)]
struct ApiPlan {
    name: String,
}

#[derive(Debug, Deserialize)]
struct ApiEmail {
    email: String,
    primary: bool,
    verified: bool,
    /// `public` / `private`; missing on older Enterprise versions.
    #[serde(default)]
    visibility: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiRepository {
    pub name: String,
    pub owner: ApiOwner,
    pub html_url: String,
    pub clone_url: String,
    #[serde(default)]
    pub ssh_url: Option<String>,
    pub default_branch: Option<String>,
    #[serde(default)]
    pub private: bool,
    #[serde(default)]
    pub fork: bool,
    pub parent: Option<Box<ApiRepository>>,
    #[serde(default)]
    pub pushed_at: Option<String>,
    #[serde(default)]
    pub archived: bool,
    /// Only on `GET /repos/{owner}/{name}` with a token (`IAPIRepositoryPermissions`).
    #[serde(default)]
    pub permissions: Option<ApiRepositoryPermissions>,
    /// `false` when the owner disabled forking.
    #[serde(default)]
    pub allow_forking: Option<bool>,
}

/// `IAPIRepositoryPermissions`
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct ApiRepositoryPermissions {
    #[serde(default)]
    pub admin: bool,
    /// aka write
    #[serde(default)]
    pub push: bool,
    /// aka read
    #[serde(default)]
    pub pull: bool,
}

impl ApiRepositoryPermissions {
    /// `getPermissionsString`
    pub fn permission(self) -> Option<RepositoryPermission> {
        if self.admin {
            Some(RepositoryPermission::Admin)
        } else if self.push {
            Some(RepositoryPermission::Write)
        } else if self.pull {
            Some(RepositoryPermission::Read)
        } else {
            None
        }
    }
}

/// A GitHub release (`GET /repos/{owner}/{repo}/releases/tags/{tag}`).
#[derive(Debug, Clone, Deserialize)]
pub struct ApiRelease {
    pub tag_name: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    /// ISO-8601.
    #[serde(default)]
    pub published_at: Option<String>,
    pub html_url: String,
}

/// GHD `IAPIRepositoryCloneInfo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryCloneInfo {
    pub url: String,
    pub default_branch: Option<String>,
}

/// `state` filter for [`Client::issues`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueState {
    Open,
    Closed,
    All,
}

impl IssueState {
    fn as_str(self) -> &'static str {
        match self {
            IssueState::Open => "open",
            IssueState::Closed => "closed",
            IssueState::All => "all",
        }
    }
}

/// `IAPIIssue` (+ the `pull_request` marker used to filter PRs out).
#[derive(Debug, Clone, Deserialize)]
pub struct ApiIssue {
    pub number: u64,
    pub title: String,
    /// `open` | `closed`
    pub state: String,
    pub updated_at: String,
    #[serde(default)]
    pub pull_request: Option<serde_json::Value>,
}

/// `IAPIFullIdentity` (`GET /users/{login}`), also the `IAPIIdentity` on
/// reviews and comments.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ApiIdentity {
    pub id: u64,
    pub login: String,
    /// The profile page (`IAPIIdentity.html_url`).
    #[serde(default)]
    pub html_url: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
    /// `type`: `User`, `Organization` or `Bot`.
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
}

/// `IAPIPullRequestReview.state`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ApiPullRequestReviewState {
    Approved,
    Dismissed,
    Pending,
    Commented,
    ChangesRequested,
}

/// `IAPIPullRequestReview` (`GET /repos/{owner}/{repo}/pulls/{n}/reviews/{id}`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ApiPullRequestReview {
    pub id: u64,
    pub user: ApiIdentity,
    /// Empty for a review without a summary comment.
    #[serde(default, deserialize_with = "null_as_empty")]
    pub body: String,
    pub html_url: String,
    pub submitted_at: String,
    pub state: ApiPullRequestReviewState,
}

/// `IAPIComment`: an issue comment on a pull request, or a review comment
/// (`GET /repos/{owner}/{repo}/issues/comments/{id}`,
/// `…/pulls/comments/{id}`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ApiIssueComment {
    pub id: u64,
    #[serde(default, deserialize_with = "null_as_empty")]
    pub body: String,
    pub html_url: String,
    pub user: ApiIdentity,
    pub created_at: String,
}

/// `null` → `""` for bodies the API may send as `null`.
fn null_as_empty<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<String, D::Error> {
    Ok(Option::<String>::deserialize(d)?.unwrap_or_default())
}

/// `IAPIMentionableUser`.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiMentionableUser {
    pub login: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiOwner {
    pub login: String,
}

/// `IAPIPullRequestRef`
#[derive(Debug, Clone, Deserialize)]
pub struct ApiPullRequestRef {
    #[serde(rename = "ref")]
    pub ref_name: String,
    pub sha: String,
    /// `null` when the head repository was deleted.
    pub repo: Option<ApiRepository>,
}

/// `IAPIPullRequest`
#[derive(Debug, Clone, Deserialize)]
pub struct ApiPullRequest {
    pub number: u64,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub user: ApiOwner,
    pub head: ApiPullRequestRef,
    pub base: ApiPullRequestRef,
    #[serde(default)]
    pub body: Option<String>,
    /// `open` | `closed`
    pub state: String,
    #[serde(default)]
    pub draft: bool,
}

/// `IAPIRefStatusItem` (the legacy commit status API).
#[derive(Debug, Clone, Deserialize)]
pub struct ApiRefStatusItem {
    pub id: u64,
    /// `success` | `pending` | `failure` | `error`
    pub state: String,
    #[serde(default)]
    pub target_url: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub context: String,
}

/// `IAPIRefStatus`
#[derive(Debug, Clone, Deserialize)]
pub struct ApiRefStatus {
    pub state: String,
    #[serde(default)]
    pub total_count: u64,
    #[serde(default)]
    pub statuses: Vec<ApiRefStatusItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiCheckSuiteRef {
    pub id: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiCheckApp {
    #[serde(default)]
    pub name: String,
}

/// `IAPIRefCheckRun`
#[derive(Debug, Clone, Deserialize)]
pub struct ApiRefCheckRun {
    pub id: u64,
    pub status: CheckStatus,
    #[serde(default)]
    pub conclusion: Option<CheckConclusion>,
    pub name: String,
    #[serde(default)]
    pub check_suite: Option<ApiCheckSuiteRef>,
    #[serde(default)]
    pub app: Option<ApiCheckApp>,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub html_url: Option<String>,
    #[serde(default)]
    pub pull_requests: Vec<serde_json::Value>,
}

/// `IAPIRefCheckRuns`
#[derive(Debug, Clone, Deserialize)]
pub struct ApiRefCheckRuns {
    #[serde(default)]
    pub total_count: u64,
    #[serde(default)]
    pub check_runs: Vec<ApiRefCheckRun>,
}

/// `IAPICheckSuite`
#[derive(Debug, Clone, Deserialize)]
pub struct ApiCheckSuite {
    pub id: u64,
    #[serde(default)]
    pub rerequestable: bool,
    #[serde(default)]
    pub runs_rerequestable: bool,
    pub status: CheckStatus,
    pub created_at: String,
}

/// `IAPIWorkflowRun`
#[derive(Debug, Clone, Deserialize)]
pub struct ApiWorkflowRun {
    pub id: u64,
    pub workflow_id: u64,
    #[serde(default)]
    pub name: String,
    pub created_at: String,
    #[serde(default)]
    pub check_suite_id: Option<u64>,
    #[serde(default)]
    pub event: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiWorkflowRuns {
    #[serde(default)]
    pub total_count: u64,
    #[serde(default)]
    pub workflow_runs: Vec<ApiWorkflowRun>,
}

/// `IAPIWorkflowJobStep`
#[derive(Debug, Clone, Deserialize)]
pub struct ApiWorkflowJobStep {
    pub name: String,
    pub number: u64,
    pub status: CheckStatus,
    #[serde(default)]
    pub conclusion: Option<CheckConclusion>,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub started_at: Option<String>,
}

/// `IAPIWorkflowJob`
#[derive(Debug, Clone, Deserialize)]
pub struct ApiWorkflowJob {
    pub id: u64,
    pub name: String,
    pub status: CheckStatus,
    #[serde(default)]
    pub conclusion: Option<CheckConclusion>,
    #[serde(default)]
    pub steps: Vec<ApiWorkflowJobStep>,
    #[serde(default)]
    pub html_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiWorkflowJobs {
    #[serde(default)]
    pub total_count: u64,
    #[serde(default)]
    pub jobs: Vec<ApiWorkflowJob>,
}

/// `IAPIPushControl`
#[derive(Debug, Clone, Deserialize)]
pub struct ApiPushControl {
    #[serde(default)]
    pub required_status_checks: Vec<String>,
    #[serde(default)]
    pub required_approving_review_count: u64,
    #[serde(default = "default_true")]
    pub allow_actor: bool,
    #[serde(default)]
    pub pattern: Option<String>,
    #[serde(default)]
    pub required_signatures: bool,
    #[serde(default)]
    pub required_linear_history: bool,
    #[serde(default = "default_true")]
    pub allow_deletions: bool,
    #[serde(default = "default_true")]
    pub allow_force_pushes: bool,
}

fn default_true() -> bool {
    true
}

impl ApiPushControl {
    /// A branch nobody can push to directly (`isBranchPushable` negated).
    pub fn is_pushable(&self) -> bool {
        self.allow_actor
            && self.required_status_checks.is_empty()
            && self.required_approving_review_count == 0
    }
}

impl Default for ApiPushControl {
    fn default() -> Self {
        Self {
            required_status_checks: Vec::new(),
            required_approving_review_count: 0,
            allow_actor: true,
            pattern: None,
            required_signatures: false,
            required_linear_history: false,
            allow_deletions: true,
            allow_force_pushes: true,
        }
    }
}

/// `IAPIRepoRuleMetadataParameters`
#[derive(Debug, Clone, Deserialize)]
pub struct ApiRepoRuleParameters {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub negate: bool,
    pub pattern: String,
    pub operator: RuleOperator,
}

/// `IAPIRepoRule`
#[derive(Debug, Clone, Deserialize)]
pub struct ApiRepoRule {
    pub ruleset_id: u64,
    /// `creation` | `update` | `required_signatures` | `commit_message_pattern` | …
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub parameters: Option<ApiRepoRuleParameters>,
}

/// `IAPISlimRepoRuleset`
#[derive(Debug, Clone, Deserialize)]
pub struct ApiSlimRepoRuleset {
    pub id: u64,
}

/// `IAPIRepoRuleset`
#[derive(Debug, Clone, Deserialize)]
pub struct ApiRepoRuleset {
    pub id: u64,
    /// `always` | `pull_requests_only` | `never`
    #[serde(default)]
    pub current_user_can_bypass: Option<String>,
}

impl ApiRepoRuleset {
    pub fn enforced(&self) -> RepoRuleEnforced {
        if self.current_user_can_bypass.as_deref() == Some("always") {
            RepoRuleEnforced::Bypass
        } else {
            RepoRuleEnforced::Yes
        }
    }
}

/// `IAPICreatePushProtectionBypassResponse`
#[derive(Debug, Clone, Deserialize)]
pub struct ApiPushProtectionBypass {
    pub reason: String,
    #[serde(default)]
    pub expire_at: Option<String>,
    #[serde(default)]
    pub token_type: Option<String>,
}

/// `encodeURIComponent` for refs and branch names in API paths.
pub fn encode_path_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// GHD `IAPIBranch` (`GET repos/{owner}/{name}/branches`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ApiBranch {
    /// The branch name on the remote (not a full ref).
    pub name: String,
    /// The branch has protection rules.
    #[serde(default)]
    pub protected: bool,
}

/// `headers.get(name)`: the first value of `name`, any case.
fn header<'a>(headers: &'a ResponseHeaders, name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

/// The headers of a response as [`ResponseHeaders`] pairs.
fn header_pairs(headers: &ureq::http::HeaderMap) -> Vec<(String, String)> {
    headers
        .iter()
        .filter_map(|(name, value)| Some((name.to_string(), value.to_str().ok()?.to_string())))
        .collect()
}

/// GHD `getNextPagePathFromLink`: the path (and query) of the `rel="next"`
/// link in the `Link` header, `None` without one.
pub fn get_next_page_path_from_link(headers: &ResponseHeaders) -> Option<String> {
    let link = header(headers, "Link").filter(|l| !l.is_empty())?;
    for part in link.split(',') {
        // `<([^>]+)>; rel="([^"]+)"`
        let Some((_, rest)) = part.split_once('<') else {
            continue;
        };
        let Some((url, rest)) = rest.split_once('>') else {
            continue;
        };
        let Some(rel) = rest
            .strip_prefix("; rel=\"")
            .and_then(|r| r.split_once('"'))
            .map(|(rel, _)| rel)
        else {
            continue;
        };
        if !url.is_empty() && rel == "next" {
            return url_path(url);
        }
    }
    None
}

/// Node's `URL.parse(url).path`: path and query, without scheme, host and
/// fragment.
fn url_path(url: &str) -> Option<String> {
    let url = url.split('#').next().unwrap_or(url);
    let path = match url.split_once("://") {
        Some((_, rest)) => match rest.find(['/', '?']) {
            Some(i) if rest[i..].starts_with('?') => format!("/{}", &rest[i..]),
            Some(i) => rest[i..].to_string(),
            None => "/".to_string(),
        },
        None => url.to_string(),
    };
    (!path.is_empty()).then_some(path)
}

/// JavaScript's `parseInt(value, 10)`: the leading integer, `None` for NaN.
fn parse_int(value: &str) -> Option<i64> {
    let value = value.trim_start();
    let (sign, digits) = match value.as_bytes().first() {
        Some(b'-') => (-1, &value[1..]),
        Some(b'+') => (1, &value[1..]),
        _ => (1, value),
    };
    let end = digits
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(digits.len());
    digits[..end].parse::<i64>().ok().map(|n| sign * n)
}

/// GHD `getNextPagePathWithIncreasingPageSize`: the next link
/// ([`get_next_page_path_from_link`]), with `per_page` doubled (at most
/// 100) and `page` recomputed once the items received so far fill whole
/// pages of the doubled size, so a long list is read in fewer requests.
pub fn get_next_page_path_with_increasing_page_size(headers: &ResponseHeaders) -> Option<String> {
    let next_path = get_next_page_path_from_link(headers)?;
    let (pathname, query) = next_path.split_once('?').unwrap_or((&next_path, ""));
    let mut pairs: Vec<(&str, &str)> = query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| pair.split_once('=').unwrap_or((pair, "")))
        .collect();
    // `typeof per_page === 'string'`: exactly one value
    let single = |key: &str| {
        let mut values = pairs.iter().filter(|(k, _)| *k == key).map(|(_, v)| *v);
        match (values.next(), values.next()) {
            (Some(value), None) => parse_int(value),
            _ => None,
        }
    };
    let (Some(page_size), Some(page_number)) = (single("per_page"), single("page")) else {
        return Some(next_path);
    };
    if page_size == 0 || page_number == 0 {
        return Some(next_path);
    }
    // the next page's link: what came before it has been received
    let (Some(received), Some(doubled)) = (
        (page_number - 1).checked_mul(page_size),
        page_size.checked_mul(2),
    ) else {
        return Some(next_path);
    };
    let next_page_size = doubled.min(100);
    if page_size == next_page_size || received % next_page_size != 0 {
        return Some(next_path);
    }
    let per_page = next_page_size.to_string();
    let page = (received / next_page_size + 1).to_string();
    for (key, value) in &mut pairs {
        match *key {
            "per_page" => *value = per_page.as_str(),
            "page" => *value = page.as_str(),
            _ => {}
        }
    }
    let query: Vec<String> = pairs.iter().map(|(k, v)| format!("{k}={v}")).collect();
    Some(format!("{pathname}?{}", query.join("&")))
}

impl Client {
    pub fn new(endpoint: Endpoint, token: impl Into<String>) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30)))
            .http_status_as_error(false)
            .user_agent(USER_AGENT)
            .build()
            .new_agent();
        Self {
            agent,
            endpoint,
            token: token.into(),
            error_details: false,
            sso_hint: false,
        }
    }

    /// A 403 that needs SAML SSO authorization says so (flag
    /// `api-saml-sso-hint`; GHD shows GitHub's message only).
    pub fn with_sso_hint(mut self, on: bool) -> Self {
        self.sso_hint = on;
        self
    }

    /// Error messages show validation `errors` without a `message` as
    /// `field code`, and publishing to an organization reports GitHub's
    /// reasons (flag `api-error-details`; GHD drops those errors and
    /// replaces an organization's error with a generic hint).
    pub fn with_error_details(mut self, on: bool) -> Self {
        self.error_details = on;
        self
    }

    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    fn get_json<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        self.get_json_accept(path, "application/vnd.github+json")
    }

    /// GHD `parsedResponse` for an error status: `APIError` with the parsed
    /// body, its `message` (or `API error <url>: <statusText> (<status>)`
    /// without one) and whether it means the token was revoked (GHD
    /// `ghRequest`).
    ///
    /// With `sso_hint` (flag `327-api-saml-sso-hint`) a 403's message names
    /// the SSO authorization its `X-GitHub-SSO` header asks for.
    fn api_error(&self, url: &str, mut response: ureq::http::Response<ureq::Body>) -> GitHubError {
        let status = response.status();
        let headers = response.headers();
        let token_invalidated = status == ureq::http::StatusCode::UNAUTHORIZED
            && headers.contains_key("X-GitHub-Request-Id")
            && !headers.contains_key("X-GitHub-OTP");
        let hint = if self.sso_hint && status == ureq::http::StatusCode::FORBIDDEN {
            headers
                .get("x-github-sso")
                .and_then(|v| v.to_str().ok())
                .and_then(sso_hint)
        } else {
            None
        };
        let body = response.body_mut().read_to_vec().unwrap_or_default();
        let api_error = ApiErrorBody::parse(&body);
        let message = api_error
            .as_ref()
            .and_then(|e| e.message(self.error_details))
            .unwrap_or_else(|| {
                format!(
                    "API error {url}: {} ({})",
                    status.canonical_reason().unwrap_or_default(),
                    status.as_u16()
                )
            });
        let message = match hint {
            Some(hint) => format!("{}. {hint}", message.trim_end_matches('.')),
            None => message,
        };
        GitHubError::Api {
            status: status.as_u16(),
            message,
            api_error,
            token_invalidated,
        }
    }

    /// `GET` with a specific `Accept` header (the preview APIs); an error
    /// status is an `Err`.
    fn get_response(&self, path: &str, accept: &str) -> Result<ureq::http::Response<ureq::Body>> {
        let url = self.endpoint.api(path);
        debug!(%url, "GET");
        let mut request = self
            .agent
            .get(&url)
            .header("Accept", accept)
            .header("X-GitHub-Api-Version", "2022-11-28");
        // an empty token is GHD's `Account.anonymous()`: no Authorization
        if !self.token.is_empty() {
            request = request.header("Authorization", &format!("Bearer {}", self.token));
        }
        let response = request.call()?;
        if !response.status().is_success() {
            return Err(self.api_error(&url, response));
        }
        Ok(response)
    }

    /// `GET` with a specific `Accept` header (the preview APIs).
    fn get_json_accept<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        accept: &str,
    ) -> Result<T> {
        let mut response = self.get_response(path, accept)?;
        Ok(response.body_mut().read_json()?)
    }

    /// GHD `fetchAll` (with `suppressErrors: false`): `path`, then each page
    /// `next_page` finds in the previous response's headers, while
    /// `keep_going(&items)` holds (asked only when there is a next page) and
    /// at most `max_pages` pages. An error on any page fails the call.
    fn fetch_all<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        max_pages: usize,
        next_page: fn(&ResponseHeaders) -> Option<String>,
        mut keep_going: impl FnMut(&[T]) -> bool,
    ) -> Result<Vec<T>> {
        let mut items: Vec<T> = Vec::new();
        let mut next = Some(path.to_string());
        let mut pages = 0;
        while let Some(path) = next.take() {
            let mut response = self.get_response(&path, "application/vnd.github+json")?;
            let headers = header_pairs(response.headers());
            if let Some(page) = response.body_mut().read_json::<Option<Vec<T>>>()? {
                items.extend(page);
            }
            pages += 1;
            next = next_page(&headers).filter(|_| pages < max_pages && keep_going(&items));
        }
        Ok(items)
    }

    /// `GET /user` (+ `/user/emails` when the scope allows) → `Account`.
    pub fn current_user(&self, scopes: Vec<String>) -> Result<Account> {
        let user: ApiUser = self.get_json("user")?;
        let (emails, private_primary_email) = match self.get_json::<Vec<ApiEmail>>("user/emails") {
            Ok(list) => {
                let mut list: Vec<ApiEmail> = list.into_iter().filter(|e| e.verified).collect();
                list.sort_by_key(|e| !e.primary);
                // GHD `isEmailPublic`: no visibility (older Enterprise) is public
                let private = list
                    .first()
                    .is_some_and(|e| e.primary && e.visibility.as_deref() == Some("private"));
                (list.into_iter().map(|e| e.email).collect(), private)
            }
            Err(err) => {
                debug!(?err, "could not read emails");
                (Vec::new(), false)
            }
        };
        Ok(Account {
            endpoint: self.endpoint.api_base.clone(),
            id: user.id,
            login: user.login,
            name: user.name,
            avatar_url: user.avatar_url,
            emails,
            scopes,
            plan: user.plan.map(|p| p.name),
            private_primary_email,
        })
    }

    /// `GET` whose error answers mean "not available" rather than an error
    /// (`fetchCombinedRefStatus`, `fetchRefCheckRuns`…), except a revoked
    /// token, which the caller reports (GHD emits it from `ghRequest`).
    fn get_json_opt<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        accept: &str,
    ) -> Result<Option<T>> {
        match self.get_json_accept::<T>(path, accept) {
            Ok(value) => Ok(Some(value)),
            Err(err) if err.is_token_invalidated() => Err(err),
            Err(GitHubError::Api {
                status, message, ..
            }) => {
                debug!(status, %message, %path, "not available");
                Ok(None)
            }
            Err(err) => Err(err),
        }
    }

    /// `POST` without a body; `Ok(true)` for a 2xx answer.
    fn post_empty(&self, path: &str) -> Result<bool> {
        let url = self.endpoint.api(path);
        debug!(%url, "POST");
        let response = self
            .agent
            .post(&url)
            .header("Accept", "application/vnd.github+json")
            .header("Authorization", &format!("Bearer {}", self.token))
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send_empty()?;
        if response.status() == ureq::http::StatusCode::UNAUTHORIZED {
            return Err(self.api_error(&url, response));
        }
        Ok(response.status().is_success())
    }

    fn post_json<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<T> {
        let url = self.endpoint.api(path);
        debug!(%url, "POST");
        let mut response = self
            .agent
            .post(&url)
            .header("Accept", "application/vnd.github+json")
            .header("Authorization", &format!("Bearer {}", self.token))
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send_json(body)?;
        if !response.status().is_success() {
            return Err(self.api_error(&url, response));
        }
        Ok(response.body_mut().read_json()?)
    }

    /// `GET /orgs/{org}/teams` (flag `publish-team`): the organization's
    /// teams the account can see, as `(id, name)` sorted by name.
    pub fn org_teams(&self, org: &str) -> Result<Vec<(u64, String)>> {
        #[derive(Deserialize)]
        struct Team {
            id: u64,
            name: String,
        }
        let teams: Vec<Team> = self.get_json(&format!("orgs/{org}/teams?per_page=100"))?;
        let mut teams: Vec<(u64, String)> = teams.into_iter().map(|t| (t.id, t.name)).collect();
        teams.sort_by_key(|(_, name)| name.to_lowercase());
        Ok(teams)
    }

    /// `GET /user/orgs`: organisations the user can publish to.
    pub fn user_orgs(&self) -> Result<Vec<String>> {
        #[derive(Deserialize)]
        struct Org {
            login: String,
        }
        let orgs: Vec<Org> = self.get_json("user/orgs?per_page=100")?;
        let mut logins: Vec<String> = orgs.into_iter().map(|o| o.login).collect();
        logins.sort_by_key(|l| l.to_lowercase());
        Ok(logins)
    }

    /// `POST /user/repos` or `/orgs/{org}/repos` (GHD `createRepository`).
    ///
    /// `team_id` (flag `publish-team`, organizations only) grants that team
    /// access to the new repository.
    pub fn create_repository(
        &self,
        org: Option<&str>,
        name: &str,
        description: &str,
        private: bool,
        team_id: Option<u64>,
    ) -> Result<GitHubRepository> {
        let path = match org {
            Some(org) => format!("orgs/{org}/repos"),
            None => "user/repos".to_string(),
        };
        let mut body = serde_json::json!({
            "name": name,
            "description": description,
            "private": private,
        });
        if let (Some(team_id), Some(_)) = (team_id, org) {
            body["team_id"] = serde_json::json!(team_id);
        }
        let repo: ApiRepository = self
            .post_json(&path, &body)
            .map_err(|err| match (err, org) {
                // GHD replaces an organization's API error with a hint
                (
                    GitHubError::Api {
                        status,
                        api_error,
                        token_invalidated,
                        ..
                    },
                    Some(org),
                ) if !self.error_details => GitHubError::Api {
                    status,
                    message: format!(
                        "Unable to create repository for organization '{org}'. Verify that the \
                     repository does not already exist and that you have permission to create a \
                     repository there."
                    ),
                    api_error,
                    token_invalidated,
                },
                (err, _) => err,
            })?;
        Ok(self.convert(repo))
    }

    /// `GET /repos/{owner}/{name}`.
    pub fn repository(&self, owner: &str, name: &str) -> Result<GitHubRepository> {
        let repo: ApiRepository = self.get_json(&format!("repos/{owner}/{name}"))?;
        Ok(self.convert(repo))
    }

    /// `fetchPushedAt`: when anything was last pushed to `owner/name`
    /// (`pushed_at`, ISO 8601), `None` when GitHub does not say.
    pub fn pushed_at(&self, owner: &str, name: &str) -> Result<Option<String>> {
        let repo: ApiRepository = self.get_json(&format!("repos/{owner}/{name}"))?;
        Ok(repo.pushed_at)
    }

    /// `fetchRepositoryCloneInfo`: the clone URL (SSH when `ssh`) and default
    /// branch of `owner/name`, `None` when the repository is not found (404,
    /// which GitHub also answers for private repositories the token can't see).
    pub fn repository_clone_info(
        &self,
        owner: &str,
        name: &str,
        ssh: bool,
    ) -> Result<Option<RepositoryCloneInfo>> {
        let path = format!(
            "repos/{}/{}",
            encode_path_component(owner),
            encode_path_component(name)
        );
        match self.get_json::<ApiRepository>(&path) {
            Ok(repo) => Ok(Some(RepositoryCloneInfo {
                url: match (ssh, repo.ssh_url) {
                    (true, Some(ssh_url)) => ssh_url,
                    _ => repo.clone_url,
                },
                default_branch: repo.default_branch,
            })),
            Err(GitHubError::Api { status: 404, .. }) => Ok(None),
            Err(err) => Err(err),
        }
    }

    /// `fetchPullRequestReview`: `GET /repos/{owner}/{name}/pulls/{number}/reviews/{id}`.
    pub fn pull_request_review(
        &self,
        owner: &str,
        name: &str,
        number: u64,
        review_id: &str,
    ) -> Result<Option<ApiPullRequestReview>> {
        self.get_json_opt(
            &format!(
                "repos/{}/{}/pulls/{number}/reviews/{}",
                encode_path_component(owner),
                encode_path_component(name),
                encode_path_component(review_id)
            ),
            "application/vnd.github+json",
        )
    }

    /// `fetchIssueComment`: `GET /repos/{owner}/{name}/issues/comments/{id}`.
    pub fn issue_comment(
        &self,
        owner: &str,
        name: &str,
        comment_id: &str,
    ) -> Result<Option<ApiIssueComment>> {
        self.get_json_opt(
            &format!(
                "repos/{}/{}/issues/comments/{}",
                encode_path_component(owner),
                encode_path_component(name),
                encode_path_component(comment_id)
            ),
            "application/vnd.github+json",
        )
    }

    /// `fetchPullRequestReviewComment`: `GET /repos/{owner}/{name}/pulls/comments/{id}`.
    pub fn pull_request_review_comment(
        &self,
        owner: &str,
        name: &str,
        comment_id: &str,
    ) -> Result<Option<ApiIssueComment>> {
        self.get_json_opt(
            &format!(
                "repos/{}/{}/pulls/comments/{}",
                encode_path_component(owner),
                encode_path_component(name),
                encode_path_component(comment_id)
            ),
            "application/vnd.github+json",
        )
    }

    /// `getAliveDesktopChannel`: `GET /desktop_internal/alive-channel`;
    /// `None` when the endpoint has no Alive service.
    pub fn alive_desktop_channel(&self) -> Result<Option<crate::alive::AliveChannel>> {
        self.get_json_opt(
            "desktop_internal/alive-channel",
            "application/vnd.github+json",
        )
    }

    /// `getAliveWebSocketURL`: `GET /alive_internal/websocket-url`; `None`
    /// on 404 (Alive disabled for the endpoint).
    pub fn alive_websocket_url(&self) -> Result<Option<String>> {
        Ok(self
            .get_json_opt::<crate::alive::AliveWebSocket>(
                "alive_internal/websocket-url",
                "application/vnd.github+json",
            )?
            .map(|ws| ws.url))
    }

    /// `GET /repos/{owner}/{name}/releases/tags/{tag}`; `None` when there is
    /// no such release.
    pub fn release_by_tag(&self, owner: &str, name: &str, tag: &str) -> Result<Option<ApiRelease>> {
        let path = format!(
            "repos/{}/{}/releases/tags/{}",
            encode_path_component(owner),
            encode_path_component(name),
            encode_path_component(tag)
        );
        match self.get_json::<ApiRelease>(&path) {
            Ok(release) => Ok(Some(release)),
            Err(GitHubError::Api { status: 404, .. }) => Ok(None),
            Err(err) => Err(err),
        }
    }

    /// `GET /user/repos` (all pages, at most 20, newest pushed first) for
    /// the Clone dialog.
    pub fn user_repositories(&self) -> Result<Vec<GitHubRepository>> {
        let repos: Vec<ApiRepository> = self.fetch_all(
            "user/repos?per_page=100&sort=pushed&affiliation=owner,collaborator,organization_member",
            20,
            get_next_page_path_from_link,
            |_| true,
        )?;
        Ok(repos.into_iter().map(|r| self.convert(r)).collect())
    }

    /// `fetchIssues`: `GET /repos/{owner}/{name}/issues` (all pages). PRs are
    /// issues too, so anything carrying a `pull_request` key is dropped.
    /// `since` is an ISO-8601 timestamp; with it the API returns every issue
    /// updated at or after that moment (closed ones included, so callers can
    /// prune them).
    pub fn issues(
        &self,
        owner: &str,
        name: &str,
        state: IssueState,
        since: Option<&str>,
    ) -> Result<Vec<ApiIssue>> {
        let mut path = format!(
            "repos/{owner}/{name}/issues?state={}&per_page=100",
            state.as_str()
        );
        if let Some(since) = since {
            path.push_str("&since=");
            path.push_str(since);
        }
        let issues: Vec<ApiIssue> =
            self.fetch_all(&path, 20, get_next_page_path_from_link, |_| true)?;
        Ok(issues
            .into_iter()
            .filter(|i| i.pull_request.is_none())
            .collect())
    }

    /// `fetchUser`: `GET /users/{login}`; `None` when there is no such user.
    pub fn user(&self, login: &str) -> Result<Option<ApiIdentity>> {
        match self.get_json::<ApiIdentity>(&format!("users/{login}")) {
            Ok(user) => Ok(Some(user)),
            Err(GitHubError::Api { status: 404, .. }) => Ok(None),
            Err(err) => Err(err),
        }
    }

    /// `fetchMentionables`: `GET /repos/{owner}/{name}/mentionables/users`
    /// (preview API; needs its own `Accept`). `None` when the repository has
    /// no mentionables endpoint (404 for repositories the token can't see).
    pub fn mentionables(&self, owner: &str, name: &str) -> Result<Option<Vec<ApiMentionableUser>>> {
        let url = self
            .endpoint
            .api(&format!("repos/{owner}/{name}/mentionables/users"));
        debug!(%url, "GET");
        let mut response = self
            .agent
            .get(&url)
            .header("Accept", "application/vnd.github.jerry-maguire-preview")
            .header("Authorization", &format!("Bearer {}", self.token))
            .call()?;
        match response.status().as_u16() {
            404 => Ok(None),
            200..=299 => Ok(Some(response.body_mut().read_json()?)),
            _ => Err(self.api_error(&url, response)),
        }
    }

    /// `fetchPullRequest`: `GET /repos/{owner}/{name}/pulls/{number}`.
    pub fn pull_request(&self, owner: &str, name: &str, number: u64) -> Result<ApiPullRequest> {
        self.get_json(&format!("repos/{owner}/{name}/pulls/{number}"))
    }

    /// `fetchAllOpenPullRequests`: every open pull request (at most 50
    /// pages), newest page first.
    pub fn open_pull_requests(&self, owner: &str, name: &str) -> Result<Vec<ApiPullRequest>> {
        self.fetch_all(
            &format!("repos/{owner}/{name}/pulls?state=open&per_page=100"),
            50,
            get_next_page_path_from_link,
            |_| true,
        )
    }

    /// `fetchUpdatedPullRequests`: pull requests (open and closed) updated
    /// at or after `since` (ISO-8601), most recently updated first. Pages
    /// start at 10 pull requests and grow
    /// ([`get_next_page_path_with_increasing_page_size`]). `None` once
    /// `max_results` came back with more to read (`MaxResultsError`): the
    /// caller refetches the open list instead.
    pub fn pull_requests_updated_since(
        &self,
        owner: &str,
        name: &str,
        since: &str,
        max_results: usize,
    ) -> Result<Option<Vec<ApiPullRequest>>> {
        let mut over_max = false;
        let prs: Vec<ApiPullRequest> = self.fetch_all(
            &format!(
                "repos/{owner}/{name}/pulls?state=all&sort=updated&direction=desc&per_page=10"
            ),
            usize::MAX,
            get_next_page_path_with_increasing_page_size,
            |prs: &[ApiPullRequest]| {
                if prs.len() >= max_results {
                    over_max = true;
                    return false;
                }
                // sorted by `updated_at`, newest first: a last one updated
                // after `since` means there may be more
                prs.last()
                    .is_some_and(|last| last.updated_at.as_str() > since)
            },
        )?;
        if over_max {
            return Ok(None);
        }
        Ok(Some(
            prs.into_iter()
                .filter(|pr| pr.updated_at.as_str() >= since)
                .collect(),
        ))
    }

    /// GHD `fetchProtectedBranches`: `GET repos/{owner}/{name}/branches?protected=true`,
    /// `None` when the request fails or the server answers with an error.
    /// GHD feeds only a usage metric with it (`updateBranchProtectionsFromAPI`
    /// for `commitsToRepositoryWithBranchProtections`), which Corvene does
    /// not collect, so nothing calls it.
    pub fn fetch_protected_branches(&self, owner: &str, name: &str) -> Option<Vec<ApiBranch>> {
        match self.get_json(&format!("repos/{owner}/{name}/branches?protected=true")) {
            Ok(branches) => Some(branches),
            Err(err) => {
                debug!(%err, "[fetchProtectedBranches] unable to list protected branches");
                None
            }
        }
    }

    /// `fetchCombinedRefStatus`: `GET /repos/{o}/{n}/commits/{ref}/status`.
    pub fn combined_ref_status(
        &self,
        owner: &str,
        name: &str,
        git_ref: &str,
    ) -> Result<Option<ApiRefStatus>> {
        let safe = encode_path_component(git_ref);
        self.get_json_opt(
            &format!("repos/{owner}/{name}/commits/{safe}/status?per_page=100"),
            "application/vnd.github+json",
        )
    }

    /// `fetchRefCheckRuns`: `GET /repos/{o}/{n}/commits/{ref}/check-runs`.
    /// GHD reads the first 100 only; `all_pages` follows `page=` until
    /// `total_count` runs are read (at most 1,000, desktop/desktop#18101).
    pub fn ref_check_runs(
        &self,
        owner: &str,
        name: &str,
        git_ref: &str,
        all_pages: bool,
    ) -> Result<Option<ApiRefCheckRuns>> {
        let safe = encode_path_component(git_ref);
        let mut out: Option<ApiRefCheckRuns> = None;
        for page in 1..=10u32 {
            let mut path = format!("repos/{owner}/{name}/commits/{safe}/check-runs?per_page=100");
            if page > 1 {
                path.push_str(&format!("&page={page}"));
            }
            let batch: Option<ApiRefCheckRuns> =
                self.get_json_opt(&path, "application/vnd.github.antiope-preview+json")?;
            let Some(batch) = batch else {
                break;
            };
            let short = batch.check_runs.len() < 100;
            let runs = out.get_or_insert_with(|| ApiRefCheckRuns {
                total_count: batch.total_count,
                check_runs: Vec::new(),
            });
            runs.check_runs.extend(batch.check_runs);
            if !all_pages || short || runs.check_runs.len() as u64 >= runs.total_count {
                break;
            }
        }
        Ok(out)
    }

    /// `fetchPRActionWorkflowRunByCheckSuiteId`
    pub fn workflow_run_by_check_suite(
        &self,
        owner: &str,
        name: &str,
        check_suite_id: u64,
    ) -> Result<Option<ApiWorkflowRun>> {
        let runs: Option<ApiWorkflowRuns> = self.get_json_opt(
            &format!(
                "repos/{owner}/{name}/actions/runs?event=pull_request&check_suite_id={check_suite_id}"
            ),
            "application/vnd.github.antiope-preview+json",
        )?;
        Ok(runs.and_then(|r| r.workflow_runs.into_iter().next()))
    }

    /// `fetchPRWorkflowRunsByBranchName`
    pub fn workflow_runs_by_branch(
        &self,
        owner: &str,
        name: &str,
        branch: &str,
    ) -> Result<Option<ApiWorkflowRuns>> {
        let safe = encode_path_component(branch);
        self.get_json_opt(
            &format!("repos/{owner}/{name}/actions/runs?event=pull_request&branch={safe}"),
            "application/vnd.github.antiope-preview+json",
        )
    }

    /// `fetchWorkflowRunJobs`
    pub fn workflow_run_jobs(
        &self,
        owner: &str,
        name: &str,
        run_id: u64,
    ) -> Result<Option<ApiWorkflowJobs>> {
        self.get_json_opt(
            &format!("repos/{owner}/{name}/actions/runs/{run_id}/jobs"),
            "application/vnd.github.antiope-preview+json",
        )
    }

    /// `fetchCheckSuite`
    pub fn check_suite(
        &self,
        owner: &str,
        name: &str,
        check_suite_id: u64,
    ) -> Result<Option<ApiCheckSuite>> {
        self.get_json_opt(
            &format!("repos/{owner}/{name}/check-suites/{check_suite_id}"),
            "application/vnd.github+json",
        )
    }

    /// `rerequestCheckSuite`
    pub fn rerequest_check_suite(&self, owner: &str, name: &str, id: u64) -> Result<bool> {
        self.post_empty(&format!("repos/{owner}/{name}/check-suites/{id}/rerequest"))
    }

    /// `rerunJob`
    pub fn rerun_job(&self, owner: &str, name: &str, job_id: u64) -> Result<bool> {
        self.post_empty(&format!("repos/{owner}/{name}/actions/jobs/{job_id}/rerun"))
    }

    /// `rerunFailedJobs`
    pub fn rerun_failed_jobs(&self, owner: &str, name: &str, run_id: u64) -> Result<bool> {
        self.post_empty(&format!(
            "repos/{owner}/{name}/actions/runs/{run_id}/rerun-failed-jobs"
        ))
    }

    /// `forkRepository`: `POST /repos/{o}/{n}/forks` (202 with the fork).
    pub fn fork_repository(&self, owner: &str, name: &str) -> Result<GitHubRepository> {
        let repo: ApiRepository = self.post_json(
            &format!("repos/{owner}/{name}/forks"),
            &serde_json::json!({}),
        )?;
        Ok(self.convert(repo))
    }

    /// `fetchPushControl`: whether the branch takes direct pushes. The
    /// defaults (pushable) come back when the endpoint has no answer.
    pub fn push_control(&self, owner: &str, name: &str, branch: &str) -> Result<ApiPushControl> {
        let safe = encode_path_component(branch);
        Ok(self
            .get_json_opt(
                &format!("repos/{owner}/{name}/branches/{safe}/push_control"),
                "application/vnd.github.phandalin-preview",
            )?
            .unwrap_or_default())
    }

    /// `fetchAllRepoRulesets`
    pub fn repo_rulesets(
        &self,
        owner: &str,
        name: &str,
    ) -> Result<Option<Vec<ApiSlimRepoRuleset>>> {
        self.get_json_opt(
            &format!("repos/{owner}/{name}/rulesets"),
            "application/vnd.github+json",
        )
    }

    /// `fetchRepoRuleset`
    pub fn repo_ruleset(&self, owner: &str, name: &str, id: u64) -> Result<Option<ApiRepoRuleset>> {
        self.get_json_opt(
            &format!("repos/{owner}/{name}/rulesets/{id}"),
            "application/vnd.github+json",
        )
    }

    /// `fetchRepoRulesForBranch`
    pub fn repo_rules_for_branch(
        &self,
        owner: &str,
        name: &str,
        branch: &str,
    ) -> Result<Vec<ApiRepoRule>> {
        let safe = encode_path_component(branch);
        Ok(self
            .get_json_opt(
                &format!("repos/{owner}/{name}/rules/branches/{safe}"),
                "application/vnd.github+json",
            )?
            .unwrap_or_default())
    }

    /// `createPushProtectionBypass`
    pub fn create_push_protection_bypass(
        &self,
        owner: &str,
        name: &str,
        reason: BypassReason,
        placeholder_id: &str,
    ) -> Result<ApiPushProtectionBypass> {
        self.post_json(
            &format!("repos/{owner}/{name}/secret-scanning/push-protection-bypasses"),
            &serde_json::json!({
                "reason": reason.as_str(),
                "placeholder_id": placeholder_id,
            }),
        )
    }

    /// The API repository → model conversion (endpoint-aware).
    pub fn convert(&self, repo: ApiRepository) -> GitHubRepository {
        GitHubRepository {
            endpoint: self.endpoint.api_base.clone(),
            owner: repo.owner.login,
            name: repo.name,
            html_url: repo.html_url,
            clone_url: repo.clone_url,
            default_branch: repo.default_branch,
            private: repo.private,
            fork: repo.fork,
            parent: repo.parent.map(|p| Box::new(self.convert(*p))),
            archived: repo.archived,
            permissions: repo.permissions.and_then(|p| p.permission()),
            allow_forking: repo.allow_forking,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sso_hints() {
        assert_eq!(
            sso_hint("required; url=https://github.com/orgs/acme/sso?authorization_request=AB12")
                .as_deref(),
            Some(
                "Re-authorize SSO for acme: \
                 https://github.com/orgs/acme/sso?authorization_request=AB12"
            )
        );
        assert_eq!(
            sso_hint("required; url=https://ghe.corp/sso/start").as_deref(),
            Some("Re-authorize SSO: https://ghe.corp/sso/start")
        );
        assert_eq!(
            sso_hint("partial-results; organizations=21955855,20582480"),
            None
        );
        assert_eq!(sso_hint("required"), None);
    }

    #[test]
    fn error_details_follow_the_message() {
        let body = r#"{"message":"Repository creation failed.","errors":[{"resource":"Repository","code":"custom","field":"description","message":"description is too long (maximum is 350 characters)"},{"resource":"Repository","code":"invalid","field":"name"}]}"#;
        let parsed = ApiErrorBody::parse(body.as_bytes()).unwrap();
        // GHD `APIError`: `errors.map(e => e.message).join(', ')`
        assert_eq!(
            parsed.message(false).as_deref(),
            Some(
                "Repository creation failed. (description is too long (maximum is 350 \
                 characters), )"
            )
        );
        assert_eq!(
            parsed.message(true).as_deref(),
            Some(
                "Repository creation failed. (description is too long (maximum is 350 \
                 characters), name invalid)"
            )
        );
        let bare = ApiErrorBody::parse(br#"{"message":"Not Found"}"#).unwrap();
        assert_eq!(bare.message(true).as_deref(), Some("Not Found"));
        assert_eq!(bare.message(false).as_deref(), Some("Not Found"));
        let only_errors =
            ApiErrorBody::parse(br#"{"errors":[{"field":"name","code":"missing"}]}"#).unwrap();
        assert_eq!(only_errors.message(false), None);
        assert_eq!(only_errors.message(true).as_deref(), Some("name missing"));
        assert_eq!(ApiErrorBody::parse(b""), None);
        assert_eq!(ApiErrorBody::parse(b"null"), None);
        assert_eq!(
            ApiErrorBody::parse(br#"{"message":"x","errors":"odd"}"#),
            Some(ApiErrorBody {
                message: Some("x".into()),
                errors: Vec::new(),
            })
        );
    }

    #[test]
    fn next_page_links() {
        let headers = |link: &str| vec![("link".to_string(), link.to_string())];
        assert_eq!(
            get_next_page_path_from_link(&headers(
                "<https://api.github.com/repositories/1/pulls?per_page=10&page=2>; rel=\"next\", \
                 <https://api.github.com/repositories/1/pulls?per_page=10&page=9>; rel=\"last\""
            ))
            .as_deref(),
            Some("/repositories/1/pulls?per_page=10&page=2")
        );
        assert_eq!(
            get_next_page_path_from_link(&headers(
                "<https://ghe.corp/api/v3/user/repos?page=1>; rel=\"prev\""
            )),
            None
        );
        assert_eq!(
            get_next_page_path_with_increasing_page_size(&headers(
                "<https://ghe.corp/api/v3/repos/o/n/pulls?state=all&per_page=40&page=3>; rel=\"next\""
            ))
            .as_deref(),
            Some("/api/v3/repos/o/n/pulls?state=all&per_page=80&page=2")
        );
        assert_eq!(parse_int(" 12abc"), Some(12));
        assert_eq!(parse_int("x"), None);
    }

    #[test]
    fn pull_request_review_deserializes() {
        let json = r#"{
            "id": 80,
            "node_id": "MDE3OlB1bGxSZXF1ZXN0UmV2aWV3ODA=",
            "user": {
                "login": "octocat",
                "id": 1,
                "avatar_url": "https://github.com/images/error/octocat_happy.gif",
                "html_url": "https://github.com/octocat",
                "type": "User"
            },
            "body": "Here is the body for the review.",
            "state": "CHANGES_REQUESTED",
            "html_url": "https://github.com/octocat/Hello-World/pull/12#pullrequestreview-80",
            "submitted_at": "2019-11-17T17:43:43Z",
            "commit_id": "ecdd80bb57125d7ba9641ffaa4d7d2c19d3f3091"
        }"#;
        let review: ApiPullRequestReview = serde_json::from_str(json).expect("review");
        assert_eq!(review.id, 80);
        assert_eq!(review.state, ApiPullRequestReviewState::ChangesRequested);
        assert_eq!(review.user.login, "octocat");
        assert_eq!(
            review.user.html_url.as_deref(),
            Some("https://github.com/octocat")
        );
        assert_eq!(review.user.kind.as_deref(), Some("User"));
        assert_eq!(review.submitted_at, "2019-11-17T17:43:43Z");

        let approved = json
            .replace("CHANGES_REQUESTED", "APPROVED")
            .replace(r#""Here is the body for the review.""#, "null");
        let review: ApiPullRequestReview = serde_json::from_str(&approved).expect("approved");
        assert_eq!(review.state, ApiPullRequestReviewState::Approved);
        assert_eq!(review.body, "");
        for (raw, state) in [
            ("COMMENTED", ApiPullRequestReviewState::Commented),
            ("DISMISSED", ApiPullRequestReviewState::Dismissed),
            ("PENDING", ApiPullRequestReviewState::Pending),
        ] {
            let parsed: ApiPullRequestReviewState =
                serde_json::from_str(&format!("\"{raw}\"")).expect("state");
            assert_eq!(parsed, state);
        }
    }

    #[test]
    fn repository_permissions_map_like_get_permissions_string() {
        let perms = |json: &str| -> Option<RepositoryPermission> {
            serde_json::from_str::<ApiRepositoryPermissions>(json)
                .expect("permissions")
                .permission()
        };
        assert_eq!(
            perms(r#"{"admin":true,"push":true,"pull":true}"#),
            Some(RepositoryPermission::Admin)
        );
        assert_eq!(
            perms(r#"{"admin":false,"push":true,"pull":true,"maintain":true}"#),
            Some(RepositoryPermission::Write)
        );
        assert_eq!(
            perms(r#"{"admin":false,"push":false,"pull":true}"#),
            Some(RepositoryPermission::Read)
        );
        assert_eq!(perms(r#"{}"#), None);

        let repo: ApiRepository = serde_json::from_str(
            r#"{"name":"desktop","owner":{"login":"desktop"},"html_url":"https://github.com/desktop/desktop",
                "clone_url":"https://github.com/desktop/desktop.git","default_branch":"development",
                "permissions":{"admin":false,"push":false,"pull":true}}"#,
        )
        .expect("repository");
        let client = Client::new(crate::Endpoint::github_com(), "");
        let converted = client.convert(repo);
        assert_eq!(converted.permissions, Some(RepositoryPermission::Read));
        assert!(!converted.has_write_permission());
    }

    #[test]
    fn issue_comment_deserializes() {
        let json = r#"{
            "id": 1,
            "node_id": "MDEyOklzc3VlQ29tbWVudDE=",
            "url": "https://api.github.com/repos/octocat/Hello-World/issues/comments/1",
            "html_url": "https://github.com/octocat/Hello-World/issues/1347#issuecomment-1",
            "body": "Me too",
            "user": { "login": "octocat", "id": 1, "avatar_url": null, "html_url": "https://github.com/octocat", "type": "User" },
            "created_at": "2011-04-14T16:00:49Z",
            "updated_at": "2011-04-14T16:00:49Z",
            "author_association": "COLLABORATOR"
        }"#;
        let comment: ApiIssueComment = serde_json::from_str(json).expect("comment");
        assert_eq!(comment.id, 1);
        assert_eq!(comment.body, "Me too");
        assert_eq!(comment.user.login, "octocat");
        assert_eq!(comment.user.avatar_url, None);
        assert_eq!(comment.created_at, "2011-04-14T16:00:49Z");
        assert!(comment.html_url.ends_with("#issuecomment-1"));
    }
}
