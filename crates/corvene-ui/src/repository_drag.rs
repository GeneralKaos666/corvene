//! Corvene (`426-drag-repository-out`): the toolbar's repository button and
//! the repository list's rows can be dragged out of the window as the
//! repository's folder, onto Finder, the Dock, an editor or Terminal (GPUI
//! hands a drag that leaves the window to AppKit as a file drag). Inside the
//! window the drag shows the repository's name and drops nowhere. GHD has no
//! drag source for repositories.

use std::path::PathBuf;

use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::icons::{Octicon, octicon};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;

/// What a repository drag carries.
#[derive(Clone, Debug)]
pub struct RepositoryDrag {
    pub path: PathBuf,
    pub name: SharedString,
}

/// The drag's look while it is still inside the window.
pub struct RepositoryDragPreview {
    name: SharedString,
}

impl Render for RepositoryDragPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(SPACING_HALF())
            .px(SPACING())
            .py(SPACING_HALF())
            .rounded(BORDER_RADIUS())
            .border_1()
            .border_color(t.box_border)
            .bg(t.box_background)
            .text_color(t.text)
            .text_size(FONT_SIZE())
            .shadow_md()
            .child(octicon(Octicon::FileDirectory, t.text))
            .child(self.name.clone())
    }
}

/// The flag is on (and the platform can start file drags).
pub fn enabled(cx: &App) -> bool {
    cfg!(target_os = "macos")
        && corvene_core::AppState::try_global(cx).is_some_and(|s| {
            s.read(cx)
                .flags
                .bool(corvene_core::flags::ids::DRAG_REPOSITORY_OUT)
        })
}

/// `element` drags the repository folder at `path` out of the window.
pub fn draggable(element: Stateful<Div>, path: PathBuf, name: SharedString) -> Stateful<Div> {
    element
        .on_drag(RepositoryDrag { path, name }, |drag, _, _, cx| {
            let name = drag.name.clone();
            cx.new(|_| RepositoryDragPreview { name })
        })
        .external_drag_payload(|drag: &RepositoryDrag, _, _| {
            Some(ExternalDragPayload::Files(FileDragPaths::new([(
                drag.path.clone(),
                true,
            )])))
        })
}
