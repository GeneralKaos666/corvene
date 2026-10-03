//! Android: "Move to shared storage" (`Popup::MoveToSharedStorage`; no GHD
//! equivalent, GitHub Desktop has no Android build). Opened from the Termux
//! error dialogs (`app_dialogs::IntegrationErrorDialog`) and Repository ›
//! Move to shared storage…
//!
//! Four states: the build cannot use shared storage (a note and Close);
//! "All files access" is missing (the button opens the system's page and the
//! dialog polls until it is granted); the form (destination + Choose…, a
//! note on what shared storage lacks, Move); the move in progress (a
//! progress bar, Cancel). A failed move shows why above the form again.

use std::path::PathBuf;
use std::time::Duration;

use corvene_core::{
    AfterSharedStorageMove, AppState, Dispatcher, SharedStorageAccess, SharedStorageMoveStage,
};
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::mac_or;
use crate::dialog::{DialogButton, dialog};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{button, labeled, text_box_opts};

pub struct MoveToSharedStorageDialog {
    state: Entity<AppState>,
    repo: u64,
    then: AfterSharedStorageMove,
    path: Entity<InputState>,
    /// Why the typed destination cannot be used (checked while typing).
    path_error: Option<String>,
    /// Waiting for "All files access" to be granted.
    polling: bool,
}

impl MoveToSharedStorageDialog {
    pub fn new(
        state: Entity<AppState>,
        repo: u64,
        then: AfterSharedStorageMove,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let default = Dispatcher::default_shared_storage_destination(repo, cx)
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let path = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("folder on shared storage")
                .default_value(default)
        });
        cx.observe(&path, |this, _, cx| {
            this.check_path(cx);
            cx.notify();
        })
        .detach();
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        let mut this = Self {
            state,
            repo,
            then,
            path,
            path_error: None,
            polling: false,
        };
        this.check_path(cx);
        this
    }

    fn check_path(&mut self, cx: &App) {
        let text = self.path.read(cx).value().to_string();
        self.path_error = Dispatcher::check_shared_storage_destination(self.repo, &text, cx).err();
    }

    /// The system's folder picker; the repository's folder name is appended
    /// to the picked folder, as the Clone dialog does.
    fn choose(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self
            .state
            .read(cx)
            .repository(self.repo)
            .and_then(|r| r.path.file_name().map(PathBuf::from));
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Move".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await
                && let Some(picked) = paths.into_iter().next()
            {
                let destination = match &name {
                    Some(name) => picked.join(name),
                    None => picked,
                };
                this.update_in(cx, |d, window, cx| {
                    d.path.update(cx, |s, cx| {
                        s.set_value(destination.display().to_string(), window, cx)
                    });
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    /// Checks once a second whether "All files access" was granted while the
    /// user is on the system's page; the form appears when it was.
    fn poll_permission(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.polling {
            return;
        }
        self.polling = true;
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let granted = matches!(
                    Dispatcher::shared_storage_access(),
                    SharedStorageAccess::Granted { .. }
                );
                let keep_polling = this
                    .update_in(cx, |this, window, cx| {
                        if granted {
                            this.polling = false;
                            if let Some(default) =
                                Dispatcher::default_shared_storage_destination(this.repo, cx)
                            {
                                this.path.update(cx, |s, cx| {
                                    s.set_value(default.display().to_string(), window, cx)
                                });
                            }
                            cx.notify();
                        }
                        !granted
                    })
                    .unwrap_or(false);
                if !keep_polling {
                    break;
                }
            }
        })
        .detach();
    }

    fn start_move(&mut self, cx: &mut Context<Self>) {
        let text = self.path.read(cx).value().to_string();
        match Dispatcher::check_shared_storage_destination(self.repo, &text, cx) {
            Ok(destination) => {
                self.path_error = None;
                Dispatcher::move_to_shared_storage(self.repo, destination, self.then.clone(), cx);
            }
            Err(message) => self.path_error = Some(message),
        }
        cx.notify();
    }
}

impl Render for MoveToSharedStorageDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let access = Dispatcher::shared_storage_access();
        if access == SharedStorageAccess::NeedsPermission {
            self.poll_permission(window, cx);
        }
        let t = cx.ghd();
        let repo = self.repo;
        let close = |_: &mut Window, cx: &mut App| {
            Dispatcher::dismiss_shared_storage_move(cx);
            Dispatcher::close_popup(cx);
        };
        let (name, on_shared_storage) = {
            let s = self.state.read(cx);
            match s.repository(repo) {
                Some(r) => (r.name(), Dispatcher::on_shared_storage(&r.path)),
                None => (String::new(), false),
            }
        };
        let moving = self
            .state
            .read(cx)
            .shared_storage_move
            .clone()
            .filter(|m| m.repo == repo);
        let title = mac_or("Move to Shared Storage", "Move to shared storage");
        let secondary = |text: &str| {
            div()
                .text_size(FONT_SIZE_SM())
                .line_height(zpx(16.5))
                .text_color(t.text_secondary)
                .child(text.to_string())
        };
        let close_button = DialogButton {
            id: "move-shared-close",
            label: "Close".into(),
            primary: true,
            disabled: false,
            on_click: Box::new(close),
        };
        let cancel_button = |disabled: bool| DialogButton {
            id: "move-shared-cancel",
            label: "Cancel".into(),
            primary: false,
            disabled,
            on_click: Box::new(close),
        };
        let content = div().w(crate::theme::fit_width(450.)).flex().flex_col();

        // the repository is there already (Repository › Move to shared storage…)
        if on_shared_storage {
            return dialog(
                "dialog-move-to-shared-storage",
                title,
                content.child(format!("\"{name}\" is already on shared storage.")),
                vec![close_button],
                close,
                window,
                cx,
            )
            .into_any_element();
        }

        let intro = format!(
            "\"{name}\" is in Corvene's own storage, which no other app can read. On shared \
             storage Termux and other apps reach it too, and it stays when Corvene is \
             uninstalled."
        );

        match access {
            SharedStorageAccess::Unavailable => dialog(
                "dialog-move-to-shared-storage",
                title,
                content
                    .child(div().mb(SPACING()).child(intro))
                    .child(secondary(
                        "This build of Corvene cannot use shared storage in place.",
                    )),
                vec![close_button],
                close,
                window,
                cx,
            )
            .into_any_element(),
            SharedStorageAccess::NeedsPermission => dialog(
                "dialog-move-to-shared-storage",
                title,
                content
                    .child(div().mb(SPACING()).child(intro))
                    .child(secondary(
                        "Shared storage needs \"All files access\". Allow it for Corvene on \
                         the system page that opens, then come back here.",
                    )),
                vec![
                    cancel_button(false),
                    DialogButton {
                        id: "move-shared-allow",
                        label: "Allow \"All files access\"".into(),
                        primary: true,
                        disabled: false,
                        on_click: Box::new(|_, cx| Dispatcher::request_all_files_access(cx)),
                    },
                ],
                close,
                window,
                cx,
            )
            .into_any_element(),
            SharedStorageAccess::Granted { .. } => {
                let in_progress = moving
                    .as_ref()
                    .filter(|m| !m.stage.is_final())
                    .map(|m| m.stage.clone());
                let failed = moving.as_ref().and_then(|m| match &m.stage {
                    SharedStorageMoveStage::Failed(message) => Some(message.clone()),
                    _ => None,
                });
                if let Some(stage) = in_progress {
                    let (value, text) = match &stage {
                        SharedStorageMoveStage::Copying { done, total } => (
                            if *total == 0 {
                                0.
                            } else {
                                *done as f32 / *total as f32
                            },
                            if *total == 0 {
                                "Counting the files…".to_string()
                            } else {
                                format!("Copying {done} of {total} files and folders…")
                            },
                        ),
                        SharedStorageMoveStage::Checking => {
                            (1., "Checking the copy with Git…".to_string())
                        }
                        SharedStorageMoveStage::Failed(_) => (0., String::new()),
                    };
                    let cancellable = matches!(stage, SharedStorageMoveStage::Copying { .. });
                    return dialog(
                        "dialog-move-to-shared-storage",
                        title,
                        content
                            .child(div().mb(SPACING_DOUBLE()).child(intro))
                            .child(
                                // <progress>, as the multi-commit operation's
                                div()
                                    .w_full()
                                    .h(zpx(6.))
                                    .mb(SPACING())
                                    .rounded(zpx(3.))
                                    .bg(t.box_alt_background)
                                    .overflow_hidden()
                                    .child(
                                        div().h_full().w(relative(value.clamp(0., 1.))).bg(t.link),
                                    ),
                            )
                            .child(secondary(&text)),
                        vec![DialogButton {
                            id: "move-shared-stop",
                            label: "Cancel".into(),
                            primary: false,
                            disabled: !cancellable,
                            on_click: Box::new(|_, cx| Dispatcher::cancel_shared_storage_move(cx)),
                        }],
                        // a dismissed dialog leaves the move running
                        |_, cx| Dispatcher::close_popup(cx),
                        window,
                        cx,
                    )
                    .into_any_element();
                }

                let weak = cx.weak_entity();
                let can_move = self.path_error.is_none();
                let field = div()
                    .flex()
                    .flex_row()
                    .items_end()
                    .gap(SPACING())
                    .child(labeled(
                        "New location",
                        text_box_opts("move-shared-path", &self.path, None, false, window, cx),
                        cx,
                    ))
                    .child(
                        button("move-shared-choose", "Choose…", cx)
                            .flex_none()
                            .on_click(cx.listener(|this, _, window, cx| this.choose(window, cx))),
                    );
                dialog(
                    "dialog-move-to-shared-storage",
                    title,
                    content
                        .children(failed.map(|message| {
                            div()
                                .mb(SPACING_DOUBLE())
                                .text_color(t.error)
                                .child(message)
                        }))
                        .child(div().mb(SPACING_DOUBLE()).child(intro))
                        .child(field)
                        .children(self.path_error.clone().map(|message| {
                            div()
                                .mt(SPACING())
                                .text_size(FONT_SIZE_SM())
                                .line_height(zpx(16.5))
                                .text_color(t.error)
                                .child(message)
                        }))
                        .child(div().mt(SPACING_DOUBLE()).child(secondary(
                            "Shared storage keeps neither symbolic links nor file modes, and \
                             Git hooks do not run there. The folder in Corvene's own storage is \
                             removed once the move is complete.",
                        ))),
                    vec![
                        cancel_button(false),
                        DialogButton {
                            id: "move-shared-move",
                            label: "Move".into(),
                            primary: true,
                            disabled: !can_move,
                            on_click: Box::new(move |_, cx| {
                                weak.update(cx, |this, cx| this.start_move(cx)).ok();
                            }),
                        },
                    ],
                    close,
                    window,
                    cx,
                )
                .into_any_element()
            }
        }
    }
}
