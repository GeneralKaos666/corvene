//! Port of GitHub Desktop's `app/test/unit/ui/component-primitives-test.tsx`.
//!
//! Only the changed files badge computes anything: GitHub Desktop's
//! `FilesChangedBadge` (`ui/changes/files-changed-badge.tsx`), the count on
//! the Changes tab, shows `formatCompactNumber(filesChangedCount)`
//! (`lib/format-number.ts`; `enableFormattingPreferences()` is always on in
//! 3.6.6), so 3000 reads "3k". Corvene's Changes tab count
//! (`workspace.rs`, drawn by `widgets::counter_text` in `tab_bar.rs`) is
//! `corvene_ui::format::files_changed_badge(count)`.
//!
//! The other cases of the file render wrappers (`Loading`, `HorizontalRule`,
//! `DialogFooter`, `Ref`, `Toolbar`) and are skipped in
//! `tools/ghd-tests/skips/ui1.tsv`.

use corvene_ui::format::files_changed_badge;

// GHD: unit/ui/component-primitives-test.tsx › component primitives › renders the changed files badge count and caps large values
#[test]
fn renders_the_changed_files_badge_count_and_caps_large_values() {
    let badges: Vec<String> = [12, 301, 3000]
        .into_iter()
        .map(files_changed_badge)
        .collect();

    assert_eq!(badges, ["12", "301", "3k"]);
}
