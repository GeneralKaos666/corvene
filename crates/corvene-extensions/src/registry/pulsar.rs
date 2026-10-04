//! Pulsar's package registry (the continuation of Atom's): `language-*`
//! packages hold `grammars/*.cson`. The API's tarball endpoint redirects to
//! the package's GitHub release.

use super::{Candidate, GrammarHint, Registry, encode};
use crate::ExtensionError;
use crate::value::{Value, parse_json};

const BASE: &str = "https://api.pulsar-edit.dev/api";

pub fn search(query: &str) -> Result<Vec<Candidate>, ExtensionError> {
    let url = format!(
        "{BASE}/packages/search?q={}&sort=downloads&direction=desc",
        encode(query.trim())
    );
    parse_search(&crate::http::get_text(&url)?)
}

pub fn for_suffix(suffix: &str) -> Result<Vec<Candidate>, ExtensionError> {
    // the registry has no suffix index; `language-<suffix>` is the convention
    let suffix = suffix.trim_start_matches('.').to_ascii_lowercase();
    let mut found = search(&format!("language-{suffix}"))?;
    found.retain(|c| c.name.starts_with("language-"));
    Ok(found)
}

/// The JSON of `/api/packages/search` (a list of packages).
pub fn parse_search(text: &str) -> Result<Vec<Candidate>, ExtensionError> {
    let value = parse_json(text)?;
    let items = match &value {
        Value::List(items) => items.as_slice(),
        _ => return Ok(Vec::new()),
    };
    Ok(items.iter().filter_map(candidate).collect())
}

fn candidate(item: &Value) -> Option<Candidate> {
    let name = item.str_of("name")?;
    let version = item
        .get("releases")
        .and_then(|r| r.str_of("latest"))
        .or_else(|| item.get("metadata").and_then(|m| m.str_of("version")))?;
    let metadata = item.get("metadata");
    let repository = crate::manifest::repository_url(item.get("repository"))
        .or_else(|| metadata.and_then(|m| crate::manifest::repository_url(m.get("repository"))));
    // the registry has no suffix index; `language-<x>` names its language
    let suffixes: Vec<String> = name
        .strip_prefix("language-")
        .filter(|s| !s.is_empty() && !s.contains('-'))
        .map(|s| vec![s.to_ascii_lowercase()])
        .unwrap_or_default();
    let tree_sitter = metadata
        .and_then(|m| m.get("dependencies"))
        .and_then(Value::as_map)
        .is_some_and(|deps| deps.iter().any(|(k, _)| k.starts_with("tree-sitter")));
    Some(Candidate {
        registry: Registry::Pulsar,
        id: name.to_string(),
        name: name.to_string(),
        display_name: name.to_string(),
        publisher: repository
            .as_deref()
            .and_then(|r| r.strip_prefix("https://github.com/"))
            .and_then(|r| r.split('/').next())
            .map(str::to_string),
        version: Some(version.to_string()),
        description: metadata
            .and_then(|m| m.str_of("description"))
            .map(str::to_string),
        repository,
        download_url: format!(
            "{BASE}/packages/{}/versions/{}/tarball",
            encode(name),
            encode(version)
        ),
        grammar: if tree_sitter {
            GrammarHint::TreeSitter
        } else {
            GrammarHint::TextMate
        },
        suffixes,
        downloads: item
            .get("downloads")
            .and_then(|d| match d {
                Value::Int(n) => Some(*n as u64),
                Value::Str(s) => s.parse().ok(),
                _ => None,
            })
            .unwrap_or(0),
    })
}

/// `/api/packages/<name>`: the latest version.
pub fn latest(name: &str) -> Result<Option<Candidate>, ExtensionError> {
    let url = format!("{BASE}/packages/{}", encode(name));
    let value = parse_json(&crate::http::get_text(&url)?)?;
    Ok(candidate(&value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_search_answer() {
        let text = r#"[{"name":"language-zig","repository":{"type":"git","url":"https://github.com/Purple-Fox-Coder/language-zig"},
            "downloads":"1234","releases":{"latest":"2.0.0"},"metadata":{"description":"Zig for Atom","version":"2.0.0",
            "dependencies":{}}},
            {"name":"language-zig-treesitter","repository":{"url":"https://github.com/GrayJack/language-zig-treesitter"},
            "downloads":5,"releases":{"latest":"0.2.0"},"metadata":{"dependencies":{"tree-sitter-zig":"0.2.0"}}}]"#;
        let found = parse_search(text).expect("parses");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].id, "language-zig");
        assert_eq!(found[0].publisher.as_deref(), Some("Purple-Fox-Coder"));
        assert_eq!(found[0].downloads, 1234);
        assert_eq!(found[0].grammar, GrammarHint::TextMate);
        assert!(
            found[0]
                .download_url
                .ends_with("/packages/language-zig/versions/2.0.0/tarball")
        );
        assert_eq!(found[1].grammar, GrammarHint::TreeSitter);
        assert_eq!(super::super::extension_id(&found[0]), "pulsar.language-zig");
    }
}
