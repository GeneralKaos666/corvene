//! Submodules: a Corvene extra with no GHD counterpart (flag
//! `1111-submodules`, see `.docs/deviations.md`). Every submodule
//! (`RepositoryState::submodules`) with its path, URL, recorded and
//! checked-out commit and state; a row's button runs the action it needs
//! (Initialize or Update), its `⋯` button and right-click menu have all of
//! them and Open as Repository; the bar above the list runs them for every
//! submodule, with or without `--recursive`.

use corvene_core::submodules::{SubmoduleAction, SubmodulesState};
use corvene_core::{AppState, Dispatcher};
use corvene_git::{SubmoduleDetails, SubmoduleState};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::{IS_MAC, MenuItem, mac_or};
use crate::dialog::{DialogButton, dialog};
use crate::icons::{Octicon, octicon};
use crate::scrollbar::ScrollbarExt;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{checkbox_row, small_button};

pub struct SubmodulesDialog {
    state: Entity<AppState>,
    repo: u64,
}

impl SubmodulesDialog {
    pub fn new(state: Entity<AppState>, repo: u64, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self { state, repo }
    }

    fn panel(&self, cx: &App) -> SubmodulesState {
        self.state
            .read(cx)
            .repo_states
            .get(&self.repo)
            .and_then(|rs| rs.submodules.clone())
            .unwrap_or_default()
    }
}

fn short(sha: &Option<String>) -> String {
    sha.as_deref()
        .map(|s| s.chars().take(7).collect())
        .unwrap_or_else(|| "–".to_string())
}

fn state_label(state: SubmoduleState) -> &'static str {
    match state {
        SubmoduleState::NotInitialized => "Not initialized",
        SubmoduleState::NotCloned => "Not cloned",
        SubmoduleState::UpToDate => "Up to date",
        SubmoduleState::DifferentCommit => "Different commit",
        SubmoduleState::Conflicted => "Conflicted",
    }
}

/// The action a row's button runs.
fn suggested_action(submodule: &SubmoduleDetails) -> Option<(SubmoduleAction, &'static str)> {
    match submodule.state {
        SubmoduleState::NotInitialized => Some((SubmoduleAction::Init, "Initialize")),
        SubmoduleState::NotCloned | SubmoduleState::DifferentCommit => {
            Some((SubmoduleAction::Update, "Update"))
        }
        _ if submodule.needs_sync() => Some((SubmoduleAction::Sync, "Sync")),
        _ => None,
    }
}

fn row_menu(repo: u64, submodule: &SubmoduleDetails, busy: bool) -> Vec<MenuItem> {
    let path = submodule.path.clone();
    let action = |label: &'static str, mac: &'static str, action: SubmoduleAction| {
        let path = path.clone();
        MenuItem::new(mac_or(mac, label), move |_, cx| {
            Dispatcher::run_submodule_action(repo, action, vec![path.clone()], cx)
        })
        .enabled(!busy)
    };
    let open = path.clone();
    let copy_path = path.clone();
    let url = submodule.url.clone();
    let checked_out = submodule.checked_out.is_some();
    vec![
        action("Initialize", "Initialize", SubmoduleAction::Init),
        action("Update", "Update", SubmoduleAction::Update),
        action("Sync URL", "Sync URL", SubmoduleAction::Sync),
        MenuItem::separator(),
        MenuItem::new(
            mac_or("Open as Repository", "Open as repository"),
            move |_, cx| Dispatcher::open_submodule_repository(repo, open.clone(), cx),
        )
        .enabled(checked_out),
        MenuItem::separator(),
        MenuItem::new(mac_or("Copy Path", "Copy path"), move |_, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(copy_path.clone()))
        }),
        MenuItem::new(mac_or("Copy URL", "Copy URL"), move |_, cx| {
            if let Some(url) = &url {
                cx.write_to_clipboard(ClipboardItem::new_string(url.clone()))
            }
        })
        .enabled(submodule.url.is_some()),
    ]
}

fn submodule_row(
    repo: u64,
    ix: usize,
    submodule: &SubmoduleDetails,
    busy: bool,
    cx: &App,
) -> AnyElement {
    let t = cx.ghd();
    let hover_bg = t.list_item_hover_background;
    let (pill_fg, pill_border) = match submodule.state {
        SubmoduleState::UpToDate => (t.color_new, t.color_new),
        SubmoduleState::DifferentCommit | SubmoduleState::Conflicted => {
            (t.file_warning, t.file_warning_border)
        }
        _ => (t.text_secondary, t.box_border),
    };
    let commits = match (&submodule.recorded, &submodule.checked_out) {
        (recorded, None) => format!("Recorded {}", short(recorded)),
        (recorded, Some(_)) if submodule.state == SubmoduleState::UpToDate => {
            format!("At {}", short(recorded))
        }
        (recorded, checked_out) => format!(
            "Recorded {} · checked out {}",
            short(recorded),
            short(checked_out)
        ),
    };
    let commits = match &submodule.describe {
        Some(describe) if submodule.checked_out.is_some() => format!("{commits} ({describe})"),
        _ => commits,
    };
    let url = match (&submodule.url, submodule.needs_sync()) {
        (Some(url), false) => url.clone(),
        (Some(url), true) => format!("{url} (not synced)"),
        (None, _) => "No URL in .gitmodules".to_string(),
    };
    let menu_for_right = row_menu(repo, submodule, busy);
    let menu_for_button = row_menu(repo, submodule, busy);
    div()
        .id(("submodule-row", ix))
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(SPACING())
        .px(SPACING())
        .py(SPACING_HALF())
        .border_b_1()
        .border_color(t.box_border)
        .hover(move |s| s.bg(hover_bg))
        .on_mouse_down(
            MouseButton::Right,
            move |ev: &MouseDownEvent, window, cx| {
                cx.stop_propagation();
                crate::native_menu::show_context_menu(
                    menu_for_right.clone(),
                    ev.position,
                    window,
                    cx,
                );
            },
        )
        .child(octicon(Octicon::FileSubmodule, t.text_secondary).flex_none())
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .truncate()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(submodule.path.clone()),
                )
                .child(
                    div()
                        .truncate()
                        .text_size(FONT_SIZE_SM())
                        .text_color(t.text_secondary)
                        .child(url),
                )
                .child(
                    div()
                        .truncate()
                        .text_size(FONT_SIZE_SM())
                        .text_color(t.text_secondary)
                        .child(commits),
                ),
        )
        .child(
            div()
                .flex_none()
                .px(SPACING_HALF())
                .border_1()
                .border_color(pill_border)
                .rounded(zpx(10.))
                .text_size(FONT_SIZE_SM())
                .text_color(pill_fg)
                .child(state_label(submodule.state)),
        )
        .children(suggested_action(submodule).map(|(action, label)| {
            let path = submodule.path.clone();
            small_button(("submodule-action", ix), label, cx)
                .flex_none()
                .when(busy, |d| d.opacity(0.6))
                .when(!busy, |d| {
                    d.on_click(move |_, _, cx| {
                        Dispatcher::run_submodule_action(repo, action, vec![path.clone()], cx)
                    })
                })
        }))
        .child(
            small_button(("submodule-more", ix), "", cx)
                .flex_none()
                .child(octicon(Octicon::KebabHorizontal, t.text_secondary))
                .on_mouse_down(MouseButton::Left, move |ev: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    crate::native_menu::show_context_menu(
                        menu_for_button.clone(),
                        ev.position,
                        window,
                        cx,
                    );
                }),
        )
        .into_any_element()
}

impl Render for SubmodulesDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd().clone();
        let repo = self.repo;
        let close = move |_: &mut Window, cx: &mut App| Dispatcher::close_submodules(repo, cx);
        let panel = self.panel(cx);
        let busy = panel.busy.is_some();
        let any = !panel.list.is_empty();

        let message: Option<String> = if panel.list.is_empty() && panel.loading {
            Some("Looking for submodules…".into())
        } else if panel.list.is_empty() && panel.error.is_none() {
            Some("This repository has no submodules.".into())
        } else {
            None
        };
        let list = div()
            .id("submodules-list")
            .h(zpx(300.))
            .overflow_y_scroll()
            .border_1()
            .border_color(t.box_border)
            .rounded(BORDER_RADIUS())
            .flex()
            .flex_col()
            .when_some(message, |d, message| {
                d.child(
                    div()
                        .p(SPACING())
                        .text_color(t.text_secondary)
                        .child(message),
                )
            })
            .children(
                panel
                    .list
                    .iter()
                    .enumerate()
                    .map(|(ix, s)| submodule_row(repo, ix, s, busy, cx)),
            )
            .with_scrollbar();

        let all_button = |id: &'static str, label: &'static str, action: SubmoduleAction| {
            small_button(id, label, cx)
                .flex_none()
                .when(busy || !any, |d| d.opacity(0.6))
                .when(!busy && any, |d| {
                    d.on_click(move |_, _, cx| {
                        Dispatcher::run_submodule_action(repo, action, Vec::new(), cx)
                    })
                })
        };
        let bar = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(SPACING_HALF())
            .child(div().flex_1().child(checkbox_row(
                "submodules-recursive",
                panel.recursive,
                "Recursive (nested submodules too)",
                move |value, _, cx| Dispatcher::set_submodules_recursive(repo, value, cx),
                cx,
            )))
            .child(all_button(
                "submodules-init-all",
                if IS_MAC {
                    "Initialize All"
                } else {
                    "Initialize all"
                },
                SubmoduleAction::Init,
            ))
            .child(all_button(
                "submodules-update-all",
                if IS_MAC { "Update All" } else { "Update all" },
                SubmoduleAction::Update,
            ))
            .child(all_button(
                "submodules-sync-all",
                if IS_MAC { "Sync All" } else { "Sync all" },
                SubmoduleAction::Sync,
            ));

        let content = div()
            .w(crate::theme::fit_width(560.))
            .flex()
            .flex_col()
            .gap(SPACING())
            .child(bar)
            .child(list)
            .when_some(panel.busy.clone(), |d, busy| {
                d.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(SPACING_HALF())
                        .text_color(t.text_secondary)
                        .child(octicon(Octicon::Sync, t.text_secondary).flex_none())
                        .child(busy),
                )
            })
            .when_some(panel.error.clone(), |d, error| {
                d.child(
                    div()
                        .id("submodules-error")
                        .max_h(zpx(90.))
                        .overflow_y_scroll()
                        .p(SPACING_HALF())
                        .border_1()
                        .border_color(t.file_warning_border)
                        .bg(t.file_warning_background)
                        .rounded(BORDER_RADIUS())
                        .text_size(FONT_SIZE_SM())
                        .child(error),
                )
            });

        let buttons = vec![DialogButton {
            id: "submodules-close",
            label: "Close".into(),
            primary: true,
            disabled: false,
            on_click: Box::new(close),
        }];
        dialog(
            "submodules",
            "Submodules",
            content,
            buttons,
            close,
            window,
            cx,
        )
    }
}
