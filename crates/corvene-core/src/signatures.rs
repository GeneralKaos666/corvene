//! Commit signature status (`1214-commit-signatures`, `1215-verify-visible-
//! signatures`; Corvene addition, GHD `ui/history/commit-summary.tsx`
//! shows none).
//!
//! Whether a commit is signed is known from the History walk
//! ([`Commit::signature`](corvene_models::Commit::signature)). Verifying it
//! is lazy: [`Dispatcher::touch_signature`] is called while rendering the
//! selected commit (and, with `1215`, the visible History rows) and queues
//! what the cache lacks:
//!
//! - local git (`corvene_git::signature::verify_commit`, which runs `gpg`,
//!   `gpgsm` or `ssh-keygen`), at most [`MAX_LOCAL_IN_FLIGHT`] at a time,
//!   the selected commit first, each cut off after [`LOCAL_TIMEOUT`];
//! - GitHub's verdict for a pushed commit of a GitHub repository with a
//!   signed-in account, batched into one GraphQL query: at once for the
//!   selected commit, [`GITHUB_DEBOUNCE`] after the last row asked
//!   otherwise, so a scroll sends one query once it settles.
//!
//! The badge shows GitHub's verdict when it has one (it matches github.com
//! and knows keys this machine does not, such as `web-flow`'s), else the
//! local one. Verdicts that may change once a key is imported or an allowed
//! signers file is set are forgotten when the app regains focus.

use std::collections::{HashMap, VecDeque};
use std::time::Duration;

use corvene_git::CancelToken;
use corvene_github::Client;
use corvene_models::{
    GitHubSignature, LocalSignature, SignatureKind, SignatureReason, SignatureState,
};

use crate::dispatcher::Dispatcher;
use crate::host::{AsyncCtx, Host};
use crate::remote::spawn_bg;

/// Cached commits per repository; the least recently touched go first.
pub const MAX_ENTRIES: usize = 4096;
/// Local verifications running at once (the selected commit may add one).
pub const MAX_LOCAL_IN_FLIGHT: usize = 2;
/// A verifier still running after this (a gpg-agent waiting on something)
/// is killed and the signature shown as unreadable.
pub const LOCAL_TIMEOUT: Duration = Duration::from_secs(15);
/// Rows wait this long after the last touch before GitHub is asked.
pub const GITHUB_DEBOUNCE: Duration = Duration::from_millis(300);

/// Who touched a commit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Priority {
    /// The selected commit's header: verified first, GitHub asked at once.
    Selected,
    /// A visible History row (`1215-verify-visible-signatures`).
    Row,
}

/// Where a verdict came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Local,
    GitHub,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum LocalSlot {
    #[default]
    Missing,
    /// Queued or running.
    Loading,
    Done(LocalSignature),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum GitHubSlot {
    #[default]
    Missing,
    Loading,
    /// `None`: GitHub has the commit unsigned.
    Done(Option<GitHubSignature>),
    /// Not on GitHub, no account or token, or the request failed.
    Unavailable,
}

#[derive(Clone, Debug, Default)]
pub struct SignatureEntry {
    pub local: LocalSlot,
    pub github: GitHubSlot,
    /// When it was last touched, for eviction.
    seq: u64,
}

impl SignatureEntry {
    pub fn new(local: LocalSlot, github: GitHubSlot) -> Self {
        Self {
            local,
            github,
            seq: 0,
        }
    }

    /// GitHub's verdict when it has one, else local git's.
    pub fn verdict(&self) -> Option<(SignatureState, Source)> {
        if let GitHubSlot::Done(Some(gh)) = &self.github
            && gh.state != SignatureState::Unsigned
        {
            return Some((gh.state, Source::GitHub));
        }
        match &self.local {
            LocalSlot::Done(local) => Some((local.state, Source::Local)),
            _ => None,
        }
    }

    pub fn loading(&self) -> bool {
        self.local == LocalSlot::Loading || self.github == GitHubSlot::Loading
    }

    pub fn github(&self) -> Option<&GitHubSignature> {
        match &self.github {
            GitHubSlot::Done(gh) => gh.as_ref(),
            _ => None,
        }
    }

    pub fn local(&self) -> Option<&LocalSignature> {
        match &self.local {
            LocalSlot::Done(local) => Some(local),
            _ => None,
        }
    }

    /// Drop what a new key, allowed signers file or push could change.
    fn forget_unsettled(&mut self) {
        if let LocalSlot::Done(local) = &self.local
            && matches!(
                local.state,
                SignatureState::Unverified | SignatureState::CantVerify
            )
        {
            self.local = LocalSlot::Missing;
        }
        if self.github == GitHubSlot::Unavailable {
            self.github = GitHubSlot::Missing;
        }
    }
}

/// `RepositoryState::signatures`.
#[derive(Clone, Debug, Default)]
pub struct SignatureStore {
    pub entries: HashMap<String, SignatureEntry>,
    queue: VecDeque<(String, SignatureKind, Priority)>,
    in_flight: usize,
    github_pending: Vec<String>,
    github_generation: u64,
    seq: u64,
}

impl SignatureStore {
    pub fn get(&self, sha: &str) -> Option<&SignatureEntry> {
        self.entries.get(sha)
    }

    /// Records a touch; returns (queued for local git, asked GitHub).
    fn touch(
        &mut self,
        sha: &str,
        kind: SignatureKind,
        priority: Priority,
        ask_github: bool,
    ) -> (bool, bool) {
        self.seq += 1;
        let seq = self.seq;
        let entry = self.entries.entry(sha.to_string()).or_default();
        entry.seq = seq;
        let mut queued = false;
        match entry.local {
            LocalSlot::Missing => {
                entry.local = LocalSlot::Loading;
                let item = (sha.to_string(), kind, priority);
                if priority == Priority::Selected {
                    self.queue.push_front(item);
                } else {
                    self.queue.push_back(item);
                }
                queued = true;
            }
            LocalSlot::Loading if priority == Priority::Selected => {
                // still queued behind rows: move it to the front
                if let Some(at) = self.queue.iter().position(|(s, ..)| s == sha) {
                    if let Some(mut item) = self.queue.remove(at) {
                        item.2 = Priority::Selected;
                        self.queue.push_front(item);
                    }
                    queued = true;
                }
            }
            _ => {}
        }
        let mut asked = false;
        if ask_github && entry.github == GitHubSlot::Missing {
            entry.github = GitHubSlot::Loading;
            self.github_pending.push(sha.to_string());
            asked = true;
        }
        if self.entries.len() > MAX_ENTRIES {
            self.evict();
        }
        (queued, asked)
    }

    /// Drops the least recently touched tenth that is not loading.
    fn evict(&mut self) {
        let mut seqs: Vec<(u64, String)> = self
            .entries
            .iter()
            .filter(|(_, e)| !e.loading())
            .map(|(sha, e)| (e.seq, sha.clone()))
            .collect();
        seqs.sort_unstable();
        for (_, sha) in seqs.into_iter().take(MAX_ENTRIES / 10) {
            self.entries.remove(&sha);
        }
    }

    /// The next queued verification, if one may start.
    fn next(&mut self) -> Option<(String, SignatureKind)> {
        let (_, _, priority) = self.queue.front()?;
        let cap = match priority {
            Priority::Selected => MAX_LOCAL_IN_FLIGHT + 1,
            Priority::Row => MAX_LOCAL_IN_FLIGHT,
        };
        if self.in_flight >= cap {
            return None;
        }
        let (sha, kind, _) = self.queue.pop_front()?;
        self.in_flight += 1;
        Some((sha, kind))
    }

    pub fn forget_unsettled(&mut self) {
        for entry in self.entries.values_mut() {
            entry.forget_unsettled();
        }
    }
}

impl Dispatcher {
    /// Asks for `sha`'s signature status if the cache lacks it. `kind` is
    /// the commit's [`Commit::signature`](corvene_models::Commit::signature);
    /// unsigned commits need nothing. Never notifies, so it can be called
    /// while rendering.
    pub fn touch_signature(
        id: u64,
        sha: &str,
        kind: Option<SignatureKind>,
        priority: Priority,
        cx: &mut dyn Host,
    ) {
        let Some(kind) = kind else {
            return;
        };
        let touched = Self::state(cx).update(cx, |s, _| {
            let ask_github = s
                .repository(id)
                .and_then(|r| r.github.as_ref())
                .is_some_and(|gh| s.account_for(&gh.endpoint).is_some());
            let rs = s.repo_states.get_mut(&id)?;
            let pushed = !rs.local_commits.contains(sha)
                && !rs
                    .unpublished_commits
                    .as_ref()
                    .is_some_and(|u| u.contains(sha));
            Some(
                rs.signatures
                    .touch(sha, kind, priority, ask_github && pushed),
            )
        });
        let Some((queued, asked)) = touched else {
            return;
        };
        if queued {
            Self::pump_signatures(id, cx);
        }
        if asked {
            Self::schedule_github_signatures(id, priority == Priority::Selected, cx);
        }
    }

    /// The app regained focus: re-check verdicts a new key could change.
    pub(crate) fn forget_unsettled_signatures(cx: &mut dyn Host) {
        Self::state(cx).update(cx, |s, _| {
            for rs in s.repo_states.values_mut() {
                rs.signatures.forget_unsettled();
            }
        });
    }

    /// Starts queued local verifications while there is room.
    fn pump_signatures(id: u64, cx: &mut dyn Host) {
        loop {
            let next = Self::state(cx).update(cx, |s, _| {
                s.repo_states
                    .get_mut(&id)
                    .and_then(|rs| rs.signatures.next())
            });
            let Some((sha, kind)) = next else {
                return;
            };
            Self::verify_signature(id, sha, kind, cx);
        }
    }

    fn verify_signature(id: u64, sha: String, kind: SignatureKind, cx: &mut dyn Host) {
        let context = Self::repo_context(id, cx);
        let cancel = CancelToken::new();
        let timeout = cancel.clone();
        cx.spawn(async move |cx: &mut AsyncCtx| {
            cx.background_executor().timer(LOCAL_TIMEOUT).await;
            timeout.cancel();
        })
        .detach();
        let key = sha.clone();
        spawn_bg(
            cx,
            move || {
                let (git, workdir) = context?;
                let result = corvene_git::signature::verify_commit(
                    git,
                    &workdir,
                    &sha,
                    Some(kind),
                    Some(cancel),
                );
                if let Err(err) = &result {
                    tracing::debug!(%sha, %err, "signature verification failed");
                }
                result.ok()
            },
            move |result, cx| {
                Self::state(cx).update(cx, |s, cx| {
                    let Some(rs) = s.repo_states.get_mut(&id) else {
                        return;
                    };
                    let store = &mut rs.signatures;
                    store.in_flight = store.in_flight.saturating_sub(1);
                    if let Some(entry) = store.entries.get_mut(&key) {
                        entry.local = LocalSlot::Done(result.unwrap_or_else(|| unreadable(kind)));
                    }
                    cx.notify();
                });
                Self::pump_signatures(id, cx);
            },
        );
    }

    fn schedule_github_signatures(id: u64, now: bool, cx: &mut dyn Host) {
        let generation = Self::state(cx).update(cx, |s, _| {
            let rs = s.repo_states.get_mut(&id)?;
            rs.signatures.github_generation += 1;
            Some(rs.signatures.github_generation)
        });
        let Some(generation) = generation else {
            return;
        };
        if now {
            Self::flush_github_signatures(id, None, cx);
            return;
        }
        cx.spawn(async move |cx: &mut AsyncCtx| {
            cx.background_executor().timer(GITHUB_DEBOUNCE).await;
            cx.update(|cx| Self::flush_github_signatures(id, Some(generation), cx));
        })
        .detach();
    }

    /// One GraphQL query (per 100 commits) for every pending commit;
    /// `generation` is the debounce's, `None` to send at once.
    fn flush_github_signatures(id: u64, generation: Option<u64>, cx: &mut dyn Host) {
        let batch = Self::state(cx).update(cx, |s, _| {
            let github = s.repository(id)?.github.clone()?;
            let store = &mut s.repo_states.get_mut(&id)?.signatures;
            if generation.is_some_and(|g| g != store.github_generation)
                || store.github_pending.is_empty()
            {
                return None;
            }
            Some((github, std::mem::take(&mut store.github_pending)))
        });
        let Some((github, shas)) = batch else {
            return;
        };
        let Some((endpoint, token, login)) = Self::api_for_repository(id, &github, cx) else {
            Self::apply_github_signatures(id, &shas, None, cx);
            return;
        };
        let api_base = github.endpoint.clone();
        spawn_bg(
            cx,
            move || {
                let result = Client::new(endpoint, token).commit_signatures(
                    &github.owner,
                    &github.name,
                    &shas,
                );
                (shas, result)
            },
            move |(shas, result), cx| {
                let auth_failed = result.as_ref().is_err_and(|e| e.is_token_invalidated());
                let found = match result {
                    Ok(found) => Some(found),
                    Err(err) => {
                        tracing::debug!(%err, "GitHub signature query failed");
                        None
                    }
                };
                Self::apply_github_signatures(id, &shas, found, cx);
                if auth_failed {
                    Self::token_invalidated(&api_base, &login, cx);
                }
            },
        );
    }

    fn apply_github_signatures(
        id: u64,
        shas: &[String],
        mut found: Option<HashMap<String, Option<GitHubSignature>>>,
        cx: &mut dyn Host,
    ) {
        Self::state(cx).update(cx, |s, cx| {
            let Some(rs) = s.repo_states.get_mut(&id) else {
                return;
            };
            for sha in shas {
                if let Some(entry) = rs.signatures.entries.get_mut(sha) {
                    entry.github = match found.as_mut().and_then(|f| f.remove(sha)) {
                        Some(verdict) => GitHubSlot::Done(verdict),
                        None => GitHubSlot::Unavailable,
                    };
                }
            }
            cx.notify();
        });
    }
}

/// A verification that failed or timed out.
fn unreadable(kind: SignatureKind) -> LocalSignature {
    LocalSignature {
        state: SignatureState::CantVerify,
        reason: Some(SignatureReason::Unreadable),
        kind: Some(kind),
        signer: None,
        key: None,
        fingerprint: None,
        primary_fingerprint: None,
        trust: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_jumps_the_queue_and_may_exceed_the_cap() {
        let mut store = SignatureStore::default();
        for sha in ["a", "b", "c"] {
            store.touch(sha, SignatureKind::Gpg, Priority::Row, false);
        }
        assert_eq!(store.next().map(|n| n.0), Some("a".into()));
        assert_eq!(store.next().map(|n| n.0), Some("b".into()));
        // the cap holds rows back
        assert_eq!(store.next(), None);
        let (queued, _) = store.touch("d", SignatureKind::Ssh, Priority::Selected, false);
        assert!(queued);
        assert_eq!(store.next().map(|n| n.0), Some("d".into()));
        assert_eq!(store.next(), None);
        // a touched entry is not queued twice
        let (queued, _) = store.touch("c", SignatureKind::Gpg, Priority::Row, false);
        assert!(!queued);
    }

    #[test]
    fn github_asked_once() {
        let mut store = SignatureStore::default();
        assert_eq!(
            store.touch("a", SignatureKind::Gpg, Priority::Row, true),
            (true, true)
        );
        assert_eq!(
            store.touch("a", SignatureKind::Gpg, Priority::Row, true),
            (false, false)
        );
        assert_eq!(store.github_pending, vec!["a".to_string()]);
    }

    #[test]
    fn verdict_prefers_github_and_focus_forgets_unsettled() {
        let mut entry = SignatureEntry {
            local: LocalSlot::Done(unreadable(SignatureKind::Gpg)),
            ..Default::default()
        };
        assert_eq!(
            entry.verdict(),
            Some((SignatureState::CantVerify, Source::Local))
        );
        let (state, reason) = GitHubSignature::classify(true, "VALID");
        entry.github = GitHubSlot::Done(Some(GitHubSignature {
            state,
            reason,
            github_state: "VALID".into(),
            kind: Some(SignatureKind::Gpg),
            signer_login: None,
            key: None,
            was_signed_by_github: true,
        }));
        assert_eq!(
            entry.verdict(),
            Some((SignatureState::Verified, Source::GitHub))
        );
        entry.forget_unsettled();
        assert_eq!(entry.local, LocalSlot::Missing);
        assert!(matches!(entry.github, GitHubSlot::Done(Some(_))));
    }

    #[test]
    fn eviction_keeps_loading_entries() {
        let mut store = SignatureStore::default();
        for i in 0..=MAX_ENTRIES {
            let sha = format!("{i:040x}");
            store.touch(&sha, SignatureKind::Gpg, Priority::Row, false);
            if let Some(e) = store.entries.get_mut(&sha)
                && i > 0
            {
                e.local = LocalSlot::Done(unreadable(SignatureKind::Gpg));
            }
        }
        assert!(store.entries.len() <= MAX_ENTRIES);
        assert!(store.entries.contains_key(&format!("{:040x}", 0)));
    }
}
