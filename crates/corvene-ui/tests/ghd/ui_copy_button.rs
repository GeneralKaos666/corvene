//! Port of GitHub Desktop's `app/test/unit/ui/copy-button-test.tsx`.
//!
//! GitHub Desktop's `CopyButton` (`ui/copy-button.tsx`, the copy buttons
//! next to a commit's SHA and in submodule diffs) writes `copyContent` to
//! the clipboard on click, then for 2 s shows a check icon, a "Copied!"
//! tooltip and "Copied!" in an `AriaLiveContainer`, and resets. Corvene's
//! counterpart is the copy-sha button of the selected commit
//! (`selected_commit.rs`, "Copy the full SHA"): a GPUI click handler that
//! writes the clipboard and shows no copied state. There is no Corvene type
//! for the button's state, so [`CopyButton`] is a stand-in driven like GitHub
//! Desktop's fake timers (`advanceTimersBy`); replace it with the Corvene
//! type once there is one and remove the `#[ignore]`.

/// Stand-in for GitHub Desktop's `CopyButton` under fake timers.
#[allow(dead_code)] // built by the real CopyButton state
struct CopyButton;

impl CopyButton {
    /// `<CopyButton copyContent={copy_content} ariaLabel={aria_label} />`
    fn new(_copy_content: &str, _aria_label: &str) -> Self {
        unimplemented!(
            "Corvene has no CopyButton state (selected_commit.rs copy-sha only writes the clipboard)"
        )
    }

    /// The button's accessible name.
    fn accessible_name(&self) -> String {
        unimplemented!()
    }

    /// `fireEvent.click(button)`, with the clipboard writes captured
    /// (GitHub Desktop's `captureClipboardWrites`).
    fn click(&mut self, _clipboard_writes: &mut Vec<String>) {
        unimplemented!()
    }

    /// `advanceTimersBy(ms)`
    fn advance_timers_by(&mut self, _ms: u64) {
        unimplemented!()
    }

    /// The text of the button's aria-live region (empty when nothing is
    /// announced).
    fn live_region_text(&self) -> String {
        unimplemented!()
    }
}

// GHD: unit/ui/copy-button-test.tsx › CopyButton › copies content and announces the copied state before resetting
#[test]
#[ignore = "ghd: missing: no CopyButton copied state (ui/copy-button.tsx); Corvene's copy-sha button (selected_commit.rs) writes the clipboard but never shows GHD's 2 s 'Copied!' tooltip, check icon and announcement"]
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
