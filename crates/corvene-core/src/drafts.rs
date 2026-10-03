//! Corvene additions that keep the Changes tab's work across restarts (GHD
//! keeps it in memory only, `RepositoryStateCache`):
//! - `776-persist-commit-drafts`: each repository's commit summary and
//!   description, saved a moment after the last edit and cleared by a commit.

use std::time::Duration;

use gpui_kit::{App, AsyncApp};
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::dispatcher::Dispatcher;
use crate::persistence::StoreExt;

/// How long after the last edit a draft is written.
const DRAFT_SAVE_DELAY: Duration = Duration::from_secs(1);

/// A repository's unfinished commit message.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitDraft {
    pub summary: String,
    pub description: String,
}

impl CommitDraft {
    /// The draft worth keeping: `None` when nothing was typed (an empty
    /// summary with no description, or only the commit template's text).
    pub fn normalized(
        summary: &str,
        description: &str,
        template: Option<&str>,
    ) -> Option<CommitDraft> {
        let untouched_description = description.trim().is_empty()
            || template.is_some_and(|t| t.trim() == description.trim());
        if summary.trim().is_empty() && untouched_description {
            return None;
        }
        Some(CommitDraft {
            summary: summary.to_string(),
            description: description.to_string(),
        })
    }
}

impl Dispatcher {
    /// `776-persist-commit-drafts`: the commit form of `id` now holds
    /// `draft` (`None`: nothing typed); written after [`DRAFT_SAVE_DELAY`]
    /// without further edits.
    pub fn set_commit_draft(id: u64, draft: Option<CommitDraft>, cx: &mut App) {
        let state = Self::state(cx);
        let nonce = state.update(cx, |s, _| {
            if !s.flags.bool(crate::flags::ids::PERSIST_COMMIT_DRAFTS)
                || s.commit_drafts.get(&id) == draft.as_ref()
            {
                return None;
            }
            match draft {
                Some(draft) => s.commit_drafts.insert(id, draft),
                None => s.commit_drafts.remove(&id),
            };
            s.commit_drafts_nonce += 1;
            Some(s.commit_drafts_nonce)
        });
        let Some(nonce) = nonce else { return };
        cx.spawn(async move |cx: &mut AsyncApp| {
            cx.background_executor().timer(DRAFT_SAVE_DELAY).await;
            let pending = state.read_with(cx, |s, _| {
                (s.commit_drafts_nonce == nonce).then(|| (s.store.clone(), s.commit_drafts.clone()))
            });
            if let Some((store, drafts)) = pending {
                cx.background_executor()
                    .spawn(async move {
                        if let Err(err) = store.save_commit_drafts(&drafts) {
                            warn!(?err, "could not save the commit drafts");
                        }
                    })
                    .await;
            }
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drafts_skip_untouched_forms() {
        assert_eq!(CommitDraft::normalized("", "  \n", None), None);
        assert_eq!(
            CommitDraft::normalized("", "Why:\n", Some("Why:\n")),
            None,
            "the template alone is no draft"
        );
        assert_eq!(
            CommitDraft::normalized("Fix", "Why:\n", Some("Why:\n")),
            Some(CommitDraft {
                summary: "Fix".into(),
                description: "Why:\n".into()
            })
        );
        assert!(CommitDraft::normalized("", "details", None).is_some());
    }
}
