//! Port of GitHub Desktop's `app/test/unit/ui/copy-button-test.tsx`.
//!
//! GitHub Desktop's `CopyButton` (`ui/copy-button.tsx`, the copy buttons
//! next to a commit's SHA and in submodule diffs) writes `copyContent` to
//! the clipboard on click, then for 2 s shows a check icon, a "Copied!"
//! tooltip and "Copied!" in an `AriaLiveContainer`, and resets. Corvene's
//! counterpart is `corvene_ui::copy_button::CopyButton`, the state of the
//! copy-sha button of the selected commit (`selected_commit.rs`, "Copy the
//! full SHA") and of the push protection locations, which takes the clock
//! as `now`. [`CopyButton`] drives it like GitHub Desktop's fake timers
//! (`advanceTimersBy`).

use std::time::{Duration, Instant};

/// GitHub Desktop's `CopyButton` under fake timers.
struct CopyButton {
    button: corvene_ui::copy_button::CopyButton,
    /// The fake clock.
    now: Instant,
}

impl CopyButton {
    /// `<CopyButton copyContent={copy_content} ariaLabel={aria_label} />`
    fn new(copy_content: &str, aria_label: &str) -> Self {
        Self {
            button: corvene_ui::copy_button::CopyButton::new(copy_content, aria_label),
            now: Instant::now(),
        }
    }

    /// The button's accessible name.
    fn accessible_name(&self) -> String {
        self.button.accessible_name().to_string()
    }

    /// `fireEvent.click(button)`, with the clipboard writes captured
    /// (GitHub Desktop's `captureClipboardWrites`).
    fn click(&mut self, clipboard_writes: &mut Vec<String>) {
        clipboard_writes.push(self.button.click(self.now));
    }

    /// `advanceTimersBy(ms)`
    fn advance_timers_by(&mut self, ms: u64) {
        self.now += Duration::from_millis(ms);
    }

    /// The text of the button's aria-live region (empty when nothing is
    /// announced).
    fn live_region_text(&self) -> String {
        self.button.live_region_text(self.now)
    }
}

// GHD: unit/ui/copy-button-test.tsx › CopyButton › copies content and announces the copied state before resetting
#[test]
fn copies_content_and_announces_the_copied_state_before_resetting() {
    let mut clipboard_writes = Vec::new();
    let mut button = CopyButton::new("refs/heads/main", "Copy branch name");
    // `screen.getByRole('button', { name: 'Copy branch name' })`
    assert_eq!(button.accessible_name(), "Copy branch name");

    button.click(&mut clipboard_writes);

    assert_eq!(clipboard_writes, ["refs/heads/main"]);

    button.advance_timers_by(1000);

    assert!(button.live_region_text().starts_with("Copied!"));

    button.advance_timers_by(2000);

    assert!(!button.live_region_text().starts_with("Copied!"));
}
