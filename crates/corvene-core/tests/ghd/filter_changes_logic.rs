//! Port of GitHub Desktop's `app/test/unit/filter-changes-logic-test.ts`
//! (`ui/changes/filter-changes-logic.ts`).
//!
//! Corvene equivalents:
//!
//! - GitHub Desktop's `IFileListFilterState` is split in Corvene: the
//!   options are `corvene_core::FileListFilter` (`renamed` is Corvene's
//!   `705-renamed-files-filter` option, off as in the github-desktop
//!   preset) and the filter text stays in the text box, passed on its own.
//! - `IChangesListItem` is [`ChangesListItem`]; Corvene's functions take its
//!   `change` (`corvene_core::WorkingDirectoryFileChange`).
//! - `applyFilterOptions` is `corvene_core::filter::matches_options`,
//!   `getNoResultsMessage` is `corvene_core::filter::no_results_message`.
//! - `isCommittingFileHiddenByFilter` is the private
//!   `ChangesSidebar::committing_hidden_files` view method of `corvene-ui`
//!   (it reads the GPUI entities), `hasActiveFilters` is inlined in the
//!   changes view (`count_active() > 0 || text_active`), and `applyFilters`
//!   has no counterpart: the changes view applies the filter text and
//!   options even while View › Hide Changes Filter hides the filter box.
//!   Those cases call stand-ins and are ignored.

use std::collections::HashMap;

use corvene_core::filter::{matches_options, no_results_message};
use corvene_core::{
    DiffSelection, DiffSelectionType, FileListFilter, FileStatusKind, WorkingDirectoryFileChange,
};
use corvene_test_support::working_directory_file_change;

/// GitHub Desktop's `IChangesListItem` (`ui/changes/filter-changes-list.tsx`).
#[derive(Clone, Debug)]
struct ChangesListItem {
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    text: Vec<String>,
    change: WorkingDirectoryFileChange,
}

/// `createTestFile`
fn create_test_file(
    path: &str,
    kind: FileStatusKind,
    selection_type: DiffSelectionType,
) -> WorkingDirectoryFileChange {
    let selection = match selection_type {
        DiffSelectionType::Partial => DiffSelection::all().with_line(0, false),
        DiffSelectionType::All => DiffSelection::all(),
        DiffSelectionType::None => DiffSelection::none(),
    };
    working_directory_file_change(path, kind, selection)
}

/// `createTestItem`
fn create_test_item(
    path: &str,
    status: FileStatusKind,
    selection_type: DiffSelectionType,
) -> ChangesListItem {
    let change = create_test_file(path, status, selection_type);
    ChangesListItem {
        id: path.to_string(),
        text: vec![path.to_string()],
        change,
    }
}

/// The options of an `IFileListFilterState`.
fn filter_options(
    is_included_in_commit: bool,
    is_excluded_from_commit: bool,
    is_new_file: bool,
    is_modified_file: bool,
    is_deleted_file: bool,
) -> FileListFilter {
    FileListFilter {
        included: is_included_in_commit,
        excluded: is_excluded_from_commit,
        new_files: is_new_file,
        modified: is_modified_file,
        deleted: is_deleted_file,
        renamed: false,
    }
}

/// `applyFilterOptions(item, filters)`
fn apply_filter_options(item: &ChangesListItem, filters: &FileListFilter) -> bool {
    matches_options(filters, &item.change)
}

/// Stand-in for GitHub Desktop's `isCommittingFileHiddenByFilter(
/// fileIdsIncludedInCommit, filteredItems, fileCount, filters)`
/// (`ui/changes/filter-changes-logic.ts`). Replace it with a
/// `corvene_core::filter` function once there is one and remove the
/// `#[ignore]`s.
fn is_committing_file_hidden_by_filter<V>(
    _file_ids_included_in_commit: &[&str],
    _filtered_items: &HashMap<String, V>,
    _file_count: usize,
    _filter_text: &str,
    _filters: &FileListFilter,
) -> bool {
    unimplemented!(
        "corvene_core::filter has no isCommittingFileHiddenByFilter (only the ChangesSidebar view method)"
    )
}

/// Stand-in for GitHub Desktop's `hasActiveFilters(filters)`
/// (`ui/changes/filter-changes-logic.ts`).
fn has_active_filters(_filter_text: &str, _filters: &FileListFilter) -> bool {
    unimplemented!("corvene_core::filter has no hasActiveFilters")
}

/// Stand-in for GitHub Desktop's `applyFilters(item, showChangesFilter,
/// filters)` (`ui/changes/filter-changes-logic.ts`).
fn apply_filters(
    _item: &ChangesListItem,
    _show_changes_filter: bool,
    _filters: &FileListFilter,
) -> bool {
    unimplemented!("corvene_core::filter has no applyFilters (showChangesFilter bypass)")
}

/// `filteredItems`: the GitHub Desktop test maps each id to `{} as
/// IChangesListItem`; only the keys and the size are read.
fn filtered_items(ids: &[&str]) -> HashMap<String, ()> {
    ids.iter().map(|id| (id.to_string(), ())).collect()
}

// GHD: unit/filter-changes-logic-test.ts › filter-changes-logic › applyFilterOptions › when no filters are active › should show all files
#[test]
fn should_show_all_files() {
    let filters = filter_options(false, false, false, false, false);

    let new_file = create_test_item("new.txt", FileStatusKind::New, DiffSelectionType::All);
    let modified_file = create_test_item(
        "modified.txt",
        FileStatusKind::Modified,
        DiffSelectionType::All,
    );
    let deleted_file = create_test_item(
        "deleted.txt",
        FileStatusKind::Deleted,
        DiffSelectionType::All,
    );

    assert!(apply_filter_options(&new_file, &filters));
    assert!(apply_filter_options(&modified_file, &filters));
    assert!(apply_filter_options(&deleted_file, &filters));
}

// GHD: unit/filter-changes-logic-test.ts › filter-changes-logic › applyFilterOptions › when using AND logic › should show files matching ALL active filters
#[test]
fn should_show_files_matching_all_active_filters() {
    let filters = filter_options(true, false, true, false, false);

    // Staged new file - matches both filters
    let staged_new_file = create_test_item(
        "staged-new.txt",
        FileStatusKind::New,
        DiffSelectionType::All,
    );
    // Unstaged new file - doesn't match included filter
    let unstaged_new_file = create_test_item(
        "unstaged-new.txt",
        FileStatusKind::New,
        DiffSelectionType::None,
    );
    // Staged modified file - doesn't match new file filter
    let staged_modified_file = create_test_item(
        "staged-modified.txt",
        FileStatusKind::Modified,
        DiffSelectionType::All,
    );

    assert!(apply_filter_options(&staged_new_file, &filters));
    assert!(!apply_filter_options(&unstaged_new_file, &filters));
    assert!(!apply_filter_options(&staged_modified_file, &filters));
}

// GHD: unit/filter-changes-logic-test.ts › filter-changes-logic › applyFilterOptions › when using AND logic › should handle conflicting filters correctly
#[test]
fn should_handle_conflicting_filters_correctly() {
    // Both can't be true at same time
    let filters = filter_options(true, true, false, false, false);

    let staged_file = create_test_item(
        "staged.txt",
        FileStatusKind::Modified,
        DiffSelectionType::All,
    );
    let unstaged_file = create_test_item(
        "unstaged.txt",
        FileStatusKind::Modified,
        DiffSelectionType::None,
    );

    // No file can be both staged and unstaged
    assert!(!apply_filter_options(&staged_file, &filters));
    assert!(!apply_filter_options(&unstaged_file, &filters));
}

// GHD: unit/filter-changes-logic-test.ts › filter-changes-logic › applyFilterOptions › when using AND logic › should treat untracked files as new files
#[test]
fn should_treat_untracked_files_as_new_files() {
    let filters = filter_options(false, false, true, false, false);

    let untracked_file = ChangesListItem {
        id: "untracked.txt".to_string(),
        text: vec!["untracked.txt".to_string()],
        change: working_directory_file_change(
            "untracked.txt",
            FileStatusKind::Untracked,
            DiffSelection::none(),
        ),
    };

    assert!(apply_filter_options(&untracked_file, &filters));
}

// GHD: unit/filter-changes-logic-test.ts › filter-changes-logic › applyFilterOptions › when using AND logic › should match excluded files when excluded filter is active
#[test]
fn should_match_excluded_files_when_excluded_filter_is_active() {
    let filters = filter_options(false, true, false, false, false);

    let excluded_file = create_test_item(
        "excluded.txt",
        FileStatusKind::Modified,
        DiffSelectionType::None,
    );
    let included_file = create_test_item(
        "included.txt",
        FileStatusKind::Modified,
        DiffSelectionType::All,
    );

    assert!(apply_filter_options(&excluded_file, &filters));
    assert!(!apply_filter_options(&included_file, &filters));
}

// GHD: unit/filter-changes-logic-test.ts › filter-changes-logic › isCommittingFileHiddenByFilter › should return false when no filters are active
#[test]
#[ignore = "ghd: missing: corvene_core::filter has no isCommittingFileHiddenByFilter, only the private corvene-ui ChangesSidebar::committing_hidden_files (ui/changes/filter-changes-logic.ts)"]
fn should_return_false_when_no_filters_are_active() {
    let filter_text = "";
    let filters = filter_options(false, false, false, false, false);

    let file_ids = ["file1", "file2"];
    let filtered_items = filtered_items(&["file1", "file2"]);

    assert!(!is_committing_file_hidden_by_filter(
        &file_ids,
        &filtered_items,
        2,
        filter_text,
        &filters
    ));
}

// GHD: unit/filter-changes-logic-test.ts › filter-changes-logic › isCommittingFileHiddenByFilter › should return true when committing files not in filtered list
#[test]
#[ignore = "ghd: missing: corvene_core::filter has no isCommittingFileHiddenByFilter, only the private corvene-ui ChangesSidebar::committing_hidden_files (ui/changes/filter-changes-logic.ts)"]
fn should_return_true_when_committing_files_not_in_filtered_list() {
    let filter_text = "";
    let filters = filter_options(true, false, false, false, false);

    let file_ids = ["file1", "file2", "file3"];
    let filtered_items = filtered_items(&["file1", "file2"]);

    assert!(is_committing_file_hidden_by_filter(
        &file_ids,
        &filtered_items,
        5,
        filter_text,
        &filters
    ));
}

// GHD: unit/filter-changes-logic-test.ts › filter-changes-logic › isCommittingFileHiddenByFilter › should return false when all files remain visible after filtering
#[test]
#[ignore = "ghd: missing: corvene_core::filter has no isCommittingFileHiddenByFilter, only the private corvene-ui ChangesSidebar::committing_hidden_files (ui/changes/filter-changes-logic.ts)"]
fn should_return_false_when_all_files_remain_visible_after_filtering() {
    let filter_text = "src";
    let filters = filter_options(false, false, false, true, false);

    let file_ids = ["file1", "file2"];
    let filtered_items = filtered_items(&["file1", "file2"]);

    assert!(!is_committing_file_hidden_by_filter(
        &file_ids,
        &filtered_items,
        2,
        filter_text,
        &filters
    ));
}

// GHD: unit/filter-changes-logic-test.ts › filter-changes-logic › getNoResultsMessage › should return undefined when no filters active
#[test]
fn should_return_undefined_when_no_filters_active() {
    let filter_text = "";
    let filters = filter_options(false, false, false, false, false);

    assert_eq!(no_results_message(filter_text, &filters), None);
}

// GHD: unit/filter-changes-logic-test.ts › filter-changes-logic › getNoResultsMessage › should return message with text filter
#[test]
fn should_return_message_with_text_filter() {
    let filter_text = "test";
    let filters = filter_options(false, false, false, false, false);

    let message = no_results_message(filter_text, &filters);
    assert!(message.is_some_and(|m| m.contains("\"test\"")));
}

// GHD: unit/filter-changes-logic-test.ts › filter-changes-logic › getNoResultsMessage › should return message with multiple filters
#[test]
fn should_return_message_with_multiple_filters() {
    let filter_text = "";
    let filters = filter_options(true, false, true, false, false);

    let message = no_results_message(filter_text, &filters);
    assert!(
        message
            .as_deref()
            .is_some_and(|m| m.contains("Included in commit"))
    );
    assert!(message.as_deref().is_some_and(|m| m.contains("New files")));
}

// GHD: unit/filter-changes-logic-test.ts › filter-changes-logic › getNoResultsMessage › should format three or more filters with commas and and
#[test]
fn should_format_three_or_more_filters_with_commas_and_and() {
    let filter_text = "src";
    let filters = filter_options(true, false, false, true, true);

    assert_eq!(
        no_results_message(filter_text, &filters).as_deref(),
        Some(
            "Sorry, I can't find any changed files matching the following filters: \"src\", Included in commit, Modified files, and Deleted files"
        )
    );
}

// GHD: unit/filter-changes-logic-test.ts › filter-changes-logic › hasActiveFilters › should return false when no text or filter options are active
#[test]
#[ignore = "ghd: missing: corvene_core::filter has no hasActiveFilters, the changes view inlines it (ui/changes/filter-changes-logic.ts)"]
fn should_return_false_when_no_text_or_filter_options_are_active() {
    let filter_text = "";
    let filters = filter_options(false, false, false, false, false);

    assert!(!has_active_filters(filter_text, &filters));
}

// GHD: unit/filter-changes-logic-test.ts › filter-changes-logic › hasActiveFilters › should return true when either text or filter options are active
#[test]
#[ignore = "ghd: missing: corvene_core::filter has no hasActiveFilters, the changes view inlines it (ui/changes/filter-changes-logic.ts)"]
fn should_return_true_when_either_text_or_filter_options_are_active() {
    assert!(has_active_filters(
        "src",
        &filter_options(false, false, false, false, false)
    ));

    assert!(has_active_filters(
        "",
        &filter_options(false, false, false, true, false)
    ));
}

// GHD: unit/filter-changes-logic-test.ts › filter-changes-logic › applyFilters › should bypass filter logic when the changes filter is hidden
#[test]
#[ignore = "ghd: missing: no applyFilters, corvene-ui ChangesSidebar::visible_files applies the filter options even while the changes filter is hidden (ui/changes/filter-changes-logic.ts)"]
fn should_bypass_filter_logic_when_the_changes_filter_is_hidden() {
    let item = create_test_item(
        "deleted.txt",
        FileStatusKind::Deleted,
        DiffSelectionType::All,
    );

    let filters = filter_options(false, false, true, false, false);

    assert!(apply_filters(&item, false, &filters));
    assert!(!apply_filters(&item, true, &filters));
}
