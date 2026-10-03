//! TextMate grammars: the data model editors share (VS Code, Atom/Pulsar,
//! IntelliJ's TextMate bundles, Sublime's `.tmLanguage`), read from any of
//! their encodings ([`parse_text`]) and converted to the sublime-syntax form
//! syntect loads ([`convert`]).

pub mod compile;
pub mod convert;
pub mod emit;

use std::path::Path;

use crate::ExtensionError;
use crate::value::Value;

/// How a grammar file is encoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrammarFormat {
    /// `.tmLanguage.json`, `.json` (JSON with comments)
    Json,
    /// `.tmLanguage`, `.plist` (XML property list)
    Plist,
    /// `.cson` (Atom, Pulsar)
    Cson,
    /// `.YAML-tmLanguage`, `.tmLanguage.yaml`
    Yaml,
}

impl GrammarFormat {
    /// The format a file's name says, if any.
    pub fn from_path(path: &Path) -> Option<Self> {
        let name = path.file_name()?.to_str()?.to_ascii_lowercase();
        if name.ends_with(".json") {
            Some(Self::Json)
        } else if name.ends_with(".cson") {
            Some(Self::Cson)
        } else if name.ends_with(".yaml-tmlanguage")
            || name.ends_with(".yaml")
            || name.ends_with(".yml")
        {
            Some(Self::Yaml)
        } else if name.ends_with(".tmlanguage") || name.ends_with(".plist") {
            Some(Self::Plist)
        } else {
            None
        }
    }

    /// The format a file's first bytes say (its name may lie: VS Code
    /// treats every non-`.json` grammar as a plist).
    pub fn sniff(text: &str) -> Option<Self> {
        let head = text.trim_start_matches('\u{feff}').trim_start();
        if head.starts_with('<') {
            Some(Self::Plist)
        } else if head.starts_with('{') || head.starts_with("//") || head.starts_with("/*") {
            Some(Self::Json)
        } else if head.starts_with('\'') || head.starts_with("# ") || head.starts_with("#!") {
            // CSON files start with a quoted key or a comment
            Some(Self::Cson)
        } else if head.starts_with("---") || head.starts_with('%') {
            Some(Self::Yaml)
        } else {
            None
        }
    }
}

/// A TextMate grammar.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TmGrammar {
    pub scope_name: String,
    pub name: Option<String>,
    /// `fileTypes`: extensions without the dot, or file names
    pub file_types: Vec<String>,
    pub first_line_match: Option<String>,
    pub patterns: Vec<Rule>,
    pub repository: Vec<(String, Rule)>,
    /// `injections`: selector → rule (not representable in sublime-syntax;
    /// reported as dropped)
    pub injections: Vec<(String, Rule)>,
    pub injection_selector: Option<String>,
}

/// One rule of a grammar. Which fields are set decides its kind: an
/// `include`, a `match`, a `begin` / `end` (or `begin` / `while`) pair, or
/// a bare list of `patterns`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rule {
    pub include: Option<String>,
    pub match_: Option<String>,
    pub begin: Option<String>,
    pub end: Option<String>,
    pub while_: Option<String>,
    pub name: Option<String>,
    pub content_name: Option<String>,
    pub captures: Vec<(String, Capture)>,
    pub begin_captures: Vec<(String, Capture)>,
    pub end_captures: Vec<(String, Capture)>,
    pub patterns: Vec<Rule>,
    pub apply_end_pattern_last: bool,
    pub disabled: bool,
    /// a rule-local repository (TextMate allows one at any level)
    pub repository: Vec<(String, Rule)>,
}

/// What a capture group gets: a scope, and possibly its own patterns.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Capture {
    pub name: Option<String>,
    pub patterns: Vec<Rule>,
}

impl TmGrammar {
    /// Read a grammar from its parsed tree.
    pub fn from_value(value: &Value) -> Result<Self, ExtensionError> {
        let scope_name = value
            .str_of("scopeName")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ExtensionError::Convert("the grammar has no scopeName".to_string()))?
            .to_string();
        let mut depth = 0;
        Ok(TmGrammar {
            scope_name,
            name: value.str_of("name").map(str::to_string),
            file_types: value.strings_of("fileTypes"),
            first_line_match: value.str_of("firstLineMatch").map(str::to_string),
            patterns: rules_of(value.get("patterns"), &mut depth)?,
            repository: repository_of(value.get("repository"), &mut depth)?,
            injections: repository_of(value.get("injections"), &mut depth)?,
            injection_selector: value.str_of("injectionSelector").map(str::to_string),
        })
    }
}

fn rules_of(value: Option<&Value>, depth: &mut usize) -> Result<Vec<Rule>, ExtensionError> {
    let Some(Value::List(items)) = value else {
        return Ok(Vec::new());
    };
    items
        .iter()
        .filter(|v| v.as_map().is_some())
        .map(|v| Rule::from_value(v, depth))
        .collect()
}

fn repository_of(
    value: Option<&Value>,
    depth: &mut usize,
) -> Result<Vec<(String, Rule)>, ExtensionError> {
    let Some(Value::Map(entries)) = value else {
        return Ok(Vec::new());
    };
    entries
        .iter()
        .filter(|(_, v)| v.as_map().is_some())
        .map(|(k, v)| Ok((k.clone(), Rule::from_value(v, depth)?)))
        .collect()
}

fn captures_of(
    value: Option<&Value>,
    depth: &mut usize,
) -> Result<Vec<(String, Capture)>, ExtensionError> {
    let Some(Value::Map(entries)) = value else {
        return Ok(Vec::new());
    };
    let mut out = Vec::with_capacity(entries.len());
    for (key, v) in entries {
        let Some(_) = v.as_map() else {
            continue;
        };
        out.push((
            key.clone(),
            Capture {
                name: v.str_of("name").map(str::to_string),
                patterns: rules_of(v.get("patterns"), depth)?,
            },
        ));
    }
    Ok(out)
}

impl Rule {
    fn from_value(value: &Value, depth: &mut usize) -> Result<Self, ExtensionError> {
        *depth += 1;
        if *depth > crate::value::MAX_DEPTH {
            return Err(ExtensionError::Convert("rules nest too deeply".to_string()));
        }
        let text = |key: &str| value.str_of(key).map(str::to_string);
        let rule = Rule {
            include: text("include"),
            match_: text("match"),
            begin: text("begin"),
            end: text("end"),
            while_: text("while"),
            name: text("name"),
            content_name: text("contentName"),
            captures: captures_of(value.get("captures"), depth)?,
            begin_captures: captures_of(value.get("beginCaptures"), depth)?,
            end_captures: captures_of(value.get("endCaptures"), depth)?,
            patterns: rules_of(value.get("patterns"), depth)?,
            apply_end_pattern_last: value
                .get("applyEndPatternLast")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            disabled: value
                .get("disabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            repository: repository_of(value.get("repository"), depth)?,
        };
        *depth -= 1;
        Ok(rule)
    }
}

/// Parse grammar text in `format`.
pub fn parse_text(text: &str, format: GrammarFormat) -> Result<TmGrammar, ExtensionError> {
    if text.len() > crate::MAX_GRAMMAR_BYTES {
        return Err(ExtensionError::Convert(
            "the grammar file is too large".to_string(),
        ));
    }
    let value = match format {
        GrammarFormat::Json => crate::value::parse_json(text)?,
        GrammarFormat::Plist => crate::plist::parse(text)?,
        GrammarFormat::Cson => crate::cson::parse(text)?,
        GrammarFormat::Yaml => crate::value::parse_yaml(text)?,
    };
    TmGrammar::from_value(&value)
}

/// Read a grammar file: the format its name says first, then the one its
/// first bytes suggest (a `.tmLanguage` holding JSON, a `.json` with a
/// comment header), whichever parses.
pub fn parse_file(path: &Path) -> Result<TmGrammar, ExtensionError> {
    let meta = std::fs::metadata(path)?;
    if meta.len() > crate::MAX_GRAMMAR_BYTES as u64 {
        return Err(ExtensionError::Convert(format!(
            "{} is too large for a grammar",
            path.display()
        )));
    }
    let text = std::fs::read_to_string(path)?;
    let mut candidates: Vec<GrammarFormat> = Vec::new();
    for format in [GrammarFormat::from_path(path), GrammarFormat::sniff(&text)]
        .into_iter()
        .flatten()
    {
        if !candidates.contains(&format) {
            candidates.push(format);
        }
    }
    if candidates.is_empty() {
        return Err(ExtensionError::Convert(format!(
            "{} is not a grammar file",
            path.display()
        )));
    }
    let mut first_error = None;
    for format in candidates {
        match parse_text(&text, format) {
            Ok(grammar) => return Ok(grammar),
            Err(err) => first_error.get_or_insert(err),
        };
    }
    Err(first_error.unwrap_or_else(|| {
        ExtensionError::Convert(format!("{} is not a grammar file", path.display()))
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_from_names_and_contents() {
        assert_eq!(
            GrammarFormat::from_path(Path::new("x.tmLanguage.json")),
            Some(GrammarFormat::Json)
        );
        assert_eq!(
            GrammarFormat::from_path(Path::new("x.tmLanguage")),
            Some(GrammarFormat::Plist)
        );
        assert_eq!(
            GrammarFormat::from_path(Path::new("x.cson")),
            Some(GrammarFormat::Cson)
        );
        assert_eq!(
            GrammarFormat::from_path(Path::new("x.YAML-tmLanguage")),
            Some(GrammarFormat::Yaml)
        );
        assert_eq!(GrammarFormat::from_path(Path::new("x.txt")), None);
        assert_eq!(GrammarFormat::sniff("<?xml"), Some(GrammarFormat::Plist));
        assert_eq!(
            GrammarFormat::sniff("\u{feff}{\"a\":1}"),
            Some(GrammarFormat::Json)
        );
        assert_eq!(
            GrammarFormat::sniff("'scopeName': 'x'"),
            Some(GrammarFormat::Cson)
        );
        assert_eq!(
            GrammarFormat::sniff("---\nname: x"),
            Some(GrammarFormat::Yaml)
        );
    }

    #[test]
    fn reads_rules_of_every_kind() {
        let g = parse_text(
            r##"{"scopeName": "source.t", "fileTypes": ["t"], "patterns": [
                {"include": "#a"},
                {"match": "x", "name": "n", "captures": {"1": {"name": "c", "patterns": [{"match": "y"}]}}},
                {"begin": "(", "end": ")", "contentName": "cn", "applyEndPatternLast": 1,
                 "patterns": [{"include": "$self"}], "repository": {"local": {"match": "z"}}},
                {"patterns": [{"match": "w", "disabled": 1}]}
            ], "repository": {"a": {"match": "a"}}, "injections": {"L:source.t": {"patterns": []}}}"##,
            GrammarFormat::Json,
        )
        .expect("parses");
        assert_eq!(g.scope_name, "source.t");
        assert_eq!(g.patterns.len(), 4);
        assert_eq!(g.patterns[0].include.as_deref(), Some("#a"));
        assert_eq!(g.patterns[1].captures[0].1.patterns.len(), 1);
        assert!(g.patterns[2].apply_end_pattern_last);
        assert_eq!(g.patterns[2].repository[0].0, "local");
        assert!(g.patterns[3].patterns[0].disabled);
        assert_eq!(g.injections[0].0, "L:source.t");
        assert!(parse_text(r#"{"name": "no scope"}"#, GrammarFormat::Json).is_err());
    }
}
