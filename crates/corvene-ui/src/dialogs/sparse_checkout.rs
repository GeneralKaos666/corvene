//! Sparse Checkout: a Corvene extra with no GHD counterpart (flag
//! `1112-sparse-checkout`, see `.docs/deviations.md`). `HEAD`'s folders
//! (`RepositoryState::sparse_editor`) as a tree with a checkbox each,
//! collapsed below the top level; ticking a folder brings everything in it
//! (cone mode, `corvene_core::sparse_checkout::SparseSelection`). Apply runs
//! `git sparse-checkout set --cone`; Turn Off, while it is on, `git
//! sparse-checkout disable`. With sparse checkout off every folder starts
//! ticked, as everything is checked out.

use std::collections::{BTreeMap, BTreeSet};

use corvene_core::sparse_checkout::{SparseEditor, SparseSelection, Tick};
use corvene_core::{AppState, Dispatcher};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::mac_or;
use crate::dialog::{DialogButton, dialog};
use crate::icons::{Octicon, octicon};
use crate::scrollbar::ScrollbarExt;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::checkbox_tristate;

pub struct SparseCheckoutDialog {
    state: Entity<AppState>,
    repo: u64,
    /// Set from the repository's patterns once they are read.
    selection: Option<SparseSelection>,
    expanded: BTreeSet<String>,
    /// Parent ("" for the top) → child folders, built once.
    children: BTreeMap<String, Vec<String>>,
}

fn parent_of(dir: &str) -> &str {
    dir.rfind('/').map_or("", |i| &dir[..i])
}

fn name_of(dir: &str) -> &str {
    dir.rfind('/').map_or(dir, |i| &dir[i + 1..])
}

impl SparseCheckoutDialog {
    pub fn new(state: Entity<AppState>, repo: u64, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self {
            state,
            repo,
            selection: None,
            expanded: BTreeSet::new(),
            children: BTreeMap::new(),
        }
    }

    fn editor(&self, cx: &App) -> SparseEditor {
        self.state
            .read(cx)
            .repo_states
            .get(&self.repo)
            .and_then(|rs| rs.sparse_editor.clone())
            .unwrap_or_else(|| SparseEditor {
                loading: true,
                ..Default::default()
            })
    }

    /// The first render after the folders came in.
    fn init(&mut self, editor: &SparseEditor) {
        if self.selection.is_some() || editor.loading {
            return;
        }
        for dir in &editor.directories {
            self.children
                .entry(parent_of(dir).to_string())
                .or_default()
                .push(dir.clone());
        }
        let selection = if editor.checkout.enabled && editor.checkout.cone {
            SparseSelection::from_patterns(&editor.checkout.patterns)
        } else if editor.checkout.enabled {
            SparseSelection::default()
        } else {
            let mut all = SparseSelection::default();
            all.selected
                .extend(self.children.get("").cloned().unwrap_or_default());
            all
        };
        // the ticked folders' parents start open
        for dir in &selection.selected {
            let mut parent = parent_of(dir);
            while !parent.is_empty() {
                self.expanded.insert(parent.to_string());
                parent = parent_of(parent);
            }
        }
        self.selection = Some(selection);
    }

    /// The rows shown: (folder, depth), children under open parents.
    fn rows(&self) -> Vec<(String, usize)> {
        let mut rows = Vec::new();
        let mut stack: Vec<(String, usize)> = self
            .children
            .get("")
            .map(|top| top.iter().rev().map(|d| (d.clone(), 0)).collect())
            .unwrap_or_default();
        while let Some((dir, depth)) = stack.pop() {
            if self.expanded.contains(&dir)
                && let Some(children) = self.children.get(&dir)
            {
                stack.extend(children.iter().rev().map(|c| (c.clone(), depth + 1)));
            }
            rows.push((dir, depth));
        }
        rows
    }
}

impl Render for SparseCheckoutDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd().clone();
        let repo = self.repo;
        let close = move |_: &mut Window, cx: &mut App| Dispatcher::close_sparse_checkout(repo, cx);
        let editor = self.editor(cx);
        self.init(&editor);
        let weak = cx.weak_entity();
        let hover_bg = t.list_item_hover_background;
        let directories = std::rc::Rc::new(editor.directories.clone());

        let message: Option<String> = if let Some(error) = &editor.error {
            Some(error.clone())
        } else if editor.loading {
            Some("Reading the folders…".into())
        } else if editor.directories.is_empty() {
            Some("This repository has no folders.".into())
        } else {
            None
        };
        let rows = self.rows();
        let selection = self.selection.clone().unwrap_or_default();
        let list = div()
            .id("sparse-checkout-list")
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
            .children(rows.iter().enumerate().map(|(ix, (dir, depth))| {
                let has_children = self.children.contains_key(dir);
                let open = self.expanded.contains(dir);
                let tick = match selection.tick_of(dir) {
                    Tick::On => Some(true),
                    Tick::Partly => None,
                    Tick::Off => Some(false),
                };
                let toggle_dir = dir.clone();
                let expand_dir = dir.clone();
                let toggle_weak = weak.clone();
                let expand_weak = weak.clone();
                let directories = directories.clone();
                div()
                    .id(("sparse-row", ix))
                    .flex_none()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING_HALF())
                    .pl(SPACING() + zpx(16.) * *depth as f32)
                    .pr(SPACING())
                    .py(zpx(3.))
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover_bg))
                    .on_click(move |_, _, cx| {
                        toggle_weak
                            .update(cx, |this, cx| {
                                if let Some(sel) = this.selection.as_mut() {
                                    sel.toggle(&toggle_dir, &directories);
                                }
                                cx.notify();
                            })
                            .ok();
                    })
                    .child(
                        div()
                            .id(("sparse-disclosure", ix))
                            .w(zpx(16.))
                            .flex_none()
                            .when(has_children, |d| {
                                d.child(octicon(
                                    if open {
                                        Octicon::ChevronDown
                                    } else {
                                        Octicon::ChevronRight
                                    },
                                    t.text_secondary,
                                ))
                                .on_click(move |_, _, cx| {
                                    cx.stop_propagation();
                                    expand_weak
                                        .update(cx, |this, cx| {
                                            if !this.expanded.remove(&expand_dir) {
                                                this.expanded.insert(expand_dir.clone());
                                            }
                                            cx.notify();
                                        })
                                        .ok();
                                })
                            }),
                    )
                    .child(checkbox_tristate(("sparse-check", ix), tick, false, cx))
                    .child(octicon(Octicon::FileDirectory, t.text_secondary).flex_none())
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(name_of(dir).to_string()),
                    )
            }))
            .with_scrollbar();

        let enabled = editor.checkout.enabled;
        let count = selection.selected.len();
        let content = div()
            .w(crate::theme::fit_width(480.))
            .flex()
            .flex_col()
            .gap(SPACING())
            .child(
                "Only the ticked folders are checked out, each with everything inside it. The \
                 files at the top of the repository are always checked out.",
            )
            .when(enabled && !editor.checkout.cone, |d| {
                d.child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(SPACING_HALF())
                        .text_color(t.text_secondary)
                        .child(octicon(Octicon::Alert, t.text_secondary).flex_none())
                        .child(div().flex_1().min_w_0().child(
                            "This repository's sparse checkout uses patterns. Applying \
                             replaces them with the ticked folders.",
                        )),
                )
            })
            .child(list)
            .child(div().text_color(t.text_secondary).child(match count {
                0 => "Only the files at the top will be checked out.".to_string(),
                1 => "1 folder selected".to_string(),
                n => format!("{n} folders selected"),
            }));

        let folders = selection.folders();
        let mut buttons = vec![
            DialogButton {
                id: "sparse-checkout-cancel",
                label: "Cancel".into(),
                primary: false,
                disabled: false,
                on_click: Box::new(close),
            },
            DialogButton {
                id: "sparse-checkout-apply",
                label: "Apply".into(),
                primary: true,
                disabled: editor.loading || editor.error.is_some(),
                on_click: Box::new(move |_, cx| {
                    Dispatcher::set_sparse_checkout(repo, folders.clone(), cx)
                }),
            },
        ];
        if enabled {
            buttons.insert(
                0,
                DialogButton {
                    id: "sparse-checkout-disable",
                    label: mac_or("Turn Off Sparse Checkout", "Turn off sparse checkout").into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(move |_, cx| Dispatcher::disable_sparse_checkout(repo, cx)),
                },
            );
        }
        dialog(
            "sparse-checkout",
            mac_or("Sparse Checkout", "Sparse checkout"),
            content,
            buttons,
            close,
            window,
            cx,
        )
    }
}
