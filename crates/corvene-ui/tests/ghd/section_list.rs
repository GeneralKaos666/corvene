//! Port of GitHub Desktop's `app/test/unit/section-list-test.ts`.
//!
//! GitHub Desktop's `getRowOffsetInSection(rowHeight, indexPath)`
//! (`ui/lib/list/section-list.tsx`) is the top of a row inside its section:
//! `row * rowHeight` for a fixed height, else the sum of the heights of the
//! rows above it. Corvene's list geometry is
//! `corvene_ui::filter_list::row_top(group_sizes, ix, header, row)`, which
//! takes one fixed row height, so the variable-height case calls a stand-in
//! and is ignored.

use gpui_kit::{Pixels, px};

use crate::lists_support::RowIndexPath;

/// Stand-in for GitHub Desktop's `getRowOffsetInSection` with a row height
/// function (`ui/lib/list/section-list.tsx`). Replace it with a
/// `corvene_ui::filter_list` function once there is one and remove the
/// `#[ignore]`.
fn get_row_offset_in_section(
    _row_height: impl Fn(RowIndexPath) -> Pixels,
    _index_path: RowIndexPath,
) -> Pixels {
    unimplemented!("corvene_ui::filter_list::row_top only takes one fixed row height")
}

// GHD: unit/section-list-test.ts › section-list › getRowOffsetInSection › sums the height of each preceding variable-height row
#[test]
#[ignore = "ghd: missing: corvene_ui::filter_list::row_top takes one fixed row height, no variable-height offset (ui/lib/list/section-list.tsx getRowOffsetInSection)"]
fn sums_the_height_of_each_preceding_variable_height_row() {
    let row_heights = [px(30.), px(46.), px(30.)];
    let offset = get_row_offset_in_section(
        |index| row_heights[index.row as usize],
        RowIndexPath { section: 0, row: 2 },
    );

    assert_eq!(offset, px(76.));
}
