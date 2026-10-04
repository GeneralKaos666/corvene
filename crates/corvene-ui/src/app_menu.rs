//! The menu bar's template, mirroring GitHub Desktop 3.6.6's
//! `buildDefaultMenuTemplate` (`app/src/main-process/menu/build-default-menu.ts`)
//! over its `MenuLabelsEvent` (`app/src/models/menu-labels.ts`): native on
//! macOS, the model of the in-window menu bar (`menu_bar`, `views_menu`)
//! elsewhere. Items are GHD's, `__DARWIN__` branch on macOS and the other
//! one elsewhere (sentence case with `&` access keys).
//!
//! [`build_default_menu_template`] is pure, so the labels a state produces
//! can be checked without an `App`; [`build_default_menu`] turns the
//! template into the GPUI menus `crates/corvene/src/menus.rs` installs.
//! Keyboard shortcuts are shown from the keymap in [`crate::keymap`]. GPUI
//! disables any item whose action has no live handler, which matches GHD's
//! context-dependent enabling. The Repository menu's push item keeps the
//! `Push` action when it reads Force Push; the binary's handler force-pushes
//! whenever a force push is possible, which is GHD's `force-push` event.
//!
//! [`MenuLabelsEvent::of`] is GHD `updateMenuItemLabels` (`app-store.ts`),
//! except:
//! - `askForConfirmationWhenStashingAllChanges` is always false: Corvene's
//!   Stash All Changes replaces an existing stash without GHD's
//!   `ConfirmOverwriteStash` warning, so its label has no ellipsis.
//! - `isChangesFilterVisible` stays at GHD's default (shown): whether the
//!   filter is shown lives in the Changes view (`changes.rs`), not in
//!   `AppState`.
//! - The editor and shell are named by `AppState::editor_label` /
//!   `shell_label`, which name a custom one (GHD's `null` reads "External
//!   Editor" / "Shell").
//!
//! Corvene additions ([`MenuExtras`], all off in the github-desktop preset):
//! "Flags…" (no GHD equivalent, always there), File › Import Repositories
//! from GitHub Desktop…, Repository › Fetch All Repositories, Repository ›
//! Fetch All Tags (flag `899-tags-in-branch-list`), Repository ›
//! View Upstream on GitHub, Repository › Add License… ("A&dd license…" off
//! macOS: `Pu&ll` has the `l`), View › Show Pull Requests List and Toggle
//! History Review Mode, File › Remove Repositories… (flag
//! `269-bulk-remove-repositories`), Edit › Undo Last Commit (flag
//! `423-undo-commit-menu-item`, enabled while the Changes tab's Undo bar
//! shows), Window › Corvene (shows the window hidden with ⌘W)
//! and Help › Show Release Notes; off macOS "Flags…" and "Install Command
//! Line Tool…" sit under File after "Options…" (GHD's Linux menu has no app
//! menu). On Android, Repository › Move to shared storage… follows Remove.

use corvene_core::AppState;
use corvene_core::flags::{Flags, ids};
use gpui_kit::{Action, Menu, MenuItem, OsAction, SystemMenuType};

use crate::actions::*;
use crate::keymap::KeymapFlags;

/// GHD `MenuLabelsEvent` (`models/menu-labels.ts`): what the menu's labels
/// depend on, plus Corvene's flag-dependent items. A change rebuilds the
/// menu bar. `Default` is GHD's defaults for the optional fields with
/// nothing selected and every Corvene addition off.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuLabelsEvent {
    /// GHD `selectedShell` (`None`: "Shell")
    pub selected_shell: Option<String>,
    /// GHD `selectedExternalEditor` (`None`: "External Editor")
    pub selected_external_editor: Option<String>,
    pub ask_for_confirmation_on_force_push: bool,
    pub ask_for_confirmation_on_repository_removal: bool,
    /// `findContributionTargetDefaultBranch`'s name (`None`: "Default Branch")
    pub contribution_target_default_branch: Option<String>,
    /// `getCurrentBranchForcePushState(…) !== NotAvailable`
    pub is_force_push_for_current_repository: bool,
    /// `branchesState.currentPullRequest !== null`
    pub has_current_pull_request: bool,
    /// The Changes list shows the stash (`ChangesSelectionKind.Stash`)
    pub is_stashed_changes_visible: bool,
    /// GHD: `changesState.stashEntry !== null` (see the module docs)
    pub ask_for_confirmation_when_stashing_all_changes: bool,
    pub is_changes_filter_visible: bool,
    /// Corvene's flag-dependent items.
    pub extras: MenuExtras,
}

impl Default for MenuLabelsEvent {
    fn default() -> Self {
        Self {
            selected_shell: None,
            selected_external_editor: None,
            ask_for_confirmation_on_force_push: false,
            ask_for_confirmation_on_repository_removal: false,
            contribution_target_default_branch: None,
            is_force_push_for_current_repository: false,
            has_current_pull_request: false,
            is_stashed_changes_visible: false,
            ask_for_confirmation_when_stashing_all_changes: true,
            is_changes_filter_visible: true,
            extras: MenuExtras::default(),
        }
    }
}

/// The menu items Corvene's flags add (`Default`: none, the github-desktop
/// preset).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MenuExtras {
    /// Flag `401-release-notes-menu-item`.
    pub show_release_notes: bool,
    /// Flag `206-import-from-github-desktop`.
    pub show_import: bool,
    /// Flag `321-view-upstream-on-github`.
    pub show_view_upstream: bool,
    /// Flag `405-window-menu-main-window`.
    pub show_main_window: bool,
    /// Flag `221-add-license`.
    pub show_add_license: bool,
    /// Flag `247-fetch-all-repositories`.
    pub fetch_all: bool,
    /// Flag `899-tags-in-branch-list`: Repository › Fetch All Tags.
    pub fetch_tags: bool,
    /// Flag `414-linux-install-cli` (the item is always there on macOS).
    pub install_cli: bool,
    /// Flag `269-bulk-remove-repositories`.
    pub show_remove_repositories: bool,
    /// Flag `423-undo-commit-menu-item`: Edit › Undo Last Commit, enabled
    /// while the Changes tab's Undo bar shows (set by
    /// [`MenuLabelsEvent::of`]).
    pub undo_last_commit: Option<bool>,
    /// Flags that add key bindings and their View menu items
    /// (`612-navigation-shortcuts`, `801-history-review-mode`).
    pub keymap: KeymapFlags,
}

impl MenuExtras {
    pub fn of(flags: &Flags) -> Self {
        Self {
            show_release_notes: flags.bool(ids::RELEASE_NOTES_MENU_ITEM),
            // GitHub Desktop does not run on Android
            show_import: !cfg!(target_os = "android")
                && flags.bool(ids::IMPORT_FROM_GITHUB_DESKTOP),
            show_view_upstream: flags.bool(ids::VIEW_UPSTREAM_ON_GITHUB),
            show_main_window: flags.bool(ids::WINDOW_MENU_MAIN_WINDOW),
            show_add_license: flags.bool(ids::ADD_LICENSE),
            fetch_all: flags.bool(ids::FETCH_ALL_REPOSITORIES),
            fetch_tags: flags.bool(ids::TAGS_IN_BRANCH_LIST),
            // Windows: the installer puts the command line tool on the PATH
            // (GHD has no menu item for it there either)
            install_cli: !cfg!(any(target_os = "android", windows))
                && flags.bool(ids::LINUX_INSTALL_CLI),
            show_remove_repositories: flags.bool(ids::BULK_REMOVE_REPOSITORIES),
            undo_last_commit: flags.bool(ids::UNDO_COMMIT_MENU_ITEM).then_some(false),
            keymap: KeymapFlags::from_flags(flags),
        }
    }
}

impl MenuLabelsEvent {
    /// GHD `updateMenuLabelsForSelectedRepository` / `updateMenuItemLabels`
    /// (see the module docs for the differences).
    pub fn of(s: &AppState) -> Self {
        let mut extras = MenuExtras::of(&s.flags);
        // `423-undo-commit-menu-item`: while the Undo bar shows
        if extras.undo_last_commit.is_some() {
            extras.undo_last_commit = Some(
                s.selected_state()
                    .is_some_and(|rs| rs.last_commit.is_some()),
            );
        }
        let labels = Self {
            selected_shell: Some(s.shell_label()),
            selected_external_editor: Some(s.editor_label()),
            ask_for_confirmation_on_force_push: s.settings.confirm_force_push,
            ask_for_confirmation_on_repository_removal: s.settings.confirm_repository_removal,
            // Stash All Changes does not ask before replacing a stash
            ask_for_confirmation_when_stashing_all_changes: false,
            extras,
            ..Self::default()
        };
        // a missing repository is not a `SelectionType.Repository`
        let Some(repository) = s.selected_repository().filter(|r| !r.missing) else {
            return labels;
        };
        let Some(rs) = s.repo_states.get(&repository.id) else {
            return labels;
        };
        let configured_default = s
            .global_git
            .as_ref()
            .map(|g| g.default_branch.as_str())
            .filter(|b| !b.is_empty())
            .unwrap_or("main");
        let branches = rs
            .info
            .as_ref()
            .map(|i| i.branches.as_slice())
            .unwrap_or(&[]);
        let contribution_target = corvene_core::menu_state::contribution_target_default_branch(
            repository,
            branches,
            rs.default_branch.as_deref(),
            configured_default,
        );
        Self {
            contribution_target_default_branch: contribution_target.map(str::to_string),
            // the menu offers a force push whenever one is possible
            is_force_push_for_current_repository: corvene_core::Dispatcher::force_push_state_in(
                s,
                repository.id,
            ) != corvene_core::ForcePushState::NotAvailable,
            is_stashed_changes_visible: rs.showing_stash,
            has_current_pull_request: s.current_pull_request(repository.id).is_some(),
            ..labels
        }
    }
}

/// GHD's `Electron.MenuItemConstructorOptions`, the fields Corvene uses: a
/// separator, an item that runs an action, a submenu or a system menu.
#[derive(Default)]
pub struct MenuItemConstructorOptions {
    pub label: Option<String>,
    /// `type: 'separator'`
    pub separator: bool,
    /// `visible` (`None` is visible)
    pub visible: Option<bool>,
    pub submenu: Option<Vec<MenuItemConstructorOptions>>,
    /// `click` / `role`: the action the item dispatches.
    pub action: Option<Box<dyn Action>>,
    /// The `role` the OS knows (undo, copy, …).
    pub os_action: Option<OsAction>,
    /// `role: 'services'` and the like.
    pub system_menu: Option<SystemMenuType>,
    /// `enabled` (`None` is enabled)
    pub enabled: Option<bool>,
}

/// GHD's label for this platform: `__DARWIN__ ? mac : other`. Off macOS
/// labels are sentence case and carry `&` access keys (Electron's classic
/// menu bar underlines the letter after `&`).
const fn l(mac: &'static str, other: &'static str) -> &'static str {
    if cfg!(target_os = "macos") {
        mac
    } else {
        other
    }
}

/// The `__DARWIN__ ? mac : other` choice for built labels.
fn l_owned(mac: String, other: String) -> String {
    if cfg!(target_os = "macos") {
        mac
    } else {
        other
    }
}

fn item(label: impl Into<String>, action: impl Action) -> MenuItemConstructorOptions {
    MenuItemConstructorOptions {
        label: Some(label.into()),
        action: Some(Box::new(action)),
        ..Default::default()
    }
}

fn os_item(label: &str, action: impl Action, os_action: OsAction) -> MenuItemConstructorOptions {
    MenuItemConstructorOptions {
        os_action: Some(os_action),
        ..item(label, action)
    }
}

fn separator() -> MenuItemConstructorOptions {
    MenuItemConstructorOptions {
        separator: true,
        ..Default::default()
    }
}

fn submenu(label: &str, items: Vec<MenuItemConstructorOptions>) -> MenuItemConstructorOptions {
    MenuItemConstructorOptions {
        label: Some(label.to_string()),
        submenu: Some(items),
        ..Default::default()
    }
}

/// GHD `getPushLabel`.
fn push_label(
    is_force_push_for_current_repository: bool,
    ask_for_confirmation: bool,
) -> &'static str {
    if !is_force_push_for_current_repository {
        l("Push", "P&ush")
    } else if ask_for_confirmation {
        l("Force Push…", "Force P&ush…")
    } else {
        l("Force Push", "Force P&ush")
    }
}

/// GHD `getStashedChangesLabel`.
fn stashed_changes_label(is_stashed_changes_visible: bool) -> &'static str {
    if is_stashed_changes_visible {
        l("Hide Stashed Changes", "H&ide stashed changes")
    } else {
        l("Show Stashed Changes", "Sho&w stashed changes")
    }
}

/// GHD `buildDefaultMenuTemplate(labels)`: the top-level menus with their
/// items (see the module docs).
pub fn build_default_menu_template(labels: &MenuLabelsEvent) -> Vec<MenuItemConstructorOptions> {
    let extras = labels.extras;
    let contribution_target_default_branch = corvene_core::notifications::truncate_with_ellipsis(
        labels
            .contribution_target_default_branch
            .as_deref()
            .unwrap_or(l("Default Branch", "default branch")),
        25,
    );
    let remove_repo_label = if labels.ask_for_confirmation_on_repository_removal {
        l("Remove…", "&Remove…")
    } else {
        l("Remove", "&Remove")
    };
    let pull_request_label = if labels.has_current_pull_request {
        l(
            "View Pull Request on GitHub",
            "View &pull request on GitHub",
        )
    } else {
        l("Create Pull Request", "Create &pull request")
    };

    let mut template = Vec::new();

    if cfg!(target_os = "macos") {
        template.push(submenu(
            "Corvene",
            vec![
                item("About Corvene", About),
                separator(),
                item("Settings…", OpenSettings),
                item("Flags…", OpenFlags),
                item("Install Command Line Tool…", InstallCli),
                separator(),
                MenuItemConstructorOptions {
                    label: Some("Services".to_string()),
                    system_menu: Some(SystemMenuType::Services),
                    ..Default::default()
                },
                separator(),
                item("Hide Corvene", Hide),
                item("Hide Others", HideOthers),
                item("Show All", ShowAll),
                separator(),
                item("Quit Corvene", Quit),
            ],
        ));
    }

    let mut file_items = vec![
        item(l("New Repository…", "New &repository…"), NewRepository),
        separator(),
        item(
            l("Add Local Repository…", "Add &local repository…"),
            AddLocalRepository,
        ),
        item(
            l("Clone Repository…", "Clo&ne repository…"),
            CloneRepository,
        ),
    ];
    if extras.show_import {
        file_items.push(item(
            l(
                "Import Repositories from GitHub Desktop…",
                "&Import repositories from GitHub Desktop…",
            ),
            ImportFromGitHubDesktop,
        ));
    }
    if extras.show_remove_repositories {
        file_items.extend([
            separator(),
            item(
                l("Remove Repositories…", "Remo&ve repositories…"),
                RemoveRepositories,
            ),
        ]);
    }
    if !cfg!(target_os = "macos") {
        file_items.extend([
            separator(),
            item("&Options…", OpenSettings),
            item("Fla&gs…", OpenFlags),
        ]);
        if extras.install_cli {
            file_items.push(item("Install command line &tool…", InstallCli));
        }
        // Android applications are left, not quit
        if !cfg!(target_os = "android") {
            file_items.extend([separator(), item("E&xit", Quit)]);
        }
    }
    template.push(submenu(l("File", "&File"), file_items));

    let mut edit_items = vec![
        os_item(l("Undo", "&Undo"), Undo, OsAction::Undo),
        os_item(l("Redo", "&Redo"), Redo, OsAction::Redo),
        separator(),
        os_item(l("Cut", "Cu&t"), Cut, OsAction::Cut),
        os_item(l("Copy", "&Copy"), Copy, OsAction::Copy),
        os_item(l("Paste", "&Paste"), Paste, OsAction::Paste),
        os_item(
            l("Select All", "Select &all"),
            SelectAll,
            OsAction::SelectAll,
        ),
        separator(),
        item(l("Find", "&Find"), Find),
    ];
    if let Some(enabled) = extras.undo_last_commit {
        edit_items.extend([
            separator(),
            MenuItemConstructorOptions {
                enabled: Some(enabled),
                ..item(l("Undo Last Commit", "Undo last &commit"), UndoLastCommit)
            },
        ]);
    }
    template.push(submenu(l("Edit", "&Edit"), edit_items));

    let mut view = vec![
        item(l("Show Changes", "&Changes"), ShowChanges),
        item(l("Show History", "&History"), ShowHistory),
        item(
            l("Show Repository List", "Repository &list"),
            ShowRepositoryList,
        ),
        item(l("Show Branches List", "&Branches list"), ShowBranchesList),
        item(
            l("Show Worktrees List", "Wor&ktrees list"),
            ShowWorktreesList,
        ),
    ];
    // Corvene (`612-navigation-shortcuts`)
    if extras.keymap.navigation_shortcuts {
        view.push(item(
            l("Show Pull Requests List", "&Pull requests list"),
            ShowPullRequestsList,
        ));
    }
    // Corvene (`801-history-review-mode`)
    if extras.keymap.history_review_mode {
        view.push(item(
            l("Toggle History Review Mode", "Toggle history &review mode"),
            ToggleHistoryReviewMode,
        ));
    }
    let filter_verb = if labels.is_changes_filter_visible {
        "Hide"
    } else {
        "Show"
    };
    view.extend([
        separator(),
        item(l("Go to Summary", "Go to &Summary"), GoToSummary),
        item(
            stashed_changes_label(labels.is_stashed_changes_visible),
            ToggleStashedChanges,
        ),
        // GHD's non-macOS label really reads "… Toggle Changes Filter"
        item(
            l_owned(
                format!("{filter_verb} Changes Filter"),
                format!("{filter_verb} Toggle Chan&ges Filter"),
            ),
            ToggleChangesFilter,
        ),
    ]);
    // Corvene's macOS menu sets Toggle Full Screen apart; GHD has no
    // separator before it (kept on macOS, GHD's layout elsewhere)
    if cfg!(target_os = "macos") {
        view.push(separator());
    }
    view.extend([
        item(
            l("Toggle Full Screen", "Toggle &full screen"),
            ToggleFullScreen,
        ),
        separator(),
        item(l("Reset Zoom", "Reset zoom"), ResetZoom),
        item(l("Zoom In", "Zoom in"), ZoomIn),
        item(l("Zoom Out", "Zoom out"), ZoomOut),
        item(
            l("Expand Active Resizable", "Expand active resizable"),
            ExpandActiveResizable,
        ),
        item(
            l("Contract Active Resizable", "Contract active resizable"),
            ContractActiveResizable,
        ),
    ]);
    template.push(submenu(l("View", "&View"), view));

    let mut repository = vec![
        item(
            push_label(
                labels.is_force_push_for_current_repository,
                labels.ask_for_confirmation_on_force_push,
            ),
            Push,
        ),
        item(l("Pull", "Pu&ll"), Pull),
        item(l("Fetch", "&Fetch"), Fetch),
    ];
    if extras.fetch_tags {
        repository.push(item(l("Fetch All Tags", "Fetch all ta&gs"), FetchAllTags));
    }
    if extras.fetch_all {
        repository.push(item(
            l("Fetch All Repositories", "Fetch &all repositories"),
            FetchAllRepositories,
        ));
    }
    repository.push(item(remove_repo_label, RemoveRepository));
    // Android: a repository in Corvene's own storage can move to shared
    // storage, where Termux reaches it (`corvene_core::shared_storage`)
    if cfg!(target_os = "android") {
        repository.push(item("&Move to shared storage…", MoveToSharedStorage));
    }
    repository.extend([
        separator(),
        item(l("View on GitHub", "&View on GitHub"), ViewOnGitHub),
    ]);
    if extras.show_view_upstream {
        repository.push(item(
            l("View Upstream on GitHub", "View upstream on GitHub"),
            ViewUpstreamOnGitHub,
        ));
    }
    let shell = labels.selected_shell.as_deref();
    let editor = labels.selected_external_editor.as_deref();
    repository.extend([
        item(
            l_owned(
                format!("Open in {}", shell.unwrap_or("Shell")),
                format!("O&pen in {}", shell.unwrap_or("shell")),
            ),
            OpenInShell,
        ),
        item(
            if cfg!(windows) {
                "Show in E&xplorer"
            } else {
                l("Show in Finder", "Show in your File Manager")
            },
            ShowInFinder,
        ),
        item(
            l_owned(
                format!("Open in {}", editor.unwrap_or("External Editor")),
                format!("&Open in {}", editor.unwrap_or("external editor")),
            ),
            OpenInEditor,
        ),
        item(l("Open With…", "Open &with…"), OpenWith),
        separator(),
        item(
            l("Create Issue on GitHub", "Create &issue on GitHub"),
            CreateIssue,
        ),
        separator(),
        item(l("New Worktree…", "New work&tree…"), NewWorktree),
        separator(),
    ]);
    if extras.show_add_license {
        repository.push(item(l("Add License…", "A&dd license…"), AddLicense));
    }
    repository.push(item(
        l("Repository Settings…", "Repository &settings…"),
        RepositorySettings,
    ));
    template.push(submenu(l("Repository", "&Repository"), repository));

    let stash_all_label = if labels.ask_for_confirmation_when_stashing_all_changes {
        l("Stash All Changes…", "&Stash all changes…")
    } else {
        l("Stash All Changes", "&Stash all changes")
    };
    let mut branch = vec![
        item(l("New Branch…", "New &branch…"), NewBranch),
        item(l("Rename…", "&Rename…"), RenameBranch),
        item(l("Delete…", "&Delete…"), DeleteBranch),
        separator(),
        item(
            l("Discard All Changes…", "Discard all changes…"),
            DiscardAllChanges,
        ),
        item(stash_all_label, StashAllChanges),
        separator(),
        item(
            l_owned(
                format!("Update from {contribution_target_default_branch}"),
                format!("&Update from {contribution_target_default_branch}"),
            ),
            UpdateFromDefaultBranch,
        ),
        item(
            l("Compare to Branch", "&Compare to branch"),
            CompareToBranch,
        ),
        item(
            l("Merge into Current Branch…", "&Merge into current branch…"),
            MergeIntoCurrentBranch,
        ),
        item(
            l(
                "Squash and Merge into Current Branch…",
                "Squas&h and merge into current branch…",
            ),
            SquashAndMergeIntoCurrentBranch,
        ),
        item(
            l("Rebase Current Branch…", "R&ebase current branch…"),
            RebaseCurrentBranch,
        ),
        separator(),
        item(
            l("Compare on GitHub", "Compare on &GitHub"),
            CompareOnGitHub,
        ),
        item(
            l("View Branch on GitHub", "View branch on GitHub"),
            ViewBranchOnGitHub,
        ),
    ];
    // same as Toggle Full Screen: a macOS-only separator
    if cfg!(target_os = "macos") {
        branch.push(separator());
    }
    branch.extend([
        item(
            l("Preview Pull Request", "Preview pull request"),
            PreviewPullRequest,
        ),
        item(pull_request_label, CreatePullRequest),
    ]);
    template.push(submenu(l("Branch", "&Branch"), branch));

    if cfg!(target_os = "macos") {
        template.push(submenu("Window", window_items(extras.show_main_window)));
    }
    template.push(submenu(
        l("Help", "&Help"),
        help_items(extras.show_release_notes),
    ));
    template
}

/// Window menu; flag `405-window-menu-main-window` appends "Corvene", which
/// shows the main window again after ⌘W or the red close button.
fn window_items(show_main_window: bool) -> Vec<MenuItemConstructorOptions> {
    let mut items = vec![
        item("Minimize", Minimize),
        item("Zoom", Zoom),
        item("Close Window", CloseWindow),
        separator(),
        item("Bring All to Front", BringAllToFront),
    ];
    if show_main_window {
        items.extend([separator(), item("Corvene", ShowMainWindow)]);
    }
    items
}

/// Help menu; debug builds append GHD's test items (`buildTestMenu`, only
/// "Show notification" so far, as "Show Test Notifications"). Off macOS
/// About closes the menu, after a separator (GHD's non-darwin `&Help`).
fn help_items(show_release_notes: bool) -> Vec<MenuItemConstructorOptions> {
    let mut items = vec![
        item(l("Report Issue…", "Report issue…"), ReportIssue),
        item(
            l("Contact GitHub Support…", "&Contact GitHub support…"),
            ContactSupport,
        ),
        item("Show User Guides", ShowUserGuides),
        item(
            l("Show Keyboard Shortcuts", "Show keyboard shortcuts"),
            ShowKeyboardShortcuts,
        ),
        item(
            if cfg!(windows) {
                "S&how logs in Explorer"
            } else {
                l("Show Logs in Finder", "S&how logs in your File Manager")
            },
            ShowLogs,
        ),
    ];
    if show_release_notes {
        items.extend([
            separator(),
            item(
                l("Show Release Notes", "Show release &notes"),
                ShowReleaseNotes,
            ),
        ]);
    }
    if cfg!(debug_assertions) {
        items.extend([
            separator(),
            item(
                l("Show Test Notifications", "Show test notifications"),
                ShowTestNotifications,
            ),
        ]);
    }
    if !cfg!(target_os = "macos") {
        items.extend([separator(), item("&About Corvene", About)]);
    }
    items
}

/// GHD `buildDefaultMenu` (`Menu.buildFromTemplate`): the template as GPUI
/// menus, invisible items left out.
pub fn build_default_menu(labels: &MenuLabelsEvent) -> Vec<Menu> {
    build_default_menu_template(labels)
        .into_iter()
        .filter(|top| top.visible != Some(false))
        .map(|top| {
            Menu::new(top.label.unwrap_or_default())
                .items(gpui_items(top.submenu.unwrap_or_default()))
        })
        .collect()
}

fn gpui_items(items: Vec<MenuItemConstructorOptions>) -> Vec<MenuItem> {
    items
        .into_iter()
        .filter(|i| i.visible != Some(false))
        .filter_map(|i| {
            let label = i.label.unwrap_or_default();
            if i.separator {
                Some(MenuItem::separator())
            } else if let Some(kind) = i.system_menu {
                Some(MenuItem::os_submenu(label, kind))
            } else if let Some(items) = i.submenu {
                Some(MenuItem::submenu(Menu::new(label).items(gpui_items(items))))
            } else {
                i.action.map(|action| MenuItem::Action {
                    name: label.into(),
                    action,
                    os_action: i.os_action,
                    checked: false,
                    disabled: i.enabled == Some(false),
                })
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The access key after an unescaped `&`, lower-cased.
    fn access_key(label: &str) -> Option<char> {
        let mut chars = label.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '&' {
                match chars.next() {
                    Some('&') => {}
                    Some(key) => return Some(key.to_ascii_lowercase()),
                    None => return None,
                }
            }
        }
        None
    }

    fn duplicates(items: &[MenuItemConstructorOptions], path: &str, out: &mut Vec<String>) {
        let mut seen: Vec<(char, &str)> = Vec::new();
        for item in items
            .iter()
            .filter(|i| !i.separator && i.visible != Some(false))
        {
            let label = item.label.as_deref().unwrap_or("");
            if let Some(key) = access_key(label) {
                match seen.iter().find(|(k, _)| *k == key) {
                    Some((_, first)) => out.push(format!("{path}: {first} / {label}")),
                    None => seen.push((key, label)),
                }
            }
            if let Some(submenu) = &item.submenu {
                duplicates(submenu, &format!("{path} > {label}"), out);
            }
        }
    }

    /// Every Corvene addition on, with each label variant: no submenu
    /// repeats an access key (the GHD-preset case is GHD's own test).
    #[test]
    fn corvene_additions_keep_access_keys_unique() {
        for bits in 0..(1u32 << 5) {
            let labels = MenuLabelsEvent {
                selected_shell: Some("Terminal".into()),
                selected_external_editor: Some("Visual Studio Code".into()),
                is_stashed_changes_visible: bits & 1 != 0,
                has_current_pull_request: bits & 2 != 0,
                is_force_push_for_current_repository: bits & 4 != 0,
                ask_for_confirmation_on_repository_removal: bits & 8 != 0,
                is_changes_filter_visible: bits & 16 != 0,
                extras: MenuExtras {
                    show_release_notes: true,
                    show_import: true,
                    show_view_upstream: true,
                    show_main_window: true,
                    show_add_license: true,
                    fetch_all: true,
                    fetch_tags: true,
                    install_cli: true,
                    show_remove_repositories: true,
                    undo_last_commit: Some(true),
                    keymap: KeymapFlags {
                        navigation_shortcuts: true,
                        history_review_mode: true,
                        ..KeymapFlags::default()
                    },
                },
                ..MenuLabelsEvent::default()
            };
            let mut out = Vec::new();
            duplicates(&build_default_menu_template(&labels), "root", &mut out);
            assert_eq!(out, Vec::<String>::new());
        }
    }

    #[test]
    fn labels_follow_the_repository_state() {
        let labels = MenuLabelsEvent {
            is_force_push_for_current_repository: true,
            ask_for_confirmation_on_force_push: true,
            has_current_pull_request: true,
            is_stashed_changes_visible: true,
            contribution_target_default_branch: Some("a-very-long-default-branch-name".into()),
            ..MenuLabelsEvent::default()
        };
        let template = build_default_menu_template(&labels);
        let labels_of = |menu: &str| -> Vec<String> {
            template
                .iter()
                .find(|m| m.label.as_deref() == Some(menu))
                .and_then(|m| m.submenu.as_ref())
                .map(|items| items.iter().filter_map(|i| i.label.clone()).collect())
                .unwrap_or_default()
        };
        let repository = labels_of(l("Repository", "&Repository"));
        assert_eq!(repository[0], l("Force Push…", "Force P&ush…"));
        let branch = labels_of(l("Branch", "&Branch"));
        assert!(
            branch.contains(
                &l(
                    "View Pull Request on GitHub",
                    "View &pull request on GitHub"
                )
                .to_string()
            )
        );
        assert!(
            branch
                .iter()
                .any(|b| b.ends_with("from a-very-long-default-branc…"))
        );
        let view = labels_of(l("View", "&View"));
        assert!(view.contains(&l("Hide Stashed Changes", "H&ide stashed changes").to_string()));
    }
}
