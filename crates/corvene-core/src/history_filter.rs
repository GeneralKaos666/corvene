//! Corvene `886-history-search`: a filter box above History. GitHub Desktop
//! has none (`app/src/ui/history/compare.tsx` only compares branches).
//!
//! The text is parsed into free words and `author:` / `before:` /
//! `after:` terms ([`parse_history_query`]). A background `git log` lists
//! the commits of HEAD that pass the git-side terms; the words are matched
//! against the message, author name and e-mail as pages are built, and a
//! lone hex word also matches abbreviated SHAs, listed first. While the
//! filter is active its results replace the History list
//! ([`crate::state::RepositoryState::visible_commits`]); clearing it brings
//! the plain list and selection back.

use std::sync::Arc;
use std::time::Duration;

use corvene_git::{CancelToken, HistoryQuery, LoggedCommit};
use corvene_models::Commit;
use gpui_kit::{App, AsyncApp};
use tracing::warn;

use crate::dispatcher::Dispatcher;

/// How long typing pauses before the search runs.
pub const HISTORY_FILTER_DEBOUNCE: Duration = Duration::from_millis(250);

/// How many commits a typed SHA prefix can match.
const SHA_PREFIX_MATCHES: usize = 10;

/// A repository's History filter.
#[derive(Clone, Debug, Default)]
pub struct HistoryFilter {
    /// The filter box's text.
    pub text: String,
    /// What `text` asks for; empty when History is not filtered.
    pub query: HistoryQuery,
    /// The matching commits loaded so far, newest first (SHA matches first).
    pub commits: Vec<Commit>,
    pub loading: bool,
    /// Every match is loaded.
    pub exhausted: bool,
    /// A search for `query` finished (its results, possibly none, are in).
    pub searched: bool,
    /// `git log`'s commits for the query, which pages are built from.
    logged: Option<Arc<Vec<LoggedCommit>>>,
    /// Where in `logged` the next page starts.
    next: usize,
    /// Commits matched by SHA prefix (already in `commits`).
    sha_matches: Vec<String>,
    /// The HEAD commit the search ran from; a refresh that moves HEAD runs
    /// it again.
    tip: Option<String>,
    /// Bumped by every search and by clearing; results of an older search
    /// are dropped.
    generation: u64,
    /// Bumped by every edit; a pending search of an older edit is dropped.
    debounce: u64,
    cancel: Option<CancelToken>,
}

impl HistoryFilter {
    pub fn is_active(&self) -> bool {
        !self.query.is_empty()
    }
}

/// Split the filter text into terms: `author:<name>`, `before:<date>` and
/// `after:<date>` (a date git understands, `2024-05-01` or `2.weeks.ago`),
/// everything else free words (lower case). Double quotes keep spaces
/// together (`author:"Mona Lisa"`, `"fix login"`); a term without a value
/// is ignored while it is being typed.
pub fn parse_history_query(text: &str) -> HistoryQuery {
    let mut query = HistoryQuery::default();
    for token in tokens(text) {
        let lower = token.to_lowercase();
        let term = ["author:", "before:", "after:"]
            .into_iter()
            .find(|key| lower.starts_with(key));
        match term {
            Some(key) => {
                let value = token[key.len()..].trim().to_string();
                if value.is_empty() {
                    continue;
                }
                match key {
                    "author:" => query.author = Some(value),
                    "before:" => query.before = Some(value),
                    _ => query.after = Some(value),
                }
            }
            None => query.words.push(lower),
        }
    }
    query
}

/// Whitespace-separated tokens; a double-quoted stretch belongs to the
/// token it is in, without the quotes.
fn tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for c in text.chars() {
        match c {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.trim().is_empty() {
        out.push(current);
    }
    out
}

impl Dispatcher {
    /// The History filter box was edited: search after a pause, or go back
    /// to the plain History once nothing is left to filter by.
    pub fn set_history_filter_text(id: u64, text: String, cx: &mut App) {
        let query = parse_history_query(&text);
        let debounce = Self::state(cx).update(cx, |s, cx| {
            let filter = &mut s.repo_state_mut(id).history_filter;
            if filter.text == text {
                return None;
            }
            filter.text = text;
            filter.debounce += 1;
            cx.notify();
            // back to what was searched for: nothing new to search
            (filter.query != query).then_some(filter.debounce)
        });
        let Some(debounce) = debounce else {
            return;
        };
        if query.is_empty() {
            Self::clear_history_filter(id, cx);
            return;
        }
        let state = Self::state(cx);
        cx.spawn(async move |cx: &mut AsyncApp| {
            cx.background_executor()
                .timer(HISTORY_FILTER_DEBOUNCE)
                .await;
            cx.update(|cx| {
                let current = state
                    .read(cx)
                    .repo_states
                    .get(&id)
                    .map(|rs| rs.history_filter.debounce);
                if current == Some(debounce) {
                    Self::run_history_filter(id, query, cx);
                }
            });
        })
        .detach();
    }

    /// Drop the filter: the plain History list and its selection come back.
    pub fn clear_history_filter(id: u64, cx: &mut App) {
        let reselect = Self::state(cx).update(cx, |s, cx| {
            let rs = s.repo_state_mut(id);
            let was_active = rs.history_filter.is_active();
            let filter = &mut rs.history_filter;
            if let Some(cancel) = filter.cancel.take() {
                cancel.cancel();
            }
            let text = std::mem::take(&mut filter.text);
            *filter = HistoryFilter {
                generation: filter.generation + 1,
                debounce: filter.debounce + 1,
                ..HistoryFilter::default()
            };
            // keep text typed after a term was removed (only spaces left)
            filter.text = if text.trim().is_empty() {
                String::new()
            } else {
                text
            };
            cx.notify();
            if !was_active || rs.compare.is_comparing() {
                return None;
            }
            let kept = !rs.selected_commits.is_empty()
                && rs
                    .selected_commits
                    .iter()
                    .all(|sha| rs.commits.iter().any(|c| &c.sha == sha));
            if kept {
                None
            } else {
                Some(rs.commits.first().map(|c| c.sha.clone()))
            }
        });
        if let Some(first) = reselect {
            Self::select_commits(id, first.into_iter().collect(), cx);
        }
    }

    /// Run the search for `query` (the box's current text) in the background.
    fn run_history_filter(id: u64, query: HistoryQuery, cx: &mut App) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let first_parent = Self::history_first_parent(Self::state(cx).read(cx));
        let cancel = CancelToken::new();
        let generation = Self::state(cx).update(cx, |s, cx| {
            let filter = &mut s.repo_state_mut(id).history_filter;
            if let Some(previous) = filter.cancel.replace(cancel.clone()) {
                previous.cancel();
            }
            filter.query = query.clone();
            filter.loading = true;
            filter.generation += 1;
            cx.notify();
            filter.generation
        });
        let task = cx.background_executor().spawn(async move {
            let Some(tip) = corvene_git::resolve_commit(&workdir, "HEAD")? else {
                return Ok((None, Arc::new(Vec::new()), Vec::new(), Vec::new(), 0));
            };
            let logged = Arc::new(corvene_git::filtered_history(
                git,
                &workdir,
                &tip,
                &query,
                first_parent,
                Some(cancel),
            )?);
            let mut commits = match query.sha_prefix() {
                Some(prefix) => corvene_git::commits_with_sha_prefix(
                    &workdir,
                    &logged,
                    prefix,
                    SHA_PREFIX_MATCHES,
                )?,
                None => Vec::new(),
            };
            let sha_matches: Vec<String> = commits.iter().map(|c| c.sha.clone()).collect();
            let (page, next) = corvene_git::filtered_history_page(
                &workdir,
                &logged,
                0,
                &query.words,
                &sha_matches,
                corvene_git::COMMIT_BATCH_SIZE,
            )?;
            commits.extend(page);
            Ok::<_, corvene_git::GitError>((Some(tip), logged, sha_matches, commits, next))
        });
        cx.spawn(async move |cx: &mut AsyncApp| {
            let result = task.await;
            cx.update(|cx| {
                let select = Self::state(cx).update(cx, |s, cx| {
                    let rs = s.repo_state_mut(id);
                    let filter = &mut rs.history_filter;
                    if filter.generation != generation {
                        return None;
                    }
                    filter.loading = false;
                    filter.cancel = None;
                    filter.searched = true;
                    match result {
                        Ok((tip, logged, sha_matches, commits, next)) => {
                            filter.exhausted = next >= logged.len();
                            filter.tip = tip;
                            filter.logged = Some(logged);
                            filter.next = next;
                            filter.sha_matches = sha_matches;
                            filter.commits = commits;
                        }
                        Err(err) => {
                            warn!(id, %err, "history filter failed");
                            filter.exhausted = true;
                            filter.logged = None;
                            filter.commits.clear();
                        }
                    }
                    cx.notify();
                    if rs.compare.is_comparing() {
                        return None;
                    }
                    let commits = &rs.history_filter.commits;
                    let kept = !rs.selected_commits.is_empty()
                        && rs
                            .selected_commits
                            .iter()
                            .all(|sha| commits.iter().any(|c| &c.sha == sha));
                    if kept {
                        None
                    } else {
                        Some(commits.first().map(|c| c.sha.clone()))
                    }
                });
                if let Some(first) = select {
                    Self::select_commits(id, first.into_iter().collect(), cx);
                }
            });
        })
        .detach();
    }

    /// The filtered list scrolled near its end: build the next page.
    pub fn load_more_history_filter(id: u64, cx: &mut App) {
        let Some((_, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let request = Self::state(cx).update(cx, |s, _| {
            let filter = &mut s.repo_state_mut(id).history_filter;
            if filter.loading || filter.exhausted || !filter.is_active() {
                return None;
            }
            let logged = filter.logged.clone()?;
            filter.loading = true;
            Some((
                logged,
                filter.next,
                filter.query.words.clone(),
                filter.sha_matches.clone(),
                filter.generation,
            ))
        });
        let Some((logged, start, words, skip, generation)) = request else {
            return;
        };
        let task = cx.background_executor().spawn(async move {
            let page = corvene_git::filtered_history_page(
                &workdir,
                &logged,
                start,
                &words,
                &skip,
                corvene_git::COMMIT_BATCH_SIZE,
            );
            (page, logged.len())
        });
        cx.spawn(async move |cx: &mut AsyncApp| {
            let (result, total) = task.await;
            cx.update(|cx| {
                Self::state(cx).update(cx, |s, cx| {
                    let filter = &mut s.repo_state_mut(id).history_filter;
                    if filter.generation != generation {
                        return;
                    }
                    filter.loading = false;
                    match result {
                        Ok((page, next)) => {
                            filter.next = next;
                            filter.exhausted = next >= total;
                            filter.commits.extend(page);
                        }
                        Err(err) => {
                            warn!(id, %err, "history filter page failed");
                            filter.exhausted = true;
                        }
                    }
                    cx.notify();
                });
            });
        })
        .detach();
    }

    /// After History reloaded: search again when HEAD moved (or `force`, when
    /// what History lists changed, like first-parent mode).
    pub(crate) fn refresh_history_filter(id: u64, force: bool, cx: &mut App) {
        let query = {
            let s = Self::state(cx).read(cx);
            let Some(rs) = s.repo_states.get(&id) else {
                return;
            };
            let filter = &rs.history_filter;
            let moved = rs.commits.first().map(|c| &c.sha) != filter.tip.as_ref();
            if !filter.is_active() || !(force || moved && filter.searched) {
                return;
            }
            filter.query.clone()
        };
        Self::run_history_filter(id, query, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terms_and_words_are_split() {
        let q = parse_history_query("Fix  author:mona before:2024-05-01 after:2.weeks.ago LOGIN");
        assert_eq!(q.words, vec!["fix".to_string(), "login".to_string()]);
        assert_eq!(q.author.as_deref(), Some("mona"));
        assert_eq!(q.before.as_deref(), Some("2024-05-01"));
        assert_eq!(q.after.as_deref(), Some("2.weeks.ago"));
    }

    #[test]
    fn quotes_keep_spaces_and_empty_terms_are_ignored() {
        let q = parse_history_query("author:\"Mona Lisa\" \"fix login\" before:");
        assert_eq!(q.author.as_deref(), Some("Mona Lisa"));
        assert_eq!(q.words, vec!["fix login".to_string()]);
        assert_eq!(q.before, None);
        assert!(parse_history_query("   ").is_empty());
        assert!(parse_history_query("author:").is_empty());
    }

    #[test]
    fn keys_ignore_case() {
        let q = parse_history_query("Author:Hubot");
        assert_eq!(q.author.as_deref(), Some("Hubot"));
        assert!(q.words.is_empty());
    }
}
