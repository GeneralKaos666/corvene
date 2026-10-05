//! Repository Settings dialog (`ui/repository-settings/repository-settings.tsx`):
//! 600 px wide, vertical tabs Remote · Ignored Files · Git Config; Cancel / Save.
//! Fork Behavior is omitted (no fork workflow yet).
//!
//! Deviation (flag `edit-global-ignore-file`): Ignored Files links to the
//! global excludes file, opened in the external editor.
//! Deviation (flag `239-line-endings-setting`): Git Config ends in a "Line
//! endings (core.autocrlf)" select stored in the repository's own config.
//! Deviation (flag `518-per-repo-editor`): an Editor tab picks the external
//! editor this repository opens in: an installed one, one of Settings'
//! custom editors (`523-custom-editor-list`) or its own path and arguments
//! ("Custom…").
//! Deviation (flag `1102-repository-credential-helper`): the Remote tab can
//! make the repository sign in through git's credential helper.
//! Deviation (flag `341-custom-autolinks`): an Autolinks tab lists the
//! GitHub repository's autolinks and edits the repository's own.

use std::rc::Rc;

use corvene_core::{
    AppState, Dispatcher, GitConfigLocation, Popup, RepositorySettingsSave, RepositorySettingsTab,
};
use gpui_kit::component::input::{InputState, Textarea, TextareaState};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::{IS_MAC, mac_or};
use crate::dialog::{GroupButtonSpec, OkCancelButtonGroup};
use crate::icons::Octicon;
use crate::tab_bar::VerticalTab;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{
    SelectHandler, call_to_action, checkbox_row, code_ref, labeled, link_button, paragraph,
    radio_row, section_heading, select_button, text_box,
};

/// GHD `NoRemote`'s `HelpURL` (`ui/repository-settings/no-remote.tsx`).
const NO_REMOTE_HELP_URL: &str = "https://help.github.com/articles/about-remote-repositories/";

/// What a button of `NoRemote` does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoRemoteAction {
    /// `onPublish`: the Publish Repository dialog.
    Publish,
}

/// What GHD `NoRemote` (`ui/repository-settings/no-remote.tsx`), the Remote
/// tab without a remote, shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoRemoteContent {
    /// The call to action's text, the link's label included.
    pub message: String,
    /// The text before the help link.
    pub lead: String,
    /// The help link: its label and the URL it opens.
    pub help_link: (String, String),
    /// The call to action's button: its label and what it does.
    pub action: (String, NoRemoteAction),
}

/// GHD `NoRemote`.
pub fn no_remote() -> NoRemoteContent {
    let lead = "Publish your repository to GitHub. Need help? ";
    let label = "Learn more about remote repositories.";
    NoRemoteContent {
        message: format!("{lead}{label}"),
        lead: lead.into(),
        help_link: (label.into(), NO_REMOTE_HELP_URL.into()),
        action: ("Publish".into(), NoRemoteAction::Publish),
    }
}

pub struct RepositorySettingsDialog {
    state: Entity<AppState>,
    repo: u64,
    tab: RepositorySettingsTab,
    remote_url: Entity<InputState>,
    gitignore: Entity<TextareaState>,
    gitignore_edited: bool,
    location: GitConfigLocation,
    initial_location: GitConfigLocation,
    name: Entity<InputState>,
    email: Entity<InputState>,
    /// Email picked from the account emails; `None` = "Other" (text box).
    email_choice: Option<String>,
    loaded: bool,
    /// Fork Behavior tab (`forkContributionTarget`).
    fork_target: corvene_core::ForkContributionTarget,
    /// `239-line-endings-setting`: the chosen `--local` `core.autocrlf`
    /// (`None`: the global config's).
    autocrlf: Option<&'static str>,
    /// `518-per-repo-editor`: the editor picked on the Editor tab (`None`:
    /// the one in Settings), or a custom one: one of Settings' list, or
    /// ("Custom…", `own_custom`) the path and arguments typed here.
    editor: Option<String>,
    custom_editor: Option<corvene_core::RepoCustomEditor>,
    own_custom: bool,
    custom_path: Entity<InputState>,
    custom_args: Entity<InputState>,
    /// `1102-repository-credential-helper`: the Remote tab's checkbox.
    credential_helper: bool,
    /// `341-custom-autolinks`: the repository's own autolinks as edited,
    /// and the Add form.
    autolinks: Vec<corvene_core::Autolink>,
    autolink_prefix: Entity<InputState>,
    autolink_url: Entity<InputState>,
    autolink_alphanumeric: bool,
    /// `focusFirstSuitableChild`: with nothing to type into on the first tab
    /// (no remote), Save holds focus until a mouse press moves it.
    default_focus: bool,
}

/// `518-per-repo-editor` with `523-custom-editor-list`: Settings' custom
/// editors, as a repository keeps one.
fn global_custom_editors(s: &AppState) -> Vec<corvene_core::RepoCustomEditor> {
    if !s.flags.bool(corvene_core::flags::ids::CUSTOM_EDITOR_LIST) {
        return Vec::new();
    }
    s.settings
        .custom_editors()
        .into_iter()
        .enumerate()
        .map(|(ix, c)| corvene_core::RepoCustomEditor {
            name: c.display_name(ix, true),
            path: c.path,
            arguments: c.arguments,
        })
        .collect()
}

/// A repository custom editor's name in the menu.
fn display_name(custom: &corvene_core::RepoCustomEditor) -> String {
    if custom.name.trim().is_empty() {
        "Custom Editor".to_string()
    } else {
        custom.name.clone()
    }
}

/// The [`AUTOCRLF_CHOICES`] entry of the repository's own `core.autocrlf`.
fn autocrlf_choice(data: &corvene_core::RepositorySettingsData) -> Option<&'static str> {
    let value = data.local_autocrlf.as_deref()?;
    AUTOCRLF_CHOICES
        .iter()
        .find_map(|(c, _)| c.filter(|c| c.eq_ignore_ascii_case(value)))
}

/// `239-line-endings-setting`: `core.autocrlf` choices (`None` = unset).
const AUTOCRLF_CHOICES: [(Option<&str>, &str); 4] = [
    (None, "Use my global Git config"),
    (Some("true"), "Check out CRLF, commit LF (true)"),
    (Some("input"), "Check out as is, commit LF (input)"),
    (Some("false"), "Check out and commit as is (false)"),
];

impl RepositorySettingsDialog {
    pub fn new(
        state: Entity<AppState>,
        repo: u64,
        tab: RepositorySettingsTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let remote_url = cx.new(|cx| InputState::new(window, cx).placeholder("Remote URL"));
        let autolink_prefix = cx.new(|cx| InputState::new(window, cx).placeholder("TICKET-"));
        let autolink_url = cx.new(|cx| {
            InputState::new(window, cx).placeholder("https://example.com/TICKET?query=<num>")
        });
        for input in [&autolink_prefix, &autolink_url] {
            cx.observe(input, |_, _, cx| cx.notify()).detach();
        }
        // `518-per-repo-editor`: a stored custom editor that is not one of
        // Settings' shows as "Custom…" with its path and arguments
        let stored_custom = state
            .read(cx)
            .repository(repo)
            .and_then(|r| r.custom_editor.clone());
        let own_custom = stored_custom
            .as_ref()
            .is_some_and(|c| !global_custom_editors(state.read(cx)).iter().any(|g| g == c));
        let (path, args) = stored_custom
            .as_ref()
            .filter(|_| own_custom)
            .map(|c| (c.path.clone(), c.arguments.clone()))
            .unwrap_or_default();
        let custom_path = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Path to executable")
                .default_value(path)
        });
        let custom_args = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Command line arguments")
                .default_value(args)
        });
        let taller = AppState::global(cx)
            .read(cx)
            .flags
            .bool(corvene_core::flags::ids::TALLER_TEXT_AREAS);
        let gitignore = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(if taller { 16 } else { 8 })
                .placeholder("Ignored files")
        });
        let name = cx.new(|cx| InputState::new(window, cx));
        let email = cx.new(|cx| InputState::new(window, cx));
        for input in [&remote_url, &name, &email] {
            cx.observe(input, |_, _, cx| cx.notify()).detach();
        }
        cx.observe(&gitignore, |this, _, cx| {
            if this.loaded {
                this.gitignore_edited = true;
            }
            cx.notify();
        })
        .detach();
        cx.observe_in(&state, window, |this, state, window, cx| {
            this.fill(&state, window, cx);
            cx.notify();
        })
        .detach();
        let mut this = Self {
            state: state.clone(),
            repo,
            tab,
            remote_url,
            gitignore,
            gitignore_edited: false,
            location: GitConfigLocation::Global,
            initial_location: GitConfigLocation::Global,
            name,
            email,
            email_choice: None,
            loaded: false,
            fork_target: state
                .read(cx)
                .repository(repo)
                .map(|r| r.fork_contribution_target())
                .unwrap_or_default(),
            autocrlf: None,
            editor: state
                .read(cx)
                .repository(repo)
                .and_then(|r| r.editor.clone()),
            custom_editor: stored_custom.clone().filter(|_| !own_custom),
            own_custom,
            custom_path,
            custom_args,
            credential_helper: state
                .read(cx)
                .repository(repo)
                .is_some_and(|r| r.use_credential_helper),
            autolinks: state
                .read(cx)
                .repository(repo)
                .map(|r| r.autolinks.clone())
                .unwrap_or_default(),
            autolink_prefix,
            autolink_url,
            autolink_alphanumeric: false,
            default_focus: true,
        };
        this.fill(&state, window, cx);
        this
    }

    fn data(&self, cx: &App) -> Option<corvene_core::RepositorySettingsData> {
        self.state
            .read(cx)
            .repo_settings
            .clone()
            .filter(|d| d.repo == self.repo)
    }

    fn fill(&mut self, state: &Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) {
        if self.loaded {
            return;
        }
        let Some(data) = state
            .read(cx)
            .repo_settings
            .clone()
            .filter(|d| d.repo == self.repo)
        else {
            return;
        };
        if let Some(remote) = &data.remote {
            self.remote_url
                .update(cx, |s, cx| s.set_value(remote.url.clone(), window, cx));
        }
        self.gitignore.update(cx, |s, cx| {
            s.set_value(data.gitignore.clone().unwrap_or_default(), window, cx)
        });
        let local = data.local_name.is_some() || data.local_email.is_some();
        self.location = if local {
            GitConfigLocation::Local
        } else {
            GitConfigLocation::Global
        };
        self.initial_location = self.location;
        let name = data.local_name.clone().unwrap_or_default();
        let email = data.local_email.clone().unwrap_or_default();
        self.name.update(cx, |s, cx| s.set_value(name, window, cx));
        self.email
            .update(cx, |s, cx| s.set_value(email.clone(), window, cx));
        let emails = self.account_emails(cx);
        self.email_choice = emails.iter().find(|e| **e == email).cloned();
        self.autocrlf = autocrlf_choice(&data);
        self.loaded = true;
        self.gitignore_edited = false;
    }

    fn account_emails(&self, cx: &App) -> Vec<String> {
        let s = self.state.read(cx);
        let account = s
            .repository(self.repo)
            .and_then(|r| r.github.as_ref())
            .and_then(|gh| s.account_for(&gh.endpoint));
        account.map(|a| a.emails.clone()).unwrap_or_default()
    }

    /// `ForkSettings` tab: "I'll be using this fork…"
    fn fork_settings_tab(&self, cx: &Context<Self>) -> AnyElement {
        let Some(github) = self
            .state
            .read(cx)
            .repository(self.repo)
            .and_then(|r| r.github.clone())
        else {
            return div().into_any_element();
        };
        let target = self.fork_target;
        let select = |value: corvene_core::ForkContributionTarget, cx: &Context<Self>| {
            let weak = cx.weak_entity();
            move |_: &mut Window, cx: &mut App| {
                weak.update(cx, |this, cx| {
                    this.fork_target = value;
                    cx.notify();
                })
                .ok();
            }
        };
        div()
            .flex()
            .flex_col()
            .child(section_heading("I'll be using this fork…", cx))
            .child(radio_row(
                "repo-settings-fork-parent",
                target == corvene_core::ForkContributionTarget::Parent,
                "To contribute to the parent repository",
                select(corvene_core::ForkContributionTarget::Parent, cx),
                cx,
            ))
            .child(radio_row(
                "repo-settings-fork-self",
                target == corvene_core::ForkContributionTarget::Own,
                "For my own purposes",
                select(corvene_core::ForkContributionTarget::Own, cx),
                cx,
            ))
            .child(crate::dialogs::fork_settings_description(
                &github, target, cx,
            ))
            .into_any_element()
    }

    fn save(&self, cx: &mut App) {
        let data = self.data(cx);
        let mut save = RepositorySettingsSave::default();
        let stored_target = self
            .state
            .read(cx)
            .repository(self.repo)
            .map(|r| r.fork_contribution_target());
        if stored_target.is_some_and(|t| t != self.fork_target) {
            Dispatcher::set_fork_contribution_target(self.repo, self.fork_target, cx);
        }
        let stored_editor = self
            .state
            .read(cx)
            .repository(self.repo)
            .map(|r| (r.editor.clone(), r.custom_editor.clone()));
        let custom = if self.own_custom {
            let path = self.custom_path.read(cx).value().trim().to_string();
            (!path.is_empty()).then(|| corvene_core::RepoCustomEditor {
                path,
                arguments: self.custom_args.read(cx).value().trim().to_string(),
                name: String::new(),
            })
        } else {
            self.custom_editor.clone()
        };
        let editor = self.editor.clone().filter(|_| custom.is_none());
        if stored_editor.is_some_and(|stored| stored != (editor.clone(), custom.clone())) {
            match custom {
                Some(custom) => {
                    Dispatcher::set_repository_custom_editor(self.repo, Some(custom), cx)
                }
                None => Dispatcher::set_repository_editor(self.repo, editor, cx),
            }
        }
        // `341-custom-autolinks`
        let stored_autolinks = self
            .state
            .read(cx)
            .repository(self.repo)
            .map(|r| r.autolinks.clone());
        if stored_autolinks.is_some_and(|a| a != self.autolinks) {
            Dispatcher::set_repository_autolinks(self.repo, self.autolinks.clone(), cx);
        }
        // `1102-repository-credential-helper`
        let stored_helper = self
            .state
            .read(cx)
            .repository(self.repo)
            .map(|r| r.use_credential_helper);
        if stored_helper.is_some_and(|on| on != self.credential_helper) {
            Dispatcher::set_repository_credential_helper(self.repo, self.credential_helper, cx);
        }
        if let Some(remote) = data.as_ref().and_then(|d| d.remote.clone()) {
            let url = self.remote_url.read(cx).value().trim().to_string();
            if url != remote.url {
                save.remote_url = Some((remote.name, url));
            }
        }
        if self.gitignore_edited {
            save.gitignore = Some(self.gitignore.read(cx).value().to_string());
        }
        let name = self.name.read(cx).value().to_string();
        let email = self.email.read(cx).value().to_string();
        match self.location {
            GitConfigLocation::Global if self.initial_location == GitConfigLocation::Local => {
                save.git_config = Some((GitConfigLocation::Global, String::new(), String::new()));
            }
            GitConfigLocation::Local => {
                let changed = data.as_ref().is_none_or(|d| {
                    d.local_name.clone().unwrap_or_default() != name
                        || d.local_email.clone().unwrap_or_default() != email
                });
                if changed || self.initial_location == GitConfigLocation::Global {
                    save.git_config = Some((GitConfigLocation::Local, name, email));
                }
            }
            GitConfigLocation::Global => {}
        }
        // an unknown stored value shows as the global one; it is only
        // replaced when another choice is picked
        if data
            .as_ref()
            .is_some_and(|d| autocrlf_choice(d) != self.autocrlf)
        {
            save.autocrlf = Some(self.autocrlf.map(str::to_string));
        }
        Dispatcher::save_repository_settings(self.repo, save, cx);
    }

    fn remote_tab(&self, window: &Window, cx: &Context<Self>) -> AnyElement {
        let data = self.data(cx);
        match data.and_then(|d| d.remote) {
            Some(remote) => {
                // flag `236-upstream-remote-in-settings` (Corvene addition,
                // desktop/desktop#6877): the `upstream` remote a fork
                // workflow adds, read-only under the primary one
                let s = self.state.read(cx);
                let upstream = s
                    .flags
                    .bool(corvene_core::flags::ids::UPSTREAM_REMOTE_IN_SETTINGS)
                    .then(|| {
                        s.repo_states
                            .get(&self.repo)
                            .and_then(|rs| rs.info.as_ref())
                            .and_then(|info| {
                                info.remotes
                                    .iter()
                                    .find(|r| r.name == "upstream" && r.name != remote.name)
                            })
                            .map(|r| r.url.clone())
                    })
                    .flatten();
                div()
                    .flex()
                    .flex_col()
                    .gap(SPACING())
                    .child(labeled(
                        if IS_MAC {
                            format!("Primary Remote Repository ({}) URL", remote.name)
                        } else {
                            format!("Primary remote repository ({}) URL", remote.name)
                        },
                        text_box(
                            "repo-settings-remote-url",
                            &self.remote_url,
                            None,
                            window,
                            cx,
                        ),
                        cx,
                    ))
                    .when_some(upstream, |d, url| {
                        d.child(labeled(
                            "Upstream Remote Repository (upstream) URL",
                            readonly_field(url, cx),
                            cx,
                        ))
                    })
                    .when(
                        s.flags
                            .bool(corvene_core::flags::ids::REPOSITORY_CREDENTIAL_HELPER),
                        |d| d.child(self.credential_helper_option(cx)),
                    )
                    .into_any_element()
            }
            None => {
                let repo = self.repo;
                let NoRemoteContent {
                    lead,
                    help_link: (help_label, help_url),
                    action: (action_title, NoRemoteAction::Publish),
                    ..
                } = no_remote();
                call_to_action(
                    "repo-settings-publish",
                    paragraph(vec![
                        lead.into(),
                        link_button("repo-settings-remote-help", help_label, cx)
                            .on_click(move |_, _, cx| Dispatcher::open_url(&help_url, cx))
                            .into_any_element()
                            .into(),
                    ]),
                    action_title,
                    move |_, cx| Dispatcher::show_popup(Popup::PublishRepository { repo }, cx),
                    cx,
                )
                .into_any_element()
            }
        }
    }

    /// Corvene (`1102-repository-credential-helper`): sign in through git's
    /// credential helper (Git Credential Manager, the keychain) instead of
    /// the account, e.g. as a second GitHub account.
    fn credential_helper_option(&self, cx: &Context<Self>) -> AnyElement {
        let t = cx.ghd();
        let this = cx.entity().downgrade();
        div()
            .flex()
            .flex_col()
            .gap(SPACING_HALF())
            .child(checkbox_row(
                "repo-settings-credential-helper",
                self.credential_helper,
                mac_or(
                    "Use Git Credential Manager for This Repository",
                    "Use Git Credential Manager for this repository",
                ),
                move |on, _, cx| {
                    let _ = this.update(cx, |this, cx| {
                        this.credential_helper = on;
                        cx.notify();
                    });
                },
                cx,
            ))
            .child(
                div()
                    .text_size(FONT_SIZE_SM())
                    .text_color(t.text_secondary)
                    .child(
                        "Fetch, pull and push sign in with git's credential helpers (Git \
                         Credential Manager, the keychain) instead of your account, e.g. to use \
                         another account for this repository.",
                    ),
            )
            .into_any_element()
    }

    /// Flag `237-gitignore-templates` (Corvene addition, desktop/desktop#2197):
    /// a bundled `.gitignore` template (the Create a New Repository list)
    /// fills an empty box or is appended under a `# <Name>` line; nothing is
    /// written until Save.
    fn gitignore_template_select(&self, cx: &Context<Self>) -> AnyElement {
        let names = corvene_core::templates::gitignore_names();
        let options: Vec<SharedString> = names.iter().map(|n| n.clone().into()).collect();
        let gitignore = self.gitignore.clone();
        let on_select: SelectHandler = Rc::new(move |ix, window, cx| {
            let Some(name) = names.get(ix) else {
                return;
            };
            let Some(template) = corvene_core::templates::gitignore_text(name) else {
                return;
            };
            gitignore.update(cx, |s, cx| {
                let current = s.value().to_string();
                let value = if current.trim().is_empty() {
                    template
                } else {
                    format!("{}\n\n# {name}\n{template}", current.trim_end())
                };
                s.set_value(value, window, cx);
            });
        });
        labeled(
            "Add a template",
            select_button(
                "repo-settings-gitignore-template",
                "Choose a template…",
                options,
                None,
                false,
                on_select,
                cx,
            ),
            cx,
        )
        .into_any_element()
    }

    fn ignored_files_tab(&self, cx: &Context<Self>) -> AnyElement {
        let t = cx.ghd();
        let templates = AppState::global(cx)
            .read(cx)
            .flags
            .bool(corvene_core::flags::ids::GITIGNORE_TEMPLATES);
        let height = if AppState::global(cx)
            .read(cx)
            .flags
            .bool(corvene_core::flags::ids::TALLER_TEXT_AREAS)
        {
            260.
        } else {
            130.
        };
        let resizable = AppState::global(cx)
            .read(cx)
            .flags
            .bool(corvene_core::flags::ids::RESIZABLE_DIALOG_TEXT_AREAS);
        div()
            .flex()
            .flex_col()
            .gap(SPACING())
            .child(paragraph(vec![
                "Editing ".into(),
                code_ref(".gitignore", cx).into_any_element().into(),
                ". This file specifies intentionally untracked files that Git should ignore. Files already tracked by Git are not affected. ".into(),
                link_button(
                    "repo-settings-gitignore-help",
                    "Learn more about gitignore files",
                    cx,
                )
                .on_click(|_, _, cx| Dispatcher::open_url("https://git-scm.com/docs/gitignore", cx))
                .into_any_element()
                .into(),
            ]))
            .when(templates, |d| d.child(self.gitignore_template_select(cx)))
            .child(
                // `textarea.gitignore { height: 130px }`; flag
                // `107-taller-text-areas` doubles it, `116` lets it be dragged
                div()
                    .when(!resizable, |d| d.h(zpx(height)))
                    .border_1()
                    .border_color(t.box_border_contrast)
                    .rounded(BORDER_RADIUS())
                    .bg(t.box_background)
                    .overflow_hidden()
                    // `.text-area-component textarea`: the sans-serif body font
                    // at its normal line height, 0 / 5 px padding
                    .px(zpx(5.))
                    .py(zpx(2.))
                    .text_size(FONT_SIZE())
                    .line_height(zpx(14.))
                    .child(crate::widgets::resizable_text_area(
                        "gitignore",
                        Textarea::new(&self.gitignore)
                            .appearance(false)
                            .h(zpx(height - 6.)),
                        cx,
                    )),
            )
            .when(
                self.state
                    .read(cx)
                    .flags
                    .bool(corvene_core::flags::ids::EDIT_GLOBAL_IGNORE_FILE),
                |d| {
                    let repo = self.repo;
                    d.child(paragraph(vec![
                        "Patterns for every repository on this computer go in the global ignore file. "
                            .into(),
                        link_button(
                            "repo-settings-global-ignore",
                            "Edit global ignore file",
                            cx,
                        )
                        .on_click(move |_, _, cx| Dispatcher::edit_global_ignore_file(repo, cx))
                        .into_any_element()
                        .into(),
                    ]))
                },
            )
            .into_any_element()
    }

    fn git_config_tab(&self, window: &Window, cx: &Context<Self>) -> AnyElement {
        let data = self.data(cx);
        let global = data.as_ref().map(|d| d.global.clone()).unwrap_or_default();
        let disabled = self.location == GitConfigLocation::Global;
        let emails = self.account_emails(cx);
        let options = [
            (
                GitConfigLocation::Global,
                "repo-settings-config-global",
                "Use my global Git config",
            ),
            (
                GitConfigLocation::Local,
                "repo-settings-config-local",
                "Use a local Git config",
            ),
        ];
        let name_field = if disabled {
            readonly_field(global.name.clone().unwrap_or_default(), cx).into_any_element()
        } else {
            text_box("repo-settings-name", &self.name, None, window, cx).into_any_element()
        };
        // `GitConfigUserForm`: GHD labels only the account-email select; its
        // lone text box loses the label whenever `emailIsOther` holds, which
        // flag `602-git-config-email-label` corrects
        let email_labeled = (!disabled && !emails.is_empty())
            || AppState::global(cx)
                .read(cx)
                .flags
                .bool(corvene_core::flags::ids::GIT_CONFIG_EMAIL_LABEL);
        let email_field: AnyElement = if disabled {
            readonly_field(global.email.clone().unwrap_or_default(), cx).into_any_element()
        } else if emails.is_empty() {
            text_box("repo-settings-email", &self.email, None, window, cx).into_any_element()
        } else {
            let mut items: Vec<SharedString> = emails
                .iter()
                .map(|e| SharedString::from(e.clone()))
                .collect();
            items.push("Other".into());
            let selected_ix = match &self.email_choice {
                Some(choice) => emails.iter().position(|e| e == choice),
                None => Some(emails.len()),
            };
            let weak = cx.weak_entity();
            let emails_for_select = emails.clone();
            let on_email: SelectHandler = Rc::new(move |ix, window, cx| {
                let choice = emails_for_select.get(ix).cloned();
                weak.update(cx, |this, cx| {
                    if let Some(email) = &choice {
                        this.email
                            .update(cx, |s, cx| s.set_value(email.clone(), window, cx));
                    }
                    this.email_choice = choice;
                    cx.notify();
                })
                .ok();
            });
            div()
                .flex()
                .flex_col()
                .gap(SPACING())
                .child(select_button(
                    "repo-settings-email-select",
                    self.email_choice
                        .clone()
                        .unwrap_or_else(|| "Other".to_string()),
                    items,
                    selected_ix,
                    false,
                    on_email,
                    cx,
                ))
                .when(self.email_choice.is_none(), |d| {
                    d.child(text_box(
                        "repo-settings-email",
                        &self.email,
                        None,
                        window,
                        cx,
                    ))
                })
                .into_any_element()
        };
        div()
            .flex()
            .flex_col()
            .child(section_heading("For this repository I wish to", cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(SPACING_HALF())
                    .mb(SPACING())
                    .children(options.iter().map(|(value, id, label)| {
                        let value = *value;
                        let weak = cx.weak_entity();
                        radio_row(
                            id,
                            self.location == value,
                            *label,
                            move |_, cx| {
                                weak.update(cx, |this, cx| {
                                    this.location = value;
                                    cx.notify();
                                })
                                .ok();
                            },
                            cx,
                        )
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(SPACING())
                    .child(labeled("Name", name_field, cx))
                    .child(if email_labeled {
                        labeled("Email", email_field, cx).into_any_element()
                    } else {
                        email_field
                    }),
            )
            .when(
                AppState::global(cx)
                    .read(cx)
                    .flags
                    .bool(corvene_core::flags::ids::LINE_ENDINGS_SETTING),
                |d| d.child(self.line_endings_field(cx)),
            )
            .into_any_element()
    }

    /// `518-per-repo-editor`: which external editor opens this repository.
    fn editor_tab(&self, window: &Window, cx: &Context<Self>) -> AnyElement {
        let names: Vec<String> = self
            .state
            .read(cx)
            .editors
            .iter()
            .map(|e| e.name.clone())
            .collect();
        // Settings' custom editors (`523-custom-editor-list`), then
        // "Custom…": this repository's own path and arguments
        let customs = global_custom_editors(self.state.read(cx));
        let default = {
            let s = self.state.read(cx);
            s.settings
                .external_editor
                .clone()
                .or_else(|| s.editors.first().map(|e| e.name.clone()))
        };
        let mut options: Vec<SharedString> = vec![match &default {
            Some(name) => format!("Use my default editor ({name})").into(),
            None => "Use my default editor".into(),
        }];
        options.extend(names.iter().cloned().map(SharedString::from));
        options.extend(customs.iter().map(|c| SharedString::from(display_name(c))));
        let own_ix = options.len();
        options.push(mac_or("Custom…", "Custom…").into());
        let selected = match (&self.editor, &self.custom_editor) {
            _ if self.own_custom => Some(own_ix),
            (_, Some(custom)) => customs
                .iter()
                .position(|c| c == custom)
                .map(|i| i + 1 + names.len()),
            (None, None) => Some(0),
            (Some(name), None) => names.iter().position(|n| n == name).map(|i| i + 1),
        };
        let value = match selected {
            Some(ix) => options[ix].clone(),
            // chosen before, not installed now
            None => format!("{} (not found)", self.editor.clone().unwrap_or_default()).into(),
        };
        let weak = cx.weak_entity();
        let installed = names.len();
        let on_select: SelectHandler = Rc::new(move |ix, _, cx| {
            let choice = ix.checked_sub(1).and_then(|i| names.get(i).cloned());
            let custom = ix
                .checked_sub(1 + installed)
                .and_then(|i| customs.get(i).cloned());
            weak.update(cx, |this, cx| {
                this.own_custom = ix == own_ix;
                this.custom_editor = custom;
                this.editor = choice;
                cx.notify();
            })
            .ok();
        });
        let own_form = self.own_custom.then(|| {
            div()
                .flex()
                .flex_col()
                .gap(SPACING_HALF())
                .child(labeled(
                    "Path",
                    text_box("repo-editor-path", &self.custom_path, None, window, cx),
                    cx,
                ))
                .child(labeled(
                    "Arguments",
                    text_box("repo-editor-args", &self.custom_args, None, window, cx),
                    cx,
                ))
        });
        div()
            .child(labeled(
                "External editor",
                div()
                    .flex()
                    .flex_col()
                    .gap(SPACING_HALF())
                    .child(select_button(
                        "repo-settings-editor",
                        value,
                        options,
                        selected,
                        false,
                        on_select,
                        cx,
                    ))
                    .child(
                        div()
                            .text_size(FONT_SIZE_SM())
                            .text_color(cx.ghd().text_secondary)
                            .child(
                                "Open in External Editor and opening this repository's files \
                                 use this editor.",
                            ),
                    ),
                cx,
            ))
            .when_some(own_form, |d, form| d.child(div().mt(SPACING()).child(form)))
            .into_any_element()
    }

    /// `341-custom-autolinks`: the GitHub repository's autolinks (read
    /// only) and the repository's own, with a form to add one.
    fn autolinks_tab(&self, window: &Window, cx: &Context<Self>) -> AnyElement {
        let t = cx.ghd();
        let api = self
            .state
            .read(cx)
            .repo_states
            .get(&self.repo)
            .map(|rs| rs.api_autolinks.clone())
            .unwrap_or_default();
        let describe = |a: &corvene_core::Autolink| {
            format!(
                "{}<num> → {}{}",
                a.key_prefix,
                a.url_template,
                if a.is_alphanumeric {
                    " (letters too)"
                } else {
                    ""
                }
            )
        };
        let row = |text: String| {
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(FONT_SIZE())
                .child(text)
        };
        let prefix = self.autolink_prefix.read(cx).value().trim().to_string();
        let url = self.autolink_url.read(cx).value().trim().to_string();
        let can_add = !prefix.is_empty() && url.contains("<num>");
        let weak = cx.weak_entity();
        let add = move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
            weak.update(cx, |this, cx| {
                let key_prefix = this.autolink_prefix.read(cx).value().trim().to_string();
                let url_template = this.autolink_url.read(cx).value().trim().to_string();
                if key_prefix.is_empty() || !url_template.contains("<num>") {
                    return;
                }
                this.autolinks.push(corvene_core::Autolink {
                    key_prefix,
                    url_template,
                    is_alphanumeric: this.autolink_alphanumeric,
                });
                this.autolink_prefix
                    .update(cx, |s, cx| s.set_value("", window, cx));
                this.autolink_url
                    .update(cx, |s, cx| s.set_value("", window, cx));
                this.autolink_alphanumeric = false;
                cx.notify();
            })
            .ok();
        };
        let weak = cx.weak_entity();
        div()
            .flex()
            .flex_col()
            .gap(SPACING())
            .child(
                div()
                    .text_size(FONT_SIZE_SM())
                    .text_color(t.text_secondary)
                    .child(
                        "References such as TICKET-123 in commit messages link to the URL, \
                         with <num> replaced by what follows the prefix.",
                    ),
            )
            .when(!api.is_empty(), |d| {
                d.child(section_heading("From GitHub", cx))
                    .children(api.iter().map(|a| row(describe(a))))
            })
            .child(section_heading("This repository", cx))
            .children(self.autolinks.iter().enumerate().map(|(ix, a)| {
                let weak = weak.clone();
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING())
                    .child(row(describe(a)))
                    .child(
                        link_button(("repo-autolink-remove", ix), "Remove", cx).on_click(
                            move |_, _, cx| {
                                weak.update(cx, |this, cx| {
                                    if ix < this.autolinks.len() {
                                        this.autolinks.remove(ix);
                                    }
                                    cx.notify();
                                })
                                .ok();
                            },
                        ),
                    )
            }))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_end()
                    .gap(SPACING())
                    .child(div().w(zpx(110.)).child(labeled(
                        "Prefix",
                        text_box(
                            "repo-autolink-prefix",
                            &self.autolink_prefix,
                            None,
                            window,
                            cx,
                        ),
                        cx,
                    )))
                    .child(div().flex_1().child(labeled(
                        "URL",
                        text_box("repo-autolink-url", &self.autolink_url, None, window, cx),
                        cx,
                    ))),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(checkbox_row(
                        "repo-autolink-alphanumeric",
                        self.autolink_alphanumeric,
                        "Letters may follow the prefix too",
                        move |on, _, cx| {
                            weak.update(cx, |this, cx| {
                                this.autolink_alphanumeric = on;
                                cx.notify();
                            })
                            .ok();
                        },
                        cx,
                    ))
                    .child(if can_add {
                        crate::widgets::button("repo-autolink-add", "Add", cx)
                            .on_click(add)
                            .into_any_element()
                    } else {
                        crate::widgets::button_disabled("repo-autolink-add", "Add", cx)
                            .into_any_element()
                    }),
            )
            .into_any_element()
    }

    /// `239-line-endings-setting`: the repository's `core.autocrlf`.
    fn line_endings_field(&self, cx: &Context<Self>) -> impl IntoElement {
        let selected = AUTOCRLF_CHOICES
            .iter()
            .position(|(c, _)| *c == self.autocrlf);
        let weak = cx.weak_entity();
        let on_select: SelectHandler = Rc::new(move |ix, _, cx| {
            if let Some((choice, _)) = AUTOCRLF_CHOICES.get(ix) {
                weak.update(cx, |this, cx| {
                    this.autocrlf = *choice;
                    cx.notify();
                })
                .ok();
            }
        });
        div().mt(SPACING()).child(labeled(
            "Line endings (core.autocrlf)",
            div()
                .flex()
                .flex_col()
                .gap(SPACING_HALF())
                .child(select_button(
                    "repo-settings-autocrlf",
                    AUTOCRLF_CHOICES[selected.unwrap_or(0)].1,
                    AUTOCRLF_CHOICES
                        .iter()
                        .map(|(_, label)| SharedString::from(*label))
                        .collect(),
                    selected,
                    false,
                    on_select,
                    cx,
                ))
                .child(
                    div()
                        .text_size(FONT_SIZE_SM())
                        .text_color(cx.ghd().text_secondary)
                        .child(
                            "Applies to files as they are checked out or committed from now \
                             on; files already checked out keep their line endings.",
                        ),
                ),
            cx,
        ))
    }
}

/// A disabled `TextBox` showing the global value (GHD greys the inputs out).
fn readonly_field(value: String, cx: &App) -> Div {
    let t = cx.ghd();
    div()
        .h(TEXT_FIELD_HEIGHT())
        .w_full()
        .flex()
        .items_center()
        .px(SPACING_HALF())
        .border_1()
        .rounded(BORDER_RADIUS())
        .border_color(t.box_border_contrast)
        .bg(t.box_alt_background)
        .text_color(t.text_secondary)
        .opacity(0.7)
        .child(value)
}

impl Render for RepositorySettingsDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        // "Fork Behavior" only for forks with a known parent
        let is_fork = self
            .state
            .read(cx)
            .repository(self.repo)
            .and_then(|r| r.github.as_ref())
            .is_some_and(|gh| gh.parent.is_some());
        // Corvene (`518-per-repo-editor`): an Editor tab last
        let editor_tab = self
            .state
            .read(cx)
            .flags
            .bool(corvene_core::flags::ids::PER_REPO_EDITOR);
        // Corvene (`341-custom-autolinks`)
        let autolinks_tab = self
            .state
            .read(cx)
            .flags
            .bool(corvene_core::flags::ids::CUSTOM_AUTOLINKS);
        let tabs: Vec<RepositorySettingsTab> = [
            RepositorySettingsTab::Remote,
            RepositorySettingsTab::IgnoredFiles,
            RepositorySettingsTab::GitConfig,
        ]
        .into_iter()
        .chain(is_fork.then_some(RepositorySettingsTab::ForkSettings))
        .chain(editor_tab.then_some(RepositorySettingsTab::Editor))
        .chain(autolinks_tab.then_some(RepositorySettingsTab::Autolinks))
        .collect();
        if !tabs.contains(&self.tab) {
            self.tab = RepositorySettingsTab::Remote;
        }
        let selected = tabs.iter().position(|t| *t == self.tab).unwrap_or(0);
        let weak = cx.weak_entity();
        let compact = crate::theme::compact(window);
        let nav = crate::tab_bar::vertical_tab_bar_sized(
            vec![
                VerticalTab {
                    id: "repo-settings-tab-remote",
                    label: "Remote".into(),
                    icon: Octicon::Server,
                },
                VerticalTab {
                    id: "repo-settings-tab-ignored",
                    label: mac_or("Ignored Files", "Ignored files").into(),
                    icon: Octicon::File,
                },
                VerticalTab {
                    id: "repo-settings-tab-git-config",
                    label: mac_or("Git Config", "Git config").into(),
                    icon: Octicon::GitCommit,
                },
            ]
            .into_iter()
            .chain(is_fork.then_some(VerticalTab {
                id: "repo-settings-tab-fork",
                label: mac_or("Fork Behavior", "Fork behavior").into(),
                icon: Octicon::RepoForked,
            }))
            .chain(editor_tab.then_some(VerticalTab {
                id: "repo-settings-tab-editor",
                label: "Editor".into(),
                icon: Octicon::FileCode,
            }))
            .chain(autolinks_tab.then_some(VerticalTab {
                id: "repo-settings-tab-autolinks",
                label: "Autolinks".into(),
                icon: Octicon::LinkExternal,
            }))
            .collect(),
            selected,
            compact,
            move |ix, _, cx| {
                if let Some(tab) = tabs.get(ix).copied() {
                    weak.update(cx, |this, cx| {
                        this.tab = tab;
                        cx.notify();
                    })
                    .ok();
                }
            },
            cx,
        );
        let body = match self.tab {
            RepositorySettingsTab::Remote => self.remote_tab(window, cx),
            RepositorySettingsTab::IgnoredFiles => self.ignored_files_tab(cx),
            RepositorySettingsTab::GitConfig => self.git_config_tab(window, cx),
            RepositorySettingsTab::ForkSettings => self.fork_settings_tab(cx),
            RepositorySettingsTab::Editor => self.editor_tab(window, cx),
            RepositorySettingsTab::Autolinks => self.autolinks_tab(window, cx),
        };
        // `#repository-settings { width: 600px; .dialog-content { min-height: 305px } }`
        let content = div()
            .w(crate::theme::fit_bleed_width(600.))
            .mx(zpx(-20.))
            .my(zpx(-20.))
            .min_h(zpx(305.))
            .flex()
            .flex_row()
            .items_stretch()
            .child(nav)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .border_l_1()
                    .border_color(t.box_border)
                    .p(SPACING_DOUBLE())
                    .child(body),
            );
        let name_valid = self.location == GitConfigLocation::Global
            || corvene_core::git_author_name_is_valid(self.name.read(cx).value().trim());
        let content = div()
            .flex()
            .flex_col()
            .mx(zpx(-20.))
            .my(zpx(-20.))
            .when(!name_valid, |d| {
                d.child(
                    crate::widgets::dialog_error_banner(
                        corvene_core::INVALID_GIT_AUTHOR_NAME_MESSAGE,
                        cx,
                    )
                    .mx(zpx(0.))
                    .mt(zpx(0.))
                    .mb(zpx(0.)),
                )
            })
            .child(content.mx(zpx(0.)).my(zpx(0.)));
        let weak = cx.weak_entity();
        let loaded = self.loaded && name_valid;
        let focus_save = self.default_focus
            && self.tab == RepositorySettingsTab::Remote
            && self.data(cx).is_some_and(|d| d.remote.is_none());
        let content = content.on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _, _, cx| {
                if this.default_focus {
                    this.default_focus = false;
                    cx.notify();
                }
            }),
        );
        crate::dialog::dialog_with_frame(
            "dialog-repository-settings",
            mac_or("Repository Settings", "Repository settings"),
            content,
            OkCancelButtonGroup {
                destructive: false,
                cancel: GroupButtonSpec {
                    id: "repo-settings-cancel",
                    label: "Cancel".into(),
                    disabled: false,
                    on_click: Box::new(close),
                },
                ok: GroupButtonSpec {
                    id: "repo-settings-save",
                    label: "Save".into(),
                    disabled: !loaded,
                    on_click: Box::new(move |_, cx| {
                        if !loaded {
                            return;
                        }
                        weak.update(cx, |this, cx| this.save(cx)).ok();
                    }),
                },
            }
            .into_buttons(),
            crate::dialog::DialogFrame {
                focus_primary: focus_save,
                ..Default::default()
            },
            close,
            window,
            cx,
        )
    }
}
