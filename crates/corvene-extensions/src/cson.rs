//! CSON (CoffeeScript Object Notation), the format Atom and Pulsar grammars
//! are written in: unquoted keys, indentation-defined objects, `[ ]` lists
//! (one item per line or comma-separated), `'…'` / `"…"` strings and `'''`
//! / `"""` block strings that hold the regexes, `#` comments, `true` /
//! `false` / `null`, numbers. Nothing else of CoffeeScript is accepted.

use crate::ExtensionError;
use crate::value::{Value, position};

/// Parse a CSON document.
pub fn parse(text: &str) -> Result<Value, ExtensionError> {
    if text.len() > crate::MAX_GRAMMAR_BYTES {
        return Err(ExtensionError::parse("cson", "the file is too large"));
    }
    let text = text.trim_start_matches('\u{feff}');
    let mut p = Parser {
        bytes: text.as_bytes(),
        text,
        at: 0,
        depth: 0,
    };
    p.skip_blank_lines();
    let indent = p.line_indent();
    let value = if p.rest().starts_with('{') || p.rest().starts_with('[') || !p.looks_like_key() {
        p.inline_value()?
    } else {
        p.block_object(indent)?
    };
    p.skip_space_and_comments();
    p.skip_blank_lines();
    if p.at < p.bytes.len() {
        return Err(p.err("trailing content"));
    }
    Ok(value)
}

struct Parser<'a> {
    text: &'a str,
    bytes: &'a [u8],
    at: usize,
    depth: usize,
}

impl Parser<'_> {
    fn err(&self, message: &str) -> ExtensionError {
        let (line, column) = position(self.bytes, self.at);
        ExtensionError::parse("cson", format!("{message} at line {line} column {column}"))
    }

    fn rest(&self) -> &str {
        &self.text[self.at..]
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    fn enter(&mut self) -> Result<(), ExtensionError> {
        self.depth += 1;
        if self.depth > crate::value::MAX_DEPTH {
            return Err(self.err("nesting too deep"));
        }
        Ok(())
    }

    /// Spaces, tabs and a `#` comment on the current line.
    fn skip_space_and_comments(&mut self) {
        while let Some(c) = self.peek() {
            match c {
                b' ' | b'\t' | b'\r' => self.at += 1,
                b'#' => {
                    while let Some(c) = self.peek() {
                        if c == b'\n' {
                            break;
                        }
                        self.at += 1;
                    }
                }
                _ => return,
            }
        }
    }

    /// Whitespace, comments and newlines.
    fn skip_blank_lines(&mut self) {
        loop {
            self.skip_space_and_comments();
            if self.peek() == Some(b'\n') {
                self.at += 1;
            } else {
                return;
            }
        }
    }

    /// Indentation of the line `at` is on (columns; a tab counts as one).
    fn line_indent(&self) -> usize {
        let start = self.bytes[..self.at]
            .iter()
            .rposition(|b| *b == b'\n')
            .map_or(0, |i| i + 1);
        self.bytes[start..]
            .iter()
            .take_while(|b| matches!(b, b' ' | b'\t'))
            .count()
    }

    /// Whether the current position starts a `key:` (quoted or bare).
    fn looks_like_key(&self) -> bool {
        let rest = self.rest();
        let bytes = rest.as_bytes();
        let end = match bytes.first() {
            Some(q @ (b'\'' | b'"')) => {
                let mut i = 1;
                loop {
                    match bytes.get(i) {
                        None | Some(b'\n') => return false,
                        Some(b'\\') => i += 2,
                        Some(c) if c == q => break i + 1,
                        Some(_) => i += 1,
                    }
                }
            }
            _ => match key_end(rest) {
                Some(end) => end,
                None => return false,
            },
        };
        rest.get(end..)
            .is_some_and(|after| after.trim_start_matches([' ', '\t']).starts_with(':'))
    }

    /// `key: value` lines at `indent`, until a line indented less.
    fn block_object(&mut self, indent: usize) -> Result<Value, ExtensionError> {
        self.enter()?;
        let mut entries: Vec<(String, Value)> = Vec::new();
        loop {
            self.skip_blank_lines();
            if self.at >= self.bytes.len() || self.line_indent() < indent {
                break;
            }
            if self.line_indent() > indent {
                return Err(self.err("unexpected indentation"));
            }
            if self.rest().starts_with('}') || self.rest().starts_with(']') {
                break;
            }
            let key = self.key()?;
            self.skip_space_and_comments();
            if self.peek() != Some(b':') {
                return Err(self.err("expected ':'"));
            }
            self.at += 1;
            self.skip_space_and_comments();
            let value = if self.peek() == Some(b'\n') || self.peek().is_none() {
                // the value is a nested block on the following lines
                let save = self.at;
                self.skip_blank_lines();
                if self.at < self.bytes.len() && self.line_indent() > indent {
                    let inner = self.line_indent();
                    if self.looks_like_key() {
                        self.block_object(inner)?
                    } else {
                        self.inline_value()?
                    }
                } else {
                    self.at = save;
                    Value::Null
                }
            } else {
                self.inline_value()?
            };
            if let Some(slot) = entries.iter_mut().find(|(k, _)| *k == key) {
                slot.1 = value;
            } else {
                entries.push((key, value));
            }
            self.skip_space_and_comments();
            if self.peek() == Some(b',') {
                self.at += 1;
            }
        }
        self.depth -= 1;
        Ok(Value::Map(entries))
    }

    fn key(&mut self) -> Result<String, ExtensionError> {
        match self.peek() {
            Some(b'\'' | b'"') => self.string(),
            _ => {
                let end = key_end(self.rest()).ok_or_else(|| self.err("expected a key"))?;
                let key = self.rest()[..end].to_string();
                self.at += end;
                Ok(key)
            }
        }
    }

    /// A value starting on the current line: a string, number, literal,
    /// `[ … ]` list or `{ … }` object.
    fn inline_value(&mut self) -> Result<Value, ExtensionError> {
        match self.peek() {
            None => Err(self.err("expected a value")),
            Some(b'\'' | b'"') => Ok(Value::Str(self.string()?)),
            Some(b'[') => self.list(),
            Some(b'{') => self.braced_object(),
            Some(c) if c == b'-' || c.is_ascii_digit() => self.number(),
            Some(_) => {
                let rest = self.rest();
                let end = rest
                    .find(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .unwrap_or(rest.len());
                let word = &rest[..end];
                let value = match word {
                    "true" | "yes" | "on" => Value::Bool(true),
                    "false" | "no" | "off" => Value::Bool(false),
                    "null" | "undefined" => Value::Null,
                    _ => return Err(self.err("unexpected word")),
                };
                self.at += end;
                Ok(value)
            }
        }
    }

    fn number(&mut self) -> Result<Value, ExtensionError> {
        let start = self.at;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() || matches!(c, b'-' | b'+' | b'.' | b'e' | b'E' | b'x' | b'X') {
                self.at += 1;
            } else {
                break;
            }
        }
        let text = &self.text[start..self.at];
        if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
            return i64::from_str_radix(hex, 16)
                .map(Value::Int)
                .map_err(|_| self.err("invalid number"));
        }
        if let Ok(n) = text.parse::<i64>() {
            return Ok(Value::Int(n));
        }
        text.parse::<f64>()
            .map(Value::Float)
            .map_err(|_| self.err("invalid number"))
    }

    fn list(&mut self) -> Result<Value, ExtensionError> {
        self.enter()?;
        self.at += 1;
        let mut items = Vec::new();
        loop {
            self.skip_blank_lines();
            match self.peek() {
                None => return Err(self.err("unterminated list")),
                Some(b']') => {
                    self.at += 1;
                    break;
                }
                Some(b',') => {
                    self.at += 1;
                    continue;
                }
                _ => {}
            }
            let item = if self.looks_like_key() {
                // an object written without braces as a list item
                let indent = self.line_indent();
                self.block_object(indent)?
            } else {
                self.inline_value()?
            };
            items.push(item);
        }
        self.depth -= 1;
        Ok(Value::List(items))
    }

    fn braced_object(&mut self) -> Result<Value, ExtensionError> {
        self.enter()?;
        self.at += 1;
        let mut entries: Vec<(String, Value)> = Vec::new();
        loop {
            self.skip_blank_lines();
            match self.peek() {
                None => return Err(self.err("unterminated object")),
                Some(b'}') => {
                    self.at += 1;
                    break;
                }
                Some(b',') => {
                    self.at += 1;
                    continue;
                }
                _ => {}
            }
            let key = self.key()?;
            self.skip_space_and_comments();
            if self.peek() != Some(b':') {
                return Err(self.err("expected ':'"));
            }
            self.at += 1;
            self.skip_blank_lines();
            let value = self.inline_value()?;
            entries.push((key, value));
        }
        self.depth -= 1;
        Ok(Value::Map(entries))
    }

    /// `'…'`, `"…"`, `'''…'''` or `"""…"""`. Block strings drop the common
    /// indentation and a leading / trailing newline, as CoffeeScript does;
    /// single-quoted strings keep backslashes (regexes) except `\'` and `\\`.
    fn string(&mut self) -> Result<String, ExtensionError> {
        let quote = self.peek().unwrap_or(b'\'');
        let triple = self.rest().as_bytes().starts_with(&[quote, quote, quote]);
        self.at += if triple { 3 } else { 1 };
        let start = self.at;
        let end = loop {
            let Some(c) = self.peek() else {
                return Err(self.err("unterminated string"));
            };
            if c == b'\\' {
                self.at += 2;
                continue;
            }
            if c == quote {
                if !triple {
                    break self.at;
                }
                if self.rest().as_bytes().starts_with(&[quote, quote, quote]) {
                    // the last of a longer run of quotes closes the block
                    let mut run = 3;
                    while self.bytes.get(self.at + run) == Some(&quote) {
                        run += 1;
                    }
                    self.at += run - 3;
                    break self.at;
                }
            }
            if !triple && c == b'\n' {
                return Err(self.err("unterminated string"));
            }
            self.at += 1;
        };
        let raw = &self.text[start..end];
        self.at = end + if triple { 3 } else { 1 };
        let body = if triple { dedent(raw) } else { raw.to_string() };
        Ok(unescape(&body, quote == b'"'))
    }
}

/// End of an unquoted key (`[A-Za-z0-9_$-]+`, or a quoted one's length).
fn key_end(rest: &str) -> Option<usize> {
    let end = rest
        .find(|c: char| !(c.is_alphanumeric() || matches!(c, '_' | '$' | '-' | '.')))
        .unwrap_or(rest.len());
    (end > 0).then_some(end)
}

/// CoffeeScript block string rules: strip one leading and trailing newline
/// and the smallest indentation of the remaining lines.
fn dedent(raw: &str) -> String {
    let raw = raw.strip_prefix('\n').unwrap_or(raw);
    let raw = raw.strip_suffix('\n').unwrap_or(raw);
    let raw = raw.trim_end_matches([' ', '\t']);
    let indent = raw
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start_matches([' ', '\t']).len())
        .min()
        .unwrap_or(0);
    raw.lines()
        .map(|l| {
            if l.len() >= indent {
                &l[indent..]
            } else {
                l.trim_start()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Backslash escapes. Double-quoted strings honour the JavaScript set; in
/// single-quoted ones only `\'` and `\\` are escapes, the rest stay (so a
/// regex written `'\\b'` means `\b` and `'\b'` stays `\b`).
fn unescape(body: &str, double: bool) -> String {
    let mut out = String::with_capacity(body.len());
    let mut chars = body.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let Some(e) = chars.next() else {
            out.push('\\');
            break;
        };
        match e {
            '\\' => out.push('\\'),
            '\'' if !double => out.push('\''),
            '"' if double => out.push('"'),
            'n' if double => out.push('\n'),
            't' if double => out.push('\t'),
            'r' if double => out.push('\r'),
            'u' if double => {
                let hex: String = chars.by_ref().take(4).collect();
                match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    Some(ch) => out.push(ch),
                    None => {
                        out.push_str("\\u");
                        out.push_str(&hex);
                    }
                }
            }
            _ => {
                out.push('\\');
                out.push(e);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"# An Atom grammar
'scopeName': 'source.sample'
'name': "Sample"
'fileTypes': [
  'smp'
  'sample'
]
'firstLineMatch': '^#!.*\bsample\b'
'patterns': [
  {
    'match': '\\b(if|else)\\b'
    'name': 'keyword.control.sample'
  }
  {
    'begin': '"'
    'end': '"'
    'name': 'string.quoted.double.sample'
    'patterns': [
      { 'include': '#escape' }
    ]
  }
]
'repository':
  'escape':
    'match': '''
      \\\\.
    '''
    'name': 'constant.character.escape.sample'
  'number':
    'match': '\\d+', 'name': 'constant.numeric'
"#;

    #[test]
    fn reads_an_atom_grammar() {
        let v = parse(SAMPLE).expect("parses");
        assert_eq!(v.str_of("scopeName"), Some("source.sample"));
        assert_eq!(v.str_of("name"), Some("Sample"));
        assert_eq!(v.strings_of("fileTypes"), vec!["smp", "sample"]);
        assert_eq!(v.str_of("firstLineMatch"), Some(r"^#!.*\bsample\b"));
        let patterns = v
            .get("patterns")
            .and_then(Value::as_list)
            .expect("patterns");
        assert_eq!(patterns.len(), 2);
        assert_eq!(patterns[0].str_of("match"), Some(r"\b(if|else)\b"));
        let inner = patterns[1]
            .get("patterns")
            .and_then(Value::as_list)
            .expect("inner");
        assert_eq!(inner[0].str_of("include"), Some("#escape"));
        let repo = v.get("repository").expect("repository");
        assert_eq!(
            repo.get("escape").and_then(|e| e.str_of("match")),
            Some(r"\\.")
        );
        assert_eq!(
            repo.get("number").and_then(|e| e.str_of("match")),
            Some(r"\d+")
        );
        assert_eq!(
            repo.get("number").and_then(|e| e.str_of("name")),
            Some("constant.numeric")
        );
    }

    #[test]
    fn braces_commas_and_literals() {
        let v = parse("{a: 1, b: [1, 2,], c: {d: true}, e: null, f: 0x10}\n").expect("parses");
        assert_eq!(v.get("a"), Some(&Value::Int(1)));
        assert_eq!(
            v.get("b"),
            Some(&Value::List(vec![Value::Int(1), Value::Int(2)]))
        );
        assert_eq!(
            v.get("c").and_then(|c| c.get("d")),
            Some(&Value::Bool(true))
        );
        assert_eq!(v.get("e"), Some(&Value::Null));
        assert_eq!(v.get("f"), Some(&Value::Int(16)));
        assert!(parse("a: [1").is_err());
        assert!(parse("a: 'x").is_err());
    }

    #[test]
    fn block_strings_dedent() {
        let v = parse("x: '''\n    line one\n      indented\n    '''\ny: \"\"\"a\"\"\"\n")
            .expect("parses");
        assert_eq!(v.str_of("x"), Some("line one\n  indented"));
        assert_eq!(v.str_of("y"), Some("a"));
    }
}
