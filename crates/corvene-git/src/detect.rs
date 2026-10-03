//! Locate a usable `git` binary. GitHub Desktop bundles git; Corvene uses the
//! system one and shows the InstallGit dialog when it is missing.

use std::path::{Path, PathBuf};
use std::process::Command;

use tracing::{debug, info};

use crate::error::{GitError, Result};

/// Oldest git Corvene supports (`--porcelain=v2`, `--force-with-lease`, sparse index fixes).
pub const MIN_VERSION: GitVersion = GitVersion {
    major: 2,
    minor: 40,
    patch: 0,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct GitVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl std::fmt::Display for GitVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl GitVersion {
    /// Parse `git version 2.54.0` / `git version 2.39.5 (Apple Git-154)`.
    pub fn parse(output: &str) -> Option<Self> {
        let rest = output.trim().strip_prefix("git version ")?;
        let token = rest.split_whitespace().next()?;
        let mut parts = token.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts
            .next()
            .and_then(|p| {
                p.chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .parse()
                    .ok()
            })
            .unwrap_or(0);
        Some(Self {
            major,
            minor,
            patch,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitBinary {
    pub path: PathBuf,
    pub version: GitVersion,
}

/// [`find_git`] started on a thread at launch ([`prefetch_git`]).
static PREFETCHED: std::sync::Mutex<Option<std::thread::JoinHandle<Result<GitBinary>>>> =
    std::sync::Mutex::new(None);

/// Start [`find_git`] on a thread (first thing at launch), so its `git
/// --version` probes overlap the app's own start-up.
pub fn prefetch_git() {
    if let Ok(mut slot) = PREFETCHED.lock() {
        *slot = Some(std::thread::spawn(find_git));
    }
}

/// The result of [`prefetch_git`], or [`find_git`] when none was started
/// (or it was already taken: a later detection probes again).
pub fn find_git_prefetched() -> Result<GitBinary> {
    let handle = PREFETCHED.lock().ok().and_then(|mut slot| slot.take());
    match handle.map(|h| h.join()) {
        Some(Ok(result)) => result,
        _ => find_git(),
    }
}

/// Find git: `$CORVENE_GIT`, then `$PATH`, then well-known locations.
/// `/usr/bin/git` is only tried when the Xcode Command Line Tools are present,
/// because Apple's shim otherwise pops an install dialog.
pub fn find_git() -> Result<GitBinary> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(p) = std::env::var("CORVENE_GIT") {
        candidates.push(PathBuf::from(p));
    }
    #[cfg(windows)]
    candidates.extend(windows_candidates());
    #[cfg(not(windows))]
    {
        if let Some(path) = std::env::var_os("PATH") {
            for dir in std::env::split_paths(&path) {
                let candidate = dir.join("git");
                if candidate == Path::new("/usr/bin/git") && !command_line_tools_present() {
                    continue;
                }
                candidates.push(candidate);
            }
        }
        // Homebrew's prefixes (the last one is Linux's): a desktop session
        // does not have them on `PATH`
        for p in [
            "/opt/homebrew/bin/git",
            "/usr/local/bin/git",
            "/home/linuxbrew/.linuxbrew/bin/git",
        ] {
            candidates.push(PathBuf::from(p));
        }
        if command_line_tools_present() {
            candidates.push(PathBuf::from("/usr/bin/git"));
        }
    }

    let mut too_old: Option<GitVersion> = None;
    for candidate in candidates {
        if !candidate.is_file() {
            continue;
        }
        match probe(&candidate) {
            Some(version) if version >= MIN_VERSION => {
                info!(path = %candidate.display(), %version, "using git");
                return Ok(GitBinary {
                    path: candidate,
                    version,
                });
            }
            Some(version) => {
                debug!(path = %candidate.display(), %version, "git too old, skipping");
                too_old = too_old.max(Some(version));
            }
            None => debug!(path = %candidate.display(), "could not probe git"),
        }
    }
    match too_old {
        Some(found) => Err(GitError::GitTooOld {
            found: found.to_string(),
            required: MIN_VERSION.to_string(),
        }),
        None => Err(GitError::GitNotFound),
    }
}

/// macOS: `/usr/bin/git` is a stub that asks to install the Command Line
/// Tools unless they (or Xcode) are there. Linux has no such stub: a
/// distribution's `/usr/bin/git` is the real thing.
#[cfg(target_os = "macos")]
fn command_line_tools_present() -> bool {
    Path::new("/Library/Developer/CommandLineTools/usr/bin/git").exists()
        || Path::new("/Applications/Xcode.app/Contents/Developer/usr/bin/git").exists()
}

#[cfg(not(any(target_os = "macos", windows)))]
fn command_line_tools_present() -> bool {
    true
}

/// Windows: `git.exe` on `%PATH%`, then the MinGit Corvene's installer
/// offers when it finds none (`<app>\git\cmd\git.exe`,
/// `packaging/windows/corvene.iss`), then where Git for Windows installs
/// (for all users, for one user), whose installer only puts `cmd` on the
/// path when asked to.
#[cfg(windows)]
fn windows_candidates() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    if let Some(path) = std::env::var_os("PATH") {
        out.extend(std::env::split_paths(&path).map(|dir| dir.join("git.exe")));
    }
    if let Some(app) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    {
        out.push(app.join("git").join("cmd").join("git.exe"));
    }
    let roots = [
        std::env::var_os("ProgramFiles").map(PathBuf::from),
        std::env::var_os("ProgramW6432").map(PathBuf::from),
        std::env::var_os("LOCALAPPDATA").map(|dir| PathBuf::from(dir).join("Programs")),
    ];
    out.extend(
        roots
            .into_iter()
            .flatten()
            .map(|root| root.join("Git").join("cmd").join("git.exe")),
    );
    out
}

fn probe(path: &Path) -> Option<GitVersion> {
    let mut command = Command::new(path);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(crate::CREATE_NO_WINDOW);
    }
    let output = command
        .arg("--version")
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    GitVersion::parse(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_versions() {
        let v = GitVersion::parse("git version 2.54.0\n").unwrap();
        assert_eq!(
            v,
            GitVersion {
                major: 2,
                minor: 54,
                patch: 0
            }
        );
        let apple = GitVersion::parse("git version 2.39.5 (Apple Git-154)").unwrap();
        assert_eq!(apple.minor, 39);
        assert!(apple < MIN_VERSION);
        assert!(GitVersion::parse("nope").is_none());
        let rc = GitVersion::parse("git version 2.50.0-rc1").unwrap();
        assert_eq!(rc.patch, 0);
    }

    #[test]
    fn finds_system_git() {
        let git = find_git().expect("git on this machine");
        assert!(git.version >= MIN_VERSION);
    }
}
