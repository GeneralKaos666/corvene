//! Building a tree-sitter grammar from source (flag
//! `1001-build-grammars-from-source`): the grammar's repository at the
//! commit an extension pins is downloaded, its generated `parser.c` (and
//! `scanner.c` / `scanner.cc`) compiled with the system C compiler together
//! with a small table ([`shim`]) that exports the grammar the way Corvene's
//! packs do, and the library is kept in the cache for the highlighter to
//! load. Nothing here runs the grammar: the caller verifies the library in
//! a helper process first.
//!
//! The user agrees to each build in a consent sheet naming the repository,
//! the commit and the compiler; this module only does what that sheet
//! says.

pub mod compiler;
pub mod runner;
pub mod shim;

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::ExtensionError;
use crate::archive::Limits;
use crate::http;

/// The table version of the libraries this writes (`corvene_grammars_v1`).
pub const ABI: u32 = 1;

/// What a build will do, shown in the consent sheet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildPlan {
    pub extension: String,
    /// the grammar's name in the extension (`nix`)
    pub grammar: String,
    /// `https://github.com/owner/repo`
    pub repository: String,
    pub rev: String,
    /// a folder of the repository holding the grammar
    pub path: Option<String>,
    pub tarball_url: String,
    /// where sources and libraries go (`…/Caches/Corvene/extensions`)
    pub cache_dir: PathBuf,
}

impl BuildPlan {
    /// A plan for `grammar` of `extension`, from the extension's metadata.
    pub fn new(
        extension: &str,
        grammar: &str,
        repository: &str,
        rev: &str,
        path: Option<&str>,
        cache_dir: &Path,
    ) -> Result<Self, ExtensionError> {
        let repo = crate::manifest::repository_url(Some(&crate::value::Value::Str(
            repository.to_string(),
        )))
        .filter(|r| r.starts_with("https://github.com/"))
        .ok_or_else(|| {
            ExtensionError::NotAnExtension(format!(
                "{repository} is not a GitHub repository; Corvene only builds grammars from GitHub"
            ))
        })?;
        let rev = rev.trim();
        if rev.is_empty() || rev.contains(['/', ' ', '\\']) || rev.contains("..") {
            return Err(ExtensionError::NotAnExtension(format!(
                "{rev:?} is not a commit or tag"
            )));
        }
        let short = repo.trim_start_matches("https://github.com/");
        let tarball_url = format!("https://codeload.github.com/{short}/tar.gz/{rev}");
        Ok(Self {
            extension: extension.to_string(),
            grammar: grammar.to_string(),
            repository: repo,
            rev: rev.to_string(),
            path: path
                .map(|p| p.trim_matches('/').to_string())
                .filter(|p| !p.is_empty()),
            tarball_url,
            cache_dir: cache_dir.to_path_buf(),
        })
    }

    /// `owner-repo`
    pub fn slug(&self) -> String {
        crate::install::slug(self.repository.trim_start_matches("https://github.com/"))
    }

    /// The first 8 characters of the commit.
    pub fn short_rev(&self) -> String {
        self.rev.chars().take(8).collect()
    }

    /// `repository@rev`, the key of a remembered consent.
    pub fn consent_key(&self) -> String {
        format!("{}@{}", self.repository, self.rev)
    }

    /// Where the sources unpack.
    pub fn source_dir(&self) -> PathBuf {
        self.cache_dir
            .join("grammar-src")
            .join(format!("{}-{}", self.slug(), self.short_rev()))
    }

    /// Where the library goes.
    pub fn library_path(&self) -> PathBuf {
        self.cache_dir
            .join("grammars")
            .join(&self.extension)
            .join(format!(
                "{}-{}-abi{ABI}.{}",
                crate::install::slug(&self.grammar),
                self.short_rev(),
                library_extension()
            ))
    }

    /// The build log next to the library.
    pub fn log_path(&self) -> PathBuf {
        self.library_path().with_extension("log")
    }
}

fn library_extension() -> &'static str {
    if cfg!(target_os = "macos") {
        "dylib"
    } else if cfg!(windows) {
        "dll"
    } else {
        "so"
    }
}

/// Where a build is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stage {
    Downloading { received: u64, total: Option<u64> },
    Unpacking,
    Checking,
    Compiling(String),
    Linking,
    Verifying,
}

impl Stage {
    pub fn describe(&self) -> String {
        match self {
            Stage::Downloading { received, total } => match total {
                Some(total) if *total > 0 => format!(
                    "Downloading source ({} of {} KB)…",
                    received / 1024,
                    total / 1024
                ),
                _ => format!("Downloading source ({} KB)…", received / 1024),
            },
            Stage::Unpacking => "Unpacking source…".to_string(),
            Stage::Checking => "Checking the source…".to_string(),
            Stage::Compiling(file) => format!("Compiling {file}…"),
            Stage::Linking => "Linking the library…".to_string(),
            Stage::Verifying => "Verifying the library…".to_string(),
        }
    }
}

/// What a finished build produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Built {
    pub library: PathBuf,
    pub log: PathBuf,
    /// the C symbol (`tree_sitter_nix`)
    pub symbol: String,
}

/// A compiler run is given up after this long.
pub const COMPILE_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// Run the plan: download, unpack, compile, link, then `verify` (a helper
/// process that loads the library). `progress` sees each stage.
pub fn build(
    plan: &BuildPlan,
    compiler: &compiler::Compiler,
    progress: &mut dyn FnMut(Stage),
    verify: &dyn Fn(&Path) -> Result<(), String>,
) -> Result<Built, ExtensionError> {
    let mut log = String::new();
    let source_dir = plan.source_dir();
    let unpacked = source_dir.join("tree");
    if !unpacked.join(".complete").is_file() {
        let _ = std::fs::remove_dir_all(&source_dir);
        std::fs::create_dir_all(&source_dir)?;
        let tarball = source_dir.join("source.tar.gz");
        let mut on_progress = |received, total| progress(Stage::Downloading { received, total });
        http::download(
            &plan.tarball_url,
            &tarball,
            http::MAX_SOURCE_BYTES,
            None,
            &mut on_progress,
        )?;
        progress(Stage::Unpacking);
        let limits = Limits {
            max_entries: 50_000,
            max_entry_bytes: 128 * 1024 * 1024,
            max_total_bytes: 512 * 1024 * 1024,
            max_depth: 32,
        };
        crate::archive::extract(&tarball, &unpacked, &limits)?;
        let _ = std::fs::remove_file(&tarball);
        std::fs::write(unpacked.join(".complete"), b"")?;
    }
    progress(Stage::Checking);
    // codeload wraps the tree in `<repo>-<ref>/`
    let top = std::fs::read_dir(&unpacked)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .find(|p| p.is_dir())
        .unwrap_or(unpacked.clone());
    let grammar_root = match &plan.path {
        Some(path) => top.join(path),
        None => top.clone(),
    };
    let src = find_parser_dir(&grammar_root, &plan.grammar)
        .or_else(|| find_parser_dir(&top, &plan.grammar))
        .ok_or_else(|| {
            ExtensionError::NotAnExtension(format!(
                "{} has no generated parser (src/parser.c) for {}; Corvene does not run tree-sitter generate",
                plan.repository, plan.grammar
            ))
        })?;
    let parser = src.join("parser.c");
    let symbol = language_symbol(&parser, &plan.grammar).ok_or_else(|| {
        ExtensionError::NotAnExtension(format!(
            "{} defines no tree_sitter_<name> function",
            parser.display()
        ))
    })?;
    let scanner_c = src.join("scanner.c");
    let scanner_cc = src.join("scanner.cc");
    let library = plan.library_path();
    if let Some(dir) = library.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let work = source_dir.join("build");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work)?;
    let table = work.join("table.c");
    std::fs::write(
        &table,
        shim::table_c(
            &plan.grammar,
            &symbol,
            &format!("{}@{}", plan.slug(), plan.short_rev()),
        ),
    )?;
    let include = src.to_string_lossy().into_owned();
    let mut objects: Vec<PathBuf> = Vec::new();
    let mut needs_cxx = false;
    let mut compile =
        |file: &Path, cxx: bool, progress: &mut dyn FnMut(Stage)| -> Result<(), ExtensionError> {
            let name = file
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            progress(Stage::Compiling(name.clone()));
            let object = work.join(format!("{name}.o"));
            let (program, standard) = if cxx {
                (
                    compiler.cxx.clone().ok_or_else(|| {
                        ExtensionError::NotAnExtension(
                            "the grammar's scanner is C++ but no C++ compiler was found"
                                .to_string(),
                        )
                    })?,
                    "-std=c++14",
                )
            } else {
                (compiler.cc.clone(), "-std=c11")
            };
            let mut args: Vec<String> = vec![
                "-c".into(),
                standard.into(),
                "-O2".into(),
                "-fPIC".into(),
                "-fvisibility=hidden".into(),
                "-w".into(),
                "-I".into(),
                include.clone(),
            ];
            args.extend(compiler.target_flags());
            args.push(file.to_string_lossy().into_owned());
            args.push("-o".into());
            args.push(object.to_string_lossy().into_owned());
            runner::run(&program, &args, &work, COMPILE_TIMEOUT, &mut log)
                .map_err(ExtensionError::Compile)?;
            objects.push(object);
            Ok(())
        };
    compile(&table, false, progress)?;
    compile(&parser, false, progress)?;
    if scanner_c.is_file() {
        compile(&scanner_c, false, progress)?;
    } else if scanner_cc.is_file() {
        needs_cxx = true;
        compile(&scanner_cc, true, progress)?;
    }
    progress(Stage::Linking);
    let linker = if needs_cxx {
        compiler.cxx.clone().unwrap_or_else(|| compiler.cc.clone())
    } else {
        compiler.cc.clone()
    };
    let partial = library.with_extension("partial");
    let mut args: Vec<String> = compiler.link_flags(&library);
    args.extend(objects.iter().map(|o| o.to_string_lossy().into_owned()));
    args.push("-o".into());
    args.push(partial.to_string_lossy().into_owned());
    runner::run(&linker, &args, &work, COMPILE_TIMEOUT, &mut log)
        .map_err(ExtensionError::Compile)?;
    if cfg!(target_os = "macos") {
        // ad hoc, like the packs' units
        let args = [
            "--force".to_string(),
            "--sign".to_string(),
            "-".to_string(),
            partial.to_string_lossy().into_owned(),
        ];
        runner::run(
            Path::new("/usr/bin/codesign"),
            &args,
            &work,
            Duration::from_secs(60),
            &mut log,
        )
        .map_err(ExtensionError::Compile)?;
    }
    progress(Stage::Verifying);
    if let Err(err) = verify(&partial) {
        let _ = std::fs::remove_file(&partial);
        log.push_str(&format!("\nverification failed: {err}\n"));
        let _ = std::fs::write(plan.log_path(), &log);
        return Err(ExtensionError::Compile(format!(
            "the built library could not be loaded: {err}"
        )));
    }
    std::fs::rename(&partial, &library)?;
    let _ = std::fs::remove_dir_all(&work);
    std::fs::write(plan.log_path(), &log)?;
    Ok(Built {
        library,
        log: plan.log_path(),
        symbol,
    })
}

/// The `src` folder holding the parser: `<root>/src/parser.c`, else the
/// one under `<root>` whose parser defines `tree_sitter_<grammar>` (a
/// repository with several grammars), else any.
fn find_parser_dir(root: &Path, grammar: &str) -> Option<PathBuf> {
    let direct = root.join("src");
    if direct.join("parser.c").is_file() {
        return Some(direct);
    }
    let mut candidates = Vec::new();
    collect_parsers(root, 0, &mut candidates);
    candidates.sort();
    let wanted = format!("tree_sitter_{}(", grammar.replace('-', "_").to_lowercase());
    for parser in &candidates {
        if let Ok(text) = std::fs::read_to_string(parser)
            && text.contains(&wanted)
        {
            return parser.parent().map(Path::to_path_buf);
        }
    }
    candidates
        .first()
        .and_then(|p| p.parent().map(Path::to_path_buf))
}

fn collect_parsers(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > 4 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if !matches!(
                name.as_str(),
                "node_modules" | ".git" | "test" | "bindings" | "examples"
            ) {
                collect_parsers(&path, depth + 1, out);
            }
        } else if name == "parser.c" && dir.file_name().is_some_and(|d| d == "src") {
            out.push(path);
        }
    }
}

/// The `tree_sitter_<name>` function `parser.c` defines, preferring the one
/// named after the grammar.
pub fn language_symbol(parser: &Path, grammar: &str) -> Option<String> {
    let text = std::fs::read_to_string(parser).ok()?;
    let mut found: Vec<String> = Vec::new();
    for (i, _) in text.match_indices("tree_sitter_") {
        let rest = &text[i..];
        let end = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(rest.len());
        let name = &rest[..end];
        let after = rest[end..].trim_start();
        // a definition: `TSLanguage *tree_sitter_x(void) {` or `TS_PUBLIC const TSLanguage *tree_sitter_x() {`
        if name.len() > "tree_sitter_".len()
            && after.starts_with('(')
            && text[..i].trim_end().ends_with('*')
            && !found.iter().any(|f| f == name)
        {
            found.push(name.to_string());
        }
    }
    let wanted = format!("tree_sitter_{}", grammar.replace('-', "_").to_lowercase());
    found
        .iter()
        .find(|f| **f == wanted)
        .cloned()
        .or_else(|| found.first().cloned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_name_their_paths() {
        let plan = BuildPlan::new(
            "zed.nix",
            "nix",
            "https://github.com/nix-community/tree-sitter-nix.git",
            "0123456789abcdef",
            None,
            Path::new("/tmp/cache"),
        )
        .expect("plan");
        assert_eq!(
            plan.repository,
            "https://github.com/nix-community/tree-sitter-nix"
        );
        assert_eq!(
            plan.tarball_url,
            "https://codeload.github.com/nix-community/tree-sitter-nix/tar.gz/0123456789abcdef"
        );
        assert_eq!(
            plan.consent_key(),
            "https://github.com/nix-community/tree-sitter-nix@0123456789abcdef"
        );
        assert!(
            plan.library_path()
                .ends_with("grammars/zed.nix/nix-01234567-abi1.dylib")
                || !cfg!(target_os = "macos")
        );
        assert!(
            BuildPlan::new(
                "x",
                "y",
                "https://gitlab.com/a/b",
                "v1",
                None,
                Path::new("/tmp")
            )
            .is_err()
        );
        assert!(
            BuildPlan::new(
                "x",
                "y",
                "https://github.com/a/b",
                "../x",
                None,
                Path::new("/tmp")
            )
            .is_err()
        );
    }

    #[test]
    fn symbols_are_found_in_parser_c() {
        let dir = tempfile::tempdir().expect("tempdir");
        let parser = dir.path().join("parser.c");
        std::fs::write(
            &parser,
            "#include \"tree_sitter/parser.h\"\nstatic const TSLanguage language = {0};\n\
             #ifdef __cplusplus\nextern \"C\" {\n#endif\n\
             TS_PUBLIC const TSLanguage *tree_sitter_foo_bar(void) {\n  return &language;\n}\n\
             const TSLanguage *tree_sitter_other() { return &language; }\n",
        )
        .expect("write");
        assert_eq!(
            language_symbol(&parser, "foo-bar").as_deref(),
            Some("tree_sitter_foo_bar")
        );
        assert_eq!(
            language_symbol(&parser, "nope").as_deref(),
            Some("tree_sitter_foo_bar")
        );
        assert_eq!(
            language_symbol(&parser, "other").as_deref(),
            Some("tree_sitter_other")
        );
    }
}
