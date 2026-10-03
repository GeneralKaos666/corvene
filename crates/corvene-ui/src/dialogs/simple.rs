//! Error / InstallGit / CLIInstalled (GHD `ui/cli-installed/cli-installed.tsx`)
//! dialogs: static content, one or two buttons.
//!
//! The error dialog is GHD's `AppError` (`ui/app-error.tsx`): an error-type
//! dialog whose content is the description GHD has for the failure
//! (`getDescriptionForError`), else git's raw output in monospace in a 750 px
//! dialog (`#app-error.raw-git-error`).
//!
//! Deviation: the error dialog can have a "Copy" button (its text cannot be
//! selected), `413-error-dialog-copy`.
//! Deviation (`265-remove-stale-index-lock`): an error caused by a left-over
//! `index.lock` offers "Remove Lock File".
//! Deviation (`415-git-error-dialog`): a failed git command leads with a
//! sentence, then shows the command, its exit code and its output in a box.

use corvene_core::{AppState, Dispatcher, Popup};
use corvene_git::{GitErrorDetails, GitFailure};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::mac_or;
use crate::dialog::{DialogButton, DialogKind, dialog, dialog_with_kind};
use crate::icons::{Octicon, octicon};
use crate::scrollbar::ScrollbarExt;
use crate::theme::sizes::*;
use crate::theme::{ActiveGhdTheme, mono_font};

pub struct SimpleDialog {
    popup: Popup,
    /// `415-git-error-dialog`: git's raw output is shown; starts collapsed
    /// when the dialog could pull details out of it.
    show_output: Option<bool>,
}

impl SimpleDialog {
    pub fn new(popup: Popup) -> Self {
        Self {
            popup,
            show_output: None,
        }
    }
}

impl Render for SimpleDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        match &self.popup {
            Popup::InstallGit { reason } => dialog(
                "dialog-install-git",
                "Unable to locate Git",
                div()
                    .flex()
                    .flex_col()
                    .gap(SPACING())
                    .child(format!(
                        "Corvene was unable to find a usable Git on your system ({reason})."
                    ))
                    .child(mac_or(
                        "Install the Xcode Command Line Tools (xcode-select --install) or run \
                         `brew install git`, then click Retry.",
                        "Install Git with your distribution's package manager, then click Retry.",
                    )),
                vec![
                    DialogButton {
                        id: "install-git-cancel",
                        label: "Cancel".into(),
                        primary: false,
                        disabled: false,
                        on_click: Box::new(close),
                    },
                    DialogButton {
                        id: "install-git-retry",
                        label: "Retry".into(),
                        primary: true,
                        disabled: false,
                        on_click: Box::new(|_, cx| {
                            Dispatcher::close_popup(cx);
                            Dispatcher::detect_git(cx);
                        }),
                    },
                ],
                close,
                window,
                cx,
            )
            .into_any_element(),
            Popup::Error {
                title,
                message,
                git,
            } => {
                let (copy, structured, settings_menu) = {
                    let s = AppState::global(cx).read(cx);
                    (
                        s.flags.bool(corvene_core::flags::ids::ERROR_DIALOG_COPY),
                        s.flags.bool(corvene_core::flags::ids::GIT_ERROR_DIALOG),
                        if cfg!(target_os = "macos") {
                            format!("{} > Settings.", s.product_name())
                        } else {
                            "File > Options.".to_string()
                        },
                    )
                };
                let body = match git {
                    Some(git) if structured => {
                        let details = git.details();
                        let show_output = self.show_output.unwrap_or(details.is_empty());
                        let toggle = cx.listener(move |this: &mut Self, _: &ClickEvent, _, cx| {
                            this.show_output = Some(!show_output);
                            cx.notify();
                        });
                        structured_git_error(
                            message,
                            git,
                            &details,
                            show_output,
                            toggle,
                            &settings_menu,
                            cx,
                        )
                    }
                    Some(git) => ghd_git_error(message, git, &settings_menu),
                    None => div().child(message.clone()).into_any_element(),
                };
                let mut buttons = Vec::new();
                if copy {
                    let text = match git {
                        Some(git) if message.is_empty() => {
                            format!("{title}\n\n{}", git.summary())
                        }
                        Some(git) => format!("{title}\n\n{message}\n\n{}", git.summary()),
                        None => format!("{title}\n\n{message}"),
                    };
                    buttons.push(DialogButton {
                        id: "error-copy",
                        label: "Copy".into(),
                        primary: false,
                        disabled: false,
                        on_click: Box::new(move |_, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(text.clone()))
                        }),
                    });
                }
                buttons.push(DialogButton {
                    id: "error-close",
                    label: "Close".into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(close),
                });
                // `raw-git-error`: the wide dialog whenever git's output shows
                let wide = git
                    .as_ref()
                    .is_some_and(|git| structured || git.description(&settings_menu).is_none());
                dialog_with_kind(
                    if wide {
                        "dialog-git-error"
                    } else {
                        "dialog-error"
                    },
                    DialogKind::Error,
                    title.clone(),
                    body,
                    buttons,
                    close,
                    window,
                    cx,
                )
                .into_any_element()
            }
            // Corvene (`265-remove-stale-index-lock`): GHD shows the plain error
            Popup::IndexLockExists {
                title,
                message,
                lock,
            } => {
                let lock = lock.clone();
                dialog(
                    "dialog-index-lock",
                    title.clone(),
                    div()
                        .flex()
                        .flex_col()
                        .gap(SPACING())
                        .child(message.clone())
                        .child(
                            "If no other Git program is working on this repository, a Git \
                             process that crashed left the lock file behind. Removing it lets \
                             Git continue; Corvene first checks that no Git process is running \
                             in the repository.",
                        ),
                    vec![
                        DialogButton {
                            id: "index-lock-close",
                            label: "Close".into(),
                            primary: false,
                            disabled: false,
                            on_click: Box::new(close),
                        },
                        DialogButton {
                            id: "index-lock-remove",
                            label: "Remove Lock File".into(),
                            primary: true,
                            disabled: false,
                            on_click: Box::new(move |_, cx| {
                                Dispatcher::remove_index_lock(lock.clone(), cx)
                            }),
                        },
                    ],
                    close,
                    window,
                    cx,
                )
                .into_any_element()
            }
            Popup::CLIInstalled { path } => dialog(
                "cli-installed",
                mac_or("Command Line Tool Installed", "Command line tool installed"),
                crate::widgets::paragraph(vec![
                    "The command line tool has been installed at ".into(),
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(path.display().to_string())
                        .into_any_element()
                        .into(),
                    ".".into(),
                ]),
                vec![DialogButton {
                    id: "cli-installed-ok",
                    label: "Ok".into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(close),
                }],
                close,
                window,
                cx,
            )
            .into_any_element(),
            _ => div().into_any_element(),
        }
    }
}

/// GHD's `AppError` content for a failed git command: the lead (Corvene's
/// explanation, else GHD's description) as a paragraph; git's output in
/// monospace when GHD has no description for it (`isRawGitError`).
fn ghd_git_error(lead: &str, git: &GitFailure, settings_menu: &str) -> AnyElement {
    let description = git.description(settings_menu);
    let raw = description.is_none();
    let lead = if lead.is_empty() {
        description
    } else {
        Some(lead.to_string())
    };
    div()
        .id("git-error-ghd")
        // `#app-error .dialog-content { max-height: 400px }`
        .max_h(zpx(400.))
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .gap(SPACING())
        .children(lead.map(|lead| div().child(lead)))
        .when(raw, |d| {
            d.child(div().font_family(mono_font()).child(git.output.clone()))
        })
        .with_scrollbar()
        .into_any_element()
}

/// `415-git-error-dialog`: a lead sentence, what git named (file lists, the
/// server's words, hints, a path), then a box with the command and exit code
/// over git's output (collapsed while details stand in for it).
#[allow(clippy::too_many_arguments)]
fn structured_git_error(
    lead: &str,
    git: &GitFailure,
    details: &GitErrorDetails,
    show_output: bool,
    toggle: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    settings_menu: &str,
    cx: &App,
) -> AnyElement {
    let t = cx.ghd();
    let lead = if lead.is_empty() {
        git.lead(settings_menu)
    } else {
        Some(lead.to_string())
    };
    let exit = match git.exit_code {
        Some(code) => format!("exit code {code}"),
        None => "killed by a signal".to_string(),
    };
    let label = |text: &'static str| {
        div()
            .text_size(FONT_SIZE_SM())
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(t.text_secondary)
            .child(text)
    };
    // `.ref-component`-style rows: monospace paths on the alt background
    let path_list = |id: &'static str, paths: Vec<String>| {
        div()
            .id(id)
            .max_h(zpx(160.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .rounded(BORDER_RADIUS())
            .border_1()
            .border_color(t.box_border)
            .bg(t.box_alt_background)
            .py(SPACING_HALF())
            .children(paths.into_iter().map(|path| {
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING_HALF())
                    .px(SPACING())
                    .h(zpx(22.))
                    .child(
                        octicon(Octicon::File, t.text_secondary)
                            .size(zpx(16.))
                            .flex_none(),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_family(mono_font())
                            .text_size(FONT_SIZE_SM())
                            .child(path),
                    )
            }))
            .with_scrollbar()
    };
    let mut body = div().flex().flex_col().gap(SPACING());
    if let Some(lead) = &lead {
        body = body.child(div().child(lead.clone()));
    }
    // the path or URL at fault, unless the lead already quotes it
    if let Some((label, value)) = details.subject.as_ref().filter(|(_, value)| {
        !lead
            .as_deref()
            .is_some_and(|lead| lead.contains(value.as_str()))
    }) {
        body = body.child(
            div()
                .flex()
                .flex_row()
                .items_baseline()
                .gap(SPACING_HALF())
                .child(
                    div()
                        .flex_none()
                        .text_color(t.text_secondary)
                        .child(format!("{label}:")),
                )
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .child(crate::widgets::code_ref(value.clone(), cx)),
                ),
        );
    }
    if let Some((what, files)) = &details.files {
        body = body
            .child(label(what))
            .child(path_list("git-error-files", files.clone()));
    }
    if !details.rejected.is_empty() {
        body = body
            .child(label("Rejected"))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(zpx(2.))
                    .children(details.rejected.iter().map(|(refs, reason)| {
                        div()
                            .flex()
                            .flex_row()
                            .items_baseline()
                            .gap(SPACING_HALF())
                            .child(
                                div()
                                    .flex_none()
                                    .child(crate::widgets::code_ref(refs.clone(), cx)),
                            )
                            .when(!reason.is_empty(), |d| {
                                d.child(div().text_color(t.text_secondary).child(reason.clone()))
                            })
                    })),
            );
    }
    if !details.remote.is_empty() {
        // what the server said, as a quote: a 2 px bar on the left
        body = body.child(label("The server said")).child(
            div()
                .flex()
                .flex_col()
                .gap(zpx(2.))
                .pl(SPACING())
                .border_l_2()
                .border_color(t.box_border_contrast)
                .children(
                    details
                        .remote
                        .iter()
                        .map(|line| div().text_color(t.text_secondary).child(line.clone())),
                ),
        );
    }
    if !details.hints.is_empty() {
        body = body.child(
            div()
                .text_size(FONT_SIZE_SM())
                .text_color(t.text_secondary)
                .child(format!("Hint: {}", details.hints.join(" "))),
        );
    }
    let header = div()
        .flex()
        .flex_row()
        .items_baseline()
        .gap(SPACING())
        .px(SPACING())
        .py(SPACING_HALF())
        .when(show_output, |d| d.border_b_1().border_color(t.box_border))
        .text_size(FONT_SIZE_SM())
        .text_color(t.text_secondary)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .font_family(mono_font())
                .child(git.command.clone()),
        )
        .child(div().flex_none().child(exit))
        .child(
            crate::widgets::link_button(
                "git-error-toggle",
                if show_output {
                    "Hide output"
                } else {
                    "Show output"
                },
                cx,
            )
            .text_size(FONT_SIZE_SM())
            .on_click(toggle),
        );
    let output = div()
        .id("git-error-output")
        .max_h(zpx(260.))
        .overflow_y_scroll()
        .p(SPACING())
        .font_family(mono_font())
        .text_size(FONT_SIZE())
        .line_height(zpx(18.))
        .text_color(t.text)
        .child(if git.output.is_empty() {
            "(no output)".to_string()
        } else {
            git.output.clone()
        })
        .with_scrollbar();
    body.child(
        div()
            .flex()
            .flex_col()
            .min_w_0()
            .rounded(BORDER_RADIUS())
            .border_1()
            .border_color(t.box_border)
            .bg(t.box_alt_background)
            .child(header)
            .when(show_output, |d| d.child(output)),
    )
    .into_any_element()
}
