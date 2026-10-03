//! Importing git settings from another git on the device (Android: Termux's,
//! which Corvene can neither run nor read: every application's storage is
//! closed to the others). The other side sends `git config --global --list`
//! through the `x-corvene://importGitConfig/<base64url>` link; the settings
//! Corvene's bundled git can use go into `~/.gitconfig-imported`, which
//! `~/.gitconfig` includes first, so what is set in Corvene still wins.
//!
//! Only settings that describe how git behaves are taken. What names a
//! program or a file of the other environment (`core.sshCommand`,
//! `credential.helper`, `gpg.program`, `core.hooksPath`, aliases that run a
//! shell) would fail here or run something the user did not expect, and
//! signing is left out because there is no gpg to sign with.
//!
//! GHD has no equivalent: it uses the system's git and its configuration.

/// The file `~/.gitconfig` includes.
pub const IMPORTED_FILE: &str = ".gitconfig-imported";

/// Decodes the link's payload: base64url, padding optional.
pub fn decode_payload(payload: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(payload.len() * 3 / 4);
    let (mut buffer, mut bits) = (0u32, 0u32);
    for c in payload.bytes() {
        let value = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            b'=' => continue,
            _ => return None,
        };
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            bytes.push((buffer >> bits) as u8);
        }
    }
    String::from_utf8(bytes).ok()
}

/// Whether `key` (as `git config --list` prints it: section and name in
/// lower case) is a setting Corvene's git may take over.
fn accepted(key: &str, value: &str) -> bool {
    let Some((section, rest)) = key.split_once('.') else {
        return false;
    };
    let name = rest.rsplit('.').next().unwrap_or(rest);
    // nothing that points into the other application's storage
    if value.contains("/data/data/") || value.contains("/data/user/") {
        return false;
    }
    match section {
        "user" => matches!(name, "name" | "email"),
        "init" => name == "defaultbranch",
        "pull" | "push" | "fetch" | "merge" | "rebase" | "diff" | "status" | "tag" | "log"
        | "branch" | "apply" | "stash" | "rerere" => {
            // tools and drivers are programs of the other environment
            !matches!(
                name,
                "tool" | "guitool" | "external" | "driver" | "textconv" | "command"
            ) && !key.contains("tool.")
        }
        "core" => matches!(
            name,
            "autocrlf"
                | "eol"
                | "safecrlf"
                | "quotepath"
                | "precomposeunicode"
                | "whitespace"
                | "abbrev"
                | "commentchar"
                | "longpaths"
        ),
        "url" => matches!(name, "insteadof" | "pushinsteadof"),
        // an alias that starts with `!` runs a shell command
        "alias" => !value.trim_start().starts_with('!'),
        "commit" => matches!(name, "verbose" | "cleanup" | "status"),
        "help" => name == "autocorrect",
        _ => false,
    }
}

/// The settings of a `git config --list` output Corvene takes, and how many
/// lines it leaves out.
pub fn parse(list: &str) -> (Vec<(String, String)>, usize) {
    let mut taken = Vec::new();
    let mut skipped = 0;
    for line in list.lines().filter(|line| !line.trim().is_empty()) {
        match line.split_once('=') {
            Some((key, value)) if accepted(key, value) => {
                taken.push((key.to_string(), value.to_string()));
            }
            _ => skipped += 1,
        }
    }
    (taken, skipped)
}

/// `settings` as a git configuration file.
pub fn render(settings: &[(String, String)]) -> String {
    let mut out = String::from("# Imported by Corvene; importing again replaces this file.\n");
    for (key, value) in settings {
        let Some((section, rest)) = key.split_once('.') else {
            continue;
        };
        let (subsection, name) = match rest.rsplit_once('.') {
            Some((subsection, name)) => (Some(subsection), name),
            None => (None, rest),
        };
        let quote = |text: &str| text.replace('\\', "\\\\").replace('"', "\\\"");
        match subsection {
            Some(subsection) => {
                out.push_str(&format!("[{section} \"{}\"]\n", quote(subsection)));
            }
            None => out.push_str(&format!("[{section}]\n")),
        }
        out.push_str(&format!(
            "\t{name} = \"{}\"\n",
            quote(value).replace('\n', "\\n").replace('\t', "\\t")
        ));
    }
    out
}

/// `gitconfig` with the include of [`IMPORTED_FILE`] in front (once).
pub fn with_include(gitconfig: &str) -> String {
    let include = format!("[include]\n\tpath = ~/{IMPORTED_FILE}\n");
    if gitconfig.contains(&format!("path = ~/{IMPORTED_FILE}")) {
        gitconfig.to_string()
    } else {
        format!("{include}{gitconfig}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_base64url_with_and_without_padding() {
        assert_eq!(
            decode_payload("dXNlci5uYW1lPUE").as_deref(),
            Some("user.name=A")
        );
        assert_eq!(
            decode_payload("dXNlci5uYW1lPUE=").as_deref(),
            Some("user.name=A")
        );
        assert_eq!(decode_payload("not base64!"), None);
    }

    #[test]
    fn takes_behaviour_and_leaves_programs_out() {
        let (taken, skipped) = parse(
            "user.name=Mona Lisa\nuser.email=mona@example.com\nuser.signingkey=ABC\n\
             commit.gpgsign=true\ncore.sshcommand=ssh -i ~/.ssh/other\n\
             credential.helper=store\npull.rebase=true\nalias.co=checkout\n\
             alias.pwn=!rm -rf ~\nurl.git@github.com:.insteadof=https://github.com/\n\
             core.hookspath=/data/data/com.termux/files/home/hooks\ndiff.tool=vimdiff\n\
             merge.conflictstyle=zdiff3\ncore.autocrlf=input\n",
        );
        let keys: Vec<&str> = taken.iter().map(|(key, _)| key.as_str()).collect();
        assert_eq!(
            keys,
            [
                "user.name",
                "user.email",
                "pull.rebase",
                "alias.co",
                "url.git@github.com:.insteadof",
                "merge.conflictstyle",
                "core.autocrlf"
            ]
        );
        assert_eq!(skipped, 7);
    }

    #[test]
    fn renders_sections_and_subsections() {
        let file = render(&[
            ("user.name".into(), "Mona \"M\" Lisa".into()),
            (
                "url.git@github.com:.insteadof".into(),
                "https://github.com/".into(),
            ),
        ]);
        assert!(file.contains("[user]\n\tname = \"Mona \\\"M\\\" Lisa\"\n"));
        assert!(
            file.contains("[url \"git@github.com:\"]\n\tinsteadof = \"https://github.com/\"\n")
        );
    }

    #[test]
    fn includes_once_and_first() {
        let once = with_include("[user]\n\tname = A\n");
        assert!(once.starts_with("[include]\n\tpath = ~/.gitconfig-imported\n[user]"));
        assert_eq!(with_include(&once), once);
    }

    #[test]
    fn git_reads_the_rendered_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config");
        std::fs::write(
            &file,
            render(&[
                ("user.name".into(), "Mona \"M\" Lisa".into()),
                (
                    "url.git@github.com:.insteadof".into(),
                    "https://github.com/".into(),
                ),
                ("alias.lg".into(), "log --graph --format='%h %s'".into()),
            ]),
        )
        .unwrap();
        let get = |key: &str| {
            let output = std::process::Command::new("git")
                .args(["config", "--file"])
                .arg(&file)
                .args(["--get", key])
                .output()
                .unwrap();
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        };
        assert_eq!(get("user.name"), "Mona \"M\" Lisa");
        assert_eq!(get("url.git@github.com:.insteadof"), "https://github.com/");
        assert_eq!(get("alias.lg"), "log --graph --format='%h %s'");
    }
}
