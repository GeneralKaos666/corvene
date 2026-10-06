//! Corvene `351-actions` (desktop/desktop#17498): a GitHub repository's
//! Actions (`corvene_core::github_actions`). GitHub Desktop has no such
//! view; its check-run popover (`ui/check-runs/`) only lists a pull
//! request's checks.
//!
//! A wide dialog in three panes, like the Actions tab on github.com: the
//! workflows (All workflows first, disabled ones dimmed), the runs of the
//! selection with the Branch / Status / Event filters, Run workflow (for a
//! workflow with `workflow_dispatch`) and Load more, and the selected run:
//! its status, what started it, Re-run all jobs / Re-run failed jobs /
//! Cancel run and a menu (Delete logs, Copy link, Open on GitHub), then its
//! jobs, each expanding to its steps. A job's or step's log opens in the
//! job log dialog (`347-actions-job-logs`). A message under the header
//! reports what an action did or why it failed.

use std::time::SystemTime;

use corvene_core::github_actions::{
    ActionsViewState, DefinitionState, JobRow, RunFilter, RunRow, STATUS_FILTERS, actions_of,
};
use corvene_core::{AppState, Dispatcher};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::ci_status::{ci_status, color_for, effective_conclusion, symbol_for_log_step};
use crate::context_menu::{MenuItem, mac_or};
use crate::dialog::{DialogFrame, dialog_with_frame};
use crate::github_list::{self, date_text_at, pill};
use crate::icons::{Octicon, octicon, spin};
use crate::scrollbar::ScrollbarExt;
use crate::theme::sizes::*;
use crate::theme::{ActiveGhdTheme, GhdTheme};
use crate::widgets::{
    GhdTooltip, IconButtonA11y, SelectHandler, SelectItem, button, button_disabled, primary_button,
    select_button_items,
};

/// The width of the dialog for a window of `viewport` (most of it, at most
/// 1,120 px).
pub fn dialog_width(viewport: Size<Pixels>) -> Pixels {
    (viewport.width * 0.92).min(zpx(1120.)).max(zpx(400.))
}

fn body_height(viewport: Size<Pixels>) -> Pixels {
    (viewport.height - zpx(150.)).clamp(zpx(300.), zpx(640.))
}

fn workflows_width() -> Pixels {
    zpx(210.)
}

fn detail_width() -> Pixels {
    zpx(400.)
}

fn run_row_height() -> Pixels {
    zpx(52.)
}

pub struct ActionsDialog {
    state: Entity<AppState>,
    focus: FocusHandle,
    runs_scroll: UniformListScrollHandle,
    detail_scroll: ScrollHandle,
    scrolled_to: Option<u64>,
}

impl ActionsDialog {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        Self {
            state,
            focus,
            runs_scroll: UniformListScrollHandle::new(),
            detail_scroll: ScrollHandle::new(),
            scrolled_to: None,
        }
    }

    fn close(_: &mut Window, cx: &mut App) {
        Dispatcher::close_actions(cx);
    }

    /// Up / Down in the runs.
    fn move_selection(&mut self, down: bool, cx: &mut Context<Self>) {
        let next = {
            let s = self.state.read(cx);
            let Some(v) = actions_of(s) else { return };
            if v.runs.is_empty() {
                return;
            }
            let current = v
                .selected_run
                .and_then(|sel| v.runs.iter().position(|r| r.id == sel));
            let ix = match (current, down) {
                (None, _) => 0,
                (Some(c), true) => (c + 1).min(v.runs.len() - 1),
                (Some(c), false) => c.saturating_sub(1),
            };
            v.runs[ix].id
        };
        Dispatcher::select_actions_run(next, cx);
    }

    // ---- workflows ----

    fn workflows_pane(&self, v: &ActionsViewState, t: &GhdTheme) -> Stateful<Div> {
        let row = |id: Option<u64>, ix: usize, label: String, disabled: bool| {
            let selected = v.selected_workflow == id;
            let hover = t.box_hover_background;
            div()
                .id(("actions-workflow", ix))
                .h(zpx(29.))
                .w_full()
                .flex()
                .flex_row()
                .items_center()
                .gap(SPACING_HALF())
                .px(SPACING())
                .rounded(BORDER_RADIUS())
                .cursor_pointer()
                .text_size(FONT_SIZE())
                .when(selected, |d| {
                    d.bg(t.box_selected_background)
                        .text_color(t.box_selected_text)
                        .font_weight(FontWeight::SEMIBOLD)
                })
                .when(!selected, move |d| d.hover(move |s| s.bg(hover)))
                .when(disabled, |d| d.text_color(t.text_secondary))
                .on_click(move |_, _, cx| Dispatcher::select_actions_workflow(id, cx))
                .child(
                    octicon(
                        if id.is_none() {
                            Octicon::ListUnordered
                        } else {
                            Octicon::Workflow
                        },
                        t.text_secondary,
                    )
                    .flex_none(),
                )
                .child(div().flex_1().min_w_0().truncate().child(label.clone()))
                .when(disabled, |d| d.child(pill("Disabled", t.text_secondary)))
                .ghd_tooltip(label)
        };
        let mut rows: Vec<AnyElement> =
            vec![row(None, 0, "All workflows".into(), false).into_any_element()];
        for (ix, w) in v.workflows.iter().enumerate() {
            rows.push(row(Some(w.id), ix + 1, w.name.clone(), !w.is_active()).into_any_element());
        }
        let status: Option<AnyElement> = if v.workflows_loading && !v.workflows_loaded {
            Some(github_list::loading("workflows", "actions-workflows-spin", t).into_any_element())
        } else {
            v.workflows_error.clone().map(|message| {
                div()
                    .px(SPACING())
                    .pt(SPACING())
                    .text_size(FONT_SIZE_SM())
                    .text_color(t.dialog_error)
                    .child(message)
                    .into_any_element()
            })
        };
        div()
            .id("actions-workflows")
            .flex_none()
            .w(workflows_width())
            .h_full()
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(t.box_border)
            .bg(t.box_alt_background)
            .child(
                div()
                    .flex_none()
                    .px(SPACING())
                    .pt(SPACING())
                    .pb(SPACING_HALF())
                    .text_size(FONT_SIZE_SM())
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(t.text_secondary)
                    .child("Workflows"),
            )
            .child(
                div()
                    .id("actions-workflow-rows")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(SPACING_HALF())
                    .pb(SPACING())
                    .flex()
                    .flex_col()
                    .gap(zpx(1.))
                    .children(rows)
                    .children(status)
                    .with_scrollbar(),
            )
    }

    // ---- runs ----

    fn filters(&self, v: &ActionsViewState, cx: &App) -> Div {
        let filter = v.filter.clone();
        // Branch
        let branches = v.branch_choices();
        let mut items = vec![SelectItem::Option("All branches".into())];
        if !branches.is_empty() {
            items.push(SelectItem::Separator);
            items.extend(
                branches
                    .iter()
                    .map(|b| SelectItem::Option(SharedString::from(b.clone()))),
            );
        }
        let selected = match &filter.branch {
            None => Some(0),
            Some(b) => branches.iter().position(|x| x == b).map(|i| i + 1),
        };
        let on_branch: SelectHandler = std::rc::Rc::new({
            let filter = filter.clone();
            let branches = branches.clone();
            move |ix, _, cx| {
                let mut f = filter.clone();
                f.branch = if ix == 0 {
                    None
                } else {
                    branches.get(ix - 1).cloned()
                };
                Dispatcher::set_actions_filter(f, cx);
            }
        });
        let branch = select_button_items(
            "actions-filter-branch",
            filter
                .branch
                .clone()
                .unwrap_or_else(|| "All branches".into()),
            items,
            selected,
            false,
            on_branch,
            cx,
        );
        // Status
        let mut items = vec![
            SelectItem::Option("Any status".into()),
            SelectItem::Separator,
        ];
        items.extend(
            STATUS_FILTERS
                .iter()
                .map(|(label, _)| SelectItem::Option((*label).into())),
        );
        let selected = match &filter.status {
            None => Some(0),
            Some(s) => STATUS_FILTERS
                .iter()
                .position(|(_, api)| api == s)
                .map(|i| i + 1),
        };
        let on_status: SelectHandler = std::rc::Rc::new({
            let filter = filter.clone();
            move |ix, _, cx| {
                let mut f = filter.clone();
                f.status = if ix == 0 {
                    None
                } else {
                    STATUS_FILTERS.get(ix - 1).map(|(_, api)| api.to_string())
                };
                Dispatcher::set_actions_filter(f, cx);
            }
        });
        let status_label = filter
            .status
            .as_deref()
            .and_then(|s| STATUS_FILTERS.iter().find(|(_, api)| *api == s))
            .map(|(label, _)| label.to_string())
            .unwrap_or_else(|| "Any status".into());
        let status = select_button_items(
            "actions-filter-status",
            status_label,
            items,
            selected,
            false,
            on_status,
            cx,
        );
        // Event
        let events = v.event_choices();
        let mut items = vec![
            SelectItem::Option("Any event".into()),
            SelectItem::Separator,
        ];
        items.extend(
            events
                .iter()
                .map(|e| SelectItem::Option(SharedString::from(e.clone()))),
        );
        let selected = match &filter.event {
            None => Some(0),
            Some(e) => events.iter().position(|x| x == e).map(|i| i + 1),
        };
        let on_event: SelectHandler = std::rc::Rc::new({
            let filter = filter.clone();
            move |ix, _, cx| {
                let mut f = filter.clone();
                f.event = if ix == 0 {
                    None
                } else {
                    events.get(ix - 1).cloned()
                };
                Dispatcher::set_actions_filter(f, cx);
            }
        });
        let event = select_button_items(
            "actions-filter-event",
            filter.event.clone().unwrap_or_else(|| "Any event".into()),
            items,
            selected,
            false,
            on_event,
            cx,
        );
        div()
            .flex_none()
            .flex()
            .flex_row()
            .gap(SPACING_HALF())
            .px(SPACING())
            .pb(SPACING())
            .child(div().flex_1().min_w_0().child(branch))
            .child(div().flex_1().min_w_0().child(status))
            .child(div().flex_1().min_w_0().child(event))
    }

    fn runs_header(&self, v: &ActionsViewState, t: &GhdTheme, cx: &App) -> Div {
        let workflow = v.selected_workflow.and_then(|id| v.workflow(id));
        let title = workflow
            .map(|w| w.name.clone())
            .unwrap_or_else(|| "All workflows".into());
        let subtitle = if v.runs_loaded && v.runs_error.is_none() {
            let n = v.total_runs;
            Some(format!("{n} run{}", if n == 1 { "" } else { "s" }))
        } else {
            None
        };
        // Run workflow: a workflow whose file has `workflow_dispatch`
        let dispatchable = workflow.is_some_and(|w| {
            w.is_active()
                && matches!(
                    v.default_definition(w.id),
                    Some(DefinitionState::Loaded(spec)) if spec.dispatchable
                )
        });
        let blocker = v.write_blocker();
        let run_button: Option<AnyElement> = workflow.filter(|_| dispatchable).map(|w| {
            let id = w.id;
            let disabled = blocker.is_some();
            let b = primary_button(
                "actions-run-workflow",
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING_HALF())
                    .child(octicon(Octicon::Play, t.button_text))
                    .child(mac_or("Run Workflow…", "Run workflow…")),
                disabled,
                cx,
            )
            .flex_none();
            match blocker.clone() {
                Some(why) => b.ghd_tooltip(why).into_any_element(),
                None => b
                    .on_click(move |_, _, cx| Dispatcher::show_run_workflow(id, cx))
                    .into_any_element(),
            }
        });
        let file: Option<String> = workflow
            .filter(|w| w.has_file())
            .map(|w| w.file_name().to_string());
        div()
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .gap(SPACING_HALF())
            .px(SPACING())
            .pt(SPACING())
            .pb(SPACING())
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .truncate()
                            .text_size(FONT_SIZE_MD())
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(FONT_SIZE_SM())
                            .text_color(t.text_secondary)
                            .child(
                                [file, subtitle]
                                    .into_iter()
                                    .flatten()
                                    .collect::<Vec<_>>()
                                    .join(" · "),
                            ),
                    ),
            )
            .children(run_button)
    }

    fn runs_list(
        &mut self,
        v: &ActionsViewState,
        t: &GhdTheme,
        window: &Window,
        cx: &App,
    ) -> AnyElement {
        if !v.runs_loaded {
            return github_list::loading("runs", "actions-runs-spin", t).into_any_element();
        }
        if let Some(message) = v.runs_error.clone().filter(|_| v.runs.is_empty()) {
            return github_list::error(
                message,
                |_, cx| Dispatcher::load_actions_runs(false, cx),
                cx,
            )
            .into_any_element();
        }
        if v.runs.is_empty() {
            let text = if v.filter.is_empty() {
                "No runs yet."
            } else {
                "No runs match the filters."
            };
            return div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .gap(SPACING_HALF())
                .child(github_list::message(text, t))
                .when(!v.filter.is_empty(), |d| {
                    d.child(
                        crate::widgets::link_button(
                            "actions-clear-filters",
                            "Clear the filters",
                            cx,
                        )
                        .on_click(|_, _, cx| {
                            Dispatcher::set_actions_filter(RunFilter::default(), cx)
                        }),
                    )
                })
                .into_any_element();
        }
        if v.selected_run != self.scrolled_to {
            self.scrolled_to = v.selected_run;
            if let Some(ix) = v
                .selected_run
                .and_then(|sel| v.runs.iter().position(|r| r.id == sel))
            {
                self.runs_scroll.scroll_to_item(ix, ScrollStrategy::Nearest);
            }
        }
        let now = SystemTime::now();
        let rows = v.runs.clone();
        let selected = v.selected_run;
        let focused = self.focus.contains_focused(window, cx);
        let has_more = v.has_more();
        let loading_more = v.loading_more;
        let t = t.clone();
        let focus = self.focus.clone();
        let blocker = v.write_blocker();
        let count = rows.len() + usize::from(has_more);
        uniform_list("actions-run-rows", count, move |range, _window, cx| {
            range
                .map(|ix| {
                    let Some(run) = rows.get(ix) else {
                        // Load more
                        return div()
                            .id("actions-load-more")
                            .h(run_row_height())
                            .w_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(if loading_more {
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap(SPACING_HALF())
                                    .text_color(t.text_secondary)
                                    .child(spin(
                                        octicon(Octicon::SyncClockwise, t.text_secondary),
                                        "actions-more-spin",
                                    ))
                                    .child("Loading…")
                                    .into_any_element()
                            } else {
                                crate::widgets::link_button(
                                    "actions-load-more-link",
                                    "Load more runs",
                                    cx,
                                )
                                .on_click(|_, _, cx| Dispatcher::load_actions_runs(true, cx))
                                .into_any_element()
                            })
                            .into_any_element();
                    };
                    run_row(
                        run,
                        ix,
                        selected == Some(run.id),
                        focused,
                        now,
                        &t,
                        &focus,
                        blocker.clone(),
                    )
                })
                .collect()
        })
        .track_scroll(&self.runs_scroll)
        .flex_1()
        .min_h_0()
        .with_scrollbar_handle(&self.runs_scroll)
        .into_any_element()
    }

    // ---- the run ----

    fn detail_pane(&self, v: &ActionsViewState, t: &GhdTheme, cx: &App) -> Stateful<Div> {
        let frame = div()
            .id("actions-detail")
            .flex_none()
            .w(detail_width())
            .h_full()
            .flex()
            .flex_col()
            .border_l_1()
            .border_color(t.box_border);
        let Some(run) = v.selected_run_row().cloned() else {
            return frame.child(github_list::message(
                if v.runs.is_empty() {
                    ""
                } else {
                    "Select a run to see its jobs."
                },
                t,
            ));
        };
        let now = SystemTime::now();
        let blocker = v.write_blocker();
        let busy = v.busy.filter(|(id, _)| *id == run.id).map(|(_, text)| text);
        let header = div()
            .flex_none()
            .flex()
            .flex_col()
            .gap(SPACING_HALF())
            .px(SPACING())
            .pt(SPACING())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_start()
                    .gap(SPACING_HALF())
                    .child(
                        div()
                            .pt(zpx(2.))
                            .child(ci_status(run.status, run.conclusion)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(FONT_SIZE_MD())
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(run.title.clone()),
                    ),
            )
            .child(
                div()
                    .text_size(FONT_SIZE_SM())
                    .text_color(t.text_secondary)
                    .child({
                        let mut text = format!("{} #{}", run.workflow_name, run.run_number);
                        if run.run_attempt > 1 {
                            text.push_str(&format!(" · attempt {}", run.run_attempt));
                        }
                        text.push_str(&format!(" · {}", run.status_text()));
                        text
                    }),
            )
            .child(meta_grid(&run, now, t))
            .child(self.run_buttons(&run, busy, blocker.clone(), t, cx))
            .when_some(blocker, |d, why| {
                d.child(
                    div()
                        .text_size(FONT_SIZE_SM())
                        .text_color(t.dialog_warning)
                        .child(why),
                )
            });
        let jobs: AnyElement =
            if v.jobs_run != Some(run.id) || (v.jobs_loading && v.jobs.is_empty()) {
                github_list::loading("jobs", "actions-jobs-spin", t).into_any_element()
            } else if let Some(message) = v.jobs_error.clone().filter(|_| v.jobs.is_empty()) {
                github_list::error(message, |_, cx| Dispatcher::load_actions_jobs(true, cx), cx)
                    .into_any_element()
            } else if v.jobs.is_empty() {
                github_list::message("This run has no jobs.", t).into_any_element()
            } else {
                div()
                    .flex()
                    .flex_col()
                    .children(v.jobs.iter().enumerate().map(|(ix, job)| {
                        job_rows(
                            &run,
                            job,
                            ix,
                            v.expanded_job == Some(job.check.id),
                            v.write_blocker().is_none() && v.busy.is_none(),
                            t,
                        )
                    }))
                    .into_any_element()
            };
        frame.child(
            div()
                .id("actions-detail-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .track_scroll(&self.detail_scroll)
                .flex()
                .flex_col()
                .child(header)
                .child(
                    div()
                        .mt(SPACING())
                        .px(SPACING())
                        .pb(SPACING_HALF())
                        .text_size(FONT_SIZE_SM())
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(t.text_secondary)
                        .child("Jobs"),
                )
                .child(div().pb(SPACING()).child(jobs))
                .with_scrollbar(),
        )
    }

    fn run_buttons(
        &self,
        run: &RunRow,
        busy: Option<&'static str>,
        blocker: Option<String>,
        t: &GhdTheme,
        cx: &App,
    ) -> Div {
        let id = run.id;
        let blocked = blocker.is_some() || busy.is_some();
        let action =
            |key: &'static str, label: &'static str, on: fn(u64, &mut App)| -> AnyElement {
                if blocked {
                    let b = button_disabled(key, label, cx).flex_none();
                    match &blocker {
                        Some(why) => b.ghd_tooltip(why.clone()).into_any_element(),
                        None => b.into_any_element(),
                    }
                } else {
                    button(key, label, cx)
                        .flex_none()
                        .on_click(move |_, _, cx| on(id, cx))
                        .into_any_element()
                }
            };
        let mut row = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap(SPACING_HALF())
            .pt(SPACING_HALF());
        if run.is_active() {
            row = row.child(action("actions-cancel", "Cancel run", |id, cx| {
                Dispatcher::cancel_actions_run(id, cx)
            }));
        } else {
            row = row.child(action("actions-rerun-all", "Re-run all jobs", |id, cx| {
                Dispatcher::rerun_actions_run(id, false, cx)
            }));
            if run.has_failed_jobs() {
                row = row.child(action(
                    "actions-rerun-failed",
                    "Re-run failed jobs",
                    |id, cx| Dispatcher::rerun_actions_run(id, true, cx),
                ));
            }
        }
        let url = run.html_url.clone();
        let can_delete = !blocked && !run.is_active();
        row = row.child(
            button("actions-run-menu", "", cx)
                .flex_none()
                .px(SPACING_HALF())
                .icon_button_label("More actions")
                .child(octicon(Octicon::KebabHorizontal, t.secondary_button_text))
                .on_click(move |ev, window, cx| {
                    let (open, copy) = (url.clone(), url.clone());
                    let items = vec![
                        MenuItem::new(mac_or("Open on GitHub", "Open on GitHub"), move |_, cx| {
                            Dispatcher::open_actions_url(open.clone(), cx)
                        }),
                        MenuItem::new(mac_or("Copy Link", "Copy link"), move |_, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()))
                        }),
                        MenuItem::separator(),
                        MenuItem::new(mac_or("Delete Logs…", "Delete logs"), move |_, cx| {
                            Dispatcher::delete_actions_run_logs(id, cx)
                        })
                        .enabled(can_delete),
                    ];
                    let position = ev.mouse_position().unwrap_or_default();
                    crate::native_menu::show_context_menu(items, position, window, cx);
                }),
        );
        if let Some(text) = busy {
            row = row.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING_HALF())
                    .text_size(FONT_SIZE_SM())
                    .text_color(t.text_secondary)
                    .child(spin(
                        octicon(Octicon::SyncClockwise, t.text_secondary),
                        "actions-busy-spin",
                    ))
                    .child(text),
            );
        }
        row
    }

    fn notice(&self, v: &ActionsViewState, t: &GhdTheme) -> Option<Div> {
        let n = v.notice.clone()?;
        let (bg, border, fg) = if n.error {
            (
                t.form_error_background,
                t.form_error_border,
                t.form_error_text,
            )
        } else {
            (t.box_alt_background, t.box_border, t.text)
        };
        Some(
            div()
                .flex_none()
                .flex()
                .flex_row()
                .items_center()
                .gap(SPACING())
                .px(SPACING_DOUBLE())
                .py(SPACING_HALF())
                .bg(bg)
                .border_b_1()
                .border_color(border)
                .text_color(fg)
                .text_size(FONT_SIZE())
                .child(
                    octicon(
                        if n.error {
                            Octicon::Alert
                        } else {
                            Octicon::Info
                        },
                        fg,
                    )
                    .flex_none(),
                )
                .child(div().flex_1().min_w_0().child(n.text))
                .child(
                    div()
                        .id("actions-notice-dismiss")
                        .flex_none()
                        .cursor_pointer()
                        .icon_button_label("Dismiss")
                        .child(octicon(Octicon::X, fg))
                        .on_click(|_, _, cx| Dispatcher::dismiss_actions_notice(cx)),
                ),
        )
    }
}

/// `Created` / `Duration` and the rest under the run's title.
fn meta_grid(run: &RunRow, now: SystemTime, t: &GhdTheme) -> Div {
    let mut items: Vec<(Octicon, String, String)> = Vec::new();
    if let Some(branch) = &run.branch {
        items.push((Octicon::GitBranch, "Branch".into(), branch.clone()));
    }
    if !run.event.is_empty() {
        items.push((Octicon::Zap, "Event".into(), run.event.clone()));
    }
    if !run.actor.is_empty() {
        items.push((Octicon::Person, "Triggered by".into(), run.actor.clone()));
    }
    if !run.head_sha.is_empty() {
        items.push((
            Octicon::GitCommit,
            "Commit".into(),
            run.head_sha[..run.head_sha.len().min(7)].to_string(),
        ));
    }
    let started = date_text_at(
        run.run_started_at
            .as_deref()
            .or(Some(run.created_at.as_str())),
        now,
    );
    if !started.is_empty() {
        items.push((Octicon::Clock, "Started".into(), started));
    }
    if let Some(ms) = run.duration_ms(now) {
        items.push((
            Octicon::Stopwatch,
            if run.is_active() {
                "Running for".into()
            } else {
                "Duration".into()
            },
            corvene_core::format_precise_duration(ms),
        ));
    }
    div()
        .flex()
        .flex_col()
        .gap(zpx(3.))
        .pt(SPACING_HALF())
        .text_size(FONT_SIZE_SM())
        .children(items.into_iter().map(|(icon, label, value)| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(SPACING_HALF())
                .child(octicon(icon, t.text_secondary).flex_none())
                .child(
                    div()
                        .flex_none()
                        .w(zpx(80.))
                        .text_color(t.text_secondary)
                        .child(label),
                )
                .child(div().flex_1().min_w_0().truncate().child(value))
        }))
}

#[allow(clippy::too_many_arguments)]
fn run_row(
    run: &RunRow,
    ix: usize,
    is_selected: bool,
    focused: bool,
    now: SystemTime,
    t: &GhdTheme,
    focus: &FocusHandle,
    blocker: Option<String>,
) -> AnyElement {
    let (bg, text, secondary) = if is_selected && focused {
        (
            t.box_selected_active_background,
            t.box_selected_active_text,
            t.box_selected_active_text,
        )
    } else if is_selected {
        (
            t.box_selected_background,
            t.box_selected_text,
            t.text_secondary,
        )
    } else {
        (t.background, t.text, t.text_secondary)
    };
    let id = run.id;
    let mut byline: Vec<String> = Vec::new();
    if let Some(b) = &run.branch {
        byline.push(b.clone());
    }
    if !run.event.is_empty() {
        byline.push(run.event.clone());
    }
    if !run.actor.is_empty() {
        byline.push(run.actor.clone());
    }
    let when = date_text_at(Some(&run.created_at), now);
    let duration = run
        .duration_ms(now)
        .map(corvene_core::format_precise_duration)
        .unwrap_or_default();
    let (focus, menu_focus) = (focus.clone(), focus.clone());
    let target = run.clone();
    let hover = t.box_hover_background;
    div()
        .id(("actions-run", ix))
        .h(run_row_height())
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .gap(SPACING_HALF())
        .px(SPACING())
        .bg(bg)
        .when(!is_selected, move |d| d.hover(move |s| s.bg(hover)))
        .border_b_1()
        .border_color(t.box_border)
        .cursor_pointer()
        .on_click(move |_, window, cx| {
            window.focus(&focus, cx);
            Dispatcher::select_actions_run(id, cx);
        })
        .on_mouse_down(MouseButton::Right, move |ev, window, cx| {
            cx.stop_propagation();
            window.focus(&menu_focus, cx);
            Dispatcher::select_actions_run(id, cx);
            run_menu(&target, blocker.clone(), ev.position, window, cx);
        })
        .child(div().flex_none().child(if is_selected && focused {
            // the status colours are lost on the selection's blue
            octicon(
                crate::ci_status::symbol_for(effective_conclusion(run.status, run.conclusion)),
                text,
            )
        } else {
            ci_status(run.status, run.conclusion)
        }))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(SPACING_HALF())
                        .text_size(FONT_SIZE())
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(text)
                                .child(run.title.clone()),
                        )
                        .child(
                            div()
                                .flex_none()
                                .text_color(secondary)
                                .child(format!("#{}", run.run_number)),
                        ),
                )
                .child(
                    div()
                        .mt(zpx(2.))
                        .truncate()
                        .text_size(FONT_SIZE_SM())
                        .text_color(secondary)
                        .child(format!("{} · {}", run.workflow_name, byline.join(" · "))),
                ),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .flex_col()
                .items_end()
                .gap(zpx(2.))
                .text_size(FONT_SIZE_SM())
                .text_color(secondary)
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(zpx(3.))
                        .child(octicon(Octicon::Clock, secondary).size(zpx(12.)))
                        .child(when),
                )
                .when(!duration.is_empty(), |d| {
                    d.child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(zpx(3.))
                            .child(octicon(Octicon::Stopwatch, secondary).size(zpx(12.)))
                            .child(duration),
                    )
                }),
        )
        .into_any_element()
}

/// A run row's context menu.
fn run_menu(
    run: &RunRow,
    blocker: Option<String>,
    position: Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let id = run.id;
    let (open, copy) = (run.html_url.clone(), run.html_url.clone());
    let writable = blocker.is_none();
    let mut items = vec![
        MenuItem::new(mac_or("Open on GitHub", "Open on GitHub"), move |_, cx| {
            Dispatcher::open_actions_url(open.clone(), cx)
        }),
        MenuItem::new(mac_or("Copy Link", "Copy link"), move |_, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()))
        }),
        MenuItem::separator(),
    ];
    if run.is_active() {
        items.push(
            MenuItem::new(mac_or("Cancel Run", "Cancel run"), move |_, cx| {
                Dispatcher::cancel_actions_run(id, cx)
            })
            .enabled(writable),
        );
    } else {
        items.push(
            MenuItem::new(
                mac_or("Re-run All Jobs", "Re-run all jobs"),
                move |_, cx| Dispatcher::rerun_actions_run(id, false, cx),
            )
            .enabled(writable),
        );
        items.push(
            MenuItem::new(
                mac_or("Re-run Failed Jobs", "Re-run failed jobs"),
                move |_, cx| Dispatcher::rerun_actions_run(id, true, cx),
            )
            .enabled(writable && run.has_failed_jobs()),
        );
    }
    crate::native_menu::show_context_menu(items, position, window, cx);
}

/// A job's row and, expanded, its steps.
fn job_rows(
    run: &RunRow,
    job: &JobRow,
    ix: usize,
    expanded: bool,
    writable: bool,
    t: &GhdTheme,
) -> Div {
    let job_id = job.check.id;
    let run_id = run.id;
    let hover = t.box_hover_background;
    let duration = job
        .duration_ms()
        .filter(|_| !job.is_active())
        .map(corvene_core::format_precise_duration)
        .unwrap_or_default();
    let url = job.check.html_url.clone().unwrap_or_default();
    let can_rerun = writable && !run.is_active();
    let header = div()
        .id(("actions-job", ix))
        .h(zpx(30.))
        .flex()
        .flex_row()
        .items_center()
        .gap(SPACING_HALF())
        .px(SPACING())
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .on_click(move |_, _, cx| Dispatcher::toggle_actions_job(job_id, cx))
        .on_mouse_down(MouseButton::Right, move |ev, window, cx| {
            cx.stop_propagation();
            let open = url.clone();
            let items = vec![
                MenuItem::new(mac_or("View Log", "View log"), move |_, cx| {
                    Dispatcher::open_actions_job_log(job_id, None, cx)
                }),
                MenuItem::new(mac_or("Open on GitHub", "Open on GitHub"), move |_, cx| {
                    Dispatcher::open_actions_url(open.clone(), cx)
                }),
                MenuItem::separator(),
                MenuItem::new(mac_or("Re-run Job", "Re-run job"), move |_, cx| {
                    Dispatcher::rerun_actions_job(run_id, job_id, cx)
                })
                .enabled(can_rerun),
            ];
            crate::native_menu::show_context_menu(items, ev.position, window, cx);
        })
        .child(
            octicon(
                if expanded {
                    Octicon::ChevronDown
                } else {
                    Octicon::ChevronRight
                },
                t.text_secondary,
            )
            .flex_none(),
        )
        .child(ci_status(job.check.status, job.check.conclusion).flex_none())
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(FONT_SIZE())
                .child(job.check.name.clone()),
        )
        .child(
            div()
                .flex_none()
                .text_size(FONT_SIZE_SM())
                .text_color(t.text_secondary)
                .child(duration),
        )
        .child(
            div()
                .id(("actions-job-log", ix))
                .flex_none()
                .p(zpx(2.))
                .rounded(BORDER_RADIUS())
                .cursor_pointer()
                .icon_button_label("View log")
                .hover(move |s| s.bg(hover))
                .child(octicon(Octicon::FileCode, t.text_secondary))
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    Dispatcher::open_actions_job_log(job_id, None, cx)
                }),
        );
    let steps: Vec<AnyElement> = if expanded {
        job.check
            .job_steps
            .iter()
            .flatten()
            .enumerate()
            .map(|(six, step)| {
                let name = step.name.clone();
                let conclusion = effective_conclusion(step.status, step.conclusion);
                let duration = corvene_core::check_duration_ms(
                    step.started_at.as_deref(),
                    step.completed_at.as_deref(),
                )
                .filter(|_| step.status == corvene_core::CheckStatus::Completed)
                .map(corvene_core::format_precise_duration)
                .unwrap_or_default();
                div()
                    .id(("actions-step", ix * 1000 + six))
                    .h(zpx(24.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING_HALF())
                    .pl(zpx(42.))
                    .pr(SPACING())
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover))
                    .text_size(FONT_SIZE_SM())
                    .on_click(move |_, _, cx| {
                        Dispatcher::open_actions_job_log(job_id, Some(name.clone()), cx)
                    })
                    .child(
                        octicon(
                            symbol_for_log_step(step.status, step.conclusion),
                            color_for(conclusion),
                        )
                        .flex_none(),
                    )
                    .child(
                        div()
                            .flex_none()
                            .w(zpx(18.))
                            .text_color(t.text_secondary)
                            .child(step.number.to_string()),
                    )
                    .child(div().flex_1().min_w_0().truncate().child(step.name.clone()))
                    .child(
                        div()
                            .flex_none()
                            .text_color(t.text_secondary)
                            .child(duration),
                    )
                    .into_any_element()
            })
            .collect()
    } else {
        Vec::new()
    };
    div()
        .flex()
        .flex_col()
        .border_b_1()
        .border_color(t.box_border)
        .child(header)
        .children(steps)
}

impl Render for ActionsDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd().clone();
        let view = actions_of(self.state.read(cx)).cloned();
        let viewport = crate::theme::page_size(window);
        let Some(v) = view else {
            return dialog_with_frame(
                "dialog-actions",
                "Actions",
                div(),
                Vec::new(),
                DialogFrame {
                    content_padding: false,
                    ..Default::default()
                },
                Self::close,
                window,
                cx,
            )
            .into_any_element();
        };
        let title = format!("Actions · {}", v.github.full_name());
        let body: AnyElement = if v.signed_out {
            div()
                .h(body_height(viewport))
                .flex()
                .flex_col()
                .child(github_list::signed_out(
                    "workflow runs",
                    v.github.full_name(),
                    v.github.endpoint.clone(),
                    cx,
                ))
                .into_any_element()
        } else {
            let runs_error_line =
                v.runs_error
                    .clone()
                    .filter(|_| !v.runs.is_empty())
                    .map(|message| {
                        div()
                            .flex_none()
                            .px(SPACING())
                            .pb(SPACING_HALF())
                            .text_size(FONT_SIZE_SM())
                            .text_color(t.dialog_error)
                            .child(message)
                    });
            let runs = self.runs_list(&v, &t, window, cx);
            let weak = cx.entity().downgrade();
            div()
                .h(body_height(viewport))
                .flex()
                .flex_row()
                .min_h_0()
                .child(self.workflows_pane(&v, &t))
                .child(
                    div()
                        .id("actions-runs")
                        .track_focus(&self.focus)
                        .on_key_down(move |ev, _, cx| {
                            let down = match ev.keystroke.key.as_str() {
                                "down" => true,
                                "up" => false,
                                _ => return,
                            };
                            cx.stop_propagation();
                            weak.update(cx, |this, cx| this.move_selection(down, cx))
                                .ok();
                        })
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .flex()
                        .flex_col()
                        .child(self.runs_header(&v, &t, cx))
                        .child(self.filters(&v, cx))
                        .children(runs_error_line)
                        .child(
                            div()
                                .flex_1()
                                .min_h_0()
                                .flex()
                                .flex_col()
                                .border_t_1()
                                .border_color(t.box_border)
                                .child(runs),
                        ),
                )
                .child(self.detail_pane(&v, &t, cx))
                .into_any_element()
        };
        let refresh = div()
            .id("actions-refresh")
            .flex_none()
            .icon_button_label("Refresh")
            .cursor_pointer()
            .child(if v.runs_loading || v.workflows_loading || v.jobs_loading {
                spin(
                    octicon(Octicon::SyncClockwise, t.text_secondary),
                    "actions-refresh-spin",
                )
                .into_any_element()
            } else {
                octicon(Octicon::Sync, t.text_secondary).into_any_element()
            })
            .on_click(|_, _, cx| Dispatcher::refresh_actions(cx));
        let open_url = format!("{}/actions", v.github.html_url);
        let toolbar = div()
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .gap(SPACING())
            .px(SPACING_DOUBLE())
            .py(SPACING_HALF())
            .border_b_1()
            .border_color(t.box_border)
            .text_size(FONT_SIZE_SM())
            .text_color(t.text_secondary)
            .child(div().flex_1().min_w_0().truncate().child(if v.is_live() {
                "Updating every few seconds while runs are in progress."
            } else {
                ""
            }))
            .child(refresh)
            .child(
                crate::widgets::link_button("actions-open-github", "Open on GitHub", cx)
                    .flex_none()
                    .on_click(move |_, _, cx| Dispatcher::open_url(&open_url, cx)),
            );
        let content = div()
            .flex()
            .flex_col()
            .child(toolbar)
            .children(self.notice(&v, &t))
            .child(body);
        dialog_with_frame(
            "dialog-actions",
            title,
            content,
            Vec::new(),
            DialogFrame {
                content_padding: false,
                ..Default::default()
            },
            Self::close,
            window,
            cx,
        )
        .into_any_element()
    }
}
