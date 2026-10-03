//! Port of GitHub Desktop's `app/test/unit/list-selection-test.ts`.
//!
//! GitHub Desktop's `findNextSelectableRow(rowCount, action, canSelectRow)`
//! (`ui/lib/list/selection.ts`, the `List`'s `moveSelection`) is Corvene's
//! `corvene_core::list_selection::step_index(len, current, delta)`, which
//! the `List`-based lists (stash files, commit files, the Open Pull Request
//! dialog) step with: `direction` is `delta` (`-1` up, `1` down) and `row:
//! -1` (the filter text box above the list) is `current: None`.
//!
//! `step_index` takes no `canSelectRow`: Corvene's lists keep group headers
//! out of the row index (`corvene_ui::filter_list::row_top`), so no row is
//! unselectable. The header case calls a stand-in and is ignored.

use corvene_core::list_selection::step_index;

const ROW_COUNT: usize = 5;

/// GitHub Desktop's `SelectionDirection`.
#[allow(dead_code)]
#[derive(Clone, Copy)]
enum SelectionDirection {
    Up,
    Down,
}

/// GitHub Desktop's `ISelectRowAction` (`wrap` defaults to true).
#[allow(dead_code)]
struct SelectRowAction {
    direction: SelectionDirection,
    row: isize,
}

/// Stand-in for GitHub Desktop's `findNextSelectableRow` with a
/// `canSelectRow` predicate (`ui/lib/list/selection.ts`). Replace it with
/// `step_index` once that takes one and remove the `#[ignore]`.
fn find_next_selectable_row(
    _row_count: usize,
    _action: SelectRowAction,
    _can_select_row: impl Fn(isize) -> bool,
) -> Option<isize> {
    unimplemented!("corvene_core::list_selection::step_index takes no canSelectRow predicate")
}

// GHD: unit/list-selection-test.ts › list-selection › findNextSelectableRow › returns first row when selecting down outside list (filter text)
#[test]
fn returns_first_row_when_selecting_down_outside_list_filter_text() {
    // { direction: 'down', row: -1 }
    let selected_row = step_index(ROW_COUNT, None, 1);
    assert_eq!(selected_row, Some(0));
}

// GHD: unit/list-selection-test.ts › list-selection › findNextSelectableRow › returns first selectable row when header is first
#[test]
#[ignore = "ghd: missing: corvene_core::list_selection::step_index has no canSelectRow predicate (ui/lib/list/selection.ts findNextSelectableRow)"]
fn returns_first_selectable_row_when_header_is_first() {
    let selected_row = find_next_selectable_row(
        ROW_COUNT,
        SelectRowAction {
            direction: SelectionDirection::Down,
            row: -1,
        },
        |row| row != 0,
    );
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
