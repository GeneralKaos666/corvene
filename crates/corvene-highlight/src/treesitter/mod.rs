//! Opt-in tree-sitter highlighting (Corvene addition: GitHub Desktop only
//! runs CodeMirror modes, `crate::cm`). Settings › Appearance › Syntax
//! highlighting picks it per [`crate::Engine`]; flag
//! `105-tree-sitter-highlighting` offers the setting.
//!
//! - [`library`]: grammars from the compiled-in table or a pack's dylib.
//! - [`detect`]: path / injection name → grammar.
//! - [`captures`]: capture names → GHD's colour classes.
//! - [`highlight`]: parses a whole file, paints every highlight capture
//!   (by nvim's `#set! priority`, then outer nodes first, so inner ones and
//!   later patterns win, as in `tree-sitter-highlight`), paints injected
//!   languages over their host, and cuts the result into per-line spans like
//!   the other engines.
//!
//! The painter replaces `tree-sitter-highlight`, whose event stream nests
//! captures of different injection layers wrongly when they start at the
//! same byte (a Markdown fence's `@none` swallowed the Rust keywords in it).
//! Locals queries (`@local.*`) are not evaluated. An injection's content
//! excludes the node's *named* children unless `injection.include-children`
//! is set (Neovim's rule, which nvim-treesitter's and tree-sitter-md's queries
//! assume; `tree-sitter-highlight` excludes every child, so Markdown's inline
//! layer lost the text around each punctuation token).

pub mod captures;
pub mod detect;
pub mod ffi;
pub mod library;

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::ControlFlow;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use tree_sitter::{
    Language, Node, ParseOptions, Parser, Point, Query, QueryCursor, Range, StreamingIterator,
};

pub use library::{
    BUNDLED, Entry, Grammar, USER_FALLBACK, USER_PREFERRED, UserGrammar, UserLanguage, available,
    bundled, generation, has_user_grammars, is_loaded, knows_grammar, load_library, load_pack,
    register_table, register_user, unload_library, unregister_user, user_entry_for,
};

use crate::{Span, TokenClass};
use captures::Style;

/// Injected languages nest at most this deep (Markdown → HTML → JS → regex).
const MAX_DEPTH: usize = 4;
/// At most this many injected regions per file.
const MAX_LAYERS: usize = 8192;
/// Give up on a parse after this long (pathological input). Per layer, and
/// not counting query compilation (a first file in a language compiles its
/// injected languages' queries too).
const TIME_LIMIT: Duration = Duration::from_secs(2);
/// nvim-treesitter's default `priority`.
const DEFAULT_PRIORITY: u32 = 100;

/// A grammar's compiled queries.
struct Compiled {
    language: Language,
    highlights: Query,
    /// style per capture index of `highlights`
    styles: Vec<Style>,
    /// `#set! priority` per pattern of `highlights` (nvim-treesitter's; 100
    /// when unset)
    priorities: Vec<u32>,
    injections: Option<Injections>,
}

struct Injections {
    query: Query,
    content: Vec<u32>,
    language: Vec<u32>,
}

type Cache = HashMap<String, Option<Arc<Compiled>>>;

/// Compiled queries by grammar name, for one generation of the registry.
fn cache() -> &'static Mutex<(u64, Cache)> {
    static CACHE: OnceLock<Mutex<(u64, Cache)>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new((0, HashMap::new())))
}

/// `grammar`'s compiled queries; `None` if they do not compile (logged once).
fn compiled(grammar: &Grammar, generation: u64) -> Option<Arc<Compiled>> {
    let mut guard = cache().lock().ok()?;
    let (cached_generation, map) = &mut *guard;
    if *cached_generation != generation {
        map.clear();
        *cached_generation = generation;
    }
    if let Some(entry) = map.get(&grammar.name) {
        return entry.clone();
    }
    let entry = compile(grammar)
        .map_err(|err| tracing::warn!("tree-sitter queries for {}: {err}", grammar.name))
        .ok()
        .map(Arc::new);
    map.insert(grammar.name.clone(), entry.clone());
    entry
}

fn compile(grammar: &Grammar) -> Result<Compiled, String> {
    let highlights = Query::new(&grammar.language, &grammar.highlights)
        .map_err(|err| format!("highlights: {err}"))?;
    let styles = highlights
        .capture_names()
        .iter()
        .map(|name| captures::resolve(name))
        .collect();
    let priorities = (0..highlights.pattern_count())
        .map(|ix| {
            highlights
                .property_settings(ix)
                .iter()
                .find(|p| &*p.key == "priority")
                .and_then(|p| p.value.as_deref()?.parse().ok())
                .unwrap_or(DEFAULT_PRIORITY)
        })
        .collect();
    let injections = if grammar.injections.trim().is_empty() {
        None
    } else {
        let query = Query::new(&grammar.language, &grammar.injections)
            .map_err(|err| format!("injections: {err}"))?;
        let indices = |names: &[&str]| -> Vec<u32> {
            names
                .iter()
                .filter_map(|n| query.capture_index_for_name(n))
                .collect()
        };
        let content = indices(&["injection.content", "content"]);
        let language = indices(&["injection.language", "language"]);
        Some(Injections {
            query,
            content,
            language,
        })
    };
    Ok(Compiled {
        language: grammar.language.clone(),
        highlights,
        styles,
        priorities,
        injections,
    })
}

/// Check that a grammar's queries compile (tests, the coverage tool).
pub fn check_queries(grammar: &Grammar) -> Result<(), String> {
    compile(grammar).map(|_| ())
}

thread_local! {
    static PARSER: RefCell<Parser> = RefCell::new(Parser::new());
}

/// Whether a grammar is available for `path` (by name, extension or first
/// line).
pub fn has_grammar(path: &str, first_line: &str) -> bool {
    detect::for_path(&library::entries(), path, first_line).is_some()
}

/// Tokenize `lines` (without newlines) as one document. `None` when no
/// grammar matches or parsing fails, so the caller can fall back. Lines past
/// `budget` bytes get no spans.
pub fn highlight(path: &str, lines: &[&str], budget: usize) -> Option<Vec<Vec<Span>>> {
    let generation = generation();
    let grammars = library::entries();
    if grammars.is_empty() {
        return None;
    }
    let grammar =
        detect::for_path(&grammars, path, lines.first().copied().unwrap_or(""))?.grammar()?;
    let root = compiled(&grammar, generation)?;
    run(root, &grammars, generation, lines, budget)
        .map_err(|err| tracing::debug!("tree-sitter highlighting {path}: {err}"))
        .ok()
}

/// The user extension claiming `path` and whether it is preferred.
pub fn user_claim(path: &str, first_line: &str) -> Option<(String, bool)> {
    library::user_entry_for(path, first_line).map(|(_, extension, preferred)| (extension, preferred))
}

/// Tokenize `lines` with the user grammar claiming `path`, if there is one
/// (and it is preferred, when `preferred_only`). Injections resolve
/// against every registered grammar.
pub fn highlight_user(path: &str, lines: &[&str], budget: usize, preferred_only: bool) -> Option<Vec<Vec<Span>>> {
    let generation = generation();
    let (entry, _, preferred) = library::user_entry_for(path, lines.first().copied().unwrap_or(""))?;
    if preferred_only && !preferred {
        return None;
    }
    let grammar = entry.grammar()?;
    let root = compiled(&grammar, generation)?;
    run(root, &library::entries(), generation, lines, budget)
        .map_err(|err| tracing::debug!("user tree-sitter grammar highlighting {path}: {err}"))
        .ok()
}

/// Corvene's grammars by the GitHub repository they are generated from
/// (`owner/repo`, lowercase) → grammar name; `tools/ts-queries/gen.py
/// --repos` writes the table. A language extension that names the same
/// repository reuses the compiled grammar instead of building it.
pub fn bundled_grammar_repos() -> &'static HashMap<String, Vec<String>> {
    static TABLE: OnceLock<HashMap<String, Vec<String>>> = OnceLock::new();
    TABLE.get_or_init(|| {
        serde_json::from_str(include_str!("../../assets/grammar-repos.json"))
            .map_err(|err| tracing::warn!("grammar-repos.json: {err}"))
            .unwrap_or_default()
    })
}

/// `https://github.com/Owner/Repo.git` → `owner/repo`.
pub fn normalize_repository(url: &str) -> Option<String> {
    let url = url.trim().trim_end_matches('/');
    let url = url.strip_suffix(".git").unwrap_or(url);
    let rest = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))
        .or_else(|| url.strip_prefix("git@github.com:"))
        .or_else(|| url.strip_prefix("github:"))?;
    let mut parts = rest.split('/');
    let (owner, repo) = (parts.next()?, parts.next()?);
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some(format!("{}/{}", owner.to_lowercase(), repo.to_lowercase()))
}

/// The name of Corvene's grammar for an extension's tree-sitter grammar:
/// the same repository (and, for a repository holding several grammars, a
/// matching `path`, e.g. `typescript/tsx`), else the same name.
pub fn resolve_bundled(name: &str, repository: Option<&str>, path: Option<&str>) -> Option<String> {
    let table = bundled_grammar_repos();
    if let Some(repo) = repository.and_then(normalize_repository)
        && let Some(names) = table.get(&repo)
    {
        if names.len() == 1 {
            return names.first().cloned();
        }
        let wanted = path
            .map(|p| p.trim_matches('/').rsplit('/').next().unwrap_or(p).to_lowercase())
            .unwrap_or_else(|| name.to_lowercase());
        if let Some(found) = names.iter().find(|n| **n == wanted || **n == name.to_lowercase()) {
            return Some(found.clone());
        }
        return names.first().cloned();
    }
    let lower = name.to_lowercase();
    table
        .values()
        .flatten()
        .any(|n| *n == lower)
        .then_some(lower)
}

/// Highlight with `grammar`'s own queries, compiled afresh (the query tools
/// compare query sources with this; injections use the registered grammars).
pub fn highlight_with_grammar(grammar: &Grammar, lines: &[&str]) -> Result<Vec<Vec<Span>>, String> {
    let root = Arc::new(compile(grammar)?);
    run(
        root,
        &library::entries(),
        generation(),
        lines,
        crate::MAX_HIGHLIGHT_BYTES,
    )
}

fn run(
    root: Arc<Compiled>,
    grammars: &[Arc<Entry>],
    generation: u64,
    lines: &[&str],
    budget: usize,
) -> Result<Vec<Vec<Span>>, String> {
    let mut starts = Vec::with_capacity(lines.len());
    let mut source = String::new();
    for line in lines {
        if source.len() + line.len() > budget {
            break;
        }
        starts.push(source.len());
        source.push_str(line);
        source.push('\n');
    }
    let paint = PARSER.with_borrow_mut(|parser| {
        paint_document(parser, grammars, generation, root, source.as_bytes())
    })?;
    let mut out: Vec<Vec<Span>> = vec![Vec::new(); lines.len()];
    for (ix, start) in starts.iter().enumerate() {
        let line = &paint[*start..start + lines[ix].len()];
        let mut i = 0;
        while i < line.len() {
            let code = line[i];
            let run = line[i..].iter().take_while(|c| **c == code).count();
            if let Some(class) = class_of(code) {
                out[ix].push(Span {
                    range: i..i + run,
                    class,
                });
            }
            i += run;
        }
    }
    Ok(out)
}

/// A region to parse with a grammar.
struct Layer {
    compiled: Arc<Compiled>,
    /// empty: the whole document
    ranges: Vec<Range>,
    depth: usize,
}

/// One code per byte: 0 for the line colour, else a class ([`code_of`]).
fn paint_document(
    parser: &mut Parser,
    grammars: &[Arc<Entry>],
    generation: u64,
    root: Arc<Compiled>,
    source: &[u8],
) -> Result<Vec<u8>, String> {
    let mut paint = vec![0u8; source.len()];
    let mut layers = vec![Layer {
        compiled: root,
        ranges: Vec::new(),
        depth: 0,
    }];
    // breadth first: a host is painted before what it injects
    let mut next = 0;
    while next < layers.len() {
        let layer = &layers[next];
        next += 1;
        let compiled = layer.compiled.clone();
        let depth = layer.depth;
        let started = Instant::now();
        parser
            .set_language(&compiled.language)
            .map_err(|err| err.to_string())?;
        parser
            .set_included_ranges(&layer.ranges)
            .map_err(|err| err.to_string())?;
        let mut on_progress = |_: &tree_sitter::ParseState| {
            if started.elapsed() > TIME_LIMIT {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        };
        let tree = parser.parse_with_options(
            &mut |i, _| &source[i.min(source.len())..],
            None,
            Some(ParseOptions::new().progress_callback(&mut on_progress)),
        );
        // a halted parse would otherwise resume on the next document
        parser.reset();
        let tree = tree.ok_or_else(|| "parsing timed out".to_string())?;
        let node = tree.root_node();
        paint_layer(&compiled, node, source, &mut paint);
        if depth < MAX_DEPTH
            && let Some(injections) = &compiled.injections
        {
            let host = layer_ranges(&layers[next - 1]);
            for (language, ranges) in injected(injections, node, source) {
                let ranges = intersect(&ranges, &host);
                if ranges.is_empty() {
                    continue;
                }
                if layers.len() >= MAX_LAYERS {
                    break;
                }
                let Some(grammar) =
                    detect::for_injection(grammars, &language).and_then(|e| e.grammar())
                else {
                    continue;
                };
                let Some(compiled) = self::compiled(&grammar, generation) else {
                    continue;
                };
                layers.push(Layer {
                    compiled,
                    ranges,
                    depth: depth + 1,
                });
            }
        }
    }
    Ok(paint)
}

/// Paint every highlight capture: lower `priority` first, then outer nodes
/// before inner ones; on one node the later pattern paints last (and wins).
fn paint_layer(compiled: &Compiled, node: Node, source: &[u8], paint: &mut [u8]) {
    let mut cursor = QueryCursor::new();
    let mut captures = cursor.captures(&compiled.highlights, node, source);
    let mut spans: Vec<(u32, usize, usize, u8)> = Vec::new();
    while let Some((m, ix)) = captures.next() {
        let capture = m.captures()[*ix];
        let code = match compiled.styles.get(capture.index as usize) {
            Some(Style::Class(class)) => code_of(*class),
            Some(Style::Plain) => 0,
            Some(Style::Inherit) | None => continue,
        };
        let priority = compiled
            .priorities
            .get(m.pattern_index)
            .copied()
            .unwrap_or(DEFAULT_PRIORITY);
        let range = capture.node.byte_range();
        if range.start < range.end && range.end <= paint.len() {
            spans.push((priority, range.start, range.end, code));
        }
    }
    // stable: captures of one node keep their pattern order
    spans.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(b.2.cmp(&a.2)));
    for (_, start, end, code) in spans {
        paint[start..end].fill(code);
    }
}

/// The injections a layer asks for: language name and ranges, one entry per
/// match (or per pattern with `injection.combined`).
fn injected(injections: &Injections, node: Node, source: &[u8]) -> Vec<(String, Vec<Range>)> {
    let query = &injections.query;
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(query, node, source);
    let mut out: Vec<(String, Vec<Range>)> = Vec::new();
    let mut combined: HashMap<(usize, String), usize> = HashMap::new();
    while let Some(m) = matches.next() {
        let mut language: Option<String> = None;
        let mut combine = false;
        let mut include_children = false;
        for property in query.property_settings(m.pattern_index) {
            match &*property.key {
                "injection.language" => language = property.value.as_deref().map(str::to_string),
                "injection.combined" => combine = true,
                "injection.include-children" => include_children = true,
                _ => {}
            }
        }
        let mut contents = Vec::new();
        for capture in m.captures() {
            if injections.language.contains(&capture.index) {
                if language.is_none() {
                    language = capture.node.utf8_text(source).ok().map(str::to_string);
                }
            } else if injections.content.contains(&capture.index) {
                contents.push(capture.node);
            }
        }
        let Some(language) = language.filter(|l| !l.trim().is_empty()) else {
            continue;
        };
        let ranges: Vec<Range> = contents
            .iter()
            .flat_map(|node| node_ranges(*node, include_children))
            .collect();
        if ranges.is_empty() {
            continue;
        }
        if combine {
            let key = (m.pattern_index, language.clone());
            match combined.get(&key) {
                Some(ix) => out[*ix].1.extend(ranges),
                None => {
                    combined.insert(key, out.len());
                    out.push((language, ranges));
                }
            }
        } else {
            out.push((language, ranges));
        }
    }
    for (_, ranges) in &mut out {
        ranges.sort_by_key(|r| r.start_byte);
        ranges.dedup_by(|b, a| b.start_byte < a.end_byte);
    }
    out
}

/// The ranges a layer parsed (`None`: the whole document).
fn layer_ranges(layer: &Layer) -> Option<Vec<Range>> {
    (!layer.ranges.is_empty()).then(|| layer.ranges.clone())
}

/// `ranges` clipped to `host` (sorted, non-overlapping), so an injection
/// never reaches text its host layer did not parse.
fn intersect(ranges: &[Range], host: &Option<Vec<Range>>) -> Vec<Range> {
    let Some(host) = host else {
        return ranges.to_vec();
    };
    let mut out = Vec::new();
    for range in ranges {
        for h in host {
            let start = range.start_byte.max(h.start_byte);
            let end = range.end_byte.min(h.end_byte);
            if start < end {
                let (start_point, end_point) = (
                    if start == range.start_byte {
                        range.start_point
                    } else {
                        h.start_point
                    },
                    if end == range.end_byte {
                        range.end_point
                    } else {
                        h.end_point
                    },
                );
                out.push(Range {
                    start_byte: start,
                    end_byte: end,
                    start_point,
                    end_point,
                });
            }
        }
    }
    out
}

/// A content node's ranges: the node itself, or (by default) the node minus
/// its named children.
fn node_ranges(node: Node, include_children: bool) -> Vec<Range> {
    if include_children || node.named_child_count() == 0 {
        return vec![node.range()];
    }
    let mut out = Vec::new();
    let mut start_byte = node.start_byte();
    let mut start_point = node.start_position();
    let mut walker = node.walk();
    for child in node.named_children(&mut walker) {
        push_range(
            &mut out,
            start_byte,
            start_point,
            child.start_byte(),
            child.start_position(),
        );
        start_byte = child.end_byte();
        start_point = child.end_position();
    }
    push_range(
        &mut out,
        start_byte,
        start_point,
        node.end_byte(),
        node.end_position(),
    );
    out
}

fn push_range(
    out: &mut Vec<Range>,
    start_byte: usize,
    start_point: Point,
    end_byte: usize,
    end_point: Point,
) {
    if start_byte < end_byte {
        out.push(Range {
            start_byte,
            end_byte,
            start_point,
            end_point,
        });
    }
}

const CLASSES: [TokenClass; 13] = [
    TokenClass::Variable,
    TokenClass::AltVariable,
    TokenClass::Keyword,
    TokenClass::Atom,
    TokenClass::String,
    TokenClass::Qualifier,
    TokenClass::Type,
    TokenClass::Comment,
    TokenClass::Tag,
    TokenClass::Attribute,
    TokenClass::Link,
    TokenClass::Header,
    TokenClass::Quote,
];

fn code_of(class: TokenClass) -> u8 {
    CLASSES
        .iter()
        .position(|c| *c == class)
        .map_or(0, |ix| ix as u8 + 1)
}

fn class_of(code: u8) -> Option<TokenClass> {
    code.checked_sub(1)
        .and_then(|ix| CLASSES.get(ix as usize))
        .copied()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Register the compiled-in grammar table once for this test binary.
    pub(crate) fn with_grammars() {
        static ONCE: OnceLock<()> = OnceLock::new();
        ONCE.get_or_init(|| {
            let table = corvene_grammars::corvene_grammars_v1().cast::<ffi::Table>();
            unsafe { register_table("test", table) }.expect("register");
        });
    }

    #[test]
    fn a_user_grammar_derives_from_a_bundled_one() {
        with_grammars();
        assert!(knows_grammar("rust"));
        assert!(!knows_grammar("rusty"));
        let before = generation();
        register_user(
            "local.rusty",
            true,
            vec![UserGrammar {
                name: "rusty".into(),
                extensions: vec!["rusty".into()],
                filenames: Vec::new(),
                first_line: None,
                aliases: Vec::new(),
                highlights: "(identifier) @comment\n\"fn\" @string".into(),
                injections: String::new(),
                locals: String::new(),
                language: UserLanguage::Derived {
                    base: "rust".into(),
                },
            }],
        );
        assert_ne!(generation(), before);
        assert!(has_user_grammars());
        assert_eq!(user_claim("x/a.rusty", ""), Some(("local.rusty".to_string(), true)));
        let spans = highlight_user("a.rusty", &["fn main() {}"], crate::MAX_HIGHLIGHT_BYTES, true)
            .expect("highlights");
        let classes: Vec<(usize, TokenClass)> = spans[0].iter().map(|s| (s.range.start, s.class)).collect();
        assert!(classes.contains(&(0, TokenClass::String)), "{classes:?}");
        assert!(classes.contains(&(3, TokenClass::Comment)), "{classes:?}");
        // a fallback registration loses to a built-in grammar for its files
        register_user(
            "local.rusty",
            false,
            vec![UserGrammar {
                name: "rusty".into(),
                extensions: vec!["rs".into()],
                filenames: Vec::new(),
                first_line: None,
                aliases: Vec::new(),
                highlights: "(identifier) @comment".into(),
                injections: String::new(),
                locals: String::new(),
                language: UserLanguage::Derived {
                    base: "rust".into(),
                },
            }],
        );
        assert!(highlight_user("a.rs", &["fn main() {}"], crate::MAX_HIGHLIGHT_BYTES, true).is_none());
        assert!(highlight_user("a.rs", &["fn main() {}"], crate::MAX_HIGHLIGHT_BYTES, false).is_some());
        unregister_user("local.rusty");
        assert!(user_claim("a.rs", "").is_none());
    }

    #[test]
    fn repositories_resolve_to_bundled_grammars() {
        assert_eq!(
            normalize_repository("https://github.com/Tree-Sitter/tree-sitter-rust.git"),
            Some("tree-sitter/tree-sitter-rust".into())
        );
        assert_eq!(normalize_repository("git@github.com:a/b"), Some("a/b".into()));
        assert_eq!(normalize_repository("https://gitlab.com/a/b"), None);
        assert_eq!(
            resolve_bundled("anything", Some("https://github.com/tree-sitter/tree-sitter-rust"), None),
            Some("rust".into())
        );
        assert_eq!(
            resolve_bundled("tsx", Some("https://github.com/tree-sitter/tree-sitter-typescript"), Some("tsx")),
            Some("tsx".into())
        );
        assert_eq!(resolve_bundled("Rust", None, None), Some("rust".into()));
        assert_eq!(resolve_bundled("no-such-grammar", Some("https://github.com/x/y"), None), None);
    }

    fn classes(path: &str, src: &str) -> Vec<Vec<(String, TokenClass)>> {
        with_grammars();
        let lines: Vec<&str> = src.lines().collect();
        let spans = highlight(path, &lines, crate::MAX_HIGHLIGHT_BYTES).expect("highlights");
        spans
            .iter()
            .zip(&lines)
            .map(|(spans, line)| {
                spans
                    .iter()
                    .map(|s| (line[s.range.clone()].to_string(), s.class))
                    .collect()
            })
            .collect()
    }

    fn has(line: &[(String, TokenClass)], text: &str, class: TokenClass) -> bool {
        line.iter().any(|(t, c)| t == text && *c == class)
    }

    #[test]
    fn codes_round_trip() {
        for class in CLASSES {
            assert_eq!(class_of(code_of(class)), Some(class));
        }
        assert_eq!(class_of(0), None);
    }

    #[test]
    fn rust() {
        let c = classes(
            "src/main.rs",
            "// hi\nfn main() {\n    let s: String = \"x\";\n    self.go(true);\n}",
        );
        assert!(has(&c[0], "// hi", TokenClass::Comment), "{c:?}");
        assert!(has(&c[1], "fn", TokenClass::Keyword), "{c:?}");
        assert!(has(&c[2], "String", TokenClass::Type), "{c:?}");
        assert!(has(&c[2], "\"x\"", TokenClass::String), "{c:?}");
        assert!(has(&c[3], "self", TokenClass::Keyword), "{c:?}");
        assert!(has(&c[3], "true", TokenClass::Atom), "{c:?}");
    }

    #[test]
    fn a_multi_line_comment_is_cut_per_line() {
        let c = classes("x.js", "/* a\nb */ let x = 1;");
        assert!(has(&c[0], "/* a", TokenClass::Comment), "{c:?}");
        assert!(has(&c[1], "b */", TokenClass::Comment), "{c:?}");
    }

    #[test]
    fn markdown_injects_inline_and_fenced_code() {
        let c = classes(
            "README.md",
            "# Title\n\nSee [docs](https://x.y).\n\n```rust\nfn f() {}\n```",
        );
        let within = |line: &[(String, TokenClass)], text: &str, class| {
            line.iter().any(|(t, c)| t.contains(text) && *c == class)
        };
        assert!(within(&c[0], "Title", TokenClass::Header), "{c:?}");
        assert!(within(&c[2], "docs", TokenClass::Link), "{c:?}");
        assert!(has(&c[2], "https://x.y", TokenClass::String), "{c:?}");
        assert!(has(&c[5], "fn", TokenClass::Keyword), "{c:?}");
    }

    #[test]
    fn unknown_files_and_the_budget() {
        with_grammars();
        assert!(highlight("x.unknownext", &["x"], 100).is_none());
        let spans = highlight("a.rs", &["fn a() {}", "fn b() {}"], 12).expect("rust");
        assert!(!spans[0].is_empty());
        assert!(spans[1].is_empty());
    }
}
