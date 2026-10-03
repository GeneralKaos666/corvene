//! Sublime Text packages: a folder (or an unpacked `.sublime-package`) with
//! `*.sublime-syntax` files, syntect's native format, and possibly
//! `*.tmLanguage` ones.

use std::path::Path;

use super::{Format, GrammarRef, Language, Manifest, suffix};
use crate::ExtensionError;
use crate::value::parse_yaml;

pub fn read(root: &Path) -> Result<Option<Manifest>, ExtensionError> {
    if !root.is_dir() {
        return Ok(None);
    }
    let mut files: Vec<_> = std::fs::read_dir(root)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file())
        .filter(|p| {
            let name = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            name.ends_with(".sublime-syntax") || name.ends_with(".tmlanguage")
        })
        .collect();
    if files.is_empty() {
        return Ok(None);
    }
    files.sort();
    let mut manifest = Manifest {
        format: Some(Format::Sublime),
        name: root
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.trim_end_matches(".sublime-package").to_string())
            .unwrap_or_else(|| "package".to_string()),
        ..Default::default()
    };
    for file in files {
        if file
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("sublime-syntax"))
        {
            add_sublime_file(root, &file, &mut manifest)?;
        } else {
            super::tmbundle::add_textmate_file(root, &file, &mut manifest)?;
        }
    }
    if manifest.grammars.is_empty() {
        return Ok(None);
    }
    Ok(Some(manifest))
}

fn add_sublime_file(
    root: &Path,
    file: &Path,
    manifest: &mut Manifest,
) -> Result<(), ExtensionError> {
    let text = super::vscode::read_capped(file)?;
    let header = match parse_yaml(&text) {
        Ok(v) => v,
        Err(err) => {
            tracing::debug!("skipping {}: {err}", file.display());
            return Ok(());
        }
    };
    if header
        .get("hidden")
        .and_then(|h| h.as_bool())
        .unwrap_or(false)
    {
        // a helper syntax other ones include; still needed in the set
        let name = header
            .str_of("scope")
            .or_else(|| header.str_of("name"))
            .unwrap_or("hidden")
            .to_string();
        manifest.grammars.push(GrammarRef::Sublime {
            name,
            path: file.strip_prefix(root).unwrap_or(file).to_path_buf(),
        });
        return Ok(());
    }
    let name = header
        .str_of("name")
        .map(str::to_string)
        .or_else(|| {
            file.file_stem()
                .and_then(|s| s.to_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "syntax".to_string());
    let id = header.str_of("scope").unwrap_or(&name).to_string();
    manifest.grammars.push(GrammarRef::Sublime {
        name: id.clone(),
        path: file.strip_prefix(root).unwrap_or(file).to_path_buf(),
    });
    let types = header.strings_of("file_extensions");
    manifest.languages.push(Language {
        id: id.clone(),
        name: Some(name),
        suffixes: types.iter().filter_map(|t| suffix(t)).collect(),
        filenames: types
            .iter()
            .filter(|t| suffix(t).is_none() || super::looks_like_filename(t))
            .map(|t| t.to_ascii_lowercase())
            .collect(),
        first_line: header.str_of("first_line_match").map(str::to_string),
        aliases: Vec::new(),
        grammar: Some(id),
    });
    Ok(())
}
