//! Corvene (`1105-clean-untracked-files`): `git clean`, previewed with a
//! dry run. GHD has no such command; it discards untracked files one at a
//! time from the changes list (to the Trash) and never touches ignored ones.

use std::path::Path;
use std::sync::Arc;

use crate::detect::GitBinary;
use crate::error::Result;
use crate::process::GitCommand;

/// The paths `git clean -f -d [-x]` would remove, from `git clean -n -d
/// [-x]`: files and, for an untracked folder, the folder itself (with a
/// trailing `/`). Paths are relative to `workdir`. Nested repositories
/// (`Would skip repository …`) are left out: a plain `-f` keeps them.
pub fn clean_dry_run(
    git: Arc<GitBinary>,
    workdir: &Path,
    include_ignored: bool,
) -> Result<Vec<String>> {
    let mut args = vec!["-c", "core.quotePath=false", "clean", "-n", "-d"];
    if include_ignored {
        args.push("-x");
    }
    let out = GitCommand::new(git).args(args).current_dir(workdir).run()?;
    Ok(parse_clean_dry_run(&String::from_utf8_lossy(&out.stdout)))
}

/// `git clean -n` lines: `Would remove <path>`, the path C-quoted when it
/// holds a quote, a backslash or a control character.
pub fn parse_clean_dry_run(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter_map(|line| line.strip_prefix("Would remove "))
        .map(unquote_c_path)
        .filter(|p| !p.is_empty())
        .collect()
}

/// git's `quote_c_style` undone: `"a\tb\"c"` → `a<TAB>b"c`; octal escapes
/// are bytes of the UTF-8 path. A path without quotes is returned as is.
pub fn unquote_c_path(text: &str) -> String {
    let Some(inner) = text
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
    else {
        return text.to_string();
    };
    let bytes = inner.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b != b'\\' || i + 1 == bytes.len() {
            out.push(b);
            i += 1;
            continue;
        }
        let next = bytes[i + 1];
        let simple = match next {
            b'a' => Some(0x07),
            b'b' => Some(0x08),
            b'f' => Some(0x0c),
            b'n' => Some(b'\n'),
            b'r' => Some(b'\r'),
            b't' => Some(b'\t'),
            b'v' => Some(0x0b),
            b'\\' => Some(b'\\'),
            b'"' => Some(b'"'),
            _ => None,
        };
        if let Some(byte) = simple {
            out.push(byte);
            i += 2;
            continue;
        }
        let octal = &bytes[i + 1..bytes.len().min(i + 4)];
        if octal.len() == 3 && octal.iter().all(|d| (b'0'..=b'7').contains(d)) {
            let value = octal
                .iter()
                .fold(0u32, |acc, d| acc * 8 + u32::from(d - b'0'));
            out.push(u8::try_from(value).unwrap_or(b'?'));
            i += 4;
        } else {
            out.push(b);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `git clean -f -d [-x] -- <paths>` for paths from [`clean_dry_run`]
/// (taken literally). Removes them for good: nothing goes to the Trash.
pub fn clean_paths(
    git: Arc<GitBinary>,
    workdir: &Path,
    paths: &[String],
    include_ignored: bool,
) -> Result<()> {
    // well below any command line limit
    for chunk in paths.chunks(500) {
        let mut args = vec!["clean", "-f", "-d", "-q"];
        if include_ignored {
            args.push("-x");
        }
        args.push("--");
        args.extend(chunk.iter().map(String::as_str));
        GitCommand::new(git.clone())
            .args(args)
            .env("GIT_LITERAL_PATHSPECS", "1")
            .current_dir(workdir)
            .run()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_dry_run_lines() {
        let out = "Would remove notes.txt\nWould remove build/\nWould skip repository vendored/\n\
                   Would remove \"tab\\there \\\"q\\\".txt\"\nWould remove \"caf\\303\\251.md\"\n";
        assert_eq!(
            parse_clean_dry_run(out),
            vec![
                "notes.txt".to_string(),
                "build/".into(),
                "tab\there \"q\".txt".into(),
                "café.md".into(),
            ]
        );
    }

    #[test]
    fn unquoted_paths_pass_through() {
        assert_eq!(unquote_c_path("plain name.txt"), "plain name.txt");
        assert_eq!(unquote_c_path("\"back\\\\slash\""), "back\\slash");
    }
}
