//! GHD `CopyButton` (`ui/copy-button.tsx`): an icon button that writes its
//! content to the clipboard, then for 2 s shows a check icon, a "Copied!"
//! tooltip and "Copied!" in a polite live region, and resets. Used by the
//! selected commit's SHA (`selected_commit.rs`) and the push protection
//! locations (`dialogs/push_protection.rs`).
//!
//! [`CopyButton`] is the button's state; the clock is passed in (`now`) so
//! it is driven like GHD's `sleep(2000)`. Deviation: the live region says
//! "Copied!" from the click on (GHD's `AriaLiveContainer` debounces its
//! message by 1 s).

use std::time::{Duration, Instant};

use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::icons::{Octicon, octicon};
use crate::widgets::{GhdTooltip, IconButtonA11y, ListRowA11y};

/// How long the copied state shows (`await sleep(2000)`).
pub const COPIED_DURATION: Duration = Duration::from_secs(2);

/// The tooltip and announcement while copied.
pub const COPIED_MESSAGE: &str = "Copied!";

/// GHD `CopyButton`'s props and `showCopied` state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CopyButton {
    copy_content: String,
    aria_label: String,
    /// When the button was last clicked.
    copied_at: Option<Instant>,
}

impl CopyButton {
    /// `<CopyButton copyContent={copy_content} ariaLabel={aria_label} />`
    pub fn new(copy_content: impl Into<String>, aria_label: impl Into<String>) -> Self {
        Self {
            copy_content: copy_content.into(),
            aria_label: aria_label.into(),
            copied_at: None,
        }
    }

    /// What a click copies.
    pub fn copy_content(&self) -> &str {
        &self.copy_content
    }

    /// The button's accessible name (`ariaLabel`, kept while copied).
    pub fn accessible_name(&self) -> &str {
        &self.aria_label
    }

    /// `onCopy` at `now`: the text to write to the clipboard; the copied
    /// state shows for [`COPIED_DURATION`].
    pub fn click(&mut self, now: Instant) -> String {
        self.copied_at = Some(now);
        self.copy_content.clone()
    }

    /// `showCopied` at `now`.
    pub fn show_copied(&self, now: Instant) -> bool {
        self.copied_at
            .is_some_and(|at| now.saturating_duration_since(at) < COPIED_DURATION)
    }

    /// `renderSymbol`: the check while copied, else the copy icon.
    pub fn icon(&self, now: Instant) -> Octicon {
        if self.show_copied(now) {
            Octicon::Check
        } else {
            Octicon::Copy
        }
    }

    /// The tooltip: "Copied!" while copied, else the accessible name.
    pub fn tooltip(&self, now: Instant) -> &str {
        if self.show_copied(now) {
            COPIED_MESSAGE
        } else {
            &self.aria_label
        }
    }

    /// The text of the button's live region (empty when nothing is
    /// announced).
    pub fn live_region_text(&self, now: Instant) -> String {
        if self.show_copied(now) {
            COPIED_MESSAGE.to_string()
        } else {
            String::new()
        }
    }
}

/// Draws `button` as a `size` icon in `color`: its icon, tooltip, accessible
/// name and live region. `on_click` copies (through [`CopyButton::click`])
/// and re-renders once [`COPIED_DURATION`] has passed.
pub fn copy_button_element(
    id: impl Into<ElementId>,
    button: &CopyButton,
    size: Pixels,
    color: Hsla,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let now = Instant::now();
    let live = button.live_region_text(now);
    div()
        .id(id)
        .relative()
        .a11y_button(button.accessible_name().to_string())
        .ghd_tooltip(button.tooltip(now).to_string())
        .cursor_pointer()
        .on_click(on_click)
        .child(octicon(button.icon(now), color).size(size))
        .child(
            // `AriaLiveContainer`
            div()
                .id("copied")
                .absolute()
                .size_0()
                .overflow_hidden()
                .when(!live.is_empty(), |d| d.a11y_live(live)),
        )
}
