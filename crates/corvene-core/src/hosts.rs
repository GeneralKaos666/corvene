//! Hosts other than GitHub: GitLab, Gitea / Forgejo / Codeberg and
//! Bitbucket Cloud (Corvene addition, flags `342-gitlab`, `343-gitea`,
//! `344-bitbucket`; design in `.docs/hosts.md`). GitHub Desktop knows
//! GitHub.com and GitHub Enterprise only (desktop/desktop#7875, #14052,
//! #18588).
//!
//! Kept apart from the GitHub code on purpose: the host accounts are their
//! own list ([`HostsState::accounts`], store key `host-accounts`) and a
//! repository's hosted counterpart is worked out from its default remote
//! ([`AppState::hosted_repository`]) instead of going into
//! `Repository::github`, so none of the GitHub-only stores (Alive,
//! notifications, rulesets, forks…) ever sends a GitHub request to another
//! host. What the hosts share with GitHub is the data: merge requests land
//! in the same `PullRequestCache`, CI results in the same
//! `CommitStatusStore`, so the Pull Requests tab, the toolbar badge and the
//! checks popover draw them unchanged. The two stores branch to this module
//! where they would call GitHub (`refresh_pull_requests`,
//! `refresh_commit_status`).
//!
//! Tokens live in the Keychain under (web host, git user name), so the
//! askpass helper answers git's HTTPS prompts for these hosts too
//! (`Dispatcher::askpass_env`). OAuth tokens expire: they are refreshed
//! before a call that finds them stale and every 30 minutes.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use corvene_github::auth::{LoopbackListener, WebFlow};
use corvene_hosts::oauth::{self, OAuthApp, TokenSet};
use corvene_hosts::{CheckTarget, HostError, HostProvider};
use corvene_models::{
    CombinedRefCheck, GitHubRepository, HostAccount, HostAuthKind, HostEndpoint, HostKind,
    HostedRepository, PullRequest,
};
use corvene_store::Store;
use tracing::{info, warn};

use crate::dispatcher::Dispatcher;
use crate::flags::ids;
use crate::host::{AsyncCtx, Host};
use crate::remote::spawn_bg;
use crate::state::{AppState, Popup};

/// The store key of the host accounts (tokens are in the Keychain).
const STORE_KEY: &str = "host-accounts";
/// How often OAuth tokens are checked for refreshing.
const TOKEN_REFRESH_INTERVAL: Duration = Duration::from_secs(30 * 60);
/// A token expiring within this is refreshed by the periodic check.
const TOKEN_REFRESH_AHEAD_SECS: u64 = 35 * 60;
/// Pull requests are fetched again at most this often (GitHub's
/// `MaxPullRequestRefreshFrequency`).
const PULL_REQUEST_MIN_INTERVAL: Duration = Duration::from_secs(2 * 60);
/// Repository metadata is read again at most this often.
const METADATA_MIN_INTERVAL: Duration = Duration::from_secs(10 * 60);

/// The browser sign-in waiting for its callback.
pub struct PendingHostWebFlow {
    pub endpoint: HostEndpoint,
    pub app: OAuthApp,
    pub flow: WebFlow,
    _listener: Arc<LoopbackListener>,
}

/// The sign-in dialog's progress (`Popup::SignInHost`).
#[derive(Default)]
pub struct HostSignIn {
    pub loading: bool,
    /// Waiting for the browser (the dialog says so and offers Cancel).
    pub waiting_for_browser: bool,
    pub error: Option<String>,
    pub pending: Option<PendingHostWebFlow>,
    cancel: Arc<AtomicBool>,
}

/// Everything Corvene keeps about the hosts.
#[derive(Default)]
pub struct HostsState {
    /// The signed-in accounts, in the order they were added.
    pub accounts: Vec<HostAccount>,
    /// The repositories read from the APIs (default branch, fork parent,
    /// permission), by [`repository_key`].
    pub repositories: HashMap<String, GitHubRepository>,
    /// When each repository's metadata was last asked for.
    metadata_requested: HashMap<String, Instant>,
    pub sign_in: Option<HostSignIn>,
    /// Accounts whose token is being refreshed (`api_base` + login).
    refreshing: HashSet<String>,
}

impl HostsState {
    /// The accounts saved by an earlier session.
    pub fn load(store: &Store) -> Self {
        let accounts = store
            .get::<Vec<HostAccount>>(STORE_KEY)
            .ok()
            .flatten()
            .unwrap_or_default();
        Self {
            accounts,
            ..Self::default()
        }
    }

    fn save(&self, store: &Store) {
        if let Err(err) = store.set(STORE_KEY, &self.accounts) {
            warn!(%err, "could not save the host accounts");
        }
    }
}

/// The cache key of a hosted repository (API base + lower-cased path), the
/// same shape as `pull_requests::cache_key`.
pub fn repository_key(repo: &GitHubRepository) -> String {
    crate::pull_requests::cache_key(repo)
}

fn account_key(account: &HostAccount) -> String {
    format!("{}#{}", account.endpoint.api_base, account.login)
}

/// Whether the flag of `kind` is on (GitHub always is).
pub fn kind_enabled(flags: &crate::flags::Flags, kind: HostKind) -> bool {
    match kind {
        HostKind::GitHub => true,
        HostKind::GitLab => flags.bool(ids::GITLAB),
        HostKind::Gitea => flags.bool(ids::GITEA),
        HostKind::Bitbucket => flags.bool(ids::BITBUCKET),
    }
}

impl AppState {
    pub fn host_kind_enabled(&self, kind: HostKind) -> bool {
        kind_enabled(&self.flags, kind)
    }

    /// The signed-in host accounts whose flag is on.
    pub fn host_accounts(&self) -> impl Iterator<Item = &HostAccount> {
        self.hosts
            .accounts
            .iter()
            .filter(|a| self.host_kind_enabled(a.kind()))
    }

    /// The servers remotes are matched against: the accounts' and, for each
    /// kind that is on, its public one (gitlab.com, codeberg.org,
    /// bitbucket.org), unless a GitHub account claims the host.
    pub fn host_endpoints(&self) -> Vec<HostEndpoint> {
        let mut endpoints: Vec<HostEndpoint> =
            self.host_accounts().map(|a| a.endpoint.clone()).collect();
        for kind in HostKind::OTHERS {
            let public = HostEndpoint::public(kind);
            if self.host_kind_enabled(kind) && !endpoints.contains(&public) {
                endpoints.push(public);
            }
        }
        let github_hosts: Vec<String> = self.accounts.iter().map(|a| a.host()).collect();
        endpoints.retain(|e| {
            !github_hosts
                .iter()
                .any(|h| h.eq_ignore_ascii_case(e.host()))
        });
        endpoints
    }

    /// The hosted repository repository `id`'s default remote points at,
    /// with what its API said about it once read. `None` for a GitHub
    /// repository, a remote elsewhere, or with the host's flag off.
    pub fn hosted_repository(&self, id: u64) -> Option<HostedRepository> {
        let repository = self.repository(id)?;
        if repository.github.is_some() {
            return None;
        }
        let info = self.repo_states.get(&id)?.info.as_ref()?;
        let remote = corvene_git::find_default_remote(&info.remotes)?;
        let endpoints = self.host_endpoints();
        let mut hosted = corvene_models::hosted_from_remote(&remote.url, &endpoints)?;
        if let Some(fresh) = self.hosts.repositories.get(&repository_key(&hosted.repo)) {
            hosted.repo = fresh.clone();
        }
        Some(hosted)
    }

    /// The host kind of a non-GitHub API base Corvene talks to.
    pub fn host_kind_of(&self, api_base: &str) -> Option<HostKind> {
        self.host_endpoints()
            .into_iter()
            .find(|e| e.api_base == api_base)
            .map(|e| e.kind)
    }

    /// The host account for an API base.
    pub fn host_account_for(&self, api_base: &str) -> Option<&HostAccount> {
        self.host_accounts()
            .find(|a| a.endpoint.api_base == api_base)
    }

    /// The repository whose pull requests the Pull Requests tab lists: the
    /// GitHub one (`getNonForkGitHubRepository`), else the hosted one (its
    /// fork parent unless the fork is set up for its own work).
    pub fn pull_request_repository(&self, id: u64) -> Option<GitHubRepository> {
        let repository = self.repository(id)?;
        if let Some(gh) = repository.non_fork_github() {
            return Some(gh.clone());
        }
        let hosted = self.hosted_repository(id)?;
        let to_parent =
            repository.fork_contribution_target() == corvene_models::ForkContributionTarget::Parent;
        Some(hosted.non_fork(to_parent).clone())
    }

    /// The kind of host a repository's web pages are on (GitHub unless it
    /// is a hosted one).
    pub fn web_host_kind(&self, api_base: &str) -> HostKind {
        self.host_kind_of(api_base).unwrap_or(HostKind::GitHub)
    }

    /// The web page of a pull request, on whichever host its base is.
    pub fn pull_request_web_url(&self, pr: &PullRequest) -> Option<String> {
        let base = pr.base.repository.as_ref()?;
        Some(corvene_hosts::urls::pull_request_url(
            self.web_host_kind(&base.endpoint),
            base,
            pr.number,
        ))
    }

    /// A commit's web page on `repo`'s host.
    pub fn commit_web_url(&self, repo: &GitHubRepository, sha: &str) -> String {
        corvene_hosts::urls::commit_url(self.web_host_kind(&repo.endpoint), repo, sha)
    }

    /// The askpass `host=user` pairs of the host accounts (after GitHub's,
    /// before the generic logins, which win for a host they share).
    pub(crate) fn host_askpass_logins(&self) -> Vec<String> {
        let github_hosts: Vec<String> = self.accounts.iter().map(|a| a.host()).collect();
        self.host_accounts()
            .filter(|a| !github_hosts.contains(&a.keychain_host()))
            .map(|a| format!("{}={}", a.keychain_host(), a.git_username()))
            .collect()
    }
}

/// What a background host call needs: the provider's endpoint, the
/// account (`None`: anonymous, public data only) and its token.
#[derive(Clone)]
pub(crate) struct HostCall {
    endpoint: HostEndpoint,
    account: Option<HostAccount>,
    token: String,
}

impl HostCall {
    /// A provider with a live token: an OAuth token that is about to expire
    /// is refreshed first (the new one is in the Keychain and returned).
    fn provider(&self) -> Result<(Box<dyn HostProvider>, Option<TokenSet>), HostError> {
        let Some(account) = &self.account else {
            return Ok((corvene_hosts::provider(&self.endpoint, "", None), None));
        };
        let mut token = self.token.clone();
        let mut refreshed = None;
        if account.auth == HostAuthKind::OAuth
            && oauth::needs_refresh(account.expires_at, oauth::now_secs())
        {
            let set = refresh_account_token(account)?;
            token = set.access_token.clone();
            refreshed = Some(set);
        }
        Ok((corvene_hosts::provider_for(account, &token), refreshed))
    }
}

/// The OAuth application an account's tokens came from (its client ID and,
/// when the build has one for it, the secret).
fn account_app(account: &HostAccount) -> Option<OAuthApp> {
    let client_id = account.oauth_client_id.clone()?;
    let secret = OAuthApp::built_in(&account.endpoint)
        .filter(|app| app.client_id == client_id)
        .and_then(|app| app.client_secret);
    Some(OAuthApp {
        client_id,
        client_secret: secret,
    })
}

/// Refresh `account`'s OAuth token and store the new pair. Blocks (network,
/// Keychain): background threads only.
fn refresh_account_token(account: &HostAccount) -> Result<TokenSet, HostError> {
    let host = account.keychain_host();
    let refresh_token = corvene_platform::keychain::token(&host, &account.refresh_token_user())
        .ok()
        .flatten()
        .ok_or_else(|| HostError::Auth("The sign-in has no refresh token.".into()))?;
    let app = account_app(account)
        .ok_or_else(|| HostError::Auth("The sign-in has no OAuth application.".into()))?;
    let set = oauth::refresh(&account.endpoint, &app, &refresh_token)?;
    store_tokens(account, &set).map_err(HostError::Auth)?;
    info!(host = %host, login = %account.login, "refreshed the host token");
    Ok(set)
}

/// The tokens of `account` into the Keychain.
fn store_tokens(account: &HostAccount, set: &TokenSet) -> Result<(), String> {
    let host = account.keychain_host();
    corvene_platform::keychain::store_token(&host, &account.git_username(), &set.access_token)
        .map_err(|e| e.to_string())?;
    if let Some(refresh) = &set.refresh_token {
        corvene_platform::keychain::store_token(&host, &account.refresh_token_user(), refresh)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

impl Dispatcher {
    /// A host call for `api_base`: its account and token, or anonymous on a
    /// public server without one. `None` for a self-hosted server nobody
    /// signed in to.
    pub(crate) fn host_call(api_base: &str, cx: &dyn Host) -> Option<HostCall> {
        let s = Self::state(cx).read(cx);
        let endpoint = s
            .host_endpoints()
            .into_iter()
            .find(|e| e.api_base == api_base)?;
        match s.host_account_for(api_base).cloned() {
            Some(account) => {
                let token = corvene_platform::keychain::token(
                    &account.keychain_host(),
                    &account.git_username(),
                )
                .ok()
                .flatten()?;
                Some(HostCall {
                    endpoint,
                    account: Some(account),
                    token,
                })
            }
            None if endpoint.is_public() => Some(HostCall {
                endpoint,
                account: None,
                token: String::new(),
            }),
            None => None,
        }
    }

    /// After a call: keep a refreshed token's expiry, and mark the account
    /// for signing in again when the server refused its token.
    fn host_call_done(
        call: &HostCall,
        refreshed: Option<TokenSet>,
        unauthorized: bool,
        cx: &mut dyn Host,
    ) {
        let Some(account) = &call.account else {
            return;
        };
        let key = account_key(account);
        Self::state(cx).update(cx, |s, cx| {
            let store = s.store.clone();
            let Some(stored) = s.hosts.accounts.iter_mut().find(|a| account_key(a) == key) else {
                return;
            };
            let mut changed = false;
            if let Some(set) = refreshed {
                stored.expires_at = set.expires_at;
                stored.needs_reauth = false;
                changed = true;
            }
            if unauthorized && !stored.needs_reauth {
                warn!(login = %stored.login, host = %stored.keychain_host(), "host token refused");
                stored.needs_reauth = true;
                changed = true;
            }
            if changed {
                s.hosts.save(&store);
                cx.notify();
            }
        });
    }

    // ---- sign-in ----

    /// Settings › Accounts › Sign Into <host>: open the sign-in dialog.
    pub fn show_host_sign_in(kind: HostKind, cx: &mut dyn Host) {
        Self::state(cx).update(cx, |s, cx| {
            s.hosts.sign_in = Some(HostSignIn::default());
            cx.notify();
        });
        Self::show_popup(Popup::SignInHost { kind }, cx);
    }

    fn set_host_sign_in(f: impl FnOnce(&mut HostSignIn), cx: &mut dyn Host) {
        Self::state(cx).update(cx, |s, cx| {
            let sign_in = s.hosts.sign_in.get_or_insert_with(HostSignIn::default);
            f(sign_in);
            cx.notify();
        });
    }

    fn host_sign_in_failed(message: String, cx: &mut dyn Host) {
        Self::set_host_sign_in(
            |s| {
                s.loading = false;
                s.waiting_for_browser = false;
                s.pending = None;
                s.error = Some(message);
            },
            cx,
        );
    }

    fn parse_host_address(
        kind: HostKind,
        address: &str,
        cx: &dyn Host,
    ) -> Result<HostEndpoint, String> {
        let allow_http = Self::state(cx)
            .read(cx)
            .flags
            .bool(ids::ENTERPRISE_PLAIN_HTTP);
        let address = if address.trim().is_empty() {
            kind.default_web_base()
        } else {
            address
        };
        HostEndpoint::parse(kind, address, allow_http).map_err(|e| e.to_string())
    }

    /// Sign in with a personal access token (an Atlassian API token and its
    /// account email on Bitbucket).
    pub fn host_sign_in_with_token(
        kind: HostKind,
        address: String,
        token: String,
        email: Option<String>,
        cx: &mut dyn Host,
    ) {
        let endpoint = match Self::parse_host_address(kind, &address, cx) {
            Ok(endpoint) => endpoint,
            Err(message) => return Self::host_sign_in_failed(message, cx),
        };
        let token = token.trim().to_string();
        if token.is_empty() {
            return Self::host_sign_in_failed("Enter a token.".into(), cx);
        }
        let email = email
            .map(|e| e.trim().to_string())
            .filter(|e| !e.is_empty());
        if kind == HostKind::Bitbucket && email.is_none() && !token.starts_with("ATCTT") {
            // an Atlassian API token goes with its account's email; access
            // tokens (`ATCTT…`) are sent on their own
            return Self::host_sign_in_failed(
                "Enter the email address of your Atlassian account.".into(),
                cx,
            );
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_bg = cancel.clone();
        Self::set_host_sign_in(
            |s| {
                s.loading = true;
                s.error = None;
                s.cancel = cancel;
            },
            cx,
        );
        spawn_bg(
            cx,
            move || {
                corvene_hosts::provider(&endpoint, &token, email.as_deref())
                    .current_user()
                    .map(|mut account| {
                        account.auth = HostAuthKind::Token;
                        account.basic_user = email;
                        (account, token)
                    })
            },
            move |result, cx| {
                if cancel_bg.load(Ordering::SeqCst) {
                    return;
                }
                match result {
                    Ok((account, token)) => Self::finish_host_sign_in(
                        account,
                        TokenSet {
                            access_token: token,
                            refresh_token: None,
                            expires_at: None,
                        },
                        cx,
                    ),
                    Err(err) => Self::host_sign_in_failed(sign_in_error_message(&err), cx),
                }
            },
        );
    }

    /// The OAuth application to sign in to `endpoint` with: the one typed
    /// in the dialog (with the build's secret when it is the build's), else
    /// the build's own.
    pub fn host_oauth_app(endpoint: &HostEndpoint, typed: Option<&str>) -> Option<OAuthApp> {
        let built_in = OAuthApp::built_in(endpoint);
        let app = match typed.map(str::trim).filter(|t| !t.is_empty()) {
            Some(id) => Some(OAuthApp {
                client_id: id.to_string(),
                client_secret: built_in
                    .filter(|app| app.client_id == id)
                    .and_then(|app| app.client_secret),
            }),
            None => built_in,
        };
        app.filter(|app| app.usable_on(endpoint.kind))
    }

    /// Sign in in the browser: start the loopback listener, open the host's
    /// authorize page and wait for the callback.
    pub fn host_sign_in_with_browser(
        kind: HostKind,
        address: String,
        client_id: Option<String>,
        cx: &mut dyn Host,
    ) {
        let endpoint = match Self::parse_host_address(kind, &address, cx) {
            Ok(endpoint) => endpoint,
            Err(message) => return Self::host_sign_in_failed(message, cx),
        };
        let Some(app) = Self::host_oauth_app(&endpoint, client_id.as_deref()) else {
            return Self::host_sign_in_failed(
                format!(
                    "No OAuth application is set up for {}. Enter one's application ID, or sign in with a token.",
                    endpoint.host()
                ),
                cx,
            );
        };
        let (tx, rx) = async_channel::unbounded::<Option<(String, String)>>();
        let listener = match LoopbackListener::start(move |result| {
            let _ = tx.send_blocking(result);
        }) {
            Ok(listener) => listener,
            Err(err) => return Self::host_sign_in_failed(err.to_string(), cx),
        };
        // a trailing `/`: GitLab compares the registered `http://127.0.0.1/`
        // path exactly, and lets the port vary for a loopback address
        let flow = match WebFlow::new(format!("{}/", listener.redirect_uri)) {
            Ok(flow) => flow,
            Err(err) => return Self::host_sign_in_failed(err.to_string(), cx),
        };
        let url = oauth::authorize_url(&endpoint, &app.client_id, &flow);
        Self::set_host_sign_in(
            |s| {
                s.loading = true;
                s.waiting_for_browser = true;
                s.error = None;
                s.pending = Some(PendingHostWebFlow {
                    endpoint,
                    app,
                    flow,
                    _listener: Arc::new(listener),
                });
            },
            cx,
        );
        cx.spawn(async move |cx: &mut AsyncCtx| {
            if let Ok(Some((code, state))) = rx.recv().await {
                cx.update(|cx| Self::complete_host_web_flow(code, state, cx));
            }
        })
        .detach();
        Self::open_url(&url, cx);
    }

    /// The browser came back with `code`: exchange it, read the user, sign in.
    fn complete_host_web_flow(code: String, state: String, cx: &mut dyn Host) {
        let pending = Self::state(cx).update(cx, |s, _| {
            let sign_in = s.hosts.sign_in.as_mut()?;
            let pending = sign_in.pending.take()?;
            if pending.flow.state != state {
                warn!("ignored an OAuth callback for another sign-in");
                sign_in.pending = Some(pending);
                return None;
            }
            sign_in.waiting_for_browser = false;
            Some((
                pending.endpoint,
                pending.app,
                pending.flow,
                sign_in.cancel.clone(),
            ))
        });
        let Some((endpoint, app, flow, cancel)) = pending else {
            return;
        };
        Self::state(cx).update(cx, |_, cx| cx.notify());
        spawn_bg(
            cx,
            move || {
                let set = oauth::exchange_code(&endpoint, &app, &flow, &code)?;
                let provider = match endpoint.kind {
                    // Gitea takes OAuth tokens as Bearer, access tokens as `token`
                    HostKind::Gitea => Box::new(corvene_hosts::gitea::Gitea::new_bearer(
                        endpoint.clone(),
                        &set.access_token,
                    )) as Box<dyn HostProvider>,
                    _ => corvene_hosts::provider(&endpoint, &set.access_token, None),
                };
                let mut account = provider.current_user()?;
                account.auth = HostAuthKind::OAuth;
                account.oauth_client_id = Some(app.client_id.clone());
                account.expires_at = set.expires_at;
                Ok::<_, HostError>((account, set))
            },
            move |result, cx| {
                if cancel.load(Ordering::SeqCst) {
                    return;
                }
                match result {
                    Ok((account, set)) => Self::finish_host_sign_in(account, set, cx),
                    Err(err) => Self::host_sign_in_failed(sign_in_error_message(&err), cx),
                }
            },
        );
    }

    /// Store the token(s), add the account (in place of the same user on
    /// the same server) and close the dialog.
    fn finish_host_sign_in(account: HostAccount, set: TokenSet, cx: &mut dyn Host) {
        if let Err(err) = store_tokens(&account, &set) {
            return Self::host_sign_in_failed(format!("Could not store the token: {err}"), cx);
        }
        info!(login = %account.login, host = %account.keychain_host(), "signed in to a host");
        Self::state(cx).update(cx, |s, cx| {
            let store = s.store.clone();
            let accounts = &mut s.hosts.accounts;
            match accounts.iter_mut().find(|a| {
                a.endpoint == account.endpoint && (a.id == account.id || a.login == account.login)
            }) {
                Some(slot) => *slot = account,
                None => accounts.push(account),
            }
            s.hosts.save(&store);
            s.hosts.sign_in = None;
            cx.notify();
        });
        Self::close_popups_where(|p| matches!(p, Popup::SignInHost { .. }), cx);
        Self::request_host_avatars(cx);
        // repositories on that host get their pull requests now
        if let Some(id) = Self::state(cx).read(cx).selected {
            Self::hosted_repository_changed(id, cx);
        }
    }

    /// Cancel in the sign-in dialog (or closing it).
    pub fn cancel_host_sign_in(cx: &mut dyn Host) {
        Self::state(cx).update(cx, |s, cx| {
            if let Some(sign_in) = s.hosts.sign_in.take() {
                sign_in.cancel.store(true, Ordering::SeqCst);
            }
            cx.notify();
        });
    }

    /// Settings › Accounts › Sign Out.
    pub fn host_sign_out(api_base: String, login: String, cx: &mut dyn Host) {
        let removed = Self::state(cx).update(cx, |s, cx| {
            let store = s.store.clone();
            let ix = s
                .hosts
                .accounts
                .iter()
                .position(|a| a.endpoint.api_base == api_base && a.login == login)?;
            let account = s.hosts.accounts.remove(ix);
            s.hosts.save(&store);
            cx.notify();
            Some(account)
        });
        if let Some(account) = removed {
            let host = account.keychain_host();
            for user in [account.git_username(), account.refresh_token_user()] {
                if let Err(err) = corvene_platform::keychain::delete_token(&host, &user) {
                    warn!(%err, %host, "could not remove a host token");
                }
            }
            info!(login = %account.login, %host, "signed out of a host");
        }
    }

    /// Every 30 minutes: refresh OAuth tokens that expire soon, so git's
    /// askpass (which reads the Keychain directly) finds a live one.
    pub fn start_host_token_refresh(cx: &mut dyn Host) {
        Self::request_host_avatars(cx);
        cx.spawn(async move |cx: &mut AsyncCtx| {
            loop {
                cx.update(Self::refresh_expiring_host_tokens);
                cx.background_executor().timer(TOKEN_REFRESH_INTERVAL).await;
            }
        })
        .detach();
    }

    /// The accounts' pictures for Settings › Accounts.
    fn request_host_avatars(cx: &mut dyn Host) {
        let urls: Vec<String> = Self::state(cx)
            .read(cx)
            .hosts
            .accounts
            .iter()
            .filter_map(|a| a.avatar_url.clone())
            .collect();
        for url in urls {
            Self::request_avatar_url(&url, cx);
        }
    }

    fn refresh_expiring_host_tokens(cx: &mut dyn Host) {
        let now = oauth::now_secs();
        let due: Vec<HostAccount> = Self::state(cx).update(cx, |s, _| {
            let due: Vec<HostAccount> = s
                .hosts
                .accounts
                .iter()
                .filter(|a| {
                    a.auth == HostAuthKind::OAuth
                        && !a.needs_reauth
                        && a.expires_at
                            .is_some_and(|at| at <= now + TOKEN_REFRESH_AHEAD_SECS)
                        && !s.hosts.refreshing.contains(&account_key(a))
                })
                .cloned()
                .collect();
            for account in &due {
                s.hosts.refreshing.insert(account_key(account));
            }
            due
        });
        for account in due {
            let call = HostCall {
                endpoint: account.endpoint.clone(),
                account: Some(account.clone()),
                token: String::new(),
            };
            spawn_bg(
                cx,
                move || refresh_account_token(&account),
                move |result, cx| {
                    if let Some(account) = &call.account {
                        let key = account_key(account);
                        Self::state(cx).update(cx, |s, _| {
                            s.hosts.refreshing.remove(&key);
                        });
                    }
                    match result {
                        Ok(set) => Self::host_call_done(&call, Some(set), false, cx),
                        Err(err) => {
                            warn!(%err, "could not refresh a host token");
                            Self::host_call_done(&call, None, err.is_unauthorized(), cx);
                        }
                    }
                },
            );
        }
    }

    // ---- repositories ----

    /// A repository was selected or its remotes read: when it is on a host,
    /// read its metadata and pull requests (each throttled).
    pub fn hosted_repository_changed(id: u64, cx: &mut dyn Host) {
        let Some(hosted) = Self::state(cx).read(cx).hosted_repository(id) else {
            return;
        };
        Self::refresh_hosted_repository(id, hosted, cx);
        Self::ensure_pull_requests(id, cx);
    }

    /// Read the hosted repository's default branch, fork parent and the
    /// user's permission.
    fn refresh_hosted_repository(id: u64, hosted: HostedRepository, cx: &mut dyn Host) {
        let key = repository_key(&hosted.repo);
        let due = Self::state(cx).update(cx, |s, _| {
            let due = s
                .hosts
                .metadata_requested
                .get(&key)
                .is_none_or(|t| t.elapsed() > METADATA_MIN_INTERVAL);
            if due {
                s.hosts
                    .metadata_requested
                    .insert(key.clone(), Instant::now());
            }
            due
        });
        if !due {
            return;
        }
        let Some(call) = Self::host_call(&hosted.repo.endpoint, cx) else {
            return;
        };
        let (owner, name) = (hosted.repo.owner.clone(), hosted.repo.name.clone());
        let bg_call = call.clone();
        spawn_bg(
            cx,
            move || {
                let (provider, refreshed) = bg_call.provider()?;
                Ok::<_, HostError>((provider.repository(&owner, &name)?, refreshed))
            },
            move |result, cx| match result {
                Ok((fresh, refreshed)) => {
                    Self::host_call_done(&call, refreshed, false, cx);
                    let parent_changed = Self::state(cx).update(cx, |s, cx| {
                        let previous = s.hosts.repositories.insert(key.clone(), fresh.clone());
                        let changed = previous.as_ref() != Some(&fresh);
                        if changed {
                            info!(id, repo = %fresh.full_name(), "read the hosted repository");
                            cx.notify();
                        }
                        changed && previous.is_none_or(|p| p.parent != fresh.parent)
                    });
                    // a fork's pull requests are its parent's
                    if parent_changed {
                        Self::refresh_pull_requests(id, false, cx);
                    }
                }
                Err(err) => {
                    warn!(id, %err, "could not read the hosted repository");
                    Self::host_call_done(&call, None, err.is_unauthorized(), cx);
                }
            },
        );
    }

    /// `refresh_pull_requests` for a hosted repository: always the whole
    /// open list (the hosts have no cheap "updated since" that also reports
    /// closed ones), at most every two minutes unless forced.
    pub(crate) fn refresh_host_pull_requests(
        id: u64,
        target: GitHubRepository,
        force: bool,
        cx: &mut dyn Host,
    ) {
        let key = crate::pull_requests::cache_key(&target);
        let skip = Self::state(cx).update(cx, |s, cx| {
            let cache = s.pull_requests.entry(key.clone()).or_default();
            if cache.loading
                || (!force
                    && cache
                        .last_refreshed
                        .is_some_and(|t| t.elapsed() < PULL_REQUEST_MIN_INTERVAL))
            {
                return true;
            }
            cache.loading = true;
            cache.last_refreshed = Some(Instant::now());
            cx.notify();
            false
        });
        if skip {
            return;
        }
        let Some(call) = Self::host_call(&target.endpoint, cx) else {
            Self::state(cx).update(cx, |s, cx| {
                if let Some(c) = s.pull_requests.get_mut(&key) {
                    c.loading = false;
                }
                cx.notify();
            });
            return;
        };
        let bg_call = call.clone();
        spawn_bg(
            cx,
            move || {
                let (provider, refreshed) = bg_call.provider()?;
                Ok::<_, HostError>((provider.open_pull_requests(&target)?, refreshed))
            },
            move |result, cx| {
                let (refreshed, unauthorized) = Self::state(cx).update(cx, |s, cx| {
                    let cache = s.pull_requests.entry(key.clone()).or_default();
                    cache.loading = false;
                    cx.notify();
                    match result {
                        Ok((prs, refreshed)) => {
                            info!(count = prs.len(), "host pull requests refreshed");
                            cache.pull_requests = prs;
                            (refreshed, false)
                        }
                        Err(err) => {
                            warn!(%err, "could not refresh the host's pull requests");
                            (None, err.is_unauthorized())
                        }
                    }
                });
                Self::host_call_done(&call, refreshed, unauthorized, cx);
                Self::subscribe_current_pull_request_status(id, cx);
            },
        );
    }

    /// `refresh_commit_status` for a subscription on a host: `refs/pull/N/head`
    /// (what the pull request list subscribes) is the cached pull request's
    /// head commit; anything else is a commit already.
    pub(crate) fn refresh_host_commit_status(
        key: String,
        sub: crate::commit_status::CommitStatusSubscription,
        cx: &mut dyn Host,
    ) {
        let finish = |key: &str, cx: &mut dyn Host| {
            Self::state(cx).update(cx, |s, _| {
                s.commit_statuses.in_flight.remove(key);
            });
        };
        let (repo, target) = {
            let s = Self::state(cx).read(cx);
            let kind = s.web_host_kind(&sub.api_base);
            let repo = GitHubRepository {
                endpoint: sub.api_base.clone(),
                owner: sub.owner.clone(),
                name: sub.name.clone(),
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
            };
            let number = sub
                .git_ref
                .strip_prefix("refs/pull/")
                .and_then(|r| r.strip_suffix("/head"))
                .and_then(|n| n.parse::<u64>().ok());
            match number {
                Some(number) => {
                    let pr = s
                        .pull_requests
                        .get(&crate::pull_requests::cache_key(&repo))
                        .and_then(|c| c.pull_requests.iter().find(|p| p.number == number));
                    match pr {
                        Some(pr) if !pr.head.sha.is_empty() => {
                            // Bitbucket keeps a fork's build statuses on the fork
                            let on = match (kind, &pr.head.repository) {
                                (HostKind::Bitbucket, Some(head)) => head.clone(),
                                _ => repo,
                            };
                            (
                                on,
                                CheckTarget {
                                    sha: pr.head.sha.clone(),
                                    pull_request: Some(number),
                                    branch: Some(pr.head.ref_name.clone()),
                                },
                            )
                        }
                        _ => return finish(&key, cx),
                    }
                }
                None => (
                    repo,
                    CheckTarget {
                        sha: sub.git_ref.clone(),
                        pull_request: None,
                        branch: sub.branch_name.clone(),
                    },
                ),
            }
        };
        let Some(call) = Self::host_call(&sub.api_base, cx) else {
            return finish(&key, cx);
        };
        let bg_call = call.clone();
        spawn_bg(
            cx,
            move || {
                let (provider, refreshed) = bg_call.provider()?;
                let checks = provider.ref_checks(&repo, &target)?;
                Ok::<_, HostError>((checks.and_then(CombinedRefCheck::from_checks), refreshed))
            },
            move |result, cx| {
                let (refreshed, unauthorized) = match result {
                    Ok((check, refreshed)) => {
                        Self::state(cx).update(cx, |s, cx| {
                            let store = &mut s.commit_statuses;
                            store.in_flight.remove(&key);
                            store.insert_entry(key.clone(), check);
                            cx.notify();
                        });
                        (refreshed, false)
                    }
                    Err(err) => {
                        warn!(%err, "could not read the host's CI status");
                        Self::state(cx).update(cx, |s, _| {
                            s.commit_statuses.in_flight.remove(&key);
                            s.commit_statuses.touch_entry(&key);
                        });
                        (None, err.is_unauthorized())
                    }
                };
                Self::host_call_done(&call, refreshed, unauthorized, cx);
            },
        );
    }

    // ---- web pages ----

    /// The hosted repository and current branch of `id`, for the
    /// Repository and Branch menu items.
    pub(crate) fn hosted_and_branch(
        id: u64,
        cx: &dyn Host,
    ) -> Option<(HostedRepository, Option<String>)> {
        let s = Self::state(cx).read(cx);
        let hosted = s.hosted_repository(id)?;
        let branch = s
            .repo_states
            .get(&id)
            .and_then(|rs| rs.info.as_ref())
            .and_then(|info| info.current_branch())
            .map(|b| b.name.clone());
        Some((hosted, branch))
    }

    /// Repository › View on <host>.
    pub(crate) fn view_hosted_repository(id: u64, cx: &mut dyn Host) -> bool {
        let Some((hosted, _)) = Self::hosted_and_branch(id, cx) else {
            return false;
        };
        Self::open_url(&hosted.repo.html_url, cx);
        true
    }

    /// Branch › Compare on <host>.
    pub(crate) fn compare_on_host(id: u64, cx: &mut dyn Host) -> bool {
        let Some((hosted, branch)) = Self::hosted_and_branch(id, cx) else {
            return false;
        };
        if let Some(branch) = branch {
            let url = corvene_hosts::urls::compare_url(hosted.kind, &hosted.repo, &branch);
            Self::open_url(&url, cx);
        }
        true
    }

    /// Branch › View Branch on <host>.
    pub(crate) fn view_branch_on_host(id: u64, cx: &mut dyn Host) -> bool {
        let Some((hosted, branch)) = Self::hosted_and_branch(id, cx) else {
            return false;
        };
        if let Some(branch) = branch {
            let url = corvene_hosts::urls::branch_url(hosted.kind, &hosted.repo, &branch);
            Self::open_url(&url, cx);
        }
        true
    }

    /// Repository › Create Issue on <host> (on the fork parent, as on GitHub).
    pub(crate) fn create_issue_on_host(id: u64, cx: &mut dyn Host) -> bool {
        let target = {
            let s = Self::state(cx).read(cx);
            s.hosted_repository(id)
                .map(|h| (h.kind, s.pull_request_repository(id).unwrap_or(h.repo)))
        };
        let Some((kind, repo)) = target else {
            return false;
        };
        Self::open_url(&corvene_hosts::urls::new_issue_url(kind, &repo), cx);
        true
    }

    /// `_openCreatePullRequestInBrowser` for a hosted repository: the host's
    /// new pull (merge) request form, from the current branch into `base`
    /// (the target's default branch when `None`).
    pub(crate) fn open_create_host_pull_request(
        id: u64,
        base: Option<String>,
        cx: &mut dyn Host,
    ) -> bool {
        let Some((hosted, Some(branch))) = Self::hosted_and_branch(id, cx) else {
            return false;
        };
        let target = Self::state(cx)
            .read(cx)
            .pull_request_repository(id)
            .unwrap_or_else(|| hosted.repo.clone());
        let fork = (target != hosted.repo).then_some(&hosted.repo);
        // the base is a remote branch name in the dialog; the host wants it bare
        let base = base.map(|b| match b.split_once('/') {
            Some((_, name)) => name.to_string(),
            None => b,
        });
        let url = corvene_hosts::urls::new_pull_request_url(
            hosted.kind,
            &target,
            fork,
            &branch,
            base.as_deref(),
        );
        Self::open_url(&url, cx);
        true
    }
}

/// The dialog's message for a failed sign-in.
fn sign_in_error_message(err: &HostError) -> String {
    match err {
        HostError::Api { status: 401, .. } => {
            "The server did not accept the token. Check that it was copied whole and has not expired.".into()
        }
        HostError::Api {
            status: 403,
            message,
        } => format!("The token lacks a permission Corvene needs: {message}"),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oauth_app_prefers_the_typed_id() {
        let gl = HostEndpoint::public(HostKind::GitLab);
        let typed = Dispatcher::host_oauth_app(&gl, Some(" abc ")).unwrap();
        assert_eq!(typed.client_id, "abc");
        let bb = HostEndpoint::public(HostKind::Bitbucket);
        // a Bitbucket consumer needs its secret, which only a build has
        assert!(Dispatcher::host_oauth_app(&bb, Some("key")).is_none());
    }
}
