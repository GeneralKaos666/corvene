//! Corvene addition (flag `768-ignore-custom-pattern`, no GHD counterpart):
//! the changes file menu's "Ignore with Pattern…" opens this dialog,
//! prefilled with the file's escaped path, and appends the edited pattern to
//! the root `.gitignore` like the other Ignore items
//! (`Dispatcher::ignore_patterns`).

use corvene_core::Dispatcher;
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::mac_or;
use crate::dialog::{DialogButton, dialog};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::text_box;

pub struct IgnoreWithPatternDialog {
    repo: u64,
    pattern: Entity<InputState>,
}

impl IgnoreWithPatternDialog {
    pub fn new(repo: u64, pattern: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx));
        input.update(cx, |s, cx| s.set_value(pattern, window, cx));
        cx.observe(&input, |_, _, cx| cx.notify()).detach();
        Self {
            repo,
            pattern: input,
        }
    }
}

impl Render for IgnoreWithPatternDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let t = cx.ghd();
        let value = self.pattern.read(cx).value().trim().to_string();
        let disabled = value.is_empty() || value.starts_with('#');
        let repo = self.repo;
        let content = div()
            .flex()
            .flex_col()
            .gap(SPACING())
            .child("Add this pattern to the repository's .gitignore file:")
            .child(text_box("ignore-pattern", &self.pattern, None, window, cx))
            .child(
                div()
                    .text_color(t.text_secondary)
                    .child("* matches within a folder, ** across folders, a trailing / matches folders only and a leading / anchors the pattern at the repository root."),
            );
        dialog(
            "dialog-ignore-with-pattern",
            mac_or("Ignore with Pattern", "Ignore with pattern"),
            content,
            vec![
                DialogButton {
                    id: "ignore-pattern-cancel",
                    label: "Cancel".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(close),
                },
                DialogButton {
                    id: "ignore-pattern-ok",
                    label: mac_or("Ignore", "Ignore").into(),
                    primary: true,
                    disabled,
                    on_click: Box::new(move |_, cx| {
                        if disabled {
                            return;
                        }
                        Dispatcher::close_popup(cx);
                        Dispatcher::ignore_patterns(repo, vec![value.clone()], cx);
                    }),
                },
            ],
            close,
            window,
            cx,
        )
    }
}
