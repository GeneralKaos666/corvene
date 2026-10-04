//! Corvene (`290-custom-repository-groups`): the repository list's "Move to
//! Group…" dialog. A group name (prefilled with the current one), the
//! groups already in use as links that fill it in, and "Remove from Group"
//! when the repository is in one. GHD has no custom groups: its list groups
//! by owner only (`ui/repositories-list/group-repositories.ts`).

use corvene_core::{AppState, Dispatcher};
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::mac_or;
use crate::dialog::{GroupButtonSpec, OkCancelButtonGroup, dialog};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{link_button, text_box};

pub struct MoveRepositoryToGroupDialog {
    state: Entity<AppState>,
    repo: u64,
    name: Entity<InputState>,
}

impl MoveRepositoryToGroupDialog {
    pub fn new(
        state: Entity<AppState>,
        repo: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let initial = state
            .read(cx)
            .repository(repo)
            .and_then(|r| r.group.clone())
            .unwrap_or_default();
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("Group name"));
        name.update(cx, |s, cx| s.set_value(initial, window, cx));
        cx.observe(&name, |_, _, cx| cx.notify()).detach();
        let handle = name.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
        Self { state, repo, name }
    }
}

/// The custom groups in use, sorted by name ignoring case.
pub fn custom_groups(state: &AppState) -> Vec<String> {
    let mut groups: Vec<String> = state
        .repositories
        .iter()
        .filter_map(|r| r.group.clone())
        .collect();
    groups.sort_by_key(|g| g.to_lowercase());
    groups.dedup();
    groups
}

impl Render for MoveRepositoryToGroupDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let t = cx.ghd().clone();
        let (name, current, groups) = {
            let s = self.state.read(cx);
            let repo = s.repository(self.repo);
            (
                repo.map(|r| r.name()).unwrap_or_default(),
                repo.and_then(|r| r.group.clone()),
                custom_groups(s),
            )
        };
        let value = self.name.read(cx).value().trim().to_string();
        let disabled = value.is_empty() || current.as_deref() == Some(value.as_str());
        let repo = self.repo;
        let others: Vec<String> = groups
            .into_iter()
            .filter(|g| current.as_deref() != Some(g.as_str()))
            .collect();
        let input = self.name.clone();
        let content = div()
            .flex()
            .flex_col()
            .gap(SPACING())
            .child(format!(
                "Choose the repository list group for \"{name}\". It shows above the owner groups."
            ))
            .child(text_box("repository-group", &self.name, None, window, cx))
            .when(!others.is_empty(), |d| {
                d.child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .gap(SPACING_HALF())
                        .child(div().text_color(t.text_secondary).child("Groups:"))
                        .children(others.into_iter().enumerate().map(|(ix, group)| {
                            let input = input.clone();
                            link_button(("repository-group-pick", ix), group.clone(), cx).on_click(
                                move |_, window, cx| {
                                    input.update(cx, |s, cx| s.set_value(group.clone(), window, cx))
                                },
                            )
                        })),
                )
            })
            .when_some(current, |d, current| {
                d.child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(SPACING_HALF())
                        .child(
                            div()
                                .text_color(t.text_secondary)
                                .child(format!("In \"{current}\".")),
                        )
                        .child(
                            link_button(
                                "repository-group-remove",
                                mac_or("Remove from Group", "Remove from group"),
                                cx,
                            )
                            .on_click(move |_, _, cx| {
                                Dispatcher::close_popup(cx);
                                Dispatcher::set_repository_group(repo, None, cx);
                            }),
                        ),
                )
            });
        dialog(
            "dialog-move-repository-to-group",
            mac_or("Move to Group", "Move to group"),
            content,
            OkCancelButtonGroup {
                destructive: false,
                cancel: GroupButtonSpec {
                    id: "repository-group-cancel",
                    label: "Cancel".into(),
                    disabled: false,
                    on_click: Box::new(close),
                },
                ok: GroupButtonSpec {
                    id: "repository-group-ok",
                    label: "Move".into(),
                    disabled,
                    on_click: Box::new(move |_, cx| {
                        if disabled {
                            return;
                        }
                        Dispatcher::close_popup(cx);
                        Dispatcher::set_repository_group(repo, Some(value.clone()), cx);
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
