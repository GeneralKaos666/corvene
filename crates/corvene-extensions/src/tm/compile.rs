//! A converted grammar as syntect's [`SyntaxDefinition`], checked the way
//! examples/tm-build.rs checks the compiled-in grammars: every regex must
//! compile under Oniguruma now, because syntect compiles lazily and would
//! otherwise panic the first time a line reaches the pattern.

use syntect::parsing::syntax_definition::{MatchOperation, Pattern};
use syntect::parsing::{Regex, SyntaxDefinition};

use crate::ExtensionError;

/// Load sublime-syntax text. `name` names the syntax when the text does not.
pub fn compile(text: &str, name: &str) -> Result<SyntaxDefinition, ExtensionError> {
    let definition =
        std::panic::catch_unwind(|| SyntaxDefinition::load_from_str(text, true, Some(name)))
            .map_err(|_| ExtensionError::Compile("syntect could not read the grammar".to_string()))?
            .map_err(|err| ExtensionError::Compile(err.to_string()))?;
    if let Some(problem) = bad_regex(&definition) {
        return Err(ExtensionError::Compile(problem));
    }
    Ok(definition)
}

/// The first regex of `definition` Oniguruma rejects. Patterns that refer
/// to the push match's captures (`\1`) are checked with a placeholder in
/// place of each reference, which is what syntect substitutes at run time.
pub fn bad_regex(definition: &SyntaxDefinition) -> Option<String> {
    for (context_name, context) in &definition.contexts {
        for pattern in &context.patterns {
            let Pattern::Match(m) = pattern else {
                continue;
            };
            let source = m.regex.regex_str();
            let candidate = if m.has_captures {
                substitute_backrefs(source)
            } else {
                source.to_string()
            };
            if let Some(err) = Regex::try_compile(&candidate) {
                let what = match m.operation {
                    MatchOperation::Pop => "pop pattern",
                    _ => "pattern",
                };
                return Some(format!("{context_name}: {what} {source:?}: {err}"));
            }
        }
    }
    None
}

/// `\1`…`\9` replaced by a literal, as syntect does with the captured text.
fn substitute_backrefs(regex: &str) -> String {
    let mut out = String::with_capacity(regex.len());
    let mut chars = regex.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.peek() {
                Some(d) if d.is_ascii_digit() => {
                    chars.next();
                    out.push('x');
                    continue;
                }
                Some(_) => {
                    out.push('\\');
                    if let Some(e) = chars.next() {
                        out.push(e);
                    }
                    continue;
                }
                None => {}
            }
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tm::convert::{Options, convert};
    use crate::tm::emit::emit;
    use crate::tm::{GrammarFormat, parse_text};

    fn compiled(json: &str) -> Result<SyntaxDefinition, ExtensionError> {
        let grammar = parse_text(json, GrammarFormat::Json)?;
        let (syntax, _) = convert(&grammar, &Options::default());
        compile(&emit(&syntax), &syntax.name)
    }

    #[test]
    fn a_converted_grammar_loads() {
        let def = compiled(
            r#"{"scopeName": "source.t", "fileTypes": ["t"], "patterns": [
                {"match": "\\b(if|else)\\b", "name": "keyword.control"},
                {"begin": "(<)(\\w+)", "end": "</\\2>", "beginCaptures": {"1": {"name": "punctuation"}},
                 "name": "meta.tag", "patterns": [{"match": "\\w+", "name": "entity.name.tag"}]}
            ]}"#,
        )
        .expect("compiles");
        assert_eq!(def.scope.to_string(), "source.t");
        assert_eq!(def.file_extensions, vec!["t"]);
        assert!(def.contexts.contains_key("main"));
    }

    #[test]
    fn a_bad_regex_is_reported_not_deferred() {
        let err = compiled(
            r#"{"scopeName": "source.b", "patterns": [{"match": "fine", "name": "x"}, {"match": "(unclosed", "name": "y"}]}"#,
        )
        .expect_err("rejected");
        assert!(matches!(err, ExtensionError::Compile(_)), "{err}");
        assert!(err.to_string().contains("unclosed"), "{err}");
        let err = compiled(
            r#"{"scopeName": "source.c", "patterns": [{"begin": "(x)", "end": "\\1(", "name": "y"}]}"#,
        )
        .expect_err("pop pattern rejected");
        // syntect itself checks pop patterns with a placeholder for `\1`
        assert!(matches!(err, ExtensionError::Compile(_)), "{err}");
        assert!(err.to_string().contains("parenthesis"), "{err}");
    }

    #[test]
    fn backrefs_are_replaced_for_the_check() {
        assert_eq!(substitute_backrefs(r"</\2>\\1\d"), r"</x>\\1\d");
    }
}
