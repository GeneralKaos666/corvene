//! Hosts other than GitHub (Corvene addition, flags `342-gitlab`,
//! `343-gitea`, `344-bitbucket`; design in `.docs/hosts.md`). GitHub
//! Desktop only knows GitHub.com and GitHub Enterprise.
//!
//! The GitHub code never sees these types: host accounts live in their own
//! list (`AppState::host_accounts`) and a repository's hosted counterpart is
//! worked out from its remote (`hosted_from_remote`), so GitHub's
//! `Account` / `Repository::github` paths stay as they are. The repository
//! itself reuses [`GitHubRepository`] (API base, owner, name, URLs, default
//! branch, fork parent, permission): a GitLab subgroup project has an owner
//! with `/` in it (`group/subgroup`).

use serde::{Deserialize, Serialize};

use crate::GitHubRepository;

/// Which kind of server a host is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum HostKind {
    #[serde(rename = "github")]
    GitHub,
    #[serde(rename = "gitlab")]
    GitLab,
    /// Gitea and its fork Forgejo (Codeberg runs Forgejo): one API.
    #[serde(rename = "gitea")]
    Gitea,
    /// Bitbucket Cloud (Data Center has another API and is not supported).
    #[serde(rename = "bitbucket")]
    Bitbucket,
}

impl HostKind {
    /// The kinds other than GitHub, in the order Settings lists them.
    pub const OTHERS: [HostKind; 3] = [HostKind::GitLab, HostKind::Gitea, HostKind::Bitbucket];

    /// "GitLab", for "View on GitLab" and "Sign Into GitLab".
    pub fn name(self) -> &'static str {
        match self {
            HostKind::GitHub => "GitHub",
            HostKind::GitLab => "GitLab",
            HostKind::Gitea => "Gitea",
            HostKind::Bitbucket => "Bitbucket",
        }
    }

    /// The site's name for "View on …": Codeberg for codeberg.org, else
    /// the kind's.
    pub fn site_name(self, api_base: &str) -> &'static str {
        if self == HostKind::Gitea && api_base.to_ascii_lowercase().contains("://codeberg.org/") {
            "Codeberg"
        } else {
            self.name()
        }
    }

    /// The Settings › Accounts section heading.
    pub fn section_title(self) -> &'static str {
        match self {
            HostKind::Gitea => "Gitea and Forgejo",
            other => other.name(),
        }
    }

    /// The server a sign-in starts from (`https://gitlab.com`).
    pub fn default_web_base(self) -> &'static str {
        match self {
            HostKind::GitHub => "https://github.com",
            HostKind::GitLab => "https://gitlab.com",
            HostKind::Gitea => "https://codeberg.org",
            HostKind::Bitbucket => "https://bitbucket.org",
        }
    }

    /// GitLab says merge request, the others pull request.
    pub fn is_merge_request(self) -> bool {
        self == HostKind::GitLab
    }

    /// "Pull Request" / "Merge Request" (title case, menus and buttons).
    pub fn pull_request_title(self) -> &'static str {
        if self.is_merge_request() {
            "Merge Request"
        } else {
            "Pull Request"
        }
    }

    /// "pull request" / "merge request" (sentence case).
    pub fn pull_request_noun(self) -> &'static str {
        if self.is_merge_request() {
            "merge request"
        } else {
            "pull request"
        }
    }

    /// The sign before a pull request number: `#12`, GitLab's `!12`.
    pub fn number_prefix(self) -> char {
        if self.is_merge_request() { '!' } else { '#' }
    }

    /// The git ref a host exposes for a pull request's head (`None` for
    /// Bitbucket, which has none).
    pub fn pull_request_ref(self, number: u64) -> Option<String> {
        match self {
            HostKind::GitHub | HostKind::Gitea => Some(format!("refs/pull/{number}/head")),
            HostKind::GitLab => Some(format!("refs/merge-requests/{number}/head")),
            HostKind::Bitbucket => None,
        }
    }

    /// The kind of an API base as Corvene builds them: `…/api/v4` GitLab,
    /// `…/api/v1` Gitea, `api.bitbucket.org` Bitbucket, anything else
    /// (`api.github.com`, `…/api/v3`) GitHub.
    pub fn of_api_base(api_base: &str) -> HostKind {
        let base = api_base.trim_end_matches('/');
        if base.ends_with("/api/v4") {
            HostKind::GitLab
        } else if base.ends_with("/api/v1") {
            HostKind::Gitea
        } else if base.ends_with("/2.0")
            && (base.contains("://api.bitbucket.org") || base.contains("://127.0.0.1"))
        {
            HostKind::Bitbucket
        } else {
            HostKind::GitHub
        }
    }

    /// The path of the REST API under the web address, `None` when the API
    /// has a host of its own (Bitbucket Cloud, GitHub.com).
    fn api_path(self) -> &'static str {
        match self {
            HostKind::GitHub => "/api/v3",
            HostKind::GitLab => "/api/v4",
            HostKind::Gitea => "/api/v1",
            HostKind::Bitbucket => "",
        }
    }
}

/// Why a server address was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum HostAddressError {
    #[error(
        "The server address doesn't appear to be a valid URL. We're expecting something like https://gitlab.example.com."
    )]
    InvalidUrl,
    #[error("Unsupported protocol. Only https is supported when signing in.")]
    InvalidProtocol,
    #[error(
        "Only Bitbucket Cloud (bitbucket.org) is supported. Bitbucket Data Center has another API."
    )]
    BitbucketServer,
}

/// Where one host's web pages and REST API are.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HostEndpoint {
    pub kind: HostKind,
    /// `https://gitlab.com`, `https://git.example.com/gitea` (no trailing `/`).
    pub web_base: String,
    /// `https://gitlab.com/api/v4`, `https://api.bitbucket.org/2.0`.
    pub api_base: String,
}

impl HostEndpoint {
    /// The public server of a kind (gitlab.com, codeberg.org, bitbucket.org).
    pub fn public(kind: HostKind) -> Self {
        Self::from_web_base(kind, kind.default_web_base())
    }

    fn from_web_base(kind: HostKind, web_base: &str) -> Self {
        let web_base = web_base.trim_end_matches('/').to_string();
        let api_base = match kind {
            HostKind::Bitbucket => "https://api.bitbucket.org/2.0".to_string(),
            HostKind::GitHub if web_base.eq_ignore_ascii_case("https://github.com") => {
                "https://api.github.com".to_string()
            }
            _ => format!("{web_base}{}", kind.api_path()),
        };
        Self {
            kind,
            web_base,
            api_base,
        }
    }

    /// A server address as typed in the sign-in dialog (`gitlab.example.com`,
    /// `https://git.example.com/gitea/`). No scheme means `https`; `http`
    /// is kept only with `allow_http` (flag `314-enterprise-plain-http`) or
    /// for a loopback host. A path is kept (Gitea under a sub-path), a query
    /// or fragment dropped.
    pub fn parse(kind: HostKind, input: &str, allow_http: bool) -> Result<Self, HostAddressError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(HostAddressError::InvalidUrl);
        }
        let (scheme, rest) = match trimmed.split_once("://") {
            Some((scheme, rest)) => (scheme.to_ascii_lowercase(), rest),
            None => ("https".to_string(), trimmed),
        };
        let rest = rest.split(['?', '#']).next().unwrap_or_default();
        let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
        let host = authority.rsplit('@').next().unwrap_or(authority);
        let host_name = host.split(':').next().unwrap_or(host);
        if host_name.is_empty()
            || host.contains(char::is_whitespace)
            || !host_name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '[' | ']'))
        {
            return Err(HostAddressError::InvalidUrl);
        }
        let loopback = matches!(
            host_name.to_ascii_lowercase().as_str(),
            "localhost" | "127.0.0.1" | "[::1]"
        );
        match scheme.as_str() {
            "https" => {}
            "http" if allow_http || loopback => {}
            s if !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '+') => {
                return Err(HostAddressError::InvalidProtocol);
            }
            _ => return Err(HostAddressError::InvalidUrl),
        }
        if kind == HostKind::Bitbucket {
            let lower = host_name.to_ascii_lowercase();
            if lower == "bitbucket.org"
                || lower == "api.bitbucket.org"
                || lower == "www.bitbucket.org"
            {
                return Ok(Self::public(HostKind::Bitbucket));
            }
            if !loopback {
                return Err(HostAddressError::BitbucketServer);
            }
            // a local stand-in for the Cloud API (tests)
            let base = format!("{scheme}://{host}");
            return Ok(Self {
                kind,
                web_base: base.clone(),
                api_base: format!("{base}/2.0"),
            });
        }
        let path = path.trim_matches('/');
        // a pasted API address (`…/api/v4`) is the server's
        let path = path
            .strip_suffix(kind.api_path().trim_start_matches('/'))
            .unwrap_or(path)
            .trim_end_matches('/');
        let web_base = if path.is_empty() {
            format!("{scheme}://{}", host.to_ascii_lowercase())
        } else {
            format!("{scheme}://{}/{path}", host.to_ascii_lowercase())
        };
        Ok(Self::from_web_base(kind, &web_base))
    }

    /// The web host (`gitlab.com`), without port.
    pub fn host(&self) -> &str {
        let rest = self
            .web_base
            .split_once("://")
            .map_or(self.web_base.as_str(), |(_, r)| r);
        let authority = rest.split('/').next().unwrap_or(rest);
        authority.split(':').next().unwrap_or(authority)
    }

    /// `scheme://host[:port]` of the web base.
    pub fn origin(&self) -> &str {
        let start = self.web_base.find("://").map_or(0, |i| i + 3);
        match self.web_base[start..].find('/') {
            Some(i) => &self.web_base[..start + i],
            None => &self.web_base,
        }
    }

    /// The path under the host the server is mounted at (`gitea` for
    /// `https://example.com/gitea`), empty for most.
    pub fn base_path(&self) -> &str {
        let rest = self
            .web_base
            .split_once("://")
            .map_or(self.web_base.as_str(), |(_, r)| r);
        rest.split_once('/')
            .map_or("", |(_, p)| p.trim_matches('/'))
    }

    /// One of the public servers (no base URL needed).
    pub fn is_public(&self) -> bool {
        *self == Self::public(self.kind)
    }

    /// "GitLab.com", "Codeberg", "Bitbucket", else the host.
    pub fn friendly_name(&self) -> String {
        match self.host().to_ascii_lowercase().as_str() {
            "gitlab.com" => "GitLab.com".into(),
            "codeberg.org" => "Codeberg".into(),
            "bitbucket.org" => "Bitbucket".into(),
            "gitea.com" => "Gitea.com".into(),
            _ => self.host().to_string(),
        }
    }

    /// `path` under the web base (one `/` between).
    pub fn web(&self, path: &str) -> String {
        format!("{}/{}", self.web_base, path.trim_start_matches('/'))
    }

    /// `path` under the API base (one `/` between); an absolute URL (a
    /// pagination link) is returned as it is.
    pub fn api(&self, path: &str) -> String {
        if path.starts_with("https://") || path.starts_with("http://") {
            return path.to_string();
        }
        format!("{}/{}", self.api_base, path.trim_start_matches('/'))
    }
}

/// How a host account's token was obtained.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HostAuthKind {
    /// A personal access token (GitLab, Gitea) or Atlassian API token
    /// (Bitbucket) pasted into the sign-in dialog.
    #[default]
    Token,
    /// The browser OAuth flow; the access token expires and is refreshed.
    OAuth,
}

/// A signed-in account on a host other than GitHub. As for [`crate::Account`],
/// the token lives in the OS keychain, never in the store: under
/// ([`HostAccount::keychain_host`], [`HostAccount::git_username`]) so git's
/// askpass finds it for HTTPS remotes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostAccount {
    pub endpoint: HostEndpoint,
    /// The user's id on the host (Bitbucket's is a `{uuid}`).
    pub id: String,
    pub login: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub emails: Vec<String>,
    #[serde(default)]
    pub auth: HostAuthKind,
    /// Bitbucket's API tokens authenticate as `email:token`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub basic_user: Option<String>,
    /// OAuth: the application the token was issued to (refreshing needs it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oauth_client_id: Option<String>,
    /// OAuth: when the access token expires (Unix seconds).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
    /// The last API call answered 401: Settings offers to sign in again.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub needs_reauth: bool,
}

impl HostAccount {
    pub fn kind(&self) -> HostKind {
        self.endpoint.kind
    }

    /// The user name git sends with the token over HTTPS: `oauth2` for a
    /// GitLab OAuth token, Bitbucket's fixed `x-bitbucket-api-token-auth`,
    /// else the login.
    pub fn git_username(&self) -> String {
        match (self.kind(), self.auth) {
            (HostKind::GitLab, HostAuthKind::OAuth) => "oauth2".into(),
            (HostKind::Bitbucket, HostAuthKind::Token) => "x-bitbucket-api-token-auth".into(),
            (HostKind::Bitbucket, HostAuthKind::OAuth) => "x-token-auth".into(),
            _ => self.login.clone(),
        }
    }

    /// The keychain item's host: the web host, as git names it in prompts.
    pub fn keychain_host(&self) -> String {
        self.endpoint.host().to_ascii_lowercase()
    }

    /// The keychain user of the OAuth refresh token.
    pub fn refresh_token_user(&self) -> String {
        format!("refresh:{}", self.login)
    }

    /// `@login (Name)` for the Settings row.
    pub fn title(&self) -> String {
        match &self.name {
            Some(name) if !name.is_empty() && name != &self.login => {
                format!("@{} ({name})", self.login)
            }
            _ => format!("@{}", self.login),
        }
    }
}

impl crate::PullRequest {
    /// The host of the pull request's base repository.
    pub fn host_kind(&self) -> HostKind {
        self.base
            .repository
            .as_ref()
            .map_or(HostKind::GitHub, |r| HostKind::of_api_base(&r.endpoint))
    }

    /// "GitLab", "Codeberg"…: where the pull request's page is.
    pub fn host_site_name(&self) -> &'static str {
        let api_base = self
            .base
            .repository
            .as_ref()
            .map_or("", |r| r.endpoint.as_str());
        self.host_kind().site_name(api_base)
    }

    /// `#12`, or `!12` for a GitLab merge request.
    pub fn number_label(&self) -> String {
        format!("{}{}", self.host_kind().number_prefix(), self.number)
    }
}

/// A repository on a host other than GitHub.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostedRepository {
    pub kind: HostKind,
    /// `endpoint` is the API base, `owner` the namespace (GitLab groups and
    /// subgroups joined with `/`), `name` the last path segment.
    pub repo: GitHubRepository,
}

impl HostedRepository {
    /// `group/subgroup/project`
    pub fn path(&self) -> String {
        self.repo.full_name()
    }

    /// The repository pull requests go to: the fork parent, unless the
    /// fork is set up for its own work.
    pub fn non_fork(&self, contribute_to_parent: bool) -> &GitHubRepository {
        match (&self.repo.parent, contribute_to_parent) {
            (Some(parent), true) => parent,
            _ => &self.repo,
        }
    }
}

/// `(host, path)` of a remote URL: [`crate::split_remote`] plus scp-style
/// URLs with any user (`gitea@git.example.com:o/n`, `org-123@bitbucket.org`).
fn split_any_remote(url: &str) -> Option<(String, String)> {
    if let Some(split) = crate::split_remote(url) {
        return Some(split);
    }
    let url = url.trim();
    if url.contains("://") {
        return None;
    }
    let (prefix, path) = url.split_once(':')?;
    let (user, host) = prefix.rsplit_once('@')?;
    (!user.is_empty() && !host.is_empty() && !prefix.contains('/'))
        .then(|| (host.to_string(), path.to_string()))
}

/// The hosted repository a remote URL points at, when its host is one of
/// `endpoints`' web hosts. HTTPS paths drop the server's base path
/// (`https://example.com/gitea/o/n`). GitLab takes any depth of groups
/// (`group/sub/project`), the others `owner/name` only.
pub fn hosted_from_remote(url: &str, endpoints: &[HostEndpoint]) -> Option<HostedRepository> {
    let (host, path) = split_any_remote(url)?;
    let endpoint = endpoints
        .iter()
        .find(|e| e.host().eq_ignore_ascii_case(&host))?;
    let mut path = path.trim_matches('/');
    let base = endpoint.base_path();
    if !base.is_empty() && url.contains("://") {
        // an HTTPS remote carries the base path, an SSH one does not
        if let Some(rest) = path.strip_prefix(base) {
            path = rest.trim_start_matches('/');
        }
    }
    let path = path.strip_suffix(".git").unwrap_or(path).trim_matches('/');
    let path = path.strip_suffix("/-").unwrap_or(path);
    let (owner, name) = path.rsplit_once('/')?;
    if owner.is_empty() || name.is_empty() {
        return None;
    }
    if endpoint.kind != HostKind::GitLab && owner.contains('/') {
        return None;
    }
    let web = endpoint.web(path);
    Some(HostedRepository {
        kind: endpoint.kind,
        repo: GitHubRepository {
            endpoint: endpoint.api_base.clone(),
            owner: owner.to_string(),
            name: name.to_string(),
            html_url: web.clone(),
            clone_url: format!("{web}.git"),
            default_branch: None,
            private: false,
            fork: false,
            parent: None,
            archived: false,
            permissions: None,
            allow_forking: None,
            node_id: None,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_server_addresses() {
        let gl = HostEndpoint::parse(HostKind::GitLab, "gitlab.example.com/", false).unwrap();
        assert_eq!(gl.web_base, "https://gitlab.example.com");
        assert_eq!(gl.api_base, "https://gitlab.example.com/api/v4");
        assert_eq!(gl.host(), "gitlab.example.com");
        let gl =
            HostEndpoint::parse(HostKind::GitLab, "https://GitLab.com/api/v4/", false).unwrap();
        assert_eq!(gl, HostEndpoint::public(HostKind::GitLab));
        assert!(gl.is_public());
        let sub =
            HostEndpoint::parse(HostKind::Gitea, "https://example.com:3000/gitea/", false).unwrap();
        assert_eq!(sub.web_base, "https://example.com:3000/gitea");
        assert_eq!(sub.api_base, "https://example.com:3000/gitea/api/v1");
        assert_eq!(sub.host(), "example.com");
        assert_eq!(sub.base_path(), "gitea");
        assert_eq!(sub.origin(), "https://example.com:3000");
        assert_eq!(
            HostEndpoint::parse(HostKind::Gitea, "http://git.corp", false),
            Err(HostAddressError::InvalidProtocol)
        );
        assert_eq!(
            HostEndpoint::parse(HostKind::Gitea, "http://git.corp", true)
                .unwrap()
                .api_base,
            "http://git.corp/api/v1"
        );
        assert_eq!(
            HostEndpoint::parse(HostKind::Gitea, "http://127.0.0.1:8080", false)
                .unwrap()
                .web_base,
            "http://127.0.0.1:8080"
        );
        assert_eq!(
            HostEndpoint::parse(HostKind::GitLab, "  ", false),
            Err(HostAddressError::InvalidUrl)
        );
        assert_eq!(
            HostEndpoint::parse(HostKind::GitLab, "not a host", false),
            Err(HostAddressError::InvalidUrl)
        );
        assert_eq!(
            HostEndpoint::parse(HostKind::Bitbucket, "bitbucket.org", false).unwrap(),
            HostEndpoint::public(HostKind::Bitbucket)
        );
        assert_eq!(
            HostEndpoint::public(HostKind::Bitbucket).api_base,
            "https://api.bitbucket.org/2.0"
        );
        assert_eq!(
            HostEndpoint::parse(HostKind::Bitbucket, "bitbucket.corp", false),
            Err(HostAddressError::BitbucketServer)
        );
        assert_eq!(
            HostEndpoint::public(HostKind::Gitea).friendly_name(),
            "Codeberg"
        );
    }

    #[test]
    fn matches_remotes_on_known_hosts() {
        let endpoints = [
            HostEndpoint::public(HostKind::GitLab),
            HostEndpoint::public(HostKind::Bitbucket),
            HostEndpoint::parse(HostKind::Gitea, "https://example.com/gitea", false).unwrap(),
        ];
        let gl = hosted_from_remote("git@gitlab.com:group/sub/proj.git", &endpoints).unwrap();
        assert_eq!(gl.kind, HostKind::GitLab);
        assert_eq!(gl.repo.owner, "group/sub");
        assert_eq!(gl.repo.name, "proj");
        assert_eq!(gl.repo.endpoint, "https://gitlab.com/api/v4");
        assert_eq!(gl.repo.html_url, "https://gitlab.com/group/sub/proj");
        assert_eq!(gl.path(), "group/sub/proj");
        let gl = hosted_from_remote("https://oauth2:x@gitlab.com/a/b", &endpoints).unwrap();
        assert_eq!(gl.repo.full_name(), "a/b");
        let bb = hosted_from_remote("https://me@bitbucket.org/ws/repo.git", &endpoints).unwrap();
        assert_eq!(bb.kind, HostKind::Bitbucket);
        assert_eq!(bb.repo.endpoint, "https://api.bitbucket.org/2.0");
        assert_eq!(bb.repo.html_url, "https://bitbucket.org/ws/repo");
        assert!(hosted_from_remote("git@bitbucket.org:ws/a/b.git", &endpoints).is_none());
        let gt = hosted_from_remote("https://example.com/gitea/o/n.git", &endpoints).unwrap();
        assert_eq!(gt.repo.full_name(), "o/n");
        assert_eq!(gt.repo.html_url, "https://example.com/gitea/o/n");
        let gt = hosted_from_remote("gitea@example.com:o/n.git", &endpoints).unwrap();
        assert_eq!(gt.repo.full_name(), "o/n");
        assert!(hosted_from_remote("git@github.com:o/n.git", &endpoints).is_none());
        assert!(hosted_from_remote("/local/path", &endpoints).is_none());
        assert!(hosted_from_remote("git@gitlab.com:solo.git", &endpoints).is_none());
    }

    #[test]
    fn git_usernames() {
        let mut account = HostAccount {
            endpoint: HostEndpoint::public(HostKind::GitLab),
            id: "1".into(),
            login: "mona".into(),
            name: Some("Mona".into()),
            avatar_url: None,
            emails: Vec::new(),
            auth: HostAuthKind::OAuth,
            basic_user: None,
            oauth_client_id: None,
            expires_at: None,
            needs_reauth: false,
        };
        assert_eq!(account.git_username(), "oauth2");
        assert_eq!(account.title(), "@mona (Mona)");
        account.auth = HostAuthKind::Token;
        assert_eq!(account.git_username(), "mona");
        account.endpoint = HostEndpoint::public(HostKind::Bitbucket);
        assert_eq!(account.git_username(), "x-bitbucket-api-token-auth");
        assert_eq!(account.keychain_host(), "bitbucket.org");
        assert_eq!(HostKind::GitLab.number_prefix(), '!');
        assert_eq!(
            HostKind::of_api_base("https://gitlab.corp/api/v4"),
            HostKind::GitLab
        );
        assert_eq!(
            HostKind::of_api_base("https://codeberg.org/api/v1"),
            HostKind::Gitea
        );
        assert_eq!(
            HostKind::of_api_base("https://api.bitbucket.org/2.0"),
            HostKind::Bitbucket
        );
        assert_eq!(
            HostKind::of_api_base("https://ghe.corp/api/v3"),
            HostKind::GitHub
        );
        assert_eq!(
            HostKind::of_api_base("https://api.github.com"),
            HostKind::GitHub
        );
        assert_eq!(
            HostKind::GitLab.pull_request_ref(3).as_deref(),
            Some("refs/merge-requests/3/head")
        );
        assert_eq!(
            serde_json::to_string(&HostKind::GitLab).unwrap(),
            "\"gitlab\""
        );
    }

    #[test]
    fn timestamps_with_offsets() {
        let utc = crate::parse_iso8601("2026-10-05T19:55:00Z").unwrap();
        assert_eq!(crate::parse_iso8601("2026-10-05T21:55:00+02:00"), Some(utc));
        assert_eq!(
            crate::parse_iso8601("2026-10-05T14:55:00.5-05:00"),
            Some(utc)
        );
        assert_eq!(
            crate::parse_iso8601("2026-10-05T19:55:00.123456+00:00"),
            Some(utc)
        );
    }
}
