//! Corvene (`527-multiple-accounts`): "Choose an account", asked once when
//! several signed-in accounts of a repository's host can push to it (or
//! see it while it is private). The pick (`Popup::ChooseRepositoryAccount`'s
//! first login) is already in use; Cancel keeps it, Use Account switches.
//! GHD has one account per host (`app/src/lib/stores/accounts-store.ts`),
//! so it never asks.

use corvene_core::{Account, AppState, Dispatcher};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::mac_or;
use crate::dialog::{GroupButtonSpec, OkCancelButtonGroup, dialog};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::radio;

pub struct ChooseRepositoryAccountDialog {
    state: Entity<AppState>,
    repo: u64,
    logins: Vec<String>,
    picked: usize,
}

impl ChooseRepositoryAccountDialog {
    pub fn new(state: Entity<AppState>, repo: u64, logins: Vec<String>) -> Self {
        Self {
            state,
            repo,
            logins,
            picked: 0,
        }
    }

    /// The offered logins that are still signed in, as accounts.
    fn accounts(&self, cx: &App) -> Vec<Account> {
        let s = self.state.read(cx);
        let Some(endpoint) = s
            .repository(self.repo)
            .and_then(|r| r.github.as_ref())
            .map(|gh| gh.endpoint.clone())
        else {
            return Vec::new();
        };
        self.logins
            .iter()
            .filter_map(|login| s.account_with_login(&endpoint, login).cloned())
            .collect()
    }
}

impl Render for ChooseRepositoryAccountDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let t = cx.ghd().clone();
        let accounts = self.accounts(cx);
        let picked = self.picked.min(accounts.len().saturating_sub(1));
        let (repo_name, host) = {
            let s = self.state.read(cx);
            let repo = s.repository(self.repo);
            (
                repo.and_then(|r| r.github.as_ref())
                    .map(|gh| gh.full_name())
                    .or_else(|| repo.map(|r| r.name()))
                    .unwrap_or_default(),
                accounts
                    .first()
                    .map(Account::friendly_endpoint)
                    .unwrap_or_default(),
            )
        };
        let this = cx.entity().downgrade();
        let rows = accounts.iter().enumerate().map(|(ix, account)| {
            let this = this.clone();
            let title = match &account.name {
                Some(name) if name != &account.login => format!("@{} ({name})", account.login),
                _ => format!("@{}", account.login),
            };
            div()
                .id(("choose-account-row", ix))
                .flex()
                .flex_row()
                .items_center()
                .gap(SPACING())
                .cursor_pointer()
                .on_click(move |_, _, cx| {
                    let _ = this.update(cx, |this, cx| {
                        this.picked = ix;
                        cx.notify();
                    });
                })
                .child(radio(("choose-account-radio", ix), ix == picked, cx))
                .child(crate::widgets::avatar_image(
                    account
                        .avatar_url
                        .as_deref()
                        .and_then(|u| crate::widgets::avatar_lookup_url(u, cx)),
                    zpx(24.),
                    cx,
                ))
                .child(div().flex_1().min_w_0().truncate().child(title))
        });
        let content = div()
            .flex()
            .flex_col()
            .gap(SPACING())
            .max_w(zpx(400.))
            .child(format!(
                "More than one of your {host} accounts has access to {repo_name}. Which one \
                 should this repository use for fetching, pushing and everything from GitHub?"
            ))
            .child(div().flex().flex_col().gap(SPACING_HALF()).children(rows))
            .child(
                div()
                    .text_size(FONT_SIZE_SM())
                    .text_color(t.text_secondary)
                    .child(mac_or(
                        "You can change it in Repository Settings › Remote.",
                        "You can change it in Repository settings › Remote.",
                    )),
            );
        let repo = self.repo;
        let login = accounts.get(picked).map(|a| a.login.clone());
        dialog(
            "dialog-choose-repository-account",
            mac_or("Choose an Account", "Choose an account"),
            content,
            OkCancelButtonGroup {
                destructive: false,
                cancel: GroupButtonSpec {
                    id: "choose-account-cancel",
                    label: "Cancel".into(),
                    disabled: false,
                    on_click: Box::new(close),
                },
                ok: GroupButtonSpec {
                    id: "choose-account-ok",
                    label: mac_or("Use Account", "Use account").into(),
                    disabled: login.is_none(),
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::close_popup(cx);
                        if let Some(login) = login.clone() {
                            Dispatcher::set_repository_account(repo, Some(login), cx);
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
