//! A [`SublimeSyntax`] as the YAML text syntect reads. The emitter is this
//! small on purpose: every string is a double-quoted scalar with JSON
//! escapes (valid YAML, and never mistaken for a number, a boolean or a
//! null, which a bare `0`, `on` or `~` would be), keys and values in the
//! order the converter produced them, no anchors, no flow style.

use std::fmt::Write;

use super::convert::{CONVERTER_VERSION, Item, MatchItem, SublimeSyntax};

/// The sublime-syntax document for `syntax`.
pub fn emit(syntax: &SublimeSyntax) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "%YAML 1.2");
    let _ = writeln!(out, "---");
    let _ = writeln!(
        out,
        "# {} ({}): converted from TextMate by corvene-extensions (converter {CONVERTER_VERSION})",
        syntax.name, syntax.scope
    );
    let _ = writeln!(out, "name: {}", quote(&syntax.name));
    let _ = writeln!(out, "scope: {}", quote(&syntax.scope));
    if !syntax.file_extensions.is_empty() {
        let _ = writeln!(out, "file_extensions:");
        for ext in &syntax.file_extensions {
            let _ = writeln!(out, "  - {}", quote(ext));
        }
    }
    if let Some(first) = &syntax.first_line_match {
        let _ = writeln!(out, "first_line_match: {}", quote(first));
    }
    if syntax.hidden {
        let _ = writeln!(out, "hidden: true");
    }
    let _ = writeln!(out, "contexts:");
    for (name, items) in &syntax.contexts {
        if items.is_empty() {
            let _ = writeln!(out, "  {}: []", quote(name));
            continue;
        }
        let _ = writeln!(out, "  {}:", quote(name));
        for item in items {
            emit_item(&mut out, item, 2);
        }
    }
    out
}

/// One list item at `indent` (in two-space steps).
fn emit_item(out: &mut String, item: &Item, indent: usize) {
    let pad = "  ".repeat(indent);
    match item {
        Item::Include(name) => {
            let _ = writeln!(out, "{pad}- include: {}", quote(name));
        }
        Item::MetaScope(scope) => {
            let _ = writeln!(out, "{pad}- meta_scope: {}", quote(scope));
        }
        Item::MetaContentScope(scope) => {
            let _ = writeln!(out, "{pad}- meta_content_scope: {}", quote(scope));
        }
        Item::Match(m) => emit_match(out, m, indent),
    }
}

fn emit_match(out: &mut String, m: &MatchItem, indent: usize) {
    let pad = "  ".repeat(indent);
    let _ = writeln!(out, "{pad}- match: {}", quote(&m.regex));
    if let Some(scope) = &m.scope {
        let _ = writeln!(out, "{pad}  scope: {}", quote(scope));
    }
    if !m.captures.is_empty() {
        let _ = writeln!(out, "{pad}  captures:");
        for (index, scope) in &m.captures {
            let _ = writeln!(out, "{pad}    {index}: {}", quote(scope));
        }
    }
    if m.pop {
        let _ = writeln!(out, "{pad}  pop: true");
    }
    if let Some(body) = &m.push {
        if body.is_empty() {
            let _ = writeln!(out, "{pad}  push: []");
        } else {
            let _ = writeln!(out, "{pad}  push:");
            for item in body {
                emit_item(out, item, indent + 2);
            }
        }
    }
}

/// A double-quoted YAML scalar.
pub fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20
                || c == '\u{7f}'
                || c == '\u{85}'
                || c == '\u{2028}'
                || c == '\u{2029}' =>
            {
                let _ = write!(out, "\\u{:04X}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_readable_yaml_that_round_trips() {
        let syntax = SublimeSyntax {
            name: "T".into(),
            scope: "source.t".into(),
            file_extensions: vec!["t".into(), "0".into()],
            first_line_match: Some("^#!.*\\bt\\b".into()),
            hidden: false,
            contexts: vec![
                (
                    "main".into(),
                    vec![
                        Item::Include("repo_a".into()),
                        Item::Match(MatchItem {
                            regex: "\"".into(),
                            scope: None,
                            captures: vec![(0, "punctuation".into())],
                            push: Some(vec![
                                Item::MetaScope("string".into()),
                                Item::Match(MatchItem {
                                    regex: "\"".into(),
                                    pop: true,
                                    ..Default::default()
                                }),
                            ]),
                            pop: false,
                        }),
                    ],
                ),
                ("repo_a".into(), vec![]),
            ],
        };
        let text = emit(&syntax);
        let value = crate::value::parse_yaml(&text).expect("valid yaml");
        assert_eq!(value.str_of("name"), Some("T"));
        assert_eq!(value.strings_of("file_extensions"), vec!["t", "0"]);
        let contexts = value.get("contexts").expect("contexts");
        let main = contexts
            .get("main")
            .and_then(|m| m.as_list())
            .expect("main");
        assert_eq!(main[0].str_of("include"), Some("repo_a"));
        let push = main[1].get("push").and_then(|p| p.as_list()).expect("push");
        assert_eq!(push[1].get("pop").and_then(|p| p.as_bool()), Some(true));
        assert_eq!(
            contexts.get("repo_a"),
            Some(&crate::value::Value::List(vec![]))
        );
        assert_eq!(quote("a\"b\\c\td\u{1}"), "\"a\\\"b\\\\c\\td\\u0001\"");
    }
}
