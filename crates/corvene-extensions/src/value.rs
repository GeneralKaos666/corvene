//! The tree every grammar and manifest format is read into, so one
//! TextMate reader serves JSON, plist, CSON and YAML sources. Maps keep
//! their order (a grammar's repository order decides tie-breaks between
//! rules, as in the editors).

use std::fmt;

use crate::ExtensionError;

/// A JSON-like value with ordered maps.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    List(Vec<Value>),
    Map(Vec<(String, Value)>),
}

/// Nesting deeper than this is refused (untrusted input).
pub const MAX_DEPTH: usize = 256;

impl Value {
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Map(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            Value::Int(n) => Some(*n != 0),
            Value::Str(s) => match s.as_str() {
                "true" | "1" => Some(true),
                "false" | "0" => Some(false),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[Value]> {
        match self {
            Value::List(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_map(&self) -> Option<&[(String, Value)]> {
        match self {
            Value::Map(entries) => Some(entries),
            _ => None,
        }
    }

    /// `key` as a string.
    pub fn str_of(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }

    /// `key` as a list of strings (non-strings skipped; a lone string is a
    /// one-item list, as grammars sometimes write `fileTypes: "x"`).
    pub fn strings_of(&self, key: &str) -> Vec<String> {
        match self.get(key) {
            Some(Value::List(items)) => items
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect(),
            Some(Value::Str(s)) => vec![s.clone()],
            _ => Vec::new(),
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Int(n) => write!(f, "{n}"),
            Value::Float(x) => write!(f, "{x}"),
            Value::Str(s) => write!(f, "{s:?}"),
            Value::List(items) => write!(f, "[{} items]", items.len()),
            Value::Map(entries) => write!(f, "{{{} keys}}", entries.len()),
        }
    }
}

// --- JSON with comments ------------------------------------------------------

/// Parse JSON, allowing `//` and `/* */` comments and trailing commas (VS
/// Code grammars are written as JSONC).
pub fn parse_json(text: &str) -> Result<Value, ExtensionError> {
    let mut p = Json {
        bytes: text.as_bytes(),
        at: 0,
        depth: 0,
    };
    p.skip_ws()?;
    let value = p.value()?;
    p.skip_ws()?;
    if p.at != p.bytes.len() {
        return Err(p.err("trailing characters after the document"));
    }
    Ok(value)
}

struct Json<'a> {
    bytes: &'a [u8],
    at: usize,
    depth: usize,
}

impl Json<'_> {
    fn err(&self, message: &str) -> ExtensionError {
        let (line, column) = position(self.bytes, self.at);
        ExtensionError::parse("json", format!("{message} at line {line} column {column}"))
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    fn skip_ws(&mut self) -> Result<(), ExtensionError> {
        loop {
            match self.peek() {
                Some(b' ' | b'\t' | b'\n' | b'\r') => self.at += 1,
                Some(b'/') => match self.bytes.get(self.at + 1) {
                    Some(b'/') => {
                        while let Some(c) = self.peek() {
                            if c == b'\n' {
                                break;
                            }
                            self.at += 1;
                        }
                    }
                    Some(b'*') => {
                        self.at += 2;
                        loop {
                            match self.peek() {
                                None => return Err(self.err("unterminated comment")),
                                Some(b'*') if self.bytes.get(self.at + 1) == Some(&b'/') => {
                                    self.at += 2;
                                    break;
                                }
                                Some(_) => self.at += 1,
                            }
                        }
                    }
                    _ => return Err(self.err("unexpected '/'")),
                },
                // a UTF-8 BOM
                Some(0xEF) if self.bytes[self.at..].starts_with(&[0xEF, 0xBB, 0xBF]) => {
                    self.at += 3
                }
                _ => return Ok(()),
            }
        }
    }

    fn value(&mut self) -> Result<Value, ExtensionError> {
        match self.peek() {
            None => Err(self.err("unexpected end of input")),
            Some(b'{') => self.map(),
            Some(b'[') => self.list(),
            Some(b'"') => Ok(Value::Str(self.string()?)),
            Some(b't') => self.literal("true", Value::Bool(true)),
            Some(b'f') => self.literal("false", Value::Bool(false)),
            Some(b'n') => self.literal("null", Value::Null),
            Some(c) if c == b'-' || c.is_ascii_digit() => self.number(),
            Some(_) => Err(self.err("unexpected character")),
        }
    }

    fn literal(&mut self, word: &str, value: Value) -> Result<Value, ExtensionError> {
        if self.bytes[self.at..].starts_with(word.as_bytes()) {
            self.at += word.len();
            Ok(value)
        } else {
            Err(self.err("unexpected word"))
        }
    }

    fn number(&mut self) -> Result<Value, ExtensionError> {
        let start = self.at;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() || matches!(c, b'-' | b'+' | b'.' | b'e' | b'E') {
                self.at += 1;
            } else {
                break;
            }
        }
        let text = std::str::from_utf8(&self.bytes[start..self.at]).unwrap_or("");
        if let Ok(n) = text.parse::<i64>() {
            return Ok(Value::Int(n));
        }
        text.parse::<f64>()
            .map(Value::Float)
            .map_err(|_| self.err("invalid number"))
    }

    fn string(&mut self) -> Result<String, ExtensionError> {
        self.at += 1; // the opening quote
        let mut out = String::new();
        loop {
            let Some(c) = self.peek() else {
                return Err(self.err("unterminated string"));
            };
            self.at += 1;
            match c {
                b'"' => return Ok(out),
                b'\\' => {
                    let Some(e) = self.peek() else {
                        return Err(self.err("unterminated escape"));
                    };
                    self.at += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let first = self.hex4()?;
                            let ch = if (0xD800..0xDC00).contains(&first) {
                                // a surrogate pair
                                if !self.bytes[self.at..].starts_with(b"\\u") {
                                    return Err(self.err("lone surrogate"));
                                }
                                self.at += 2;
                                let second = self.hex4()?;
                                let code = 0x10000
                                    + ((first - 0xD800) << 10)
                                    + second.wrapping_sub(0xDC00);
                                char::from_u32(code)
                            } else {
                                char::from_u32(first)
                            };
                            out.push(ch.unwrap_or('\u{FFFD}'));
                        }
                        _ => return Err(self.err("invalid escape")),
                    }
                }
                _ => {
                    // copy a whole UTF-8 sequence
                    let start = self.at - 1;
                    let len = utf8_len(c);
                    let end = (start + len).min(self.bytes.len());
                    let text = std::str::from_utf8(&self.bytes[start..end])
                        .map_err(|_| self.err("invalid UTF-8"))?;
                    out.push_str(text);
                    self.at = end;
                }
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, ExtensionError> {
        let end = self.at + 4;
        let text = self
            .bytes
            .get(self.at..end)
            .and_then(|b| std::str::from_utf8(b).ok())
            .ok_or_else(|| self.err("short unicode escape"))?;
        let n = u32::from_str_radix(text, 16).map_err(|_| self.err("invalid unicode escape"))?;
        self.at = end;
        Ok(n)
    }

    fn enter(&mut self) -> Result<(), ExtensionError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(self.err("nesting too deep"));
        }
        Ok(())
    }

    fn list(&mut self) -> Result<Value, ExtensionError> {
        self.enter()?;
        self.at += 1;
        let mut items = Vec::new();
        loop {
            self.skip_ws()?;
            match self.peek() {
                Some(b']') => {
                    self.at += 1;
                    break;
                }
                Some(b',') if !items.is_empty() => {
                    // a trailing or repeated comma
                    self.at += 1;
                    continue;
                }
                _ => {}
            }
            items.push(self.value()?);
            self.skip_ws()?;
            match self.peek() {
                Some(b',') => self.at += 1,
                Some(b']') => {}
                _ => return Err(self.err("expected ',' or ']'")),
            }
        }
        self.depth -= 1;
        Ok(Value::List(items))
    }

    fn map(&mut self) -> Result<Value, ExtensionError> {
        self.enter()?;
        self.at += 1;
        let mut entries: Vec<(String, Value)> = Vec::new();
        loop {
            self.skip_ws()?;
            match self.peek() {
                Some(b'}') => {
                    self.at += 1;
                    break;
                }
                Some(b',') if !entries.is_empty() => {
                    self.at += 1;
                    continue;
                }
                Some(b'"') => {}
                _ => return Err(self.err("expected a key")),
            }
            let key = self.string()?;
            self.skip_ws()?;
            if self.peek() != Some(b':') {
                return Err(self.err("expected ':'"));
            }
            self.at += 1;
            self.skip_ws()?;
            let value = self.value()?;
            // a repeated key: the last one wins, as in the editors
            if let Some(slot) = entries.iter_mut().find(|(k, _)| *k == key) {
                slot.1 = value;
            } else {
                entries.push((key, value));
            }
            self.skip_ws()?;
            match self.peek() {
                Some(b',') => self.at += 1,
                Some(b'}') => {}
                _ => return Err(self.err("expected ',' or '}'")),
            }
        }
        self.depth -= 1;
        Ok(Value::Map(entries))
    }
}

pub(crate) fn utf8_len(first: u8) -> usize {
    match first {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        _ => 4,
    }
}

/// 1-based line and column of byte `at`.
pub(crate) fn position(bytes: &[u8], at: usize) -> (usize, usize) {
    let at = at.min(bytes.len());
    let line = bytes[..at].iter().filter(|b| **b == b'\n').count() + 1;
    let column = at
        - bytes[..at]
            .iter()
            .rposition(|b| *b == b'\n')
            .map_or(0, |i| i + 1)
        + 1;
    (line, column)
}

// --- YAML --------------------------------------------------------------------

/// Parse a YAML document (`.YAML-tmLanguage`, Sublime's `.sublime-syntax`
/// header fields).
pub fn parse_yaml(text: &str) -> Result<Value, ExtensionError> {
    if text.len() > crate::MAX_GRAMMAR_BYTES {
        return Err(ExtensionError::parse("yaml", "the file is too large"));
    }
    let docs = yaml_rust::YamlLoader::load_from_str(text)
        .map_err(|err| ExtensionError::parse("yaml", err.to_string()))?;
    let doc = docs.into_iter().next().unwrap_or(yaml_rust::Yaml::Null);
    from_yaml(&doc, 0)
}

fn from_yaml(yaml: &yaml_rust::Yaml, depth: usize) -> Result<Value, ExtensionError> {
    use yaml_rust::Yaml;
    if depth > MAX_DEPTH {
        return Err(ExtensionError::parse("yaml", "nesting too deep"));
    }
    Ok(match yaml {
        Yaml::Null | Yaml::BadValue => Value::Null,
        Yaml::Boolean(b) => Value::Bool(*b),
        Yaml::Integer(n) => Value::Int(*n),
        Yaml::Real(s) => s.parse().map(Value::Float).unwrap_or(Value::Str(s.clone())),
        Yaml::String(s) => Value::Str(s.clone()),
        Yaml::Alias(_) => Value::Null,
        Yaml::Array(items) => Value::List(
            items
                .iter()
                .map(|v| from_yaml(v, depth + 1))
                .collect::<Result<_, _>>()?,
        ),
        Yaml::Hash(map) => {
            let mut entries = Vec::with_capacity(map.len());
            for (k, v) in map {
                let key = match k {
                    Yaml::String(s) => s.clone(),
                    Yaml::Integer(n) => n.to_string(),
                    Yaml::Real(s) => s.clone(),
                    Yaml::Boolean(b) => b.to_string(),
                    _ => continue,
                };
                entries.push((key, from_yaml(v, depth + 1)?));
            }
            Value::Map(entries)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jsonc_with_comments_and_trailing_commas() {
        let v = parse_json(
            r#"// a grammar
            { "scopeName": "source.x", /* block */ "fileTypes": ["x", "xx",],
              "n": -12, "f": 1.5e2, "t": true, "u": "\u00e9\ud83d\ude00", }"#,
        )
        .expect("parses");
        assert_eq!(v.str_of("scopeName"), Some("source.x"));
        assert_eq!(v.strings_of("fileTypes"), vec!["x", "xx"]);
        assert_eq!(v.get("n"), Some(&Value::Int(-12)));
        assert_eq!(v.get("f"), Some(&Value::Float(150.0)));
        assert_eq!(v.get("t"), Some(&Value::Bool(true)));
        assert_eq!(v.str_of("u"), Some("é😀"));
    }

    #[test]
    fn json_keeps_key_order_and_rejects_garbage() {
        let v = parse_json(r#"{"b": 1, "a": 2, "b": 3}"#).expect("parses");
        let keys: Vec<&str> = v
            .as_map()
            .unwrap()
            .iter()
            .map(|(k, _)| k.as_str())
            .collect();
        assert_eq!(keys, vec!["b", "a"]);
        assert_eq!(v.get("b"), Some(&Value::Int(3)));
        assert!(parse_json("{").is_err());
        assert!(parse_json("[1] x").is_err());
        let deep = "[".repeat(300) + &"]".repeat(300);
        assert!(parse_json(&deep).is_err());
    }

    #[test]
    fn yaml_maps_and_lists() {
        let v = parse_yaml("name: X\nfileTypes:\n  - a\n  - b\npatterns:\n  - match: \\d+\n")
            .expect("parses");
        assert_eq!(v.str_of("name"), Some("X"));
        assert_eq!(v.strings_of("fileTypes"), vec!["a", "b"]);
        assert_eq!(
            v.get("patterns")
                .and_then(|p| p.as_list())
                .and_then(|l| l[0].str_of("match")),
            Some("\\d+")
        );
    }
}
