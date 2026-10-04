//! Shells on Android: Termux, opened in the repository through its
//! `RUN_COMMAND` intent (GHD has no Android build).
//!
//! Termux runs under its own user id, so it reaches a repository only on
//! shared storage (the `foss` flavour with "All files access"), never one in
//! Corvene's private storage. The user has to allow the intent once:
//! `allow-external-apps = true` in `~/.termux/termux.properties`, and the
//! "Run commands in Termux environment" permission, which Android asks for
//! on first use.

use std::path::Path;

use super::FoundShell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shell {
    Termux,
}

pub const DEFAULT_SHELL: Shell = Shell::Termux;

impl Shell {
    pub fn label(self) -> &'static str {
        "Termux"
    }

    pub fn parse(_label: &str) -> Shell {
        DEFAULT_SHELL
    }
}

/// Termux when it is installed.
pub fn available_shells() -> Vec<FoundShell> {
    let installed = crate::android::bridge()
        .is_some_and(|bridge| bridge.package_installed(crate::android::TERMUX_PACKAGE));
    if !installed {
        return Vec::new();
    }
    vec![FoundShell {
        shell: Shell::Termux,
        bundle_id: crate::android::TERMUX_PACKAGE.to_string(),
        path: "/data/data/com.termux/files/usr/bin/login".into(),
    }]
}

pub fn launch(_found: &FoundShell, path: &Path) -> std::io::Result<()> {
    if !crate::android::is_shared_storage(path) {
        return Err(std::io::Error::other(
            crate::android::TERMUX_PRIVATE_STORAGE,
        ));
    }
    let bridge = crate::android::bridge().ok_or_else(|| std::io::Error::other("no activity"))?;
    bridge.open_termux(path).map_err(std::io::Error::other)
}
