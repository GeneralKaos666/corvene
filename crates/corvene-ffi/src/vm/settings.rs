//! Settings › Appearance and the few other settings the Android screens
//! read directly.

use corvene_core::AppState;
use corvene_models::{DesignStyle, ThemeSetting};

#[derive(uniffi::Enum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum DesignStyleVm {
    GitHubMobile,
    GitHubDesktop,
    Material,
}

impl From<DesignStyle> for DesignStyleVm {
    fn from(style: DesignStyle) -> Self {
        match style {
            DesignStyle::GitHubMobile => DesignStyleVm::GitHubMobile,
            DesignStyle::GitHubDesktop => DesignStyleVm::GitHubDesktop,
            DesignStyle::Material => DesignStyleVm::Material,
        }
    }
}

impl From<DesignStyleVm> for DesignStyle {
    fn from(style: DesignStyleVm) -> Self {
        match style {
            DesignStyleVm::GitHubMobile => DesignStyle::GitHubMobile,
            DesignStyleVm::GitHubDesktop => DesignStyle::GitHubDesktop,
            DesignStyleVm::Material => DesignStyle::Material,
        }
    }
}

#[derive(uniffi::Enum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeVm {
    Light,
    Dark,
    System,
    HighContrast,
}

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct SettingsVm {
    /// What the app draws with: the setting, unless flag `112-design-style`
    /// pins one.
    pub design_style: DesignStyleVm,
    /// The stored setting (what the Appearance screen shows as chosen).
    pub design_style_setting: DesignStyleVm,
    pub design_style_pinned: bool,
    pub theme: ThemeVm,
    pub welcome_completed: bool,
    pub confirm_discard_changes: bool,
    pub confirm_force_push: bool,
    pub confirm_repository_removal: bool,
    pub notifications_enabled: bool,
    pub repository_indicators_enabled: bool,
    pub hide_whitespace_in_changes_diff: bool,
    pub hide_whitespace_in_history_diff: bool,
    pub show_diff_check_marks: bool,
    pub underline_links: bool,
}

pub fn settings(s: &AppState) -> SettingsVm {
    let setting = s.settings.design_style;
    let pinned = DesignStyle::parse(s.flags.text(corvene_core::flags::ids::DESIGN_STYLE));
    SettingsVm {
        design_style: pinned.unwrap_or(setting).into(),
        design_style_setting: setting.into(),
        design_style_pinned: pinned.is_some(),
        theme: match s.settings.theme {
            ThemeSetting::Light => ThemeVm::Light,
            ThemeSetting::Dark => ThemeVm::Dark,
            ThemeSetting::System => ThemeVm::System,
            ThemeSetting::HighContrast => ThemeVm::HighContrast,
        },
        welcome_completed: s.settings.welcome_completed,
        confirm_discard_changes: s.settings.confirm_discard_changes,
        confirm_force_push: s.settings.confirm_force_push,
        confirm_repository_removal: s.settings.confirm_repository_removal,
        notifications_enabled: s.settings.notifications_enabled,
        repository_indicators_enabled: s.settings.repository_indicators_enabled,
        hide_whitespace_in_changes_diff: s.settings.hide_whitespace_in_changes_diff,
        hide_whitespace_in_history_diff: s.settings.hide_whitespace_in_history_diff,
        show_diff_check_marks: s.settings.show_diff_check_marks,
        underline_links: s.settings.underline_links,
    }
}
