//! The offline extension index: one JSON file, built by `tools/ext-index`
//! from the three registries and published with the packs, mapping file
//! suffixes and names to the extensions that cover them. The diff's "no
//! syntax highlighting" hint answers from it at once; the registries are
//! asked only when it has nothing.

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::ExtensionError;
use crate::registry::Candidate;

/// The schema this build reads.
pub const SCHEMA: u32 = 1;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Index {
    pub schema: u32,
    /// RFC 3339
    #[serde(default)]
    pub generated: String,
    /// suffix (no dot, lowercase) → candidates, best first
    #[serde(default)]
    pub suffixes: HashMap<String, Vec<Candidate>>,
    /// whole file name (lowercase) → candidates
    #[serde(default)]
    pub filenames: HashMap<String, Vec<Candidate>>,
}

impl Index {
    /// Read `index.json` or `index.json.gz`.
    pub fn load(path: &Path) -> Result<Self, ExtensionError> {
        let bytes = std::fs::read(path)?;
        let text = if bytes.starts_with(&[0x1f, 0x8b]) {
            let mut out = String::new();
            flate2::read::GzDecoder::new(&bytes[..])
                .take(64 * 1024 * 1024)
                .read_to_string(&mut out)?;
            out
        } else {
            String::from_utf8(bytes)
                .map_err(|_| ExtensionError::parse("json", "the index is not UTF-8"))?
        };
        let index: Index = serde_json::from_str(&text)
            .map_err(|err| ExtensionError::parse("json", format!("the extension index: {err}")))?;
        if index.schema > SCHEMA {
            return Err(ExtensionError::parse(
                "json",
                "the extension index was written for a newer Corvene",
            ));
        }
        Ok(index)
    }

    /// Candidates for a file name or a suffix: the whole name first, then
    /// its suffixes from the longest (`d.ts` before `ts`).
    pub fn lookup(&self, name_or_suffix: &str) -> Vec<Candidate> {
        let name = name_or_suffix
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(name_or_suffix)
            .trim_start_matches('.')
            .to_ascii_lowercase();
        let mut out: Vec<Candidate> = Vec::new();
        let mut push = |list: &[Candidate]| {
            for c in list {
                if !out.iter().any(|o| o.registry == c.registry && o.id == c.id) {
                    out.push(c.clone());
                }
            }
        };
        if let Some(list) = self.filenames.get(&name) {
            push(list);
        }
        let parts: Vec<&str> = name.split('.').collect();
        for start in 0..parts.len() {
            let suffix = parts[start..].join(".");
            if let Some(list) = self.suffixes.get(&suffix) {
                push(list);
            }
        }
        if parts.len() == 1
            && let Some(list) = self.suffixes.get(&name)
        {
            push(list);
        }
        out
    }

    /// Days since `generated`, when it parses (date part only).
    pub fn age_days(&self, now_unix: u64) -> Option<u64> {
        let date = self.generated.get(..10)?;
        let mut it = date.split('-');
        let (y, m, d): (i64, i64, i64) = (
            it.next()?.parse().ok()?,
            it.next()?.parse().ok()?,
            it.next()?.parse().ok()?,
        );
        // days from civil (Howard Hinnant)
        let (y, m) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
        let era = if y >= 0 { y } else { y - 399 } / 400;
        let yoe = y - era * 400;
        let doy = (153 * m + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        let days = era * 146_097 + doe - 719_468;
        let now_days = (now_unix / 86_400) as i64;
        Some((now_days - days).max(0) as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{GrammarHint, Registry};

    fn candidate(registry: Registry, id: &str) -> Candidate {
        Candidate {
            registry,
            id: id.into(),
            name: id.into(),
            display_name: id.into(),
            publisher: None,
            version: Some("1".into()),
            description: None,
            repository: None,
            download_url: "https://example.invalid".into(),
            grammar: GrammarHint::TextMate,
            suffixes: Vec::new(),
            downloads: 0,
            icon_themes: false,
        }
    }

    #[test]
    fn looks_up_names_then_suffixes() {
        let mut index = Index {
            schema: SCHEMA,
            generated: "2026-10-01T00:00:00Z".into(),
            ..Default::default()
        };
        index
            .suffixes
            .insert("ts".into(), vec![candidate(Registry::OpenVsx, "a.ts")]);
        index
            .suffixes
            .insert("d.ts".into(), vec![candidate(Registry::Zed, "dts")]);
        index
            .filenames
            .insert("justfile".into(), vec![candidate(Registry::Zed, "just")]);
        let ids: Vec<String> = index
            .lookup("src/types.d.ts")
            .into_iter()
            .map(|c| c.id)
            .collect();
        assert_eq!(ids, vec!["dts", "a.ts"]);
        let ids: Vec<String> = index.lookup("Justfile").into_iter().map(|c| c.id).collect();
        assert_eq!(ids, vec!["just"]);
        assert!(index.lookup(".nothing").is_empty());
        assert_eq!(index.age_days(1_791_417_600), Some(7)); // 2026-10-08 - 2026-10-01
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("index.json");
        std::fs::write(&path, serde_json::to_string(&index).expect("json")).expect("write");
        assert_eq!(Index::load(&path).expect("load").suffixes.len(), 2);
    }
}
