//! TextMate → sublime-syntax, the conversion tools/tm-grammars/sync.py
//! makes at build time for the compiled-in grammars, done at run time for
//! an extension's. Each `begin` / `end` rule becomes a `push` whose context
//! carries the rule's scopes and ends with the `pop`; `while` becomes a pop
//! on the first line the pattern does not match. What sublime-syntax cannot
//! express is recorded in a [`ConversionReport`] rather than silently lost:
//! patterns inside captures, `$1`-style scope substitutions, injections.

use super::{Capture, Rule, TmGrammar};

/// Bumped when the conversion changes, so cached user sets rebuild.
pub const CONVERTER_VERSION: u32 = 1;

/// A sublime-syntax document, before it is emitted as YAML.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SublimeSyntax {
    pub name: String,
    pub scope: String,
    pub file_extensions: Vec<String>,
    pub first_line_match: Option<String>,
    pub hidden: bool,
    /// `main` first, then the repository's contexts in order
    pub contexts: Vec<(String, Vec<Item>)>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Include(String),
    MetaScope(String),
    MetaContentScope(String),
    Match(MatchItem),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MatchItem {
    pub regex: String,
    pub scope: Option<String>,
    pub captures: Vec<(usize, String)>,
    pub push: Option<Vec<Item>>,
    pub pop: bool,
}

/// What the grammar said that the result does not.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConversionReport {
    pub dropped: Vec<Dropped>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Dropped {
    /// the sublime-syntax context the rule landed in (`main`, `repo_x`)
    pub context: String,
    pub kind: DroppedKind,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DroppedKind {
    /// a scope name with a `$1` substitution or a backslash
    ScopeSubstitution,
    /// `captures` with their own `patterns` (only the scopes survive)
    CapturePatterns,
    /// an `include` of a repository key the grammar lacks
    UnknownInclude,
    /// a top-level `injections` entry
    Injection,
    /// `\G` (match where the previous one ended), removed from a regex
    GAnchor,
    /// a rule with neither `include`, `match`, `begin` nor `patterns`
    EmptyRule,
    /// a rule marked `disabled`
    DisabledRule,
}

impl DroppedKind {
    pub fn describe(self) -> &'static str {
        match self {
            Self::ScopeSubstitution => "scope name with a $-substitution",
            Self::CapturePatterns => "patterns inside a capture",
            Self::UnknownInclude => "include of a missing repository rule",
            Self::Injection => "injection",
            Self::GAnchor => "\\G anchor removed",
            Self::EmptyRule => "empty rule",
            Self::DisabledRule => "disabled rule",
        }
    }
}

/// What the result should be called and when it applies; the grammar's
/// own `fileTypes` / `firstLineMatch` are used when these are empty.
#[derive(Clone, Debug, Default)]
pub struct Options {
    pub name: Option<String>,
    pub file_extensions: Vec<String>,
    pub first_line_match: Option<String>,
    pub hidden: bool,
}

/// Convert `grammar`.
pub fn convert(grammar: &TmGrammar, options: &Options) -> (SublimeSyntax, ConversionReport) {
    let mut c = Converter {
        grammar,
        report: ConversionReport::default(),
        repository: Vec::new(),
        context: "main".to_string(),
    };
    // every repository key, including rule-local ones (merged when absent
    // at the top, so `#name` includes inside those rules still resolve)
    for (key, rule) in &grammar.repository {
        c.repository.push((key.clone(), rule.clone()));
    }
    collect_local_repositories(&grammar.patterns, &mut c.repository);
    for (_, rule) in grammar.repository.clone() {
        collect_local_repositories(&rule.patterns, &mut c.repository);
        collect_local_repositories(&[rule], &mut c.repository);
    }
    let mut contexts = Vec::new();
    c.context = "main".to_string();
    let main = c.patterns(&grammar.patterns);
    contexts.push(("main".to_string(), main));
    let repository = c.repository.clone();
    for (key, rule) in &repository {
        c.context = format!("repo_{key}");
        let items = c.rule(rule);
        contexts.push((c.context.clone(), items));
    }
    for (selector, _) in &grammar.injections {
        c.drop(DroppedKind::Injection, selector.clone());
    }
    let name = options
        .name
        .clone()
        .or_else(|| grammar.name.clone())
        .unwrap_or_else(|| grammar.scope_name.clone());
    let file_extensions = if options.file_extensions.is_empty() {
        grammar
            .file_types
            .iter()
            .map(|t| t.trim_start_matches('.').to_string())
            .filter(|t| !t.is_empty())
            .collect()
    } else {
        options.file_extensions.clone()
    };
    let first_line_match = options
        .first_line_match
        .clone()
        .or_else(|| grammar.first_line_match.clone())
        .map(|rx| c.fix_regex(&rx));
    (
        SublimeSyntax {
            name,
            scope: grammar.scope_name.clone(),
            file_extensions,
            first_line_match,
            hidden: options.hidden,
            contexts,
        },
        c.report,
    )
}

fn collect_local_repositories(rules: &[Rule], into: &mut Vec<(String, Rule)>) {
    for rule in rules {
        for (key, local) in &rule.repository {
            if !into.iter().any(|(k, _)| k == key) {
                into.push((key.clone(), local.clone()));
            }
        }
        collect_local_repositories(&rule.patterns, into);
        for (_, capture) in rule
            .captures
            .iter()
            .chain(&rule.begin_captures)
            .chain(&rule.end_captures)
        {
            collect_local_repositories(&capture.patterns, into);
        }
    }
}

struct Converter<'a> {
    grammar: &'a TmGrammar,
    report: ConversionReport,
    repository: Vec<(String, Rule)>,
    /// the context being converted (for the report)
    context: String,
}

impl Converter<'_> {
    fn drop(&mut self, kind: DroppedKind, detail: String) {
        self.report.dropped.push(Dropped {
            context: self.context.clone(),
            kind,
            detail,
        });
    }

    /// Oniguruma-isms syntect's loader does not take, where a safe rewrite
    /// exists: `\G` (where the previous match ended; usually redundant in a
    /// line-based engine) goes, `\h` / `\H` become hex-digit classes.
    fn fix_regex(&mut self, regex: &str) -> String {
        let mut out = String::with_capacity(regex.len());
        let mut chars = regex.chars().peekable();
        while let Some(c) = chars.next() {
            if c != '\\' {
                out.push(c);
                continue;
            }
            match chars.next() {
                Some('G') => self.drop(DroppedKind::GAnchor, regex.to_string()),
                Some('h') => out.push_str("[0-9A-Fa-f]"),
                Some('H') => out.push_str("[^0-9A-Fa-f]"),
                Some(e) => {
                    out.push('\\');
                    out.push(e);
                }
                None => out.push('\\'),
            }
        }
        out
    }

    /// A scope name without the `$1`-style parts sublime-syntax lacks.
    fn scope_name(&mut self, name: Option<&str>) -> Option<String> {
        let name = name?.trim();
        if name.is_empty() {
            return None;
        }
        let mut kept = Vec::new();
        let mut dropped = false;
        for part in name.split_whitespace() {
            if part.contains('$') || part.contains('\\') {
                dropped = true;
            } else {
                kept.push(part);
            }
        }
        if dropped {
            self.drop(DroppedKind::ScopeSubstitution, name.to_string());
        }
        (!kept.is_empty()).then(|| kept.join(" "))
    }

    fn captures(&mut self, captures: &[(String, Capture)]) -> Vec<(usize, String)> {
        let mut out = Vec::new();
        for (key, capture) in captures {
            let Ok(index) = key.trim().parse::<usize>() else {
                continue;
            };
            if !capture.patterns.is_empty() {
                self.drop(DroppedKind::CapturePatterns, format!("capture {index}"));
            }
            if let Some(scope) = self.scope_name(capture.name.as_deref()) {
                out.push((index, scope));
            }
        }
        out
    }

    fn include(&mut self, reference: &str) -> Option<Item> {
        if let Some(key) = reference.strip_prefix('#') {
            if self.repository.iter().any(|(k, _)| k == key) {
                return Some(Item::Include(format!("repo_{key}")));
            }
            self.drop(DroppedKind::UnknownInclude, reference.to_string());
            return None;
        }
        if reference == "$self" || reference == "$base" {
            return Some(Item::Include("main".to_string()));
        }
        let scope = reference.split('#').next().unwrap_or(reference);
        if scope == self.grammar.scope_name {
            return Some(Item::Include("main".to_string()));
        }
        Some(Item::Include(format!("scope:{scope}")))
    }

    fn patterns(&mut self, rules: &[Rule]) -> Vec<Item> {
        let mut out = Vec::new();
        for rule in rules {
            out.extend(self.rule(rule));
        }
        out
    }

    fn rule(&mut self, rule: &Rule) -> Vec<Item> {
        if rule.disabled {
            self.drop(DroppedKind::DisabledRule, String::new());
            return Vec::new();
        }
        if let Some(reference) = &rule.include {
            return self.include(reference).into_iter().collect();
        }
        if let Some(regex) = &rule.match_ {
            let item = MatchItem {
                regex: self.fix_regex(regex),
                scope: self.scope_name(rule.name.as_deref()),
                captures: self.captures(&rule.captures),
                push: None,
                pop: false,
            };
            return vec![Item::Match(item)];
        }
        if let Some(begin) = &rule.begin
            && (rule.end.is_some() || rule.while_.is_some())
        {
            let begin_captures = if rule.begin_captures.is_empty() {
                &rule.captures
            } else {
                &rule.begin_captures
            };
            let end_captures = if rule.end_captures.is_empty() {
                &rule.captures
            } else {
                &rule.end_captures
            };
            let regex = self.fix_regex(begin);
            let captures = self.captures(begin_captures);
            let mut body = Vec::new();
            if let Some(scope) = self.scope_name(rule.name.as_deref()) {
                body.push(Item::MetaScope(scope));
            }
            if let Some(scope) = self.scope_name(rule.content_name.as_deref()) {
                body.push(Item::MetaContentScope(scope));
            }
            let end = match (&rule.end, &rule.while_) {
                (Some(end), _) => MatchItem {
                    regex: self.fix_regex(end),
                    captures: self.captures(end_captures),
                    pop: true,
                    ..Default::default()
                },
                (None, Some(while_)) => {
                    // leave at the first line the pattern does not match
                    let fixed = self.fix_regex(while_);
                    MatchItem {
                        regex: format!("^(?!{fixed})"),
                        pop: true,
                        ..Default::default()
                    }
                }
                (None, None) => unreachable!("checked above"),
            };
            let inner = self.patterns(&rule.patterns);
            if rule.apply_end_pattern_last {
                body.extend(inner);
                body.push(Item::Match(end));
            } else {
                body.push(Item::Match(end));
                body.extend(inner);
            }
            return vec![Item::Match(MatchItem {
                regex,
                scope: None,
                captures,
                push: Some(body),
                pop: false,
            })];
        }
        if !rule.patterns.is_empty() {
            return self.patterns(&rule.patterns);
        }
        self.drop(DroppedKind::EmptyRule, String::new());
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tm::{GrammarFormat, parse_text};

    fn grammar() -> TmGrammar {
        parse_text(
            r##"{"scopeName": "source.t", "name": "T", "fileTypes": [".t", "tt"],
                "firstLineMatch": "^#!\\G.*t",
                "patterns": [
                    {"include": "#kw"},
                    {"include": "#missing"},
                    {"include": "source.other#frag"},
                    {"include": "$self"},
                    {"begin": "\"", "end": "\"", "name": "string.quoted meta.$1",
                     "beginCaptures": {"0": {"name": "punctuation.begin"}},
                     "endCaptures": {"0": {"name": "punctuation.end", "patterns": [{"match": "x"}]}},
                     "patterns": [{"match": "\\\\.", "name": "constant.character.escape"}]},
                    {"begin": "<<", "while": "^\\s+", "name": "heredoc", "applyEndPatternLast": true,
                     "patterns": [{"include": "#kw"}]},
                    {"match": "\\h+", "name": "constant.numeric.hex", "captures": {"0": {"name": "x"}, "bad": {"name": "y"}}},
                    {"disabled": 1, "match": "z"},
                    {"name": "nothing"}
                ],
                "repository": {"kw": {"match": "\\b(if)\\b", "name": "keyword.control"}},
                "injections": {"L:comment": {"patterns": [{"match": "TODO"}]}}}"##,
            GrammarFormat::Json,
        )
        .expect("grammar")
    }

    #[test]
    fn converts_each_rule_kind() {
        let (syntax, report) = convert(&grammar(), &Options::default());
        assert_eq!(syntax.name, "T");
        assert_eq!(syntax.scope, "source.t");
        assert_eq!(syntax.file_extensions, vec!["t", "tt"]);
        assert_eq!(syntax.first_line_match.as_deref(), Some("^#!.*t"));
        assert_eq!(syntax.contexts[0].0, "main");
        assert_eq!(syntax.contexts[1].0, "repo_kw");
        let main = &syntax.contexts[0].1;
        assert_eq!(main[0], Item::Include("repo_kw".into()));
        assert_eq!(main[1], Item::Include("scope:source.other".into()));
        assert_eq!(main[2], Item::Include("main".into()));
        let Item::Match(string) = &main[3] else {
            panic!("expected the string rule");
        };
        assert_eq!(string.regex, "\"");
        assert_eq!(string.captures, vec![(0, "punctuation.begin".to_string())]);
        let body = string.push.as_ref().expect("push");
        assert_eq!(body[0], Item::MetaScope("string.quoted".into()));
        let Item::Match(end) = &body[1] else {
            panic!("pop first");
        };
        assert!(end.pop);
        assert_eq!(end.captures, vec![(0, "punctuation.end".to_string())]);
        let Item::Match(heredoc) = &main[4] else {
            panic!("heredoc");
        };
        let body = heredoc.push.as_ref().expect("push");
        assert_eq!(body[0], Item::MetaScope("heredoc".into()));
        assert_eq!(body[1], Item::Include("repo_kw".into()));
        let Item::Match(pop) = &body[2] else {
            panic!("pop last");
        };
        assert_eq!(pop.regex, "^(?!^\\s+)");
        let Item::Match(hex) = &main[5] else {
            panic!("hex");
        };
        assert_eq!(hex.regex, "[0-9A-Fa-f]+");
        assert_eq!(hex.captures, vec![(0, "x".to_string())]);
        assert_eq!(main.len(), 6);
        let kinds: Vec<DroppedKind> = report.dropped.iter().map(|d| d.kind).collect();
        assert!(kinds.contains(&DroppedKind::GAnchor));
        assert!(kinds.contains(&DroppedKind::UnknownInclude));
        assert!(kinds.contains(&DroppedKind::ScopeSubstitution));
        assert!(kinds.contains(&DroppedKind::CapturePatterns));
        assert!(kinds.contains(&DroppedKind::DisabledRule));
        assert!(kinds.contains(&DroppedKind::EmptyRule));
        assert!(kinds.contains(&DroppedKind::Injection));
    }

    #[test]
    fn options_override_detection() {
        let options = Options {
            name: Some("Other".into()),
            file_extensions: vec!["o".into()],
            first_line_match: Some("^x".into()),
            hidden: true,
        };
        let (syntax, _) = convert(&grammar(), &options);
        assert_eq!(syntax.name, "Other");
        assert_eq!(syntax.file_extensions, vec!["o"]);
        assert_eq!(syntax.first_line_match.as_deref(), Some("^x"));
        assert!(syntax.hidden);
    }
}
