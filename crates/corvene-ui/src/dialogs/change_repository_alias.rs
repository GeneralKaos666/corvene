//! `PopupType.ChangeRepositoryAlias` - GHD
//! `ui/change-repository-alias/change-repository-alias-dialog.tsx`:
//! "Create / Change Repository Alias", prefilled with the alias or the name,
//! OK disabled while empty.

use corvene_core::{AppState, Dispatcher};
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::mac_or;
use crate::dialog::{GroupButtonSpec, OkCancelButtonGroup, dialog};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::text_box;

pub struct ChangeRepositoryAliasDialog {
    state: Entity<AppState>,
    repo: u64,
    alias: Entity<InputState>,
}

impl ChangeRepositoryAliasDialog {
    pub fn new(
        state: Entity<AppState>,
        repo: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // `newAlias: repository.alias ?? repository.name`
        let initial = state
            .read(cx)
            .repository(repo)
            .map(|r| r.alias.clone().unwrap_or_else(|| r.name()))
            .unwrap_or_default();
        let alias = cx.new(|cx| InputState::new(window, cx));
        alias.update(cx, |s, cx| s.set_value(initial, window, cx));
        cx.observe(&alias, |_, _, cx| cx.notify()).detach();
        Self { state, repo, alias }
    }
}

impl Render for ChangeRepositoryAliasDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let t = cx.ghd();
        let (name, has_alias, is_github) = self
            .state
            .read(cx)
            .repository(self.repo)
            // `nameOf(repository)`: owner/name or the folder, not the alias
            .map(|r| {
                (
                    corvene_core::name_of(r),
                    r.alias.is_some(),
                    r.github.is_some(),
                )
            })
            .unwrap_or_default();
        let verb = if has_alias { "Change" } else { "Create" };
        let value = self.alias.read(cx).value().to_string();
        let disabled = value.is_empty();
        let repo = self.repo;
        let content = div()
            .flex()
            .flex_col()
            .gap(SPACING())
            .child(format!(
                "Choose a new alias for the repository \"{name}\". "
            ))
            .child(text_box("repository-alias", &self.alias, None, window, cx))
            .when(is_github, |d| {
                d.child(
                    div()
                        .text_color(t.text_secondary)
                        .child("This will not affect the original repository name on GitHub."),
                )
            });
        dialog(
            "dialog-change-repository-alias",
            format!("{verb} {}", mac_or("Repository Alias", "repository alias")),
            content,
            OkCancelButtonGroup {
                destructive: false,
                cancel: GroupButtonSpec {
                    id: "alias-cancel",
                    label: "Cancel".into(),
                    disabled: false,
                    on_click: Box::new(close),
                },
                ok: GroupButtonSpec {
                    id: "alias-ok",
                    label: format!("{verb} {}", mac_or("Alias", "alias")).into(),
                    disabled,
                    on_click: Box::new(move |_, cx| {
                        if disabled {
                            return;
                        }
                        Dispatcher::close_popup(cx);
                        Dispatcher::change_repository_alias(repo, Some(value.clone()), cx);
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
