//! "SSH key passphrase": asked when the key picked in Options ›
//! Integrations is encrypted (`Dispatcher::import_ssh_key`). Corvene on
//! Android only; on the desktop the system's ssh and its agent own the keys.

use std::path::PathBuf;

use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use corvene_core::Dispatcher;

use crate::dialog::{DialogButton, dialog};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::password_text_box;

pub struct SshKeyPassphraseDialog {
    path: PathBuf,
    wrong: bool,
    passphrase: Entity<InputState>,
}

impl SshKeyPassphraseDialog {
    pub fn new(path: PathBuf, wrong: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let passphrase = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Passphrase")
                .masked(true)
        });
        cx.observe(&passphrase, |_, _, cx| cx.notify()).detach();
        let handle = passphrase.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
        Self {
            path,
            wrong,
            passphrase,
        }
    }
}

impl Render for SshKeyPassphraseDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let value = self.passphrase.read(cx).value().to_string();
        let path = self.path.clone();
        let content = div()
            .w(crate::theme::fit_width(400.))
            .flex()
            .flex_col()
            .gap(SPACING())
            .child(
                "This key is protected by a passphrase. Corvene keeps its own copy without \
                 one, in storage no other application can read, so that fetches in the \
                 background can use it.",
            )
            .child(password_text_box(
                "ssh-key-passphrase-input",
                &self.passphrase,
                window,
                cx,
            ))
            .when(self.wrong, |d| {
                d.child(
                    div()
                        .text_size(FONT_SIZE_SM())
                        .text_color(t.error)
                        .child("That passphrase did not open the key."),
                )
            });
        dialog(
            "ssh-key-passphrase",
            "SSH key passphrase",
            content,
            vec![
                DialogButton {
                    id: "ssh-key-passphrase-cancel",
                    label: "Cancel".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(|_, cx| Dispatcher::close_popup(cx)),
                },
                DialogButton {
                    id: "ssh-key-passphrase-ok",
                    label: "Import".into(),
                    primary: true,
                    disabled: value.is_empty(),
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::close_popup(cx);
                        #[cfg(target_os = "android")]
                        Dispatcher::import_ssh_key(path.clone(), Some(value.clone()), cx);
                        #[cfg(not(target_os = "android"))]
                        let _ = (&path, &value);
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}
