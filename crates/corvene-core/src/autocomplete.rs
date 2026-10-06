//! Commit-form autocompletion data: the `:emoji:` / `#issue` / `@user`
//! trigger scanner (GHD `ui/autocompletion/autocompleting-text-input.tsx`
//! `attemptAutocompletion`) and the per-repository issue and mentionable-user
//! caches (`lib/stores/issues-store.ts`, `lib/stores/github-user-store.ts`).
//!
//! Deviation from GHD: both caches live in memory for the session instead of
//! IndexedDB, so the first `#` / `@` after launch fetches from the API.
//!
//! Deviation (flag `issues-full-refresh-hours`): the issue cache fetches
//! every open issue again once this many hours have passed since the last
//! full fetch, so deleted and transferred issues leave `#` completion (GHD
//! only ever asks for issues updated since the newest cached one).
//!
//! Deviation (`770-co-authors-from-history`): co-author suggestions also
//! offer the distinct authors of the newest commits ([`authors_matching`]),
//! for people without a GitHub account (GHD suggests mentionable users only).

use std::collections::HashMap;
use std::ops::Range;
use std::time::{Duration, Instant};

use crate::host::Host;
use corvene_models::{Author, GitHubRepository, UnknownAuthorState};
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::dispatcher::Dispatcher;
use crate::remote::spawn_bg;

/// GHD `DefaultMaxHits`.
pub const DEFAULT_MAX_HITS: usize = 25;
/// GHD `UpdateIssuesThrottleInterval`.
const ISSUES_REFRESH_INTERVAL: Duration = Duration::from_secs(60);
/// GHD `MaxFetchFrequency` for mentionables.
const MENTIONABLES_REFRESH_INTERVAL: Duration = Duration::from_secs(10 * 60);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TriggerKind {
    Emoji,
    Issue,
    User,
    /// GHD `BranchAutocompletionProvider`: the whole input is the filter
    /// (`/^(.*)$/`), so there is no trigger character.
    Branch,
    /// Corvene addition (`223-add-local-path-completion`): folders for a
    /// typed path, see [`folder_completions`].
    Path,
}

impl TriggerKind {
    fn char(self) -> Option<u8> {
        match self {
            TriggerKind::Emoji => Some(b':'),
            TriggerKind::Issue => Some(b'#'),
            TriggerKind::User => Some(b'@'),
            TriggerKind::Branch | TriggerKind::Path => None,
        }
    }
}

/// A trigger whose match ends exactly at the caret.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trigger {
    pub kind: TriggerKind,
    /// Byte range of the filter text (everything after the trigger character).
    pub range: Range<usize>,
    /// The filter text, lower-cased.
    pub text: String,
}

/// GHD's provider regexes, all of the shape
/// `(?:^|\n| )(?:<trigger>)([a-z\d\\+-][a-z\d_]*)?` (`@` also allows `-` in
/// the tail). The text is lower-cased first; ASCII lower-casing keeps byte
/// offsets stable.
pub fn find_trigger(text: &str, caret: usize) -> Option<Trigger> {
    let lower = text.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    for kind in [TriggerKind::Emoji, TriggerKind::Issue, TriggerKind::User] {
        let Some(trigger) = kind.char() else {
            continue;
        };
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] != trigger || !(i == 0 || bytes[i - 1] == b'\n' || bytes[i - 1] == b' ') {
                i += 1;
                continue;
            }
            let start = i + 1;
            let mut end = start;
            if end < bytes.len() && is_first(bytes[end]) {
                end += 1;
                while end < bytes.len() && is_tail(bytes[end], kind) {
                    end += 1;
                }
            }
            if end == caret {
                return Some(Trigger {
                    kind,
                    range: start..end,
                    text: lower[start..end].to_string(),
                });
            }
            i = end.max(i + 1);
        }
    }
    None
}

fn is_first(b: u8) -> bool {
    b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'\\' || b == b'+' || b == b'-'
}

fn is_tail(b: u8, kind: TriggerKind) -> bool {
    b.is_ascii_lowercase()
        || b.is_ascii_digit()
        || b == b'_'
        || (kind == TriggerKind::User && b == b'-')
}

// ---- issues ----

/// GHD `IIssue` (open issues only; closed ones are pruned on refresh).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Issue {
    pub number: u64,
    pub title: String,
    /// ISO-8601 `updated_at`, the `since` watermark for the next refresh.
    pub updated_at: String,
}

/// GHD `IIssueHit`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IssueHit {
    pub number: u64,
    pub title: String,
}

#[derive(Clone, Debug, Default)]
pub struct IssueCache {
    pub issues: Vec<Issue>,
    pub refreshed_at: Option<Instant>,
    pub loading: bool,
    /// The redb copy was consulted (GHD: IndexedDB `IssuesDatabase`).
    pub loaded: bool,
    /// Unix time of the last fetch of every open issue.
    pub full_refreshed_at_secs: u64,
}

/// What redb keeps between launches (`issues:<html_url>`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PersistedIssues {
    pub issues: Vec<Issue>,
    #[serde(default)]
    pub full_refreshed_at_secs: u64,
}

/// `mentionables:<html_url>`, with the fetch time so the ten-minute
/// throttle survives a restart (GHD `MaxFetchFrequency`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PersistedMentionables {
    pub users: Vec<MentionableUser>,
    pub fetched_at_secs: u64,
}

pub(crate) fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// GHD `getIssuesMatching`: newest first for an empty filter, otherwise a
/// substring match on `"<number> <title>"` sorted by match position, then
/// title.
pub fn issues_matching(issues: &[Issue], text: &str, max_hits: usize) -> Vec<IssueHit> {
    if text.is_empty() {
        let mut all: Vec<&Issue> = issues.iter().collect();
        all.sort_by_key(|i| std::cmp::Reverse(i.number));
        return all
            .into_iter()
            .take(max_hits)
            .map(|i| IssueHit {
                number: i.number,
                title: i.title.clone(),
            })
            .collect();
    }
    let needle = text.to_lowercase();
    let mut hits: Vec<(usize, IssueHit)> = issues
        .iter()
        .filter_map(|i| {
            format!("{} {}", i.number, i.title)
                .trim()
                .to_lowercase()
                .find(&needle)
                .map(|ix| {
                    (
                        ix,
                        IssueHit {
                            number: i.number,
                            title: i.title.clone(),
                        },
                    )
                })
        })
        .collect();
    hits.sort_by(|x, y| x.0.cmp(&y.0).then(x.1.title.cmp(&y.1.title)));
    hits.into_iter().take(max_hits).map(|h| h.1).collect()
}

// ---- mentionable users ----

/// GHD `IMentionableUser`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MentionableUser {
    pub login: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct MentionableCache {
    pub users: Vec<MentionableUser>,
    pub refreshed_at: Option<Instant>,
    pub loading: bool,
    pub loaded: bool,
}

/// GHD `matchMentionableUsers` + the provider's "dotcom doesn't let you
/// autocomplete on your own handle" filter.
pub fn users_matching(
    users: &[MentionableUser],
    text: &str,
    exclude_login: Option<&str>,
    max_hits: usize,
) -> Vec<MentionableUser> {
    let needle = text.to_lowercase();
    let mut hits: Vec<(usize, &MentionableUser)> = users
        .iter()
        .filter(|u| exclude_login.is_none_or(|own| u.login != own))
        .filter_map(|u| {
            format!("{} {}", u.login, u.name.as_deref().unwrap_or(""))
                .trim()
                .to_lowercase()
                .find(&needle)
                .map(|ix| (ix, u))
        })
        .collect();
    hits.sort_by(|x, y| x.0.cmp(&y.0).then(x.1.login.cmp(&y.1.login)));
    hits.into_iter()
        .take(max_hits)
        .map(|h| h.1.clone())
        .collect()
}

/// `735-free-form-co-authors`: `Name <email>` typed in the co-authors box,
/// for a co-author without a GitHub account.
pub fn parse_co_author_address(text: &str) -> Option<(String, String)> {
    let (name, email) = text.trim().strip_suffix('>')?.rsplit_once('<')?;
    let (name, email) = (name.trim(), email.trim());
    let valid_email = email
        .split_once('@')
        .is_some_and(|(user, host)| !user.is_empty() && !host.is_empty())
        && !email.contains(char::is_whitespace);
    (!name.is_empty() && valid_email).then(|| (name.to_string(), email.to_string()))
}

/// Cache key: one entry per GitHub repository.
pub fn cache_key(github: &GitHubRepository) -> String {
    github.html_url.clone()
}

/// Corvene `882-commit-author-links`: the GitHub login behind a commit
/// author's e-mail in `github`'s repository: a no-reply address, a signed-in
/// account of that endpoint with the address, or a mentionable user (the
/// commit box's @ suggestions) with it.
pub fn login_for_email(
    email: &str,
    github: &GitHubRepository,
    accounts: &[corvene_models::Account],
    mentionables: &MentionableCaches,
) -> Option<String> {
    if let Some(login) = corvene_models::stealth_email_login(email, &github.endpoint) {
        return Some(login);
    }
    let email = email.trim();
    if email.is_empty() {
        return None;
    }
    if let Some(account) = accounts.iter().find(|a| {
        a.endpoint == github.endpoint && a.emails.iter().any(|e| e.eq_ignore_ascii_case(email))
    }) {
        return Some(account.login.clone());
    }
    mentionables
        .get(&cache_key(github))?
        .users
        .iter()
        .find(|u| {
            u.email
                .as_deref()
                .is_some_and(|e| e.eq_ignore_ascii_case(email))
        })
        .map(|u| u.login.clone())
}

/// How many recent commits `770-co-authors-from-history` reads authors from.
pub const RECENT_AUTHOR_COMMITS: usize = 500;

/// `770-co-authors-from-history`: the authors whose name or email contains
/// `text` (case ignored; all of them for an empty text), best match first,
/// leaving out `exclude_emails` (already co-authors, yourself).
pub fn authors_matching(
    authors: &[(String, String)],
    text: &str,
    exclude_emails: &[String],
    max_hits: usize,
) -> Vec<(String, String)> {
    let needle = text.to_lowercase();
    let mut hits: Vec<(usize, usize, &(String, String))> = authors
        .iter()
        .enumerate()
        .filter(|(_, (_, email))| !exclude_emails.iter().any(|e| e.eq_ignore_ascii_case(email)))
        .filter_map(|(order, author)| {
            format!("{} {}", author.0, author.1)
                .to_lowercase()
                .find(&needle)
                .map(|ix| (ix, order, author))
        })
        .collect();
    // where the text matched, then recency
    hits.sort_by(|x, y| x.0.cmp(&y.0).then(x.1.cmp(&y.1)));
    hits.into_iter()
        .take(max_hits)
        .map(|h| h.2.clone())
        .collect()
}

impl Dispatcher {
    /// `770-co-authors-from-history`: read the repository's recent commit
    /// authors once per session.
    pub fn load_recent_authors(id: u64, cx: &mut dyn Host) {
        let start = Self::state(cx).update(cx, |s, _| {
            if !s.flags.bool(crate::flags::ids::CO_AUTHORS_FROM_HISTORY) {
                return false;
            }
            let rs = s.repo_state_mut(id);
            if rs.recent_authors.is_some() || rs.recent_authors_loading {
                return false;
            }
            rs.recent_authors_loading = true;
            true
        });
        if !start {
            return;
        }
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        spawn_bg(
            cx,
            move || corvene_git::recent_authors(git, &workdir, RECENT_AUTHOR_COMMITS),
            move |result, cx| {
                let authors = result.unwrap_or_else(|err| {
                    warn!(%err, "could not read recent commit authors");
                    Vec::new()
                });
                Self::state(cx).update(cx, |s, cx| {
                    let rs = s.repo_state_mut(id);
                    rs.recent_authors_loading = false;
                    rs.recent_authors = Some(std::sync::Arc::new(authors));
                    cx.notify();
                });
            },
        );
    }

    /// The endpoint, token and login to call the API about `github` with:
    /// the account of the listed repository it belongs to
    /// (`AppState::account_for_github`, `527-multiple-accounts`). Code that
    /// knows the repository asks [`Self::api_for_repository`].
    pub(crate) fn api_for(
        github: &GitHubRepository,
        cx: &dyn Host,
    ) -> Option<(corvene_github::Endpoint, String, String)> {
        let s = Self::state(cx).read(cx);
        api_with(s.account_for_github(github)?)
    }

    /// [`Self::api_for`] with the account repository `id` uses on
    /// `github`'s endpoint (`AppState::account_for_repository_on`).
    pub(crate) fn api_for_repository(
        id: u64,
        github: &GitHubRepository,
        cx: &dyn Host,
    ) -> Option<(corvene_github::Endpoint, String, String)> {
        let s = Self::state(cx).read(cx);
        api_with(s.account_for_repository_on(id, &github.endpoint)?)
    }

    /// GHD `refreshIssues`, throttled to once a minute per repository: the
    /// first fetch takes every open issue, later ones ask for everything
    /// updated since the newest cached issue and prune what closed.
    pub fn refresh_issues(github: &GitHubRepository, cx: &mut dyn Host) {
        let key = cache_key(github);
        let since = {
            let mut skip = false;
            let mut since = None;
            Self::state(cx).update(cx, |s, cx| {
                let store = s.store.clone();
                let full_every_hours = s.flags.number(crate::flags::ids::ISSUES_FULL_REFRESH_HOURS);
                let cache = s.issues.entry(key.clone()).or_default();
                if !cache.loaded {
                    cache.loaded = true;
                    if let Ok(Some(persisted)) =
                        store.get::<PersistedIssues>(&format!("issues:{key}"))
                    {
                        cache.issues = persisted.issues;
                        cache.full_refreshed_at_secs = persisted.full_refreshed_at_secs;
                    }
                }
                if cache.loading
                    || cache
                        .refreshed_at
                        .is_some_and(|t| t.elapsed() < ISSUES_REFRESH_INTERVAL)
                {
                    skip = true;
                    return;
                }
                cache.loading = true;
                since = cache.issues.iter().map(|i| i.updated_at.clone()).max();
                // `issues-full-refresh-hours`: now and then fetch every open
                // issue again so deleted and transferred ones drop out (an
                // incremental fetch never reports them)
                let full_every_hours = full_every_hours.max(0) as u64;
                if full_every_hours > 0
                    && now_secs().saturating_sub(cache.full_refreshed_at_secs)
                        >= full_every_hours * 3600
                {
                    since = None;
                }
                cx.notify();
            });
            if skip {
                return;
            }
            since
        };
        let Some((endpoint, token, _)) = Self::api_for(github, cx) else {
            Self::state(cx).update(cx, |s, _| {
                if let Some(c) = s.issues.get_mut(&key) {
                    c.loading = false;
                }
            });
            return;
        };
        let (owner, name) = (github.owner.clone(), github.name.clone());
        let full = since.is_none();
        let state = if since.is_some() {
            corvene_github::IssueState::All
        } else {
            corvene_github::IssueState::Open
        };
        spawn_bg(
            cx,
            move || {
                corvene_github::Client::new(endpoint, token)
                    .issues(&owner, &name, state, since.as_deref())
                    .map_err(|e| e.to_string())
            },
            move |result, cx| {
                Self::state(cx).update(cx, |s, cx| {
                    let cache = s.issues.entry(key.clone()).or_default();
                    cache.loading = false;
                    match result {
                        Ok(fetched) => {
                            cache.refreshed_at = Some(Instant::now());
                            if full {
                                // every open issue is in `fetched`
                                cache.issues.clear();
                                cache.full_refreshed_at_secs = now_secs();
                            }
                            for issue in fetched {
                                cache.issues.retain(|i| i.number != issue.number);
                                if issue.state == "open" {
                                    cache.issues.push(Issue {
                                        number: issue.number,
                                        title: issue.title,
                                        updated_at: issue.updated_at,
                                    });
                                }
                            }
                            cache.issues.sort_by_key(|i| std::cmp::Reverse(i.number));
                            let persisted = PersistedIssues {
                                issues: cache.issues.clone(),
                                full_refreshed_at_secs: cache.full_refreshed_at_secs,
                            };
                            if let Err(err) = s.store.set(&format!("issues:{key}"), &persisted) {
                                warn!(%err, "could not persist issues");
                            }
                        }
                        Err(err) => warn!(%err, "could not refresh issues"),
                    }
                    cx.notify();
                });
            },
        );
    }

    /// GHD `updateMentionables`, throttled to once every ten minutes.
    pub fn refresh_mentionables(github: &GitHubRepository, cx: &mut dyn Host) {
        let key = cache_key(github);
        let skip = Self::state(cx).update(cx, |s, cx| {
            let store = s.store.clone();
            let cache = s.mentionables.entry(key.clone()).or_default();
            if !cache.loaded {
                cache.loaded = true;
                if let Ok(Some(persisted)) =
                    store.get::<PersistedMentionables>(&format!("mentionables:{key}"))
                {
                    cache.users = persisted.users;
                    let age = now_secs().saturating_sub(persisted.fetched_at_secs);
                    if age < MENTIONABLES_REFRESH_INTERVAL.as_secs() {
                        cache.refreshed_at = Instant::now().checked_sub(Duration::from_secs(age));
                    }
                }
            }
            if cache.loading
                || cache
                    .refreshed_at
                    .is_some_and(|t| t.elapsed() < MENTIONABLES_REFRESH_INTERVAL)
            {
                return true;
            }
            cache.loading = true;
            cx.notify();
            false
        });
        if skip {
            return;
        }
        let Some((endpoint, token, _)) = Self::api_for(github, cx) else {
            Self::state(cx).update(cx, |s, _| {
                if let Some(c) = s.mentionables.get_mut(&key) {
                    c.loading = false;
                }
            });
            return;
        };
        let (owner, name) = (github.owner.clone(), github.name.clone());
        spawn_bg(
            cx,
            move || {
                corvene_github::Client::new(endpoint, token)
                    .mentionables(&owner, &name)
                    .map_err(|e| e.to_string())
            },
            move |result, cx| {
                Self::state(cx).update(cx, |s, cx| {
                    let cache = s.mentionables.entry(key.clone()).or_default();
                    cache.loading = false;
                    match result {
                        Ok(Some(users)) => {
                            cache.refreshed_at = Some(Instant::now());
                            cache.users = users
                                .into_iter()
                                .map(|u| MentionableUser {
                                    login: u.login,
                                    name: u.name,
                                    email: u.email,
                                    avatar_url: u.avatar_url,
                                })
                                .collect();
                            let persisted = PersistedMentionables {
                                users: cache.users.clone(),
                                fetched_at_secs: now_secs(),
                            };
                            if let Err(err) =
                                s.store.set(&format!("mentionables:{key}"), &persisted)
                            {
                                warn!(%err, "could not persist mentionable users");
                            }
                        }
                        Ok(None) => cache.refreshed_at = Some(Instant::now()),
                        Err(err) => warn!(%err, "could not refresh mentionable users"),
                    }
                    cx.notify();
                });
            },
        );
    }

    /// GHD `_setShowCoAuthoredBy` (the "Add Co-Authors" toggle, per repository).
    pub fn set_show_co_authored_by(id: u64, show: bool, cx: &mut dyn Host) {
        Self::state(cx).update(cx, |s, cx| {
            s.repo_state_mut(id).show_co_authored_by = show;
            cx.notify();
        });
    }

    /// GHD `_setCoAuthors`.
    pub fn set_co_authors(id: u64, authors: Vec<Author>, cx: &mut dyn Host) {
        Self::state(cx).update(cx, |s, cx| {
            s.repo_state_mut(id).co_authors = authors;
            cx.notify();
        });
    }

    /// GHD `AuthorInput.attemptUnknownAuthorSearch`: look a typed handle up
    /// (`GET /users/{login}`) and turn it into a known author, or mark it as
    /// not found. Without a signed-in account the handle stays unknown.
    pub fn resolve_unknown_author(
        id: u64,
        github: &GitHubRepository,
        username: String,
        cx: &mut dyn Host,
    ) {
        fn mark_error(id: u64, username: &str, cx: &mut dyn Host) {
            Dispatcher::state(cx).update(cx, |s, cx| {
                for a in &mut s.repo_state_mut(id).co_authors {
                    if let Author::Unknown { username: u, state } = a
                        && u.eq_ignore_ascii_case(username)
                    {
                        *state = UnknownAuthorState::Error;
                    }
                }
                cx.notify();
            });
        }
        let Some((endpoint, token, _)) = Self::api_for_repository(id, github, cx) else {
            mark_error(id, &username, cx);
            return;
        };
        let api_base = github.endpoint.clone();
        let login = username.clone();
        spawn_bg(
            cx,
            move || {
                corvene_github::Client::new(endpoint, token)
                    .user(&login)
                    .map_err(|e| e.to_string())
            },
            move |result, cx| match result {
                // `getByLogin`: only users can co-author (not organizations or bots)
                Ok(Some(user)) if user.kind.as_deref() == Some("User") => {
                    let email = user
                        .email
                        .clone()
                        .filter(|e| !e.is_empty())
                        .unwrap_or_else(|| {
                            corvene_models::stealth_email(user.id, &user.login, &api_base)
                        });
                    let known = Author::Known {
                        name: user.name.clone().unwrap_or_else(|| user.login.clone()),
                        email,
                        username: Some(user.login.clone()),
                    };
                    Self::state(cx).update(cx, |s, cx| {
                        for a in &mut s.repo_state_mut(id).co_authors {
                            if a.username()
                                .is_some_and(|u| u.eq_ignore_ascii_case(&username))
                                && matches!(a, Author::Unknown { .. })
                            {
                                *a = known.clone();
                            }
                        }
                        cx.notify();
                    });
                }
                Ok(_) => mark_error(id, &username, cx),
                Err(err) => {
                    warn!(%err, "could not look up co-author");
                    mark_error(id, &username, cx);
                }
            },
        );
    }

    /// The signed-in login for a repository's endpoint (excluded from `@`
    /// completions, as on dotcom).
    pub fn own_login_for(github: &GitHubRepository, cx: &dyn Host) -> Option<String> {
        Self::state(cx)
            .read(cx)
            .account_for_github(github)
            .map(|a| a.login.clone())
    }
}

/// `account`'s endpoint, token (from the Keychain) and login.
fn api_with(
    account: &corvene_models::Account,
) -> Option<(corvene_github::Endpoint, String, String)> {
    let token = corvene_platform::keychain::token(&account.host(), &account.login)
        .ok()
        .flatten()?;
    Some((
        corvene_github::Endpoint::from_api_base(&account.endpoint),
        token,
        account.login.clone(),
    ))
}

/// `223-add-local-path-completion`: the folders completing a typed path,
/// as `(completion, folder name)`: the text up to the last `/` plus each
/// sub-folder whose name starts with the rest (case-insensitive; hidden
/// ones only when the rest starts with `.`), sorted, at most `max`. `~/`
/// is listed from `$HOME` but kept in the completion.
pub fn folder_completions(text: &str, max: usize) -> Vec<(String, String)> {
    // Windows: either separator, and the completion continues with the one
    // that was typed
    let Some(slash) = text.rfind(|c| c == '/' || (cfg!(windows) && c == '\\')) else {
        return Vec::new();
    };
    let (dir, prefix) = (&text[..=slash], &text[slash + 1..]);
    let separator = &text[slash..=slash];
    let home_relative = dir
        .strip_prefix("~/")
        .or_else(|| dir.strip_prefix("~\\").filter(|_| cfg!(windows)));
    let listed = match home_relative {
        Some(rest) => match std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
            Some(home) => std::path::PathBuf::from(home).join(rest),
            None => return Vec::new(),
        },
        None if dir.starts_with('/') || std::path::Path::new(dir).is_absolute() => {
            std::path::PathBuf::from(dir)
        }
        None => return Vec::new(),
    };
    let Ok(entries) = std::fs::read_dir(&listed) else {
        return Vec::new();
    };
    let lower = prefix.to_lowercase();
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| {
            (!n.starts_with('.') || prefix.starts_with('.')) && n.to_lowercase().starts_with(&lower)
        })
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    names.truncate(max);
    names
        .into_iter()
        .map(|n| (format!("{dir}{n}{separator}"), n))
        .collect()
}

/// Per-repository caches keyed by [`cache_key`].
pub type IssueCaches = HashMap<String, IssueCache>;
pub type MentionableCaches = HashMap<String, MentionableCache>;

#[cfg(test)]
mod tests {
    #[test]
    fn authors_match_name_or_email_and_skip_excluded() {
        let authors = vec![
            ("Ann Lee".to_string(), "ann@x.io".to_string()),
            ("Bob".to_string(), "bob@lee.dev".to_string()),
            ("Cy".to_string(), "cy@x.io".to_string()),
        ];
        let names = |hits: Vec<(String, String)>| hits.into_iter().map(|h| h.0).collect::<Vec<_>>();
        assert_eq!(
            names(authors_matching(&authors, "lee", &[], 5)),
            vec!["Ann Lee", "Bob"]
        );
        assert_eq!(
            names(authors_matching(&authors, "", &["ANN@x.io".to_string()], 5)),
            vec!["Bob", "Cy"]
        );
        assert_eq!(
            names(authors_matching(&authors, "", &[], 1)),
            vec!["Ann Lee"]
        );
    }

    #[test]
    fn parses_co_author_address() {
        assert_eq!(
            parse_co_author_address(" Jane Q. Doe <jane@example.com> "),
            Some(("Jane Q. Doe".to_string(), "jane@example.com".to_string()))
        );
        assert_eq!(parse_co_author_address("<jane@example.com>"), None);
        assert_eq!(parse_co_author_address("Jane <jane>"), None);
        assert_eq!(parse_co_author_address("Jane <ja ne@example.com>"), None);
        assert_eq!(parse_co_author_address("Jane jane@example.com"), None);
    }
    use super::*;

    #[test]
    fn logins_for_author_emails() {
        let gh = GitHubRepository {
            endpoint: "https://api.github.com".into(),
            owner: "o".into(),
            name: "r".into(),
            html_url: "https://github.com/o/r".into(),
            clone_url: String::new(),
            default_branch: None,
            private: false,
            fork: false,
            parent: None,
            archived: false,
            permissions: None,
            allow_forking: None,
            node_id: None,
        };
        let mut mentionables = MentionableCaches::new();
        mentionables.insert(
            cache_key(&gh),
            MentionableCache {
                users: vec![MentionableUser {
                    login: "mona".into(),
                    name: None,
                    email: Some("Mona@Example.com".into()),
                    avatar_url: None,
                }],
                ..MentionableCache::default()
            },
        );
        let login = |email: &str| login_for_email(email, &gh, &[], &mentionables);
        assert_eq!(
            login("7+hub@users.noreply.github.com").as_deref(),
            Some("hub")
        );
        assert_eq!(login("mona@example.com").as_deref(), Some("mona"));
        assert_eq!(login("someone@example.com"), None);
        assert_eq!(login(""), None);
    }

    #[test]
    fn folder_completions_list_matching_sub_folders() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["alpha", "Apple", "beta", ".hidden"] {
            std::fs::create_dir(dir.path().join(name)).unwrap();
        }
        std::fs::write(dir.path().join("afile"), "").unwrap();
        let base = format!("{}/", dir.path().display());
        let names = |text: &str| -> Vec<String> {
            folder_completions(text, 25)
                .into_iter()
                .map(|(_, n)| n)
                .collect()
        };
        assert_eq!(names(&format!("{base}a")), ["alpha", "Apple"]);
        assert_eq!(names(&base), ["alpha", "Apple", "beta"]);
        assert_eq!(names(&format!("{base}.")), [".hidden"]);
        assert_eq!(
            folder_completions(&format!("{base}b"), 25)[0].0,
            format!("{base}beta/")
        );
        assert!(folder_completions("relative/a", 25).is_empty());
        assert!(folder_completions("nothing", 25).is_empty());
    }

    #[test]
    fn trigger_at_caret_only() {
        let t = "fix :smi";
        assert_eq!(
            find_trigger(t, t.len()),
            Some(Trigger {
                kind: TriggerKind::Emoji,
                range: 5..8,
                text: "smi".into()
            })
        );
        // caret elsewhere → nothing
        assert_eq!(find_trigger(t, 3), None);
        // bare trigger right after a space
        assert_eq!(
            find_trigger("see #", 5).map(|t| (t.kind, t.text)),
            Some((TriggerKind::Issue, String::new()))
        );
        // needs start / newline / space before the trigger
        assert_eq!(find_trigger("a:b", 3), None);
        assert_eq!(
            find_trigger("x\n@Ab-c", 7).map(|t| t.text),
            Some("ab-c".into())
        );
        // `-` is not part of an emoji tail
        assert_eq!(find_trigger(":ab-c", 5), None);
        assert_eq!(find_trigger(":ab-c", 3).map(|t| t.text), Some("ab".into()));
    }

    #[test]
    fn issues_rank_by_match_position() {
        let issues = vec![
            Issue {
                number: 12,
                title: "Crash on launch".into(),
                updated_at: String::new(),
            },
            Issue {
                number: 3,
                title: "Add crash reporter".into(),
                updated_at: String::new(),
            },
            Issue {
                number: 20,
                title: "Docs".into(),
                updated_at: String::new(),
            },
        ];
        let all = issues_matching(&issues, "", 25);
        assert_eq!(
            all.iter().map(|h| h.number).collect::<Vec<_>>(),
            vec![20, 12, 3]
        );
        // "12 crash on launch" matches at 3, "3 add crash reporter" at 6
        let hits = issues_matching(&issues, "crash", 25);
        assert_eq!(
            hits.iter().map(|h| h.number).collect::<Vec<_>>(),
            vec![12, 3]
        );
        let by_number = issues_matching(&issues, "1", 25);
        assert_eq!(by_number[0].number, 12);
    }

    #[test]
    fn users_exclude_self_and_rank() {
        let users = vec![
            MentionableUser {
                login: "wasi".into(),
                name: Some("Wasi Master".into()),
                email: None,
                avatar_url: None,
            },
            MentionableUser {
                login: "bob".into(),
                name: None,
                email: None,
                avatar_url: None,
            },
            MentionableUser {
                login: "alice".into(),
                name: Some("Alice Bobson".into()),
                email: None,
                avatar_url: None,
            },
        ];
        let hits = users_matching(&users, "bob", Some("wasi"), 25);
        assert_eq!(
            hits.iter().map(|u| u.login.as_str()).collect::<Vec<_>>(),
            vec!["bob", "alice"]
        );
        let all = users_matching(&users, "", None, 25);
        assert_eq!(
            all.iter().map(|u| u.login.as_str()).collect::<Vec<_>>(),
            vec!["alice", "bob", "wasi"]
        );
    }
}
