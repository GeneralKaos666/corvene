//! TextMate bundles (`Name.tmbundle/`: `info.plist` + `Syntaxes/*`), the
//! form TextMate and IntelliJ's TextMate Bundles plugin use. Each grammar
//! in `Syntaxes/` names its own scope and file types.

use std::path::Path;

use super::{Format, GrammarRef, Language, Manifest, suffix};
use crate::ExtensionError;
use crate::tm::{GrammarFormat, parse_file};

pub fn read(root: &Path) -> Result<Option<Manifest>, ExtensionError> {
    let syntaxes = ["Syntaxes", "syntaxes"]
        .iter()
        .map(|d| root.join(d))
        .find(|d| d.is_dir());
    let is_bundle = root
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("tmbundle"))
        || root.join("info.plist").is_file();
    let (Some(syntaxes), true) = (syntaxes, is_bundle) else {
        return Ok(None);
    };
    let info = root
        .join("info.plist")
        .is_file()
        .then(|| std::fs::read_to_string(root.join("info.plist")).ok())
        .flatten()
        .and_then(|text| crate::plist::parse(&text).ok());
    let bundle_name = root
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("bundle")
        .to_string();
    let mut manifest = Manifest {
        format: Some(Format::TmBundle),
        name: info
            .as_ref()
            .and_then(|i| i.str_of("name"))
            .unwrap_or(&bundle_name)
            .to_string(),
        description: info
            .as_ref()
            .and_then(|i| i.str_of("description"))
            .map(str::to_string),
        ..Default::default()
    };
    let mut files: Vec<_> = std::fs::read_dir(&syntaxes)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file())
        .collect();
    files.sort();
    for file in files {
        if GrammarFormat::from_path(&file).is_none() {
            continue;
        }
        add_textmate_file(root, &file, &mut manifest)?;
    }
    if manifest.grammars.is_empty() {
        return Ok(None);
    }
    Ok(Some(manifest))
}

/// Register one TextMate grammar file as a grammar plus the language its
/// `fileTypes` describe (shared with the Sublime and raw readers).
pub(crate) fn add_textmate_file(
    root: &Path,
    file: &Path,
    manifest: &mut Manifest,
) -> Result<(), ExtensionError> {
    let grammar = match parse_file(file) {
        Ok(g) => g,
        Err(err) => {
            tracing::debug!("skipping {}: {err}", file.display());
            return Ok(());
        }
    };
    let name = grammar
        .name
        .clone()
        .unwrap_or_else(|| grammar.scope_name.clone());
    let relative = file.strip_prefix(root).unwrap_or(file).to_path_buf();
    let id = grammar.scope_name.clone();
    manifest.grammars.push(GrammarRef::TextMate {
        name: id.clone(),
        scope: Some(grammar.scope_name.clone()),
        path: relative,
        language: Some(id.clone()),
        inject_to: Vec::new(),
    });
    manifest.languages.push(Language {
        id: id.clone(),
        name: Some(name),
        suffixes: grammar
            .file_types
            .iter()
            .filter_map(|t| suffix(t))
            .collect(),
        filenames: grammar
            .file_types
            .iter()
            .filter(|t| suffix(t).is_none() || super::looks_like_filename(t))
            .map(|t| t.to_ascii_lowercase())
            .collect(),
        first_line: grammar.first_line_match.clone(),
        aliases: Vec::new(),
        grammar: Some(id),
    });
    Ok(())
}
