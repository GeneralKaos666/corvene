//! Corvene (`899-tags-in-branch-list`): tags in the branch list and
//! Repository › Fetch All Tags. GHD's branch list (`ui/branches/
//! branch-list.tsx`) holds branches only and its fetch (`lib/git/fetch.ts`)
//! brings only the tags pointing into fetched history.

use std::sync::Arc;

use gpui_kit::App;

use crate::Dispatcher;
use crate::remote::spawn_bg;

impl Dispatcher {
    /// Reads the repository's tags (name, commit) into
    /// `RepositoryState::branch_list_tags`, for the branch list's Tags group.
    pub fn load_branch_list_tags(id: u64, cx: &mut App) {
        let Some((_, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        spawn_bg(
            cx,
            move || {
                let mut tags: Vec<(String, String)> = corvene_git::get_all_tags(&workdir)
                    .map(|tags| tags.into_iter().collect())
                    .unwrap_or_default();
                tags.sort_by_key(|(name, _)| name.to_lowercase());
                tags
            },
            move |tags, cx| {
                Self::state(cx).update(cx, |s, cx| {
                    s.repo_state_mut(id).branch_list_tags = Some(Arc::new(tags));
                    cx.notify();
                });
            },
        );
    }

    /// Repository › Fetch All Tags: `git fetch --tags` from the current
    /// branch's remote (else the default remote), then a refresh.
    pub fn fetch_all_tags(id: u64, cx: &mut App) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let remote = {
            let s = Self::state(cx).read(cx);
            let info = s.repo_states.get(&id).and_then(|r| r.info.as_ref());
            info.and_then(|info| {
                let upstream = info
                    .current_branch()
                    .and_then(|b| b.upstream_remote_name())
                    .and_then(|n| info.remotes.iter().find(|r| r.name == n));
                upstream
                    .or_else(|| corvene_git::find_default_remote(&info.remotes))
                    .cloned()
            })
        };
        let Some(remote) = remote else {
            return;
        };
        Self::arm_credential_helper(&remote.url, cx);
        let askpass = Self::askpass_env(cx);
        spawn_bg(
            cx,
            move || corvene_git::fetch_tags(git, &workdir, &remote.name, askpass.as_ref()),
            move |result, cx| {
                if let Err(err) = result {
                    Self::show_error("Could not fetch tags", &err, cx);
                }
                Self::refresh_repository(id, cx);
                Self::load_branch_list_tags(id, cx);
            },
        );
    }
}
