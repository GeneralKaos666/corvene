//! Atom and Pulsar packages: `package.json` (no `contributes`) next to a
//! `grammars/` folder of `.cson` or `.json` grammars, each carrying its own
//! `scopeName` and `fileTypes`. Tree-sitter grammars (`type: "tree-sitter"`
//! for the legacy kind, `"modern-tree-sitter"` for Pulsar's) name an npm
//! parser package and, for the modern kind, their `.scm` queries.

use std::path::Path;

use super::{GrammarRef, Language, Manifest, repository_url, suffix};
use crate::ExtensionError;
use crate::tm::GrammarFormat;
use crate::value::{Value, parse_json};

pub fn read(root: &Path) -> Result<Option<Manifest>, ExtensionError> {
    let grammars_dir = root.join("grammars");
    let package_path = root.join("package.json");
    if !grammars_dir.is_dir() || !package_path.is_file() {
        return Ok(None);
    }
    let package = parse_json(&super::vscode::read_capped(&package_path)?)?;
    if package
        .get("contributes")
        .is_some_and(|c| c.get("grammars").is_some())
    {
        return Ok(None); // a VS Code extension with a grammars folder
    }
    let mut manifest = Manifest {
        format: Some(super::Format::Atom),
        name: package.str_of("name").unwrap_or("package").to_string(),
        display_name: None,
        version: package.str_of("version").map(str::to_string),
        publisher: None,
        description: package.str_of("description").map(str::to_string),
        repository: repository_url(package.get("repository")),
        license: package.str_of("license").map(str::to_string),
        ..Default::default()
    };
    let mut files: Vec<_> = std::fs::read_dir(&grammars_dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "cson" | "json"))
        })
        .collect();
    files.sort();
    for file in files {
        let text = super::vscode::read_capped(&file)?;
        let format = if file
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("cson"))
        {
            GrammarFormat::Cson
        } else {
            GrammarFormat::Json
        };
        let value = match format {
            GrammarFormat::Cson => crate::cson::parse(&text),
            _ => parse_json(&text),
        };
        let Ok(value) = value else {
            tracing::debug!("skipping {}: not a grammar", file.display());
            continue;
        };
        let relative = file.strip_prefix(root).unwrap_or(&file).to_path_buf();
        let grammar_name = value
            .str_of("name")
            .or_else(|| value.str_of("scopeName"))
            .unwrap_or("grammar")
            .to_string();
        let suffixes: Vec<String> = value
            .strings_of("fileTypes")
            .iter()
            .filter_map(|t| suffix(t))
            .collect();
        let filenames: Vec<String> = value
            .strings_of("fileTypes")
            .iter()
            .filter(|t| t.contains('.') && !t.starts_with('.') && suffix(t).is_none())
            .map(|t| t.to_ascii_lowercase())
            .collect();
        let kind = value.str_of("type").unwrap_or("textmate");
        let grammar = match kind {
            "tree-sitter" | "modern-tree-sitter" => {
                let (repository, rev) = parser_source(&value, &package);
                let query = |key: &str| {
                    value
                        .get("treeSitter")
                        .and_then(|t| t.str_of(key))
                        .map(|p| Path::new("grammars").join(p.trim_start_matches("./")))
                        .filter(|p| root.join(p).is_file())
                        .or_else(|| {
                            // Pulsar keeps queries next to the grammar file
                            let guess = Path::new("grammars")
                                .join(format!("{}.scm", key.trim_end_matches("Query")));
                            root.join(&guess).is_file().then_some(guess)
                        })
                };
                GrammarRef::TreeSitter {
                    name: tree_sitter_name(&value, &grammar_name),
                    repository,
                    rev,
                    path: None,
                    highlights: query("highlightsQuery"),
                    injections: query("injectionsQuery"),
                    locals: query("localsQuery"),
                    wasm: value
                        .get("treeSitter")
                        .and_then(|t| t.str_of("grammar"))
                        .map(|p| Path::new("grammars").join(p)),
                }
            }
            _ => {
                if value.str_of("scopeName").is_none() {
                    continue;
                }
                GrammarRef::TextMate {
                    name: grammar_name.clone(),
                    scope: value.str_of("scopeName").map(str::to_string),
                    path: relative,
                    language: Some(grammar_name.clone()),
                    inject_to: Vec::new(),
                }
            }
        };
        manifest.languages.push(Language {
            id: grammar_name.clone(),
            name: value.str_of("name").map(str::to_string),
            suffixes,
            filenames,
            first_line: value.str_of("firstLineMatch").map(str::to_string),
            aliases: Vec::new(),
            grammar: Some(grammar.name().to_string()),
        });
        manifest.grammars.push(grammar);
    }
    if manifest.grammars.is_empty() {
        return Ok(None);
    }
    Ok(Some(manifest))
}

/// `tree-sitter-zig` → `zig`.
fn tree_sitter_name(value: &Value, fallback: &str) -> String {
    value
        .str_of("parser")
        .or_else(|| value.get("treeSitter").and_then(|t| t.str_of("parser")))
        .map(|p| p.trim_start_matches("tree-sitter-").to_string())
        .unwrap_or_else(|| fallback.to_ascii_lowercase().replace(' ', "-"))
}

/// Where the parser's source is: Pulsar's `parserSource`
/// (`github:owner/repo#rev`), else the npm dependency's git URL in
/// `package.json`.
fn parser_source(value: &Value, package: &Value) -> (Option<String>, Option<String>) {
    let source = value
        .get("treeSitter")
        .and_then(|t| t.str_of("parserSource"))
        .or_else(|| value.str_of("parserSource"));
    if let Some(source) = source {
        let (repo, rev) = match source.split_once('#') {
            Some((repo, rev)) => (repo, Some(rev.to_string())),
            None => (source, None),
        };
        return (repository_url(Some(&Value::Str(repo.to_string()))), rev);
    }
    let spec = value
        .str_of("parser")
        .and_then(|parser| package.get("dependencies").and_then(|d| d.str_of(parser)));
    let Some(spec) = spec else {
        return (None, None);
    };
    let (repo, rev) = match spec.split_once('#') {
        Some((repo, rev)) => (repo, Some(rev.to_string())),
        None => (spec, None),
    };
    (repository_url(Some(&Value::Str(repo.to_string()))), rev)
}
