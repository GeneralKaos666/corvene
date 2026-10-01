//! Shell detection and launching: GHD `lib/shells/darwin.ts` on macOS,
//! `lib/shells/linux.ts` on Linux.

use std::path::PathBuf;

#[cfg(target_os = "macos")]
mod darwin;
#[cfg(target_os = "macos")]
pub use darwin::*;
#[cfg(not(any(target_os = "macos", target_os = "android")))]
mod linux;
#[cfg(not(any(target_os = "macos", target_os = "android")))]
pub use linux::*;
#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "android")]
pub use android::*;

/// GHD `FoundShell`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoundShell {
    pub shell: Shell,
    /// macOS: the bundle identifier that matched (empty on Linux).
    pub bundle_id: String,
    /// The `.app` bundle, or the executable for shells launched directly.
    pub path: PathBuf,
}
