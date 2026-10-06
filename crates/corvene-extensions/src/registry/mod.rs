//! The open extension registries: Open VSX (VS Code extensions), Zed's and
//! Pulsar's. Each answers a text search and a lookup by file suffix with
//! [`Candidate`]s that the install flow downloads like any archive.
//! Microsoft's marketplace is not queried: its terms allow VS Code alone.

pub mod openvsx;
pub mod pulsar;
pub mod zed;
pub mod zed_suggestions;

use serde::{Deserialize, Serialize};

/// Which registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Registry {
    OpenVsx,
    Zed,
    Pulsar,
}

impl Registry {
    pub const ALL: [Registry; 3] = [Registry::OpenVsx, Registry::Zed, Registry::Pulsar];

    pub fn title(self) -> &'static str {
        match self {
            Registry::OpenVsx => "Open VSX",
            Registry::Zed => "Zed",
            Registry::Pulsar => "Pulsar",
        }
    }

    pub fn source_kind(self) -> crate::install::SourceKind {
        match self {
            Registry::OpenVsx => crate::install::SourceKind::OpenVsx,
            Registry::Zed => crate::install::SourceKind::Zed,
            Registry::Pulsar => crate::install::SourceKind::Pulsar,
        }
    }

    /// Search `query` (free text) on this registry.
    pub fn search(self, query: &str) -> Result<Vec<Candidate>, crate::ExtensionError> {
        match self {
            Registry::OpenVsx => openvsx::search(query),
            Registry::Zed => zed::search(query),
            Registry::Pulsar => pulsar::search(query),
        }
    }

    /// `117-file-icons`: extensions with file icon themes matching `query`
    /// (Pulsar has none it could read).
    pub fn search_icon_themes(
        self,
        query: &str,
    ) -> Result<Vec<Candidate>, crate::ExtensionError> {
        match self {
            Registry::OpenVsx => openvsx::search_icon_themes(query),
            Registry::Zed => zed::search_icon_themes(query),
            Registry::Pulsar => Ok(Vec::new()),
        }
    }

    /// Extensions for files with `suffix` (no dot).
    pub fn for_suffix(self, suffix: &str) -> Result<Vec<Candidate>, crate::ExtensionError> {
        match self {
            Registry::OpenVsx => openvsx::for_suffix(suffix),
            Registry::Zed => zed::for_suffix(suffix),
            Registry::Pulsar => pulsar::for_suffix(suffix),
        }
    }
}

/// What kind of grammar an extension is known to carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GrammarHint {
    TextMate,
    TreeSitter,
    Unknown,
}

/// An extension a registry offers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub registry: Registry,
    /// the registry's own id (`namespace.name`, Zed's id, a Pulsar name)
    pub id: String,
    pub name: String,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    pub download_url: String,
    #[serde(default = "unknown")]
    pub grammar: GrammarHint,
    /// file suffixes the registry says it covers (often none)
    #[serde(default)]
    pub suffixes: Vec<String>,
    #[serde(default)]
    pub downloads: u64,
    /// `117-file-icons`: it contributes file icon themes.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub icon_themes: bool,
}

fn unknown() -> GrammarHint {
    GrammarHint::Unknown
}

/// The install id for a candidate.
pub fn extension_id(candidate: &Candidate) -> String {
    let (namespace, name) = match candidate.registry {
        Registry::OpenVsx => match candidate.id.split_once('.') {
            Some((ns, n)) => (Some(ns), n),
            None => (None, candidate.id.as_str()),
        },
        Registry::Zed | Registry::Pulsar => (None, candidate.id.as_str()),
    };
    crate::install::extension_id(candidate.registry.source_kind(), namespace, name)
}

/// Percent-encode a query value.
pub(crate) fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}
