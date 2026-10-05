//! Host providers: one trait over the hosting services Corvene talks to,
//! with GitLab, Gitea / Forgejo / Codeberg and Bitbucket Cloud
//! implementations and an adapter over `corvene_github::Client`.
//! Corvene addition (flags `342-gitlab`, `343-gitea`, `344-bitbucket`);
//! the design is `.docs/hosts.md`.
//!
//! Every call blocks on the network: run them on a background thread. The
//! data comes back in Corvene's GitHub-shaped models (`GitHubRepository`,
//! `PullRequest`, `RefCheck`), so the pull request list, the toolbar badge
//! and the checks popover draw any host unchanged.

pub mod bitbucket;
pub mod gitea;
pub mod github;
pub mod gitlab;
pub mod http;
pub mod oauth;
pub mod urls;

use corvene_models::{
    GitHubRepository, HostAccount, HostAuthKind, HostEndpoint, HostKind, PullRequest, RefCheck,
};

pub use http::Auth;

/// What a host call can fail with.
#[derive(Debug, thiserror::Error)]
pub enum HostError {
    #[error("network error: {0}")]
    Http(#[from] ureq::Error),
    #[error("unexpected response: {0}")]
    Json(#[from] serde_json::Error),
    /// The server answered with an error status; `message` is its own.
    #[error("{message}")]
    Api { status: u16, message: String },
    #[error("{0}")]
    Auth(String),
    #[error("GitHub: {0}")]
    GitHub(#[from] corvene_github::GitHubError),
}

impl HostError {
    pub fn status(&self) -> Option<u16> {
        match self {
            HostError::Api { status, .. } => Some(*status),
            HostError::GitHub(err) => err.status(),
            _ => None,
        }
    }

    /// Not shown to this caller (401, 403 or 404): an anonymous request
    /// for something the project keeps to its members.
    pub fn is_hidden(&self) -> bool {
        matches!(self.status(), Some(401 | 403 | 404))
    }

    /// The token was refused (revoked, expired, wrong): the account has to
    /// sign in again.
    pub fn is_unauthorized(&self) -> bool {
        match self {
            HostError::GitHub(err) => err.is_token_invalidated(),
            _ => self.status() == Some(401),
        }
    }
}

pub type Result<T> = std::result::Result<T, HostError>;

/// A pull request to open (`create_pull_request`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NewPullRequest {
    pub title: String,
    pub body: String,
    /// The branch with the changes.
    pub head: String,
    /// The branch they go into.
    pub base: String,
    /// The repository `head` is on when it is a fork of the target.
    pub head_repository: Option<GitHubRepository>,
    pub draft: bool,
}

/// One hosting service's REST API, as Corvene uses it.
pub trait HostProvider: Send + Sync {
    fn endpoint(&self) -> &HostEndpoint;

    fn kind(&self) -> HostKind {
        self.endpoint().kind
    }

    /// The signed-in user (`auth` fields left at their defaults: the
    /// sign-in flow fills them).
    fn current_user(&self) -> Result<HostAccount>;

    /// One repository by owner (namespace) and name, with its default
    /// branch, visibility, fork parent, archived state and the user's
    /// permission.
    fn repository(&self, owner: &str, name: &str) -> Result<GitHubRepository>;

    /// The repositories the user is a member of (clone lists).
    fn user_repositories(&self) -> Result<Vec<GitHubRepository>>;

    /// The open pull (merge) requests of `repo`, newest number first.
    fn open_pull_requests(&self, repo: &GitHubRepository) -> Result<Vec<PullRequest>>;

    /// Open a pull request on `repo` through the API.
    fn create_pull_request(
        &self,
        repo: &GitHubRepository,
        new: &NewPullRequest,
    ) -> Result<PullRequest>;

    /// The CI results of a commit (`None` when the host has none for it or
    /// does not show them to this account).
    fn ref_checks(
        &self,
        repo: &GitHubRepository,
        target: &CheckTarget,
    ) -> Result<Option<Vec<RefCheck>>>;
}

/// Whose CI results [`HostProvider::ref_checks`] reads.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CheckTarget {
    /// The commit.
    pub sha: String,
    /// The pull request it heads: GitLab runs merge request pipelines on
    /// the merged result, which another commit names.
    pub pull_request: Option<u64>,
    /// The branch it is the tip of (GitLab's pipelines by ref).
    pub branch: Option<String>,
}

impl CheckTarget {
    pub fn commit(sha: impl Into<String>) -> Self {
        Self {
            sha: sha.into(),
            ..Self::default()
        }
    }
}

/// The provider for `endpoint`, authenticating with `token` (empty: anonymous).
/// `basic_user` is Bitbucket's Atlassian email for API tokens.
pub fn provider(
    endpoint: &HostEndpoint,
    token: &str,
    basic_user: Option<&str>,
) -> Box<dyn HostProvider> {
    match endpoint.kind {
        HostKind::GitHub => Box::new(github::GitHub::new(endpoint.clone(), token)),
        HostKind::GitLab => Box::new(gitlab::GitLab::new(endpoint.clone(), token)),
        HostKind::Gitea => Box::new(gitea::Gitea::new(endpoint.clone(), token)),
        HostKind::Bitbucket => Box::new(bitbucket::Bitbucket::new(
            endpoint.clone(),
            token,
            basic_user,
        )),
    }
}

/// The provider for a signed-in account and its token.
pub fn provider_for(account: &HostAccount, token: &str) -> Box<dyn HostProvider> {
    match (account.auth, account.kind()) {
        (HostAuthKind::OAuth, HostKind::Gitea) => {
            Box::new(gitea::Gitea::new_bearer(account.endpoint.clone(), token))
        }
        (HostAuthKind::OAuth, _) => provider(&account.endpoint, token, None),
        (HostAuthKind::Token, _) => {
            provider(&account.endpoint, token, account.basic_user.as_deref())
        }
    }
}

/// An empty [`GitHubRepository`] for `endpoint` (filled by the providers).
pub(crate) fn blank_repository(endpoint: &HostEndpoint) -> GitHubRepository {
    GitHubRepository {
        endpoint: endpoint.api_base.clone(),
        owner: String::new(),
        name: String::new(),
        html_url: String::new(),
        clone_url: String::new(),
        default_branch: None,
        private: false,
        fork: false,
        parent: None,
        archived: false,
        permissions: None,
        allow_forking: None,
        node_id: None,
    }
}

/// `owner/name` split at the last `/` (GitLab namespaces nest).
pub(crate) fn split_path(path: &str) -> (String, String) {
    match path.rsplit_once('/') {
        Some((owner, name)) => (owner.to_string(), name.to_string()),
        None => (String::new(), path.to_string()),
    }
}
