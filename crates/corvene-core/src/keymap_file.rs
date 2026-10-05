//! Corvene (`618-keymap-overrides`): `keymap.json` in the app's data folder
//! changes or removes keyboard shortcuts. GitHub Desktop's shortcuts are
//! fixed (`app/src/main-process/menu/build-default-menu.ts`).
//!
//! The file is one JSON object, action name → keystroke, `null` removing
//! the action's shortcuts:
//!
//! ```json
//! { "Push": null, "Pull": "cmd-shift-l", "corvene::ShowHistory": "ctrl-2" }
//! ```
//!
//! Names are the menu actions' (`OpenInEditor`, with or without the
//! `corvene::` namespace; case, `-` and `_` are ignored, so `open-in-editor`
//! works too). Keystrokes use GPUI's syntax: modifiers `cmd`, `ctrl`, `alt`,
//! `shift`, `secondary` (⌘ on macOS, Ctrl elsewhere) joined to the key with
//! `-`, several keystrokes separated by spaces. The file is read at launch
//! and whenever Settings closes; `corvene_ui::keymap::sync` applies it after
//! the default and flag bindings, so the menus show the new shortcuts.

use std::path::{Path, PathBuf};

/// The file's name in the app's data folder.
pub const FILE_NAME: &str = "keymap.json";

/// The overrides in file order: (action name as written, keystroke or
/// `None` to unbind).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeymapOverrides {
    pub entries: Vec<(String, Option<String>)>,
}

impl KeymapOverrides {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Where the file lives.
pub fn path() -> PathBuf {
    corvene_platform::paths::app_support_dir().join(FILE_NAME)
}

/// Parse the file's text; the messages name the entries that were skipped.
pub fn parse(text: &str) -> (KeymapOverrides, Vec<String>) {
    let mut errors = Vec::new();
    let value: serde_json::Value = match serde_json::from_str(text) {
        Ok(value) => value,
        Err(err) => return (KeymapOverrides::default(), vec![err.to_string()]),
    };
    let serde_json::Value::Object(map) = value else {
        return (
            KeymapOverrides::default(),
            vec!["the file must hold one object of action names".to_string()],
        );
    };
    let mut entries = Vec::new();
    for (name, value) in map {
        match value {
            serde_json::Value::String(keys) if !keys.trim().is_empty() => {
                entries.push((name, Some(keys.trim().to_string())))
            }
            serde_json::Value::Null => entries.push((name, None)),
            _ => errors.push(format!(
                "`{name}`: give a keystroke such as \"cmd-k\" or null"
            )),
        }
    }
    (KeymapOverrides { entries }, errors)
}

/// Read `path`: no file is no overrides.
pub fn load(path: &Path) -> (KeymapOverrides, Vec<String>) {
    match std::fs::read_to_string(path) {
        Ok(text) => parse(&text),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            (KeymapOverrides::default(), Vec::new())
        }
        Err(err) => (KeymapOverrides::default(), vec![err.to_string()]),
    }
}

/// The comparable form of an action name: lower case, letters and digits
/// only, without the `corvene::` namespace.
pub fn normalized_action_name(name: &str) -> String {
    let name = name.trim();
    let name = name.strip_prefix("corvene::").unwrap_or(name);
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == ':')
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// The registered action `name` stands for, Corvene's own first.
pub fn resolve_action_name<'a>(name: &str, registered: &[&'a str]) -> Option<&'a str> {
    let wanted = normalized_action_name(name);
    if wanted.is_empty() {
        return None;
    }
    let short = |full: &str| normalized_action_name(full.strip_prefix("corvene::").unwrap_or(full));
    registered
        .iter()
        .find(|full| full.starts_with("corvene::") && short(full) == wanted)
        .or_else(|| {
            registered
                .iter()
                .find(|full| normalized_action_name(full) == wanted)
        })
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_keystrokes_and_unbindings() {
        let (overrides, errors) =
            parse(r#"{ "Push": null, "Pull": " cmd-shift-l ", "Fetch": 3, "Find": "" }"#);
        assert_eq!(
            overrides.entries,
            vec![
                ("Push".to_string(), None),
                ("Pull".to_string(), Some("cmd-shift-l".to_string())),
            ]
        );
        assert_eq!(errors.len(), 2, "{errors:?}");
        assert!(!parse("[1]").1.is_empty());
        assert!(!parse("{").1.is_empty());
    }

    #[test]
    fn resolves_action_names_loosely() {
        let registered = [
            "corvene::OpenInEditor",
            "corvene::Push",
            "input::Copy",
            "corvene::Copy",
        ];
        for name in [
            "OpenInEditor",
            "open-in-editor",
            "open_in_editor",
            "corvene::OpenInEditor",
        ] {
            assert_eq!(
                resolve_action_name(name, &registered),
                Some("corvene::OpenInEditor"),
                "{name}"
            );
        }
        assert_eq!(
            resolve_action_name("copy", &registered),
            Some("corvene::Copy")
        );
        assert_eq!(
            resolve_action_name("input::Copy", &registered),
            Some("input::Copy")
        );
        assert_eq!(resolve_action_name("Shove", &registered), None);
        assert_eq!(resolve_action_name("", &registered), None);
    }

    #[test]
    fn a_missing_file_is_no_overrides() {
        let dir = std::env::temp_dir().join(format!("corvene-keymap-{}", std::process::id()));
        let (overrides, errors) = load(&dir.join(FILE_NAME));
        assert!(overrides.is_empty() && errors.is_empty());
    }
}
