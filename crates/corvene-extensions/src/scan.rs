//! Finding the extension in whatever the user handed over: an archive, a
//! folder, a bare grammar file, or a whole repository checkout that holds
//! an extension somewhere inside it.

use std::path::{Path, PathBuf};

use crate::ExtensionError;
use crate::archive::{Kind, Limits};
use crate::manifest::{self, Format, GrammarRef, Manifest};
use crate::tm::GrammarFormat;

/// An extension and where its files are.
#[derive(Clone, Debug, PartialEq)]
pub struct Scanned {
    pub root: PathBuf,
    pub manifest: Manifest,
}

/// Folders never searched for an extension.
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    "target",
    "test",
    "tests",
    "__tests__",
    "spec",
    "specs",
    "fixtures",
    "snippets",
    "themes",
    "images",
    "icons",
];

/// How deep a repository is searched.
const MAX_SCAN_DEPTH: usize = 6;

/// Scan `path`: an archive is unpacked into `staging` first.
pub fn scan(path: &Path, staging: &Path, limits: &Limits) -> Result<Scanned, ExtensionError> {
    if path.is_dir() {
        return scan_dir(path);
    }
    if !path.is_file() {
        return Err(ExtensionError::NotAnExtension(format!(
            "{} does not exist",
            path.display()
        )));
    }
    if Kind::sniff(path)?.is_some() {
        let unpacked = staging.join("unpacked");
        crate::archive::extract(path, &unpacked, limits)?;
        let mut scanned = scan_dir(&unpacked)?;
        if scanned.manifest.name == "unpacked" || scanned.manifest.name.is_empty() {
            scanned.manifest.name = archive_stem(path);
        }
        return Ok(scanned);
    }
    if (GrammarFormat::from_path(path).is_some() || is_sublime_syntax(path))
        && let Some(manifest) = manifest::raw::read(path)?
    {
        return Ok(Scanned {
            root: path.parent().unwrap_or(Path::new(".")).to_path_buf(),
            manifest,
        });
    }
    Err(ExtensionError::NotAnExtension(format!(
        "{} is not an extension archive or a grammar file",
        path.display()
    )))
}

/// `my-ext-1.2.3.vsix` → `my-ext-1.2.3`.
fn archive_stem(path: &Path) -> String {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("extension");
    let name = name
        .strip_suffix(".tar.gz")
        .or_else(|| name.strip_suffix(".tgz"))
        .unwrap_or(name);
    Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(name)
        .to_string()
}

fn is_sublime_syntax(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("sublime-syntax"))
}

/// Scan a folder: the extension at its root, else the ones found inside it
/// (merged into one when several), else its loose grammar files.
pub fn scan_dir(dir: &Path) -> Result<Scanned, ExtensionError> {
    // a `.vsix` unpacks to `extension/` next to its metadata
    for candidate in [dir.to_path_buf(), dir.join("extension")] {
        if let Some(manifest) = manifest::read(&candidate)? {
            return Ok(Scanned {
                root: candidate,
                manifest,
            });
        }
    }
    let mut found: Vec<Scanned> = Vec::new();
    walk(dir, 0, &mut found)?;
    match found.len() {
        0 => loose_grammars(dir),
        1 => Ok(found.remove(0)),
        _ => Ok(merge(dir, found)),
    }
}

fn walk(dir: &Path, depth: usize, found: &mut Vec<Scanned>) -> Result<(), ExtensionError> {
    if depth > MAX_SCAN_DEPTH {
        return Ok(());
    }
    let mut children: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| !SKIP_DIRS.contains(&n) && !n.starts_with('.'))
        })
        .collect();
    children.sort();
    for child in children {
        if let Some(manifest) = manifest::read(&child)? {
            found.push(Scanned {
                root: child,
                manifest,
            });
            continue;
        }
        walk(&child, depth + 1, found)?;
    }
    Ok(())
}

/// Several extensions in one tree become one, rooted at `dir`, each
/// grammar's path prefixed with its subfolder.
fn merge(dir: &Path, found: Vec<Scanned>) -> Scanned {
    let mut manifest = Manifest {
        format: found.first().and_then(|s| s.manifest.format),
        name: dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("extensions")
            .to_string(),
        ..Default::default()
    };
    for scanned in found {
        let prefix = scanned
            .root
            .strip_prefix(dir)
            .unwrap_or(&scanned.root)
            .to_path_buf();
        let m = scanned.manifest;
        manifest.repository = manifest.repository.or(m.repository);
        manifest.license = manifest.license.or(m.license);
        manifest.version = manifest.version.or(m.version);
        for mut grammar in m.grammars {
            match &mut grammar {
                GrammarRef::TextMate { path, .. } | GrammarRef::Sublime { path, .. } => {
                    *path = prefix.join(&*path);
                }
                GrammarRef::TreeSitter {
                    highlights,
                    injections,
                    locals,
                    wasm,
                    ..
                } => {
                    for p in [highlights, injections, locals, wasm].into_iter().flatten() {
                        *p = prefix.join(&*p);
                    }
                }
            }
            if !manifest.grammars.iter().any(|g| g.name() == grammar.name()) {
                manifest.grammars.push(grammar);
            }
        }
        for language in m.languages {
            if !manifest.languages.iter().any(|l| l.id == language.id) {
                manifest.languages.push(language);
            }
        }
        for mut theme in m.icon_themes {
            theme.path = prefix.join(&theme.path).to_string_lossy().into_owned();
            if !manifest.icon_themes.iter().any(|t| t.id == theme.id) {
                manifest.icon_themes.push(theme);
            }
        }
    }
    Scanned {
        root: dir.to_path_buf(),
        manifest,
    }
}

/// Grammar files anywhere in the tree, as one raw extension.
fn loose_grammars(dir: &Path) -> Result<Scanned, ExtensionError> {
    let mut files = Vec::new();
    collect_grammar_files(dir, 0, &mut files)?;
    files.sort();
    let mut manifest = Manifest {
        format: Some(Format::Raw),
        name: dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("grammars")
            .to_string(),
        ..Default::default()
    };
    for file in &files {
        if is_sublime_syntax(file) {
            if let Some(m) = manifest::raw::read(file)? {
                manifest.grammars.extend(m.grammars);
                manifest.languages.extend(m.languages);
            }
        } else {
            manifest::tmbundle::add_textmate_file(dir, file, &mut manifest)?;
        }
    }
    if manifest.grammars.is_empty() {
        return Err(ExtensionError::NotAnExtension(format!(
            "{} holds no grammar Corvene can use (no VS Code, Zed, Atom, TextMate or Sublime grammar found)",
            dir.display()
        )));
    }
    if manifest.languages.len() == 1 {
        manifest.display_name = manifest.languages[0].name.clone();
    }
    Ok(Scanned {
        root: dir.to_path_buf(),
        manifest,
    })
}

fn collect_grammar_files(
    dir: &Path,
    depth: usize,
    out: &mut Vec<PathBuf>,
) -> Result<(), ExtensionError> {
    if depth > MAX_SCAN_DEPTH {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)?.filter_map(|e| e.ok()) {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if !SKIP_DIRS.contains(&name.as_str()) && !name.starts_with('.') {
                collect_grammar_files(&path, depth + 1, out)?;
            }
        } else if is_sublime_syntax(&path) || looks_like_grammar(&path, &name) {
            out.push(path);
        }
    }
    Ok(())
}

/// A file that is probably a TextMate grammar: by its name, or a `.json` /
/// `.plist` whose text names a scope and patterns.
fn looks_like_grammar(path: &Path, name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".tmlanguage")
        || lower.ends_with(".tmlanguage.json")
        || lower.ends_with(".yaml-tmlanguage")
        || lower.ends_with(".cson")
    {
        return true;
    }
    if lower.ends_with(".json") || lower.ends_with(".plist") {
        let Ok(meta) = std::fs::metadata(path) else {
            return false;
        };
        if meta.len() > crate::MAX_GRAMMAR_BYTES as u64 {
            return false;
        }
        let Ok(text) = std::fs::read_to_string(path) else {
            return false;
        };
        return text.contains("scopeName") && text.contains("patterns");
    }
    false
}
