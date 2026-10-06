//! Installed extensions on disk: `<extensions dir>/<id>/` holds the
//! prepared grammars, the queries, the unpacked source and a
//! `corvene-extension.json` ([`Metadata`]) describing all of it.

pub mod prepare;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::ExtensionError;
use crate::manifest::{Format, Language};

/// The metadata file in an extension's folder.
pub const METADATA_FILE: &str = "corvene-extension.json";
/// The conversion report next to it.
pub const REPORT_FILE: &str = "report.json";
/// The schema this build writes and reads.
pub const SCHEMA: u32 = 1;

/// Where an extension came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceKind {
    OpenVsx,
    Zed,
    Pulsar,
    GitHub,
    Url,
    LocalFile,
    LocalFolder,
    /// copied from an editor installed on this machine
    Imported,
}

impl SourceKind {
    pub fn title(self) -> &'static str {
        match self {
            SourceKind::OpenVsx => "Open VSX",
            SourceKind::Zed => "Zed",
            SourceKind::Pulsar => "Pulsar",
            SourceKind::GitHub => "GitHub",
            SourceKind::Url => "URL",
            SourceKind::LocalFile => "File",
            SourceKind::LocalFolder => "Folder",
            SourceKind::Imported => "Imported",
        }
    }

    /// The first part of an extension id.
    pub fn slug(self) -> &'static str {
        match self {
            SourceKind::OpenVsx => "openvsx",
            SourceKind::Zed => "zed",
            SourceKind::Pulsar => "pulsar",
            SourceKind::GitHub => "github",
            SourceKind::Url => "url",
            SourceKind::LocalFile => "local",
            SourceKind::LocalFolder => "local",
            SourceKind::Imported => "imported",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    pub kind: SourceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    /// the editor an import came from
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor: Option<String>,
    /// a registry's own id (`namespace.name`, Zed's id, a Pulsar name)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry_id: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GrammarKind {
    TextMate,
    Sublime,
    TreeSitter,
}

/// How a prepared grammar fared.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Ok,
    /// syntect refused it (the reason is in `error`)
    Rejected,
    /// usable, but the self-test took long: a pathological regex
    Slow,
}

/// Where a tree-sitter grammar's parser comes from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Resolution {
    /// not looked up yet
    Unresolved,
    /// Corvene's own grammar `name` (compiled in or in a pack)
    Bundled {
        name: String,
    },
    /// built from source; the library's path
    Built {
        library: String,
    },
    /// needs a build from source (flag `build-grammars-from-source`)
    NeedsBuild,
    Failed {
        error: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrammarStatus {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    pub kind: GrammarKind,
    /// `grammars/<n>.sublime-syntax`, relative to the extension folder
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    pub status: Status,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default)]
    pub dropped: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rev: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// `queries/<name>`, relative to the extension folder
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queries: Option<String>,
    #[serde(default = "unresolved")]
    pub resolution: Resolution,
}

fn unresolved() -> Resolution {
    Resolution::Unresolved
}

/// `corvene-extension.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Metadata {
    pub schema: u32,
    pub id: String,
    pub name: String,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub source: Source,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<Format>,
    #[serde(default)]
    pub languages: Vec<Language>,
    #[serde(default)]
    pub grammars: Vec<GrammarStatus>,
    /// `117-file-icons`: the file icon themes, paths relative to the
    /// extension's folder.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub icon_themes: Vec<crate::icon_theme::IconThemeRef>,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default = "yes")]
    pub prefer_over_builtin: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    /// unix seconds
    pub installed_at: u64,
}

fn yes() -> bool {
    true
}

impl Metadata {
    /// Every suffix the extension's languages claim.
    pub fn suffixes(&self) -> Vec<&str> {
        let mut out: Vec<&str> = self
            .languages
            .iter()
            .flat_map(|l| l.suffixes.iter().map(String::as_str))
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Whether any grammar highlights anything.
    pub fn usable(&self) -> bool {
        self.grammars.iter().any(|g| match g.kind {
            GrammarKind::TextMate | GrammarKind::Sublime => g.status != Status::Rejected,
            GrammarKind::TreeSitter => matches!(
                g.resolution,
                Resolution::Bundled { .. } | Resolution::Built { .. }
            ),
        })
    }
}

/// An installed extension: its folder and metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct Installed {
    pub dir: PathBuf,
    pub metadata: Metadata,
}

/// `<kind>.<namespace>.<name>` as a folder-safe id.
pub fn extension_id(kind: SourceKind, namespace: Option<&str>, name: &str) -> String {
    let mut parts = vec![kind.slug().to_string()];
    if let Some(ns) = namespace.map(slug).filter(|s| !s.is_empty()) {
        parts.push(ns);
    }
    let name = slug(name);
    parts.push(if name.is_empty() {
        "extension".to_string()
    } else {
        name
    });
    parts.join(".")
}

/// Lowercase ASCII letters, digits, `-` and `_`; everything else becomes a
/// dash, runs collapsed.
pub fn slug(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut dash = false;
    for c in text.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c.to_ascii_lowercase());
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    out.trim_end_matches('-').to_string()
}

pub fn read(dir: &Path) -> Result<Metadata, ExtensionError> {
    let text = std::fs::read_to_string(dir.join(METADATA_FILE))?;
    let metadata: Metadata = serde_json::from_str(&text)
        .map_err(|err| ExtensionError::parse("json", format!("{}: {err}", dir.display())))?;
    if metadata.schema > SCHEMA {
        return Err(ExtensionError::parse(
            "json",
            format!("{} was written by a newer Corvene", dir.display()),
        ));
    }
    Ok(metadata)
}

pub fn write(dir: &Path, metadata: &Metadata) -> Result<(), ExtensionError> {
    std::fs::create_dir_all(dir)?;
    let text = serde_json::to_string_pretty(metadata)
        .map_err(|err| ExtensionError::parse("json", err.to_string()))?;
    let partial = dir.join(format!("{METADATA_FILE}.partial"));
    std::fs::write(&partial, text)?;
    std::fs::rename(partial, dir.join(METADATA_FILE))?;
    Ok(())
}

/// Every extension under `extensions_dir`, sorted by display name.
/// Folders without readable metadata are skipped (and logged).
pub fn list(extensions_dir: &Path) -> Vec<Installed> {
    let Ok(entries) = std::fs::read_dir(extensions_dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.filter_map(|e| e.ok()) {
        let dir = entry.path();
        if !dir.is_dir() || !dir.join(METADATA_FILE).is_file() {
            continue;
        }
        match read(&dir) {
            Ok(metadata) => out.push(Installed { dir, metadata }),
            Err(err) => tracing::warn!("{}: {err}", dir.display()),
        }
    }
    out.sort_by(|a, b| {
        a.metadata
            .display_name
            .to_lowercase()
            .cmp(&b.metadata.display_name.to_lowercase())
    });
    out
}

/// Delete an extension's folder.
pub fn remove(dir: &Path) -> Result<(), ExtensionError> {
    if dir.join(METADATA_FILE).is_file() {
        std::fs::remove_dir_all(dir)?;
    }
    Ok(())
}

/// Seconds since the epoch.
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_slugs() {
        assert_eq!(
            extension_id(SourceKind::OpenVsx, Some("Bierner"), "lit-html"),
            "openvsx.bierner.lit-html"
        );
        assert_eq!(extension_id(SourceKind::Zed, None, "nix"), "zed.nix");
        assert_eq!(
            extension_id(SourceKind::LocalFile, None, "My Grammar!!"),
            "local.my-grammar"
        );
        assert_eq!(extension_id(SourceKind::Url, None, "---"), "url.extension");
        assert_eq!(slug("a__b.C"), "a__b-c");
    }

    #[test]
    fn metadata_round_trips_and_lists() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ext = dir.path().join("local.x");
        let metadata = Metadata {
            schema: SCHEMA,
            id: "local.x".into(),
            name: "x".into(),
            display_name: "X".into(),
            version: Some("1.0".into()),
            publisher: None,
            description: None,
            source: Source {
                kind: SourceKind::LocalFile,
                url: None,
                path: Some("/tmp/x.vsix".into()),
                sha256: None,
                editor: None,
                registry_id: None,
            },
            format: Some(Format::VsCode),
            languages: vec![Language {
                id: "x".into(),
                suffixes: vec!["x".into(), "xx".into()],
                ..Default::default()
            }],
            grammars: vec![GrammarStatus {
                name: "x".into(),
                scope: Some("source.x".into()),
                kind: GrammarKind::TextMate,
                file: Some("grammars/x.sublime-syntax".into()),
                status: Status::Ok,
                error: None,
                dropped: 2,
                repository: None,
                rev: None,
                path: None,
                queries: None,
                resolution: Resolution::Unresolved,
            }],
            icon_themes: Vec::new(),
            enabled: true,
            prefer_over_builtin: false,
            license: None,
            repository: None,
            installed_at: 1,
        };
        write(&ext, &metadata).expect("write");
        assert_eq!(read(&ext).expect("read"), metadata);
        assert_eq!(metadata.suffixes(), vec!["x", "xx"]);
        assert!(metadata.usable());
        std::fs::create_dir_all(dir.path().join("junk")).expect("junk");
        let listed = list(dir.path());
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].metadata.id, "local.x");
        remove(&ext).expect("remove");
        assert!(list(dir.path()).is_empty());
    }
}
