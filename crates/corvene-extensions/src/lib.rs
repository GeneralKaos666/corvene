//! Language extensions: grammars users add for diff highlighting, taken
//! from the formats editors ship them in (Corvene addition; GitHub Desktop
//! highlights a fixed set of CodeMirror modes).
//!
//! This crate only prepares data: it reads the extension formats, converts
//! TextMate grammars to the sublime-syntax form syntect loads
//! ([`tm`]), and builds the user grammar set ([`cache`]). Loading the result
//! into the highlighter is `corvene_highlight::user`; downloading, the UI
//! and the state machine live in `corvene_core::extensions`.

pub mod archive;
pub mod cache;
pub mod cson;
pub mod install;
pub mod manifest;
pub mod plist;
pub mod scan;
pub mod tm;
pub mod value;

use thiserror::Error;

/// A grammar or manifest file larger than this is refused (untrusted input).
pub const MAX_GRAMMAR_BYTES: usize = 8 * 1024 * 1024;

/// Everything that can go wrong preparing an extension.
#[derive(Debug, Error)]
pub enum ExtensionError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    /// A grammar or manifest file that does not parse: the format it was
    /// read as and the reason.
    #[error("{format}: {message}")]
    Parse {
        format: &'static str,
        message: String,
    },
    /// A TextMate grammar the converter cannot represent (no `scopeName`,
    /// no patterns).
    #[error("{0}")]
    Convert(String),
    /// A converted grammar syntect rejects (a regex Oniguruma cannot
    /// compile, an invalid scope).
    #[error("{0}")]
    Compile(String),
    /// An archive that breaks the extraction policy (a path outside the
    /// target, a symlink, too many or too large entries) or is not one.
    #[error("{0}")]
    Archive(String),
    /// A folder or archive with nothing Corvene can use as a grammar.
    #[error("{0}")]
    NotAnExtension(String),
}

impl ExtensionError {
    pub(crate) fn parse(format: &'static str, message: impl Into<String>) -> Self {
        Self::Parse {
            format,
            message: message.into(),
        }
    }
}
