//! git's credential helper protocol: GHD `parseCredential` /
//! `formatCredential` (`lib/git/credential.ts`).
//!
//! GHD speaks it in its credential-helper trampoline and for `git credential
//! fill / approve / reject` (Git Credential Manager). Corvene answers git
//! through `GIT_ASKPASS` instead (`crate::AskpassEnv`,
//! `crates/corvene/src/askpass.rs`) and hands non-GitHub hosts to the
//! credential manager with `-c credential.helper=manager`, so neither GHD
//! caller has a Corvene counterpart; these are the protocol's two halves for
//! whoever needs them.
//!
//! A credential is an ordered list of `key=value` entries (GHD's
//! `Map<string, string>`, which keeps insertion order). Array keys
//! (`wwwauth[]=…` lines) are numbered `wwwauth[0]`, `wwwauth[1]`, … while
//! parsing and written back as `wwwauth[]` lines.

/// A value [`format_credential`] cannot write.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CredentialError {
    /// A value with a newline or a NUL, which the line protocol cannot carry.
    #[error("forbidden characters in credential value: {0}")]
    ForbiddenCharacters(String),
}

/// `Map.set`: replaces the value of an existing key in place, else appends.
fn set(credential: &mut Vec<(String, String)>, key: String, value: String) {
    match credential.iter_mut().find(|(k, _)| *k == key) {
        Some(entry) => entry.1 = value,
        None => credential.push((key, value)),
    }
}

/// GHD `parseCredential`: the `key=value` lines of `value` (split at the
/// first `=`; lines without one are skipped), `key[]` entries numbered from
/// 0 in the order they come.
pub fn parse_credential(value: &str) -> Vec<(String, String)> {
    let mut credential: Vec<(String, String)> = Vec::new();
    // `/\r?\n/`
    for line in value.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.strip_suffix("[]") {
            Some(array) => {
                let mut i = 0;
                let numbered = loop {
                    let candidate = format!("{array}[{i}]");
                    if !credential.iter().any(|(k, _)| *k == candidate) {
                        break candidate;
                    }
                    i += 1;
                };
                set(&mut credential, numbered, value.to_string());
            }
            None => set(&mut credential, key.to_string(), value.to_string()),
        }
    }
    credential
}

/// GHD `formatCredential`: one `key=value\n` line per entry, numbered array
/// keys (`key[3]`) written as `key[]`.
pub fn format_credential(credential: &[(String, String)]) -> Result<String, CredentialError> {
    let mut out = String::new();
    for (key, value) in credential {
        if value.contains(['\n', '\0']) {
            return Err(CredentialError::ForbiddenCharacters(key.clone()));
        }
        out.push_str(&unnumbered(key));
        out.push('=');
        out.push_str(value);
        out.push('\n');
    }
    Ok(out)
}

/// `key.replace(/\[\d+\]$/, '[]')`.
fn unnumbered(key: &str) -> String {
    if let Some(open) = key.strip_suffix(']').and_then(|rest| rest.rfind('['))
        && key[open + 1..key.len() - 1]
            .bytes()
            .all(|b| b.is_ascii_digit())
        && key.len() - 1 > open + 1
    {
        return format!("{}[]", &key[..open]);
    }
    key.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_plain_and_array_entries() {
        let parsed = parse_credential(
            "protocol=https\r\nhost=github.com\nwwwauth[]=a\nwwwauth[]=b\nnoise\n",
        );
        assert_eq!(
            parsed,
            vec![
                ("protocol".to_string(), "https".to_string()),
                ("host".to_string(), "github.com".to_string()),
                ("wwwauth[0]".to_string(), "a".to_string()),
                ("wwwauth[1]".to_string(), "b".to_string()),
            ]
        );
        assert_eq!(
            format_credential(&parsed).unwrap(),
            "protocol=https\nhost=github.com\nwwwauth[]=a\nwwwauth[]=b\n"
        );
        assert_eq!(unnumbered("key[x]"), "key[x]");
        assert_eq!(unnumbered("key[]"), "key[]");
    }

    #[test]
    fn refuses_values_with_newlines() {
        let credential = vec![("password".to_string(), "a\nb".to_string())];
        assert_eq!(
            format_credential(&credential),
            Err(CredentialError::ForbiddenCharacters("password".into()))
        );
    }
}
