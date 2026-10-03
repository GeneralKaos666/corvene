//! Port of GitHub Desktop's `app/test/unit/ui/branch-list-item-test.tsx`.
//!
//! GitHub Desktop's `BranchListItem` (`ui/branches/branch-list-item.tsx`)
//! is Corvene's `BranchFoldout::row` (`corvene_ui::branch_list`), a GPUI
//! element that draws the branch name and
//! `corvene_ui::relative_time::relative` of the branch tip's time (GitHub
//! Desktop's `RelativeTime` with `onlyRelative`, as long as "Prefer absolute
//! dates" is off, its default and the default of `corvene_ui::format`).
//!
//! The case is ported twice:
//!
//! - [`renders_the_branch_name_and_relative_author_date`] calls `relative`
//!   on its own. GitHub Desktop pins the clock (`enableTestTimers(['Date',
//!   …], now)`) and renders an author date 30 s before it; `relative` reads
//!   the system clock, so the date here is 30 s before the system clock: the
//!   same distance, which is all `relative` uses below a minute. It cannot
//!   check the name: the row draws it inline.
//! - [`renders_the_branch_name_and_relative_author_date_in_the_row`] checks
//!   both texts of the row at GitHub Desktop's pinned instant. Corvene has
//!   no function returning a row's content (and `relative` takes no clock),
//!   so [`branch_list_item`] is a stand-in; replace it with the Corvene
//!   function once there is one and remove the `#[ignore]`.
//!
//! The two drop cases are skipped in `tools/ghd-tests/skips/ui1.tsv`.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use corvene_ui::relative_time::relative;

/// `Date.parse('2026-03-26T12:00:00.000Z')`
fn now() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_774_526_400)
}

/// What GitHub Desktop's `BranchListItem` shows: the name and the author
/// date's `RelativeTime` (`onlyRelative`).
#[derive(Clone, Debug, PartialEq, Eq)]
struct BranchListItemContent {
    name: String,
    author_date: Option<String>,
}

/// Stand-in for GitHub Desktop's `BranchListItem`
/// (`ui/branches/branch-list-item.tsx`) rendered with `name`,
/// `isCurrentBranch` and `authorDate` while the clock reads `now`.
fn branch_list_item(
    _name: &str,
    _is_current_branch: bool,
    _author_date: Option<SystemTime>,
    _now: SystemTime,
) -> BranchListItemContent {
    unimplemented!(
        "Corvene has no BranchListItem content: BranchFoldout::row draws the name and \
         relative(tip_time) inline (GPUI), and relative reads the system clock"
    )
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
#[ignore = "ghd: missing: no BranchListItem content fn (ui/branches/branch-list-item.tsx); BranchFoldout::row draws the name and relative(tip_time) inline in GPUI, and relative has no injectable clock"]
fn renders_the_branch_name_and_relative_author_date_in_the_row() {
    let item = branch_list_item("main", true, Some(now() - Duration::from_secs(30)), now());

    // `screen.getByText('main').textContent === 'main'`
    assert_eq!(item.name, "main");
    // `screen.getByText('just now').textContent === 'just now'`
    assert_eq!(item.author_date.as_deref(), Some("just now"));
}
