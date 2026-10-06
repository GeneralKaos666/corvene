//! What an extension says it provides, read from each editor's packaging
//! into one [`Manifest`]: the languages (file associations) and the
//! grammars behind them.

pub mod atom;
pub mod raw;
pub mod sublime;
pub mod tmbundle;
pub mod vscode;
pub mod zed;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::ExtensionError;

/// The packaging an extension came in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Format {
    /// `package.json` with `contributes.grammars` (also Cursor, VSCodium)
    VsCode,
    /// `package.json` + `grammars/*.cson` (Atom, Pulsar)
    Atom,
    /// `extension.toml` + `languages/*/config.toml`
    Zed,
    /// `*.tmbundle` (TextMate, IntelliJ's TextMate bundles)
    TmBundle,
    /// `*.sublime-syntax` / `*.tmLanguage` in a Sublime package
    Sublime,
    /// a bare grammar file
    Raw,
}

impl Format {
    pub fn title(self) -> &'static str {
        match self {
            Format::VsCode => "VS Code extension",
            Format::Atom => "Atom package",
            Format::Zed => "Zed extension",
            Format::TmBundle => "TextMate bundle",
            Format::Sublime => "Sublime Text package",
            Format::Raw => "grammar file",
        }
    }
}

/// An extension's contents, paths relative to its root.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Manifest {
    pub format: Option<Format>,
    pub name: String,
    pub display_name: Option<String>,
    pub version: Option<String>,
    pub publisher: Option<String>,
    pub description: Option<String>,
    pub repository: Option<String>,
    pub license: Option<String>,
    pub languages: Vec<Language>,
    pub grammars: Vec<GrammarRef>,
    /// File icon themes (`117-file-icons`).
    pub icon_themes: Vec<crate::icon_theme::IconThemeRef>,
}

/// A language the extension adds: when it applies, and which grammar
/// highlights it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Language {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// file extensions without the dot, lowercase (`d.ts` allowed)
    #[serde(default)]
    pub suffixes: Vec<String>,
    /// exact file names, lowercase
    #[serde(default)]
    pub filenames: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_line: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    /// the [`GrammarRef::name`] that highlights it
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grammar: Option<String>,
}

/// A grammar file the extension carries.
#[derive(Clone, Debug, PartialEq)]
pub enum GrammarRef {
    /// A TextMate grammar in any encoding.
    TextMate {
        name: String,
        scope: Option<String>,
        path: PathBuf,
        /// the language id it is registered for (VS Code); `None` for an
        /// injection-only grammar
        language: Option<String>,
        inject_to: Vec<String>,
    },
    /// A `.sublime-syntax`, syntect's own format.
    Sublime { name: String, path: PathBuf },
    /// A tree-sitter grammar: queries from the extension, the parser from
    /// Corvene's own grammars or built from `repository` at `rev`.
    TreeSitter {
        name: String,
        repository: Option<String>,
        rev: Option<String>,
        /// a subfolder of the repository holding the grammar
        path: Option<String>,
        highlights: Option<PathBuf>,
        injections: Option<PathBuf>,
        locals: Option<PathBuf>,
        /// Zed's compiled grammar, kept for the record (never loaded)
        wasm: Option<PathBuf>,
    },
}

impl GrammarRef {
    pub fn name(&self) -> &str {
        match self {
            GrammarRef::TextMate { name, .. }
            | GrammarRef::Sublime { name, .. }
            | GrammarRef::TreeSitter { name, .. } => name,
        }
    }
}

/// `.ext` / `ext` → `ext`, lowercase; empty for `.`.
pub(crate) fn suffix(text: &str) -> Option<String> {
    let s = text.trim().trim_start_matches('.').to_ascii_lowercase();
    (!s.is_empty() && !s.contains(['/', '*', '?', '[', ']', '{', '}', ',', ' '])).then_some(s)
}

/// A `fileTypes` / `path_suffixes` entry that names whole files as well
/// (`Makefile`, `Foofile`, `CMakeLists.txt`): one with an uppercase letter,
/// which no extension has. Editors match these against the whole name too.
pub(crate) fn looks_like_filename(entry: &str) -> bool {
    let entry = entry.trim();
    !entry.starts_with('.') && entry.chars().any(|c| c.is_ascii_uppercase())
}

/// A VS Code `filenamePatterns` glob as a suffix (`*.foo.bar` → `foo.bar`)
/// or a literal file name; other globs are dropped.
pub(crate) fn pattern(text: &str) -> (Option<String>, Option<String>) {
    let text = text.trim();
    if let Some(rest) = text.strip_prefix("*.") {
        return (suffix(rest), None);
    }
    if let Some(rest) = text.strip_prefix("**/") {
        return pattern(rest);
    }
    if text.contains(['*', '?', '[', '{', '/']) {
        return (None, None);
    }
    (None, Some(text.to_ascii_lowercase()))
}

/// A repository field as its URL (`{"url": …}` or a string, `git+` and
/// `.git` trimmed).
pub(crate) fn repository_url(value: Option<&crate::value::Value>) -> Option<String> {
    let text = match value? {
        crate::value::Value::Str(s) => s.as_str(),
        map @ crate::value::Value::Map(_) => map.str_of("url")?,
        _ => return None,
    };
    let text = text.trim().trim_start_matches("git+");
    let text = text.strip_suffix(".git").unwrap_or(text);
    if let Some(short) = text.strip_prefix("github:") {
        return Some(format!("https://github.com/{short}"));
    }
    if text.starts_with("https://") || text.starts_with("http://") {
        return Some(text.replace("http://", "https://"));
    }
    if let Some(rest) = text.strip_prefix("git@github.com:") {
        return Some(format!("https://github.com/{rest}"));
    }
    // `owner/repo`
    let mut parts = text.split('/');
    if let (Some(owner), Some(repo), None) = (parts.next(), parts.next(), parts.next())
        && !owner.is_empty()
        && !repo.is_empty()
        && !text.contains(':')
    {
        return Some(format!("https://github.com/{owner}/{repo}"));
    }
    None
}

/// Read the manifest of the extension rooted at `root`, trying each format.
/// `Ok(None)` when the folder is none of them.
pub fn read(root: &Path) -> Result<Option<Manifest>, ExtensionError> {
    if !root.is_dir() {
        return Ok(None);
    }
    for reader in [
        vscode::read,
        zed::read,
        atom::read,
        tmbundle::read,
        sublime::read,
    ] {
        if let Some(manifest) = reader(root)? {
            return Ok(Some(manifest));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::parse_json;

    #[test]
    fn suffixes_and_patterns() {
        assert_eq!(suffix(".TS"), Some("ts".into()));
        assert_eq!(suffix("d.ts"), Some("d.ts".into()));
        assert_eq!(suffix("."), None);
        assert_eq!(suffix("*.x"), None);
        assert_eq!(pattern("*.foo.bar"), (Some("foo.bar".into()), None));
        assert_eq!(pattern("**/Dockerfile"), (None, Some("dockerfile".into())));
        assert_eq!(pattern("Makefile"), (None, Some("makefile".into())));
        assert_eq!(pattern("*.{a,b}"), (None, None));
    }

    #[test]
    fn repository_urls() {
        let v = |s: &str| parse_json(s).expect("json");
        assert_eq!(
            repository_url(Some(&v(
                r#"{"type": "git", "url": "git+https://github.com/a/b.git"}"#
            ))),
            Some("https://github.com/a/b".into())
        );
        assert_eq!(
            repository_url(Some(&v(r#""a/b""#))),
            Some("https://github.com/a/b".into())
        );
        assert_eq!(
            repository_url(Some(&v(r#""github:a/b""#))),
            Some("https://github.com/a/b".into())
        );
        assert_eq!(
            repository_url(Some(&v(r#""git@github.com:a/b.git""#))),
            Some("https://github.com/a/b".into())
        );
        assert_eq!(repository_url(Some(&v(r#""nonsense""#))), None);
    }
}
