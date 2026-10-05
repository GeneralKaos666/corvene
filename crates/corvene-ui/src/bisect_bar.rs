//! Corvene `1212-bisect` (GitHub Desktop has no bisect): the bar under the
//! toolbar while the selected repository is bisecting, History's Bisect ▸
//! context menu and the marks on History's rows.
//!
//! The bar is laid out as GHD's banners (`banner.rs`,
//! `styles/ui/_banners.scss`: a 30 px strip with an icon and the message)
//! but is not one: it stays while the bisect does, under any banner. It
//! says what to do next (mark a bad or a good commit), which commit is being
//! tested with Good / Bad / Skip and the steps left, or the first bad
//! commit; Stop runs `git bisect reset`.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use corvene_core::bisect::{BisectMark, row_mark, selected_bisect, steps_left, term_label};
use corvene_core::{AppState, BisectPhase, BisectState, Dispatcher, RepositoryState};
use corvene_git::BisectVerdict;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::{IS_MAC, MenuItem, mac_or};
use crate::icons::{Octicon, octicon};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{ListRowA11y, small_button};

fn short(sha: &str) -> String {
    sha.chars().take(7).collect()
}

/// A term as it reads inside a sentence.
fn word(state: &BisectState, verdict: BisectVerdict) -> String {
    word_or_default(Some(state), verdict)
}

fn word_or_default(state: Option<&BisectState>, verdict: BisectVerdict) -> String {
    term_label(state, verdict).to_lowercase()
}

/// The message as text runs (`true` = bold).
#[doc(hidden)]
pub fn parts(
    state: &BisectState,
    head: Option<&str>,
    summary: Option<&str>,
) -> Vec<(String, bool)> {
    let bad = word(state, BisectVerdict::Bad);
    let good = word(state, BisectVerdict::Good);
    let commit = |sha: &str, summary: Option<&str>| -> Vec<(String, bool)> {
        let mut out = vec![(short(sha), true)];
        if let Some(summary) = summary.filter(|s| !s.is_empty()) {
            out.push((format!("\u{a0}{summary}"), false));
        }
        out
    };
    match state.phase() {
        BisectPhase::Waiting { has_bad, has_good } => {
            let wanted = match (has_bad, has_good) {
                (false, false) => format!("a {bad} and a {good} commit"),
                (true, _) => format!("a {good} commit"),
                (false, true) => format!("a {bad} commit"),
            };
            vec![(
                format!("Bisecting. Mark {wanted} in History to begin."),
                false,
            )]
        }
        BisectPhase::Testing { .. } => {
            let mut out = vec![("Testing\u{a0}".to_string(), false)];
            match head {
                Some(sha) => out.extend(commit(sha, summary)),
                None => out.push(("the checked out commit".to_string(), false)),
            }
            out
        }
        BisectPhase::Found { sha } => {
            let mut out = commit(&sha, summary);
            out.push((format!("\u{a0}is the first {bad} commit."), false));
            out
        }
        BisectPhase::OnlySkipped { count } => vec![(
            format!(
                "Only skipped commits are left. The first {bad} commit is one of these {count}."
            ),
            false,
        )],
        BisectPhase::Empty => vec![(
            format!(
                "No commits lie between the marks: the {good} commit must come before the {bad} one."
            ),
            false,
        )],
    }
}

/// "About N steps left", counting the commit being tested.
pub fn steps_text(state: &BisectState) -> String {
    match steps_left(state) {
        1 => "About 1 step left".to_string(),
        n => format!("About {n} steps left"),
    }
}

/// The bar for the selected repository, `None` while it is not bisecting
/// (or the flag is off).
pub fn bisect_bar(state: &AppState, cx: &App) -> Option<AnyElement> {
    let bisect = selected_bisect(state)?;
    let id = state.selected?;
    let rs = state.selected_state()?;
    let head = corvene_core::bisect::detached_head(rs).map(str::to_string);
    let phase = bisect.phase();
    let shown = match &phase {
        BisectPhase::Found { sha } => Some(sha.clone()),
        _ => head.clone(),
    };
    let summary = shown.as_deref().and_then(|sha| commit_summary(rs, sha));
    let parts = parts(&bisect, shown.as_deref(), summary.as_deref());
    let plain = parts
        .iter()
        .map(|(text, _)| text.as_str())
        .collect::<String>()
        .replace('\u{a0}', " ");
    let t = cx.ghd();
    let message = div()
        .flex()
        .flex_row()
        .items_center()
        .whitespace_nowrap()
        .children(parts.into_iter().map(|(text, bold)| {
            div()
                .when(bold, |d| d.font_weight(FontWeight::SEMIBOLD))
                .child(text)
        }));
    let mark = |verdict: BisectVerdict, id_str: &'static str| {
        small_button(id_str, term_label(Some(&bisect), verdict), cx)
            .on_click(move |_, _, cx| Dispatcher::bisect_mark(id, verdict, None, cx))
    };
    let testing = matches!(phase, BisectPhase::Testing { .. });
    let found = match &phase {
        BisectPhase::Found { sha } => Some(sha.clone()),
        _ => None,
    };
    Some(
        div()
            .id("bisect-bar")
            .a11y_live(plain)
            .w_full()
            .h(crate::banner::BANNER_HEIGHT())
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .pl(SPACING())
            .overflow_hidden()
            .bg(t.background)
            .border_b_1()
            .border_color(t.box_border)
            .text_size(FONT_SIZE())
            .text_color(t.text)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_row()
                    .items_center()
                    .child(octicon(Octicon::Search, t.text).mr(SPACING()))
                    .child(div().min_w_0().truncate().child(message))
                    .when_some(found, |d, sha| {
                        d.child(
                            crate::widgets::link_button("bisect-show-first-bad", "Show", cx)
                                .ml(SPACING_HALF())
                                .on_click(move |_, _, cx| {
                                    Dispatcher::show_section(
                                        id,
                                        corvene_core::Section::History,
                                        cx,
                                    );
                                    Dispatcher::select_commit(id, sha.clone(), cx);
                                }),
                        )
                    }),
            )
            .child(
                div()
                    .flex_none()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING_HALF())
                    .ml(SPACING())
                    .mr(SPACING())
                    .when(testing, |d| {
                        d.child(
                            div()
                                .mr(SPACING_HALF())
                                .text_color(t.text_secondary)
                                .whitespace_nowrap()
                                .child(steps_text(&bisect)),
                        )
                        .child(mark(BisectVerdict::Good, "bisect-good"))
                        .child(mark(BisectVerdict::Bad, "bisect-bad"))
                        .child(mark(BisectVerdict::Skip, "bisect-skip"))
                    })
                    .child(
                        small_button(
                            "bisect-stop",
                            if testing {
                                "Stop"
                            } else {
                                mac_or("Stop Bisecting", "Stop bisecting")
                            },
                            cx,
                        )
                        .when(testing, |d| d.ml(SPACING_HALF()))
                        .on_click(move |_, _, cx| Dispatcher::stop_bisect(id, cx)),
                    ),
            )
            .into_any_element(),
    )
}

fn commit_summary(rs: &RepositoryState, sha: &str) -> Option<String> {
    rs.commits
        .iter()
        .chain(rs.compare.commits.iter())
        .find(|c| c.sha == sha)
        .map(|c| corvene_core::text_tokens::with_emoji(&c.summary))
}

/// History's Bisect ▸ for one commit: mark it (starting a bisect when none
/// runs), and Stop Bisecting.
pub fn bisect_menu(id: u64, sha: &str, state: &AppState) -> Option<MenuItem> {
    if !state.flags.bool(corvene_core::flags::ids::BISECT) {
        return None;
    }
    let bisect = selected_bisect(state);
    let running = bisect.is_some();
    let item = |verdict: BisectVerdict| {
        // "Mark as Good" (macOS title case), "Mark as good"
        let label = match verdict {
            BisectVerdict::Skip => "Skip".to_string(),
            _ if IS_MAC => format!("Mark as {}", term_label(bisect.as_ref(), verdict)),
            _ => format!("Mark as {}", word_or_default(bisect.as_ref(), verdict)),
        };
        let sha = sha.to_string();
        MenuItem::new(label, move |_, cx| {
            if running {
                Dispatcher::bisect_mark(id, verdict, Some(sha.clone()), cx)
            } else {
                Dispatcher::request_start_bisect(id, Some((verdict, sha.clone())), cx)
            }
        })
    };
    let mut items = vec![item(BisectVerdict::Bad), item(BisectVerdict::Good)];
    if running {
        items.extend([
            item(BisectVerdict::Skip),
            MenuItem::separator(),
            MenuItem::new(mac_or("Stop Bisecting", "Stop bisecting"), move |_, cx| {
                Dispatcher::stop_bisect(id, cx)
            }),
        ]);
    }
    Some(MenuItem::submenu("Bisect", items))
}

/// What History's rows show of the bisect: each row's mark, and whether the
/// commit is still in question.
pub struct BisectRows {
    state: BisectState,
    head: Option<String>,
    range: HashSet<String>,
}

thread_local! {
    /// The last [`BisectRows`] built: History renders on every state change,
    /// and the range can hold many commits.
    static ROWS: RefCell<Option<Rc<BisectRows>>> = const { RefCell::new(None) };
}

impl BisectRows {
    pub fn of(s: &AppState, rs: &RepositoryState) -> Option<Rc<Self>> {
        let state = rs.status.as_ref()?.bisect.as_ref()?;
        if !s.flags.bool(corvene_core::flags::ids::BISECT) {
            return None;
        }
        let head = corvene_core::bisect::detached_head(rs);
        ROWS.with_borrow_mut(|cached| {
            if let Some(rows) = cached.as_ref()
                && rows.state == *state
                && rows.head.as_deref() == head
            {
                return Some(rows.clone());
            }
            let rows = Rc::new(Self {
                state: state.clone(),
                head: head.map(str::to_string),
                range: state.candidates.iter().cloned().collect(),
            });
            *cached = Some(rows.clone());
            Some(rows)
        })
    }

    pub fn mark(&self, sha: &str) -> Option<BisectMark> {
        row_mark(&self.state, self.head.as_deref(), sha)
    }

    /// The commit may still be the first bad one (marked bad, skipped or
    /// not tested yet).
    pub fn in_range(&self, sha: &str) -> bool {
        self.range.contains(sha)
    }

    /// The pill a row with `mark` shows; `selected` draws it in the
    /// selected row's text colour.
    pub fn pill(&self, mark: BisectMark, selected: Option<Hsla>, cx: &App) -> Div {
        let t = cx.ghd();
        let bad = word(&self.state, BisectVerdict::Bad);
        let (label, color) = match mark {
            BisectMark::Bad => (bad, t.color_deleted),
            BisectMark::Good => (word(&self.state, BisectVerdict::Good), t.color_new),
            BisectMark::Skipped => ("skipped".to_string(), t.text_secondary),
            BisectMark::Testing => ("testing".to_string(), t.link),
            BisectMark::FirstBad => (format!("first {bad}"), t.color_deleted),
        };
        let color = selected.unwrap_or(color);
        div()
            .flex_none()
            .ml(SPACING())
            .h(zpx(16.))
            .px(SPACING_HALF())
            .flex()
            .items_center()
            .rounded(BORDER_RADIUS())
            .border_1()
            .border_color(color)
            .text_color(color)
            .text_size(FONT_SIZE_SM())
            .line_height(zpx(14.))
            .when(mark == BisectMark::FirstBad, |d| {
                d.font_weight(FontWeight::SEMIBOLD)
            })
            .child(label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> BisectState {
        BisectState {
            start: "main".into(),
            term_bad: "bad".into(),
            term_good: "good".into(),
            bad: Some("d".repeat(40)),
            good: vec!["a".repeat(40)],
            skipped: Vec::new(),
            candidates: vec!["d".repeat(40), "c".repeat(40), "b".repeat(40)],
        }
    }

    fn text(parts: Vec<(String, bool)>) -> String {
        parts
            .into_iter()
            .map(|(t, _)| t)
            .collect::<String>()
            .replace('\u{a0}', " ")
    }

    #[::core::prelude::v1::test]
    fn messages() {
        let s = state();
        assert_eq!(
            text(parts(&s, Some(&"c".repeat(40)), Some("Fix it"))),
            "Testing ccccccc Fix it"
        );
        assert_eq!(steps_text(&s), "About 2 steps left");
        let waiting = BisectState {
            good: Vec::new(),
            candidates: Vec::new(),
            ..state()
        };
        assert_eq!(
            text(parts(&waiting, None, None)),
            "Bisecting. Mark a good commit in History to begin."
        );
        let found = BisectState {
            candidates: vec!["d".repeat(40)],
            ..state()
        };
        assert_eq!(
            text(parts(&found, Some(&"d".repeat(40)), Some("Break it"))),
            "ddddddd Break it is the first bad commit."
        );
        let custom = BisectState {
            term_bad: "fixed".into(),
            term_good: "broken".into(),
            bad: None,
            good: Vec::new(),
            candidates: Vec::new(),
            ..state()
        };
        assert_eq!(
            text(parts(&custom, None, None)),
            "Bisecting. Mark a fixed and a broken commit in History to begin."
        );
    }
}
