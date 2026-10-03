//! Port of GitHub Desktop's
//! `app/test/unit/ui/path-and-selection-surfaces-test.tsx`.
//!
//! GitHub Desktop's `PathLabel` (`ui/lib/path-label.tsx`, used by the
//! changes list, a commit's file list and the diff header) renders one
//! `PathText` for most statuses and, for a rename or copy, the old path's
//! `PathText`, a `.rename-arrow` octicon and the new path's `PathText`.
//! Corvene has no such component: the changes list row
//! (`changes.rs`, `display_path(file.directory())` + `file.file_name()`),
//! the commit file list and `diff_view::diff_header` (which takes the new
//! path and the status kind only) split the new path inline and never show
//! a rename's old path. [`path_label`] is a stand-in returning what
//! `PathLabel` renders; GitHub Desktop's `AppFileStatus` carries a rename's
//! `oldPath`, Corvene keeps it beside the status (`old_path` of
//! `WorkingDirectoryFileChange` / `CommittedFileChange`), so the stand-in
//! takes it as a parameter.
//!
//! GitHub Desktop's `MultipleSelection` (`ui/changes/multiple-selection.tsx`)
//! is the changes pane's blank slate for several selected files: a
//! decorative image (`alt=""`) and "<count> files selected". Corvene's
//! counterpart `corvene_ui::no_changes::multiple_selection(count, cx)` draws
//! the same as a GPUI element with nothing to read, so
//! [`multiple_selection`] is a stand-in returning its content.
//!
//! Not ported (React DOM only): the `.path-label-component` /
//! `.path-text-component` elements' presence as such, `aria-hidden` (the
//! `ariaHidden` prop), `availableWidth` (truncation width; the cases
//! assert no truncation) and `MultipleSelection`'s `#no-changes.panel.blankslate`
//! / `.blankslate-image` classes.

use corvene_core::FileStatusKind;

/// One piece of what GitHub Desktop's `PathLabel` renders.
#[allow(dead_code)] // built by the real PathLabel
#[derive(Clone, Debug, PartialEq, Eq)]
enum PathLabelPart {
    /// A `PathText`: the directory (platform separators, trailing
    /// separator kept) and the file name.
    Path {
        directory: String,
        file_name: String,
    },
    /// The `.rename-arrow` octicon between a rename's old and new path.
    RenameArrow,
}

/// Stand-in for GitHub Desktop's `PathLabel` (`ui/lib/path-label.tsx`):
/// the parts it renders for the file at `path` with status `kind`
/// (`old_path`: a rename's or copy's previous path). Replace it with the
/// Corvene function once there is one and remove the `#[ignore]`s.
fn path_label(_path: &str, _kind: FileStatusKind, _old_path: Option<&str>) -> Vec<PathLabelPart> {
    unimplemented!(
        "Corvene has no PathLabel: file rows and diff_view::diff_header split the new path inline and never show a rename's old path"
    )
}

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

/// What GitHub Desktop's `MultipleSelection` shows.
#[allow(dead_code)] // filled by the real MultipleSelection content
struct MultipleSelectionContent {
    /// The blank-slate image's alternative text (`Some("")`: decorative),
    /// `None` when there is no image.
    image_alt: Option<String>,
    /// The pane's text.
    text: String,
}

/// Stand-in for GitHub Desktop's `MultipleSelection`
/// (`ui/changes/multiple-selection.tsx`) for `count` selected files.
/// Replace it with the Corvene function once there is one and remove the
/// `#[ignore]`.
fn multiple_selection(_count: usize) -> MultipleSelectionContent {
    unimplemented!(
        "Corvene has no MultipleSelection content: no_changes::multiple_selection returns a GPUI element"
    )
}

fn path_count(parts: &[PathLabelPart]) -> usize {
    parts
        .iter()
        .filter(|part| matches!(part, PathLabelPart::Path { .. }))
        .count()
}

// GHD: unit/ui/path-and-selection-surfaces-test.tsx › path and selection surfaces › renders a simple path label for non-rename statuses
#[test]
#[ignore = "ghd: missing: no PathLabel (ui/lib/path-label.tsx); file rows and diff_view::diff_header split the path inline"]
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
#[ignore = "ghd: missing: no PathLabel (ui/lib/path-label.tsx); Corvene's file rows and diff header show only a rename's new path, never the old path and arrow"]
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
#[ignore = "ghd: missing: no MultipleSelection content (multiple-selection.tsx); no_changes::multiple_selection(count, cx) draws the image and text as a GPUI element"]
fn renders_the_multiple_selection_blank_slate_with_the_selected_file_count() {
    let view = multiple_selection(3);

    // `.blankslate-image` with `alt=""`
    assert!(view.image_alt.is_some());
    assert_eq!(view.image_alt.as_deref(), Some(""));
    assert!(view.text.contains("3 files selected"));
}
