//! GHD `CommitMessageAvatar` (`app/src/ui/changes/commit-message-avatar.tsx`,
//! `styles/ui/_commit-message-avatar.scss`; the warning comes from
//! `renderAvatar` in `ui/changes/commit-message.tsx`): the commit form's
//! avatar is a button. Without a warning its popover says who commits and
//! opens the Git settings; when the email is not one of the repository
//! account's ("misattribution") or a repository rule disallows it, a badge
//! sits on the avatar and the popover offers the account's emails with
//! Ignore / Update Email (the global `user.email`).
//!
//! Deviation (`1309-commit-identity-update`): the warning popover also has a
//! Name box (the account's name, else `user.name`), Update writes the name
//! with the email, both go to the repository's own config when it already
//! has a local identity (GHD writes the global email, which a local one
//! hides), and the text names that config.

use std::cell::Cell;
use std::rc::Rc;

use corvene_core::{
    AppState, Dispatcher, GitHubRepository, PreferencesTab, RepoRulesMetadataFailures,
    RepoRulesMetadataStatus, RepositorySettingsTab,
};
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::changes::{repo_ruleset_link, repo_rulesets_for_branch_link};
use crate::context_menu::mac_or;
use crate::icons::{Octicon, octicon};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{
    IconButtonA11y, SelectHandler, avatar_image, avatar_lookup, button, labeled, link_button,
    paragraph, primary_button, select_button, text_box,
};

/// GHD's "Learn more" link of the misattribution warning.
const ATTRIBUTION_HELP_URL: &str = "https://docs.github.com/en/github/committing-changes-to-your-project/why-are-my-commits-linked-to-the-wrong-user";

/// `CommitMessageAvatarWarningType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Warning {
    None,
    Misattribution,
    DisallowedEmail,
}

/// What the avatar shows for the selected repository.
struct Model {
    repo: u64,
    name: Option<String>,
    email: Option<String>,
    /// `isGitConfigLocal`
    local: bool,
    warning: Warning,
    email_failures: RepoRulesMetadataFailures,
    github: Option<GitHubRepository>,
    branch: Option<String>,
    enterprise: bool,
    /// `accountEmails`: the account's addresses and its no-reply address.
    account_emails: Vec<String>,
    /// `lookupPreferredEmail`
    preferred_email: String,
    account_name: Option<String>,
    /// `1309-commit-identity-update`
    edit_identity: bool,
}

pub struct CommitMessageAvatar {
    state: Entity<AppState>,
    open: bool,
    /// The address picked in "Your Account Emails" (`None`: the preferred).
    account_email: Option<String>,
    /// `1309-commit-identity-update`: the warning popover's Name box.
    name: Entity<InputState>,
    avatar_bounds: Rc<Cell<Bounds<Pixels>>>,
    badge_bounds: Rc<Cell<Bounds<Pixels>>>,
    /// The open balloon takes focus, so Escape closes it (GHD `Popover`).
    focus: FocusHandle,
}

impl CommitMessageAvatar {
    /// The commit form renders this view, so its own state observer
    /// redraws it.
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            state,
            open: false,
            account_email: None,
            name: cx.new(|cx| InputState::new(window, cx)),
            avatar_bounds: Rc::default(),
            badge_bounds: Rc::default(),
            focus: cx.focus_handle(),
        }
    }

    fn model(&self, cx: &App) -> Option<Model> {
        let s = self.state.read(cx);
        let repo = s.selected?;
        let rs = s.repo_states.get(&repo)?;
        let info = rs.info.as_ref()?;
        let identity = &info.identity;
        let github = s.repository(repo).and_then(|r| r.github.clone());
        let account = s.account_for_repository(repo);
        let mut account_emails: Vec<String> = account.map(|a| a.emails.clone()).unwrap_or_default();
        if let Some(a) = account.filter(|a| a.is_dotcom()) {
            let stealth = corvene_core::stealth_email(a.id, &a.login, &a.endpoint);
            if !account_emails
                .iter()
                .any(|e| e.eq_ignore_ascii_case(&stealth))
            {
                account_emails.push(stealth);
            }
        }
        let email_failures = match (&github, identity.email.as_deref()) {
            (Some(_), Some(email)) => {
                corvene_core::failed_rules(&rs.repo_rules.commit_author_email_patterns, email)
            }
            _ => RepoRulesMetadataFailures::default(),
        };
        let warning = match identity.email.as_deref() {
            None => Warning::None,
            Some(_) if email_failures.status() != RepoRulesMetadataStatus::Pass => {
                Warning::DisallowedEmail
            }
            Some(email) if account.is_some_and(|a| !a.is_attributable_email(email)) => {
                Warning::Misattribution
            }
            Some(_) => Warning::None,
        };
        Some(Model {
            repo,
            name: identity.name.clone(),
            email: identity.email.clone(),
            local: identity.local,
            warning,
            email_failures,
            github,
            branch: info.current_branch().map(|b| b.name.clone()),
            enterprise: account.is_some_and(|a| !a.is_dotcom()),
            account_emails,
            preferred_email: account.map(|a| a.preferred_email()).unwrap_or_default(),
            account_name: account
                .and_then(|a| a.name.clone())
                .filter(|n| !n.is_empty()),
            edit_identity: s
                .flags
                .bool(corvene_core::flags::ids::COMMIT_IDENTITY_UPDATE),
        })
    }

    /// `onAvatarClick`: the popover opens with the preferred address picked
    /// and, for `1309-commit-identity-update`, the Name box filled.
    fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open = !self.open;
        if self.open {
            self.account_email = None;
            if let Some(model) = self.model(cx) {
                let name = model.account_name.or(model.name).unwrap_or_default();
                self.name.update(cx, |s, cx| s.set_value(name, window, cx));
            }
            window.focus(&self.focus, cx);
        }
        cx.notify();
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        if self.open {
            self.open = false;
            cx.notify();
        }
    }

    fn open_repository_settings(&mut self, repo: u64, cx: &mut Context<Self>) {
        self.close(cx);
        Dispatcher::open_repository_settings(repo, RepositorySettingsTab::GitConfig, cx);
    }

    /// `onUpdateEmailClick`
    fn update(&mut self, model: &Model, cx: &mut Context<Self>) {
        self.close(cx);
        let email = self
            .account_email
            .clone()
            .unwrap_or_else(|| model.preferred_email.clone());
        let email_changed = model.email.as_deref() != Some(email.as_str());
        if model.edit_identity {
            let name = self.name.read(cx).value().trim().to_string();
            let name_changed = !name.is_empty() && model.name.as_deref() != Some(name.as_str());
            if email_changed || name_changed {
                Dispatcher::update_commit_identity(
                    model.repo,
                    name_changed.then_some(name),
                    if email_changed { email } else { String::new() },
                    model.local,
                    cx,
                );
            }
        } else if email_changed {
            Dispatcher::update_commit_identity(model.repo, None, email, false, cx);
        }
    }

    /// `.warning-badge`: 18 px over the avatar's top-left corner.
    fn badge(&self, model: &Model, cx: &App) -> impl IntoElement {
        let t = cx.ghd();
        let error = model.warning == Warning::DisallowedEmail
            && model.email_failures.status() == RepoRulesMetadataStatus::Fail;
        let bounds = self.badge_bounds.clone();
        div()
            .absolute()
            .top(zpx(-6.))
            .left(zpx(-7.))
            .size(zpx(18.))
            .rounded(zpx(9.))
            .border_1()
            .border_color(t.commit_warning_badge_border)
            .bg(t.commit_warning_badge_background)
            .flex()
            .items_center()
            .justify_center()
            .child(
                canvas(move |b, _, _| bounds.set(b), |_, _, _, _| {})
                    .absolute()
                    .inset_0(),
            )
            .child(
                if error {
                    octicon(Octicon::Stop, t.input_icon_error)
                } else {
                    octicon(Octicon::Alert, t.input_icon_warning)
                }
                .size(zpx(10.)),
            )
    }

    /// `.button-row`: right-aligned 120 px buttons.
    fn button_row(buttons: Vec<AnyElement>) -> Div {
        div().flex().flex_row().justify_end().children(
            buttons
                .into_iter()
                .map(|b| div().mr(SPACING_HALF()).child(b)),
        )
    }

    /// `renderGitConfigPopover`
    fn git_config_popover(&self, model: &Model, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let t = cx.ghd().clone();
        let local = model.local;
        let repo = model.repo;
        let settings = if local {
            "repository settings".to_string()
        } else {
            format!("git {}", mac_or("settings", "options"))
        };
        let p = |text: String| div().mt(FONT_SIZE()).child(text).into_any_element();
        let mut rows = vec![p(match (&model.name, &model.email) {
            (Some(_), Some(email)) => format!("Email: {email}"),
            _ => String::new(),
        })];
        rows.push(p(format!(
            "You can update your {} git configuration{} in your {settings}.",
            if local { "local" } else { "global" },
            if local { " for your repository" } else { "" },
        )));
        if !local {
            rows.push(
                div()
                    .mt(FONT_SIZE())
                    .text_color(t.text_secondary)
                    .child(paragraph(vec![
                        "You can also set an email local to this repository from the ".into(),
                        link_button("commit-avatar-repo-settings", "repository settings", cx)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.open_repository_settings(repo, cx)
                            }))
                            .into_any_element()
                            .into(),
                        ".".into(),
                    ]))
                    .into_any_element(),
            );
        }
        let cancel = button("commit-avatar-cancel", "Cancel", cx)
            .min_w(zpx(120.))
            .on_click(cx.listener(|this, _, _, cx| this.close(cx)))
            .into_any_element();
        let open = primary_button(
            "commit-avatar-open-settings",
            mac_or("Open Git Settings", "Open git settings"),
            false,
            cx,
        )
        .min_w(zpx(120.))
        .on_click(cx.listener(move |this, _, _, cx| {
            if local {
                this.open_repository_settings(repo, cx);
            } else {
                this.close(cx);
                Dispatcher::open_preferences(PreferencesTab::Git, cx);
            }
        }))
        .into_any_element();
        // `OkCancelButtonGroup`: Cancel first on macOS
        let buttons = if cfg!(target_os = "macos") {
            vec![cancel, open]
        } else {
            vec![open, cancel]
        };
        rows.push(Self::button_row(buttons).mt(FONT_SIZE()).into_any_element());
        rows
    }

    /// `renderWarningPopover`
    fn warning_popover(
        &self,
        model: Rc<Model>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let t = cx.ghd().clone();
        let repo = model.repo;
        let email = model.email.clone().unwrap_or_default();
        // `1309-commit-identity-update` names the config the email is in
        let config = if model.edit_identity && model.local {
            "repository's"
        } else {
            "global"
        };
        let header = format!("The email in your {config} Git config ({email})");
        let mut rows: Vec<AnyElement> = Vec::new();
        match model.warning {
            Warning::Misattribution => {
                let suffix = if model.enterprise { " Enterprise" } else { "" };
                let user = model
                    .name
                    .as_ref()
                    .map(|n| format!(" for {n}"))
                    .unwrap_or_default();
                rows.push(
                    paragraph(vec![
                        format!("{header} doesn't match your GitHub{suffix} account{user}. ")
                            .into(),
                        link_button("commit-avatar-learn-more", "Learn more", cx)
                            .on_click(|_, _, cx| Dispatcher::open_url(ATTRIBUTION_HELP_URL, cx))
                            .into_any_element()
                            .into(),
                    ])
                    .into_any_element(),
                );
            }
            Warning::DisallowedEmail => {
                rows.push(self.failure_list(&model, header, cx));
            }
            Warning::None => {}
        }
        let has_emails = !model.account_emails.is_empty();
        if has_emails {
            let selected = self
                .account_email
                .clone()
                .unwrap_or_else(|| model.preferred_email.clone());
            let ix = model.account_emails.iter().position(|e| *e == selected);
            let options = model
                .account_emails
                .iter()
                .cloned()
                .map(Into::into)
                .collect();
            let weak = cx.weak_entity();
            let emails = model.account_emails.clone();
            let on_select: SelectHandler = Rc::new(move |ix, _, cx| {
                let email = emails.get(ix).cloned();
                weak.update(cx, |this, cx| {
                    this.account_email = email;
                    cx.notify();
                })
                .ok();
            });
            rows.push(
                labeled(
                    "Your Account Emails",
                    select_button(
                        "commit-avatar-emails",
                        selected,
                        options,
                        ix,
                        false,
                        on_select,
                        cx,
                    ),
                    cx,
                )
                .into_any_element(),
            );
        }
        // `1309-commit-identity-update`
        if model.edit_identity && has_emails {
            rows.push(
                labeled(
                    "Name",
                    text_box("commit-avatar-name", &self.name, None, window, cx),
                    cx,
                )
                .into_any_element(),
            );
        }
        rows.push(
            div()
                .text_color(t.text_secondary)
                .child(paragraph(vec![
                    format!(
                        "You can{} choose an email local to this repository from the ",
                        if has_emails { " also" } else { "" }
                    )
                    .into(),
                    link_button("commit-avatar-repo-settings", "repository settings", cx)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_repository_settings(repo, cx)
                        }))
                        .into_any_element()
                        .into(),
                    ".".into(),
                ]))
                .into_any_element(),
        );
        let mut buttons = vec![
            button("commit-avatar-ignore", "Ignore", cx)
                .min_w(zpx(120.))
                .on_click(cx.listener(|this, _, _, cx| this.close(cx)))
                .into_any_element(),
        ];
        if has_emails {
            let label = if model.edit_identity {
                mac_or("Update", "Update")
            } else {
                mac_or("Update Email", "Update email")
            };
            buttons.push(
                primary_button("commit-avatar-update", label, false, cx)
                    .min_w(zpx(120.))
                    .on_click(cx.listener(move |this, _, _, cx| this.update(&model, cx)))
                    .into_any_element(),
            );
        }
        rows.push(Self::button_row(buttons).into_any_element());
        // `.row-component:not(:last-child) { margin-bottom: var(--spacing) }`
        let last = rows.len() - 1;
        rows.into_iter()
            .enumerate()
            .map(|(ix, row)| {
                div()
                    .when(ix == 0, |d| d.mt(FONT_SIZE()))
                    .when(ix < last, |d| d.mb(SPACING()))
                    .child(row)
                    .into_any_element()
            })
            .collect()
    }

    /// `RepoRulesMetadataFailureList` with the email header.
    fn failure_list(&self, model: &Model, header: String, cx: &App) -> AnyElement {
        let failures = &model.email_failures;
        let total = failures.failed.len() + failures.bypassed.len();
        let end = if failures.status() == RepoRulesMetadataStatus::Bypass {
            format!(
                ", but you can bypass {}. Proceed with caution!",
                if total == 1 { "it" } else { "them" }
            )
        } else {
            ".".to_string()
        };
        let all_url = repo_rulesets_for_branch_link(model.github.as_ref(), model.branch.as_deref())
            .unwrap_or_default();
        let github = model.github.clone();
        let list = |label: &'static str, items: &[corvene_core::RepoRulesMetadataFailure]| {
            let github = github.clone()?;
            (!items.is_empty()).then(|| {
                div()
                    .mt(FONT_SIZE())
                    .flex()
                    .flex_col()
                    .child(format!("{label} {}:", mac_or("Rules", "rules")))
                    .children(items.iter().enumerate().map(|(ix, f)| {
                        let url = repo_ruleset_link(&github, f.ruleset_id);
                        div()
                            .flex()
                            .flex_row()
                            .gap(SPACING_HALF())
                            .pl(SPACING_DOUBLE())
                            .child("•")
                            .child(
                                link_button(
                                    SharedString::from(format!("commit-avatar-rule-{label}-{ix}")),
                                    f.description.clone(),
                                    cx,
                                )
                                .on_click(move |_, _, cx| Dispatcher::open_url(&url, cx)),
                            )
                    }))
            })
        };
        div()
            .flex()
            .flex_col()
            .child(paragraph(vec![
                format!(
                    "{header} fails {total} rule{}{end} ",
                    if total > 1 { "s" } else { "" }
                )
                .into(),
                link_button(
                    "commit-avatar-rules-all",
                    "View all rulesets for this branch.",
                    cx,
                )
                .on_click(move |_, _, cx| Dispatcher::open_url(&all_url, cx))
                .into_any_element()
                .into(),
            ]))
            .children(list("Failed", &failures.failed))
            .children(list("Bypassed", &failures.bypassed))
            .into_any_element()
    }

    fn popover(&self, model: Rc<Model>, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let header: AnyElement = match model.warning {
            Warning::Misattribution => "This commit will be misattributed".into_any_element(),
            Warning::DisallowedEmail => "This email address is disallowed".into_any_element(),
            // `getCommittingAsTitle`
            Warning::None => match (&model.name, &model.email) {
                // `<strong>` in the bold `h3`: `font-weight: bolder`
                (Some(name), _) => {
                    let lead = "Committing as ";
                    StyledText::new(format!("{lead}{name}"))
                        .with_highlights([(
                            lead.len()..lead.len() + name.len(),
                            HighlightStyle {
                                font_weight: Some(FontWeight::BLACK),
                                ..Default::default()
                            },
                        )])
                        .into_any_element()
                }
                (None, Some(email)) => format!("Committing with {email}").into_any_element(),
                (None, None) => "Unknown user".into_any_element(),
            },
        };
        let body = if model.warning == Warning::None {
            self.git_config_popover(&model, cx)
        } else {
            self.warning_popover(model.clone(), window, cx)
        };
        let anchor = if model.warning == Warning::None {
            self.avatar_bounds.get()
        } else {
            self.badge_bounds.get()
        };
        let close = |this: &mut Self,
                     _: &MouseDownEvent,
                     _: &mut Window,
                     cx: &mut Context<Self>| { this.close(cx) };
        // a window-sized layer under the balloon closes it on any click
        // outside (`onMousedownOutside`)
        let overlay = deferred(
            anchored().position(point(zpx(0.), zpx(0.))).child(
                div()
                    .id("commit-avatar-popover-overlay")
                    .relative()
                    .size_full()
                    .on_mouse_down(MouseButton::Left, cx.listener(close))
                    .on_mouse_down(MouseButton::Right, cx.listener(close)),
            ),
        )
        .with_priority(25);
        div()
            .child(overlay)
            .child(
                crate::popover::balloon_popover_right_bottom(
                    anchor,
                    crate::popover::popover_component(cx)
                        .id("commit-avatar-popover")
                        .track_focus(&self.focus)
                        .on_action(
                            cx.listener(|this, _: &crate::actions::CloseFoldout, _, cx| {
                                this.close(cx)
                            }),
                        )
                        .occlude()
                        // `.popover-component { width: 300px }`,
                        // `.popover-content { padding: var(--spacing-double) }`
                        .w(zpx(300.))
                        .p(SPACING_DOUBLE())
                        .text_size(FONT_SIZE())
                        .line_height(zpx(18.))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            // `h3` (1.17em, bold; the first child's top
                            // margin is dropped, the bottom one collapses
                            // with the rows' 12 px)
                            div()
                                .text_size(FONT_SIZE() * 1.17)
                                .line_height(FONT_SIZE() * 1.17 * 1.5)
                                .font_weight(FontWeight::BOLD)
                                .child(header),
                        )
                        .children(body),
                    cx,
                )
                .with_priority(26),
            )
            .into_any_element()
    }
}

impl Render for CommitMessageAvatar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let model = self.model(cx).map(Rc::new);
        let avatar = model
            .as_ref()
            .and_then(|m| m.email.as_deref())
            .and_then(|email| avatar_lookup(email, cx));
        let warning = model.as_ref().map_or(Warning::None, |m| m.warning);
        if model.is_none() && self.open {
            self.open = false;
        }
        let bounds = self.avatar_bounds.clone();
        let label = match warning {
            Warning::None => "View commit author information",
            Warning::Misattribution => "Commit may be misattributed. View warning.",
            Warning::DisallowedEmail => "Email address is disallowed. View warning.",
        };
        // `.commit-message-avatar-component`: a text field's height square
        div()
            .relative()
            .flex_none()
            .size(TEXT_FIELD_HEIGHT())
            .child(
                div()
                    .id("commit-avatar-button")
                    .relative()
                    .size_full()
                    .rounded_full()
                    .cursor_pointer()
                    .icon_button_label(label)
                    .on_click(cx.listener(|this, _, window, cx| this.toggle(window, cx)))
                    .child(
                        canvas(move |b, _, _| bounds.set(b), |_, _, _, _| {})
                            .absolute()
                            .inset_0(),
                    )
                    .child(avatar_image(avatar, AVATAR_SIZE(), cx))
                    .when_some(
                        model.as_ref().filter(|m| m.warning != Warning::None),
                        |d, m| d.child(self.badge(m, cx)),
                    ),
            )
            .when_some(model.filter(|_| self.open), |d, model| {
                d.child(self.popover(model, window, cx))
            })
    }
}
