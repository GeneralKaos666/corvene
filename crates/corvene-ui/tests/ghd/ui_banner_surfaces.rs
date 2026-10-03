//! Port of GitHub Desktop's `app/test/unit/ui/banner-surfaces-test.tsx`.
//!
//! GitHub Desktop renders each banner as its own React component
//! (`ui/banners/*.tsx`); Corvene has one `corvene_core::Banner` enum, drawn
//! by `corvene_ui::banner::banner_bar` from the text runs of
//! `corvene_ui::banner::parts(banner)` (`true` marks a bold run, GitHub
//! Desktop's `<strong>`). The ported cases read those runs:
//!
//! - `BranchAlreadyUpToDate { ourBranch, theirBranch }` is
//!   `Banner::BranchAlreadyUpToDate { our_branch, their_branch }`,
//! - `CherryPickUndone { countCherryPicked, targetBranchName }` is
//!   `Banner::CherryPickUndone { count, target_branch }`.
//!
//! The rendered container's `textContent` is the runs joined
//! ([`text_content`]); Testing Library's `getByText(text)` finds the one
//! element whose own text, normalised, is `text`, which here is the one run
//! that is ([`get_by_text`]).
//!
//! The generic `Banner` (`ui/banners/banner.tsx`) focuses its first link (or
//! button) 200 ms after it mounts and only starts its dismissal timeout when
//! focus leaves it. Corvene's `banner::BannerView` does the same with
//! `corvene_core::banner_focus::BannerFocus`, whose timers take the clock as
//! `now`; [`BannerView`] drives it like GitHub Desktop's fake timers, for a
//! banner whose one link is its first suitable element, with the close
//! button `banner::close_button_label` names.
//!
//! `SuccessBanner`'s case (arbitrary children and its Undo click wiring) is
//! DOM behaviour and is skipped in `tools/ghd-tests/skips/ui1.tsv`.

use std::time::{Duration, Instant};

use corvene_core::Banner;
use corvene_core::banner_focus::{BannerFocus, BannerFocusEvent};
use corvene_ui::banner::{close_button_label, parts};

/// `document.activeElement` in the focus case.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)] // `Button`: the first suitable element of a banner without links
enum ActiveElement {
    /// `document.body`: nothing in the banner has focus.
    Body,
    /// A link in the banner, by its accessible name.
    Link(String),
    /// A button in the banner, by its accessible name.
    Button(String),
}

/// GitHub Desktop's generic `Banner` (`ui/banners/banner.tsx`) rendered
/// with one link, under fake timers (`enableTestTimers(['setTimeout'])`),
/// with a counting `onDismissed`.
struct BannerView {
    focus: BannerFocus,
    /// The fake clock.
    now: Instant,
    /// The link's accessible name.
    link: String,
    active: ActiveElement,
    dismissed: usize,
}

impl BannerView {
    /// `render(<Banner id={id} timeout={timeout_ms} onDismissed={…}><a
    /// href="…">{link}</a></Banner>)`
    fn render(_id: &str, timeout_ms: u64, link: &str) -> Self {
        let now = Instant::now();
        Self {
            focus: BannerFocus::mount(now, Some(Duration::from_millis(timeout_ms)), true),
            now,
            link: link.to_string(),
            active: ActiveElement::Body,
            dismissed: 0,
        }
    }

    /// The accessible names of the banner's buttons.
    fn button_names(&self) -> Vec<String> {
        close_button_label(true)
            .into_iter()
            .map(str::to_string)
            .collect()
    }

    /// `advanceTimersBy(ms)`
    fn advance_timers_by(&mut self, ms: u64) {
        self.now += Duration::from_millis(ms);
        for event in self.focus.advance(self.now) {
            match event {
                // the first `<a>` is the first suitable element
                BannerFocusEvent::FocusFirstElement => {
                    self.active = ActiveElement::Link(self.link.clone());
                    self.focus.focus_in();
                }
                BannerFocusEvent::Dismiss => self.dismissed += 1,
            }
        }
    }

    fn active_element(&self) -> ActiveElement {
        self.active.clone()
    }

    /// `fireEvent.focusOut(<the focused element>, { relatedTarget:
    /// document.body })`
    fn focus_out_to_body(&mut self) {
        self.focus.focus_out(self.now, false);
        self.active = ActiveElement::Body;
    }

    /// How often `onDismissed` was called.
    fn dismissed(&self) -> usize {
        self.dismissed
    }
}

/// The rendered banner's `container.textContent`: every run, in order.
fn text_content(banner: &Banner) -> String {
    parts(banner).into_iter().map(|(text, _)| text).collect()
}

/// Testing Library's default normaliser: trim and collapse whitespace
/// (JavaScript's `\s`, which includes the no-break space).
fn normalize(text: &str) -> String {
    text.split(|c: char| c.is_whitespace())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// `screen.getByText(text)`: exactly one run reads `text` once normalised
/// (`getByText` throws on no match and on several).
fn get_by_text(banner: &Banner, text: &str) -> String {
    let matches: Vec<String> = parts(banner)
        .into_iter()
        .map(|(run, _)| run)
        .filter(|run| normalize(run) == text)
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one element with the text {text:?} in {:?}",
        parts(banner)
    );
    matches[0].clone()
}

// GHD: unit/ui/banner-surfaces-test.tsx › banner surfaces › focuses the first suitable banner element and auto-dismisses on focus out
#[test]
fn focuses_the_first_suitable_banner_element_and_auto_dismisses_on_focus_out() {
    let mut view = BannerView::render("test-banner", 500, "Learn more");

    // `#test-banner.banner` is the rendered view; `getByRole('button', {
    // name: 'Dismiss this message' })` finds exactly one button
    let dismiss_buttons = view
        .button_names()
        .into_iter()
        .filter(|name| name == "Dismiss this message")
        .count();
    assert_eq!(dismiss_buttons, 1);

    view.advance_timers_by(200);

    assert_eq!(
        view.active_element(),
        ActiveElement::Link("Learn more".to_string())
    );

    view.focus_out_to_body();

    view.advance_timers_by(500);

    assert_eq!(view.dismissed(), 1);
}

// GHD: unit/ui/banner-surfaces-test.tsx › banner surfaces › renders branch up-to-date banner messages with and without the compared branch
#[test]
fn renders_branch_up_to_date_banner_messages_with_and_without_the_compared_branch() {
    let view = Banner::BranchAlreadyUpToDate {
        our_branch: "main".to_string(),
        their_branch: Some("origin/main".to_string()),
    };

    get_by_text(&view, "main");
    get_by_text(&view, "origin/main");
    assert!(text_content(&view).contains("is already up to date with"));

    // `view.rerender(<BranchAlreadyUpToDate ourBranch="release" … />)`
    let view = Banner::BranchAlreadyUpToDate {
        our_branch: "release".to_string(),
        their_branch: None,
    };

    get_by_text(&view, "release");
    assert!(text_content(&view).contains("is already up to date"));
}

// GHD: unit/ui/banner-surfaces-test.tsx › banner surfaces › renders cherry-pick undone messages with singular and plural commit copy
#[test]
fn renders_cherry_pick_undone_messages_with_singular_and_plural_commit_copy() {
    let view = Banner::CherryPickUndone {
        count: 1,
        target_branch: "main".to_string(),
    };

    assert!(
        text_content(&view)
            .contains("Cherry-pick undone. Successfully removed the 1 copied commit from")
    );
    get_by_text(&view, "main");

    // `view.rerender(<CherryPickUndone countCherryPicked={3} … />)`
    let view = Banner::CherryPickUndone {
        count: 3,
        target_branch: "release".to_string(),
    };

    assert!(
        text_content(&view)
            .contains("Cherry-pick undone. Successfully removed the 3 copied commits from")
    );
    get_by_text(&view, "release");
}
