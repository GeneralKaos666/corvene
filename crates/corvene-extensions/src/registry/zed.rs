//! Zed's extension registry (api.zed.dev). The listing names each
//! extension's id, version, repository and what it `provides`; nothing in
//! it says which files an extension covers, so suffix lookups use the
//! table Zed itself suggests from ([`super::zed_suggestions`]). A download
//! is a tar.gz with `extension.toml`, `languages/` and `grammars/*.wasm`.

use super::{Candidate, GrammarHint, Registry, encode, zed_suggestions::SUGGESTIONS};
use crate::ExtensionError;
use crate::value::{Value, parse_json};

const BASE: &str = "https://api.zed.dev";
/// The extension schema this reader understands.
const MAX_SCHEMA_VERSION: u32 = 1;

pub fn search(query: &str) -> Result<Vec<Candidate>, ExtensionError> {
    let url = format!(
        "{BASE}/extensions?filter={}&max_schema_version={MAX_SCHEMA_VERSION}",
        encode(query.trim())
    );
    let mut found = parse_list(&crate::http::get_text(&url)?)?;
    // language extensions only
    found.retain(|c| c.grammar != GrammarHint::Unknown);
    Ok(found)
}

/// Extensions Zed suggests for `suffix` (or a file name), from its own
/// table; each is looked up for its current version.
pub fn for_suffix(suffix: &str) -> Result<Vec<Candidate>, ExtensionError> {
    let wanted = suffix.trim_start_matches('.').to_ascii_lowercase();
    let ids = suggested_ids(&wanted);
    let mut out = Vec::new();
    for id in ids {
        if let Some(mut candidate) = latest(id)? {
            candidate.suffixes = SUGGESTIONS
                .iter()
                .find(|(i, _)| *i == id)
                .map(|(_, s)| s.iter().map(|s| s.to_ascii_lowercase()).collect())
                .unwrap_or_default();
            out.push(candidate);
        }
    }
    Ok(out)
}

/// Zed extension ids suggested for a suffix or file name (no network).
pub fn suggested_ids(suffix_or_name: &str) -> Vec<&'static str> {
    let wanted = suffix_or_name.trim_start_matches('.').to_ascii_lowercase();
    SUGGESTIONS
        .iter()
        .filter(|(_, suffixes)| {
            suffixes
                .iter()
                .any(|s| s.trim_start_matches('.').eq_ignore_ascii_case(&wanted))
        })
        .map(|(id, _)| *id)
        .collect()
}

/// `/extensions/<id>`: every published version; the newest this reader
/// takes.
pub fn latest(id: &str) -> Result<Option<Candidate>, ExtensionError> {
    let url = format!(
        "{BASE}/extensions/{}?max_schema_version={MAX_SCHEMA_VERSION}",
        encode(id)
    );
    let versions = parse_list(&crate::http::get_text(&url)?)?;
    Ok(versions
        .into_iter()
        .filter(|c| c.id == id)
        .max_by(|a, b| version_key(a.version.as_deref()).cmp(&version_key(b.version.as_deref()))))
}

fn version_key(version: Option<&str>) -> Vec<u64> {
    version
        .unwrap_or("0")
        .split(['.', '-', '+'])
        .map(|p| p.parse().unwrap_or(0))
        .collect()
}

/// The JSON of `/extensions` and `/extensions/<id>` (`{"data": [...]}`).
pub fn parse_list(text: &str) -> Result<Vec<Candidate>, ExtensionError> {
    let value = parse_json(text)?;
    let Some(Value::List(items)) = value.get("data") else {
        return Ok(Vec::new());
    };
    Ok(items
        .iter()
        .filter(|item| {
            item.get("schema_version")
                .and_then(|v| match v {
                    Value::Int(n) => Some(*n as u32),
                    _ => None,
                })
                .is_none_or(|v| v <= MAX_SCHEMA_VERSION)
        })
        .filter_map(candidate)
        .collect())
}

fn candidate(item: &Value) -> Option<Candidate> {
    let id = item.str_of("id")?;
    let version = item.str_of("version")?;
    let provides = item.strings_of("provides");
    let grammar = if provides.iter().any(|p| p == "grammars") {
        GrammarHint::TreeSitter
    } else if provides.iter().any(|p| p == "languages") {
        // queries over a grammar Zed ships
        GrammarHint::TreeSitter
    } else {
        GrammarHint::Unknown
    };
    Some(Candidate {
        registry: Registry::Zed,
        id: id.to_string(),
        name: id.to_string(),
        display_name: item.str_of("name").unwrap_or(id).to_string(),
        publisher: item
            .strings_of("authors")
            .first()
            .map(|a| a.split('<').next().unwrap_or(a).trim().to_string()),
        version: Some(version.to_string()),
        description: item.str_of("description").map(str::to_string),
        repository: item.str_of("repository").map(str::to_string),
        download_url: download_url(id, version),
        grammar,
        suffixes: Vec::new(),
        downloads: item
            .get("download_count")
            .and_then(|d| match d {
                Value::Int(n) => Some(*n as u64),
                _ => None,
            })
            .unwrap_or(0),
    })
}

/// `/extensions/<id>/<version>/download` (answers with a redirect to the
/// archive).
pub fn download_url(id: &str, version: &str) -> String {
    format!(
        "{BASE}/extensions/{}/{}/download",
        encode(id),
        encode(version)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_listing() {
        let text = r#"{"data":[{"id":"zig","name":"Zig","version":"0.4.2","description":"Zig support.",
            "authors":["Allan Calix <contact@acx.dev>"],"repository":"https://github.com/zed-extensions/zig",
            "schema_version":1,"wasm_api_version":"0.7.0","provides":["languages","grammars","language-servers"],
            "published_at":"2026-03-03T01:43:25Z","download_count":222269},
            {"id":"theme-x","name":"Theme","version":"1.0.0","provides":["themes"],"schema_version":1},
            {"id":"future","name":"F","version":"1.0.0","provides":["grammars"],"schema_version":2}]}"#;
        let found = parse_list(text).expect("parses");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].id, "zig");
        assert_eq!(found[0].publisher.as_deref(), Some("Allan Calix"));
        assert_eq!(found[0].grammar, GrammarHint::TreeSitter);
        assert_eq!(
            found[0].download_url,
            "https://api.zed.dev/extensions/zig/0.4.2/download"
        );
        assert_eq!(found[1].grammar, GrammarHint::Unknown);
        assert_eq!(super::super::extension_id(&found[0]), "zed.zig");
    }

    #[test]
    fn suggestions_cover_suffixes_and_names() {
        assert_eq!(suggested_ids("nix"), vec!["nix"]);
        assert!(suggested_ids("Dockerfile").contains(&"dockerfile"));
        assert!(suggested_ids(".gitignore").contains(&"git-firefly"));
        assert!(suggested_ids("nothing-like-this").is_empty());
        assert!(version_key(Some("0.10.1")) > version_key(Some("0.9.9")));
    }
}
