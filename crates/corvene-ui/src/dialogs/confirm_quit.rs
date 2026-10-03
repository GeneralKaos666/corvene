//! `ConfirmQuit` - Corvene (`446-confirm-quit-while-busy`): Quit while a
//! clone, push, pull, fetch or update runs asks first. GHD quits at once
//! (`app/src/main-process/app-window.ts` has no quit guard).

use corvene_core::{Dispatcher, Popup};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::mac_or;
use crate::dialog::{DialogButton, DialogKind, dialog_with_kind};
use crate::theme::sizes::*;

pub struct ConfirmQuitDialog {
    busy: &'static str,
    /// The dialog this one covered, shown again on Cancel.
    previous: Option<Popup>,
}

impl ConfirmQuitDialog {
    pub fn new(busy: &'static str, previous: Option<Popup>) -> Self {
        Self { busy, previous }
    }
}

fn back_to(previous: &Option<Popup>, cx: &mut App) {
    match previous.clone() {
        Some(popup) => Dispatcher::show_popup(popup, cx),
        None => Dispatcher::close_popup(cx),
    }
}

impl Render for ConfirmQuitDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let previous = self.previous.clone();
        let close = move |_: &mut Window, cx: &mut App| back_to(&previous, cx);
        let previous = self.previous.clone();
        dialog_with_kind(
            "confirm-quit",
            DialogKind::Warning,
            mac_or("Quit Anyway?", "Quit anyway?"),
            div()
                .w(zpx(360.))
                .child(format!("{} Quitting now stops it.", self.busy)),
            vec![
                DialogButton {
                    id: "confirm-quit-cancel",
                    label: "Cancel".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(move |_, cx| back_to(&previous, cx)),
                },
                DialogButton {
                    id: "confirm-quit-ok",
                    label: mac_or("Quit Anyway", "Quit anyway").into(),
                    primary: true,
                    disabled: false,
                    on_click: Box::new(|_, cx| cx.quit()),
                },
            ],
            close,
            window,
            cx,
        )
    }
}
