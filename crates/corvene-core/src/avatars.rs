//! Avatar cache (GHD `ui/lib/avatar.tsx` + `AvatarStore`): commit authors
//! resolve through GitHub's e-mail avatar endpoint, signed-in accounts
//! through their API `avatar_url`. Images land in `~/Library/Caches/Corvene/avatars`
//! and are loaded from there afterwards. An Enterprise Server repository's
//! authors resolve through that server's endpoint, as in GHD.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::host::Host;
use tracing::debug;

use crate::dispatcher::Dispatcher;
use crate::remote::spawn_bg;

/// One cached (or in-flight) avatar, keyed by lower-case e-mail or `url:<url>`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AvatarEntry {
    pub path: Option<PathBuf>,
    pub loading: bool,
}

pub type Avatars = HashMap<String, AvatarEntry>;

const AVATAR_SIZE: u32 = 64;

/// `getAvatarUrlCandidates` for a commit author e-mail. `api_base` is the
/// selected repository's GitHub endpoint: an Enterprise Server answers on
/// its own `enterprise/avatars/u/e`, a ghe.com host only with an avatar
/// token (which Corvene does not fetch), anything else on GitHub.com's
/// (`getEmailAvatarUrl`).
fn candidates_for_email(
    email: &str,
    accounts: &[corvene_models::Account],
    api_base: Option<&str>,
) -> Vec<String> {
    let mut out = Vec::new();
    for account in accounts {
        if account.emails.iter().any(|e| e.eq_ignore_ascii_case(email))
            && let Some(url) = &account.avatar_url
        {
            out.push(with_size(url));
        }
    }
    let endpoint = api_base.map(corvene_github::Endpoint::from_api_base);
    let query = format!("email={}&s={AVATAR_SIZE}", urlencode(email));
    match endpoint.filter(|e| !e.is_dotcom()) {
        // `isGHE`: needs `api.getAvatarToken()`
        Some(e) if e.host().to_ascii_lowercase().ends_with(".ghe.com") => {}
        // `isGHES`
        Some(e) => out.push(format!("{}/enterprise/avatars/u/e?{query}", e.api_base)),
        // GitHub's e-mail avatar endpoint falls back to Gravatar server-side.
        None => out.push(format!("https://avatars.githubusercontent.com/u/e?{query}")),
    }
    out
}

fn with_size(url: &str) -> String {
    if url.contains('?') {
        format!("{url}&s={AVATAR_SIZE}")
    } else {
        format!("{url}?s={AVATAR_SIZE}")
    }
}

fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn cache_file(url: &str) -> PathBuf {
    // FNV-1a keeps the file name short and stable per URL.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in url.bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    corvene_platform::paths::cache_dir()
        .join("avatars")
        .join(format!("{hash:016x}"))
}

/// Download the first candidate that answers; cached files win.
fn fetch(candidates: Vec<String>) -> Option<PathBuf> {
    for url in candidates {
        let file = cache_file(&url);
        if file.is_file() {
            return Some(file);
        }
        match corvene_github::download(&url) {
            Ok(bytes) if !bytes.is_empty() => {
                if let Some(dir) = file.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                if std::fs::write(&file, bytes).is_ok() {
                    return Some(file);
                }
            }
            Ok(_) => {}
            Err(err) => debug!(%url, %err, "avatar download failed"),
        }
    }
    None
}

impl Dispatcher {
    fn request_avatar(key: String, candidates: Vec<String>, cx: &mut dyn Host) {
        let started = Self::state(cx).update(cx, |s, cx| {
            if s.avatars.contains_key(&key) {
                return false;
            }
            s.avatars.insert(
                key.clone(),
                AvatarEntry {
                    path: None,
                    loading: true,
                },
            );
            cx.notify();
            true
        });
        if !started {
            return;
        }
        spawn_bg(
            cx,
            move || fetch(candidates),
            move |path, cx| {
                Self::state(cx).update(cx, |s, cx| {
                    s.avatars.insert(
                        key,
                        AvatarEntry {
                            path,
                            loading: false,
                        },
                    );
                    cx.notify();
                });
            },
        );
    }

    /// Commit author / committer avatar by e-mail (no-op once requested).
    pub fn request_avatar_for_email(email: &str, cx: &mut dyn Host) {
        let email = email.trim().to_lowercase();
        if email.is_empty() {
            return;
        }
        if Self::state(cx).read(cx).avatars.contains_key(&email) {
            return;
        }
        let s = Self::state(cx).read(cx);
        let api_base = s
            .selected_repository()
            .and_then(|r| r.github.as_ref())
            .map(|g| g.endpoint.clone());
        let candidates = candidates_for_email(&email, &s.accounts, api_base.as_deref());
        Self::request_avatar(email, candidates, cx);
    }

    /// An account's own `avatar_url`.
    pub fn request_avatar_url(url: &str, cx: &mut dyn Host) {
        let key = format!("url:{url}");
        if Self::state(cx).read(cx).avatars.contains_key(&key) {
            return;
        }
        Self::request_avatar(key, vec![with_size(url)], cx);
    }
}

/// Corvene `112-initials-avatars`: up to two upper-case initials of an
/// author name (first and last word), else of the e-mail's local part.
pub fn initials(name: &str, email: &str) -> String {
    let words: Vec<&str> = name
        .split(|c: char| c.is_whitespace() || c == '.' || c == '_' || c == '-')
        .filter(|w| w.chars().next().is_some_and(char::is_alphanumeric))
        .collect();
    let words = if words.is_empty() {
        email
            .split('@')
            .next()
            .unwrap_or("")
            .split(['.', '_', '-', '+'])
            .filter(|w| w.chars().next().is_some_and(char::is_alphanumeric))
            .collect()
    } else {
        words
    };
    let first = |w: &str| {
        w.chars()
            .next()
            .map(|c| c.to_uppercase().collect::<String>())
    };
    match words.as_slice() {
        [] => String::new(),
        [only] => first(only).unwrap_or_default(),
        [head, .., last] => first(head).unwrap_or_default() + &first(last).unwrap_or_default(),
    }
}

/// Corvene `112-initials-avatars`: a stable hue (0–359) for an e-mail.
pub fn initials_hue(email: &str) -> u16 {
    let mut hash: u32 = 0x811c_9dc5;
    for b in email.trim().to_lowercase().bytes() {
        hash ^= b as u32;
        hash = hash.wrapping_mul(0x0100_0193);
    }
    (hash % 360) as u16
}

/// Cached image path for an e-mail, if resolved.
pub fn avatar_for_email(avatars: &Avatars, email: &str) -> Option<PathBuf> {
    avatars
        .get(&email.trim().to_lowercase())
        .and_then(|e| e.path.clone())
}

/// Cached image path for an account `avatar_url`, if resolved.
pub fn avatar_for_url(avatars: &Avatars, url: &str) -> Option<PathBuf> {
    avatars
        .get(&format!("url:{url}"))
        .and_then(|e| e.path.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn author_initials() {
        assert_eq!(initials("Ada Lovelace", "a@x.io"), "AL");
        assert_eq!(initials("ada  b. king-lovelace", "a@x.io"), "AL");
        assert_eq!(initials("Mona", "a@x.io"), "M");
        assert_eq!(initials("", "jane.doe@x.io"), "JD");
        assert_eq!(initials(" ", ""), "");
        assert_eq!(initials("élodie", ""), "É");
        assert_eq!(initials_hue("A@x.io"), initials_hue("a@x.io "));
        assert!(initials_hue("a@x.io") < 360);
    }

    #[test]
    fn email_candidates_end_with_the_github_endpoint() {
        let c = candidates_for_email("a+b@example.com", &[], None);
        assert_eq!(
            c,
            vec!["https://avatars.githubusercontent.com/u/e?email=a%2Bb%40example.com&s=64"]
        );
        let dotcom = candidates_for_email("a@b.c", &[], Some("https://api.github.com"));
        assert_eq!(
            dotcom,
            vec!["https://avatars.githubusercontent.com/u/e?email=a%40b.c&s=64"]
        );
        // GHD `getEmailAvatarUrl` for an Enterprise Server
        let ghes = candidates_for_email("a@b.c", &[], Some("https://ghe.corp/api/v3"));
        assert_eq!(
            ghes,
            vec!["https://ghe.corp/api/v3/enterprise/avatars/u/e?email=a%40b.c&s=64"]
        );
        assert!(candidates_for_email("a@b.c", &[], Some("https://api.x.ghe.com")).is_empty());
        assert_ne!(cache_file("x"), cache_file("y"));
    }
}
