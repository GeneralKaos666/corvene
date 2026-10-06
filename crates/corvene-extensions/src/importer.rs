//! Grammars from the editors installed on this machine: their extension
//! folders are scanned for language-contributing packages, which the user
//! can copy into Corvene without a download.

use std::path::{Path, PathBuf};

use crate::manifest::{self, GrammarRef};

/// An editor whose extension folder is scanned.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Editor {
    VsCode,
    VsCodeInsiders,
    Cursor,
    VsCodium,
    Zed,
    Pulsar,
    Atom,
    SublimeText,
    JetBrains,
}

impl Editor {
    pub const ALL: [Editor; 9] = [
        Editor::VsCode,
        Editor::VsCodeInsiders,
        Editor::Cursor,
        Editor::VsCodium,
        Editor::Zed,
        Editor::Pulsar,
        Editor::Atom,
        Editor::SublimeText,
        Editor::JetBrains,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Editor::VsCode => "Visual Studio Code",
            Editor::VsCodeInsiders => "Visual Studio Code Insiders",
            Editor::Cursor => "Cursor",
            Editor::VsCodium => "VSCodium",
            Editor::Zed => "Zed",
            Editor::Pulsar => "Pulsar",
            Editor::Atom => "Atom",
            Editor::SublimeText => "Sublime Text",
            Editor::JetBrains => "JetBrains IDEs",
        }
    }

    /// Folders whose children are extensions (or, for Sublime, packages).
    pub fn roots(self, home: &Path) -> Vec<PathBuf> {
        let app_support = home.join("Library/Application Support");
        match self {
            Editor::VsCode => vec![home.join(".vscode/extensions")],
            Editor::VsCodeInsiders => vec![home.join(".vscode-insiders/extensions")],
            Editor::Cursor => vec![home.join(".cursor/extensions")],
            Editor::VsCodium => vec![home.join(".vscode-oss/extensions")],
            Editor::Zed => vec![
                app_support.join("Zed/extensions/installed"),
                home.join(".local/share/zed/extensions/installed"),
            ],
            Editor::Pulsar => vec![home.join(".pulsar/packages")],
            Editor::Atom => vec![home.join(".atom/packages")],
            Editor::SublimeText => vec![
                app_support.join("Sublime Text/Packages"),
                app_support.join("Sublime Text/Installed Packages"),
                app_support.join("Sublime Text 3/Packages"),
                app_support.join("Sublime Text 3/Installed Packages"),
                home.join(".config/sublime-text/Packages"),
                home.join(".config/sublime-text/Installed Packages"),
            ],
            Editor::JetBrains => {
                // every IDE's folder may hold user TextMate bundles
                let mut out = Vec::new();
                for base in [
                    app_support.join("JetBrains"),
                    home.join(".config/JetBrains"),
                ] {
                    if let Ok(entries) = std::fs::read_dir(&base) {
                        for entry in entries.filter_map(|e| e.ok()) {
                            let textmate = entry.path().join("textmate");
                            if textmate.is_dir() {
                                out.push(textmate);
                            }
                        }
                    }
                }
                out
            }
        }
    }
}

/// An extension found in an editor's folder.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportCandidate {
    pub editor: Editor,
    pub path: PathBuf,
    pub name: String,
    pub display_name: String,
    pub version: Option<String>,
    pub publisher: Option<String>,
    /// language names (or ids) the extension adds
    pub languages: Vec<String>,
    /// file suffixes it covers
    pub suffixes: Vec<String>,
    pub tree_sitter: bool,
    /// `117-file-icons`: the file icon themes it adds
    pub icon_themes: Vec<String>,
}

/// Scan every editor's folders under `home`.
pub fn scan(home: &Path) -> Vec<ImportCandidate> {
    let mut out = Vec::new();
    for editor in Editor::ALL {
        for root in editor.roots(home) {
            scan_root(editor, &root, &mut out);
        }
    }
    out.sort_by(|a, b| {
        (a.editor, a.display_name.to_lowercase()).cmp(&(b.editor, b.display_name.to_lowercase()))
    });
    out
}

fn scan_root(editor: Editor, root: &Path, out: &mut Vec<ImportCandidate>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "User" || name == "Default" {
            continue;
        }
        if path.is_file() {
            // Sublime's Installed Packages are zips; a `.sublime-package`
            // is scanned by the install flow itself
            if name.ends_with(".sublime-package") {
                out.push(ImportCandidate {
                    editor,
                    path: path.clone(),
                    name: name.trim_end_matches(".sublime-package").to_string(),
                    display_name: name.trim_end_matches(".sublime-package").to_string(),
                    version: None,
                    publisher: None,
                    languages: Vec::new(),
                    suffixes: Vec::new(),
                    tree_sitter: false,
                    icon_themes: Vec::new(),
                });
            }
            continue;
        }
        if !path.is_dir() {
            continue;
        }
        let manifest = match manifest::read(&path) {
            Ok(Some(m)) => m,
            Ok(None) => continue,
            Err(err) => {
                tracing::debug!("{}: {err}", path.display());
                continue;
            }
        };
        if manifest.grammars.is_empty() && manifest.icon_themes.is_empty() {
            continue;
        }
        let mut suffixes: Vec<String> = manifest
            .languages
            .iter()
            .flat_map(|l| l.suffixes.iter().cloned())
            .collect();
        suffixes.sort();
        suffixes.dedup();
        let languages: Vec<String> = manifest
            .languages
            .iter()
            .map(|l| l.name.clone().unwrap_or_else(|| l.id.clone()))
            .collect();
        out.push(ImportCandidate {
            editor,
            path,
            name: manifest.name.clone(),
            display_name: manifest
                .display_name
                .clone()
                .filter(|d| !d.trim().is_empty())
                .unwrap_or_else(|| manifest.name.clone()),
            version: manifest.version.clone(),
            publisher: manifest.publisher.clone(),
            languages,
            suffixes,
            tree_sitter: manifest
                .grammars
                .iter()
                .any(|g| matches!(g, GrammarRef::TreeSitter { .. })),
            icon_themes: manifest
                .icon_themes
                .iter()
                .map(|t| t.label.clone())
                .collect(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_extensions_in_editor_folders() {
        let home = tempfile::tempdir().expect("tempdir");
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let copy = |from: &Path, to: &Path| {
            std::fs::create_dir_all(to.parent().expect("parent")).expect("mkdir");
            copy_dir(from, to);
        };
        copy(
            &fixtures.join("vscode-ext"),
            &home
                .path()
                .join(".vscode/extensions/corvene-tests.sample-langs-1.2.3"),
        );
        copy(
            &fixtures.join("zed-ext"),
            &home
                .path()
                .join("Library/Application Support/Zed/extensions/installed/foo-zed"),
        );
        copy(
            &fixtures.join("atom-pkg"),
            &home.path().join(".pulsar/packages/language-baz"),
        );
        // a theme-only extension: no grammars, skipped
        let theme = home.path().join(".vscode/extensions/someone.theme-1.0.0");
        std::fs::create_dir_all(&theme).expect("mkdir");
        std::fs::write(
            theme.join("package.json"),
            r#"{"name":"theme","contributes":{"themes":[]}}"#,
        )
        .expect("write");
        let found = scan(home.path());
        let names: Vec<(Editor, &str)> =
            found.iter().map(|c| (c.editor, c.name.as_str())).collect();
        assert_eq!(
            names,
            vec![
                (Editor::VsCode, "sample-langs"),
                (Editor::Zed, "foo-zed"),
                (Editor::Pulsar, "language-baz"),
            ]
        );
        assert!(found[0].suffixes.contains(&"foo".to_string()));
        assert!(found[1].tree_sitter);
        assert_eq!(found[0].display_name, "Sample Languages");
    }

    fn copy_dir(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).expect("mkdir");
        for entry in std::fs::read_dir(from)
            .expect("read_dir")
            .filter_map(|e| e.ok())
        {
            let target = to.join(entry.file_name());
            if entry.path().is_dir() {
                copy_dir(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), target).expect("copy");
            }
        }
    }
}
