//! Corvene (`522-settings-file`): a read-only settings file
//! (`corvene_platform::paths::settings_file`, `~/.config/corvene/settings.json`)
//! whose keys are applied over the stored settings at launch, so a
//! dotfiles repository can set up Corvene. GitHub Desktop keeps its
//! preferences in `localStorage` only (`app/src/lib/local-storage.ts`).
//!
//! The file is one JSON object with [`Settings`]' field names
//! (`{"theme": "dark", "tab_size": 8}`) and an optional `"flags"` entry in
//! `CORVENE_FLAGS` syntax, applied for the session like that variable
//! (`CORVENE_FLAGS` itself still wins). Corvene never writes the file and
//! never saves the values it took from it: a save writes the stored value of
//! every key the file set ([`SettingsOverlay::stored_form`]), so removing a
//! key from the file brings the stored value back. Settings › Advanced ›
//! "Export Settings…" writes the current settings and flags in this format
//! ([`export`]).

use std::path::Path;

use serde_json::{Map, Value};

use crate::flags::EnvFlags;
use crate::persistence::Settings;

/// The file's name (the Flags dialog's "Set by settings.json").
pub const FILE_NAME: &str = "settings.json";

/// The key that carries the flags spec.
pub const FLAGS_KEY: &str = "flags";

/// Fields the export leaves out: when they last happened, not a choice.
const NOT_EXPORTED: &[&str] = &["last_launched_at", "last_successful_update_check"];

/// The keys the file set and the stored values they replaced.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SettingsOverlay {
    /// key → the stored value it replaced
    pub stored: Map<String, Value>,
}

impl SettingsOverlay {
    /// `settings` as it is saved: the stored value back in each key the
    /// file set.
    pub fn stored_form(&self, settings: &Settings) -> Settings {
        if self.stored.is_empty() {
            return settings.clone();
        }
        let Ok(Value::Object(mut map)) = serde_json::to_value(settings) else {
            return settings.clone();
        };
        for (key, value) in &self.stored {
            map.insert(key.clone(), value.clone());
        }
        serde_json::from_value(Value::Object(map)).unwrap_or_else(|_| settings.clone())
    }
}

/// What the file holds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SettingsFile {
    pub values: Map<String, Value>,
    /// The `"flags"` spec.
    pub flags: Option<String>,
}

/// Read the file at `path`: `Ok(None)` when there is none.
pub fn load(path: &Path) -> Result<Option<SettingsFile>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err.to_string()),
    };
    parse(&text).map(Some)
}

pub fn parse(text: &str) -> Result<SettingsFile, String> {
    let Value::Object(mut values) = serde_json::from_str(text).map_err(|err| err.to_string())?
    else {
        return Err("the file must hold one object of settings".to_string());
    };
    let flags = match values.remove(FLAGS_KEY) {
        None | Some(Value::Null) => None,
        Some(Value::String(spec)) => Some(spec),
        Some(_) => {
            return Err(format!(
                "`{FLAGS_KEY}` must be a text such as \"preset=max\""
            ));
        }
    };
    Ok(SettingsFile { values, flags })
}

/// Apply `values` over `settings`: the overlay to save with, and a message
/// per key that is unknown or holds a value the setting cannot take (those
/// keys are left alone).
pub fn apply(
    settings: &mut Settings,
    values: &Map<String, Value>,
) -> (SettingsOverlay, Vec<String>) {
    let mut errors = Vec::new();
    let mut overlay = SettingsOverlay::default();
    let Ok(Value::Object(mut current)) = serde_json::to_value(&*settings) else {
        return (overlay, vec!["the settings could not be read".to_string()]);
    };
    for (key, value) in values {
        let Some(stored) = current.get(key).cloned() else {
            errors.push(format!("there is no setting named `{key}`"));
            continue;
        };
        let mut trial = current.clone();
        trial.insert(key.clone(), value.clone());
        if serde_json::from_value::<Settings>(Value::Object(trial)).is_err() {
            errors.push(format!("`{key}` cannot be {value}"));
            continue;
        }
        current.insert(key.clone(), value.clone());
        overlay.stored.insert(key.clone(), stored);
    }
    if let Ok(applied) = serde_json::from_value(Value::Object(current)) {
        *settings = applied;
    }
    (overlay, errors)
}

/// Read and apply the file at `path` at launch: its settings over
/// `settings` and its flags spec under `env` (`CORVENE_FLAGS` wins).
/// Returns the overlay, the flag entries the file decided (for the Flags
/// dialog's "Set by" label) and what could not be used.
pub fn apply_at_launch(
    path: &Path,
    settings: &mut Settings,
    env: &mut EnvFlags,
) -> (SettingsOverlay, EnvFlags, Vec<String>) {
    let file = match load(path) {
        Ok(Some(file)) => file,
        Ok(None) => return Default::default(),
        Err(err) => return (SettingsOverlay::default(), EnvFlags::default(), vec![err]),
    };
    let (overlay, mut errors) = apply(settings, &file.values);
    let mut decided = EnvFlags::default();
    if let Some(spec) = file.flags {
        let (mut from_file, flag_errors) = crate::flags::env::parse(&spec);
        errors.extend(flag_errors.into_iter().map(|err| {
            err.strip_prefix(crate::flags::env::VAR)
                .map(|rest| format!("{FLAGS_KEY}{rest}"))
                .unwrap_or(err)
        }));
        from_file
            .values
            .retain(|id, _| !env.values.contains_key(id));
        if env.preset.is_some() {
            from_file.preset = None;
        }
        env.preset = env.preset.or(from_file.preset);
        env.values.extend(
            from_file
                .values
                .iter()
                .map(|(id, value)| (*id, value.clone())),
        );
        decided = from_file;
    }
    (overlay, decided, errors)
}

/// The text Settings › Advanced › "Export Settings…" writes: every setting
/// (but the timestamps) and, when given, the flags spec.
pub fn export(settings: &Settings, flags: Option<&str>) -> String {
    let mut map = match serde_json::to_value(settings) {
        Ok(Value::Object(map)) => map,
        _ => Map::new(),
    };
    for key in NOT_EXPORTED {
        map.remove(*key);
    }
    if let Some(flags) = flags {
        map.insert(FLAGS_KEY.to_string(), Value::String(flags.to_string()));
    }
    serde_json::to_string_pretty(&Value::Object(map)).unwrap_or_default() + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_known_keys_and_names_the_rest() {
        let file = parse(
            r#"{"tab_size": 8, "confirm_force_push": false, "colour": "red",
                "show_commit_length_warning": "yes", "flags": "preset=max"}"#,
        )
        .unwrap();
        assert_eq!(file.flags.as_deref(), Some("preset=max"));
        let mut settings = Settings::default();
        let (overlay, errors) = apply(&mut settings, &file.values);
        assert_eq!(settings.tab_size, 8);
        assert!(!settings.confirm_force_push);
        assert!(settings.show_commit_length_warning);
        assert_eq!(errors.len(), 2, "{errors:?}");
        assert_eq!(overlay.stored.len(), 2);
    }

    #[test]
    fn saving_keeps_the_stored_values_of_the_keys_the_file_set() {
        let stored = Settings {
            tab_size: 2,
            ..Settings::default()
        };
        let mut settings = stored.clone();
        let file = parse(r#"{"tab_size": 8}"#).unwrap();
        let (overlay, _) = apply(&mut settings, &file.values);
        settings.sidebar_width = 300.0;
        let saved = overlay.stored_form(&settings);
        assert_eq!(saved.tab_size, 2);
        assert_eq!(saved.sidebar_width, 300.0);
    }

    #[test]
    fn an_export_reads_back_as_the_same_settings() {
        let settings = Settings {
            tab_size: 3,
            last_launched_at: Some(5),
            ..Settings::default()
        };
        let text = export(&settings, Some("preset=corvene,115-sidebar-on-right=on"));
        let file = parse(&text).unwrap();
        assert!(!file.values.contains_key("last_launched_at"));
        assert_eq!(
            file.flags.as_deref(),
            Some("preset=corvene,115-sidebar-on-right=on")
        );
        let mut back = Settings::default();
        let (_, errors) = apply(&mut back, &file.values);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(back.tab_size, 3);
    }

    #[test]
    fn corvene_flags_wins_over_the_files_flags() {
        let dir = std::env::temp_dir().join(format!("corvene-settings-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        std::fs::write(
            &path,
            r#"{"tab_size": 6, "flags": "preset=max,115-sidebar-on-right,fs-watcher=off,nope"}"#,
        )
        .unwrap();
        let (mut env, _) = crate::flags::env::parse("fs-watcher=on");
        let mut settings = Settings::default();
        let (overlay, decided, errors) = apply_at_launch(&path, &mut settings, &mut env);
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(settings.tab_size, 6);
        assert!(overlay.stored.contains_key("tab_size"));
        assert_eq!(errors, vec!["flags: unknown flag `nope`".to_string()]);
        assert_eq!(env.preset, Some(crate::flags::Preset::Max));
        assert_eq!(
            env.values.get(&crate::flags::ids::FS_WATCHER),
            Some(&crate::flags::Value::Bool(true))
        );
        assert!(
            decided
                .values
                .contains_key(&crate::flags::ids::SIDEBAR_ON_RIGHT)
        );
        assert!(!decided.values.contains_key(&crate::flags::ids::FS_WATCHER));
    }

    #[test]
    fn bad_files_are_errors_and_no_file_is_none() {
        assert!(parse("[]").is_err());
        assert!(parse("{").is_err());
        assert!(parse(r#"{"flags": 3}"#).is_err());
        let missing =
            std::env::temp_dir().join(format!("corvene-no-settings-{}", std::process::id()));
        assert_eq!(load(&missing.join("settings.json")), Ok(None));
    }
}
