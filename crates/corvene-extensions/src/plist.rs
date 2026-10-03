//! XML property lists, the subset TextMate grammars use (`.tmLanguage`,
//! `.plist`): `dict`, `array`, `string`, `integer`, `real`, `true`,
//! `false`. The `plist` crate would bring an XML stack along; grammar files
//! need none of it. A DOCTYPE is skipped unread, so external entities are
//! never fetched.

use crate::ExtensionError;
use crate::value::{Value, position};

/// Parse an XML plist document.
pub fn parse(text: &str) -> Result<Value, ExtensionError> {
    if text.len() > crate::MAX_GRAMMAR_BYTES {
        return Err(ExtensionError::parse("plist", "the file is too large"));
    }
    let mut p = Parser {
        text,
        at: 0,
        depth: 0,
    };
    p.prolog()?;
    let (name, _) = p.open_tag()?;
    if name != "plist" {
        return Err(p.err("expected a <plist> element"));
    }
    p.skip_ws();
    let value = if p.text[p.at..].starts_with("</plist>") {
        Value::Null
    } else {
        p.value()?
    };
    p.skip_ws();
    p.close_tag("plist")?;
    Ok(value)
}

struct Parser<'a> {
    text: &'a str,
    at: usize,
    depth: usize,
}

impl Parser<'_> {
    fn err(&self, message: &str) -> ExtensionError {
        let (line, column) = position(self.text.as_bytes(), self.at);
        ExtensionError::parse("plist", format!("{message} at line {line} column {column}"))
    }

    fn rest(&self) -> &str {
        &self.text[self.at..]
    }

    fn skip_ws(&mut self) {
        loop {
            let rest = self.rest();
            let trimmed = rest.trim_start();
            let skipped = rest.len() - trimmed.len();
            let comment = trimmed
                .starts_with("<!--")
                .then(|| trimmed.find("-->").map(|end| end + 3));
            self.at += skipped;
            match comment {
                None => return,
                Some(Some(len)) => self.at += len,
                Some(None) => self.at = self.text.len(),
            }
        }
    }

    /// The XML declaration and the DOCTYPE, skipped.
    fn prolog(&mut self) -> Result<(), ExtensionError> {
        self.at += self.rest().len() - self.rest().trim_start_matches('\u{feff}').len();
        loop {
            self.skip_ws();
            let rest = self.rest();
            if rest.starts_with("<?") {
                let end = rest
                    .find("?>")
                    .ok_or_else(|| self.err("unterminated declaration"))?;
                self.at += end + 2;
            } else if rest.starts_with("<!DOCTYPE") || rest.starts_with("<!doctype") {
                // a DOCTYPE may hold an internal subset in brackets
                let mut depth = 0usize;
                let mut end = None;
                for (i, c) in rest.char_indices() {
                    match c {
                        '[' => depth += 1,
                        ']' => depth = depth.saturating_sub(1),
                        '>' if depth == 0 => {
                            end = Some(i + 1);
                            break;
                        }
                        _ => {}
                    }
                }
                self.at += end.ok_or_else(|| self.err("unterminated DOCTYPE"))?;
            } else {
                return Ok(());
            }
        }
    }

    /// `<name attr…>` or `<name/>`: the name and whether it self-closed.
    fn open_tag(&mut self) -> Result<(String, bool), ExtensionError> {
        self.skip_ws();
        if !self.rest().starts_with('<') || self.rest().starts_with("</") {
            return Err(self.err("expected an element"));
        }
        let rest = self.rest();
        let end = rest.find('>').ok_or_else(|| self.err("unterminated tag"))?;
        let inner = &rest[1..end];
        let self_closing = inner.ends_with('/');
        let inner = inner.trim_end_matches('/');
        let name = inner
            .split(|c: char| c.is_whitespace())
            .next()
            .unwrap_or("")
            .to_string();
        if name.is_empty() {
            return Err(self.err("an element without a name"));
        }
        self.at += end + 1;
        Ok((name, self_closing))
    }

    fn close_tag(&mut self, name: &str) -> Result<(), ExtensionError> {
        self.skip_ws();
        let rest = self.rest();
        if let Some(after) = rest.strip_prefix("</")
            && let Some(end) = after.find('>')
            && after[..end].trim() == name
        {
            self.at += 2 + end + 1;
            return Ok(());
        }
        Err(self.err(&format!("expected </{name}>")))
    }

    /// Text up to the next `<`, with entities and CDATA resolved.
    fn text_content(&mut self) -> Result<String, ExtensionError> {
        let mut out = String::new();
        loop {
            let rest = self.rest();
            if let Some(after) = rest.strip_prefix("<![CDATA[") {
                let end = after
                    .find("]]>")
                    .ok_or_else(|| self.err("unterminated CDATA"))?;
                out.push_str(&after[..end]);
                self.at += 9 + end + 3;
                continue;
            }
            let end = rest.find('<').unwrap_or(rest.len());
            unescape(&rest[..end], &mut out).map_err(|m| self.err(&m))?;
            self.at += end;
            if !self.rest().starts_with("<![CDATA[") {
                return Ok(out);
            }
        }
    }

    fn value(&mut self) -> Result<Value, ExtensionError> {
        self.depth += 1;
        if self.depth > crate::value::MAX_DEPTH {
            return Err(self.err("nesting too deep"));
        }
        let (name, self_closing) = self.open_tag()?;
        let value = match name.as_str() {
            "true" | "false" if self_closing => Value::Bool(name == "true"),
            "dict" => {
                let mut entries: Vec<(String, Value)> = Vec::new();
                if !self_closing {
                    loop {
                        self.skip_ws();
                        if self.rest().starts_with("</") {
                            break;
                        }
                        let (key_tag, key_closed) = self.open_tag()?;
                        if key_tag != "key" {
                            return Err(self.err("expected <key>"));
                        }
                        let key = if key_closed {
                            String::new()
                        } else {
                            let key = self.text_content()?;
                            self.close_tag("key")?;
                            key
                        };
                        let value = self.value()?;
                        if let Some(slot) = entries.iter_mut().find(|(k, _)| *k == key) {
                            slot.1 = value;
                        } else {
                            entries.push((key, value));
                        }
                    }
                    self.close_tag("dict")?;
                }
                Value::Map(entries)
            }
            "array" => {
                let mut items = Vec::new();
                if !self_closing {
                    loop {
                        self.skip_ws();
                        if self.rest().starts_with("</") {
                            break;
                        }
                        items.push(self.value()?);
                    }
                    self.close_tag("array")?;
                }
                Value::List(items)
            }
            "string" | "integer" | "real" | "date" | "data" => {
                let text = if self_closing {
                    String::new()
                } else {
                    let text = self.text_content()?;
                    self.close_tag(&name)?;
                    text
                };
                match name.as_str() {
                    "integer" => text
                        .trim()
                        .parse()
                        .map(Value::Int)
                        .map_err(|_| self.err("invalid integer"))?,
                    "real" => text
                        .trim()
                        .parse()
                        .map(Value::Float)
                        .map_err(|_| self.err("invalid real"))?,
                    _ => Value::Str(text),
                }
            }
            _ => return Err(self.err(&format!("unexpected element <{name}>"))),
        };
        self.depth -= 1;
        Ok(value)
    }
}

/// Resolve the XML entities grammars use.
fn unescape(text: &str, out: &mut String) -> Result<(), String> {
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let after = &rest[amp + 1..];
        let semi = after
            .find(';')
            .filter(|i| *i <= 10)
            .ok_or_else(|| "a bare '&'".to_string())?;
        let entity = &after[..semi];
        match entity {
            "lt" => out.push('<'),
            "gt" => out.push('>'),
            "amp" => out.push('&'),
            "quot" => out.push('"'),
            "apos" => out.push('\''),
            _ => {
                let code = if let Some(hex) = entity.strip_prefix("#x") {
                    u32::from_str_radix(hex, 16).ok()
                } else if let Some(dec) = entity.strip_prefix('#') {
                    dec.parse().ok()
                } else {
                    None
                };
                let ch = code
                    .and_then(char::from_u32)
                    .ok_or_else(|| format!("unknown entity &{entity};"))?;
                out.push(ch);
            }
        }
        rest = &after[semi + 1..];
    }
    out.push_str(rest);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>name</key>
	<string>Sample &amp; co</string>
	<key>fileTypes</key>
	<array>
		<string>smp</string>
		<string>sample</string>
	</array>
	<!-- a comment -->
	<key>patterns</key>
	<array>
		<dict>
			<key>match</key>
			<string>\b(if|else)\b &lt;x&gt;</string>
			<key>name</key>
			<string>keyword.control</string>
		</dict>
	</array>
	<key>uuid</key>
	<string><![CDATA[a<b]]></string>
	<key>n</key>
	<integer>3</integer>
	<key>flag</key>
	<true/>
</dict>
</plist>
"#;

    #[test]
    fn reads_a_grammar_plist() {
        let v = parse(SAMPLE).expect("parses");
        assert_eq!(v.str_of("name"), Some("Sample & co"));
        assert_eq!(v.strings_of("fileTypes"), vec!["smp", "sample"]);
        let patterns = v
            .get("patterns")
            .and_then(Value::as_list)
            .expect("patterns");
        assert_eq!(patterns[0].str_of("match"), Some(r"\b(if|else)\b <x>"));
        assert_eq!(v.str_of("uuid"), Some("a<b"));
        assert_eq!(v.get("n"), Some(&Value::Int(3)));
        assert_eq!(v.get("flag"), Some(&Value::Bool(true)));
    }

    #[test]
    fn rejects_broken_documents() {
        assert!(parse("<plist><dict><key>a</key></dict></plist>").is_err());
        assert!(parse("<plist><dict><key>a</key><string>x</dict></plist>").is_err());
        assert!(parse("<html/>").is_err());
        assert!(parse("<plist><string>&bogus;</string></plist>").is_err());
    }
}
