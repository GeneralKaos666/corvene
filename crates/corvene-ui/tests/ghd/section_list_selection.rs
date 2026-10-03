//! Port of GitHub Desktop's `app/test/unit/section-list-selection-test.ts`.
//!
//! GitHub Desktop's `findNextSelectableRow(rowCount, action, canSelectRow)`
//! (`ui/lib/list/section-list-selection.ts`) steps through a
//! `SectionList` (the filter lists: branches, pull requests, repositories,
//! changes) by `RowIndexPath`. Corvene's filter lists address a row by its
//! index across all groups instead (`BranchFoldout::highlighted` over
//! `sizes.iter().sum()` rows) and step it with
//! `corvene_ui::filter_list::step(current, delta, count)`. The cases go
//! through [`filter_list_step`]: `direction` is `delta`, the row is
//! converted to that index and back (GitHub Desktop's
//! `rowIndexPathToGlobalIndex` / `globalIndexToRowIndexPath`, a change of
//! representation only), and `InvalidRowIndexPath` (the filter text box) is
//! `current: None`.
//!
//! `step` takes no `canSelectRow`: Corvene keeps group headers out of the
//! row index (`filter_list::row_top`), so no row is unselectable. The
//! header case calls a stand-in and is ignored.

use corvene_ui::filter_list::step;

use crate::lists_support::{INVALID_ROW_INDEX_PATH, RowIndexPath, row_index_path_equals};

const ROW_COUNT: [usize; 3] = [5, 3, 8];

/// GitHub Desktop's `SelectionDirection`.
#[derive(Clone, Copy)]
enum SelectionDirection {
    Up,
    Down,
}

/// GitHub Desktop's `ISelectRowAction` (`wrap` defaults to true).
struct SelectRowAction {
    direction: SelectionDirection,
    row: RowIndexPath,
}

/// A row as Corvene's grouped filter lists address it: its index across all
/// sections, `None` for `InvalidRowIndexPath` (nothing highlighted).
fn highlighted_index(path: RowIndexPath, row_count: &[usize]) -> Option<usize> {
    if row_index_path_equals(path, INVALID_ROW_INDEX_PATH) {
        return None;
    }
    let section = usize::try_from(path.section).expect("a valid section");
    let row = usize::try_from(path.row).expect("a valid row");
    Some(row_count[..section].iter().sum::<usize>() + row)
}

/// The `RowIndexPath` of an index across all sections.
fn row_index_path(index: usize, row_count: &[usize]) -> RowIndexPath {
    let mut section = 0;
    let mut row = index;
    while row >= row_count[section] {
        row -= row_count[section];
        section += 1;
    }
    RowIndexPath {
        section: section as isize,
        row: row as isize,
    }
}

/// `findNextSelectableRow(rowCount, action)` (every row selectable) through
/// Corvene's `filter_list::step`.
fn filter_list_step(row_count: &[usize], action: SelectRowAction) -> Option<RowIndexPath> {
    let delta = match action.direction {
        SelectionDirection::Up => -1,
        SelectionDirection::Down => 1,
    };
    step(
        highlighted_index(action.row, row_count),
        delta,
        row_count.iter().sum(),
    )
    .map(|index| row_index_path(index, row_count))
}

/// Stand-in for GitHub Desktop's `findNextSelectableRow` with a
/// `canSelectRow` predicate (`ui/lib/list/section-list-selection.ts`).
/// Replace it with `filter_list::step` once that takes one and remove the
/// `#[ignore]`.
fn find_next_selectable_row(
    _row_count: &[usize],
    _action: SelectRowAction,
    _can_select_row: impl Fn(RowIndexPath) -> bool,
) -> Option<RowIndexPath> {
    unimplemented!("corvene_ui::filter_list::step takes no canSelectRow predicate")
}

// GHD: unit/section-list-selection-test.ts › section-list-selection › findNextSelectableRow › returns first row when selecting down outside list (filter text)
#[test]
fn returns_first_row_when_selecting_down_outside_list_filter_text() {
    let selected_row = filter_list_step(
        &ROW_COUNT,
        SelectRowAction {
            direction: SelectionDirection::Down,
            row: INVALID_ROW_INDEX_PATH,
        },
    );
    assert_eq!(selected_row.map(|r| r.row), Some(0));
}

// GHD: unit/section-list-selection-test.ts › section-list-selection › findNextSelectableRow › returns first selectable row when header is first
#[test]
#[ignore = "ghd: missing: corvene_ui::filter_list::step has no canSelectRow predicate (ui/lib/list/section-list-selection.ts findNextSelectableRow)"]
fn returns_first_selectable_row_when_header_is_first() {
    let selected_row = find_next_selectable_row(
        &ROW_COUNT,
        SelectRowAction {
            direction: SelectionDirection::Down,
            row: INVALID_ROW_INDEX_PATH,
        },
        |row| !(row.section == 0 && row.row == 0),
    );
    assert_eq!(selected_row.map(|r| r.row), Some(1));
}

// GHD: unit/section-list-selection-test.ts › section-list-selection › findNextSelectableRow › returns first row when selecting down from last row
#[test]
fn returns_first_row_when_selecting_down_from_last_row() {
    let last_row = ROW_COUNT[0] as isize - 1;
    let selected_row = filter_list_step(
        &ROW_COUNT,
        SelectRowAction {
            direction: SelectionDirection::Down,
            row: RowIndexPath {
                section: 0,
                row: last_row,
            },
        },
    );
    assert_eq!(selected_row.map(|r| r.row), Some(0));
}

// GHD: unit/section-list-selection-test.ts › section-list-selection › findNextSelectableRow › returns last row when selecting up from top row
#[test]
fn returns_last_row_when_selecting_up_from_top_row() {
    let selected_row = filter_list_step(
        &ROW_COUNT,
        SelectRowAction {
            direction: SelectionDirection::Up,
            row: RowIndexPath { section: 0, row: 0 },
        },
    );
    let selected_row = selected_row.expect("selectedRow !== null");
    assert!(row_index_path_equals(
        selected_row,
        RowIndexPath { section: 2, row: 7 }
    ));
}

// GHD: unit/section-list-selection-test.ts › section-list-selection › findNextSelectableRow › returns first row of next section when selecting down from last row of a section
#[test]
fn returns_first_row_of_next_section_when_selecting_down_from_last_row_of_a_section() {
    let selected_row = filter_list_step(
        &ROW_COUNT,
        SelectRowAction {
            direction: SelectionDirection::Down,
            row: RowIndexPath { section: 0, row: 4 },
        },
    );
    let selected_row = selected_row.expect("selectedRow !== null");
    assert!(row_index_path_equals(
        selected_row,
        RowIndexPath { section: 1, row: 0 }
    ));
}

// GHD: unit/section-list-selection-test.ts › section-list-selection › findNextSelectableRow › returns last row of previous section when selecting up from first row of a section
#[test]
fn returns_last_row_of_previous_section_when_selecting_up_from_first_row_of_a_section() {
    let selected_row = filter_list_step(
        &ROW_COUNT,
        SelectRowAction {
            direction: SelectionDirection::Up,
            row: RowIndexPath { section: 2, row: 0 },
        },
    );
    let selected_row = selected_row.expect("selectedRow !== null");
    assert!(row_index_path_equals(
        selected_row,
        RowIndexPath { section: 1, row: 2 }
    ));
}
