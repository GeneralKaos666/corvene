//! The user grammar set: the converted grammars of every enabled extension
//! compiled into one syntect `SyntaxSet`, written as a dump so later
//! launches load it in milliseconds instead of compiling regexes. The dump
//! is keyed by what went into it ([`cache_key`]); a different key means a
//! rebuild.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use syntect::parsing::syntax_definition::{ContextReference, MatchOperation, Pattern};
use syntect::parsing::{SyntaxDefinition, SyntaxSet, SyntaxSetBuilder};

use crate::ExtensionError;
use crate::tm::convert::CONVERTER_VERSION;

/// The dump format this build writes and reads. Bump it when the pinned
/// syntect version changes: the dump holds syntect's serialized types.
pub const DUMP_VERSION: u32 = 1;

/// A short digest of everything the set depends on: the converter and
/// syntect versions plus `parts` (extension ids, versions, grammar text
/// hashes), in the order given.
pub fn cache_key<I, P>(parts: I) -> String
where
    I: IntoIterator<Item = P>,
    P: AsRef<[u8]>,
{
    let mut hasher = Sha256::new();
    hasher.update(format!(
        "corvene-extensions dump {DUMP_VERSION} converter {CONVERTER_VERSION}\n"
    ));
    for part in parts {
        hasher.update(part.as_ref());
        hasher.update(b"\n");
    }
    let digest = hasher.finalize();
    digest.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// The `scope:` includes of `definitions` that none of them define, so
/// the caller can decide to build on top of the built-in set (a user HTML
/// grammar embedding `source.js`).
pub fn external_scopes(definitions: &[SyntaxDefinition]) -> BTreeSet<String> {
    let own: BTreeSet<String> = definitions.iter().map(|d| d.scope.to_string()).collect();
    let mut external = BTreeSet::new();
    for definition in definitions {
        for context in definition.contexts.values() {
            for pattern in &context.patterns {
                let references: Vec<&ContextReference> = match pattern {
                    Pattern::Include(reference) => vec![reference],
                    Pattern::Match(m) => match &m.operation {
                        MatchOperation::Push(refs) | MatchOperation::Set(refs) => {
                            refs.iter().collect()
                        }
                        _ => Vec::new(),
                    },
                };
                for reference in references {
                    if let ContextReference::ByScope { scope, .. } = reference {
                        let scope = scope.to_string();
                        if !own.contains(&scope) {
                            external.insert(scope);
                        }
                    }
                }
            }
        }
    }
    external
}

/// Compile `definitions` into a set, on top of `base` when given.
pub fn build_set(definitions: Vec<SyntaxDefinition>, base: Option<SyntaxSet>) -> SyntaxSet {
    let mut builder: SyntaxSetBuilder = match base {
        Some(set) => set.into_builder(),
        None => SyntaxSetBuilder::new(),
    };
    for definition in definitions {
        builder.add(definition);
    }
    builder.build()
}

/// Write `set` to `path` (through a temporary file next to it).
pub fn dump(set: &SyntaxSet, path: &Path) -> Result<(), ExtensionError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let partial: PathBuf = path.with_extension("partial");
    syntect::dumps::dump_to_file(set, &partial)
        .map_err(|err| ExtensionError::Compile(format!("writing the grammar set: {err}")))?;
    std::fs::rename(&partial, path)?;
    Ok(())
}

/// Read a set written by [`dump`].
pub fn load(path: &Path) -> Result<SyntaxSet, ExtensionError> {
    syntect::dumps::from_dump_file(path)
        .map_err(|err| ExtensionError::Compile(format!("reading the grammar set: {err}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tm::compile::compile;
    use crate::tm::convert::{Options, convert};
    use crate::tm::emit::emit;
    use crate::tm::{GrammarFormat, parse_text};

    fn definition(json: &str) -> SyntaxDefinition {
        let grammar = parse_text(json, GrammarFormat::Json).expect("grammar");
        let (syntax, _) = convert(&grammar, &Options::default());
        compile(&emit(&syntax), &syntax.name).expect("compiles")
    }

    #[test]
    fn keys_change_with_their_parts() {
        let a = cache_key(["x", "1"]);
        let b = cache_key(["x", "2"]);
        assert_ne!(a, b);
        assert_eq!(a, cache_key(["x", "1"]));
        assert_eq!(a.len(), 16);
    }

    #[test]
    fn external_scopes_are_found() {
        let defs = vec![
            definition(
                r#"{"scopeName": "text.a", "patterns": [{"include": "source.js"}, {"include": "text.b"},
                {"begin": "x", "end": "y", "patterns": [{"include": "source.css#rule"}]}]}"#,
            ),
            definition(r#"{"scopeName": "text.b", "patterns": [{"match": "b"}]}"#),
        ];
        let external = external_scopes(&defs);
        assert_eq!(
            external.into_iter().collect::<Vec<_>>(),
            vec!["source.css".to_string(), "source.js".to_string()]
        );
    }

    #[test]
    fn a_set_dumps_and_loads() {
        let defs = vec![definition(
            r#"{"scopeName": "source.t", "fileTypes": ["tee"], "patterns": [{"match": "\\bif\\b", "name": "keyword"}]}"#,
        )];
        let set = build_set(defs, None);
        assert!(set.find_syntax_by_extension("tee").is_some());
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("user.packdump");
        dump(&set, &path).expect("dump");
        assert!(!path.with_extension("partial").exists());
        let loaded = load(&path).expect("load");
        assert!(loaded.find_syntax_by_extension("tee").is_some());
        // highlighting through the loaded set works
        let syntax = loaded.find_syntax_by_extension("tee").expect("syntax");
        let mut state = syntect::parsing::ParseState::new(syntax);
        let ops = state.parse_line("if x\n", &loaded).expect("parse");
        assert!(ops.iter().any(|(_, op)| matches!(op, syntect::parsing::ScopeStackOp::Push(s) if s.to_string() == "keyword")));
    }
}
