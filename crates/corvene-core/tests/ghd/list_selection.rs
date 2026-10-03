//! Port of GitHub Desktop's `app/test/unit/list-selection-test.ts`.
//!
//! GitHub Desktop's `findNextSelectableRow(rowCount, action, canSelectRow)`
//! (`ui/lib/list/selection.ts`, the `List`'s `moveSelection`) is Corvene's
//! `corvene_core::list_selection::step_index(len, current, delta)`, which
//! the `List`-based lists (stash files, commit files, the Open Pull Request
//! dialog) step with: `direction` is `delta` (`-1` up, `1` down) and `row:
//! -1` (the filter text box above the list) is `current: None`.
//!
//! The header case passes its `canSelectRow` to
//! `corvene_core::list_selection::find_next_selectable_row(rowCount, row,
//! delta, wrap, canSelectRow)`, the function `step_index` wraps for lists
//! whose rows are all selectable (Corvene's lists keep group headers out of
//! the row index, `corvene_ui::filter_list::row_top`).

use corvene_core::list_selection::{find_next_selectable_row, step_index};

const ROW_COUNT: usize = 5;

// GHD: unit/list-selection-test.ts › list-selection › findNextSelectableRow › returns first row when selecting down outside list (filter text)
#[test]
fn returns_first_row_when_selecting_down_outside_list_filter_text() {
    // { direction: 'down', row: -1 }
    let selected_row = step_index(ROW_COUNT, None, 1);
    assert_eq!(selected_row, Some(0));
}

// GHD: unit/list-selection-test.ts › list-selection › findNextSelectableRow › returns first selectable row when header is first
#[test]
fn returns_first_selectable_row_when_header_is_first() {
    // { direction: 'down', row: -1 } (`wrap` defaults to true)
    let selected_row = find_next_selectable_row(ROW_COUNT, None, 1, true, |row| row != 0);
    assert_eq!(selected_row, Some(1));
}

// GHD: unit/list-selection-test.ts › list-selection › findNextSelectableRow › returns first row when selecting down from last row
#[test]
fn returns_first_row_when_selecting_down_from_last_row() {
    let last_row = ROW_COUNT - 1;
    // { direction: 'down', row: lastRow }
    let selected_row = step_index(ROW_COUNT, Some(last_row), 1);
    assert_eq!(selected_row, Some(0));
}

// GHD: unit/list-selection-test.ts › list-selection › findNextSelectableRow › returns last row when selecting up from top row
#[test]
fn returns_last_row_when_selecting_up_from_top_row() {
    // { direction: 'up', row: 0 }
    let selected_row = step_index(ROW_COUNT, Some(0), -1);
    assert_eq!(selected_row, Some(4));
}
