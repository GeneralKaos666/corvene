//! Port of GitHub Desktop's
//! `app/test/unit/ui/path-and-selection-surfaces-test.tsx`.
//!
//! GitHub Desktop's `PathLabel` (`ui/lib/path-label.tsx`, used by the
//! changes list, a commit's file list and the diff header) renders one
//! `PathText` for most statuses and, for a rename or copy, the old path's
//! `PathText`, a `.rename-arrow` octicon and the new path's `PathText`.
//! Corvene's is `corvene_ui::path_label::path_label(path, kind, old_path)`,
//! the parts the changes list, a commit's file list and
//! `diff_view::diff_header` draw (`path_label::path_label_element`).
//! GitHub Desktop's `AppFileStatus` carries a rename's `oldPath`; Corvene
//! keeps it beside the status (`old_path` of `WorkingDirectoryFileChange` /
//! `CommittedFileChange`), so it is a parameter.
//!
//! GitHub Desktop's `MultipleSelection` (`ui/changes/multiple-selection.tsx`)
//! is the changes pane's blank slate for several selected files: a
//! decorative image (`alt=""`) and "<count> files selected". Corvene's
//! counterpart is `corvene_ui::no_changes::multiple_selection_content(count)`,
//! the content `no_changes::multiple_selection(count, cx)` draws.
//!
//! Not ported (React DOM only): the `.path-label-component` /
//! `.path-text-component` elements' presence as such, `aria-hidden` (the
//! `ariaHidden` prop), `availableWidth` (truncation width; the cases
//! assert no truncation) and `MultipleSelection`'s `#no-changes.panel.blankslate`
//! / `.blankslate-image` classes.

use corvene_core::FileStatusKind;
use corvene_ui::no_changes::multiple_selection_content as multiple_selection;
use corvene_ui::path_label::{PathLabelPart, path_label};

/// The label's `textContent`: each path's directory and file name in order
/// (the arrow is an svg without text).
fn text_content(parts: &[PathLabelPart]) -> String {
    parts
        .iter()
        .map(|part| match part {
            PathLabelPart::Path {
                directory,
                file_name,
            } => format!("{directory}{file_name}"),
            PathLabelPart::RenameArrow => String::new(),
        })
        .collect()
}

fn path_count(parts: &[PathLabelPart]) -> usize {
    parts
        .iter()
        .filter(|part| matches!(part, PathLabelPart::Path { .. }))
        .count()
}

// GHD: unit/ui/path-and-selection-surfaces-test.tsx › path and selection surfaces › renders a simple path label for non-rename statuses
#[test]
fn renders_a_simple_path_label_for_non_rename_statuses() {
    // `status: { kind: AppFileStatusKind.Modified }`
    let parts = path_label("src/ui/branch.tsx", FileStatusKind::Modified, None);

    // `.path-text-component` is rendered
    assert!(path_count(&parts) > 0);
    let text = text_content(&parts);
    assert!(text.contains(if cfg!(windows) {
        "src\\ui\\"
    } else {
        "src/ui/"
    }));
    assert!(text.contains("branch.tsx"));
    // no `.rename-arrow`
    assert!(!parts.contains(&PathLabelPart::RenameArrow));
}

// GHD: unit/ui/path-and-selection-surfaces-test.tsx › path and selection surfaces › renders old and new paths for renamed files with a rename arrow
#[test]
fn renders_old_and_new_paths_for_renamed_files_with_a_rename_arrow() {
    // `status: { kind: Renamed, oldPath: 'src/ui/old-name.tsx',
    // renameIncludesModifications: false }`
    let parts = path_label(
        "src/ui/new-name.tsx",
        FileStatusKind::Renamed,
        Some("src/ui/old-name.tsx"),
    );

    assert_eq!(path_count(&parts), 2);
    assert!(parts.contains(&PathLabelPart::RenameArrow));
    let text = text_content(&parts);
    assert!(text.contains("old-name.tsx"));
    assert!(text.contains("new-name.tsx"));
}

// GHD: unit/ui/path-and-selection-surfaces-test.tsx › path and selection surfaces › renders the multiple-selection blank slate with the selected file count
#[test]
fn renders_the_multiple_selection_blank_slate_with_the_selected_file_count() {
    let view = multiple_selection(3);

    // `.blankslate-image` with `alt=""`
    assert!(view.image_alt.is_some());
    assert_eq!(view.image_alt.as_deref(), Some(""));
    assert!(view.text.contains("3 files selected"));
}
