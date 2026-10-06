//! Corvene (`1310-file-list-tree`, desktop/desktop#20624 and #20529): the
//! pieces the changes list, a commit's file list and a stash's file list
//! share to show their files as a folder tree ([`corvene_core::file_tree`]).
//! GHD lists changed files flat (`ui/changes/filter-changes-list.tsx`,
//! `ui/history/file-list.tsx`).
//!
//! Rows keep GHD's 29 px height; a level indents 16 px, a folder row has a
//! chevron and a folder icon (github.com's file tree), and tree rows have
//! no bottom border.

use std::collections::BTreeSet;
use std::rc::Rc;

use corvene_core::file_tree::{FileTree, TreeRow};
use corvene_core::{AppState, Dispatcher, FileStatusKind};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::icons::{Octicon, octicon};
use crate::path_label::PathLabelPart;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::{FONT_SIZE, SPACING_HALF, zpx};
use crate::widgets::IconButtonA11y;

/// One level of indentation.
pub fn indent(depth: usize) -> Pixels {
    zpx(16. * depth as f32)
}

/// The chevron column every tree row keeps, so a file's name lines up with
/// its sibling folders' icons.
pub fn chevron_width() -> Pixels {
    zpx(16.)
}

/// Where a file row's label starts in a tree, after the row's gap: past
/// the indentation and the chevron column (and its gap).
pub fn file_label_margin(depth: usize) -> Pixels {
    indent(depth) + chevron_width() + SPACING_HALF()
}

/// Whether file lists show as trees: the flag, and View › Show Changes as
/// Tree.
pub fn tree_mode(cx: &App) -> bool {
    AppState::try_global(cx).is_some_and(|s| {
        let s = s.read(cx);
        s.flags.bool(corvene_core::flags::ids::FILE_LIST_TREE) && s.settings.file_list_tree
    })
}

/// Whether the flag offers the tree at all.
pub fn tree_available(cx: &App) -> bool {
    AppState::try_global(cx).is_some_and(|s| {
        s.read(cx)
            .flags
            .bool(corvene_core::flags::ids::FILE_LIST_TREE)
    })
}

/// A file row's label in a tree: its name, and for a rename or copy the old
/// name (the old path when it moved to another folder), an arrow and the
/// new name.
pub fn tree_label(path: &str, kind: FileStatusKind, old_path: Option<&str>) -> Vec<PathLabelPart> {
    let (name, directory) = corvene_core::file_name_and_directory(path);
    let name = PathLabelPart::Path {
        directory: String::new(),
        file_name: name.to_string(),
    };
    match (kind, old_path) {
        (FileStatusKind::Renamed | FileStatusKind::Copied, Some(old)) => {
            let (old_name, old_directory) = corvene_core::file_name_and_directory(old);
            let old_directory = if old_directory == directory {
                String::new()
            } else {
                crate::format::display_path(old_directory)
            };
            vec![
                PathLabelPart::Path {
                    directory: old_directory,
                    file_name: old_name.to_string(),
                },
                PathLabelPart::RenameArrow,
                name,
            ]
        }
        _ => vec![name],
    }
}

/// A tree over a list's paths: the paths, the collapse key it was built
/// for, the tree.
pub type PathTree = Rc<(Vec<String>, u64, FileTree)>;

/// Collapse state the History and stash lists keep while the app runs (the
/// changes list's is saved per repository).
#[derive(Clone, Default)]
pub struct SessionCollapse {
    folders: BTreeSet<String>,
    /// Bumped by every change (keys the cached tree).
    generation: u64,
}

impl SessionCollapse {
    pub fn set(&mut self, folders: impl IntoIterator<Item = String>, collapsed: bool) {
        for folder in folders {
            if collapsed {
                self.folders.insert(folder);
            } else {
                self.folders.remove(&folder);
            }
        }
        self.generation += 1;
    }

    pub fn contains(&self, folder: &str) -> bool {
        self.folders.contains(folder)
    }

    pub fn key(&self) -> u64 {
        self.generation
    }
}

/// The tree state of a list whose collapse state lives with the view
/// (a commit's or a stash's files): collapsed folders, the folder row the
/// user put the cursor on, and the tree of the last paths.
#[derive(Default)]
pub struct ListTree {
    pub collapse: SessionCollapse,
    /// The folder row the user put the cursor on, for the paths of
    /// [`Self::paths_generation`] it was set on.
    cursor: Option<(String, u64)>,
    cache: std::cell::RefCell<Option<PathTree>>,
    /// Bumped whenever the tree is built for other paths (another commit or
    /// stash), which drops the cursor.
    paths_generation: std::cell::Cell<u64>,
}

impl ListTree {
    /// The tree over `paths` (rebuilt when they or the collapsed folders
    /// change; compared without copying them).
    pub fn tree<'a>(&self, paths: impl ExactSizeIterator<Item = &'a str> + Clone) -> PathTree {
        let collapsed = self.collapse.key();
        let same_paths = |c: &PathTree| {
            c.0.len() == paths.len() && c.0.iter().map(String::as_str).eq(paths.clone())
        };
        let cached = self.cache.borrow().clone();
        if let Some(hit) = &cached
            && same_paths(hit)
        {
            if hit.1 == collapsed {
                return hit.clone();
            }
        } else {
            self.paths_generation.set(self.paths_generation.get() + 1);
        }
        let owned: Vec<String> = paths.map(str::to_string).collect();
        let refs: Vec<&str> = owned.iter().map(String::as_str).collect();
        let tree = FileTree::build(&refs, |folder| self.collapse.contains(folder));
        let built = Rc::new((owned, collapsed, tree));
        *self.cache.borrow_mut() = Some(built.clone());
        built
    }

    /// Put the cursor on folder `path` of the current tree.
    pub fn set_cursor(&mut self, path: String) {
        self.cursor = Some((path, self.paths_generation.get()));
    }

    pub fn clear_cursor(&mut self) {
        self.cursor = None;
    }

    /// The row of the cursor's folder, while it is in `tree` and the paths
    /// are the ones it was set on.
    pub fn cursor_row(&self, tree: &FileTree) -> Option<usize> {
        let (cursor, generation) = self.cursor.as_ref()?;
        if *generation != self.paths_generation.get() {
            return None;
        }
        tree.rows
            .iter()
            .position(|r| matches!(r, TreeRow::Folder { path, .. } if path == cursor))
    }
}

/// The folder paths of `tree`'s folder rows' files of `paths` (Expand All /
/// Collapse All).
pub fn all_folders(paths: &[String]) -> Vec<String> {
    let refs: Vec<&str> = paths.iter().map(String::as_str).collect();
    FileTree::folder_paths(&refs)
}

/// The items of `row` as paths.
pub fn row_paths(tree: &FileTree, row: usize, paths: &[String]) -> Vec<String> {
    tree.row_items(row)
        .iter()
        .filter_map(|&i| paths.get(i).cloned())
        .collect()
}

/// A folder row of a list without checkboxes (a commit's or a stash's
/// files). `on_select` runs on a press, `on_toggle` on the chevron or a
/// double-click, `on_menu` on a right-click with the pointer position.
#[allow(clippy::too_many_arguments)]
pub fn plain_folder_row(
    id: SharedString,
    group: &'static str,
    row: &TreeRow,
    is_selected: bool,
    list_focused: bool,
    on_select: impl Fn(&mut Window, &mut App) + 'static,
    on_toggle: impl Fn(&mut Window, &mut App) + Clone + 'static,
    on_menu: impl Fn(Point<Pixels>, &mut Window, &mut App) + 'static,
    menu_anchor: Option<&crate::context_menu::RowMenuAnchor>,
    cx: &App,
) -> AnyElement {
    use crate::widgets::ListRowA11y;
    let TreeRow::Folder {
        path,
        label,
        depth,
        items,
        expanded,
    } = row
    else {
        return div().into_any_element();
    };
    let t = cx.ghd();
    let hover_bg = t.list_item_hover_background;
    let text_color = match (is_selected, list_focused) {
        (true, true) => Some(t.box_selected_active_text),
        (true, false) => Some(t.box_selected_text),
        _ => None,
    };
    let on_select = Rc::new(on_select);
    let toggle_on_double = on_toggle.clone();
    div()
        .id(id)
        .group(group)
        .a11y_row(
            format!(
                "{path}, folder, {} {}",
                items.len(),
                if items.len() == 1 { "file" } else { "files" }
            ),
            is_selected,
        )
        .w_full()
        .h(crate::theme::sizes::ROW_HEIGHT())
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(SPACING_HALF())
        .px(crate::theme::sizes::SPACING())
        .cursor_pointer()
        .on_mouse_down(MouseButton::Right, {
            let on_select = on_select.clone();
            move |ev: &MouseDownEvent, window, cx| {
                cx.stop_propagation();
                if !is_selected {
                    on_select(window, cx);
                }
                on_menu(ev.position, window, cx);
            }
        })
        .on_mouse_down(MouseButton::Left, {
            let on_select = on_select.clone();
            move |_: &MouseDownEvent, window, cx| on_select(window, cx)
        })
        .on_click(move |ev: &ClickEvent, window, cx| {
            if ev.click_count() == 2 {
                toggle_on_double(window, cx);
            }
        })
        .when(is_selected, |d| {
            if list_focused {
                d.bg(t.box_selected_active_background)
                    .text_color(t.box_selected_active_text)
            } else {
                d.bg(t.box_selected_background)
                    .text_color(t.box_selected_text)
            }
        })
        .when(
            !(is_selected && (list_focused || crate::widgets::selection_keeps_colour_on_hover(cx))),
            move |d| d.hover(move |s| s.bg(hover_bg)),
        )
        .children(folder_cells(
            label,
            *depth,
            items.len(),
            *expanded,
            text_color,
            on_toggle,
            cx,
        ))
        .when_some(menu_anchor, |d, anchor| d.child(anchor.track()))
        .into_any_element()
}

/// The menu of a folder row in a list without checkboxes: copy its path,
/// reveal it, expand or collapse every folder.
pub fn plain_folder_menu(
    repo: &std::path::Path,
    folder: &str,
    folders: Vec<String>,
    on_collapse: impl Fn(Vec<String>, bool, &mut App) + Clone + 'static,
) -> Vec<crate::context_menu::MenuItem> {
    use crate::context_menu::{MenuItem, labels, mac_or};
    let full = repo.join(folder);
    let absolute = full.to_string_lossy().into_owned();
    let relative = folder.to_string();
    let expand = folders.clone();
    let collapse_all = on_collapse.clone();
    vec![
        MenuItem::new(
            mac_or("Copy Folder Path", "Copy folder path"),
            move |_, cx| cx.write_to_clipboard(ClipboardItem::new_string(absolute.clone())),
        ),
        MenuItem::new(
            mac_or("Copy Relative Folder Path", "Copy relative folder path"),
            move |_, cx| cx.write_to_clipboard(ClipboardItem::new_string(relative.clone())),
        ),
        MenuItem::new(labels::REVEAL_IN_FILE_MANAGER, move |_, cx| {
            Dispatcher::show_in_finder(&full, cx)
        }),
        MenuItem::separator(),
        MenuItem::new(mac_or("Expand All", "Expand all"), move |_, cx| {
            on_collapse(expand.clone(), false, cx)
        }),
        MenuItem::new(mac_or("Collapse All", "Collapse all"), move |_, cx| {
            collapse_all(folders.clone(), true, cx)
        }),
    ]
}

/// The row a keyboard move starts from: `cursor` (a folder row the user
/// put the selection on) while its files are still the selection, else the
/// row showing the last selected item.
pub fn current_row(
    tree: &FileTree,
    cursor: Option<&str>,
    selected_items: usize,
    last_selected_item: Option<usize>,
) -> Option<usize> {
    if let Some(cursor) = cursor
        && let Some(row) = tree.rows.iter().position(
            |r| matches!(r, TreeRow::Folder { path, items, .. } if path == cursor && items.len() == selected_items),
        )
    {
        return Some(row);
    }
    last_selected_item.and_then(|i| tree.row_of_item(i))
}

/// What ← does on `row`: collapse an expanded folder, else go to the
/// parent folder's row.
pub enum LeftKey {
    Collapse(String),
    Select(usize),
    Nothing,
}

pub fn left_key(tree: &FileTree, row: usize) -> LeftKey {
    match tree.rows.get(row) {
        Some(TreeRow::Folder {
            path,
            expanded: true,
            ..
        }) => LeftKey::Collapse(path.clone()),
        Some(_) => tree.parent(row).map_or(LeftKey::Nothing, LeftKey::Select),
        None => LeftKey::Nothing,
    }
}

/// `619-arrow-keys-between-panes`: ← or → the tree has no use for moves to
/// the pane on that side, as in a flat list.
pub fn focus_pane(right: bool, window: &mut Window, cx: &mut App) {
    let on = AppState::try_global(cx).is_some_and(|s| {
        s.read(cx)
            .flags
            .bool(corvene_core::flags::ids::ARROW_KEYS_BETWEEN_PANES)
    });
    if on {
        if right {
            window.dispatch_action(Box::new(crate::actions::FocusPaneRight), cx);
        } else {
            window.dispatch_action(Box::new(crate::actions::FocusPaneLeft), cx);
        }
    }
}

/// What → does on `row`: expand a collapsed folder, else go to an expanded
/// folder's first row. `None` on a file.
pub enum RightKey {
    Expand(String),
    Select(usize),
}

pub fn right_key(tree: &FileTree, row: usize) -> Option<RightKey> {
    match tree.rows.get(row)? {
        TreeRow::Folder {
            path,
            expanded: false,
            ..
        } => Some(RightKey::Expand(path.clone())),
        TreeRow::Folder { .. } => Some(RightKey::Select(row + 1)),
        TreeRow::File { .. } => None,
    }
}

/// `getCheckAllValue` of `kinds`: checked, unchecked or mixed.
pub fn check_value(mut kinds: impl Iterator<Item = Option<bool>>) -> Option<bool> {
    let Some(first) = kinds.next() else {
        return Some(true);
    };
    let mut value = first;
    for kind in kinds {
        if kind != value {
            value = None;
        }
        if value.is_none() {
            break;
        }
    }
    value
}

/// A folder row's chevron, icon, name and file count. `on_toggle` runs for
/// a press on the chevron (the rest of the row selects).
pub fn folder_cells(
    label: &str,
    depth: usize,
    count: usize,
    expanded: bool,
    text_color: Option<Hsla>,
    on_toggle: impl Fn(&mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoIterator<Item = AnyElement> {
    let t = cx.ghd();
    let secondary = text_color.unwrap_or(t.text_secondary);
    let icon_color = text_color.unwrap_or(t.text_secondary);
    [
        div()
            .id("tree-chevron")
            .ml(indent(depth))
            .w(chevron_width())
            .h_full()
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                cx.stop_propagation();
                on_toggle(window, cx);
            })
            .child(
                octicon(
                    if expanded {
                        Octicon::ChevronDown
                    } else {
                        Octicon::ChevronRight
                    },
                    secondary,
                )
                .size(zpx(12.)),
            )
            .into_any_element(),
        octicon(
            if expanded {
                Octicon::FileDirectoryOpenFill
            } else {
                Octicon::FileDirectoryFill
            },
            icon_color,
        )
        .into_any_element(),
        div()
            .flex_1()
            .min_w_0()
            .text_size(FONT_SIZE())
            .truncate()
            .child(label.to_string())
            .into_any_element(),
        div()
            .flex_none()
            .ml(SPACING_HALF())
            .text_size(zpx(11.))
            .text_color(secondary)
            .child(count.to_string())
            .into_any_element(),
    ]
}

/// The button at the end of the "N changed files" row that switches every
/// file list between the flat list and the tree.
pub fn view_toggle_button(cx: &App) -> AnyElement {
    let t = cx.ghd();
    let tree = tree_mode(cx);
    let (icon, label) = if tree {
        (Octicon::ListUnordered, "Show as List")
    } else {
        (Octicon::FileDirectory, "Show as Tree")
    };
    let hover = t.text;
    let icon_color = t.text_secondary;
    crate::widgets::with_directed_tooltip(
        div().id("file-list-tree-toggle").a11y_button(label),
        label,
        crate::widgets::TooltipDirection::North,
    )
    .flex_none()
    .ml_auto()
    .size(zpx(18.))
    .flex()
    .items_center()
    .justify_center()
    .rounded(zpx(3.))
    .cursor_pointer()
    .group("file-list-tree-toggle")
    .on_click(move |_, _, cx| Dispatcher::set_file_list_tree(!tree, cx))
    .child(
        octicon(icon, icon_color)
            .size(zpx(14.))
            .group_hover("file-list-tree-toggle", move |s| s.text_color(hover)),
    )
    .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{check_value, tree_label};
    use crate::path_label::PathLabelPart;
    use corvene_core::FileStatusKind;

    #[::core::prelude::v1::test]
    fn a_rename_within_its_folder_shows_names_only() {
        let parts = tree_label("a/new.rs", FileStatusKind::Renamed, Some("a/old.rs"));
        assert_eq!(
            parts[0],
            PathLabelPart::Path {
                directory: String::new(),
                file_name: "old.rs".into()
            }
        );
        let moved = tree_label("b/new.rs", FileStatusKind::Renamed, Some("a/old.rs"));
        let PathLabelPart::Path { directory, .. } = &moved[0] else {
            panic!()
        };
        assert!(!directory.is_empty());
    }

    #[::core::prelude::v1::test]
    fn folder_checkboxes_mix() {
        assert_eq!(
            check_value([Some(true), Some(true)].into_iter()),
            Some(true)
        );
        assert_eq!(check_value([Some(true), Some(false)].into_iter()), None);
        assert_eq!(check_value([None].into_iter()), None);
        assert_eq!(check_value([].into_iter()), Some(true));
    }
}
