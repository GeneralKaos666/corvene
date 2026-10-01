//! The command line tool on Windows (GitHub Desktop's `app/src/cli/main.ts`).
//! `packaging/windows/corvane.bat` starts Corvane with `--cli` and its own
//! arguments; what the shell scripts do on macOS and Linux
//! (`packaging/corvane.sh`) happens here, a batch file being no place for
//! percent-encoding:
//!   corvane                            open the current directory
//!   corvane open [path]                open the provided path
//!   corvane clone [-b branch] <url>    clone the repository by url or
//!                                      owner/name, optionally checking out
//!                                      the branch
//! Each becomes an `x-corvane://` URL, which goes the way of the URLs on
//! the command line: to the running Corvane, or into this one.

use std::path::Path;

/// The arguments after `--cli`, when it is there.
fn cli_arguments(args: impl IntoIterator<Item = String>) -> Option<Vec<String>> {
    let mut args = args.into_iter();
    args.by_ref().find(|arg| arg == "--cli")?;
    Some(args.collect())
}

/// Percent-encode everything but unreserved characters and `/` (UTF-8 bytes).
fn url_encode(text: &str) -> String {
    let mut out = String::new();
    for &b in text.as_bytes() {
        if b.is_ascii_alphanumeric() || b"-._~/".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// A remote URL goes into the URL path as is, except what would end it.
fn encode_remote(url: &str) -> String {
    let mut out = String::new();
    for c in url.chars() {
        match c {
            '%' | '?' | '#' | ' ' => out.push_str(&format!("%{:02X}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// `clone [-b branch] <url>` → `x-corvane://openRepo/<url>[?branch=…]`
fn clone_url(args: &[String]) -> Result<String, String> {
    let usage = || "usage: corvane clone [-b branch] <url>".to_string();
    let (mut url, mut branch) = (None, None);
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-b" | "--branch" => branch = Some(args.next().ok_or_else(usage)?.clone()),
            other => match other.strip_prefix("--branch=") {
                Some(name) => branch = Some(name.to_string()),
                None if url.is_none() => url = Some(other.to_string()),
                None => return Err(usage()),
            },
        }
    }
    let mut url = url.ok_or_else(usage)?;
    // assume name with owner slug if it looks like it
    if url.split('/').count() == 2 && url.split('/').all(|part| !part.is_empty()) {
        url = format!("https://github.com/{url}");
    }
    let mut target = format!("x-corvane://openRepo/{}", encode_remote(&url));
    if let Some(branch) = branch {
        target.push_str(&format!("?branch={}", url_encode(&branch)));
    }
    Ok(target)
}

/// `[open] [path]` → `x-corvane://openLocalRepo/<path>`
fn open_url(args: &[String], cwd: &Path) -> Result<String, String> {
    let path = args.first().map_or(".", String::as_str);
    let dir = dunce::canonicalize(cwd.join(path))
        .ok()
        .filter(|dir| dir.is_dir())
        .ok_or_else(|| format!("corvane: {path}: no such directory"))?;
    Ok(corvane_core::app_url::open_local_repo_url(&dir))
}

fn url_for(args: &[String], cwd: &Path) -> Result<String, String> {
    match args.first().map(String::as_str) {
        Some("clone") => clone_url(&args[1..]),
        Some("open") => open_url(&args[1..], cwd),
        _ => open_url(args, cwd),
    }
}

/// The URL the command line asks for (`None` without `--cli`, or when the
/// request makes no sense, which is logged: a program without a console
/// has nowhere to print to).
pub fn url(args: impl IntoIterator<Item = String>) -> Option<String> {
    let args = cli_arguments(args)?;
    let cwd = std::env::current_dir().unwrap_or_default();
    url_for(&args, &cwd)
        .map_err(|message| tracing::warn!("{message}"))
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|arg| arg.to_string()).collect()
    }

    #[test]
    fn only_after_the_flag() {
        assert_eq!(cli_arguments(args(&["corvane.exe", "open", "."])), None);
        assert_eq!(
            cli_arguments(args(&["corvane.exe", "--cli", "open", "."])),
            Some(args(&["open", "."]))
        );
        assert_eq!(cli_arguments(args(&["corvane.exe", "--cli"])), Some(vec![]));
    }

    #[test]
    fn clone_urls() {
        assert_eq!(
            clone_url(&args(&["torvalds/linux"])).unwrap(),
            "x-corvane://openRepo/https://github.com/torvalds/linux"
        );
        assert_eq!(
            clone_url(&args(&["-b", "my branch", "https://example.com/a/b.git"])).unwrap(),
            "x-corvane://openRepo/https://example.com/a/b.git?branch=my%20branch"
        );
        assert_eq!(
            clone_url(&args(&["--branch=dev", "ssh://git@example.com/a/b.git"])).unwrap(),
            "x-corvane://openRepo/ssh://git@example.com/a/b.git?branch=dev"
        );
        assert!(clone_url(&args(&[])).is_err());
        assert!(clone_url(&args(&["a/b", "c/d"])).is_err());
    }

    #[test]
    fn open_urls_name_an_existing_folder() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("my repo")).unwrap();
        let url = url_for(&args(&["open", "my repo"]), dir.path()).unwrap();
        assert!(url.starts_with("x-corvane://openLocalRepo/"), "{url}");
        assert!(url.ends_with("my%20repo"), "{url}");
        // without `open`, and the folder itself
        assert_eq!(url_for(&args(&["my repo"]), dir.path()).unwrap(), url);
        assert!(url_for(&args(&[]), dir.path()).is_ok());
        assert!(url_for(&args(&["missing"]), dir.path()).is_err());
    }
}
