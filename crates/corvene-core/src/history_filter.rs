//! Corvene `886-history-search`: a filter box above History. GitHub Desktop
//! has none (`app/src/ui/history/compare.tsx` only compares branches).
//!
//! The text is parsed into free words and `author:` / `before:` /
//! `after:` terms ([`parse_history_query`]). A background `git log` lists
//! the commits of HEAD that pass the git-side terms; the words are matched
//! against the message, author name and e-mail as pages are built, and a
//! lone hex word also matches abbreviated SHAs, listed first. The log and
//! the text read for matching are kept while only the words change, so
//! typing does not run git again. While the filter is active its results
//! replace the History list
//! ([`crate::state::RepositoryState::visible_commits`]) without moving the
//! selection, so the diff does not reload on every keystroke; clearing it
//! brings the plain list and selection back.
//!
//! `887-file-history`: "Show History" on a changed or committed file narrows
//! the same search to that file (`git log --follow -- <path>`, shown as a
//! removable chip), and selecting one of its commits selects the file under
//! the name it had there.
//!
//! `1220-history-search-terms`: `committer:<name>` (`git log --committer`)
//! and `path:<file or folder>` terms, the path followed like a file history
//! (`--follow -- <path>`; the chip wins over a typed `path:`). With the
//! flag off they are free words.

use std::sync::Arc;
use std::time::Duration;

use crate::host::{AsyncCtx, Host};
use corvene_git::{CancelToken, HistoryQuery, LoggedHistory};
use corvene_models::Commit;
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
    /// `887-file-history`: History of this file only.
    pub path: Option<String>,
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
    logged: Option<Arc<LoggedHistory>>,
    /// What `logged` was listed for; a search that only changes the words
    /// reuses it.
    logged_for: Option<LogKey>,
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

/// The `git log` behind a filtered History: its tip, first-parent mode and
/// the query without its free words.
#[derive(Clone, Debug, PartialEq, Eq)]
struct LogKey {
    tip: String,
    first_parent: bool,
    query: HistoryQuery,
}

impl LogKey {
    fn new(tip: String, first_parent: bool, query: &HistoryQuery) -> Self {
        let query = HistoryQuery {
            words: Vec::new(),
            ..query.clone()
        };
        Self {
            tip,
            first_parent,
            query,
        }
    }
}

impl HistoryFilter {
    pub fn is_active(&self) -> bool {
        !self.query.is_empty()
    }

    /// `887-file-history`: the file's path in commit `sha` of a file history
    /// (or of a `path:` search, `1220-history-search-terms`).
    pub fn file_path_at(&self, sha: &str) -> Option<&str> {
        let path = self.query.path.as_deref()?;
        self.logged
            .as_ref()?
            .commits
            .iter()
            .find(|c| c.sha == sha)
            .and_then(|c| c.path.as_deref())
            .or(Some(path))
    }

    /// What the text and the file ask for (`terms`:
    /// `1220-history-search-terms`); the file history's chip wins over a
    /// typed `path:`.
    fn wanted(&self, terms: bool) -> HistoryQuery {
        let mut query = parse_history_query(&self.text, terms);
        if self.path.is_some() {
            query.path = self.path.clone();
        }
        query
    }
}

/// The terms [`parse_history_query`] knows; the last two only with
/// `1220-history-search-terms`.
const TERMS: [&str; 5] = ["author:", "before:", "after:", "committer:", "path:"];

/// Split the filter text into terms: `author:<name>`, `before:<date>` and
/// `after:<date>` (a date git understands, `2024-05-01` or `2.weeks.ago`),
/// with `terms` (`1220-history-search-terms`) also `committer:<name>` and
/// `path:<file or folder>`; everything else free words (lower case).
/// Double quotes keep spaces together (`author:"Mona Lisa"`, `"fix
/// login"`); a term without a value is ignored while it is being typed, and
/// a repeated term keeps its last value.
pub fn parse_history_query(text: &str, terms: bool) -> HistoryQuery {
    let known = if terms { &TERMS[..] } else { &TERMS[..3] };
    let mut query = HistoryQuery::default();
    for token in tokens(text) {
        let lower = token.to_lowercase();
        let term = known.iter().copied().find(|key| lower.starts_with(key));
        match term {
            Some(key) => {
                let value = token[key.len()..].trim().to_string();
                if value.is_empty() {
                    continue;
                }
                match key {
                    "author:" => query.author = Some(value),
                    "before:" => query.before = Some(value),
                    "after:" => query.after = Some(value),
                    "committer:" => query.committer = Some(value),
                    _ => query.path = Some(value.trim_start_matches("./").to_string()),
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
    pub fn set_history_filter_text(id: u64, text: String, cx: &mut dyn Host) {
        let request = Self::state(cx).update(cx, |s, _| {
            let terms = s.flags.bool(crate::flags::ids::HISTORY_SEARCH_TERMS);
            let filter = &mut s.repo_state_mut(id).history_filter;
            if filter.text == text {
                return None;
            }
            // the box shows the text itself: nothing to render until the
            // search runs
            filter.text = text;
            filter.debounce += 1;
            let query = filter.wanted(terms);
            // back to what was searched for: nothing new to search
            (filter.query != query).then_some((filter.debounce, query))
        });
        let Some((debounce, query)) = request else {
            return;
        };
        if query.is_empty() {
            Self::clear_history_filter(id, cx);
            return;
        }
        let state = Self::state(cx);
        cx.spawn(async move |cx: &mut AsyncCtx| {
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
                    Self::run_history_filter(id, query, false, cx);
                }
            });
        })
        .detach();
    }

    /// `887-file-history`: History of `path` only (with the filter box's
    /// terms), shown at once.
    pub fn show_file_history(id: u64, path: String, cx: &mut dyn Host) {
        Self::exit_compare(id, cx);
        Self::show_section(id, corvene_models::Section::History, cx);
        let query = Self::state(cx).update(cx, |s, cx| {
            let terms = s.flags.bool(crate::flags::ids::HISTORY_SEARCH_TERMS);
            let filter = &mut s.repo_state_mut(id).history_filter;
            filter.path = Some(path);
            filter.debounce += 1;
            cx.notify();
            filter.wanted(terms)
        });
        Self::run_history_filter(id, query, true, cx);
    }

    /// `887-file-history`: the chip's ×; the filter box's terms stay.
    pub fn clear_file_history(id: u64, cx: &mut dyn Host) {
        let query = Self::state(cx).update(cx, |s, cx| {
            let terms = s.flags.bool(crate::flags::ids::HISTORY_SEARCH_TERMS);
            let filter = &mut s.repo_state_mut(id).history_filter;
            filter.path = None;
            filter.debounce += 1;
            cx.notify();
            filter.wanted(terms)
        });
        if query.is_empty() {
            Self::clear_history_filter(id, cx);
        } else {
            Self::run_history_filter(id, query, true, cx);
        }
    }

    /// Drop the filter: the plain History list and its selection come back.
    pub fn clear_history_filter(id: u64, cx: &mut dyn Host) {
        let reselect = Self::state(cx).update(cx, |s, cx| {
            let rs = s.repo_state_mut(id);
            let was_active = rs.history_filter.is_active();
            let filter = &mut rs.history_filter;
            if let Some(cancel) = filter.cancel.take() {
                cancel.cancel();
            }
            let text = std::mem::take(&mut filter.text);
            // the log stays for the next search over the same commits
            *filter = HistoryFilter {
                generation: filter.generation + 1,
                debounce: filter.debounce + 1,
                logged: filter.logged.take(),
                logged_for: filter.logged_for.take(),
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
    /// `reselect`: select the first match when the selection is not among
    /// them (a file history being opened); typing keeps the selection.
    fn run_history_filter(id: u64, query: HistoryQuery, reselect: bool, cx: &mut dyn Host) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let first_parent = Self::history_first_parent(Self::state(cx).read(cx));
        let cancel = CancelToken::new();
        let (generation, cached) = Self::state(cx).update(cx, |s, cx| {
            let filter = &mut s.repo_state_mut(id).history_filter;
            if let Some(previous) = filter.cancel.replace(cancel.clone()) {
                previous.cancel();
            }
            let was_loading = filter.loading;
            filter.query = query.clone();
            filter.loading = true;
            filter.generation += 1;
            // only the empty list's message shows the search running
            if !was_loading && filter.commits.is_empty() {
                cx.notify();
            }
            let cached = filter.logged.clone().zip(filter.logged_for.clone());
            (filter.generation, cached)
        });
        let task = cx.background_executor().spawn(async move {
            let Some(tip) = corvene_git::resolve_commit(&workdir, "HEAD")? else {
                return Ok(None);
            };
            let key = LogKey::new(tip, first_parent, &query);
            let logged = match cached {
                Some((logged, for_key)) if for_key == key => logged,
                _ => Arc::new(LoggedHistory::new(corvene_git::filtered_history(
                    git,
                    &workdir,
                    &key.tip,
                    &query,
                    first_parent,
                    Some(cancel.clone()),
                )?)),
            };
            let mut commits = match query.sha_prefix() {
                Some(prefix) => corvene_git::commits_with_sha_prefix(
                    &workdir,
                    &logged.commits,
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
                Some(&cancel),
            )?;
            commits.extend(page);
            Ok::<_, corvene_git::GitError>(Some((key, logged, sha_matches, commits, next)))
        });
        cx.spawn(async move |cx: &mut AsyncCtx| {
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
                        Ok(Some((key, logged, sha_matches, commits, next))) => {
                            filter.exhausted = next >= logged.len();
                            filter.tip = Some(key.tip.clone());
                            filter.logged = Some(logged);
                            filter.logged_for = Some(key);
                            filter.next = next;
                            filter.sha_matches = sha_matches;
                            filter.commits = commits;
                        }
                        Ok(None) => {
                            filter.exhausted = true;
                            filter.tip = None;
                            filter.logged = None;
                            filter.logged_for = None;
                            filter.commits.clear();
                        }
                        Err(err) => {
                            warn!(id, %err, "history filter failed");
                            filter.exhausted = true;
                            filter.logged = None;
                            filter.logged_for = None;
                            filter.commits.clear();
                        }
                    }
                    cx.notify();
                    if rs.compare.is_comparing() {
                        return None;
                    }
                    let commits = &rs.history_filter.commits;
                    if !reselect && !rs.selected_commits.is_empty() {
                        return None;
                    }
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
    pub fn load_more_history_filter(id: u64, cx: &mut dyn Host) {
        let Some((_, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let request = Self::state(cx).update(cx, |s, _| {
            let filter = &mut s.repo_state_mut(id).history_filter;
            if filter.loading || filter.exhausted || !filter.is_active() {
                return None;
            }
            let logged = filter.logged.clone()?;
            let cancel = CancelToken::new();
            filter.cancel = Some(cancel.clone());
            filter.loading = true;
            Some((
                logged,
                filter.next,
                filter.query.words.clone(),
                filter.sha_matches.clone(),
                filter.generation,
                cancel,
            ))
        });
        let Some((logged, start, words, skip, generation, cancel)) = request else {
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
                Some(&cancel),
            );
            (page, logged.len())
        });
        cx.spawn(async move |cx: &mut AsyncCtx| {
            let (result, total) = task.await;
            cx.update(|cx| {
                Self::state(cx).update(cx, |s, cx| {
                    let filter = &mut s.repo_state_mut(id).history_filter;
                    if filter.generation != generation {
                        return;
                    }
                    filter.loading = false;
                    filter.cancel = None;
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
    pub(crate) fn refresh_history_filter(id: u64, force: bool, cx: &mut dyn Host) {
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
        Self::run_history_filter(id, query, false, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terms_and_words_are_split() {
        let q = parse_history_query(
            "Fix  author:mona before:2024-05-01 after:2.weeks.ago LOGIN",
            true,
        );
        assert_eq!(q.words, vec!["fix".to_string(), "login".to_string()]);
        assert_eq!(q.author.as_deref(), Some("mona"));
        assert_eq!(q.before.as_deref(), Some("2024-05-01"));
        assert_eq!(q.after.as_deref(), Some("2.weeks.ago"));
    }

    #[test]
    fn quotes_keep_spaces_and_empty_terms_are_ignored() {
        let q = parse_history_query("author:\"Mona Lisa\" \"fix login\" before:", true);
        assert_eq!(q.author.as_deref(), Some("Mona Lisa"));
        assert_eq!(q.words, vec!["fix login".to_string()]);
        assert_eq!(q.before, None);
        assert!(parse_history_query("   ", true).is_empty());
        assert!(parse_history_query("author:", true).is_empty());
    }

    #[test]
    fn committer_and_path_terms_need_the_flag() {
        let q = parse_history_query("committer:Octo path:\"docs/user guide.md\" fix", true);
        assert_eq!(q.committer.as_deref(), Some("Octo"));
        assert_eq!(q.path.as_deref(), Some("docs/user guide.md"));
        assert_eq!(q.words, vec!["fix".to_string()]);
        assert_eq!(
            parse_history_query("path:./src/main.rs", true)
                .path
                .as_deref(),
            Some("src/main.rs")
        );
        let off = parse_history_query("committer:octo path:src", false);
        assert_eq!(off.committer, None);
        assert_eq!(off.path, None);
        assert_eq!(
            off.words,
            vec!["committer:octo".to_string(), "path:src".to_string()]
        );
    }

    #[test]
    fn the_file_history_chip_wins_over_a_typed_path() {
        let filter = HistoryFilter {
            text: "path:docs".into(),
            path: Some("src/main.rs".into()),
            ..HistoryFilter::default()
        };
        assert_eq!(filter.wanted(true).path.as_deref(), Some("src/main.rs"));
        let typed = HistoryFilter {
            text: "path:docs".into(),
            ..HistoryFilter::default()
        };
        assert_eq!(typed.wanted(true).path.as_deref(), Some("docs"));
        assert_eq!(typed.wanted(false).path, None);
    }

    #[test]
    fn keys_ignore_case() {
        let q = parse_history_query("Author:Hubot", true);
        assert_eq!(q.author.as_deref(), Some("Hubot"));
        assert!(q.words.is_empty());
    }
}
