//! Open VSX (open-vsx.org), the open registry of VS Code extensions. Its
//! search indexes `__ext_<suffix>` tags from each extension's
//! `contributes.languages`, so a file suffix finds the extensions for it.

use super::{Candidate, GrammarHint, Registry, encode};
use crate::ExtensionError;
use crate::value::{Value, parse_json};

const BASE: &str = "https://open-vsx.org/api";

pub fn search(query: &str) -> Result<Vec<Candidate>, ExtensionError> {
    let url = format!(
        "{BASE}/-/search?query={}&size=20&sortBy=relevance&includeAllVersions=false",
        encode(query.trim())
    );
    let mut found = parse_search(&crate::http::get_text(&url)?)?;
    fill_suffixes(&mut found);
    Ok(found)
}

/// `117-file-icons`: Themes-category extensions tagged `icon-theme`
/// (VS Code tags every extension that contributes `iconThemes`).
pub fn search_icon_themes(query: &str) -> Result<Vec<Candidate>, ExtensionError> {
    let query = query.trim();
    let url = format!(
        "{BASE}/-/search?query={}&category=Themes&size=20&sortBy={}&includeAllVersions=false",
        encode(if query.is_empty() { "icon" } else { query }),
        if query.is_empty() {
            "downloadCount"
        } else {
            "relevance"
        },
    );
    let mut found = parse_search(&crate::http::get_text(&url)?)?;
    // the search answer has no tags: each detail has them
    fill_suffixes(&mut found);
    found.retain(|c| c.icon_themes);
    Ok(found)
}

/// The search answer carries no tags; each extension's detail does
/// (`__ext_<suffix>` from its `contributes.languages`). Fetched a few at a
/// time so the Find tab shows what every result covers.
fn fill_suffixes(found: &mut [Candidate]) {
    let ids: Vec<(usize, String)> = found
        .iter()
        .enumerate()
        .filter(|(_, c)| c.suffixes.is_empty())
        .map(|(i, c)| (i, c.id.clone()))
        .collect();
    let mut details: Vec<(usize, Vec<String>, bool)> = Vec::new();
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for chunk in ids.chunks(4) {
            handles.push(scope.spawn(move || {
                chunk
                    .iter()
                    .filter_map(|(i, id)| {
                        latest(id)
                            .ok()
                            .flatten()
                            .map(|c| (*i, c.suffixes, c.icon_themes))
                    })
                    .collect::<Vec<_>>()
            }));
        }
        for handle in handles {
            if let Ok(part) = handle.join() {
                details.extend(part);
            }
        }
    });
    for (i, suffixes, icon_themes) in details {
        if let Some(candidate) = found.get_mut(i) {
            candidate.suffixes = suffixes;
            candidate.icon_themes |= icon_themes;
        }
    }
}

pub fn for_suffix(suffix: &str) -> Result<Vec<Candidate>, ExtensionError> {
    let suffix = suffix.trim_start_matches('.').to_ascii_lowercase();
    let url = format!(
        "{BASE}/-/search?query=__ext_{}&size=20&sortBy=downloadCount",
        encode(&suffix)
    );
    let mut found = parse_search(&crate::http::get_text(&url)?)?;
    for candidate in &mut found {
        if !candidate.suffixes.contains(&suffix) {
            candidate.suffixes.push(suffix.clone());
        }
    }
    Ok(found)
}

/// The JSON of `/api/-/search`.
pub fn parse_search(text: &str) -> Result<Vec<Candidate>, ExtensionError> {
    let value = parse_json(text)?;
    let Some(Value::List(items)) = value.get("extensions") else {
        return Ok(Vec::new());
    };
    Ok(items.iter().filter_map(candidate).collect())
}

fn candidate(item: &Value) -> Option<Candidate> {
    let namespace = item.str_of("namespace")?;
    let name = item.str_of("name")?;
    let download = item.get("files").and_then(|f| f.str_of("download"))?;
    let tags = item.strings_of("tags");
    let suffixes: Vec<String> = tags
        .iter()
        .filter_map(|t| t.strip_prefix("__ext_"))
        .map(|s| s.to_ascii_lowercase())
        .collect();
    Some(Candidate {
        registry: Registry::OpenVsx,
        id: format!("{namespace}.{name}"),
        name: name.to_string(),
        display_name: item.str_of("displayName").unwrap_or(name).to_string(),
        publisher: Some(namespace.to_string()),
        version: item.str_of("version").map(str::to_string),
        description: item.str_of("description").map(str::to_string),
        repository: None,
        download_url: download.to_string(),
        grammar: GrammarHint::TextMate,
        // `117-file-icons`
        icon_themes: tags.iter().any(|t| t == "icon-theme"),
        suffixes,
        downloads: item
            .get("downloadCount")
            .and_then(|d| match d {
                Value::Int(n) => Some(*n as u64),
                Value::Float(f) => Some(*f as u64),
                _ => None,
            })
            .unwrap_or(0),
    })
}

/// `/api/<namespace>/<name>`: the latest version and its download.
pub fn latest(id: &str) -> Result<Option<Candidate>, ExtensionError> {
    let (namespace, name) = id
        .split_once('.')
        .ok_or_else(|| ExtensionError::Archive(format!("{id} is not an Open VSX id")))?;
    let url = format!("{BASE}/{}/{}", encode(namespace), encode(name));
    let value = parse_json(&crate::http::get_text(&url)?)?;
    Ok(candidate(&value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_search_answer() {
        let text = r#"{"offset":0,"totalSize":1,"extensions":[{"namespace":"ziglang","name":"vscode-zig",
            "version":"0.6.19","displayName":"Zig Language","description":"Language support","downloadCount":95033,
            "files":{"download":"https://open-vsx.org/api/ziglang/vscode-zig/0.6.19/file/ziglang.vscode-zig-0.6.19.vsix"},
            "tags":["__ext_zig","__ext_zon","zig"]}]}"#;
        let found = parse_search(text).expect("parses");
        assert_eq!(found.len(), 1);
        let c = &found[0];
        assert_eq!(c.id, "ziglang.vscode-zig");
        assert_eq!(c.display_name, "Zig Language");
        assert_eq!(c.suffixes, vec!["zig", "zon"]);
        assert_eq!(c.downloads, 95033);
        assert_eq!(super::super::extension_id(c), "openvsx.ziglang.vscode-zig");
        assert!(c.download_url.ends_with(".vsix"));
    }
}
