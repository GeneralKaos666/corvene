//! Port of GitHub Desktop's `app/test/unit/ui/branch-list-item-test.tsx`.
//!
//! GitHub Desktop's `BranchListItem` (`ui/branches/branch-list-item.tsx`)
//! is Corvene's `BranchFoldout::row` (`corvene_ui::branch_list`), a GPUI
//! element that draws the branch name and
//! `corvene_ui::relative_time::relative_at` of the branch tip's time (GitHub
//! Desktop's `RelativeTime` with `onlyRelative`, as long as "Prefer absolute
//! dates" is off, its default and the default of `corvene_ui::format`).
//!
//! The case is ported twice:
//!
//! - [`renders_the_branch_name_and_relative_author_date`] calls `relative`
//!   on its own. GitHub Desktop pins the clock (`enableTestTimers(['Date',
//!   …], now)`) and renders an author date 30 s before it; `relative` reads
//!   the system clock, so the date here is 30 s before the system clock: the
//!   same distance, which is all `relative` uses below a minute.
//! - [`renders_the_branch_name_and_relative_author_date_in_the_row`] checks
//!   both texts of the row at GitHub Desktop's pinned instant through
//!   `corvene_ui::branch_list::branch_list_item(name, is_current_branch,
//!   author_date, now)`, the content `BranchFoldout::row` draws.
//!
//! The two drop cases are skipped in `tools/ghd-tests/skips/ui1.tsv`.

use std::time::{Duration, SystemTime};

use corvene_test_support::date_parse;
use corvene_ui::branch_list::branch_list_item;
use corvene_ui::relative_time::relative;

/// `Date.parse('2026-03-26T12:00:00.000Z')`
fn now() -> SystemTime {
    date_parse("2026-03-26T12:00:00.000Z")
}

// GHD: unit/ui/branch-list-item-test.tsx › BranchListItem › renders the branch name and relative author date
#[test]
fn renders_the_branch_name_and_relative_author_date() {
    // `authorDate={new Date(now - 30 * 1000)}`, 30 s before the system clock
    let author_date = SystemTime::now() - Duration::from_secs(30);

    // `screen.getByText('just now').textContent === 'just now'`
    assert_eq!(relative(author_date), "just now");
}

// GHD: unit/ui/branch-list-item-test.tsx › BranchListItem › renders the branch name and relative author date
#[test]
fn renders_the_branch_name_and_relative_author_date_in_the_row() {
    let item = branch_list_item("main", true, Some(now() - Duration::from_secs(30)), now());

    // `screen.getByText('main').textContent === 'main'`
    assert_eq!(item.name, "main");
    // `screen.getByText('just now').textContent === 'just now'`
    assert_eq!(item.author_date.as_deref(), Some("just now"));
}
