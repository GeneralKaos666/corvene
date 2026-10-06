//! Corvene `350-ssh-key-helper` dialogs: Create SSH Key (Settings ›
//! Integrations on the desktop) and the "add SSH keys" permission prompt.
//! GitHub Desktop has neither; it leaves keys to the terminal.
//!
//! Create SSH Key makes `~/.ssh/id_ed25519` with an optional passphrase,
//! hands it to `ssh-agent` (on macOS with `--apple-use-keychain`, and with a
//! passphrase optionally `UseKeychain` in `~/.ssh/config`) and adds it to a
//! signed-in GitHub account (`Dispatcher::create_ssh_key_with`).

use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use corvene_core::ssh_keys::SshKeyOptions;
use corvene_core::{AppState, Dispatcher};

use crate::context_menu::mac_or;
use crate::dialog::{GroupButtonSpec, OkCancelButtonGroup, dialog};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{checkbox_row, labeled, password_text_box, select_button, text_box};

pub struct CreateSshKeyDialog {
    passphrase: Entity<InputState>,
    confirm: Entity<InputState>,
    add_to_agent: bool,
    remember_in_keychain: bool,
    upload: bool,
    /// The GitHub accounts (API endpoint, "login on host"), GitHub.com
    /// first, and the one picked.
    accounts: Vec<(String, String)>,
    account: usize,
    title: Entity<InputState>,
}

impl CreateSshKeyDialog {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let masked = |placeholder: &'static str, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder(placeholder)
                    .masked(true)
            })
        };
        let passphrase = masked("Passphrase (optional)", window, cx);
        let confirm = masked("Passphrase again", window, cx);
        let title = cx.new(|cx| {
            InputState::new(window, cx).default_value(corvene_core::ssh_keys::default_key_title())
        });
        for input in [&passphrase, &confirm, &title] {
            cx.observe(input, |_, _, cx| cx.notify()).detach();
        }
        let mut accounts: Vec<(String, String)> = AppState::global(cx)
            .read(cx)
            .accounts
            .iter()
            .map(|a| {
                (
                    a.endpoint.clone(),
                    format!("{} on {}", a.login, a.friendly_endpoint()),
                )
            })
            .collect();
        accounts.sort_by_key(|(endpoint, _)| {
            !corvene_github::Endpoint::from_api_base(endpoint).is_dotcom()
        });
        let handle = passphrase.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
        Self {
            passphrase,
            confirm,
            add_to_agent: true,
            remember_in_keychain: true,
            upload: !accounts.is_empty(),
            accounts,
            account: 0,
            title,
        }
    }

    fn options(&self, cx: &App) -> SshKeyOptions {
        SshKeyOptions {
            passphrase: self.passphrase.read(cx).value().to_string(),
            add_to_agent: self.add_to_agent,
            remember_in_keychain: self.remember_in_keychain,
            upload: self
                .accounts
                .get(self.account)
                .filter(|_| self.upload)
                .map(|(endpoint, _)| {
                    let title = self.title.read(cx).value().trim().to_string();
                    let title = if title.is_empty() {
                        corvene_core::ssh_keys::default_key_title()
                    } else {
                        title
                    };
                    (endpoint.clone(), title)
                }),
        }
    }
}

impl Render for CreateSshKeyDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let passphrase = self.passphrase.read(cx).value().to_string();
        let mismatch = passphrase != self.confirm.read(cx).value();
        let toggle = |set: fn(&mut Self, bool), cx: &Context<Self>| {
            let weak = cx.weak_entity();
            move |on: bool, _: &mut Window, cx: &mut App| {
                weak.update(cx, |this, cx| {
                    set(this, on);
                    cx.notify();
                })
                .ok();
            }
        };
        let account_select = (self.accounts.len() > 1).then(|| {
            let weak = cx.weak_entity();
            select_button(
                "create-ssh-key-account",
                self.accounts[self.account].1.clone(),
                self.accounts
                    .iter()
                    .map(|(_, label)| SharedString::from(label.clone()))
                    .collect(),
                Some(self.account),
                !self.upload,
                std::rc::Rc::new(move |ix, _, cx| {
                    weak.update(cx, |this, cx| {
                        this.account = ix;
                        cx.notify();
                    })
                    .ok();
                }),
                cx,
            )
        });
        let upload_label = match self.accounts.first() {
            Some((_, label)) if self.accounts.len() == 1 => format!("Add it to GitHub as {label}"),
            _ => "Add it to GitHub".to_string(),
        };
        let content = div()
            .w(crate::theme::fit_width(440.))
            .flex()
            .flex_col()
            .gap(SPACING())
            .child(
                "Creates ~/.ssh/id_ed25519, the key ssh uses for remotes with an SSH address \
                 (git@…).",
            )
            .child(labeled(
                "Passphrase",
                password_text_box("create-ssh-key-passphrase", &self.passphrase, window, cx),
                cx,
            ))
            .child(password_text_box(
                "create-ssh-key-confirm",
                &self.confirm,
                window,
                cx,
            ))
            .when(mismatch, |d| {
                d.child(
                    div()
                        .text_size(FONT_SIZE_SM())
                        .text_color(t.error)
                        .child("The passphrases differ."),
                )
            })
            .when(!cfg!(windows) || passphrase.is_empty(), |d| {
                d.child(checkbox_row(
                    "create-ssh-key-agent",
                    self.add_to_agent,
                    if cfg!(target_os = "macos") && !passphrase.is_empty() {
                        "Add it to ssh-agent and its passphrase to the keychain"
                    } else {
                        "Add it to ssh-agent"
                    },
                    toggle(|this, on| this.add_to_agent = on, cx),
                    cx,
                ))
            })
            .when(cfg!(target_os = "macos") && !passphrase.is_empty(), |d| {
                d.child(checkbox_row(
                    "create-ssh-key-keychain",
                    self.remember_in_keychain,
                    "Use the keychain's passphrase after a restart (UseKeychain in ~/.ssh/config)",
                    toggle(|this, on| this.remember_in_keychain = on, cx),
                    cx,
                ))
            })
            .when(!self.accounts.is_empty(), |d| {
                d.child(checkbox_row(
                    "create-ssh-key-upload",
                    self.upload,
                    upload_label,
                    toggle(|this, on| this.upload = on, cx),
                    cx,
                ))
                .when_some(account_select, |d, select| d.child(select))
                .when(self.upload, |d| {
                    d.child(labeled(
                        "Title on GitHub",
                        text_box("create-ssh-key-title", &self.title, None, window, cx),
                        cx,
                    ))
                })
            });
        let weak = cx.weak_entity();
        dialog(
            "create-ssh-key",
            mac_or("Create SSH Key", "Create SSH key"),
            content,
            OkCancelButtonGroup {
                destructive: false,
                cancel: GroupButtonSpec {
                    id: "create-ssh-key-cancel",
                    label: "Cancel".into(),
                    disabled: false,
                    on_click: Box::new(close),
                },
                ok: GroupButtonSpec {
                    id: "create-ssh-key-ok",
                    label: mac_or("Create Key", "Create key").into(),
                    disabled: mismatch,
                    on_click: Box::new(move |_, cx| {
                        if let Ok(options) = weak.update(cx, |this, cx| this.options(cx)) {
                            Dispatcher::create_ssh_key_with(options, cx);
                        }
                    }),
                },
            }
            .into_buttons(),
            close,
            window,
            cx,
        )
    }
}

/// The account may not add SSH keys: sign in again, asking for the
/// `write:public_key` permission as well.
pub struct SshKeyNeedsScopeDialog {
    endpoint: String,
    login: String,
    title: String,
}

impl SshKeyNeedsScopeDialog {
    pub fn new(endpoint: String, login: String, title: String) -> Self {
        Self {
            endpoint,
            login,
            title,
        }
    }
}

impl Render for SshKeyNeedsScopeDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let (endpoint, title) = (self.endpoint.clone(), self.title.clone());
        let content = div()
            .w(crate::theme::fit_width(400.))
            .flex()
            .flex_col()
            .gap(SPACING())
            .child(format!(
                "Corvene's sign-in as {} may not add SSH keys to the account. Sign in again to \
                 allow it (the write:public_key permission); the key is added once you are \
                 signed in.",
                self.login
            ));
        dialog(
            "ssh-key-needs-scope",
            mac_or("Allow Adding SSH Keys", "Allow adding SSH keys"),
            content,
            OkCancelButtonGroup {
                destructive: false,
                cancel: GroupButtonSpec {
                    id: "ssh-key-needs-scope-cancel",
                    label: "Cancel".into(),
                    disabled: false,
                    on_click: Box::new(close),
                },
                ok: GroupButtonSpec {
                    id: "ssh-key-needs-scope-ok",
                    label: mac_or("Sign In", "Sign in").into(),
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::sign_in_for_ssh_key(endpoint.clone(), title.clone(), cx)
                    }),
                },
            }
            .into_buttons(),
            close,
            window,
            cx,
        )
    }
}
