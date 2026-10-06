//! Corvene (flag `527-multiple-accounts`): the account each repository
//! uses when its host has several signed-in accounts.
//!
//! GHD 3.6.6 keeps one account per endpoint (`accounts-store.ts`
//! `addAccount`) and every lookup takes the endpoint's account
//! (`lib/get-account-for-repository.ts`, `lib/trampoline/find-account.ts`).
//! Here a repository stores the login it uses (`Repository::account`):
//!
//! - a clone from an account's repository list, or a URL only one account
//!   can see, keeps that account (`account_when_added`);
//! - otherwise, once the repository is added (and for repositories added
//!   before a second account signed in, the first time their GitHub
//!   repository is refreshed), every account of the host is asked for the
//!   repository and `accounts::choose_repository_account` picks one; when
//!   several can push to it (or see it while it is private) the user is
//!   asked once (`Popup::ChooseRepositoryAccount`);
//! - Repository Settings › Remote and the toolbar's account button change
//!   it (`set_repository_account`), effective at once: the askpass
//!   environment and API calls read it each time.

use std::collections::HashSet;
use std::path::Path;

use corvene_github::{Client, Endpoint};
use corvene_models::{Account, GitHubRepository};
use tracing::{debug, info, warn};

use crate::accounts::{RepositoryAccess, choose_repository_account};
use crate::host::Host;
use crate::remote::spawn_bg;
use crate::state::Popup;
use crate::{Dispatcher, dispatcher::persist_repositories};

/// Repositories whose account was looked up this session (at most once
/// each, even when the lookup failed).
#[derive(Default)]
pub struct AccountProbes {
    pub probed: HashSet<u64>,
}

impl Dispatcher {
    /// Remember that the repository cloned or added at `path` uses `login`'s
    /// account (the Clone dialog's account, or the one that could see the
    /// URL), so it is not looked up again once added.
    pub fn account_when_added(path: &Path, login: String, cx: &mut dyn Host) {
        let path = crate::dispatcher::resolve_path(path);
        Self::state(cx).update(cx, |s, _| {
            if !s.multiple_accounts() {
                return;
            }
            s.pending_accounts
                .retain(|(p, _)| !crate::dispatcher::same_path(p, &path));
            s.pending_accounts.push((path, login));
        });
    }

    /// Remove and return the login waiting for `path`.
    pub(crate) fn take_pending_account(path: &Path, cx: &mut dyn Host) -> Option<String> {
        Self::state(cx).update(cx, |s, _| {
            let ix = s
                .pending_accounts
                .iter()
                .position(|(p, _)| crate::dispatcher::same_path(p, path))?;
            Some(s.pending_accounts.remove(ix).1)
        })
    }

    /// The login waiting for `path`, left in place (the clone's askpass).
    pub(crate) fn pending_account(path: &Path, cx: &dyn Host) -> Option<String> {
        Self::state(cx)
            .read(cx)
            .pending_accounts
            .iter()
            .find(|(p, _)| crate::dispatcher::same_path(p, path))
            .map(|(_, login)| login.clone())
    }

    /// A repository was added (or picked again): give it the pending account
    /// for its path, else look its account up.
    pub(crate) fn settle_repository_account(id: u64, pending: Option<String>, cx: &mut dyn Host) {
        match pending {
            Some(login) => {
                let changed = Self::state(cx).update(cx, |s, cx| {
                    let Some(repo) = s.repositories.iter_mut().find(|r| r.id == id) else {
                        return false;
                    };
                    if repo.account.as_deref() == Some(login.as_str()) {
                        return false;
                    }
                    repo.account = Some(login);
                    persist_repositories(s);
                    cx.notify();
                    true
                });
                if changed {
                    Self::apply_account_commit_email(id, cx);
                }
            }
            None => Self::resolve_repository_account(id, cx),
        }
    }

    /// Whether repository `id` still needs its account looked up: the flag
    /// is on, it has none, its GitHub host has several accounts and it was
    /// not looked up this session.
    pub(crate) fn needs_account_lookup(s: &crate::state::AppState, id: u64) -> bool {
        s.multiple_accounts()
            && !s.account_probes.probed.contains(&id)
            && s.repository(id).is_some_and(|r| {
                r.account.is_none()
                    && r.github
                        .as_ref()
                        .is_some_and(|gh| s.accounts_for(&gh.endpoint).len() > 1)
            })
    }

    /// Look up which of the host's accounts repository `id` uses (see the
    /// module docs). Without several accounts, or with an account set, it
    /// does nothing.
    pub fn resolve_repository_account(id: u64, cx: &mut dyn Host) {
        let (github, accounts) = {
            let s = Self::state(cx).read(cx);
            if !Self::needs_account_lookup(s, id) {
                return;
            }
            let Some(github) = s.repository(id).and_then(|r| r.github.clone()) else {
                return;
            };
            let accounts: Vec<Account> = s
                .accounts_for(&github.endpoint)
                .into_iter()
                .cloned()
                .collect();
            (github, accounts)
        };
        Self::state(cx).update(cx, |s, _| {
            s.account_probes.probed.insert(id);
        });
        info!(id, repo = %github.full_name(), "looking up the repository's account");
        let owner = github.owner.clone();
        spawn_bg(
            cx,
            move || probe_accounts(&github, &accounts),
            move |access, cx| {
                // none could ask (offline, tokens missing): the host's first
                // account stands in until the next session looks again
                if !access.iter().any(|a| a.found) {
                    info!(
                        id,
                        "no account could read the repository, its account stays open"
                    );
                    return;
                }
                let Some(choice) = choose_repository_account(&owner, &access) else {
                    return;
                };
                let set = Self::state(cx).update(cx, |s, cx| {
                    let Some(repo) = s.repositories.iter_mut().find(|r| r.id == id) else {
                        return false;
                    };
                    // the user picked one meanwhile
                    if repo.account.is_some() {
                        return false;
                    }
                    info!(id, login = %choice.login, ask = choice.ask, "repository account");
                    repo.account = Some(choice.login.clone());
                    persist_repositories(s);
                    cx.notify();
                    true
                });
                if !set {
                    return;
                }
                Self::apply_account_commit_email(id, cx);
                Self::refresh_github_repository(id, cx);
                if choice.ask {
                    Self::show_popup(
                        Popup::ChooseRepositoryAccount {
                            repo: id,
                            logins: choice.candidates,
                        },
                        cx,
                    );
                }
            },
        );
    }

    /// Repository Settings › Remote › Account, the toolbar's account menu,
    /// ChooseRepositoryAccount: repository `id` uses `login`'s account from
    /// now on (`None`: the host's first). A local `user.email` that was the
    /// old account's commit email (`525-account-commit-email`) follows.
    pub fn set_repository_account(id: u64, login: Option<String>, cx: &mut dyn Host) {
        let old = Self::state(cx).read(cx).account_for_repository(id).cloned();
        let changed = Self::state(cx).update(cx, |s, cx| {
            s.account_probes.probed.insert(id);
            let Some(repo) = s.repositories.iter_mut().find(|r| r.id == id) else {
                return false;
            };
            if repo.account == login {
                return false;
            }
            repo.account = login;
            persist_repositories(s);
            cx.notify();
            true
        });
        if !changed {
            return;
        }
        let new = Self::state(cx).read(cx).account_for_repository(id).cloned();
        info!(
            id,
            from = ?old.as_ref().map(|a| &a.login),
            to = ?new.as_ref().map(|a| &a.login),
            "repository account changed"
        );
        Self::swap_account_commit_email(id, old.as_ref(), new.as_ref(), cx);
        // what the other account sees
        Self::refresh_github_repository(id, cx);
        Self::refresh_pull_requests(id, true, cx);
    }

    /// `525-account-commit-email`: a local `user.email` that is `old`'s
    /// commit email becomes `new`'s (or is unset when `new` has none).
    fn swap_account_commit_email(
        id: u64,
        old: Option<&Account>,
        new: Option<&Account>,
        cx: &mut dyn Host,
    ) {
        let s = Self::state(cx).read(cx);
        if !s.flags.bool(crate::flags::ids::ACCOUNT_COMMIT_EMAIL) {
            return;
        }
        let email_of = |a: Option<&Account>| {
            a.and_then(|a| {
                s.settings
                    .account_commit_emails
                    .get(&a.key())
                    .map(|e| e.trim().to_string())
                    .filter(|e| !e.is_empty())
            })
        };
        let (Some(old_email), new_email) = (email_of(old), email_of(new)) else {
            return;
        };
        let (Some(git), Some(path)) = (s.git.clone(), s.repository(id).map(|r| r.path.clone()))
        else {
            return;
        };
        spawn_bg(
            cx,
            move || {
                let local = corvene_git::local_config_value(git.clone(), &path, "user.email");
                if local.as_deref().map(str::trim) != Some(old_email.as_str()) {
                    return Ok(false);
                }
                match new_email {
                    Some(email) => {
                        corvene_git::set_local_config_value(git, &path, "user.email", &email)
                    }
                    None => corvene_git::remove_local_config_value(git, &path, "user.email"),
                }
                .map(|_| true)
            },
            move |result, _| match result {
                Ok(true) => info!(id, "the commit email followed the account"),
                Ok(false) => {}
                Err(err) => warn!(id, %err, "could not change the commit email"),
            },
        );
    }
}

/// Ask every account about `github` with its own token (background).
fn probe_accounts(github: &GitHubRepository, accounts: &[Account]) -> Vec<RepositoryAccess> {
    accounts
        .iter()
        .map(|account| {
            let mut access = RepositoryAccess {
                login: account.login.clone(),
                ..RepositoryAccess::default()
            };
            let Some(token) = corvene_platform::keychain::token(&account.host(), &account.login)
                .ok()
                .flatten()
            else {
                return access;
            };
            let client = Client::new(Endpoint::from_api_base(&account.endpoint), token);
            match client.repository(&github.owner, &github.name) {
                Ok(repo) => {
                    access.found = true;
                    access.private = repo.private;
                    access.permission = repo.permissions;
                }
                Err(err) => {
                    debug!(login = %account.login, %err, "account cannot see the repository")
                }
            }
            if !account.login.eq_ignore_ascii_case(&github.owner) {
                access.orgs = client.user_orgs().unwrap_or_default();
            }
            access
        })
        .collect()
}
