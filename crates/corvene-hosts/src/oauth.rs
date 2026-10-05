//! OAuth sign-in for the hosts other than GitHub: the authorization code
//! flow with PKCE in the browser, the callback on the loopback listener
//! (`corvene_github::auth::LoopbackListener`, `http://127.0.0.1:<port>/`),
//! and the refresh of tokens that expire.
//!
//! Each server needs an application registered for Corvene: GitLab and
//! Gitea / Forgejo accept a public one (no secret) whose redirect URI is
//! `http://127.0.0.1/` (any port is let through for a loopback address);
//! Bitbucket's consumers always have a secret, so OAuth there needs one
//! baked into the build. Builds list their applications in
//! `CORVENE_HOSTS_OAUTH` (`gitlab:gitlab.com=<id>,gitea:codeberg.org=<id>,
//! bitbucket:bitbucket.org=<key>:<secret>`); otherwise the sign-in dialog
//! takes an application ID typed in.

use std::time::{SystemTime, UNIX_EPOCH};

use corvene_github::auth::{WebFlow, code_challenge};
use corvene_models::{HostEndpoint, HostKind};
use serde::Deserialize;

use crate::http::{Auth, Http, encode};
use crate::{HostError, Result};

/// The applications this build knows (see the module docs).
pub const BUILT_IN_APPS: Option<&str> = option_env!("CORVENE_HOSTS_OAUTH");

/// An OAuth application on one host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OAuthApp {
    pub client_id: String,
    pub client_secret: Option<String>,
}

impl OAuthApp {
    /// The build's application for `endpoint`, if any.
    pub fn built_in(endpoint: &HostEndpoint) -> Option<Self> {
        Self::from_spec(BUILT_IN_APPS?, endpoint)
    }

    /// `endpoint`'s entry in a `kind:host=id[:secret],…` list.
    pub fn from_spec(spec: &str, endpoint: &HostEndpoint) -> Option<Self> {
        let kind = slug(endpoint.kind);
        spec.split(',').find_map(|entry| {
            let (key, app) = entry.trim().split_once('=')?;
            let (entry_kind, host) = key.trim().split_once(':')?;
            if !entry_kind.eq_ignore_ascii_case(kind) || !host.eq_ignore_ascii_case(endpoint.host())
            {
                return None;
            }
            let (id, secret) = match app.split_once(':') {
                Some((id, secret)) => (id.trim(), Some(secret.trim())),
                None => (app.trim(), None),
            };
            (!id.is_empty()).then(|| Self {
                client_id: id.to_string(),
                client_secret: secret.filter(|s| !s.is_empty()).map(str::to_string),
            })
        })
    }

    /// Whether this application can sign in on `kind` (Bitbucket needs a
    /// secret).
    pub fn usable_on(&self, kind: HostKind) -> bool {
        kind != HostKind::Bitbucket || self.client_secret.is_some()
    }
}

fn slug(kind: HostKind) -> &'static str {
    match kind {
        HostKind::GitHub => "github",
        HostKind::GitLab => "gitlab",
        HostKind::Gitea => "gitea",
        HostKind::Bitbucket => "bitbucket",
    }
}

/// The scopes asked for: the API (pull requests, statuses), the user's
/// profile and emails, and git over HTTPS.
pub fn scopes(kind: HostKind) -> &'static str {
    match kind {
        HostKind::GitHub => corvene_github::SCOPES,
        HostKind::GitLab => "api read_user write_repository",
        // Gitea limits a token to the scopes named; Forgejo ignores them
        HostKind::Gitea => "read:user write:repository",
        // a consumer's scopes are fixed when it is registered
        HostKind::Bitbucket => "",
    }
}

fn authorize_endpoint(endpoint: &HostEndpoint) -> String {
    match endpoint.kind {
        HostKind::GitLab => endpoint.web("oauth/authorize"),
        HostKind::Bitbucket => "https://bitbucket.org/site/oauth2/authorize".into(),
        HostKind::GitHub | HostKind::Gitea => endpoint.web("login/oauth/authorize"),
    }
}

fn token_endpoint(endpoint: &HostEndpoint) -> String {
    match endpoint.kind {
        HostKind::GitLab => endpoint.web("oauth/token"),
        HostKind::Bitbucket => "https://bitbucket.org/site/oauth2/access_token".into(),
        HostKind::GitHub | HostKind::Gitea => endpoint.web("login/oauth/access_token"),
    }
}

/// Where the browser goes to authorize `client_id` for `flow`.
pub fn authorize_url(endpoint: &HostEndpoint, client_id: &str, flow: &WebFlow) -> String {
    let mut url = format!(
        "{}?client_id={}&response_type=code&state={}&redirect_uri={}",
        authorize_endpoint(endpoint),
        encode(client_id),
        encode(&flow.state),
        encode(&flow.redirect_uri),
    );
    if endpoint.kind != HostKind::Bitbucket {
        url.push_str(&format!(
            "&scope={}&code_challenge={}&code_challenge_method=S256",
            encode(scopes(endpoint.kind)),
            code_challenge(&flow.code_verifier)
        ));
    }
    url
}

/// What a token endpoint hands back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenSet {
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// Unix seconds; `None` when the token does not expire.
    pub expires_at: Option<u64>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    error_description: Option<String>,
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

fn request_token(
    endpoint: &HostEndpoint,
    app: &OAuthApp,
    form: &mut Vec<(&str, String)>,
) -> Result<TokenSet> {
    // Bitbucket authenticates the consumer with Basic, the others take the
    // id (and a secret, if the application has one) in the form
    let auth = match (&app.client_secret, endpoint.kind) {
        (Some(secret), HostKind::Bitbucket) => Auth::Basic {
            user: app.client_id.clone(),
            token: secret.clone(),
        },
        (secret, _) => {
            form.push(("client_id", app.client_id.clone()));
            if let Some(secret) = secret {
                form.push(("client_secret", secret.clone()));
            }
            Auth::Anonymous
        }
    };
    let http = Http::new(endpoint.clone(), auth);
    let pairs: Vec<(&str, &str)> = form.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let response: TokenResponse = http.post_form(&token_endpoint(endpoint), &pairs)?;
    match (response.access_token, response.error) {
        (Some(access_token), _) => Ok(TokenSet {
            access_token,
            refresh_token: response.refresh_token.filter(|t| !t.is_empty()),
            expires_at: response.expires_in.map(|s| now_secs() + s),
        }),
        (None, Some(error)) => Err(HostError::Auth(response.error_description.unwrap_or(error))),
        (None, None) => Err(HostError::Auth(
            "The server did not hand out a token.".into(),
        )),
    }
}

/// The callback's `code` → tokens.
pub fn exchange_code(
    endpoint: &HostEndpoint,
    app: &OAuthApp,
    flow: &WebFlow,
    code: &str,
) -> Result<TokenSet> {
    let mut form = vec![
        ("grant_type", "authorization_code".to_string()),
        ("code", code.to_string()),
        ("redirect_uri", flow.redirect_uri.clone()),
    ];
    if endpoint.kind != HostKind::Bitbucket {
        form.push(("code_verifier", flow.code_verifier.clone()));
    }
    request_token(endpoint, app, &mut form)
}

/// A refresh token → a new access token (and, on GitLab, a new refresh
/// token: the old one stops working).
pub fn refresh(endpoint: &HostEndpoint, app: &OAuthApp, refresh_token: &str) -> Result<TokenSet> {
    let mut form = vec![
        ("grant_type", "refresh_token".to_string()),
        ("refresh_token", refresh_token.to_string()),
    ];
    request_token(endpoint, app, &mut form)
}

/// Whether a token expiring at `expires_at` should be refreshed now (five
/// minutes ahead, so a git operation started now still gets a live one).
pub fn needs_refresh(expires_at: Option<u64>, now: u64) -> bool {
    expires_at.is_some_and(|at| at <= now + 5 * 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_build_spec() {
        let gl = HostEndpoint::public(HostKind::GitLab);
        let bb = HostEndpoint::public(HostKind::Bitbucket);
        let spec = "gitlab:gitlab.com=abc, bitbucket:bitbucket.org = key:secret ,gitea:x=";
        assert_eq!(
            OAuthApp::from_spec(spec, &gl),
            Some(OAuthApp {
                client_id: "abc".into(),
                client_secret: None
            })
        );
        let app = OAuthApp::from_spec(spec, &bb).unwrap();
        assert!(app.usable_on(HostKind::Bitbucket));
        assert_eq!(app.client_secret.as_deref(), Some("secret"));
        assert!(OAuthApp::from_spec(spec, &HostEndpoint::public(HostKind::Gitea)).is_none());
        assert!(
            !OAuthApp {
                client_id: "k".into(),
                client_secret: None
            }
            .usable_on(HostKind::Bitbucket)
        );
    }

    #[test]
    fn builds_authorize_urls() {
        let flow = WebFlow {
            state: "st".into(),
            code_verifier: "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".into(),
            redirect_uri: "http://127.0.0.1:4567/".into(),
        };
        let gl = HostEndpoint::parse(HostKind::GitLab, "gitlab.corp", false).unwrap();
        assert_eq!(
            authorize_url(&gl, "id", &flow),
            "https://gitlab.corp/oauth/authorize?client_id=id&response_type=code&state=st\
             &redirect_uri=http%3A%2F%2F127.0.0.1%3A4567%2F&scope=api%20read_user%20write_repository\
             &code_challenge=E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM&code_challenge_method=S256"
        );
        let gt = HostEndpoint::public(HostKind::Gitea);
        assert!(
            authorize_url(&gt, "id", &flow)
                .starts_with("https://codeberg.org/login/oauth/authorize?client_id=id")
        );
        let bb = HostEndpoint::public(HostKind::Bitbucket);
        assert_eq!(
            authorize_url(&bb, "key", &flow),
            "https://bitbucket.org/site/oauth2/authorize?client_id=key&response_type=code&state=st\
             &redirect_uri=http%3A%2F%2F127.0.0.1%3A4567%2F"
        );
        assert!(needs_refresh(Some(1000), 800));
        assert!(!needs_refresh(Some(10_000), 800));
        assert!(!needs_refresh(None, 800));
    }
}
