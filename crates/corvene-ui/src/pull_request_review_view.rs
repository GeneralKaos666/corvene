//! Corvene `348-pull-request-review`: the content area while a pull
//! request is under review: the overview pane
//! ([`crate::pull_request_overview`]) for the sidebar's Overview row, else
//! the selected file's diff with its header and the review threads in it
//! ([`crate::diff_view::DiffSource::Review`]). GitHub Desktop has neither.

use corvene_core::AppState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::diff_view::{DiffSource, DiffView, diff_header};
use crate::pull_request_overview::PullRequestOverviewView;
use crate::theme::ActiveGhdTheme;

pub struct PullRequestReviewView {
    state: Entity<AppState>,
    overview: Entity<PullRequestOverviewView>,
    diff: Entity<DiffView>,
}

impl PullRequestReviewView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        let overview = cx.new(|cx| PullRequestOverviewView::new(state.clone(), cx));
        let diff = cx.new(|cx| DiffView::new(state.clone(), DiffSource::Review, cx));
        Self {
            state,
            overview,
            diff,
        }
    }

    /// The diff's keyboard focus (⌘3 and the arrow keys between panes).
    pub fn diff(&self) -> &Entity<DiffView> {
        &self.diff
    }
}

impl Render for PullRequestReviewView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd().clone();
        let selected = {
            let s = self.state.read(cx);
            s.selected.and_then(|id| {
                let review = crate::review_threads::review_state_of(s, id)?;
                let path = review.selected_file()?;
                let change = review
                    .changeset
                    .as_ref()
                    .and_then(|c| c.files.iter().find(|f| f.path == path))?;
                Some((
                    change.path.clone(),
                    change.status.kind,
                    change.old_path.clone(),
                ))
            })
        };
        match selected {
            None => self.overview.clone().into_any_element(),
            Some((path, kind, old_path)) => div()
                .id("pr-review-file")
                .size_full()
                .flex()
                .flex_col()
                .min_h_0()
                .bg(t.background)
                .child(diff_header(
                    &path,
                    kind,
                    old_path.as_deref(),
                    None,
                    &self.diff,
                    cx,
                ))
                .child(DiffView::embed(&self.diff))
                .into_any_element(),
        }
    }
}
