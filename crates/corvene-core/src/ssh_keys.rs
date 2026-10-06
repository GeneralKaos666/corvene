//! Corvene `350-ssh-key-helper`: Settings › Integrations' SSH key section
//! on the desktop (Android always has it, for its bundled ssh). GitHub
//! Desktop leaves SSH keys to the terminal and github.com (desktop#2579).
//!
//! The key itself is `corvene_platform::ssh_key`'s: the first of
//! `~/.ssh/id_{ed25519,ecdsa,rsa}`, created as `id_ed25519` when there is
//! none ([`Dispatcher::create_ssh_key_with`]), never replaced. The desktop
//! hands it to `ssh-agent` (`ssh-add --apple-use-keychain` on macOS) and
//! adds it to a GitHub account with `POST /user/keys`
//! ([`Dispatcher::upload_ssh_key`]). That needs the `write:public_key`
//! scope, which Corvene's sign-in does not ask for: a token without it
//! (`X-OAuth-Scopes`) gets `Popup::SshKeyNeedsScope`, whose Sign In asks
//! for the scope as well ([`AppState::extra_oauth_scopes`]) and uploads
//! once the sign-in succeeds.
//!
//! [`AppState::extra_oauth_scopes`]: crate::state::AppState::extra_oauth_scopes

use corvene_github::Client;
use tracing::info;

use crate::dispatcher::Dispatcher;
use crate::host::{AsyncCtx, Host};
use crate::remote::spawn_bg;
use crate::sign_in::SignInResult;
use crate::state::Popup;

/// What the section shows under the key: work in progress or how the last
/// step went.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SshKeyState {
    /// What runs now ("Creating the key…"); the buttons wait.
    pub busy: Option<String>,
    /// The last outcome: `(succeeded, text)`.
    pub status: Option<(bool, String)>,
}

/// The Create SSH Key dialog's choices.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SshKeyOptions {
    /// Empty: no passphrase.
    pub passphrase: String,
    /// Hand the key to `ssh-agent`.
    pub add_to_agent: bool,
    /// macOS, with a passphrase: `UseKeychain` in `~/.ssh/config`, so ssh
    /// finds the passphrase in the keychain after a restart.
    pub remember_in_keychain: bool,
    /// Add it to the GitHub account at this API endpoint, named `title`.
    pub upload: Option<(String, String)>,
}

/// The title GitHub lists a key under: "Corvene on <computer>".
pub fn default_key_title() -> String {
    let host = std::process::Command::new("hostname")
        .arg("-s")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|h| !h.is_empty());
    match host {
        Some(host) => format!("Corvene on {host}"),
        None => "Corvene".to_string(),
    }
}

/// How a GitHub answer to adding a key reads in the section.
fn upload_error(err: &corvene_github::GitHubError) -> String {
    match err {
        corvene_github::GitHubError::Api { status: 422, .. } => {
            "GitHub already has this key (on this account or another one).".to_string()
        }
        other => format!("Could not add the key to GitHub: {other}"),
    }
}

enum Upload {
    Added,
    NeedsScope,
}

impl Dispatcher {
    fn set_ssh_key_state(busy: Option<&str>, status: Option<(bool, String)>, cx: &mut dyn Host) {
        Self::state(cx).update(cx, |s, cx| {
            s.ssh_key = SshKeyState {
                busy: busy.map(str::to_string),
                status,
            };
            cx.notify();
        });
    }

    /// Create SSH Key: `ssh-keygen`, then (as chosen) `ssh-add` and the
    /// upload. Failures after the key exists leave it in place.
    pub fn create_ssh_key_with(options: SshKeyOptions, cx: &mut dyn Host) {
        Self::close_popup_if(|p| matches!(p, Popup::CreateSshKey), cx);
        let git = Self::state(cx).read(cx).git.clone();
        Self::set_ssh_key_state(Some("Creating the key…"), None, cx);
        let upload = options.upload.clone();
        spawn_bg(
            cx,
            move || {
                let comment = git
                    .and_then(|git| corvene_git::global_config_value(git, "user.email"))
                    .unwrap_or_else(|| "corvene".to_string());
                let passphrase = Some(options.passphrase.as_str()).filter(|p| !p.is_empty());
                corvene_platform::ssh_key::create_ssh_key(&comment, passphrase)?;
                let mut notes = Vec::new();
                if options.add_to_agent
                    && let Some(file) = corvene_platform::ssh_key::ssh_key_file()
                    && let Err(err) = corvene_platform::ssh_key::add_to_agent(&file, passphrase)
                {
                    notes.push(format!("Could not add it to ssh-agent: {err}"));
                }
                if cfg!(target_os = "macos")
                    && options.remember_in_keychain
                    && passphrase.is_some()
                    && let Err(err) = corvene_platform::ssh_key::remember_in_keychain()
                {
                    notes.push(format!("Could not update ~/.ssh/config: {err}"));
                }
                Ok::<_, String>(notes)
            },
            move |result, cx| match result {
                Err(err) => {
                    Self::set_ssh_key_state(None, None, cx);
                    Self::show_error("Could not create an SSH key", err, cx);
                }
                Ok(notes) => {
                    info!("ssh key created");
                    let text = if notes.is_empty() {
                        "Created ~/.ssh/id_ed25519.".to_string()
                    } else {
                        format!("Created ~/.ssh/id_ed25519. {}", notes.join(" "))
                    };
                    Self::set_ssh_key_state(None, Some((notes.is_empty(), text)), cx);
                    if let Some((endpoint, title)) = upload {
                        Self::upload_ssh_key(endpoint, title, cx);
                    }
                }
            },
        );
    }

    /// Add to ssh-agent: `ssh-add` the key in use (a key with a passphrase
    /// fails here; Create SSH Key adds a new one with its passphrase).
    pub fn add_ssh_key_to_agent(cx: &mut dyn Host) {
        Self::set_ssh_key_state(Some("Adding the key to ssh-agent…"), None, cx);
        spawn_bg(
            cx,
            || {
                let file = corvene_platform::ssh_key::ssh_key_file()
                    .ok_or_else(|| "no home directory".to_string())?;
                corvene_platform::ssh_key::add_to_agent(&file, None)
            },
            |result, cx| {
                let status = match result {
                    Ok(()) => (true, "Added to ssh-agent.".to_string()),
                    Err(err) => (false, format!("Could not add it to ssh-agent: {err}")),
                };
                Self::set_ssh_key_state(None, Some(status), cx);
            },
        );
    }

    /// Add to GitHub: `POST /user/keys` with the account at `endpoint`,
    /// after checking that its token may (`write:public_key`). `endpoint`
    /// may name the login too (`Account::key`, `endpoint|login`) when the
    /// endpoint has several accounts (`527-multiple-accounts`).
    pub fn upload_ssh_key(endpoint: String, title: String, cx: &mut dyn Host) {
        let Some(key) = corvene_platform::ssh_key::ssh_public_key() else {
            return;
        };
        let (endpoint, login) = match endpoint.split_once('|') {
            Some((endpoint, login)) => (endpoint.to_string(), Some(login.to_string())),
            None => (endpoint, None),
        };
        let account = {
            let s = Self::state(cx).read(cx);
            match &login {
                Some(login) => s.account_with_login(&endpoint, login),
                None => s.account_for(&endpoint),
            }
            .cloned()
        };
        let Some(account) = account else {
            Self::show_error("Could not add the SSH key", "Sign in to GitHub first.", cx);
            return;
        };
        let Some(token) = corvene_platform::keychain::token(&account.host(), &account.login)
            .ok()
            .flatten()
        else {
            Self::show_error(
                "Could not add the SSH key",
                format!(
                    "The sign-in of {} is missing; sign in again.",
                    account.login
                ),
                cx,
            );
            return;
        };
        Self::set_ssh_key_state(
            Some(&format!(
                "Adding the key to {}…",
                account.friendly_endpoint()
            )),
            None,
            cx,
        );
        let client = Client::new(corvene_github::Endpoint::from_api_base(&endpoint), token);
        let login = account.login.clone();
        let task_title = title.clone();
        spawn_bg(
            cx,
            move || {
                // a classic OAuth token lists its scopes; a fine-grained one
                // does not, and is only refused by the call itself
                if let Some(scopes) = client.token_scopes()?
                    && !corvene_github::allows_adding_ssh_keys(&scopes)
                {
                    return Ok(Upload::NeedsScope);
                }
                match client.add_ssh_key(&task_title, &key) {
                    Ok(()) => Ok(Upload::Added),
                    Err(corvene_github::GitHubError::Api {
                        status: 403 | 404, ..
                    }) => Ok(Upload::NeedsScope),
                    Err(err) => Err(err),
                }
            },
            move |result, cx| match result {
                Ok(Upload::Added) => {
                    info!(%login, "ssh key added to the account");
                    Self::set_ssh_key_state(
                        None,
                        Some((true, format!("Added to GitHub as {login}."))),
                        cx,
                    );
                }
                Ok(Upload::NeedsScope) => {
                    Self::set_ssh_key_state(None, None, cx);
                    Self::show_popup(
                        Popup::SshKeyNeedsScope {
                            endpoint,
                            login,
                            title,
                        },
                        cx,
                    );
                }
                Err(err) => Self::set_ssh_key_state(None, Some((false, upload_error(&err))), cx),
            },
        );
    }

    /// `SshKeyNeedsScope` › Sign In: the sign-in dialog for the account's
    /// endpoint, asking for `write:public_key` too; once it succeeds
    /// Settings › Integrations opens again and the upload runs, its outcome
    /// under the key.
    pub fn sign_in_for_ssh_key(endpoint: String, title: String, cx: &mut dyn Host) {
        Self::close_popup_if(|p| matches!(p, Popup::SshKeyNeedsScope { .. }), cx);
        let enterprise = !corvene_github::Endpoint::from_api_base(&endpoint).is_dotcom();
        Self::state(cx).update(cx, |s, _| {
            s.extra_oauth_scopes = vec![corvene_github::PUBLIC_KEY_SCOPE.to_string()];
        });
        let async_cx = cx.async_ctx();
        let callback: crate::sign_in::ResultCallback = Box::new(move |result| {
            if let SignInResult::Success { account } = result {
                // the account that signed in, among several on the
                // endpoint (`527-multiple-accounts`)
                let endpoint = account.key();
                let title = title.clone();
                // the store calls back in the middle of an `AppState` update
                async_cx
                    .spawn(async move |cx: &mut AsyncCtx| {
                        cx.update(|cx| {
                            Self::open_preferences(crate::state::PreferencesTab::Integrations, cx);
                            Self::upload_ssh_key(endpoint, title, cx)
                        });
                    })
                    .detach();
            }
        });
        Self::show_sign_in_dialog(enterprise, Some(callback), cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_a_duplicate_key() {
        let err = corvene_github::GitHubError::api(422, "key is already in use");
        assert!(upload_error(&err).contains("already has this key"));
    }
}
