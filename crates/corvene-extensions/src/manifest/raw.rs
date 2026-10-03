//! A bare grammar file (`x.tmLanguage.json`, `x.cson`, `x.plist`,
//! `x.YAML-tmLanguage`, `x.sublime-syntax`) given on its own.

use std::path::Path;

use super::{Format, Manifest};
use crate::ExtensionError;

/// A manifest for the single grammar file at `file`, rooted at its folder.
pub fn read(file: &Path) -> Result<Option<Manifest>, ExtensionError> {
    if !file.is_file() {
        return Ok(None);
    }
    let root = file.parent().unwrap_or(Path::new("."));
    let mut manifest = Manifest {
        format: Some(Format::Raw),
        name: file
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| {
                s.trim_end_matches(".tmLanguage")
                    .trim_end_matches(".YAML-tmLanguage")
                    .to_string()
            })
            .unwrap_or_else(|| "grammar".to_string()),
        ..Default::default()
    };
    let lower = file
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if lower.ends_with(".sublime-syntax") {
        let dir_manifest = super::sublime::read(root)?;
        // only this file, not its neighbours
        if let Some(m) = dir_manifest {
            let relative = file.strip_prefix(root).unwrap_or(file);
            manifest.grammars = m
                .grammars
                .into_iter()
                .filter(
                    |g| matches!(g, super::GrammarRef::Sublime { path, .. } if path == relative),
                )
                .collect();
            let names: Vec<String> = manifest
                .grammars
                .iter()
                .map(|g| g.name().to_string())
                .collect();
            manifest.languages = m
                .languages
                .into_iter()
                .filter(|l| l.grammar.as_ref().is_some_and(|g| names.contains(g)))
                .collect();
        }
    } else {
        super::tmbundle::add_textmate_file(root, file, &mut manifest)?;
    }
    if manifest.grammars.is_empty() {
        return Ok(None);
    }
    if let Some(name) = manifest.languages.first().and_then(|l| l.name.clone()) {
        manifest.display_name = Some(name);
    }
    Ok(Some(manifest))
}
