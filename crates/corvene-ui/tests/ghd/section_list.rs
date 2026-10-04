//! Port of GitHub Desktop's `app/test/unit/section-list-test.ts`.
//!
//! GitHub Desktop's `getRowOffsetInSection(rowHeight, indexPath)`
//! (`ui/lib/list/section-list.tsx`) is the top of a row inside its section:
//! `row * rowHeight` for a fixed height, else the sum of the heights of the
//! rows above it. Corvene's is
//! `corvene_ui::filter_list::row_offset_in_section(row, row_height)`, which
//! `filter_list::row_top` builds on; the row height function gets the row
//! inside the section.

use gpui_kit::{Pixels, px};

use crate::lists_support::RowIndexPath;

/// GitHub Desktop's `getRowOffsetInSection` with a row height function
/// (`ui/lib/list/section-list.tsx`).
fn get_row_offset_in_section(
    row_height: impl Fn(RowIndexPath) -> Pixels,
    index_path: RowIndexPath,
) -> Pixels {
    let section = index_path.section;
    corvene_ui::filter_list::row_offset_in_section(
        usize::try_from(index_path.row).expect("a valid row"),
        |row| {
            row_height(RowIndexPath {
                section,
                row: row as isize,
            })
        },
    )
}

// GHD: unit/section-list-test.ts › section-list › getRowOffsetInSection › sums the height of each preceding variable-height row
#[test]
fn sums_the_height_of_each_preceding_variable_height_row() {
    let row_heights = [px(30.), px(46.), px(30.)];
    let offset = get_row_offset_in_section(
        |index| row_heights[index.row as usize],
        RowIndexPath { section: 0, row: 2 },
    );

    assert_eq!(offset, px(76.));
}
