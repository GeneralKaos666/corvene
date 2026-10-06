//! Turning a scanned extension into an installed folder: the source is
//! copied, every TextMate grammar converted and checked, Sublime syntaxes
//! and tree-sitter queries copied, and the metadata written. Nothing here
//! runs grammars against real files beyond a short self-test; syntect
//! compiles and the diff view runs them later.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::Serialize;
use syntect::parsing::{ParseState, SyntaxDefinition, SyntaxSet, SyntaxSetBuilder};

use super::{GrammarKind, GrammarStatus, Metadata, Resolution, SCHEMA, Source, Status};
use crate::ExtensionError;
use crate::manifest::{GrammarRef, Language};
use crate::scan::Scanned;
use crate::tm::convert::{ConversionReport, Options, convert};
use crate::tm::{compile, emit, parse_file};

/// The source copy is capped at this many bytes (the grammars are already
/// converted; the copy is for the details panel and updates).
const MAX_SOURCE_BYTES: u64 = 64 * 1024 * 1024;

/// A grammar whose self-test exceeds this is marked slow.
const SLOW_AFTER: Duration = Duration::from_millis(200);

/// Decides where a tree-sitter grammar's parser comes from.
pub type Resolver<'a> = &'a dyn Fn(&GrammarRef) -> Resolution;

/// `report.json`: what each grammar lost in conversion.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Report {
    pub grammars: Vec<GrammarReport>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GrammarReport {
    pub name: String,
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub self_test_ms: u64,
    pub dropped: Vec<DroppedEntry>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DroppedEntry {
    pub context: String,
    pub kind: String,
    pub detail: String,
}

/// What came out.
#[derive(Clone, Debug)]
pub struct Prepared {
    pub metadata: Metadata,
    pub report: Report,
}

/// Build the installed folder `out` for `scanned` (replaced if present).
pub fn prepare(
    scanned: &Scanned,
    id: &str,
    source: Source,
    out: &Path,
    resolver: Resolver<'_>,
) -> Result<Prepared, ExtensionError> {
    if out.exists() {
        std::fs::remove_dir_all(out)?;
    }
    std::fs::create_dir_all(out.join("grammars"))?;
    let manifest = &scanned.manifest;
    let mut languages = manifest.languages.clone();
    let mut grammars = Vec::new();
    let mut report = Report::default();
    for grammar in &manifest.grammars {
        let (status, grammar_report) = match grammar {
            GrammarRef::TextMate {
                name,
                scope,
                path,
                language,
                ..
            } => prepare_textmate(
                scanned,
                name,
                scope.as_deref(),
                path,
                language.as_deref(),
                &mut languages,
                out,
            ),
            GrammarRef::Sublime { name, path } => prepare_sublime(scanned, name, path, out),
            GrammarRef::TreeSitter {
                name,
                repository,
                rev,
                path,
                highlights,
                injections,
                locals,
                ..
            } => {
                let queries_dir = out.join("queries").join(super::slug(name));
                let mut copied = false;
                for (file, from) in [
                    ("highlights.scm", highlights),
                    ("injections.scm", injections),
                    ("locals.scm", locals),
                ] {
                    if let Some(from) = from {
                        std::fs::create_dir_all(&queries_dir)?;
                        std::fs::copy(scanned.root.join(from), queries_dir.join(file))?;
                        copied = true;
                    }
                }
                let resolution = resolver(grammar);
                let status = GrammarStatus {
                    name: name.clone(),
                    scope: None,
                    kind: GrammarKind::TreeSitter,
                    file: None,
                    status: if copied { Status::Ok } else { Status::Rejected },
                    error: (!copied).then(|| {
                        "the extension carries no highlight queries for this grammar".to_string()
                    }),
                    dropped: 0,
                    repository: repository.clone(),
                    rev: rev.clone(),
                    path: path.clone(),
                    queries: copied.then(|| format!("queries/{}", super::slug(name))),
                    resolution,
                };
                let grammar_report = GrammarReport {
                    name: name.clone(),
                    status: status.status,
                    error: status.error.clone(),
                    self_test_ms: 0,
                    dropped: Vec::new(),
                };
                (status, grammar_report)
            }
        };
        grammars.push(status);
        report.grammars.push(grammar_report);
    }
    if grammars.is_empty() && manifest.icon_themes.is_empty() {
        return Err(ExtensionError::NotAnExtension(
            "no syntax grammars or file icon themes in this extension".to_string(),
        ));
    }
    // languages that lost their grammar (rejected) still describe files;
    // drop those without any
    languages
        .retain(|l| !l.suffixes.is_empty() || !l.filenames.is_empty() || l.first_line.is_some());
    copy_source(&scanned.root, &out.join("source"))?;
    let report_text = serde_json::to_string_pretty(&report)
        .map_err(|err| ExtensionError::parse("json", err.to_string()))?;
    std::fs::write(out.join(super::REPORT_FILE), report_text)?;
    let display_name = manifest
        .display_name
        .clone()
        .filter(|d| !d.trim().is_empty())
        .unwrap_or_else(|| manifest.name.clone());
    let metadata = Metadata {
        schema: SCHEMA,
        id: id.to_string(),
        name: manifest.name.clone(),
        display_name,
        version: manifest.version.clone(),
        publisher: manifest.publisher.clone(),
        description: manifest.description.clone(),
        source,
        format: manifest.format,
        languages,
        grammars,
        // `117-file-icons`: the copy under `source/` (a theme the copy
        // budget left out is dropped)
        icon_themes: manifest
            .icon_themes
            .iter()
            .map(|t| crate::icon_theme::IconThemeRef {
                path: format!("source/{}", t.path),
                ..t.clone()
            })
            .filter(|t| out.join(&t.path).is_file())
            .collect(),
        enabled: true,
        prefer_over_builtin: true,
        license: manifest.license.clone(),
        repository: manifest.repository.clone(),
        installed_at: super::now(),
    };
    super::write(out, &metadata)?;
    Ok(Prepared { metadata, report })
}

fn prepare_textmate(
    scanned: &Scanned,
    name: &str,
    scope: Option<&str>,
    path: &Path,
    language_id: Option<&str>,
    languages: &mut Vec<Language>,
    out: &Path,
) -> (GrammarStatus, GrammarReport) {
    let file_name = format!("grammars/{}.sublime-syntax", super::slug(name));
    let mut status = GrammarStatus {
        name: name.to_string(),
        scope: scope.map(str::to_string),
        kind: GrammarKind::TextMate,
        file: Some(file_name.clone()),
        status: Status::Ok,
        error: None,
        dropped: 0,
        repository: None,
        rev: None,
        path: None,
        queries: None,
        resolution: Resolution::Unresolved,
    };
    let mut grammar_report = GrammarReport {
        name: name.to_string(),
        status: Status::Ok,
        error: None,
        self_test_ms: 0,
        dropped: Vec::new(),
    };
    let fail = |status: &mut GrammarStatus, report: &mut GrammarReport, message: String| {
        status.status = Status::Rejected;
        status.error = Some(message.clone());
        status.file = None;
        report.status = Status::Rejected;
        report.error = Some(message);
    };
    let grammar = match parse_file(&scanned.root.join(path)) {
        Ok(g) => g,
        Err(err) => {
            fail(&mut status, &mut grammar_report, err.to_string());
            return (status, grammar_report);
        }
    };
    if status.scope.is_none() {
        status.scope = Some(grammar.scope_name.clone());
    }
    // the language this grammar highlights: the manifest's, else one made
    // from the grammar's own fileTypes
    let language = match language_id.and_then(|id| languages.iter().find(|l| l.id == id)) {
        Some(language) => language.clone(),
        None => {
            let synthesized = Language {
                id: name.to_string(),
                name: grammar.name.clone(),
                suffixes: grammar
                    .file_types
                    .iter()
                    .filter_map(|t| crate::manifest::suffix(t))
                    .collect(),
                filenames: Vec::new(),
                first_line: grammar.first_line_match.clone(),
                aliases: Vec::new(),
                grammar: Some(name.to_string()),
            };
            if language_id.is_none()
                && !synthesized.suffixes.is_empty()
                && !languages.iter().any(|l| l.id == name)
            {
                languages.push(synthesized.clone());
            }
            synthesized
        }
    };
    let hidden = language_id.is_none() && language.suffixes.is_empty();
    let options = Options {
        name: language.name.clone().or_else(|| grammar.name.clone()),
        file_extensions: language
            .suffixes
            .iter()
            .chain(language.filenames.iter())
            .cloned()
            .collect(),
        first_line_match: language
            .first_line
            .clone()
            .or_else(|| grammar.first_line_match.clone()),
        hidden,
    };
    let (syntax, conversion) = convert(&grammar, &options);
    record_dropped(&conversion, &mut status, &mut grammar_report);
    let text = emit::emit(&syntax);
    let definition = match compile::compile(&text, &syntax.name) {
        Ok(d) => d,
        Err(err) => {
            fail(&mut status, &mut grammar_report, err.to_string());
            return (status, grammar_report);
        }
    };
    if let Err(err) = std::fs::write(out.join(&file_name), &text) {
        fail(&mut status, &mut grammar_report, err.to_string());
        return (status, grammar_report);
    }
    let elapsed = self_test(definition);
    grammar_report.self_test_ms = elapsed.as_millis() as u64;
    if elapsed > SLOW_AFTER {
        status.status = Status::Slow;
        grammar_report.status = Status::Slow;
    }
    (status, grammar_report)
}

fn record_dropped(
    conversion: &ConversionReport,
    status: &mut GrammarStatus,
    report: &mut GrammarReport,
) {
    status.dropped = conversion.dropped.len();
    report.dropped = conversion
        .dropped
        .iter()
        .map(|d| DroppedEntry {
            context: d.context.clone(),
            kind: d.kind.describe().to_string(),
            detail: d.detail.clone(),
        })
        .collect();
}

fn prepare_sublime(
    scanned: &Scanned,
    name: &str,
    path: &Path,
    out: &Path,
) -> (GrammarStatus, GrammarReport) {
    let file_name = format!("grammars/{}.sublime-syntax", super::slug(name));
    let mut status = GrammarStatus {
        name: name.to_string(),
        scope: None,
        kind: GrammarKind::Sublime,
        file: Some(file_name.clone()),
        status: Status::Ok,
        error: None,
        dropped: 0,
        repository: None,
        rev: None,
        path: None,
        queries: None,
        resolution: Resolution::Unresolved,
    };
    let mut report = GrammarReport {
        name: name.to_string(),
        status: Status::Ok,
        error: None,
        self_test_ms: 0,
        dropped: Vec::new(),
    };
    let result = std::fs::read_to_string(scanned.root.join(path))
        .map_err(ExtensionError::from)
        .and_then(|text| compile::compile(&text, name).map(|d| (text, d)));
    match result {
        Ok((text, definition)) => {
            status.scope = Some(definition.scope.to_string());
            if let Err(err) = std::fs::write(out.join(&file_name), text) {
                status.status = Status::Rejected;
                status.error = Some(err.to_string());
            } else {
                let elapsed = self_test(definition);
                report.self_test_ms = elapsed.as_millis() as u64;
                if elapsed > SLOW_AFTER {
                    status.status = Status::Slow;
                }
            }
        }
        Err(err) => {
            status.status = Status::Rejected;
            status.error = Some(err.to_string());
            status.file = None;
        }
    }
    report.status = status.status;
    report.error = status.error.clone();
    (status, report)
}

/// Parse a short synthetic sample with the grammar alone and time it.
fn self_test(definition: SyntaxDefinition) -> Duration {
    let scope = definition.scope;
    let mut builder = SyntaxSetBuilder::new();
    builder.add(definition);
    let set: SyntaxSet = builder.build();
    let Some(syntax) = set.find_syntax_by_scope(scope) else {
        return Duration::ZERO;
    };
    let sample = sample_text();
    let start = Instant::now();
    let mut state = ParseState::new(syntax);
    for line in sample.lines() {
        if state.parse_line(&format!("{line}\n"), &set).is_err() {
            break;
        }
        if start.elapsed() > SLOW_AFTER * 10 {
            break;
        }
    }
    start.elapsed()
}

/// About 2 KB of text with the shapes grammars react to.
fn sample_text() -> String {
    let lines = [
        "#!/usr/bin/env thing",
        "// comment line with words and 12345 numbers",
        "/* block comment */ \"a string with \\\" escapes\" 'single' `tick`",
        "if (x == 1 && y != 2) { return foo(bar, baz[0]); } else { throw new Error(\"x\"); }",
        "<tag attr=\"value\" other='v'>text &amp; more</tag> <!-- note -->",
        "def method(self, *args, **kwargs): yield from range(0x1F, 1e10)",
        "SELECT a.b, COUNT(*) FROM table WHERE c IN (1, 2, 3) GROUP BY d;",
        "key: value\n  - item\n  - [1, 2, {a: b}]",
        "\\begin{document} $x^2 + y_1$ \\end{document} @decorator #pragma once",
        "((((((((((unbalanced [[[[[ {{{{{ ))))) ]]]]] }}}}} \"\"\"\"\"\" ''''''",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa!",
        "λ → ∀x ∈ ℝ: émoji 😀 tab\there end",
    ];
    let mut out = String::new();
    while out.len() < 2048 {
        for line in lines {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// Copy the extension's files for the record, skipping what nothing reads.
fn copy_source(from: &Path, to: &Path) -> Result<(), ExtensionError> {
    let mut budget = MAX_SOURCE_BYTES;
    copy_dir(from, to, 0, &mut budget)
}

fn copy_dir(from: &Path, to: &Path, depth: usize, budget: &mut u64) -> Result<(), ExtensionError> {
    if depth > 32 {
        return Ok(());
    }
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)?.filter_map(|e| e.ok()) {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if matches!(name_str.as_ref(), "node_modules" | ".git" | "target") {
            continue;
        }
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            copy_dir(&path, &to.join(&name), depth + 1, budget)?;
        } else if file_type.is_file() {
            let len = entry.metadata()?.len();
            if len > *budget {
                tracing::debug!("source copy budget reached at {}", path.display());
                return Ok(());
            }
            *budget -= len;
            std::fs::copy(&path, to.join(&name))?;
        }
    }
    Ok(())
}

/// `out` as `PathBuf` helper for callers that build the folder path.
pub fn extension_dir(extensions_dir: &Path, id: &str) -> PathBuf {
    extensions_dir.join(id)
}
