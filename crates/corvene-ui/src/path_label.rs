//! GHD `PathLabel` (`ui/lib/path-label.tsx`, `styles/ui/_path-label.scss`):
//! a changed file's path as a [`crate::path_text`] `PathText`, or for a
//! rename or copy the old path, an arrow and the new path, each side at
//! most half the width.

use corvene_core::FileStatusKind;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::icons::{Octicon, octicon};
use crate::path_text::path_text;
use crate::theme::sizes::SPACING_HALF;

/// One piece of a [`path_label`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PathLabelPart {
    /// A `PathText`: the directory (platform separators, trailing separator
    /// kept) and the file name.
    Path {
        directory: String,
        file_name: String,
    },
    /// The `.rename-arrow` octicon between a rename's old and new path.
    RenameArrow,
}

/// GHD `PathLabel` for the file at `path` with status `kind`; `old_path` is
/// a rename's or copy's previous path (GHD `status.oldPath`).
pub fn path_label(path: &str, kind: FileStatusKind, old_path: Option<&str>) -> Vec<PathLabelPart> {
    let part = |path: &str| {
        let (file_name, directory) = corvene_core::file_name_and_directory(path);
        PathLabelPart::Path {
            directory: crate::format::display_path(directory),
            file_name: file_name.to_string(),
        }
    };
    match (kind, old_path) {
        (FileStatusKind::Renamed | FileStatusKind::Copied, Some(old_path)) => {
            vec![part(old_path), PathLabelPart::RenameArrow, part(path)]
        }
        _ => vec![part(path)],
    }
}

/// `.path-label-component`: the [`path_label`] parts in a row, `matches`
/// (char indices into the path) bolded in the current path, directories
/// in `directory_color`, the arrow in `arrow_color`.
pub fn path_label_element(
    parts: Vec<PathLabelPart>,
    matches: Vec<usize>,
    directory_color: Hsla,
    arrow_color: Hsla,
) -> Div {
    let rename = parts.contains(&PathLabelPart::RenameArrow);
    let current = parts
        .iter()
        .rposition(|part| matches!(part, PathLabelPart::Path { .. }));
    let mut matches = Some(matches);
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_row()
        .items_center()
        .children(parts.into_iter().enumerate().map(|(ix, part)| {
            match part {
                PathLabelPart::Path {
                    directory,
                    file_name,
                } => {
                    let matches = if Some(ix) == current {
                        matches.take().unwrap_or_default()
                    } else {
                        Vec::new()
                    };
                    path_text(directory, file_name, matches, directory_color)
                        .half(rename)
                        .into_any_element()
                }
                PathLabelPart::RenameArrow => octicon(Octicon::ArrowRight, arrow_color)
                    .flex_none()
                    .mx(SPACING_HALF())
                    .into_any_element(),
            }
        }))
}

#[cfg(test)]
mod tests {
    // not `super::*`: GPUI's `test` attribute would shadow the standard one
    use super::{PathLabelPart, path_label};
    use corvene_core::FileStatusKind;

    #[test]
    fn a_copy_shows_both_paths() {
        let parts = path_label("b/new.txt", FileStatusKind::Copied, Some("a/old.txt"));
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[1], PathLabelPart::RenameArrow);
        // a rename without its old path falls back to the one path
        assert_eq!(path_label("x", FileStatusKind::Renamed, None).len(), 1);
    }
}
