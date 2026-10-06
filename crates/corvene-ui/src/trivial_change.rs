//! Corvene `1319-trivial-change-icon` (desktop/desktop#17573): a modified
//! file whose change is only its mode, or only patch metadata in a patch
//! file (`corvene_git::trivial_change`), gets a grey modified icon with a
//! tooltip saying why, so a review can skip it. GitHub Desktop
//! (`ui/octicons/status.ts`) colours every modified file the same.

use corvene_core::AppState;
use corvene_core::TrivialChange;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::icons::{Octicon, octicon};
use crate::theme::ActiveGhdTheme;
use crate::widgets::GhdTooltip;

/// Which file list a row is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileList {
    Changes,
    Commit,
    Stash,
}

/// `path`'s trivial change in `list` of repository `repo`, while the flag
/// is on.
pub fn lookup(repo: u64, list: FileList, path: &str, cx: &App) -> Option<TrivialChange> {
    let s = AppState::try_global(cx)?.read(cx);
    if !s.flags.bool(corvene_core::flags::ids::TRIVIAL_CHANGE_ICON) {
        return None;
    }
    let rs = s.repo_states.get(&repo)?;
    match list {
        FileList::Changes => rs.status.as_ref()?.trivial.get(path).copied(),
        FileList::Commit => rs.changeset.as_ref()?.trivial.get(path).copied(),
        FileList::Stash => rs.stash_trivial.get(path).copied(),
    }
}

pub fn tooltip(change: TrivialChange) -> &'static str {
    match change {
        TrivialChange::ModeOnly => "Only the file mode changed",
        TrivialChange::PatchMetadata => {
            "Only patch metadata changed (index lines, line numbers, commit ids)"
        }
    }
}

/// The row's status icon: `icon` in `color`, or for a trivial change the
/// modified icon in the secondary colour (`selected` keeps the selected
/// row's colour) with its tooltip.
pub fn status_icon(
    icon: Octicon,
    color: Hsla,
    selected: bool,
    trivial: Option<TrivialChange>,
    cx: &App,
) -> AnyElement {
    let Some(change) = trivial else {
        return octicon(icon, color).into_any_element();
    };
    let color = if selected {
        color
    } else {
        cx.ghd().text_secondary
    };
    div()
        .id("trivial-change-icon")
        .flex_none()
        .flex()
        .items_center()
        .ghd_tooltip(tooltip(change))
        .child(octicon(Octicon::DiffModified, color))
        .into_any_element()
}
