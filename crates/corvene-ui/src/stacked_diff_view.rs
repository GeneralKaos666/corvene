//! Corvene `1311-stacked-diff`: every changed file's diff in one scrolling
//! list, each under its own `diff_header`, for the Changes tab (the files
//! included in the next commit, or the ⌘/⇧-selected ones) and for History
//! (every file of the selected commit, or the ⌘/⇧-selected ones of flag
//! `810`). GitHub Desktop shows one file at a time (`ui/history/
//! selected-commits.tsx` `renderDiff`, `ui/repository.tsx`
//! `renderContentForChanges`; desktop/desktop#5218, #18184).
//!
//! One `gpui::list` holds every file's items: a header item (the
//! `diff_header`, a fold chevron, and the file's warnings), then one item
//! per diff row, or one panel item for a binary, image, empty or too-large
//! diff, or one placeholder while the diff loads. Each file is a
//! [`DiffView`] of its own (`DiffView::new_stacked`) that is never rendered
//! as a view: it keeps the file's rows, expansion, highlighting, line and
//! text selection, and lends its rows to this list through
//! `DiffView::stacked_body`, so the rows look and behave exactly as in the
//! single-file diff (hunk handles, check marks, context menus, ⌥-click).
//! A file's diff is requested when its header is first rendered (plus the
//! next two files), so a big commit loads as it scrolls.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use corvene_core::stacked_diff::{StackKind, StackedFile};
use corvene_core::{AppState, Diff, Dispatcher};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::actions::Copy;
use crate::diff_view::{
    DIFF_LINE_HEIGHT, DiffSource, DiffView, StackedBody, StackedShape, diff_header,
};
use crate::diff_view_rows::{render_row, render_split_row};
use crate::icons::{Octicon, octicon};
use crate::scrollbar::{gutter, scrollbar};
use crate::theme::sizes::*;
use crate::theme::{ActiveGhdTheme, mono_font, ui_font};
use crate::widgets::{GhdTooltip, IconButtonA11y};

/// Height of the placeholder item while a file's diff loads.
const LOADING_ROWS: f32 = 3.;
/// Height of a panel item (binary, empty, too large, submodule): room for
/// the too-large panel's message and its two `762` buttons.
const PANEL_HEIGHT: f32 = 160.;
/// Height of an image diff's item.
const IMAGE_HEIGHT: f32 = 420.;
/// How many files past the one rendered are asked for ahead.
const PREFETCH: usize = 2;

/// One file of the stack and the `DiffView` that holds its rows.
struct Slot {
    file: Rc<StackedFile>,
    view: Entity<DiffView>,
    /// Items this file took in the list at the last frame.
    count: usize,
    shape: StackedShape,
    _observe: Subscription,
}

pub struct StackedDiffView {
    state: Entity<AppState>,
    source: DiffSource,
    files: Vec<Slot>,
    /// History: the ⌘/⇧-selected paths (flag `810`) the owner passes in.
    multi: Vec<String>,
    list_state: ListState,
    /// Files folded by a header click.
    folded: HashSet<String>,
    focus_handle: FocusHandle,
    /// The tab's selected file as last seen; a change scrolls to it.
    revealed: Option<String>,
    reveal_pending: Option<String>,
}

impl StackedDiffView {
    pub fn new(state: Entity<AppState>, source: DiffSource, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self {
            state,
            source,
            files: Vec::new(),
            multi: Vec::new(),
            list_state: ListState::new(0, ListAlignment::Top, zpx(400.)),
            folded: HashSet::new(),
            focus_handle: cx.focus_handle().tab_stop(true),
            revealed: None,
            reveal_pending: None,
        }
    }

    /// Like `DiffView::embed`: a cached element filling the column.
    pub fn embed(view: &Entity<Self>) -> impl IntoElement + use<> {
        view.clone()
            .cached(StyleRefinement::default().flex_1().min_h_0().w_full())
    }

    /// History: the ⌘/⇧-selected paths (file-list order).
    pub fn set_multi(&mut self, multi: Vec<String>, cx: &mut Context<Self>) {
        if self.multi != multi {
            self.multi = multi;
            cx.notify();
        }
    }

    /// The files the tab stacks now, or `None` for the single-file diff.
    fn stack(&self, cx: &App) -> Option<Vec<StackedFile>> {
        let s = self.state.read(cx);
        let rs = s.selected_state()?;
        match self.source {
            DiffSource::WorkingDirectory => corvene_core::stacked_diff::working_stack(s, rs),
            DiffSource::Commit => corvene_core::stacked_diff::commit_stack(s, rs, &self.multi),
            _ => None,
        }
    }

    /// Whose diffs the stack reads.
    fn kind(&self, cx: &App) -> Option<StackKind> {
        match self.source {
            DiffSource::WorkingDirectory => Some(StackKind::Working),
            DiffSource::Commit => self
                .state
                .read(cx)
                .selected_state()
                .map(Dispatcher::commit_stack_kind),
            _ => None,
        }
    }

    /// Bring `files` in: `DiffView`s are kept by path (their rows,
    /// expansion and selection with them), the list is rebuilt from the
    /// kept files' item counts, and the scroll top stays on the file it
    /// was on.
    fn set_files(&mut self, files: Vec<StackedFile>, cx: &mut Context<Self>) {
        if self.files.len() == files.len()
            && self
                .files
                .iter()
                .zip(&files)
                .all(|(slot, f)| *slot.file == *f)
        {
            return;
        }
        let top = self.list_state.logical_scroll_top();
        let top_file = self
            .file_at(top.item_ix)
            .map(|(ix, local)| (self.files[ix].file.path.clone(), local, top.offset_in_item));
        let mut old: HashMap<String, Slot> = self
            .files
            .drain(..)
            .map(|slot| (slot.file.path.clone(), slot))
            .collect();
        for file in files {
            let slot = match old.remove(&file.path) {
                Some(mut slot) => {
                    slot.file = Rc::new(file);
                    slot
                }
                None => {
                    let (state, source, path) =
                        (self.state.clone(), self.source, file.path.clone());
                    let view = cx.new(|cx| DiffView::new_stacked(state, source, path, cx));
                    let observe = cx.observe(&view, |_, _, cx| cx.notify());
                    Slot {
                        file: Rc::new(file),
                        view,
                        count: 0,
                        shape: StackedShape::Loading,
                        _observe: observe,
                    }
                }
            };
            self.files.push(slot);
        }
        let total: usize = self.files.iter().map(|s| s.count).sum();
        self.list_state.reset(total);
        if let Some((path, local, offset)) = top_file
            && let Some(ix) = self.files.iter().position(|s| s.file.path == path)
        {
            let start: usize = self.files[..ix].iter().map(|s| s.count).sum();
            let local = local.min(self.files[ix].count.saturating_sub(1));
            self.list_state.scroll_to(ListOffset {
                item_ix: start + local,
                offset_in_item: offset,
            });
        }
        cx.notify();
    }

    /// The file and the item within it at list index `ix` (as of the
    /// counts in `files`).
    fn file_at(&self, ix: usize) -> Option<(usize, usize)> {
        let mut start = 0;
        for (i, slot) in self.files.iter().enumerate() {
            if ix < start + slot.count {
                return Some((i, ix - start));
            }
            start += slot.count;
        }
        None
    }

    /// A header click folds or unfolds the file.
    fn toggle_fold(&mut self, path: String, cx: &mut Context<Self>) {
        if !self.folded.remove(&path) {
            self.folded.insert(path);
        }
        cx.notify();
    }

    /// Every file's `DiffView`, for the mouse events the rows need from
    /// their container (`DiffView::render` handles these for one file).
    fn each_view(&self, cx: &mut Context<Self>, f: impl Fn(&mut DiffView, &mut Context<DiffView>)) {
        for slot in &self.files {
            slot.view.update(cx, |view, cx| f(view, cx));
        }
    }
}

/// The items a shape takes after the header.
fn body_items(shape: StackedShape, folded: bool) -> usize {
    if folded {
        return 0;
    }
    match shape {
        StackedShape::Loading | StackedShape::Panel => 1,
        StackedShape::Rows(n) => n,
    }
}

/// What the list closure needs of a file: its descriptor, its `DiffView`
/// and this frame's shape.
type FileSlot = (Rc<StackedFile>, Entity<DiffView>, StackedShape);

/// The rows a file lends to the list this frame, kept across the items
/// of one frame.
struct RowsMemo {
    ctx: Rc<crate::diff_view_rows::RowContext>,
    rows: Rc<Vec<crate::diff_view_rows::Row>>,
    split_rows: Rc<Vec<crate::diff_view_rows::SplitRow>>,
    split_mode: bool,
}

impl Render for StackedDiffView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let background = t.background;
        let diff_text = t.diff_text;
        let (repo, kind) = {
            let s = self.state.read(cx);
            (s.selected, self.kind(cx))
        };
        let (Some(repo), Some(kind)) = (repo, kind) else {
            return div().size_full().bg(background).into_any_element();
        };
        let selected = {
            let s = self.state.read(cx);
            s.selected_state().and_then(|rs| match self.source {
                DiffSource::WorkingDirectory => rs.selected_file.clone(),
                DiffSource::Commit => rs.commit_selected_file.clone(),
                _ => None,
            })
        };
        if self.files.is_empty() {
            // a stack just shown starts at its top
            self.revealed = selected.clone();
        }
        let files = self.stack(cx).unwrap_or_default();
        self.set_files(files, cx);
        // the tab's selected file moved: scroll to it
        if selected != self.revealed {
            self.revealed = selected.clone();
            if let Some(path) = selected.filter(|p| self.files.iter().any(|s| s.file.path == *p)) {
                self.reveal_pending = Some(path);
            }
        }
        // prepare every file and lay the items out
        let kind = Rc::new(kind);
        let mut start = 0usize;
        let mut spans: Vec<(usize, usize)> = Vec::with_capacity(self.files.len());
        let folded = Rc::new(self.folded.clone());
        let mut overlays: Vec<AnyElement> = Vec::new();
        let mut options_open: Vec<usize> = Vec::new();
        for i in 0..self.files.len() {
            let view = self.files[i].view.clone();
            let shape = view.update(cx, |view, cx| view.stacked_prepare(kind.clone(), cx));
            if view.read(cx).options_open() {
                options_open.push(i);
            }
            let count = 1 + body_items(shape, folded.contains(&self.files[i].file.path));
            let slot = &mut self.files[i];
            slot.shape = shape;
            if count != slot.count {
                self.list_state.splice(start..start + slot.count, count);
                slot.count = count;
            }
            spans.push((start, count));
            start += count;
        }
        if self.list_state.item_count() != start {
            self.list_state.reset(start);
        }
        // one Diff Settings popover at a time: the last opened wins
        if let Some((_, earlier)) = options_open.split_last() {
            for &i in earlier {
                self.files[i]
                    .view
                    .update(cx, |view, cx| view.close_options(cx));
            }
        }
        for slot in &self.files {
            overlays.extend(
                slot.view
                    .update(cx, |view, cx| view.stacked_overlays(window, cx)),
            );
        }
        if let Some(path) = self.reveal_pending.take()
            && let Some(ix) = self.files.iter().position(|s| s.file.path == path)
        {
            self.list_state.scroll_to(ListOffset {
                item_ix: spans[ix].0,
                offset_in_item: Pixels::ZERO,
            });
        }
        let text_size = self
            .files
            .first()
            .map(|s| s.view.read(cx).text_size())
            .unwrap_or_else(FONT_SIZE_SM);
        let spans = Rc::new(spans);
        let slots: Rc<Vec<FileSlot>> = Rc::new(
            self.files
                .iter()
                .map(|s| (s.file.clone(), s.view.clone(), s.shape))
                .collect(),
        );
        let state = self.state.clone();
        let weak = cx.weak_entity();
        let memo: Rc<RefCell<HashMap<usize, Rc<RowsMemo>>>> = Rc::new(RefCell::new(HashMap::new()));

        // ask for a file's diff (and the next few) once its header renders
        let request = {
            let (slots, state, kind) = (slots.clone(), state.clone(), kind.clone());
            move |from: usize, cx: &mut App| {
                let s = state.read(cx);
                let Some(rs) = s.repo_states.get(&repo) else {
                    return;
                };
                let wanted: Vec<String> = slots
                    .iter()
                    .skip(from)
                    .take(1 + PREFETCH)
                    .filter(|(_, _, shape)| *shape == StackedShape::Loading)
                    .filter(|(file, _, _)| !rs.stacked.requested(&kind, &file.path))
                    .map(|(file, _, _)| file.path.clone())
                    .collect();
                for path in wanted {
                    let kind = (*kind).clone();
                    cx.defer(move |cx| Dispatcher::load_stacked_diff(repo, kind, path, false, cx));
                }
            }
        };

        let item =
            move |ix: usize, window: &mut Window, cx: &mut App| -> AnyElement {
                // the file this item belongs to
                let file_ix = match spans.binary_search_by(|(start, count)| {
                    if ix < *start {
                        std::cmp::Ordering::Greater
                    } else if ix >= start + count {
                        std::cmp::Ordering::Less
                    } else {
                        std::cmp::Ordering::Equal
                    }
                }) {
                    Ok(i) => i,
                    Err(_) => return div().into_any_element(),
                };
                let local = ix - spans[file_ix].0;
                let (file, view, shape) = &slots[file_ix];
                // the items update the files' views, so the theme is copied
                let t = cx.ghd().clone();
                let item_id = ElementId::NamedInteger(
                    "stacked-item".into(),
                    ((file_ix as u64) << 32) | local as u64,
                );
                if local == 0 {
                    request(file_ix, cx);
                    // the body is built here so its warnings go under the header
                    let (warnings, hint) = match view.update(cx, |v, cx| v.stacked_body(window, cx))
                    {
                        StackedBody::Rows {
                            ctx,
                            rows,
                            split_rows,
                            split_mode,
                            warnings,
                            hint,
                        } => {
                            memo.borrow_mut().insert(
                                file_ix,
                                Rc::new(RowsMemo {
                                    ctx,
                                    rows,
                                    split_rows,
                                    split_mode,
                                }),
                            );
                            (warnings, hint)
                        }
                        _ => (None, None),
                    };
                    let is_folded = folded.contains(&file.path);
                    let path = file.path.clone();
                    let weak = weak.clone();
                    return div()
                        .id(item_id)
                        .flex()
                        .flex_col()
                        .w_full()
                        .font_family(ui_font())
                        .text_size(FONT_SIZE())
                        .line_height(relative(1.5))
                        .text_color(t.text)
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_stretch()
                                .w_full()
                                .bg(t.box_alt_background)
                                .child(
                                    div()
                                        .id("stacked-fold")
                                        .a11y_button(if is_folded {
                                            "Unfold file"
                                        } else {
                                            "Fold file"
                                        })
                                        .ghd_tooltip(if is_folded {
                                            "Show this file's changes"
                                        } else {
                                            "Hide this file's changes"
                                        })
                                        .flex_none()
                                        .w(zpx(24.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .cursor_pointer()
                                        .border_b_1()
                                        .border_color(t.diff_border)
                                        .hover(|d| d.bg(t.box_hover_background))
                                        .child(octicon(
                                            if is_folded {
                                                Octicon::ChevronRight
                                            } else {
                                                Octicon::ChevronDown
                                            },
                                            t.text_secondary,
                                        ))
                                        .on_click(move |_, _, cx| {
                                            weak.update(cx, |this, cx| {
                                                this.toggle_fold(path.clone(), cx)
                                            })
                                            .ok();
                                        }),
                                )
                                .child(div().flex_1().min_w_0().flex().flex_col().child(
                                    diff_header(
                                        &file.path,
                                        file.kind,
                                        file.old_path.as_deref(),
                                        None,
                                        view,
                                        cx,
                                    ),
                                )),
                        )
                        .children(warnings)
                        .children(hint)
                        .into_any_element();
                }
                match shape {
                    StackedShape::Loading => {
                        request(file_ix, cx);
                        div()
                            .id(item_id)
                            .h(DIFF_LINE_HEIGHT() * LOADING_ROWS)
                            .w_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(t.text_secondary)
                            .child(crate::icons::loading("stacked-loading", t.text_secondary))
                            .into_any_element()
                    }
                    StackedShape::Panel => {
                        let is_image = state.read(cx).selected_state().is_some_and(|rs| {
                            rs.stacked
                                .entry(&kind, &file.path)
                                .is_some_and(|e| matches!(*e.diff, Diff::Image { .. }))
                        });
                        let panel = match view.update(cx, |v, cx| v.stacked_body(window, cx)) {
                            StackedBody::Panel(panel) => Some(panel),
                            _ => None,
                        };
                        div()
                            .id(item_id)
                            .h(zpx(if is_image { IMAGE_HEIGHT } else { PANEL_HEIGHT }))
                            .w_full()
                            .flex()
                            .flex_col()
                            .font_family(ui_font())
                            .text_color(t.text)
                            .border_b_1()
                            .border_color(t.diff_border)
                            .children(panel)
                            .into_any_element()
                    }
                    StackedShape::Rows(count) => {
                        let rows = {
                            let have = memo.borrow().get(&file_ix).cloned();
                            match have {
                                Some(m) => m,
                                None => match view.update(cx, |v, cx| v.stacked_body(window, cx)) {
                                    StackedBody::Rows {
                                        ctx,
                                        rows,
                                        split_rows,
                                        split_mode,
                                        ..
                                    } => {
                                        let m = Rc::new(RowsMemo {
                                            ctx,
                                            rows,
                                            split_rows,
                                            split_mode,
                                        });
                                        memo.borrow_mut().insert(file_ix, m.clone());
                                        m
                                    }
                                    _ => return div().id(item_id).into_any_element(),
                                },
                            }
                        };
                        let row_ix = local - 1;
                        let row = if rows.split_mode {
                            rows.split_rows
                                .get(row_ix)
                                .map(|row| render_split_row(&rows.ctx, row_ix, row, &rows.rows, cx))
                        } else {
                            rows.rows
                                .get(row_ix)
                                .map(|row| render_row(&rows.ctx, row_ix, row, cx))
                        };
                        // ids repeat across files (`("diff-row", abs)`): the
                        // wrapper's id keeps every row's element state apart
                        let wrapper = div().id(item_id).w_full();
                        match row {
                            // the grid ends in a 1 px `--diff-border-color` line
                            Some(row) if row_ix + 1 == *count => wrapper
                                .border_b_1()
                                .border_color(t.diff_border)
                                .child(row)
                                .into_any_element(),
                            Some(row) => wrapper.child(row).into_any_element(),
                            None => wrapper.into_any_element(),
                        }
                    }
                }
            };

        div()
            .id("stacked-diff")
            .track_focus(&self.focus_handle)
            .key_context("Diff")
            .on_action(cx.listener(|this, _: &Copy, _, cx| {
                for slot in &this.files {
                    if slot.view.update(cx, |v, cx| v.copy_text_selection(cx)) {
                        cx.stop_propagation();
                        break;
                    }
                }
            }))
            .relative()
            .size_full()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(background)
            .font_family(mono_font())
            .text_size(text_size)
            .line_height(DIFF_LINE_HEIGHT())
            .text_color(diff_text)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, ev: &MouseDownEvent, window, cx| {
                    if !this.focus_handle.contains_focused(window, cx) {
                        window.focus(&this.focus_handle, cx);
                    }
                    let position = ev.position;
                    this.each_view(cx, move |view, cx| {
                        view.clear_text_selection_unless_on_text(position, cx)
                    });
                }),
            )
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| {
                if ev.pressed_button == Some(MouseButton::Left) {
                    let position = ev.position;
                    this.each_view(cx, move |view, cx| view.drag_text_selection(position, cx));
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.each_view(cx, |view, cx| {
                        view.end_selection(cx);
                        view.end_text_selection(cx);
                    });
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.each_view(cx, |view, cx| {
                        view.end_selection(cx);
                        view.end_text_selection(cx);
                    });
                }),
            )
            .child(
                list(self.list_state.clone(), item)
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .pr(gutter(&self.list_state)),
            )
            .child(scrollbar("stacked-scrollbar", self.list_state.clone()))
            .children(overlays)
            .into_any_element()
    }
}

/// The stack button at the end of a "N changed files" header: shows every
/// file's diff in one list (`history`: the History tab's setting, else the
/// Changes tab's). `on` draws it pressed.
pub fn stack_toggle(history: bool, on: bool, cx: &App) -> Stateful<Div> {
    let t = cx.ghd();
    let tooltip = match (history, on) {
        (true, true) => {
            "Showing every file's changes in one list. Click to show one file at a time."
        }
        (true, false) => "Show every file's changes in one list.",
        (false, true) => {
            "Showing the changes to be committed in one list. Click to show one file at a time."
        }
        (false, false) => "Show the changes to be committed in one list.",
    };
    div()
        .id(if history {
            "stacked-diff-toggle-history"
        } else {
            "stacked-diff-toggle-changes"
        })
        .a11y_button(if on {
            "Show one file at a time"
        } else {
            "Show all files in one list"
        })
        .ghd_tooltip(tooltip)
        .flex_none()
        .size(zpx(22.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(BORDER_RADIUS())
        .cursor_pointer()
        .when(on, |d| d.bg(t.box_selected_active_background))
        .when(!on, |d| d.hover(|d| d.bg(t.box_hover_background)))
        .child(octicon(
            Octicon::Stack,
            if on {
                t.box_selected_active_text
            } else {
                t.text_secondary
            },
        ))
        .on_click(move |_, _, cx| Dispatcher::set_stacked_diff(history, !on, cx))
}

/// The flag is on.
pub fn enabled(cx: &App) -> bool {
    AppState::try_global(cx).is_some_and(|s| corvene_core::stacked_diff::enabled(s.read(cx)))
}
