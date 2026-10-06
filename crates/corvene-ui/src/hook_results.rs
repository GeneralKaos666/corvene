//! Corvene (flag `1114-hook-results`, desktop/desktop#22476): the outcome of
//! the hooks that cannot stop an operation (post-merge, post-rewrite,
//! post-commit, post-checkout), under the Changes and History tabs. GHD
//! drops it (`hooks-proxy.ts` `ignoredOnFailureHooks`); the list shows when
//! a hook failed or printed something, one row per hook with its status and
//! time, and a row opens the hook's output in GHD's terminal
//! ([`crate::terminal`]) sized to the sidebar.

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

use corvene_core::hooks::HookReport;
use corvene_core::{AppState, Dispatcher};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::icons::{Octicon, octicon};
use crate::terminal::Terminal;
use crate::theme::sizes::*;
use crate::theme::{ActiveGhdTheme, c, primer};
use crate::widgets::IconButtonA11y;

/// The most output rows an open hook shows before it scrolls.
const MAX_OUTPUT_ROWS: usize = 12;

pub struct HookResults {
    state: Entity<AppState>,
    /// The sidebar's width, kept by the workspace.
    width: Rc<Cell<Pixels>>,
    /// The report shown last: (repository, run, hooks).
    shown: Option<(u64, u64, usize)>,
    /// Open rows of the shown report.
    open: Vec<usize>,
    /// Output terminals by row, with the columns they were made for.
    terminals: HashMap<usize, (usize, Entity<Terminal>)>,
}

impl HookResults {
    pub fn new(state: Entity<AppState>, width: Rc<Cell<Pixels>>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |this, state, cx| {
            if this.key(state.read(cx)) != this.shown {
                cx.notify();
            }
        })
        .detach();
        Self {
            state,
            width,
            shown: None,
            open: Vec::new(),
            terminals: HashMap::new(),
        }
    }

    fn report(state: &AppState) -> Option<(u64, &HookReport)> {
        if !state.flags.bool(corvene_core::flags::ids::HOOK_RESULTS) {
            return None;
        }
        let id = state.selected?;
        let report = state.repo_states.get(&id)?.hook_report.as_ref()?;
        report.worth_showing().then_some((id, report))
    }

    fn key(&self, state: &AppState) -> Option<(u64, u64, usize)> {
        Self::report(state).map(|(id, r)| (id, r.run, r.hooks.len()))
    }

    fn toggle(&mut self, ix: usize, cx: &mut Context<Self>) {
        if let Some(pos) = self.open.iter().position(|i| *i == ix) {
            self.open.remove(pos);
        } else {
            self.open.push(ix);
        }
        cx.notify();
    }

    fn terminal(&mut self, ix: usize, output: &[u8], cx: &mut Context<Self>) -> Entity<Terminal> {
        // the row's padding on both sides
        let cols = Terminal::cols_for_width(unzoom(self.width.get()) - 2. * 10.);
        match self.terminals.get(&ix) {
            Some((made_for, terminal)) if *made_for == cols => terminal.clone(),
            _ => {
                let terminal = cx.new(|_| Terminal::fitted(cols, MAX_OUTPUT_ROWS, output));
                self.terminals.insert(ix, (cols, terminal.clone()));
                terminal
            }
        }
    }
}

/// "0.4s", "12s", "2m 5s".
fn duration_label(d: std::time::Duration) -> String {
    let secs = d.as_secs_f64();
    if secs < 10. {
        format!("{secs:.1}s")
    } else if secs < 60. {
        format!("{}s", secs.round() as u64)
    } else {
        let secs = secs.round() as u64;
        format!("{}m {}s", secs / 60, secs % 60)
    }
}

impl Render for HookResults {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let key = self.key(self.state.read(cx));
        if key.map(|(id, run, _)| (id, run)) != self.shown.map(|(id, run, _)| (id, run)) {
            self.open.clear();
            self.terminals.clear();
        }
        self.shown = key;
        let Some((id, report)) = Self::report(self.state.read(cx)).map(|(id, r)| (id, r.clone()))
        else {
            return div().into_any_element();
        };
        let open = self.open.clone();
        let open_terminals: HashMap<usize, Entity<Terminal>> = report
            .hooks
            .iter()
            .enumerate()
            .filter(|(ix, _)| open.contains(ix))
            .map(|(ix, hook)| {
                let mut output = hook.output.clone();
                output.extend_from_slice(hook.termination.as_bytes());
                output.push(b'\n');
                (ix, self.terminal(ix, &output, cx))
            })
            .collect();
        let t = cx.ghd();
        let hover_bg = t.box_selected_background;
        let count = report.hooks.len();
        let title = format!(
            "{} · {count} {} ran",
            report.operation,
            if count == 1 { "hook" } else { "hooks" }
        );
        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(SPACING_HALF())
            .pl(SPACING())
            .pr(SPACING_HALF())
            .py(SPACING_HALF())
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title),
            )
            .child(
                div()
                    .id("hook-results-dismiss")
                    .flex_none()
                    .p(zpx(2.))
                    .rounded(BORDER_RADIUS())
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover_bg))
                    .icon_button_label("Dismiss")
                    .child(octicon(Octicon::X, t.text_secondary))
                    .on_click(move |_, _, cx| Dispatcher::dismiss_hook_report(id, cx)),
            );
        let mut rows: Vec<AnyElement> = Vec::new();
        for (ix, hook) in report.hooks.iter().enumerate() {
            let open = self.open.contains(&ix);
            let icon = if hook.failed() {
                octicon(Octicon::XCircleFill, c(primer::RED_500))
            } else {
                octicon(Octicon::CheckCircleFill, c(primer::GREEN_500))
            };
            rows.push(
                div()
                    .id(SharedString::from(format!("hook-result-{ix}")))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING_HALF())
                    .px(SPACING())
                    .py(zpx(3.))
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover_bg))
                    .child(icon.flex_none())
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(hook.hook_name.clone()),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_size(FONT_SIZE_SM())
                            .text_color(t.text_secondary)
                            .child(duration_label(hook.duration)),
                    )
                    .child(
                        octicon(
                            if open {
                                Octicon::ChevronDown
                            } else {
                                Octicon::ChevronRight
                            },
                            t.text_secondary,
                        )
                        .flex_none(),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle(ix, cx)))
                    .into_any_element(),
            );
            if let Some(terminal) = open_terminals.get(&ix).cloned() {
                rows.push(
                    div()
                        .id(SharedString::from(format!("hook-result-output-{ix}")))
                        .px(SPACING())
                        .pb(SPACING_HALF())
                        .child(terminal)
                        .into_any_element(),
                );
            }
        }
        div()
            .id("hook-results")
            .flex_none()
            .flex()
            .flex_col()
            .max_h(zpx(320.))
            .overflow_y_scroll()
            .border_b_1()
            .border_color(t.box_border)
            .bg(t.box_alt_background)
            .text_size(FONT_SIZE())
            .child(header)
            .children(rows)
            .into_any_element()
    }
}
