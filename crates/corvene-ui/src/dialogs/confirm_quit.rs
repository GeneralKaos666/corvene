//! `ConfirmQuit` - Corvene (`418-confirm-quit-while-busy`): Quit while a
//! clone, push, pull, fetch or update runs asks first. GHD quits at once
//! (`app/src/main-process/app-window.ts` has no quit guard).

use corvene_core::Dispatcher;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::mac_or;
use crate::dialog::{DialogButton, DialogKind, dialog_with_kind};
use crate::theme::sizes::*;

/// Cancel closes it, which shows the dialog it covered again (the popup
/// stack keeps it).
pub struct ConfirmQuitDialog {
    busy: &'static str,
}

impl ConfirmQuitDialog {
    pub fn new(busy: &'static str) -> Self {
        Self { busy }
    }
}

impl Render for ConfirmQuitDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
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
                    on_click: Box::new(|_, cx| Dispatcher::close_popup(cx)),
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
