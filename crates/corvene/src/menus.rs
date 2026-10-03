//! The menu bar, mirroring GitHub Desktop 3.6.6
//! (`app/src/main-process/menu/build-default-menu.ts`): native on macOS, the
//! model of the in-window menu bar elsewhere. The template and the labels it
//! takes are `corvene_ui::app_menu` (GHD `buildDefaultMenuTemplate` over
//! `MenuLabelsEvent`); this installs it.

use gpui_kit::App;

/// What the menu bar depends on (GHD `MenuLabelsEvent` plus Corvene's
/// flag-dependent items, read from the state with `MenuOptions::of`); a
/// change rebuilds it, which is GHD's `updatePreferredAppMenuItemLabels`.
pub use corvene_ui::app_menu::MenuLabelsEvent as MenuOptions;

/// Build (or rebuild) the menu bar: the native macOS one, or elsewhere the
/// model the in-window menu bar (`corvene_ui::menu_bar`, Electron's classic
/// menu bar) draws.
pub fn install(cx: &mut App, options: &MenuOptions) {
    cx.set_menus(corvene_ui::app_menu::build_default_menu(options));
}
