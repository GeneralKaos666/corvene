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
//! focus leaves it. Corvene's banners (`banner::banner_bar`) never take
//! focus, and `Dispatcher::set_banner` (`corvene-core` `mco.rs`) removes them
//! `Banner::timeout()` (5 or 15 s) after they appear, in a GPUI task. There
//! is no Corvene state for a banner's focus and dismissal, so [`BannerView`]
//! is a stand-in driven like GitHub Desktop's fake timers; replace it with
//! the Corvene type once there is one and remove the `#[ignore]`.
//!
//! `SuccessBanner`'s case (arbitrary children and its Undo click wiring) is
//! DOM behaviour and is skipped in `tools/ghd-tests/skips/ui1.tsv`.

use corvene_core::Banner;
use corvene_ui::banner::parts;

/// `document.activeElement` in the focus case.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)] // `Body`, `Button`: built by the real banner focus state
enum ActiveElement {
    /// `document.body`: nothing in the banner has focus.
    Body,
    /// A link in the banner, by its accessible name.
    Link(String),
    /// A button in the banner, by its accessible name.
    Button(String),
}

/// Stand-in for GitHub Desktop's generic `Banner` (`ui/banners/banner.tsx`:
/// `componentDidMount`'s focus timeout, `onFocusIn` / `onFocusOut`'s
/// dismissal timeout) under fake timers (`enableTestTimers(['setTimeout'])`),
/// with a counting `onDismissed`.
#[allow(dead_code)] // built by the real banner focus state
struct BannerView;

impl BannerView {
    /// `render(<Banner id={id} timeout={timeout_ms} onDismissed={…}><a
    /// href="…">{link}</a></Banner>)`
    fn render(_id: &str, _timeout_ms: u64, _link: &str) -> Self {
        unimplemented!(
            "Corvene has no banner focus/dismissal state: banners take no focus and \
             Dispatcher::set_banner times them out from when they appear"
        )
    }

    /// The accessible names of the banner's buttons.
    fn button_names(&self) -> Vec<String> {
        unimplemented!()
    }

    /// `advanceTimersBy(ms)`
    fn advance_timers_by(&mut self, _ms: u64) {
        unimplemented!()
    }

    fn active_element(&self) -> ActiveElement {
        unimplemented!()
    }

    /// `fireEvent.focusOut(<the focused element>, { relatedTarget:
    /// document.body })`
    fn focus_out_to_body(&mut self) {
        unimplemented!()
    }

    /// How often `onDismissed` was called.
    fn dismissed(&self) -> usize {
        unimplemented!()
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
#[ignore = "ghd: missing: no banner focus/dismissal state (ui/banners/banner.tsx componentDidMount, onFocusOut); Corvene banners never take focus and Dispatcher::set_banner (core mco.rs) drops them 5/15 s after they appear, GHD focuses the first link after 200 ms and starts the timeout on focus out"]
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
