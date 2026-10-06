//! Zed extensions: `extension.toml` names the tree-sitter grammars
//! (`[grammars.<name>] repository = …, rev = …`), `languages/<dir>/config.toml`
//! the languages (`name`, `grammar`, `path_suffixes`, `first_line_pattern`)
//! next to their `highlights.scm` / `injections.scm` / `locals.scm`. A
//! packaged extension also has `grammars/<name>.wasm`, which Corvene does
//! not run.

use std::path::Path;

use super::{GrammarRef, Language, Manifest, suffix};
use crate::ExtensionError;

pub fn read(root: &Path) -> Result<Option<Manifest>, ExtensionError> {
    let path = root.join("extension.toml");
    if !path.is_file() {
        return Ok(None);
    }
    let text = super::vscode::read_capped(&path)?;
    let table: toml::Table =
        toml::from_str(&text).map_err(|err| ExtensionError::parse("toml", err.to_string()))?;
    let str_of =
        |t: &toml::Table, key: &str| t.get(key).and_then(|v| v.as_str()).map(str::to_string);
    let mut manifest = Manifest {
        format: Some(super::Format::Zed),
        name: str_of(&table, "id")
            .or_else(|| str_of(&table, "name"))
            .unwrap_or_else(|| "extension".to_string()),
        display_name: str_of(&table, "name"),
        version: str_of(&table, "version"),
        publisher: table
            .get("authors")
            .and_then(|a| a.as_array())
            .and_then(|a| a.first())
            .and_then(|a| a.as_str())
            .map(|a| a.split('<').next().unwrap_or(a).trim().to_string()),
        description: str_of(&table, "description"),
        repository: str_of(&table, "repository"),
        license: None,
        ..Default::default()
    };
    if let Some(grammars) = table.get("grammars").and_then(|g| g.as_table()) {
        for (name, grammar) in grammars {
            let grammar = grammar.as_table().cloned().unwrap_or_default();
            let wasm = Path::new("grammars").join(format!("{name}.wasm"));
            manifest.grammars.push(GrammarRef::TreeSitter {
                name: name.clone(),
                repository: str_of(&grammar, "repository"),
                rev: str_of(&grammar, "rev").or_else(|| str_of(&grammar, "commit")),
                path: str_of(&grammar, "path"),
                highlights: None,
                injections: None,
                locals: None,
                wasm: root.join(&wasm).is_file().then_some(wasm),
            });
        }
    }
    // languages: the listed folders, else every folder under languages/
    let mut dirs: Vec<std::path::PathBuf> = table
        .get("languages")
        .and_then(|l| l.as_array())
        .map(|l| {
            l.iter()
                .filter_map(|v| v.as_str())
                .map(|p| Path::new(p.trim_start_matches("./")).to_path_buf())
                .collect()
        })
        .unwrap_or_default();
    if dirs.is_empty() && root.join("languages").is_dir() {
        dirs = std::fs::read_dir(root.join("languages"))?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .map(|e| Path::new("languages").join(e.file_name()))
            .collect();
        dirs.sort();
    }
    for dir in dirs {
        let config_path = root.join(&dir).join("config.toml");
        if !config_path.is_file() {
            continue;
        }
        let config: toml::Table = toml::from_str(&super::vscode::read_capped(&config_path)?)
            .map_err(|err| {
                ExtensionError::parse("toml", format!("{}: {err}", config_path.display()))
            })?;
        let name = str_of(&config, "name").unwrap_or_else(|| {
            dir.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("language")
                .to_string()
        });
        let grammar = str_of(&config, "grammar");
        let strings = |key: &str| -> Vec<String> {
            config
                .get(key)
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str())
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default()
        };
        let mut suffixes = Vec::new();
        let mut filenames = Vec::new();
        for item in strings("path_suffixes") {
            // Zed treats a suffix with no dot-separated extension as a file name too
            match suffix(&item) {
                Some(s) if super::looks_like_filename(&item) => {
                    filenames.push(item.to_ascii_lowercase());
                    suffixes.push(s);
                }
                Some(s) => suffixes.push(s),
                None => filenames.push(item.to_ascii_lowercase()),
            }
        }
        let query = |file: &str| {
            let p = dir.join(file);
            root.join(&p).is_file().then_some(p)
        };
        // the queries sit with the language; attach them to its grammar
        if let Some(grammar_name) = &grammar {
            let (highlights, injections, locals) = (
                query("highlights.scm"),
                query("injections.scm"),
                query("locals.scm"),
            );
            match manifest
                .grammars
                .iter_mut()
                .find(|g| g.name() == grammar_name)
            {
                Some(GrammarRef::TreeSitter {
                    highlights: h,
                    injections: i,
                    locals: l,
                    ..
                }) => {
                    if h.is_none() {
                        *h = highlights;
                    }
                    if i.is_none() {
                        *i = injections;
                    }
                    if l.is_none() {
                        *l = locals;
                    }
                }
                Some(_) => {}
                None => {
                    // a grammar Zed itself ships (`grammar = "javascript"`):
                    // the extension brings queries only
                    manifest.grammars.push(GrammarRef::TreeSitter {
                        name: grammar_name.clone(),
                        repository: None,
                        rev: None,
                        path: None,
                        highlights,
                        injections,
                        locals,
                        wasm: None,
                    });
                }
            }
        }
        manifest.languages.push(Language {
            id: name.to_ascii_lowercase().replace(' ', "-"),
            name: Some(name),
            suffixes,
            filenames,
            first_line: str_of(&config, "first_line_pattern"),
            aliases: Vec::new(),
            grammar,
        });
    }
    // `117-file-icons`
    let icon_theme_files: Vec<String> = table
        .get("icon_themes")
        .and_then(|l| l.as_array())
        .map(|l| {
            l.iter()
                .filter_map(|v| v.as_str())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    manifest.icon_themes = crate::icon_theme::zed_refs(root, &icon_theme_files);
    if manifest.grammars.is_empty()
        && manifest.languages.is_empty()
        && manifest.icon_themes.is_empty()
    {
        return Ok(None);
    }
    Ok(Some(manifest))
}
