//! Port of GitHub Desktop's `app/test/unit/ui/component-primitives-test.tsx`.
//!
//! Only the changed files badge computes anything: GitHub Desktop's
//! `FilesChangedBadge` (`ui/changes/files-changed-badge.tsx`), the count on
//! the Changes tab, shows `formatCompactNumber(filesChangedCount)`
//! (`lib/format-number.ts`; `enableFormattingPreferences()` is always on in
//! 3.6.6), so 3000 reads "3k". Corvene's Changes tab count is
//! `corvene_ui::widgets::counter(count, cx)` (`tab_bar.rs`), a GPUI element
//! that renders `count.to_string()`; there is no function returning the
//! badge text, so [`files_changed_badge`] is a stand-in. Replace it with the
//! Corvene function once there is one and remove the `#[ignore]`.
//!
//! The other cases of the file render wrappers (`Loading`, `HorizontalRule`,
//! `DialogFooter`, `Ref`, `Toolbar`) and are skipped in
//! `tools/ghd-tests/skips/ui1.tsv`.

/// Stand-in for the text GitHub Desktop's `FilesChangedBadge` renders in its
/// `.counter` (`formatCompactNumber(filesChangedCount)`).
fn files_changed_badge(_files_changed_count: usize) -> String {
    unimplemented!(
        "Corvene has no FilesChangedBadge text: widgets::counter renders count.to_string()"
    )
}

// GHD: unit/ui/component-primitives-test.tsx › component primitives › renders the changed files badge count and caps large values
#[test]
#[ignore = "ghd: missing: no FilesChangedBadge / formatCompactNumber (ui/changes/files-changed-badge.tsx, lib/format-number.ts); widgets::counter renders count.to_string(), so 3000 shows '3000' where GHD shows '3k'"]
fn renders_the_changed_files_badge_count_and_caps_large_values() {
    let badges: Vec<String> = [12, 301, 3000]
        .into_iter()
        .map(files_changed_badge)
        .collect();

    assert_eq!(badges, ["12", "301", "3k"]);
}
