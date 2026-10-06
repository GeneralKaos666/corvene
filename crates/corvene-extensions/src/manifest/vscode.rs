//! VS Code extensions (also what Cursor, VSCodium and Open VSX ship):
//! `package.json` with `contributes.languages` (the file associations) and
//! `contributes.grammars` (TextMate grammar files, JSON or plist).

use std::path::Path;

use super::{GrammarRef, Language, Manifest, pattern, repository_url, suffix};
use crate::ExtensionError;
use crate::value::{Value, parse_json};

/// `Ok(None)` unless `root/package.json` contributes grammars or file icon
/// themes.
pub fn read(root: &Path) -> Result<Option<Manifest>, ExtensionError> {
    let path = root.join("package.json");
    if !path.is_file() {
        return Ok(None);
    }
    let text = read_capped(&path)?;
    let package = parse_json(&text)?;
    let Some(contributes) = package.get("contributes") else {
        return Ok(None);
    };
    let icon_themes = crate::icon_theme::vscode_refs(&package);
    let grammars = match contributes.get("grammars") {
        Some(Value::List(grammars)) => &grammars[..],
        _ if !icon_themes.is_empty() => &[],
        _ => return Ok(None),
    };
    let mut manifest = Manifest {
        format: Some(super::Format::VsCode),
        name: package.str_of("name").unwrap_or("extension").to_string(),
        display_name: package.str_of("displayName").map(str::to_string),
        version: package.str_of("version").map(str::to_string),
        publisher: package.str_of("publisher").map(str::to_string),
        description: package.str_of("description").map(str::to_string),
        repository: repository_url(package.get("repository")),
        license: package.str_of("license").map(str::to_string),
        icon_themes,
        ..Default::default()
    };
    // the display name may be an `%nls.key%` placeholder
    if manifest
        .display_name
        .as_deref()
        .is_some_and(|d| d.starts_with('%'))
    {
        manifest.display_name = nls_lookup(root, manifest.display_name.as_deref().unwrap_or(""));
    }
    for grammar in grammars {
        let Some(path) = grammar.str_of("path") else {
            continue;
        };
        let language = grammar.str_of("language").map(str::to_string);
        let scope = grammar.str_of("scopeName").map(str::to_string);
        let name = language
            .clone()
            .or_else(|| scope.clone())
            .unwrap_or_else(|| path.to_string());
        manifest.grammars.push(GrammarRef::TextMate {
            name,
            scope,
            path: Path::new(path.trim_start_matches("./")).to_path_buf(),
            language,
            inject_to: grammar.strings_of("injectTo"),
        });
    }
    if let Some(Value::List(languages)) = contributes.get("languages") {
        for language in languages {
            let Some(id) = language.str_of("id") else {
                continue;
            };
            let mut suffixes: Vec<String> = language
                .strings_of("extensions")
                .iter()
                .filter_map(|e| suffix(e))
                .collect();
            let mut filenames: Vec<String> = language
                .strings_of("filenames")
                .iter()
                .map(|f| f.to_ascii_lowercase())
                .collect();
            for glob in language.strings_of("filenamePatterns") {
                let (s, f) = pattern(&glob);
                suffixes.extend(s);
                filenames.extend(f);
            }
            let aliases = language.strings_of("aliases");
            let grammar = manifest
                .grammars
                .iter()
                .find(|g| matches!(g, GrammarRef::TextMate { language: Some(l), .. } if l == id))
                .map(|g| g.name().to_string());
            manifest.languages.push(Language {
                id: id.to_string(),
                name: aliases.first().cloned(),
                suffixes,
                filenames,
                first_line: language.str_of("firstLine").map(str::to_string),
                aliases,
                grammar,
            });
        }
    }
    Ok(Some(manifest))
}

/// `%key%` → `package.nls.json`'s value.
fn nls_lookup(root: &Path, key: &str) -> Option<String> {
    let key = key.trim_matches('%');
    let text = std::fs::read_to_string(root.join("package.nls.json")).ok()?;
    let nls = parse_json(&text).ok()?;
    nls.str_of(key).map(str::to_string)
}

pub(crate) fn read_capped(path: &Path) -> Result<String, ExtensionError> {
    if std::fs::metadata(path)?.len() > crate::MAX_GRAMMAR_BYTES as u64 {
        return Err(ExtensionError::parse(
            "json",
            format!("{} is too large", path.display()),
        ));
    }
    Ok(std::fs::read_to_string(path)?)
}
