//! File icon themes from editor extensions (Corvene `117-file-icons`):
//! VS Code's `contributes.iconThemes` (an icon theme JSON with
//! `iconDefinitions`, `fileNames`, `fileExtensions`, `languageIds`, folder
//! maps, a `light` section and icon fonts) and Zed's `icon_themes/*.json`
//! (`file_stems`, `file_suffixes`, `file_icons`, directory icons). GHD has
//! no file icons.
//!
//! A theme resolves a file to one [`Icon`] the way its editor does: VS Code
//! tries the file name, then its extensions longest first, then the
//! language id, then the default file icon; Zed tries the full name in
//! `file_stems`, then the suffixes longest first.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::ExtensionError;
use crate::value::{Value, parse_json};

/// The editor format an icon theme is written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IconThemeFormat {
    VsCode,
    Zed,
}

/// An icon theme an extension contributes, as its manifest names it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IconThemeRef {
    /// Unique within the extension (VS Code `id`, Zed theme `name`).
    pub id: String,
    pub label: String,
    /// The theme file, relative to the extension's root.
    pub path: String,
    pub format: IconThemeFormat,
    /// Zed `appearance` (`dark` / `light`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub appearance: Option<String>,
}

/// What an icon is drawn from.
#[derive(Clone, Debug, PartialEq)]
pub enum Icon {
    /// An SVG drawn in its own colours.
    Svg(PathBuf),
    /// An SVG painted with `currentColor`: drawn as a mask in the text
    /// colour.
    SvgMask(PathBuf),
    /// A PNG or another raster image.
    Image(PathBuf),
    /// A glyph of one of the theme's fonts (VS Code `fontCharacter`), as
    /// an SVG of its outline ([`glyph_svg`]: text rendering would need the
    /// font registered, and GPUI refuses fonts without an `m`).
    Glyph {
        svg: std::sync::Arc<[u8]>,
        /// `#rrggbb` (`fontColor`), else the text colour.
        color: Option<String>,
    },
}

/// The lookup tables of a theme (VS Code has a second set for light
/// themes).
#[derive(Clone, Debug, Default, PartialEq)]
struct Tables {
    file: Option<usize>,
    folder: Option<usize>,
    folder_expanded: Option<usize>,
    names: HashMap<String, usize>,
    suffixes: HashMap<String, usize>,
    languages: HashMap<String, usize>,
    folder_names: HashMap<String, usize>,
    folder_names_expanded: HashMap<String, usize>,
}

/// A loaded icon theme.
#[derive(Clone, Debug, PartialEq)]
pub struct IconTheme {
    pub label: String,
    pub format: IconThemeFormat,
    icons: Vec<Icon>,
    dark: Tables,
    light: Option<Tables>,
    /// VS Code `hidesExplorerArrows`.
    pub hides_arrows: bool,
}

impl IconTheme {
    /// Loads the theme `theme` names, inside an extension at `root`.
    pub fn load(root: &Path, theme: &IconThemeRef) -> Result<Self, ExtensionError> {
        let file = root.join(&theme.path);
        let text = std::fs::read_to_string(&file)?;
        let value = parse_json(&text)?;
        match theme.format {
            IconThemeFormat::VsCode => {
                let base = file.parent().unwrap_or(root);
                read_vscode(&value, base, &theme.label)
            }
            IconThemeFormat::Zed => read_zed(&value, root, &theme.id),
        }
    }

    pub fn icon(&self, index: usize) -> Option<&Icon> {
        self.icons.get(index)
    }

    fn tables(&self, light: bool) -> impl Iterator<Item = &Tables> {
        light
            .then_some(self.light.as_ref())
            .flatten()
            .into_iter()
            .chain(std::iter::once(&self.dark))
    }

    /// The icon of a file named `name` (no folders). `language` maps a
    /// file name to a VS Code language id (for `languageIds`).
    pub fn file_icon(
        &self,
        name: &str,
        light: bool,
        language: impl Fn(&str) -> Option<String>,
    ) -> Option<&Icon> {
        self.file_icon_index(name, light, language)
            .and_then(|i| self.icons.get(i))
    }

    /// [`Self::file_icon`] as an index for [`Self::icon`] (to cache).
    pub fn file_icon_index(
        &self,
        name: &str,
        light: bool,
        language: impl Fn(&str) -> Option<String>,
    ) -> Option<usize> {
        let lower = name.to_lowercase();
        let found = self.lookup(light, |t| t.names.get(&lower).copied());
        let found = found.or_else(|| {
            suffixes(&lower)
                .find_map(|suffix| self.lookup(light, |t| t.suffixes.get(suffix).copied()))
        });
        let found = found.or_else(|| {
            let id = language(name)?;
            self.lookup(light, |t| t.languages.get(&id).copied())
        });
        found.or_else(|| self.lookup(light, |t| t.file))
    }

    /// The icon of a folder named `name` (its last path segment).
    pub fn folder_icon(&self, name: &str, expanded: bool, light: bool) -> Option<&Icon> {
        let lower = name.to_lowercase();
        let found = if expanded {
            self.lookup(light, |t| t.folder_names_expanded.get(&lower).copied())
                .or_else(|| self.lookup(light, |t| t.folder_expanded))
                .or_else(|| self.lookup(light, |t| t.folder_names.get(&lower).copied()))
        } else {
            self.lookup(light, |t| t.folder_names.get(&lower).copied())
        };
        found
            .or_else(|| self.lookup(light, |t| t.folder))
            .and_then(|i| self.icons.get(i))
    }

    /// The first table (light, then the base one) that has an answer.
    fn lookup(&self, light: bool, get: impl Fn(&Tables) -> Option<usize>) -> Option<usize> {
        self.tables(light).find_map(get)
    }
}

/// `a.b.c` → `b.c`, `c`: what follows each dot, longest first, as VS Code
/// tries them (a dotfile's whole name counts: `.eslintrc.json` →
/// `eslintrc.json`, `json`).
fn suffixes(name: &str) -> impl Iterator<Item = &str> {
    name.char_indices()
        .filter(|&(i, c)| c == '.' && i + 1 < name.len())
        .map(move |(i, _)| &name[i + 1..])
}

fn read_vscode(value: &Value, base: &Path, label: &str) -> Result<IconTheme, ExtensionError> {
    let parse = |message: &str| ExtensionError::Parse {
        format: "icon theme",
        message: message.to_string(),
    };
    let definitions = value
        .get("iconDefinitions")
        .and_then(Value::as_map)
        .ok_or_else(|| parse("no iconDefinitions"))?;
    // (id, TrueType data)
    let mut fonts: Vec<(String, Vec<u8>)> = Vec::new();
    for font in value.get("fonts").and_then(Value::as_list).unwrap_or(&[]) {
        let Some(id) = font.str_of("id") else {
            continue;
        };
        let Some(src) = font
            .get("src")
            .and_then(Value::as_list)
            .and_then(|l| l.iter().find_map(|s| s.str_of("path")))
        else {
            continue;
        };
        let Ok(bytes) = std::fs::read(base.join(src)) else {
            continue;
        };
        // WOFF2 needs Brotli and table transforms: its glyphs are left out
        let Some(data) = decode_font(&bytes) else {
            continue;
        };
        fonts.push((id.to_string(), data));
    }
    let default_font = fonts.first().map(|_| 0);
    let mut icons = Vec::new();
    let mut ids = HashMap::new();
    for (id, def) in definitions {
        let icon = if let Some(path) = def.str_of("iconPath") {
            let path = base.join(path);
            if has_extension(&path, "svg") {
                Icon::Svg(path)
            } else {
                Icon::Image(path)
            }
        } else if let Some(character) = def.str_of("fontCharacter").and_then(font_character) {
            let font = match def.str_of("fontId") {
                Some(font_id) => fonts.iter().position(|(id, _)| id == font_id),
                None => default_font,
            };
            let Some(svg) = font.and_then(|f| glyph_svg(&fonts[f].1, character)) else {
                continue;
            };
            Icon::Glyph {
                svg: svg.into(),
                color: def.str_of("fontColor").map(str::to_string),
            }
        } else {
            continue;
        };
        ids.insert(id.as_str(), icons.len());
        icons.push(icon);
    }
    let tables = |v: &Value| {
        let one = |key: &str| v.str_of(key).and_then(|id| ids.get(id).copied());
        let map = |key: &str| -> HashMap<String, usize> {
            v.get(key)
                .and_then(Value::as_map)
                .unwrap_or(&[])
                .iter()
                .filter_map(|(k, id)| Some((k.to_lowercase(), *ids.get(id.as_str()?)?)))
                .collect()
        };
        Tables {
            file: one("file"),
            folder: one("folder"),
            folder_expanded: one("folderExpanded"),
            names: map("fileNames"),
            suffixes: map("fileExtensions"),
            languages: map("languageIds"),
            folder_names: map("folderNames"),
            folder_names_expanded: map("folderNamesExpanded"),
        }
    };
    let dark = tables(value);
    let light = value.get("light").map(tables);
    Ok(IconTheme {
        label: label.to_string(),
        format: IconThemeFormat::VsCode,
        icons,
        dark,
        light,
        hides_arrows: value
            .get("hidesExplorerArrows")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

fn read_zed(value: &Value, root: &Path, name: &str) -> Result<IconTheme, ExtensionError> {
    let parse = |message: String| ExtensionError::Parse {
        format: "icon theme",
        message,
    };
    let theme = value
        .get("themes")
        .and_then(Value::as_list)
        .unwrap_or(&[])
        .iter()
        .find(|t| t.str_of("name") == Some(name))
        .ok_or_else(|| parse(format!("no icon theme named {name}")))?;
    let mut icons = Vec::new();
    let mut paths: HashMap<String, usize> = HashMap::new();
    let mut icon_for = |path: &str, icons: &mut Vec<Icon>| -> usize {
        if let Some(&i) = paths.get(path) {
            return i;
        }
        let full = root.join(path);
        let icon = if has_extension(&full, "svg") {
            // Zed's own icons use `currentColor` and take the text colour
            let mask = std::fs::read_to_string(&full)
                .map(|svg| svg.contains("currentColor"))
                .unwrap_or(false);
            if mask {
                Icon::SvgMask(full)
            } else {
                Icon::Svg(full)
            }
        } else {
            Icon::Image(full)
        };
        icons.push(icon);
        paths.insert(path.to_string(), icons.len() - 1);
        icons.len() - 1
    };
    let mut keys: HashMap<&str, usize> = HashMap::new();
    for (key, def) in theme
        .get("file_icons")
        .and_then(Value::as_map)
        .unwrap_or(&[])
    {
        if let Some(path) = def.str_of("path") {
            keys.insert(key, icon_for(path, &mut icons));
        }
    }
    let keyed = |field: &str| -> HashMap<String, usize> {
        let mut map = HashMap::new();
        for (k, key) in theme.get(field).and_then(Value::as_map).unwrap_or(&[]) {
            if let Some(&i) = key.as_str().and_then(|key| keys.get(key)) {
                map.entry(k.to_lowercase()).or_insert(i);
            }
        }
        map
    };
    let names = keyed("file_stems");
    let suffixes = keyed("file_suffixes");
    // Zed maps other files to its own file types first (`file_types.json`),
    // mostly named like VS Code's language ids
    let mut languages: HashMap<String, usize> =
        keys.iter().map(|(key, &i)| (key.to_string(), i)).collect();
    for (language, key) in ZED_TYPES {
        if let Some(&i) = keys.get(key) {
            languages.entry(language.to_string()).or_insert(i);
        }
    }
    let mut tables = Tables {
        file: keys.get("default").or_else(|| keys.get("file")).copied(),
        names,
        suffixes,
        languages,
        ..Tables::default()
    };
    if let Some(dirs) = theme.get("directory_icons") {
        tables.folder = dirs.str_of("collapsed").map(|p| icon_for(p, &mut icons));
        tables.folder_expanded = dirs.str_of("expanded").map(|p| icon_for(p, &mut icons));
    }
    for (folder, dirs) in theme
        .get("named_directory_icons")
        .and_then(Value::as_map)
        .unwrap_or(&[])
    {
        let folder = folder.to_lowercase();
        if let Some(p) = dirs.str_of("collapsed") {
            let i = icon_for(p, &mut icons);
            tables.folder_names.entry(folder.clone()).or_insert(i);
        }
        if let Some(p) = dirs.str_of("expanded") {
            let i = icon_for(p, &mut icons);
            tables.folder_names_expanded.entry(folder).or_insert(i);
        }
    }
    Ok(IconTheme {
        label: name.to_string(),
        format: IconThemeFormat::Zed,
        icons,
        dark: tables,
        light: None,
        hides_arrows: false,
    })
}

/// The icon themes of a VS Code `package.json`'s `contributes.iconThemes`.
pub fn vscode_refs(package: &Value) -> Vec<IconThemeRef> {
    package
        .get("contributes")
        .and_then(|c| c.get("iconThemes"))
        .and_then(Value::as_list)
        .unwrap_or(&[])
        .iter()
        .filter_map(|t| {
            let id = t.str_of("id")?.to_string();
            let path = t.str_of("path")?.trim_start_matches("./").to_string();
            Some(IconThemeRef {
                label: t.str_of("label").unwrap_or(&id).to_string(),
                id,
                path,
                format: IconThemeFormat::VsCode,
                appearance: None,
            })
        })
        .collect()
}

/// The icon themes of the Zed theme files `files` (paths relative to the
/// extension `root`).
pub fn zed_refs(root: &Path, files: &[String]) -> Vec<IconThemeRef> {
    let mut refs = Vec::new();
    for file in files {
        let rel = file.trim_start_matches("./");
        let Ok(text) = std::fs::read_to_string(root.join(rel)) else {
            continue;
        };
        let Ok(value) = parse_json(&text) else {
            continue;
        };
        for theme in value.get("themes").and_then(Value::as_list).unwrap_or(&[]) {
            let Some(name) = theme.str_of("name") else {
                continue;
            };
            refs.push(IconThemeRef {
                id: name.to_string(),
                label: name.to_string(),
                path: rel.to_string(),
                format: IconThemeFormat::Zed,
                appearance: theme.str_of("appearance").map(str::to_string),
            });
        }
    }
    refs
}

/// VS Code language ids whose Zed file type has another name.
const ZED_TYPES: &[(&str, &str)] = &[
    ("shellscript", "terminal"),
    ("ignore", "vcs"),
    ("git-commit", "vcs"),
    ("git-rebase", "vcs"),
    ("properties", "settings"),
    ("ini", "settings"),
    ("dockerfile", "docker"),
    ("javascriptreact", "react"),
    ("typescriptreact", "react"),
    ("plaintext", "document"),
    ("restructuredtext", "document"),
    ("jsonc", "json"),
    ("jsonl", "json"),
    ("objective-c", "c"),
    ("objective-cpp", "cpp"),
    ("proto3", "proto"),
    ("terraform", "hcl"),
    ("bat", "terminal"),
    ("powershell", "terminal"),
];

/// VS Code's built-in language ids by file suffix and name (its
/// `extensions/*/package.json` `contributes.languages`), for a theme's
/// `languageIds` (Material Icon Theme maps many file types only there).
const LANGUAGES: &[(&str, &[&str], &[&str])] = &[
    ("bat", &["bat", "cmd"], &[]),
    ("c", &["c", "i"], &[]),
    (
        "clojure",
        &["clj", "cljs", "cljc", "cljx", "clojure", "edn"],
        &[],
    ),
    ("coffeescript", &["coffee", "cson", "iced"], &[]),
    (
        "cpp",
        &[
            "cpp", "cc", "cxx", "c++", "hpp", "hh", "hxx", "h++", "h", "ii", "ino", "inl", "ipp",
            "ixx", "tpp", "txx",
        ],
        &[],
    ),
    ("csharp", &["cs", "csx", "cake"], &[]),
    ("css", &["css"], &[]),
    ("cuda-cpp", &["cu", "cuh"], &[]),
    ("dart", &["dart"], &[]),
    ("diff", &["diff", "patch", "rej"], &[]),
    (
        "dockerfile",
        &["dockerfile", "containerfile"],
        &["dockerfile", "containerfile"],
    ),
    ("fsharp", &["fs", "fsi", "fsx", "fsscript"], &[]),
    ("git-commit", &[], &["commit_editmsg", "merge_msg"]),
    ("git-rebase", &[], &["git-rebase-todo"]),
    ("go", &["go"], &[]),
    (
        "groovy",
        &["groovy", "gvy", "gradle", "jenkinsfile", "nf"],
        &["jenkinsfile"],
    ),
    ("handlebars", &["handlebars", "hbs", "hjs"], &[]),
    (
        "hlsl",
        &[
            "hlsl", "hlsli", "fx", "fxh", "vsh", "psh", "cginc", "compute",
        ],
        &[],
    ),
    (
        "html",
        &[
            "html", "htm", "shtml", "xhtml", "xht", "mdoc", "jsp", "asp", "aspx", "jshtm", "volt",
            "ejs", "rhtml",
        ],
        &[],
    ),
    (
        "ignore",
        &["gitignore_global", "gitignore", "git-blame-ignore-revs"],
        &[".gitignore", ".npmignore", ".dockerignore", ".vscodeignore"],
    ),
    ("ini", &["ini"], &[]),
    ("java", &["java", "jav"], &[]),
    (
        "javascript",
        &["js", "es6", "mjs", "cjs", "pac"],
        &["jakefile"],
    ),
    ("javascriptreact", &["jsx"], &[]),
    (
        "json",
        &[
            "json",
            "bowerrc",
            "jscsrc",
            "webmanifest",
            "js.map",
            "css.map",
            "ts.map",
            "har",
            "jslintrc",
            "jsonld",
            "geojson",
            "ipynb",
            "vuerc",
        ],
        &["composer.lock", ".watchmanconfig"],
    ),
    (
        "jsonc",
        &[
            "jsonc",
            "eslintrc",
            "eslintrc.json",
            "jsfmtrc",
            "jshintrc",
            "swcrc",
            "hintrc",
            "babelrc",
        ],
        &[
            "babel.config.json",
            ".babelrc.json",
            ".ember-cli",
            "typedoc.json",
            "tsconfig.json",
            "jsconfig.json",
        ],
    ),
    ("jsonl", &["jsonl", "ndjson"], &[]),
    ("julia", &["jl"], &[]),
    ("latex", &["tex", "ltx", "ctx"], &[]),
    ("bibtex", &["bib"], &[]),
    ("less", &["less"], &[]),
    ("log", &["log"], &[]),
    ("lua", &["lua"], &[]),
    (
        "makefile",
        &["mak", "mk"],
        &["makefile", "gnumakefile", "ocamlmakefile"],
    ),
    (
        "markdown",
        &[
            "md", "mkd", "mdwn", "mdown", "markdown", "markdn", "mdtxt", "mdtext", "workbook",
        ],
        &[],
    ),
    ("objective-c", &["m"], &[]),
    ("objective-cpp", &["mm"], &[]),
    ("perl", &["pl", "pm", "pod", "t", "psgi"], &[]),
    ("php", &["php", "php4", "php5", "phtml", "ctp"], &[]),
    ("plaintext", &["txt"], &[]),
    ("powershell", &["ps1", "psm1", "psd1", "pssc", "psrc"], &[]),
    (
        "properties",
        &[
            "properties",
            "cfg",
            "conf",
            "directory",
            "gitattributes",
            "gitconfig",
            "gitmodules",
            "editorconfig",
            "repo",
        ],
        &[
            "gitattributes",
            ".gitattributes",
            ".gitconfig",
            ".gitmodules",
            ".editorconfig",
        ],
    ),
    ("pug", &["pug", "jade"], &[]),
    (
        "python",
        &[
            "py", "rpy", "pyw", "cpy", "gyp", "gypi", "pyi", "ipy", "pyt",
        ],
        &["sconstruct", "sconscript"],
    ),
    ("r", &["r", "rhistory", "rprofile", "rt"], &[]),
    ("razor", &["cshtml", "razor"], &[]),
    ("restructuredtext", &["rst"], &[]),
    (
        "ruby",
        &[
            "rb", "rbx", "rjs", "gemspec", "rake", "ru", "erb", "podspec", "rbi",
        ],
        &[
            "rakefile",
            "gemfile",
            "guardfile",
            "podfile",
            "capfile",
            "cheffile",
            "hobofile",
            "vagrantfile",
            "appraisals",
            "rantfile",
            "berksfile",
            "berksfile.lock",
            "thorfile",
            "puppetfile",
            "dangerfile",
            "brewfile",
            "fastfile",
            "appfile",
            "deliverfile",
            "matchfile",
            "scanfile",
            "snapfile",
            "gymfile",
        ],
    ),
    ("rust", &["rs"], &[]),
    ("scss", &["scss"], &[]),
    ("shaderlab", &["shader"], &[]),
    (
        "shellscript",
        &[
            "sh",
            "bash",
            "bashrc",
            "bash_aliases",
            "bash_profile",
            "bash_login",
            "ebuild",
            "eclass",
            "profile",
            "bash_logout",
            "xprofile",
            "xsession",
            "xsessionrc",
            "zsh",
            "zshrc",
            "zprofile",
            "zlogin",
            "zlogout",
            "zshenv",
            "zsh-theme",
            "fish",
            "ksh",
            "csh",
            "cshrc",
            "tcshrc",
            "yashrc",
            "yash_profile",
        ],
        &[
            "apkbuild",
            "pkgbuild",
            ".envrc",
            ".hushlogin",
            "zshrc",
            "zshenv",
            "zlogin",
            "zprofile",
            "zlogout",
            "bashrc_apple_terminal",
            "zshrc_apple_terminal",
        ],
    ),
    ("sql", &["sql", "dsql"], &[]),
    ("swift", &["swift"], &[]),
    ("typescript", &["ts", "cts", "mts"], &[]),
    ("typescriptreact", &["tsx"], &[]),
    ("vb", &["vb", "brs", "vbs", "bas", "vba"], &[]),
    (
        "xml",
        &[
            "xml",
            "xsd",
            "ascx",
            "atom",
            "axml",
            "axaml",
            "bpmn",
            "cpt",
            "csl",
            "csproj",
            "dita",
            "ditamap",
            "dtd",
            "ent",
            "mod",
            "dtml",
            "fsproj",
            "fxml",
            "iml",
            "isml",
            "jmx",
            "launch",
            "menu",
            "mxml",
            "nuspec",
            "opml",
            "owl",
            "proj",
            "props",
            "pt",
            "publishsettings",
            "pubxml",
            "rdf",
            "rng",
            "rss",
            "shproj",
            "storyboard",
            "svg",
            "targets",
            "tld",
            "tmx",
            "vbproj",
            "vcxproj",
            "wsdl",
            "wxi",
            "wxl",
            "wxs",
            "xaml",
            "xbl",
            "xib",
            "xlf",
            "xliff",
            "xpdl",
            "xul",
            "xoml",
        ],
        &[],
    ),
    ("xsl", &["xsl", "xslt"], &[]),
    (
        "yaml",
        &[
            "yaml",
            "yml",
            "eyaml",
            "eyml",
            "cff",
            "yaml-tmlanguage",
            "yaml-tmpreferences",
            "yaml-tmtheme",
            "winget",
        ],
        &[],
    ),
    ("toml", &["toml"], &[]),
    ("vue", &["vue"], &[]),
    ("svelte", &["svelte"], &[]),
    ("kotlin", &["kt", "kts"], &[]),
    ("scala", &["scala", "sc", "sbt"], &[]),
    ("haskell", &["hs", "lhs"], &[]),
    ("elixir", &["ex", "exs"], &[]),
    ("erlang", &["erl", "hrl"], &[]),
    ("ocaml", &["ml", "mli"], &[]),
    ("zig", &["zig"], &[]),
    ("nim", &["nim", "nims"], &[]),
    ("graphql", &["graphql", "gql"], &[]),
    ("proto3", &["proto"], &[]),
    ("terraform", &["tf", "tfvars"], &[]),
    ("nix", &["nix"], &[]),
    ("astro", &["astro"], &[]),
    ("prisma", &["prisma"], &[]),
    ("solidity", &["sol"], &[]),
    ("csv", &["csv"], &[]),
    ("tsv", &["tsv", "tab"], &[]),
];

/// The VS Code language id for a file named `name` (no folders), from
/// VS Code's own language list.
pub fn builtin_language_id(name: &str) -> Option<&'static str> {
    let lower = name.to_lowercase();
    if let Some((id, _, _)) = LANGUAGES
        .iter()
        .find(|(_, _, names)| names.contains(&lower.as_str()))
    {
        return Some(id);
    }
    suffixes(&lower).find_map(|suffix| {
        LANGUAGES
            .iter()
            .find(|(_, exts, _)| exts.contains(&suffix))
            .map(|(id, _, _)| *id)
    })
}

fn has_extension(path: &Path, ext: &str) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(ext))
}

/// VS Code `fontCharacter`: `\E001` (CSS escape) or the character itself.
fn font_character(text: &str) -> Option<char> {
    match text.strip_prefix('\\') {
        Some(hex) => u32::from_str_radix(hex, 16).ok().and_then(char::from_u32),
        None => {
            let mut chars = text.chars();
            let c = chars.next()?;
            chars.next().is_none().then_some(c)
        }
    }
}

/// TrueType / OpenType data as is, WOFF 1.0 decompressed; `None` for WOFF2
/// and anything else.
pub fn decode_font(bytes: &[u8]) -> Option<Vec<u8>> {
    match bytes.get(..4)? {
        [0, 1, 0, 0] | b"OTTO" | b"true" => Some(bytes.to_vec()),
        b"wOFF" => decode_woff(bytes),
        _ => None,
    }
}

fn be16(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// WOFF 1.0 (W3C): a 44-byte header, a 20-byte directory entry per table,
/// each table zlib-compressed unless its compressed length equals its
/// original one. Rebuilt as an sfnt with a 16-byte directory entry per
/// table, tables 4-byte aligned.
fn decode_woff(b: &[u8]) -> Option<Vec<u8>> {
    let flavor = be32(b, 4)?;
    let count = be16(b, 12)? as usize;
    if count == 0 || count > 1024 {
        return None;
    }
    let mut tables = Vec::with_capacity(count);
    for i in 0..count {
        let at = 44 + i * 20;
        let tag = be32(b, at)?;
        let offset = be32(b, at + 4)? as usize;
        let comp = be32(b, at + 8)? as usize;
        let orig = be32(b, at + 12)? as usize;
        let checksum = be32(b, at + 16)?;
        let raw = b.get(offset..offset.checked_add(comp)?)?;
        let data = if comp == orig {
            raw.to_vec()
        } else {
            use std::io::Read;
            let mut out = Vec::with_capacity(orig);
            flate2::read::ZlibDecoder::new(raw)
                .take(orig as u64 + 1)
                .read_to_end(&mut out)
                .ok()?;
            if out.len() != orig {
                return None;
            }
            out
        };
        tables.push((tag, checksum, data));
    }
    let mut search = 1usize;
    let mut selector = 0u16;
    while search * 2 <= count {
        search *= 2;
        selector += 1;
    }
    let search_range = (search * 16) as u16;
    let mut out = Vec::new();
    out.extend_from_slice(&flavor.to_be_bytes());
    out.extend_from_slice(&(count as u16).to_be_bytes());
    out.extend_from_slice(&search_range.to_be_bytes());
    out.extend_from_slice(&selector.to_be_bytes());
    out.extend_from_slice(&((count as u16 * 16).saturating_sub(search_range)).to_be_bytes());
    let mut offset = 12 + count * 16;
    let mut body = Vec::new();
    for (tag, checksum, data) in &tables {
        out.extend_from_slice(&tag.to_be_bytes());
        out.extend_from_slice(&checksum.to_be_bytes());
        out.extend_from_slice(&(offset as u32).to_be_bytes());
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        body.extend_from_slice(data);
        let pad = (4 - data.len() % 4) % 4;
        body.extend(std::iter::repeat_n(0u8, pad));
        offset += data.len() + pad;
    }
    out.extend_from_slice(&body);
    Some(out)
}

/// The tables of an sfnt font, by tag.
fn sfnt_tables(font: &[u8]) -> Option<HashMap<[u8; 4], &[u8]>> {
    let count = be16(font, 4)? as usize;
    let mut tables = HashMap::new();
    for i in 0..count {
        let at = 12 + i * 16;
        let tag: [u8; 4] = font.get(at..at + 4)?.try_into().ok()?;
        let offset = be32(font, at + 8)? as usize;
        let len = be32(font, at + 12)? as usize;
        tables.insert(tag, font.get(offset..offset.checked_add(len)?)?);
    }
    Some(tables)
}

/// The glyph id of `c` from a `cmap` (format 4 or 12 subtables).
fn cmap_glyph(cmap: &[u8], c: char) -> Option<u16> {
    let c = c as u32;
    let count = be16(cmap, 2)? as usize;
    let mut best: Option<(u16, usize)> = None;
    for i in 0..count {
        let at = 4 + i * 8;
        let (platform, encoding) = (be16(cmap, at)?, be16(cmap, at + 2)?);
        let offset = be32(cmap, at + 4)? as usize;
        let format = be16(cmap, offset)?;
        let rank = match (platform, encoding, format) {
            (3, 10, 12) | (0, 4, 12) | (0, 6, 12) => 3,
            (3, 1, 4) | (0, _, 4) => 2,
            _ => continue,
        };
        if best.is_none_or(|(r, _)| rank > r) {
            best = Some((rank, offset));
        }
    }
    let (_, offset) = best?;
    let table = cmap.get(offset..)?;
    match be16(table, 0)? {
        4 => {
            let segs = be16(table, 6)? as usize / 2;
            let ends = 14;
            let starts = ends + segs * 2 + 2;
            let deltas = starts + segs * 2;
            let ranges = deltas + segs * 2;
            for s in 0..segs {
                let end = be16(table, ends + s * 2)? as u32;
                if c > end {
                    continue;
                }
                let start = be16(table, starts + s * 2)? as u32;
                if c < start {
                    return None;
                }
                let delta = be16(table, deltas + s * 2)?;
                let range = be16(table, ranges + s * 2)? as usize;
                if range == 0 {
                    return Some((c as u16).wrapping_add(delta));
                }
                let at = ranges + s * 2 + range + (c - start) as usize * 2;
                let glyph = be16(table, at)?;
                return (glyph != 0).then(|| glyph.wrapping_add(delta));
            }
            None
        }
        12 => {
            let groups = be32(table, 12)? as usize;
            for g in 0..groups {
                let at = 16 + g * 12;
                let (start, end) = (be32(table, at)?, be32(table, at + 4)?);
                if (start..=end).contains(&c) {
                    return u16::try_from(be32(table, at + 8)? + (c - start)).ok();
                }
            }
            None
        }
        _ => None,
    }
}

/// The outline of glyph `id` as SVG path commands (y up, font units),
/// appended to `out`; composite glyphs place their parts by offset.
/// `[a, b, c, d, e, f]`: x' = a·x + c·y + e, y' = b·x + d·y + f.
type Transform = [f32; 6];

const IDENTITY: Transform = [1., 0., 0., 1., 0., 0.];

fn glyph_path(
    tables: &HashMap<[u8; 4], &[u8]>,
    id: u16,
    m: Transform,
    depth: u8,
    out: &mut String,
) -> Option<()> {
    use std::fmt::Write;
    if depth > 8 {
        return None;
    }
    let head = tables.get(b"head")?;
    let loca = tables.get(b"loca")?;
    let glyf = tables.get(b"glyf")?;
    let long = be16(head, 50)? == 1;
    let id = id as usize;
    let (start, end) = if long {
        (
            be32(loca, id * 4)? as usize,
            be32(loca, id * 4 + 4)? as usize,
        )
    } else {
        (
            be16(loca, id * 2)? as usize * 2,
            be16(loca, id * 2 + 2)? as usize * 2,
        )
    };
    if end <= start {
        return Some(()); // an empty glyph
    }
    let g = glyf.get(start..end)?;
    let contours = be16(g, 0)? as i16;
    if contours < 0 {
        // composite: flags, glyph index, offsets
        let mut at = 10;
        loop {
            let flags = be16(g, at)?;
            let part = be16(g, at + 2)?;
            at += 4;
            let (x, y) = if flags & 1 != 0 {
                let v = (be16(g, at)? as i16 as f32, be16(g, at + 2)? as i16 as f32);
                at += 4;
                v
            } else {
                let v = (*g.get(at)? as i8 as f32, *g.get(at + 1)? as i8 as f32);
                at += 2;
                v
            };
            // F2Dot14 scales: one, x and y, or a 2×2 matrix
            let f2 = |at: usize| be16(g, at).map(|v| v as i16 as f32 / 16384.);
            let (a, b, c, d) = if flags & 0x8 != 0 {
                let s = f2(at)?;
                at += 2;
                (s, 0., 0., s)
            } else if flags & 0x40 != 0 {
                let v = (f2(at)?, 0., 0., f2(at + 2)?);
                at += 4;
                v
            } else if flags & 0x80 != 0 {
                let v = (f2(at)?, f2(at + 2)?, f2(at + 4)?, f2(at + 6)?);
                at += 8;
                v
            } else {
                (1., 0., 0., 1.)
            };
            // point-matched placement (flag 2 off) is left at the origin
            let (x, y) = if flags & 2 != 0 { (x, y) } else { (0., 0.) };
            let part_m: Transform = [a, b, c, d, x, y];
            // the part's transform, then this glyph's
            let combined = [
                m[0] * part_m[0] + m[2] * part_m[1],
                m[1] * part_m[0] + m[3] * part_m[1],
                m[0] * part_m[2] + m[2] * part_m[3],
                m[1] * part_m[2] + m[3] * part_m[3],
                m[0] * part_m[4] + m[2] * part_m[5] + m[4],
                m[1] * part_m[4] + m[3] * part_m[5] + m[5],
            ];
            glyph_path(tables, part, combined, depth + 1, out)?;
            if flags & 0x20 == 0 {
                return Some(());
            }
        }
    }
    let contours = contours as usize;
    let mut ends = Vec::with_capacity(contours);
    for i in 0..contours {
        ends.push(be16(g, 10 + i * 2)? as usize);
    }
    let points = ends.last().map_or(0, |e| e + 1);
    let mut at = 10 + contours * 2;
    let instructions = be16(g, at)? as usize;
    at += 2 + instructions;
    let mut flags = Vec::with_capacity(points);
    while flags.len() < points {
        let f = *g.get(at)?;
        at += 1;
        flags.push(f);
        if f & 8 != 0 {
            let repeat = *g.get(at)?;
            at += 1;
            for _ in 0..repeat {
                flags.push(f);
            }
        }
    }
    flags.truncate(points);
    let read = |short: u8, same: u8, at: &mut usize| -> Option<Vec<f32>> {
        let mut v = 0i32;
        let mut out = Vec::with_capacity(points);
        for f in &flags {
            if f & short != 0 {
                let d = *g.get(*at)? as i32;
                *at += 1;
                v += if f & same != 0 { d } else { -d };
            } else if f & same == 0 {
                v += be16(g, *at)? as i16 as i32;
                *at += 2;
            }
            out.push(v as f32);
        }
        Some(out)
    };
    let xs = read(2, 16, &mut at)?;
    let ys = read(4, 32, &mut at)?;
    let point = |i: usize| {
        let (x, y) = (xs[i], ys[i]);
        (
            m[0] * x + m[2] * y + m[4],
            m[1] * x + m[3] * y + m[5],
            flags[i] & 1 != 0,
        )
    };
    let mut from = 0;
    for &end in &ends {
        if end < from || end >= points {
            return None;
        }
        let ring: Vec<(f32, f32, bool)> = (from..=end).map(point).collect();
        from = end + 1;
        let n = ring.len();
        // start on an on-curve point (or the midpoint of two off-curve ones)
        let first = ring.iter().position(|p| p.2);
        let (sx, sy) = match first {
            Some(i) => (ring[i].0, ring[i].1),
            None => (
                (ring[0].0 + ring[n - 1].0) / 2.,
                (ring[0].1 + ring[n - 1].1) / 2.,
            ),
        };
        let offset = first.map_or(0, |i| i + 1);
        let _ = write!(out, "M{sx} {}", -sy);
        let mut control: Option<(f32, f32)> = None;
        for k in 0..n {
            let (x, y, on) = ring[(offset + k) % n];
            if on {
                match control.take() {
                    Some((cx, cy)) => {
                        let _ = write!(out, "Q{cx} {} {x} {}", -cy, -y);
                    }
                    None => {
                        let _ = write!(out, "L{x} {}", -y);
                    }
                }
            } else if let Some((cx, cy)) = control.replace((x, y)) {
                let (mx, my) = ((cx + x) / 2., (cy + y) / 2.);
                let _ = write!(out, "Q{cx} {} {mx} {}", -cy, -my);
            }
        }
        if let Some((cx, cy)) = control {
            let _ = write!(out, "Q{cx} {} {sx} {}", -cy, -sy);
        }
        out.push('Z');
    }
    Some(())
}

/// An SVG of `c`'s glyph in a TrueType font, in an em-square view box
/// (the advance centred, ascender to descender). `None` for CFF fonts and
/// characters the font lacks.
pub fn glyph_svg(font: &[u8], c: char) -> Option<Vec<u8>> {
    let tables = sfnt_tables(font)?;
    let id = cmap_glyph(tables.get(b"cmap")?, c)?;
    let head = tables.get(b"head")?;
    let upem = be16(head, 18)? as f32;
    let (ascent, descent) = match tables.get(b"hhea") {
        Some(hhea) => (be16(hhea, 4)? as i16 as f32, be16(hhea, 6)? as i16 as f32),
        None => (upem * 0.8, -upem * 0.2),
    };
    let advance = tables
        .get(b"hhea")
        .and_then(|hhea| {
            let metrics = be16(hhea, 34)? as usize;
            let hmtx = tables.get(b"hmtx")?;
            let at = (id as usize).min(metrics.saturating_sub(1)) * 4;
            be16(hmtx, at).map(f32::from)
        })
        .unwrap_or(upem);
    let mut path = String::new();
    glyph_path(&tables, id, IDENTITY, 0, &mut path)?;
    if path.is_empty() {
        return None;
    }
    let height = ascent - descent;
    let side = upem.max(height).max(advance);
    let x = advance / 2. - side / 2.;
    let y = -ascent + height / 2. - side / 2.;
    Some(
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{x} {y} {side} {side}\"><path d=\"{path}\"/></svg>"
        )
        .into_bytes(),
    )
}

/// The family name (name id 16, else 1) of an sfnt font.
pub fn font_family(font: &[u8]) -> Option<String> {
    let count = be16(font, 4)? as usize;
    let name = (0..count).find_map(|i| {
        let at = 12 + i * 16;
        (font.get(at..at + 4)? == b"name").then(|| be32(font, at + 8))?
    })? as usize;
    let records = be16(font, name + 2)? as usize;
    let strings = name + be16(font, name + 4)? as usize;
    let mut best: Option<(u16, String)> = None;
    for i in 0..records {
        let at = name + 6 + i * 12;
        let platform = be16(font, at)?;
        let id = be16(font, at + 6)?;
        if id != 1 && id != 16 {
            continue;
        }
        let len = be16(font, at + 8)? as usize;
        let off = strings + be16(font, at + 10)? as usize;
        let raw = font.get(off..off + len)?;
        let text = if platform == 0 || platform == 3 {
            let units: Vec<u16> = raw
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_be_bytes(*c))
                .collect();
            String::from_utf16(&units).ok()?
        } else {
            raw.iter().map(|&c| c as char).collect()
        };
        // the typographic family (16) wins over the legacy one (1)
        if best.as_ref().is_none_or(|(have, _)| id > *have) {
            best = Some((id, text));
        }
    }
    best.map(|(_, name)| name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, path: &str, text: &str) {
        let full = dir.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, text).unwrap();
    }

    #[test]
    fn vscode_themes_resolve_names_then_suffixes_then_languages() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "dist/theme.json",
            r#"{
                // comments are allowed
                "iconDefinitions": {
                    "file": {"iconPath": "./icons/file.svg"},
                    "folder": {"iconPath": "./icons/folder.svg"},
                    "folder-open": {"iconPath": "./icons/folder-open.svg"},
                    "folder-src": {"iconPath": "./icons/folder-src.svg"},
                    "ts": {"iconPath": "./icons/ts.svg"},
                    "dts": {"iconPath": "./icons/dts.svg"},
                    "rust": {"iconPath": "./icons/rust.png"},
                    "cargo": {"iconPath": "./icons/cargo.svg"},
                    "ts-light": {"iconPath": "./icons/ts-light.svg"}
                },
                "file": "file",
                "folder": "folder",
                "folderExpanded": "folder-open",
                "folderNames": {"src": "folder-src"},
                "fileExtensions": {"ts": "ts", "d.ts": "dts"},
                "fileNames": {"cargo.toml": "cargo"},
                "languageIds": {"rust": "rust"},
                "light": {"fileExtensions": {"ts": "ts-light"}},
            }"#,
        );
        let theme = IconTheme::load(
            dir.path(),
            &IconThemeRef {
                id: "t".into(),
                label: "T".into(),
                path: "dist/theme.json".into(),
                format: IconThemeFormat::VsCode,
                appearance: None,
            },
        )
        .unwrap();
        let lang = |name: &str| name.ends_with(".rs").then(|| "rust".to_string());
        let icon = |name: &str, light: bool| match theme.file_icon(name, light, lang) {
            Some(Icon::Svg(p) | Icon::Image(p)) => {
                p.file_name().unwrap().to_string_lossy().into_owned()
            }
            other => format!("{other:?}"),
        };
        assert_eq!(icon("Cargo.toml", false), "cargo.svg");
        assert_eq!(icon("x.d.ts", false), "dts.svg");
        assert_eq!(icon("x.ts", false), "ts.svg");
        assert_eq!(icon("x.ts", true), "ts-light.svg");
        assert_eq!(icon("main.rs", false), "rust.png");
        assert_eq!(icon("notes.txt", false), "file.svg");
        let folder = |name: &str, expanded| match theme.folder_icon(name, expanded, false) {
            Some(Icon::Svg(p)) => p.file_name().unwrap().to_string_lossy().into_owned(),
            other => format!("{other:?}"),
        };
        assert_eq!(folder("src", false), "folder-src.svg");
        assert_eq!(folder("docs", true), "folder-open.svg");
        assert_eq!(folder("docs", false), "folder.svg");
    }

    #[test]
    fn zed_themes_use_stems_suffixes_and_masks() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "icons/rust.svg", "<svg fill=\"#f00\"/>");
        write(dir.path(), "icons/file.svg", "<svg fill=\"currentColor\"/>");
        write(dir.path(), "icons/folder.svg", "<svg/>");
        write(
            dir.path(),
            "icon_themes/t.json",
            r#"{"themes": [{"name": "Mine", "appearance": "dark",
                "file_icons": {"rust": {"path": "./icons/rust.svg"}, "default": {"path": "./icons/file.svg"}},
                "file_stems": {"Cargo.lock": "rust"},
                "file_suffixes": {"rs": "rust"},
                "directory_icons": {"collapsed": "./icons/folder.svg"}}]}"#,
        );
        let refs = zed_refs(dir.path(), &["icon_themes/t.json".into()]);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].appearance.as_deref(), Some("dark"));
        let theme = IconTheme::load(dir.path(), &refs[0]).unwrap();
        assert!(matches!(
            theme.file_icon("lib.rs", false, |_| None),
            Some(Icon::Svg(_))
        ));
        assert!(matches!(
            theme.file_icon("Cargo.lock", false, |_| None),
            Some(Icon::Svg(_))
        ));
        assert!(matches!(
            theme.file_icon("a.txt", false, |_| None),
            Some(Icon::SvgMask(_))
        ));
        assert!(matches!(
            theme.folder_icon("x", true, false),
            Some(Icon::Svg(_))
        ));
    }

    #[test]
    fn builtin_language_ids() {
        assert_eq!(builtin_language_id("main.rs"), Some("rust"));
        assert_eq!(builtin_language_id("Makefile"), Some("makefile"));
        assert_eq!(builtin_language_id("tsconfig.json"), Some("jsonc"));
        assert_eq!(builtin_language_id("x.unknown"), None);
    }

    #[test]
    fn suffixes_run_longest_first() {
        assert_eq!(suffixes("a.d.ts").collect::<Vec<_>>(), ["d.ts", "ts"]);
        assert_eq!(
            suffixes(".eslintrc.json").collect::<Vec<_>>(),
            ["eslintrc.json", "json"]
        );
        assert_eq!(suffixes("Makefile").count(), 0);
    }

    #[test]
    fn font_characters_and_woff_headers() {
        assert_eq!(font_character("\\E001"), Some('\u{E001}'));
        assert_eq!(font_character("x"), Some('x'));
        assert_eq!(decode_font(b"wOF2...."), None);
        assert_eq!(decode_font(&[0, 1, 0, 0, 9]), Some(vec![0, 1, 0, 0, 9]));
    }

    #[test]
    fn woff_round_trips_to_an_sfnt_with_its_family_name() {
        // a font with only a `name` table naming the family "Seti"
        let family: Vec<u8> = "Seti"
            .encode_utf16()
            .flat_map(|u| u.to_be_bytes())
            .collect();
        let mut name = vec![0, 0, 0, 1, 0, 18];
        name.extend_from_slice(&[0, 3, 0, 1, 4, 9, 0, 1]);
        name.extend_from_slice(&(family.len() as u16).to_be_bytes());
        name.extend_from_slice(&[0, 0]);
        name.extend_from_slice(&family);
        let compressed = {
            use std::io::Write;
            let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
            z.write_all(&name).unwrap();
            z.finish().unwrap()
        };
        let mut woff = b"wOFF".to_vec();
        woff.extend_from_slice(&[0, 1, 0, 0]);
        woff.extend_from_slice(&[0; 4]);
        woff.extend_from_slice(&1u16.to_be_bytes());
        woff.resize(44, 0);
        woff.extend_from_slice(b"name");
        woff.extend_from_slice(&(64u32).to_be_bytes());
        woff.extend_from_slice(&(compressed.len() as u32).to_be_bytes());
        woff.extend_from_slice(&(name.len() as u32).to_be_bytes());
        woff.extend_from_slice(&[0; 4]);
        woff.extend_from_slice(&compressed);
        let sfnt = decode_font(&woff).unwrap();
        assert_eq!(&sfnt[..4], &[0, 1, 0, 0]);
        assert_eq!(font_family(&sfnt).as_deref(), Some("Seti"));
    }
}
