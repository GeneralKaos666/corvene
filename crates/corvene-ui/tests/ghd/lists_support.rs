//! Shared by `corvene-ui`'s ports of GitHub Desktop's list tests: GitHub
//! Desktop's `RowIndexPath` (`ui/lib/list/list-row-index-path.ts`), the
//! address of a row in a list of sections.

/// GitHub Desktop's `RowIndexPath`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RowIndexPath {
    pub section: isize,
    pub row: isize,
}

/// GitHub Desktop's `InvalidRowIndexPath`.
pub const INVALID_ROW_INDEX_PATH: RowIndexPath = RowIndexPath {
    section: -1,
    row: -1,
};

/// GitHub Desktop's `rowIndexPathEquals`.
pub fn row_index_path_equals(a: RowIndexPath, b: RowIndexPath) -> bool {
    a.section == b.section && a.row == b.row
}
