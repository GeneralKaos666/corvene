//! Right pane of the History tab - GHD `ui/history/selected-commits.tsx`
//! with `expandable-commit-summary.tsx` and `file-list.tsx`
//! (`styles/ui/history/_expandable-commit-summary.scss`, `_commit-details.scss`):
//! title + expander, description, meta row (author, sha + copy, +adds −dels,
//! tags), then a resizable 250 px file list next to the commit's diff.
//!
//! Deviation (`.docs/deviations.md` › History, flag `810`): the file
//! list multi-selects with ⌘/⇧-click, and a multi-selection's context menu
//! copies all the paths; GHD's history file list selects one file. Open with
//! Default Program opens the file as of the commit (flag `811`), not the
//! working copy. A file gone from disk keeps its Copy path items (flag
//! `812`). A multi-commit selection's summary shows the range's +added
//! -deleted line totals (flag `813`). The meta row adds the author date and
//! links the SHA to the commit on GitHub (flag `805`); the tags' tooltip
//! lists every tag (flag `806`). The title and description (GHD `RichText`:
//! emoji, `#123`, `@name`, URLs) also show `code` spans and link SHAs
//! (flag `804`). A file's menu can revert that file's
//! changes from the commit (flag `814`).
//! A file's context menu adds "Open All Files of Commit in <editor>"
//! (`712-open-multiple-files`). The author's name can link to their GitHub
//! profile (`882-commit-author-links`). A merge commit's file list header
//! toggles "conflict resolutions only" (`773-merge-remerge-diff`: git's
//! `--remerge-diff`), with a "Merged cleanly" note when there are none; GHD
//! diffs merges against their first parent only. While whitespace is hidden,
//! files with whitespace changes only can be left out of the list, with a
//! note under it (`793-hide-whitespace-only-files`).

use corvene_core::{AppState, CommittedFileChange, Dispatcher, Popup, UnreachableCommitsTab};
use gpui_kit::component::resizable::{
    ResizablePanelEvent, ResizableState, h_resizable, resizable_panel, v_resizable,
};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::widgets::GhdTooltip;
use crate::widgets::IconButtonA11y;
use crate::widgets::ListRowA11y;

use crate::actions::{
    CopySelectedFilePaths, CopySelectedRelativeFilePaths, ExtendSelectionDown, ExtendSelectionUp,
    OpenSelectedFileInEditor, OpenSelectedFileWithDefaultProgram, SelectFirstFile, SelectLastFile,
    SelectNextFile, SelectPreviousFile,
};
use crate::diff_view::{DiffSource, DiffView, diff_header, status_icon};
use crate::icons::{Octicon, octicon};
use crate::scrollbar::ScrollbarExt;
use crate::theme::sizes::*;
use crate::theme::{ActiveGhdTheme, mono_font};
use crate::widgets::{avatar_stack, link_button};

/// `commitSummaryWidth` constraints (GHD `constrain(250, 100, 600)`).
#[allow(non_snake_case)]
fn FILE_LIST_MIN() -> Pixels {
    zpx(100.)
}
#[allow(non_snake_case)]
fn FILE_LIST_MAX() -> Pixels {
    zpx(600.)
}

pub struct SelectedCommitView {
    state: Entity<AppState>,
    diff: Entity<DiffView>,
    resizable: Entity<ResizableState>,
    file_list_width: Pixels,
    /// The file list takes focus on click so ⌘9 / ⌘8 resize it.
    file_list_focus: FocusHandle,
    /// `file_list_focus` held focus at the last render (active selection colours).
    file_list_focused: bool,
    /// Flag `810`: the ⌘/⇧-clicked files (file-list order) and the commit
    /// selection they belong to; stale once the commit selection changes.
    multi_files: Option<(Vec<String>, Vec<String>)>,
    /// The moving end of a ⇧↑ / ⇧↓ selection (flag `810`); the diffed file
    /// is its origin.
    multi_end: Option<String>,
    file_scroll: UniformListScrollHandle,
    /// Shift+F10 / Menu: the selected file's row.
    menu_anchor: crate::context_menu::RowMenuAnchor,
    /// Corvene (`801-history-review-mode`): the file list is hidden.
    file_list_hidden: bool,
    /// GHD `CopyButton` of the commit's SHA (its copied state).
    copy_sha: Option<crate::copy_button::CopyButton>,
}

/// How a click in the commit file list changes the selection.
#[derive(Clone, Copy, PartialEq, Eq)]
enum FileClick {
    Plain,
    Toggle,
    Range,
}

impl SelectedCommitView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        let diff = cx.new(|cx| DiffView::new(state.clone(), DiffSource::Commit, cx));
        let file_list_width = zpx(state.read(cx).settings.commit_summary_width);
        let resizable = cx.new(|_| ResizableState::default());
        cx.subscribe(&resizable, |this, state, _: &ResizablePanelEvent, cx| {
            if let Some(width) = state.read(cx).sizes().first().copied()
                && width != this.file_list_width
            {
                this.file_list_width = width;
                Dispatcher::update_settings(cx, |s| s.commit_summary_width = unzoom(width));
                cx.notify();
            }
        })
        .detach();
        let file_scroll = UniformListScrollHandle::new();
        Self {
            state,
            diff,
            resizable,
            file_list_width,
            file_list_focus: cx.focus_handle().tab_stop(true),
            file_list_focused: false,
            copy_sha: None,
            multi_files: None,
            multi_end: None,
            menu_anchor: crate::context_menu::RowMenuAnchor::for_uniform_list(&file_scroll),
            file_scroll,
            file_list_hidden: false,
        }
    }

    /// Corvene (`611-copy-path-shortcuts`): Copy File Path / Copy Relative
    /// File Path for the selected commit file.
    fn copy_selected_path(&self, absolute: bool, cx: &mut Context<Self>) {
        let text = {
            let s = self.state.read(cx);
            let (Some(rs), Some(repo)) = (s.selected_state(), s.selected_repository()) else {
                return;
            };
            let Some(path) = rs.commit_selected_file.as_ref() else {
                return;
            };
            if absolute {
                repo.path.join(path).to_string_lossy().into_owned()
            } else {
                path.clone()
            }
        };
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }

    /// Corvene (`801-history-review-mode`): hide or show the file list.
    pub fn set_file_list_hidden(&mut self, hidden: bool, cx: &mut Context<Self>) {
        if self.file_list_hidden != hidden {
            self.file_list_hidden = hidden;
            cx.notify();
        }
    }

    /// Corvene (`612-navigation-shortcuts`): focus the commit's diff.
    pub fn focus_diff(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.diff.update(cx, |diff, cx| diff.focus(window, cx));
    }

    /// Corvene (`619-arrow-keys-between-panes`): the panes left to right,
    /// the file list only while it shows.
    pub fn pane_focus_handles(&self, cx: &App) -> Vec<FocusHandle> {
        let diff = self.diff.read(cx).focus_handle();
        if self.file_list_hidden {
            vec![diff]
        } else {
            vec![self.file_list_focus.clone(), diff]
        }
    }

    /// Corvene (`606-open-file-shortcuts`): the selected commit file, when
    /// it exists in the working directory (the context menu's condition).
    fn selected_file_on_disk(&self, cx: &App) -> Option<std::path::PathBuf> {
        let s = self.state.read(cx);
        let rs = s.selected_state()?;
        let full = s
            .selected_repository()?
            .path
            .join(rs.commit_selected_file.as_ref()?);
        full.exists().then_some(full)
    }

    /// The multi-selected files (flag `810`), empty when fewer than two.
    fn multi_selected(&self, id: u64, cx: &App) -> Vec<String> {
        let s = self.state.read(cx);
        if !s
            .flags
            .bool(corvene_core::flags::ids::COMMIT_FILES_MULTI_SELECT)
        {
            return Vec::new();
        }
        match (&self.multi_files, s.repo_states.get(&id)) {
            (Some((commits, paths)), Some(rs))
                if *commits == rs.selected_commits
                    && paths.len() > 1
                    && rs
                        .commit_selected_file
                        .as_ref()
                        .is_some_and(|f| paths.contains(f)) =>
            {
                paths.clone()
            }
            _ => Vec::new(),
        }
    }

    /// The commit's files in list order.
    fn file_order(&self, id: u64, cx: &App) -> Vec<String> {
        self.state
            .read(cx)
            .repo_states
            .get(&id)
            .and_then(|rs| rs.changeset.as_ref())
            .map(|c| c.files.iter().map(|f| f.path.clone()).collect())
            .unwrap_or_default()
    }

    /// GHD `List.moveSelection` on the commit's `FileList` (↑ / ↓, and ⌥↓ /
    /// ⌥↑ from the diff): the file `delta` rows from the moving end of the
    /// selection, wrapping around the ends (GHD `List.moveSelection`), scrolled into view.
    pub fn select_relative(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some(id) = self.state.read(cx).selected else {
            return;
        };
        let order = self.file_order(id, cx);
        let current = self
            .multi_end
            .clone()
            .filter(|end| self.multi_selected(id, cx).contains(end))
            .or_else(|| {
                let s = self.state.read(cx);
                s.repo_states.get(&id)?.commit_selected_file.clone()
            })
            .and_then(|p| order.iter().position(|o| *o == p));
        // Corvene `620-lists-stop-at-ends`: no wrap around the ends
        let wrap = !self
            .state
            .read(cx)
            .flags
            .bool(corvene_core::flags::ids::LISTS_STOP_AT_ENDS);
        let next = corvene_core::list_selection::find_next_selectable_row(
            order.len(),
            current,
            delta,
            wrap,
            |_| true,
        );
        if let Some(ix) = next {
            self.select_index(id, &order, ix, cx);
        }
    }

    /// Home / End, ⌘↑ / ⌘↓: the first or last file.
    fn select_edge(&mut self, last: bool, cx: &mut Context<Self>) {
        let Some(id) = self.state.read(cx).selected else {
            return;
        };
        let order = self.file_order(id, cx);
        if !order.is_empty() {
            let ix = if last { order.len() - 1 } else { 0 };
            self.select_index(id, &order, ix, cx);
        }
    }

    fn select_index(&mut self, id: u64, order: &[String], ix: usize, cx: &mut Context<Self>) {
        self.multi_files = None;
        self.multi_end = None;
        Dispatcher::select_commit_file(id, order[ix].clone(), cx);
        self.file_scroll.scroll_to_item(ix, ScrollStrategy::Nearest);
        cx.notify();
    }

    /// ⇧↓ / ⇧↑ (GHD `List.addSelection`): with flag `810` the selection runs
    /// from the diffed file to a moving end one row further; without it the
    /// list is single-select and the keys move like ↓ / ↑.
    fn extend_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        let (id, anchor, commits, multi_select) = {
            let s = self.state.read(cx);
            let Some(id) = s.selected else { return };
            let Some(rs) = s.repo_states.get(&id) else {
                return;
            };
            (
                id,
                rs.commit_selected_file.clone(),
                rs.selected_commits.clone(),
                s.flags
                    .bool(corvene_core::flags::ids::COMMIT_FILES_MULTI_SELECT),
            )
        };
        let Some(anchor) = anchor.filter(|_| multi_select) else {
            return self.select_relative(delta, cx);
        };
        let order = self.file_order(id, cx);
        let multi = self.multi_selected(id, cx);
        let end = self
            .multi_end
            .clone()
            .filter(|end| multi.contains(end))
            .unwrap_or_else(|| anchor.clone());
        let Some(range) = corvene_core::list_selection::extend_selection(
            &order,
            &anchor,
            std::slice::from_ref(&end),
            delta,
        ) else {
            return;
        };
        let Some(new_end) = range.last().cloned() else {
            return;
        };
        if let Some(ix) = order.iter().position(|p| *p == new_end) {
            self.file_scroll.scroll_to_item(ix, ScrollStrategy::Nearest);
        }
        let next: Vec<String> = order.into_iter().filter(|p| range.contains(p)).collect();
        self.multi_files = (next.len() > 1).then_some((commits, next));
        self.multi_end = Some(new_end);
        cx.notify();
    }

    /// ⌘-click toggles `path`, ⇧-click selects from the diffed file to
    /// `path`, a plain click leaves a single selection.
    fn click_file(&mut self, id: u64, path: String, click: FileClick, cx: &mut Context<Self>) {
        let (order, anchor, commits) = {
            let s = self.state.read(cx);
            let Some(rs) = s.repo_states.get(&id) else {
                return;
            };
            let order: Vec<String> = rs
                .changeset
                .as_ref()
                .map(|c| c.files.iter().map(|f| f.path.clone()).collect())
                .unwrap_or_default();
            (
                order,
                rs.commit_selected_file.clone(),
                rs.selected_commits.clone(),
            )
        };
        let mut current = self.multi_selected(id, cx);
        if current.is_empty() {
            current.extend(anchor.clone());
        }
        let next = match (click, anchor) {
            (FileClick::Range, Some(anchor)) => {
                match (
                    order.iter().position(|p| *p == anchor),
                    order.iter().position(|p| *p == path),
                ) {
                    (Some(from), Some(to)) => {
                        corvene_core::list_selection::selection_between(&order, from, to)
                    }
                    _ => vec![path.clone()],
                }
            }
            (FileClick::Toggle, _) => {
                if current.contains(&path) {
                    if current.len() > 1 {
                        current.retain(|p| *p != path);
                    }
                } else {
                    current.push(path.clone());
                }
                current
            }
            _ => vec![path.clone()],
        };
        let mut next: Vec<String> = order.into_iter().filter(|p| next.contains(p)).collect();
        if next.is_empty() {
            next.push(path.clone());
        }
        // the diff shows the clicked file, or stays on one still selected
        let diffed =
            if next.contains(&path) {
                Some(path)
            } else {
                let current =
                    self.state.read(cx).repo_states.get(&id).and_then(|rs| {
                        rs.commit_selected_file.clone().filter(|f| next.contains(f))
                    });
                if current.is_some() {
                    None
                } else {
                    next.first().cloned()
                }
            };
        self.multi_files = (next.len() > 1).then_some((commits, next));
        self.multi_end = None;
        if let Some(path) = diffed {
            Dispatcher::select_commit_file(id, path, cx);
        }
        cx.notify();
    }

    /// `ExpandableCommitSummary` for a contiguous multi-commit selection:
    /// "Showing changes from N commits" (+ how many are unreachable).
    fn multi_summary(&self, id: u64, cx: &Context<Self>) -> Option<AnyElement> {
        let t = cx.ghd();
        let s = self.state.read(cx);
        let rs = s.repo_states.get(&id)?;
        let selected = rs.selected_commits.len();
        if selected <= 1 {
            return None;
        }
        let shas_in_diff: Vec<String> = rs.shas_in_diff.clone();
        let shas_not_in_diff: Vec<String> = rs
            .selected_commits
            .iter()
            .filter(|sha| !rs.shas_in_diff.contains(sha))
            .cloned()
            .collect();
        let not_in_diff = shas_not_in_diff.len();
        let in_diff = selected - not_in_diff;
        // `813`: the range's line totals follow the count
        let totals = s
            .flags
            .bool(corvene_core::flags::ids::MULTI_COMMIT_LINE_TOTALS)
            .then(|| {
                rs.changeset
                    .as_ref()
                    .map(|c| (c.lines_added, c.lines_deleted))
            })
            .flatten()
            .filter(|(a, d)| *a > 0 || *d > 0);
        // `onHighlightShas`: hovering either count dims the other rows.
        let highlight = |shas: Vec<String>| {
            move |hovered: &bool, _: &mut Window, cx: &mut App| {
                Dispatcher::set_highlighted_shas(
                    id,
                    if *hovered { shas.clone() } else { Vec::new() },
                    cx,
                )
            }
        };
        Some(
            div()
                .id("expandable-commit-summary")
                .flex_none()
                .flex()
                .flex_col()
                .border_b_1()
                .border_color(t.box_border)
                .child(
                    div()
                        .id("commits-in-diff")
                        .pt(SPACING())
                        .px(SPACING())
                        .pb(SPACING_HALF())
                        .text_size(FONT_SIZE_MD())
                        .font_weight(FontWeight::SEMIBOLD)
                        .line_height(zpx(16.))
                        .flex()
                        .flex_row()
                        .on_hover(highlight(shas_in_diff))
                        .child(format!(
                            "Showing changes from {in_diff} {}",
                            if in_diff == 1 { "commit" } else { "commits" }
                        ))
                        .when_some(totals, |d, (added, deleted)| {
                            d.child(
                                div()
                                    .ml_auto()
                                    .pl(SPACING())
                                    .flex_none()
                                    .flex()
                                    .flex_row()
                                    .gap(SPACING_HALF())
                                    .font_weight(FontWeight::NORMAL)
                                    .text_size(FONT_SIZE_SM())
                                    .child(
                                        div().text_color(t.color_new).child(format!(
                                            "+{}",
                                            crate::format::format_count(added)
                                        )),
                                    )
                                    .child(div().text_color(t.color_deleted).child(format!(
                                        "-{}",
                                        crate::format::format_count(deleted)
                                    ))),
                            )
                        }),
                )
                .when(not_in_diff > 0, |d| {
                    // `renderCommitsNotReachable` (`.commit-unreachable-info`)
                    d.child(
                        div()
                            .px(SPACING())
                            .pb(SPACING_HALF())
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(SPACING_HALF())
                            .text_size(FONT_SIZE_SM())
                            .text_color(t.text_secondary)
                            .child(octicon(Octicon::Info, t.text_secondary))
                            .child(
                                link_button(
                                    "commits-not-in-diff",
                                    format!(
                                        "{not_in_diff} unreachable {}",
                                        if not_in_diff == 1 {
                                            "commit"
                                        } else {
                                            "commits"
                                        }
                                    ),
                                    cx,
                                )
                                .text_size(FONT_SIZE_SM())
                                .on_hover(highlight(shas_not_in_diff))
                                .on_click(move |_, _, cx| {
                                    Dispatcher::set_highlighted_shas(id, Vec::new(), cx);
                                    Dispatcher::show_popup(
                                        Popup::UnreachableCommits {
                                            repo: id,
                                            tab: UnreachableCommitsTab::Unreachable,
                                        },
                                        cx,
                                    )
                                }),
                            )
                            .child("not included."),
                    )
                })
                .into_any_element(),
        )
    }

    /// `ExpandableCommitSummary`
    fn summary(&self, id: u64, cx: &Context<Self>) -> Option<AnyElement> {
        if let Some(multi) = self.multi_summary(id, cx) {
            return Some(multi);
        }
        let t = cx.ghd();
        let s = self.state.read(cx);
        let rs = s.repo_states.get(&id)?;
        let sha = rs.selected_commit.as_ref()?;
        let commit = rs.find_commit(sha)?.clone();
        let expanded = rs.commit_summary_expanded;
        let (added, deleted) = rs
            .changeset
            .as_ref()
            .map(|c| (c.lines_added, c.lines_deleted))
            .unwrap_or((0, 0));
        // `805`: the author date, and the SHA links to the commit on GitHub
        let extras = s
            .flags
            .bool(corvene_core::flags::ids::COMMIT_DETAILS_EXTRAS);
        // `1214-commit-signatures`: the pill after the SHA
        let signature_pill = crate::signature_badge::enabled(cx)
            .then(|| {
                crate::signature_badge::header_pill(&commit, rs.signatures.get(&commit.sha), cx)
            })
            .flatten();
        // `883-unpublished-commit-links`: not for a commit no remote has
        let unpublished = rs
            .unpublished_commits
            .as_ref()
            .is_some_and(|shas| shas.contains(&commit.sha));
        let commit_url = extras
            .then(|| s.repository(id).and_then(|r| r.github.as_ref()))
            .flatten()
            .filter(|_| !unpublished)
            .map(|g| format!("{}/commit/{}", g.html_url, commit.sha));
        // GHD `RichText`: emoji, `#123`, `@name` and links; `804` adds `code`
        // spans and (GitHub repositories) SHAs
        let token_repository = s
            .repository(id)
            .and_then(corvene_core::text_tokens::TokenRepository::of);
        let rich_extras = s
            .flags
            .bool(corvene_core::flags::ids::COMMIT_MESSAGE_RICH_TEXT);
        let commit_base = rich_extras
            .then(|| s.repository(id).and_then(|r| r.github.as_ref()))
            .flatten()
            .map(|g| g.html_url.clone());
        let token_options = corvene_core::text_tokens::TokenOptions {
            trailing_punctuation: s
                .flags
                .bool(corvene_core::flags::ids::LINKIFY_TRAILING_PUNCTUATION),
            cross_repository: s
                .flags
                .bool(corvene_core::flags::ids::CROSS_REPOSITORY_ISSUE_LINKS),
            // `341-custom-autolinks`
            links: s.link_rules(id),
        };
        // `882-commit-author-links`: the author's GitHub profile, when known
        let author_url = s
            .flags
            .bool(corvene_core::flags::ids::COMMIT_AUTHOR_LINKS)
            .then(|| s.repository(id).and_then(|r| r.github.as_ref()))
            .flatten()
            .and_then(|gh| {
                let login = corvene_core::autocomplete::login_for_email(
                    &commit.author.email,
                    gh,
                    &s.accounts,
                    &s.mentionables,
                )?;
                Some(corvene_github::Endpoint::from_api_base(&gh.endpoint).web(&login))
            });
        // `881-issue-title-tooltips`
        let issue_titles = s
            .flags
            .bool(corvene_core::flags::ids::ISSUE_TITLE_TOOLTIPS)
            .then(|| s.repository(id).and_then(|r| r.non_fork_github()).cloned())
            .flatten();
        // GHD `wrapRichTextCommitMessage`: a summary past 72 characters
        // continues at the start of the description
        let (title_text, description_text) = corvene_core::markdown::commit_summary_rich_text(
            &commit.summary,
            &commit.body,
            token_repository.as_ref(),
            token_options,
            rich_extras,
            commit_base.as_deref(),
        );
        let empty = commit.summary.is_empty();
        let title = if empty {
            "Empty commit message".to_string()
        } else {
            commit.summary.clone()
        };
        let meta_item = |d: Div| {
            d.flex()
                .flex_row()
                .items_center()
                .mr(SPACING())
                .text_size(FONT_SIZE_SM())
        };
        Some(
            div()
                .id("expandable-commit-summary")
                .flex_none()
                .flex()
                .flex_col()
                .min_h_0()
                .border_b_1()
                .border_color(t.box_border)
                .child(
                    // `.ecs-title`
                    div()
                        .flex()
                        .flex_row()
                        .items_start()
                        .pt(SPACING())
                        .px(SPACING())
                        .pb(SPACING_HALF())
                        .text_size(FONT_SIZE_MD())
                        .font_weight(FontWeight::SEMIBOLD)
                        .line_height(zpx(16.))
                        .when(empty, |d| d.text_color(t.text_secondary))
                        // the expander follows the title (`margin-left: 10px`)
                        .child(div().min_w_0().child(if empty {
                            title.into_any_element()
                        } else {
                            crate::markdown::rich_text_with_issue_titles(
                                "commit-title",
                                &title_text,
                                issue_titles.clone(),
                                cx,
                            )
                        }))
                        .child(
                            div()
                                .id("commit-summary-expander")
                                .a11y_button(if expanded {
                                    "Collapse commit details"
                                } else {
                                    "Expand commit details"
                                })
                                .ghd_tooltip(if expanded { "Collapse" } else { "Expand" })
                                .ml(SPACING())
                                .flex_none()
                                // a `<button>` at `line-height: normal`: its
                                // height sets `.ecs-title`'s (SF's is the
                                // icon's 16 px on macOS)
                                .when(!cfg!(target_os = "macos"), |d| {
                                    d.h(crate::theme::normal_line_height(FONT_SIZE_MD(), cx))
                                })
                                .cursor_pointer()
                                .on_click(move |_, _, cx| {
                                    Dispatcher::set_commit_summary_expanded(id, !expanded, cx)
                                })
                                .child(octicon(
                                    if expanded {
                                        Octicon::Fold
                                    } else {
                                        Octicon::Unfold
                                    },
                                    t.text,
                                )),
                        ),
                )
                .child(
                    // `.beneath-summary`
                    div()
                        .id("ecs-scroll")
                        .overflow_y_scroll()
                        .px(SPACING())
                        .pb(SPACING_HALF())
                        .flex()
                        .flex_col()
                        .when(expanded, |d| d.max_h(zpx(400.)))
                        .when(!description_text.is_empty(), |d| {
                            // `.ecs-description-text`: a 5 px padded box in
                            // `--box-alt-background-color`, 5 px above the meta row
                            d.child(div().pb(SPACING_HALF()).child({
                                let text = div()
                                    .p(SPACING_HALF())
                                    .bg(t.box_alt_background)
                                    .font_family(mono_font())
                                    .text_size(FONT_SIZE_SM())
                                    .line_height(zpx(16.5))
                                    .child(crate::markdown::rich_text_with_issue_titles(
                                        "commit-description",
                                        &description_text,
                                        issue_titles.clone(),
                                        cx,
                                    ));
                                if expanded {
                                    // `.beneath-summary` scrolls the whole body
                                    text.into_any_element()
                                } else {
                                    // `.ecs-description-scroll-view`: 30–80 px,
                                    // `overflow-y: auto` while collapsed
                                    div()
                                        .id("ecs-description-scroll-view")
                                        .min_h(zpx(30.))
                                        .max_h(zpx(80.))
                                        .overflow_y_scroll()
                                        .child(text)
                                        .with_scrollbar()
                                        .into_any_element()
                                }
                            }))
                        })
                        .child(
                            // `.ecs-meta`
                            div()
                                .flex()
                                .flex_row()
                                .flex_wrap()
                                .items_center()
                                .line_height(zpx(16.5))
                                .child(
                                    // `renderAuthorStack`: `AvatarStack`, then
                                    // `CommitAttribution`; `882-commit-author-links`
                                    // links the author's name to their profile
                                    meta_item(div()).child(avatar_stack(
                                        "avatar-stack",
                                        &crate::history::avatar_users_for(&commit, cx),
                                        {
                                            let attribution =
                                                crate::history::commit_attribution_for(&commit, cx);
                                            let rest = attribution
                                                .text
                                                .strip_prefix(commit.author.name.as_str())
                                                .filter(|_| !attribution.authors.is_empty())
                                                .map(str::to_string);
                                            match (author_url, rest) {
                                                (Some(url), Some(rest)) => div()
                                                    .flex()
                                                    .flex_row()
                                                    .child(
                                                        link_button(
                                                            "commit-author-link",
                                                            commit.author.name.clone(),
                                                            cx,
                                                        )
                                                        .text_size(FONT_SIZE_SM())
                                                        .ghd_tooltip(url.clone())
                                                        .on_click(move |_, _, cx| {
                                                            Dispatcher::open_url(&url, cx)
                                                        }),
                                                    )
                                                    .child(rest)
                                                    .into_any_element(),
                                                _ => attribution.text.into_any_element(),
                                            }
                                        },
                                        cx,
                                    )),
                                )
                                .when(extras, |d| {
                                    let date = commit.author.date();
                                    d.child(
                                        meta_item(div())
                                            .id("commit-date")
                                            .ghd_tooltip(crate::relative_time::relative(date))
                                            .child(octicon(Octicon::History, t.text))
                                            .child(
                                                div()
                                                    .pl(SPACING_HALF())
                                                    .child(crate::format::format_date_time(date)),
                                            ),
                                    )
                                })
                                .child(
                                    meta_item(div())
                                        .child(octicon(Octicon::GitCommit, t.text))
                                        .child({
                                            let label = if expanded {
                                                commit.sha.clone()
                                            } else {
                                                commit.short_sha().to_string()
                                            };
                                            match commit_url {
                                                Some(url) => div().pl(SPACING_HALF()).child(
                                                    link_button("commit-sha-link", label, cx)
                                                        .text_size(FONT_SIZE_SM())
                                                        .ghd_tooltip("View on GitHub")
                                                        .on_click(move |_, _, cx| {
                                                            Dispatcher::open_url(&url, cx)
                                                        }),
                                                ),
                                                None => div().pl(SPACING_HALF()).child(label),
                                            }
                                        })
                                        .child({
                                            // GHD `CopyButton`, its copied state
                                            // kept while the commit stays selected
                                            let button = self
                                                .copy_sha
                                                .clone()
                                                .filter(|b| b.copy_content() == commit.sha)
                                                .unwrap_or_else(|| {
                                                    crate::copy_button::CopyButton::new(
                                                        commit.sha.clone(),
                                                        "Copy the full SHA",
                                                    )
                                                });
                                            let entity = cx.entity().downgrade();
                                            let clicked = button.clone();
                                            // `.copy-button`: 16 px wide with a 12 px
                                            // icon, a `<button>` at `line-height:
                                            // normal` (14 px with SF; with a wider
                                            // UI font, 17 px, which sets the row's)
                                            crate::copy_button::copy_button_element(
                                                "copy-sha",
                                                &button,
                                                zpx(12.),
                                                t.text,
                                                move |_, _, cx| {
                                                    let mut button = clicked.clone();
                                                    let text =
                                                        button.click(std::time::Instant::now());
                                                    cx.write_to_clipboard(
                                                        ClipboardItem::new_string(text),
                                                    );
                                                    entity
                                                        .update(cx, |this, cx| {
                                                            this.copy_sha = Some(button);
                                                            cx.notify();
                                                            cx.spawn(async |this, cx| {
                                                                cx.background_executor()
                                                                    .timer(
                                                                        crate::copy_button::COPIED_DURATION,
                                                                    )
                                                                    .await;
                                                                this.update(cx, |_, cx| {
                                                                    cx.notify()
                                                                })
                                                                .ok();
                                                            })
                                                            .detach();
                                                        })
                                                        .ok();
                                                },
                                            )
                                                .ml(SPACING_HALF())
                                                .w(zpx(16.))
                                                .h(if cfg!(target_os = "macos") {
                                                    zpx(14.)
                                                } else {
                                                    crate::theme::normal_line_height(
                                                        FONT_SIZE(),
                                                        cx,
                                                    )
                                                })
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                        }),
                                )
                                .when_some(signature_pill, |d, pill| d.child(pill))
                                .when(added > 0 || deleted > 0, |d| {
                                    // `.lines-added-deleted { margin-left: auto }`
                                    d.child(
                                        meta_item(div())
                                            .ml_auto()
                                            .when(expanded, |d| {
                                                d.child(
                                                    octicon(Octicon::FileDiff, t.text_secondary)
                                                        .mr(SPACING_HALF()),
                                                )
                                            })
                                            .child(
                                                div()
                                                    .pr(SPACING_HALF())
                                                    .text_color(t.color_new)
                                                    .child(if expanded {
                                                        format!(
                                                            "{} added lines",
                                                            crate::format::format_count(added)
                                                        )
                                                    } else {
                                                        format!(
                                                            "+{}",
                                                            crate::format::format_count(added)
                                                        )
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .pr(SPACING_HALF())
                                                    .text_color(t.color_deleted)
                                                    .child(if expanded {
                                                        format!("{deleted} removed lines")
                                                    } else {
                                                        format!("-{deleted}")
                                                    }),
                                            ),
                                    )
                                })
                                .when(!commit.tags.is_empty(), |d| {
                                    let tags = meta_item(div())
                                        .id("commit-tags")
                                        .min_w_0()
                                        .child(octicon(Octicon::Tag, t.text).mr(SPACING_HALF()))
                                        .child(div().truncate().child(commit.tags.join(", ")));
                                    // `806`: hovering lists every tag
                                    d.child(if crate::history::tags_tooltip(cx) {
                                        tags.ghd_tooltip(commit.tags.join("\n"))
                                    } else {
                                        tags
                                    })
                                }),
                        )
                        .with_scrollbar(),
                )
                .into_any_element(),
        )
    }

    /// `.file-list-header`; `remerge` (`773-merge-remerge-diff`) adds the
    /// "conflict resolutions only" toggle at its end, on or off.
    fn file_list_header(
        &self,
        id: u64,
        label: String,
        remerge: Option<bool>,
        cx: &Context<Self>,
    ) -> Div {
        let t = cx.ghd();
        div()
            .relative()
            .h(zpx(30.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .px(SPACING())
            .bg(t.box_alt_background)
            .border_b_1()
            .border_color(t.box_border)
            .text_size(FONT_SIZE())
            .child(label)
            .when_some(remerge, |d, on| {
                let tooltip = if on {
                    "Showing the conflict resolutions only: what differs from git's \
                     automatic merge. Click to show all changes."
                } else {
                    "Show the conflict resolutions only: what differs from git's \
                     automatic merge of the parents."
                };
                d.child(
                    div()
                        .id("remerge-diff-toggle")
                        .a11y_button(if on {
                            "Show all changes"
                        } else {
                            "Show conflict resolutions only"
                        })
                        .ghd_tooltip(tooltip)
                        .absolute()
                        .right(SPACING_HALF())
                        .top(zpx(4.))
                        .size(zpx(22.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(BORDER_RADIUS())
                        .cursor_pointer()
                        .when(on, |d| d.bg(t.box_selected_active_background))
                        .child(octicon(
                            Octicon::GitMerge,
                            if on {
                                t.box_selected_active_text
                            } else {
                                t.text_secondary
                            },
                        ))
                        .on_click(move |_, _, cx| Dispatcher::set_remerge_diff(id, !on, cx)),
                )
            })
    }

    /// `FileList` + `.file-list-header`
    fn file_list(&self, id: u64, cx: &Context<Self>) -> AnyElement {
        let t = cx.ghd();
        let s = self.state.read(cx);
        let rs = s.repo_states.get(&id);
        let files: Vec<CommittedFileChange> = rs
            .and_then(|r| r.changeset.as_ref())
            .map(|c| c.files.clone())
            .unwrap_or_default();
        let selected = rs.and_then(|r| r.commit_selected_file.clone());
        let multi = std::rc::Rc::new(self.multi_selected(id, cx));
        let weak = cx.weak_entity();
        // `773-merge-remerge-diff`: (toggle shown, showing resolutions only)
        let remerge = rs
            .filter(|rs| Dispatcher::remerge_available(s, rs))
            .map(|rs| rs.remerge_diff);
        // `793-hide-whitespace-only-files`
        let whitespace_hidden = rs.map_or(0, |r| r.changeset_whitespace_hidden);
        if rs.and_then(|r| r.changeset.as_ref()).is_some() && files.is_empty() {
            let empty = div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .p(SPACING())
                .text_center()
                .text_color(t.text_secondary)
                .child(if remerge == Some(true) {
                    "Merged cleanly: nothing differs from git's automatic merge".to_string()
                } else if whitespace_hidden > 0 {
                    format!(
                        "No files in commit apart from {} with whitespace changes only",
                        crate::format::format_count(whitespace_hidden as u64)
                    )
                } else {
                    "No files in commit".to_string()
                });
            return match remerge {
                Some(on) => div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .child(self.file_list_header(id, String::new(), Some(on), cx))
                    .child(empty)
                    .into_any_element(),
                None => empty.size_full().into_any_element(),
            };
        }
        let count = files.len();
        let files = std::rc::Rc::new(files);
        let focus = self.file_list_focus.clone();
        let focused = self.file_list_focused;
        let scroll = self.file_scroll.clone();
        let menu_anchor = self.menu_anchor.clone();
        div()
            .size_full()
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(t.box_border)
            .child(self.file_list_header(
                id,
                if count == 1 {
                    "1 changed file".to_string()
                } else {
                    format!(
                        "{} changed files",
                        crate::format::format_count(count as u64)
                    )
                },
                remerge,
                cx,
            ))
            .child(
                // a `List` node owning the file rows
                div()
                    .id("commit-files")
                    .role(Role::List)
                    .aria_label("Changed files")
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .child(
                        uniform_list("commit-file-rows", count, move |range, _, cx| {
                            range
                                .map(|ix| {
                                    let file = &files[ix];
                                    let is_selected = if multi.is_empty() {
                                        selected.as_deref() == Some(file.path.as_str())
                                    } else {
                                        multi.contains(&file.path)
                                    };
                                    commit_file_row(
                                        id,
                                        file,
                                        is_selected,
                                        &focus,
                                        focused,
                                        &multi,
                                        &weak,
                                        (selected.as_deref() == Some(file.path.as_str()))
                                            .then_some(&menu_anchor),
                                        cx,
                                    )
                                })
                                .collect()
                        })
                        .flex_1()
                        .min_h_0()
                        .with_scrollbar_handle(&scroll),
                    ),
            )
            .when(whitespace_hidden > 0, |d| {
                d.child(crate::changes::whitespace_hidden_note(
                    whitespace_hidden,
                    cx,
                ))
            })
            .into_any_element()
    }
}

/// History `FileList` row: dimmed directory + name, status icon (no checkbox).
/// GHD `SelectedCommits.onContextMenu`: open / reveal, copy paths, View on
/// GitHub; a file gone from disk gets a single disabled item.
fn open_commit_file_menu(
    id: u64,
    path: &str,
    multi: &[String],
    position: Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    use crate::context_menu::{IS_MAC, MenuItem, labels, mac_or};
    let state = AppState::global(cx).read(cx);
    let Some(repo) = state.repository(id) else {
        return;
    };
    // flag `810`: a multi-selection copies all its paths and opens the
    // files still on disk (`712-open-multiple-files`' bulk items)
    if multi.len() > 1 && multi.iter().any(|p| p == path) {
        let full = multi
            .iter()
            .map(|p| repo.path.join(p).to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let relative = multi.join("\n");
        let on_disk: Vec<std::path::PathBuf> = {
            let files = multi.iter().map(|p| repo.path.join(p));
            // past the cap the items are disabled anyway: skip the disk checks
            if multi.len() > crate::changes::MAX_BULK_OPEN {
                files.collect()
            } else {
                files.filter(|f| f.exists()).collect()
            }
        };
        let editor_label = state.editor_label();
        let open = match on_disk.as_slice() {
            [one] => {
                let default = one.clone();
                vec![
                    crate::changes::open_all_in_editor_item(
                        labels::open_in(&editor_label),
                        on_disk.clone(),
                    ),
                    MenuItem::new(labels::OPEN_WITH_DEFAULT_PROGRAM, move |_, cx| {
                        cx.open_with_system(&default)
                    }),
                ]
            }
            files => crate::changes::open_many_items(files, &editor_label),
        };
        let mut items = vec![
            MenuItem::new(
                mac_or("Copy File Paths", "Copy file paths"),
                move |_, cx| cx.write_to_clipboard(ClipboardItem::new_string(full.clone())),
            ),
            MenuItem::new(
                mac_or("Copy Relative File Paths", "Copy relative file paths"),
                move |_, cx| cx.write_to_clipboard(ClipboardItem::new_string(relative.clone())),
            ),
        ];
        if !on_disk.is_empty() {
            items.push(MenuItem::separator());
            items.extend(open);
        }
        crate::native_menu::show_context_menu(items, position, window, cx);
        return;
    }
    let full = repo.path.join(path);
    let editor_label = state.editor_label();
    let rs = state.repo_states.get(&id);
    let selected: Vec<String> = rs.map(|r| r.selected_commits.clone()).unwrap_or_default();
    // `localCommitSHAs`: here the newest unpushed commit is known
    let local = rs
        .and_then(|r| r.last_commit.as_ref())
        .is_some_and(|c| selected.first() == Some(&c.sha));
    let github = repo.github.clone();
    // `811`: open the file as of the (newest) selected commit
    let historical = state
        .flags
        .bool(corvene_core::flags::ids::OPEN_HISTORICAL_FILE)
        .then(|| {
            rs.and_then(|rs| {
                rs.commits
                    .iter()
                    .find(|c| selected.contains(&c.sha))
                    .map(|c| c.sha.clone())
            })
            .or_else(|| selected.first().cloned())
        })
        .flatten();
    // `814`: Revert Changes to This File, for a single selected commit
    let revert_file = state
        .flags
        .bool(corvene_core::flags::ids::REVERT_FILE_IN_COMMIT)
        .then(|| {
            let sha = selected.first().filter(|_| selected.len() == 1)?.clone();
            let old_path = rs
                .and_then(|r| r.changeset.as_ref())
                .and_then(|c| c.files.iter().find(|f| f.path == path))
                .and_then(|f| f.old_path.clone());
            let path = path.to_string();
            Some(MenuItem::new(
                mac_or("Revert Changes to This File", "Revert changes to this file"),
                move |_, cx| {
                    Dispatcher::revert_file_in_commit(
                        id,
                        sha.clone(),
                        path.clone(),
                        old_path.clone(),
                        cx,
                    )
                },
            ))
        })
        .flatten();
    // `712-open-multiple-files`: every file of the commit still on disk
    let open_all = state
        .flags
        .bool(corvene_core::flags::ids::OPEN_MULTIPLE_FILES)
        .then(|| {
            let files: Vec<std::path::PathBuf> = rs
                .and_then(|r| r.changeset.as_ref())
                .map(|c| c.files.iter().map(|f| repo.path.join(&f.path)).collect())
                .unwrap_or_default();
            // past the cap the item is disabled anyway: skip the disk checks
            if files.len() > crate::changes::MAX_BULK_OPEN {
                files
            } else {
                files.into_iter().filter(|f| f.exists()).collect()
            }
        })
        .filter(|files| files.len() > 1);
    let mut items = if !full.exists() {
        let mut items = vec![
            MenuItem::new(
                mac_or("File Does Not Exist on Disk", "File does not exist on disk"),
                |_, _| {},
            )
            .enabled(false),
        ];
        // `812`: the paths can still be copied
        if state
            .flags
            .bool(corvene_core::flags::ids::COPY_PATH_OF_MISSING_FILE)
        {
            let (full, relative) = (full.to_string_lossy().to_string(), path.to_string());
            items.extend([
                MenuItem::separator(),
                MenuItem::new(labels::COPY_FILE_PATH, move |_, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(full.clone()))
                }),
                MenuItem::new(labels::COPY_RELATIVE_FILE_PATH, move |_, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(relative.clone()))
                }),
            ]);
        }
        items
    } else {
        let (reveal, editor, default, copy_full) =
            (full.clone(), full.clone(), full.clone(), full.clone());
        let relative = path.to_string();
        let view_label = match &github {
            Some(gh) if gh.endpoint != "https://api.github.com" => "View on GitHub Enterprise",
            _ => "View on GitHub",
        };
        let view_url = github.as_ref().and_then(|gh| {
            selected
                .first()
                .map(|sha| format!("{}/blob/{sha}/{path}", gh.html_url))
        });
        vec![
            MenuItem::new(labels::REVEAL_IN_FILE_MANAGER, move |_, cx| {
                Dispatcher::show_in_finder(&reveal, cx)
            }),
            MenuItem::new(labels::open_in(&editor_label), move |_, cx| {
                Dispatcher::open_in_editor(editor.clone(), cx)
            }),
            // `isSafeFileExtension` is always true on macOS
            MenuItem::new(labels::OPEN_WITH_DEFAULT_PROGRAM, {
                let path = relative.clone();
                move |_, cx| match &historical {
                    Some(sha) => Dispatcher::open_commit_file_with_default_program(
                        id,
                        sha.clone(),
                        path.clone(),
                        cx,
                    ),
                    None => cx.open_with_system(&default),
                }
            }),
            MenuItem::separator(),
            MenuItem::new(labels::COPY_FILE_PATH, move |_, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(
                    copy_full.to_string_lossy().to_string(),
                ))
            }),
            MenuItem::new(labels::COPY_RELATIVE_FILE_PATH, move |_, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(relative.clone()))
            }),
            MenuItem::separator(),
            MenuItem::new(view_label, move |_, cx| {
                if let Some(url) = &view_url {
                    Dispatcher::open_url(url, cx)
                }
            })
            .enabled(selected.len() == 1 && !local && github.is_some()),
        ]
    };
    if let Some(item) = revert_file {
        items.extend([MenuItem::separator(), item]);
    }
    // `887-file-history`: History narrowed to this file
    if state.flags.bool(corvene_core::flags::ids::FILE_HISTORY) {
        let path = path.to_string();
        items.extend([
            MenuItem::separator(),
            MenuItem::new(mac_or("Show History", "Show history"), move |_, cx| {
                Dispatcher::show_file_history(id, path.clone(), cx)
            }),
        ]);
    }
    // `798-blame`: the file as this commit left it
    if state.flags.bool(corvene_core::flags::ids::BLAME) {
        if !state.flags.bool(corvene_core::flags::ids::FILE_HISTORY) {
            items.push(MenuItem::separator());
        }
        let path = path.to_string();
        items.push(MenuItem::new("Blame", move |_, cx| {
            Dispatcher::show_blame_for_commit_file(id, path.clone(), cx)
        }));
    }
    // `1113-lfs-locks`: the file as it is named now
    let lock_items = crate::changes::lfs_lock_items(state, id, &[path.to_string()]);
    if !lock_items.is_empty() {
        items.push(MenuItem::separator());
        items.extend(lock_items);
    }
    if let Some(files) = open_all {
        items.push(MenuItem::separator());
        items.push(crate::changes::open_all_in_editor_item(
            if IS_MAC {
                format!("Open All Files of Commit in {editor_label}")
            } else {
                format!("Open all files of commit in {editor_label}")
            },
            files,
        ));
    }
    crate::native_menu::show_context_menu(items, position, window, cx);
}

#[allow(clippy::too_many_arguments)]
fn commit_file_row(
    id: u64,
    file: &CommittedFileChange,
    is_selected: bool,
    focus: &FocusHandle,
    list_focused: bool,
    multi: &std::rc::Rc<Vec<String>>,
    view: &WeakEntity<SelectedCommitView>,
    menu_anchor: Option<&crate::context_menu::RowMenuAnchor>,
    cx: &App,
) -> AnyElement {
    let t = cx.ghd();
    let hover_bg = t.list_item_hover_background;
    let (icon, color) = status_icon(file.status.kind, t);
    // `.focus-within .list-item.selected` has no status fill: the icon
    // takes the row's text colour
    let color = if is_selected && list_focused {
        t.box_selected_active_text
    } else {
        color
    };
    let path = file.path.clone();
    let menu_path = file.path.clone();
    let multi_select = AppState::global(cx)
        .read(cx)
        .flags
        .bool(corvene_core::flags::ids::COMMIT_FILES_MULTI_SELECT);
    let click_kind = move |m: &Modifiers| {
        if !multi_select {
            FileClick::Plain
        } else if m.secondary() {
            FileClick::Toggle
        } else if m.shift {
            FileClick::Range
        } else {
            FileClick::Plain
        }
    };
    div()
        .id(SharedString::from(format!("commit-file-{}", file.path)))
        .group("commit-file-row")
        // GHD `SelectedCommits.onContextMenu`
        .on_mouse_down(MouseButton::Right, {
            let focus = focus.clone();
            let multi = multi.clone();
            let view = view.clone();
            move |ev: &MouseDownEvent, window, cx| {
                cx.stop_propagation();
                // a right-click focuses the list and selects the file first
                // (`List.onRowMouseDown`)
                window.focus(&focus, cx);
                if !is_selected {
                    view.update(cx, |this, cx| {
                        this.click_file(id, menu_path.clone(), FileClick::Plain, cx)
                    })
                    .ok();
                }
                let multi: &[String] = if is_selected { &multi } else { &[] };
                open_commit_file_menu(id, &menu_path, multi, ev.position, window, cx);
            }
        })
        // presses select at once and focus the list
        .on_mouse_down(MouseButton::Left, {
            let focus = focus.clone();
            let path = file.path.clone();
            let view = view.clone();
            move |ev: &MouseDownEvent, window, cx| {
                window.focus(&focus, cx);
                if click_kind(&ev.modifiers) == FileClick::Plain && !is_selected {
                    view.update(cx, |this, cx| {
                        this.click_file(id, path.clone(), FileClick::Plain, cx)
                    })
                    .ok();
                }
            }
        })
        .a11y_row(
            format!(
                "{}, {}",
                file.path,
                crate::widgets::status_label(&file.status)
            ),
            is_selected,
        )
        .w_full()
        .h(ROW_HEIGHT())
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(SPACING_HALF())
        .px(SPACING())
        // `.list-item { border-bottom: 1px solid var(--box-border-color) }`
        .border_b_1()
        .border_color(t.box_border)
        .cursor_pointer()
        .when(is_selected, |d| {
            if list_focused {
                d.bg(t.box_selected_active_background)
                    .text_color(t.box_selected_active_text)
            } else {
                d.bg(t.box_selected_background)
                    .text_color(t.box_selected_text)
            }
        })
        .when(
            !(is_selected && (list_focused || crate::widgets::selection_keeps_colour_on_hover(cx))),
            move |d| d.hover(move |s| s.bg(hover_bg)),
        )
        .on_click({
            let view = view.clone();
            move |ev: &ClickEvent, _, cx| {
                let click = click_kind(&ev.modifiers());
                view.update(cx, |this, cx| this.click_file(id, path.clone(), click, cx))
                    .ok();
            }
        })
        .child({
            // GHD `PathLabel`: `PathText` keeps the file name and shortens
            // the directory from its middle when the row is too narrow;
            // `.list-item.selected .dirname` inherits the row colour
            let (directory_color, arrow_color) = match (is_selected, list_focused) {
                (true, true) => (t.box_selected_active_text, t.box_selected_active_text),
                (true, false) => (t.box_selected_text, t.box_selected_text),
                _ => (t.text_secondary, t.text),
            };
            crate::path_label::path_label_element(
                crate::path_label::path_label(
                    &file.path,
                    file.status.kind,
                    file.old_path.as_deref(),
                ),
                Vec::new(),
                directory_color,
                arrow_color,
            )
            .text_size(FONT_SIZE())
        })
        // `1113-lfs-locks`
        .when_some(
            crate::changes::file_lfs_lock(Some(id), &file.path, cx),
            |d, lock| {
                d.child(crate::changes::lfs_lock_badge(
                    &lock,
                    if is_selected && list_focused {
                        t.box_selected_active_text
                    } else {
                        t.text_secondary
                    },
                ))
            },
        )
        .child(octicon(icon, color))
        // `621-context-menu-buttons`
        .when(crate::context_menu::row_menu_buttons(cx), |d| {
            d.child(
                crate::context_menu::row_menu_button(
                    "row-menu",
                    "commit-file-row",
                    is_selected,
                    if is_selected && list_focused {
                        t.box_selected_active_text
                    } else {
                        t.text_secondary
                    },
                )
                .ml(SPACING_HALF()),
            )
        })
        .when_some(menu_anchor, |d, anchor| d.child(anchor.track()))
        .into_any_element()
}

impl Render for SelectedCommitView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let compact = crate::theme::compact(window);
        self.file_list_focused = self.file_list_focus.is_focused(window);
        let commit = self.state.read(cx).selected_state().and_then(|rs| {
            let sha = rs.selected_commit.as_ref()?;
            rs.find_commit(sha).cloned()
        });
        if let Some(commit) = commit {
            crate::history::request_commit_avatars(&commit, cx);
            if crate::signature_badge::enabled(cx) {
                crate::signature_badge::touch(
                    &commit,
                    corvene_core::signatures::Priority::Selected,
                    cx,
                );
            }
        }
        let t = cx.ghd();
        let (id, has_commit, selected_file, non_contiguous) = {
            let s = self.state.read(cx);
            let id = s.selected;
            let rs = id.and_then(|id| s.repo_states.get(&id));
            (
                id,
                rs.map(|r| r.selected_commit.is_some()).unwrap_or(false),
                rs.and_then(|r| {
                    let path = r.commit_selected_file.as_ref()?;
                    r.changeset
                        .as_ref()?
                        .files
                        .iter()
                        .find(|f| &f.path == path)
                        .map(|f| (f.path.clone(), f.status.kind, f.old_path.clone()))
                }),
                rs.is_some_and(|r| r.selected_commits.len() > 1 && !r.commits_contiguous),
            )
        };
        if non_contiguous {
            // `renderMultipleCommitsBlankSlate`
            let bullet = |text: &'static str| {
                div()
                    .flex()
                    .flex_row()
                    .gap(SPACING_HALF())
                    .child("•")
                    .child(text)
            };
            return div()
                .size_full()
                .flex()
                .flex_col()
                .items_start()
                .p(SPACING_DOUBLE())
                .gap(SPACING_HALF())
                .bg(t.background)
                .text_color(t.text_secondary)
                .text_size(FONT_SIZE())
                .child("Unable to display diff when multiple non-consecutive selected.")
                .child("You can:")
                .child(bullet(
                    "Select a single commit or a range of consecutive commits to view a diff.",
                ))
                .child(bullet(
                    "Drag the commits to the branch menu to cherry-pick them.",
                ))
                .child(bullet("Drag the commits to squash or reorder them."))
                .child(bullet("Right click on multiple commits to see options."))
                .into_any_element();
        }
        let Some(id) = id.filter(|_| has_commit) else {
            // GHD `NoCommitSelected`
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(t.background)
                .text_color(t.text_secondary)
                .child("No commit selected")
                .into_any_element();
        };
        let diff_pane = div()
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .when_some(selected_file, |d, (path, kind, old_path)| {
                d.child(diff_header(
                    &path,
                    kind,
                    old_path.as_deref(),
                    None,
                    &self.diff,
                    cx,
                ))
            })
            .child(DiffView::embed(&self.diff));
        // Corvene (`801-history-review-mode`): the diff alone, full width
        if self.file_list_hidden {
            return div()
                .size_full()
                .flex()
                .flex_col()
                .min_h_0()
                .bg(t.background)
                .children(self.summary(id, cx))
                .child(diff_pane)
                .into_any_element();
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .bg(t.background)
            .children(self.summary(id, cx))
            .child(
                // a phone: the files above the diff instead of beside it
                if compact {
                    v_resizable("commit-details-compact")
                } else {
                    h_resizable("commit-details").with_state(&self.resizable)
                }
                .with_handle_appearance(std::rc::Rc::new(|_, _, _| Some(div().into_any_element())))
                .child(
                    resizable_panel()
                        .size(if compact {
                            zpx(96.)
                        } else {
                            self.file_list_width
                        })
                        .size_range(if compact {
                            zpx(48.)..zpx(600.)
                        } else {
                            FILE_LIST_MIN()..FILE_LIST_MAX()
                        })
                        .child(
                            crate::active_resizable::active_resizable(
                                "commit-file-list-resizable",
                                &self.resizable,
                                Some(&self.file_list_focus),
                                crate::active_resizable::ResizableDescription::new(
                                    "Selected commit file list",
                                    FILE_LIST_MIN()..FILE_LIST_MAX(),
                                ),
                                self.file_list(id, cx),
                            )
                            .key_context("CommitFileList")
                            .on_action(self.menu_anchor.action_handler())
                            .on_action(cx.listener(|this, _: &SelectNextFile, _, cx| {
                                this.select_relative(1, cx)
                            }))
                            .on_action(cx.listener(|this, _: &SelectPreviousFile, _, cx| {
                                this.select_relative(-1, cx)
                            }))
                            .on_action(cx.listener(|this, _: &SelectFirstFile, _, cx| {
                                this.select_edge(false, cx)
                            }))
                            .on_action(cx.listener(|this, _: &SelectLastFile, _, cx| {
                                this.select_edge(true, cx)
                            }))
                            .on_action(cx.listener(|this, _: &ExtendSelectionDown, _, cx| {
                                this.extend_selection(1, cx)
                            }))
                            .on_action(cx.listener(|this, _: &ExtendSelectionUp, _, cx| {
                                this.extend_selection(-1, cx)
                            }))
                            .on_action(cx.listener(|this, _: &CopySelectedFilePaths, _, cx| {
                                this.copy_selected_path(true, cx)
                            }))
                            .on_action(cx.listener(
                                |this, _: &CopySelectedRelativeFilePaths, _, cx| {
                                    this.copy_selected_path(false, cx)
                                },
                            ))
                            .on_action(cx.listener(|this, _: &OpenSelectedFileInEditor, _, cx| {
                                if let Some(path) = this.selected_file_on_disk(cx) {
                                    Dispatcher::open_in_editor(path, cx)
                                }
                            }))
                            .on_action(cx.listener(
                                |this, _: &OpenSelectedFileWithDefaultProgram, _, cx| {
                                    if let Some(path) = this.selected_file_on_disk(cx) {
                                        cx.open_with_system(&path)
                                    }
                                },
                            )),
                        ),
                )
                .child(resizable_panel().child(diff_pane)),
            )
            .into_any_element()
    }
}
