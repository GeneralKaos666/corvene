//! Corvene (`117-file-icons`): the file icon theme the file lists draw
//! with. GHD shows no file type icons. The setting
//! ([`crate::persistence::Settings::file_icon_theme`]) is
//! [`BUILTIN`] (Octicons, drawn by the UI), [`NONE`], or
//! `<extension id>/<theme id>` for a theme of an installed language
//! extension ([`corvene_extensions::icon_theme`]), loaded here on a
//! background thread.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use corvene_extensions::icon_theme::builtin_language_id;
pub use corvene_extensions::icon_theme::{Icon, IconTheme};

use crate::dispatcher::Dispatcher;
use crate::host::Host;
use crate::remote::spawn_bg;
use crate::state::AppState;

/// The built-in Octicons set.
pub const BUILTIN: &str = "octicons";
/// No file icons.
pub const NONE: &str = "none";

/// A file icon theme an installed, enabled extension offers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IconThemeChoice {
    /// The setting's value.
    pub key: String,
    pub label: String,
    /// The extension's display name.
    pub extension: String,
    /// Which editor the theme is written for (`VS Code`, `Zed`).
    pub editor: &'static str,
}

/// The theme the setting names, loaded.
#[derive(Debug)]
pub struct LoadedIconTheme {
    pub key: String,
    pub theme: IconTheme,
    /// Language ids the installed VS Code extensions give file suffixes and
    /// names (a theme's `languageIds`), lowercase.
    languages: HashMap<String, String>,
    /// Each file name's icon as resolved (rows ask every frame).
    resolved: std::sync::Mutex<HashMap<(String, bool), Option<usize>>>,
}

impl LoadedIconTheme {
    /// The icon of a file named `name` (no folders).
    pub fn file_icon(&self, name: &str, light: bool) -> Option<&Icon> {
        let key = (name.to_string(), light);
        let cached = self.resolved.lock().ok().and_then(|c| c.get(&key).copied());
        let index = cached.unwrap_or_else(|| {
            let index = self
                .theme
                .file_icon_index(name, light, |name| self.language_id(name));
            if let Ok(mut cache) = self.resolved.lock() {
                cache.insert(key, index);
            }
            index
        });
        index.and_then(|i| self.theme.icon(i))
    }

    /// The icon of a folder named `name`.
    pub fn folder_icon(&self, name: &str, expanded: bool, light: bool) -> Option<&Icon> {
        self.theme.folder_icon(name, expanded, light)
    }

    fn language_id(&self, name: &str) -> Option<String> {
        let lower = name.to_lowercase();
        if let Some(id) = self.languages.get(&lower) {
            return Some(id.clone());
        }
        let mut at = lower.find('.');
        while let Some(dot) = at {
            if let Some(id) = self.languages.get(&format!(".{}", &lower[dot + 1..])) {
                return Some(id.clone());
            }
            at = lower[dot + 1..].find('.').map(|next| dot + 1 + next);
        }
        builtin_language_id(name).map(str::to_string)
    }
}

/// The setting value for theme `theme` of extension `extension`.
pub fn theme_key(extension: &str, theme: &str) -> String {
    format!("{extension}/{theme}")
}

/// Every file icon theme the enabled extensions offer, by extension then
/// label.
pub fn choices(s: &AppState) -> Vec<IconThemeChoice> {
    let mut out: Vec<IconThemeChoice> = s
        .extensions
        .enabled()
        .flat_map(|installed| {
            let m = &installed.metadata;
            m.icon_themes.iter().map(|t| IconThemeChoice {
                key: theme_key(&m.id, &t.id),
                label: t.label.clone(),
                extension: m.display_name.clone(),
                editor: match t.format {
                    corvene_extensions::icon_theme::IconThemeFormat::VsCode => "VS Code",
                    corvene_extensions::icon_theme::IconThemeFormat::Zed => "Zed",
                },
            })
        })
        .collect();
    out.sort_by(|a, b| {
        a.label
            .to_lowercase()
            .cmp(&b.label.to_lowercase())
            .then(a.extension.cmp(&b.extension))
    });
    out
}

/// The theme a background load is reading (settings writes during it do
/// not start another).
static LOADING: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

impl Dispatcher {
    /// Settings › Appearance › File icons.
    pub fn set_file_icon_theme(key: String, cx: &mut dyn Host) {
        Self::update_settings(cx, |s| s.file_icon_theme = key);
    }

    /// Load the theme the setting names, unless it is loaded already
    /// (`force`: again, after the extensions changed).
    pub fn load_file_icon_theme(force: bool, cx: &mut dyn Host) {
        let state = Self::state(cx);
        let wanted = {
            let s = state.read(cx);
            let key = s.settings.file_icon_theme.clone();
            let on = s.flags.bool(crate::flags::ids::FILE_ICONS);
            let loaded = s.file_icon_theme.as_ref().map(|t| t.key.as_str());
            let loading = LOADING.lock().ok().and_then(|l| l.clone());
            if !force && on && (loaded == Some(key.as_str()) || loading.as_deref() == Some(&key)) {
                return;
            }
            let found = on
                .then(|| {
                    s.extensions.enabled().find_map(|installed| {
                        let m = &installed.metadata;
                        let theme = m
                            .icon_themes
                            .iter()
                            .find(|t| theme_key(&m.id, &t.id) == key)?;
                        Some((installed.dir.clone(), theme.clone()))
                    })
                })
                .flatten();
            found.map(|(dir, theme)| {
                let languages: HashMap<String, String> = s
                    .extensions
                    .enabled()
                    .filter(|i| {
                        i.metadata.format == Some(corvene_extensions::manifest::Format::VsCode)
                    })
                    .flat_map(|i| i.metadata.languages.iter())
                    .flat_map(|l| {
                        l.filenames
                            .iter()
                            .cloned()
                            .chain(l.suffixes.iter().map(|x| format!(".{x}")))
                            .map(|k| (k.to_lowercase(), l.id.clone()))
                            .collect::<Vec<_>>()
                    })
                    .collect();
                (key, dir, theme, languages)
            })
        };
        let Some((key, dir, theme, languages)) = wanted else {
            state.update(cx, |s, cx| {
                if s.file_icon_theme.take().is_some() {
                    cx.notify();
                }
            });
            return;
        };
        if let Ok(mut loading) = LOADING.lock() {
            *loading = Some(key.clone());
        }
        spawn_bg(
            cx,
            move || IconTheme::load(&PathBuf::from(&dir), &theme),
            move |loaded, cx| {
                if let Ok(mut loading) = LOADING.lock()
                    && loading.as_deref() == Some(&key)
                {
                    *loading = None;
                }
                Self::state(cx).update(cx, |s, cx| {
                    // a later choice wins
                    if s.settings.file_icon_theme != key {
                        return;
                    }
                    s.file_icon_theme = match loaded {
                        Ok(theme) => Some(Arc::new(LoadedIconTheme {
                            key: key.clone(),
                            theme,
                            languages,
                            resolved: Default::default(),
                        })),
                        Err(err) => {
                            tracing::warn!(%err, theme = %key, "could not load the file icon theme");
                            None
                        }
                    };
                    cx.notify();
                });
            },
        );
    }
}
