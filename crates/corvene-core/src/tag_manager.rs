//! Corvene `1219-tag-manager`: Branch › Tags…, every tag as a History mode.
//! GitHub Desktop shows tags only as labels on History's commits and can
//! delete the ones it created until they are pushed (`ui/history/
//! commit-list-item.tsx`, `tagsToPush`).
//!
//! [`Dispatcher::show_tags`] swaps History's commit list for the tags
//! ([`corvene_git::list_tags`], newest first). While it is open
//! [`RepositoryState::visible_commits`] lists the tagged commits, so
//! selecting a tag fills the usual commit view. The row actions reuse the
//! existing paths: checkout through the detached HEAD confirmation
//! (`899-tags-in-branch-list`), deletion through flag `826`'s
//! ([`Dispatcher::request_delete_tag`]), pushing and deleting on the remote
//! through `Dispatcher::tag_on_remote` in `remote.rs`. The list reloads
//! with History on every refresh.
//!
//! [`RepositoryState::visible_commits`]: crate::state::RepositoryState::visible_commits

use corvene_git::TagInfo;
use corvene_models::{Commit, Section};
use tracing::warn;

use crate::dispatcher::Dispatcher;
use crate::host::{AsyncCtx, Host};
use crate::state::{AppState, Popup, RepositoryState};

#[derive(Clone, Debug, Default)]
pub struct TagsViewState {
    /// Newest first.
    pub tags: Vec<TagInfo>,
    /// Each tagged commit once.
    pub commits: Vec<Commit>,
    /// The selected tag's name.
    pub selected: Option<String>,
    pub loading: bool,
    pub loaded: bool,
    pub reload_pending: bool,
    /// History's selection when the list opened, restored on close.
    pub saved_selection: Vec<String>,
}

impl TagsViewState {
    pub fn tag(&self, name: &str) -> Option<&TagInfo> {
        self.tags.iter().find(|t| t.name == name)
    }
}

/// The tags `query` matches (name, annotation or commit summary, ignoring
/// case), as indexes into `tags`.
pub fn filter_tags(tags: &[TagInfo], query: &str) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    tags.iter()
        .enumerate()
        .filter(|(_, t)| {
            query.is_empty()
                || t.name.to_lowercase().contains(&query)
                || t.message.to_lowercase().contains(&query)
                || t.summary.to_lowercase().contains(&query)
        })
        .map(|(i, _)| i)
        .collect()
}

/// The open list of the selected repository, `None` while the flag is off.
pub fn tags_of<'a>(s: &AppState, rs: &'a RepositoryState) -> Option<&'a TagsViewState> {
    if !s.flags.bool(crate::flags::ids::TAG_MANAGER) {
        return None;
    }
    rs.tags_view.as_ref()
}

impl Dispatcher {
    /// Branch › Tags…: History lists the tags.
    pub fn show_tags(id: u64, cx: &mut dyn Host) {
        if !Self::state(cx)
            .read(cx)
            .flags
            .bool(crate::flags::ids::TAG_MANAGER)
        {
            return;
        }
        Self::close_foldout(cx);
        Self::show_section(id, Section::History, cx);
        let opened = Self::state(cx).update(cx, |s, cx| {
            let rs = s.repo_state_mut(id);
            if rs.tags_view.is_some() {
                return false;
            }
            rs.tags_view = Some(TagsViewState {
                saved_selection: rs.selected_commits.clone(),
                ..Default::default()
            });
            rs.selected_commits.clear();
            rs.selected_commit = None;
            rs.changeset = None;
            rs.commit_selected_file = None;
            rs.commit_diff = None;
            cx.notify();
            true
        });
        if opened {
            Self::close_blame(id, cx);
            Self::close_recent_activity(id, cx);
            Self::close_issues(id, cx);
            Self::close_releases(id, cx);
            Self::load_tags(id, cx);
        }
    }

    /// The header's close button and Escape: back to History, its
    /// selection as it was.
    pub fn close_tags(id: u64, cx: &mut dyn Host) {
        let saved = Self::state(cx).update(cx, |s, cx| {
            let rs = s.repo_states.get_mut(&id)?;
            let view = rs.tags_view.take()?;
            rs.selected_commits.clear();
            rs.selected_commit = None;
            rs.changeset = None;
            rs.commit_selected_file = None;
            rs.commit_diff = None;
            cx.notify();
            Some(view.saved_selection)
        });
        if let Some(saved) = saved
            && !saved.is_empty()
        {
            Self::select_commits(id, saved, cx);
        }
    }

    /// Read the tags again (opened, refreshed). One load at a time; one
    /// asked for meanwhile runs after it.
    pub fn load_tags(id: u64, cx: &mut dyn Host) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let state = Self::state(cx);
        let busy = state.update(cx, |s, _| {
            let view = s.repo_state_mut(id).tags_view.as_mut()?;
            if view.loading {
                view.reload_pending = true;
                return Some(true);
            }
            view.loading = true;
            Some(false)
        });
        if busy != Some(false) {
            return;
        }
        let task = cx.background_executor().spawn(async move {
            let tags = corvene_git::list_tags(git, &workdir)?;
            let shas: Vec<String> = tags.iter().map(|t| t.sha.clone()).collect();
            let commits = corvene_git::commits_by_sha(&workdir, &shas)?;
            corvene_git::error::Result::Ok((tags, commits))
        });
        cx.spawn(async move |cx: &mut AsyncCtx| {
            let result = task.await;
            cx.update(|cx| {
                let (reload, select) = Self::state(cx).update(cx, |s, cx| {
                    let Some(view) = s.repo_state_mut(id).tags_view.as_mut() else {
                        return (false, None);
                    };
                    view.loading = false;
                    let reload = std::mem::take(&mut view.reload_pending);
                    let (tags, commits) = match result {
                        Ok(loaded) => loaded,
                        Err(err) => {
                            warn!(id, %err, "listing tags failed");
                            view.loaded = true;
                            cx.notify();
                            return (reload, None);
                        }
                    };
                    if view.loaded && view.tags == tags && view.commits == commits {
                        return (reload, None);
                    }
                    let first_load = !view.loaded;
                    view.tags = tags;
                    view.commits = commits;
                    view.loaded = true;
                    // a deleted tag's selection moves to the newest one
                    if view
                        .selected
                        .as_deref()
                        .is_some_and(|name| view.tag(name).is_none())
                    {
                        view.selected = None;
                    }
                    let select = (first_load || view.selected.is_none())
                        .then(|| view.tags.first().map(|t| t.name.clone()))
                        .flatten();
                    cx.notify();
                    (reload, select)
                });
                if let Some(name) = select {
                    Self::select_tag(id, name, cx);
                }
                if reload {
                    Self::load_tags(id, cx);
                }
            });
        })
        .detach();
    }

    /// Select tag `name`: the commit view shows its commit.
    pub fn select_tag(id: u64, name: String, cx: &mut dyn Host) {
        let sha = Self::state(cx).update(cx, |s, cx| {
            let view = s.repo_state_mut(id).tags_view.as_mut()?;
            let sha = view.tag(&name)?.sha.clone();
            if view.selected.as_deref() != Some(name.as_str()) {
                view.selected = Some(name);
                cx.notify();
            }
            Some(sha)
        });
        if let Some(sha) = sha {
            Self::select_commits(id, vec![sha], cx);
        }
    }

    /// A tag was pushed or deleted elsewhere than in `tagsToPush`'s paths:
    /// read the repository again (the open list reloads with History) and
    /// the branch list's tags (`899`).
    pub(crate) fn tags_changed(id: u64, cx: &mut dyn Host) {
        Self::refresh_repository(id, cx);
        if Self::state(cx)
            .read(cx)
            .repo_states
            .get(&id)
            .is_some_and(|rs| rs.branch_list_tags.is_some())
        {
            Self::load_branch_list_tags(id, cx);
        }
    }

    /// Delete Tag… as History and the branch list offer it: a tag created
    /// here and not pushed yet goes at once (GHD), any other asks first
    /// with the option to delete it from the remote too (`826`).
    pub fn request_delete_tag(id: u64, tag: String, cx: &mut dyn Host) {
        let unpushed = Self::state(cx)
            .read(cx)
            .repository(id)
            .is_some_and(|r| r.tags_to_push.contains(&tag));
        if unpushed {
            Self::delete_tag(id, tag, cx)
        } else {
            Self::show_popup(
                Popup::ConfirmDeletePushedTag {
                    repo: id,
                    tag,
                    remote_only: false,
                },
                cx,
            )
        }
    }

    /// Delete from Remote…: confirmed first.
    pub fn request_delete_remote_tag(id: u64, tag: String, cx: &mut dyn Host) {
        Self::show_popup(
            Popup::ConfirmDeletePushedTag {
                repo: id,
                tag,
                remote_only: true,
            },
            cx,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(name: &str, message: &str, summary: &str) -> TagInfo {
        TagInfo {
            name: name.into(),
            sha: "a".repeat(40),
            annotated: !message.is_empty(),
            seconds: 0,
            message: message.into(),
            summary: summary.into(),
        }
    }

    #[test]
    fn filters_by_name_message_and_summary() {
        let tags = vec![
            tag("v1.0.0", "First release", "Ship it"),
            tag("v2.0.0-rc", "", "Prepare two"),
            tag("nightly", "", "Fix the build"),
        ];
        assert_eq!(filter_tags(&tags, ""), vec![0, 1, 2]);
        assert_eq!(filter_tags(&tags, "V2"), vec![1]);
        assert_eq!(filter_tags(&tags, "release"), vec![0]);
        assert_eq!(filter_tags(&tags, "build"), vec![2]);
    }
}
