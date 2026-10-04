//! The checks GHD's commit flow runs before committing (`onCreateCommit`,
//! `app/src/ui/changes/sidebar.tsx`): included files over 100 MiB
//! (`getLargeFilePaths`, `app/src/lib/large-files.ts`) that Git LFS does not
//! track open `OversizedFiles` (`app/src/ui/changes/oversized-files-warning.tsx`)
//! instead of committing; its "Commit Anyway" commits them.
//!
//! GHD runs them in the view after the commit message's own checks
//! (unknown co-authors); Corvene runs them in [`Dispatcher::commit_with`], which
//! the commit form and those dialogs call, off the main thread.
//!
//! Deviation: with `784-suggest-lfs-tracking` (and Git LFS installed) the
//! dialog can track the files' extensions in Git LFS instead
//! ([`lfs_track_patterns`], [`Dispatcher::track_in_lfs`]).
//!
//! Corvene addition (`785-embedded-repo-commit`): an included untracked
//! folder that is a git repository (GHD's `update-index` skips it, so the
//! commit leaves it out or fails with nothing added) opens
//! `AddEmbeddedRepositories`: with an `origin` it becomes a submodule, without
//! one a pointer to its commit; the commit then goes on with it.

use crate::dispatcher::Dispatcher;
use crate::host::Host;
use crate::state::Popup;
use corvene_models::DiffSelectionType;

/// What the user already agreed to in this commit's dialogs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommitChecks {
    /// `OversizedFiles` › Commit Anyway.
    pub allow_oversized: bool,
    /// Corvene `785-embedded-repo-commit`: the included nested repositories
    /// to add as submodules (or pointers) in the commit, once asked; `None`
    /// before the check (or `Some(empty)` to skip it).
    pub embedded: Option<Vec<corvene_git::EmbeddedRepository>>,
}

/// What the background check found.
struct Found {
    oversized: Vec<String>,
    lfs_patterns: Vec<String>,
    embedded: Vec<corvene_git::EmbeddedRepository>,
}

impl Dispatcher {
    /// Commit the included changes, after the checks `checks` does not
    /// settle yet.
    pub fn commit_with(
        id: u64,
        summary: String,
        description: String,
        mut checks: CommitChecks,
        cx: &mut dyn Host,
    ) {
        // `785-embedded-repo-commit` off: GHD's commit skips such folders
        if checks.embedded.is_none()
            && !Self::state(cx)
                .read(cx)
                .flags
                .bool(crate::flags::ids::EMBEDDED_REPO_COMMIT)
        {
            checks.embedded = Some(Vec::new());
        }
        if checks.allow_oversized
            && let Some(embedded) = &checks.embedded
        {
            return Self::create_commit(id, summary, description, embedded.clone(), cx);
        }
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let (paths, untracked, committing) = {
            let s = Self::state(cx).read(cx);
            let rs = s.repo_states.get(&id);
            let included: Vec<&corvene_models::WorkingDirectoryFileChange> = rs
                .and_then(|rs| rs.status.as_deref())
                .map(|st| {
                    st.files
                        .iter()
                        .filter(|f| f.selection.kind() != DiffSelectionType::None)
                        .collect()
                })
                .unwrap_or_default();
            (
                included.iter().map(|f| f.path.clone()).collect::<Vec<_>>(),
                included
                    .iter()
                    .filter(|f| f.status.kind == corvene_models::FileStatusKind::Untracked)
                    .map(|f| f.path.clone())
                    .collect::<Vec<_>>(),
                rs.is_some_and(|rs| rs.committing),
            )
        };
        if committing {
            return;
        }
        if paths.is_empty() {
            return Self::create_commit(id, summary, description, Vec::new(), cx);
        }
        let (suggest_lfs, hide_note) = {
            let s = Self::state(cx).read(cx);
            (
                s.flags.bool(crate::flags::ids::SUGGEST_LFS_TRACKING),
                s.settings.hide_embedded_repository_note,
            )
        };
        let (check_size, check_embedded) = (!checks.allow_oversized, checks.embedded.is_none());
        Self::set_committing(id, true, cx);
        crate::remote::spawn_bg(
            cx,
            move || {
                let mut found = Found {
                    oversized: Vec::new(),
                    lfs_patterns: Vec::new(),
                    embedded: Vec::new(),
                };
                if check_size {
                    let large =
                        corvene_git::large_file_paths(&workdir, &paths, corvene_git::RECEIVE_LIMIT);
                    if !large.is_empty() {
                        found.oversized =
                            corvene_git::files_not_tracked_by_lfs(git.clone(), &workdir, &large)
                                .unwrap_or(large);
                    }
                    // `784-suggest-lfs-tracking`
                    if suggest_lfs
                        && !found.oversized.is_empty()
                        && corvene_git::lfs_available(git.clone())
                    {
                        found.lfs_patterns = lfs_track_patterns(&found.oversized);
                    }
                }
                if check_embedded {
                    found.embedded = corvene_git::embedded_repositories(git, &workdir, &untracked);
                }
                found
            },
            move |found, cx| {
                Self::set_committing(id, false, cx);
                if !found.oversized.is_empty() {
                    return Self::show_popup(
                        Popup::OversizedFiles {
                            repo: id,
                            files: found.oversized,
                            summary,
                            description,
                            lfs_patterns: found.lfs_patterns,
                        },
                        cx,
                    );
                }
                checks.allow_oversized = true;
                if check_embedded {
                    // only pointers, and their note dismissed: nothing to ask
                    let ask = !found.embedded.is_empty()
                        && !(hide_note && found.embedded.iter().all(|r| r.url.is_none()));
                    if ask {
                        return Self::show_popup(
                            Popup::AddEmbeddedRepositories {
                                repo: id,
                                repositories: found.embedded,
                                commit: Some((summary, description)),
                            },
                            cx,
                        );
                    }
                    checks.embedded = Some(found.embedded);
                }
                Self::commit_with(id, summary, description, checks, cx);
            },
        );
    }

    /// Corvene `785-embedded-repo-commit`: the changes list's "Add as
    /// Submodule…" for an untracked folder `path` (`Sub/`): ask, with its
    /// `origin`, if it is a repository.
    pub fn request_add_embedded_repository(id: u64, path: String, cx: &mut dyn Host) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        crate::remote::spawn_bg(
            cx,
            move || corvene_git::embedded_repositories(git, &workdir, &[path]),
            move |repositories, cx| {
                if !repositories.is_empty() {
                    Self::show_popup(
                        Popup::AddEmbeddedRepositories {
                            repo: id,
                            repositories,
                            commit: None,
                        },
                        cx,
                    );
                }
            },
        );
    }

    /// Corvene `785-embedded-repo-commit`: add the nested repositories to the
    /// index now ([`corvene_git::add_embedded_repositories`]); the commit
    /// stages the gitlinks and `.gitmodules` again like any other change.
    pub fn add_embedded_repositories(
        id: u64,
        repositories: Vec<corvene_git::EmbeddedRepository>,
        cx: &mut dyn Host,
    ) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        crate::remote::spawn_bg(
            cx,
            move || corvene_git::add_embedded_repositories(git, &workdir, &repositories),
            move |result, cx| {
                if let Err(err) = result {
                    Self::show_error("Could not add the repository", &err, cx);
                }
                Self::refresh_repository(id, cx);
            },
        );
    }

    /// `784-suggest-lfs-tracking`: `OversizedFiles` › Track … in Git LFS:
    /// track `patterns` and turn `files` into pointers
    /// ([`corvene_git::track_in_lfs`]), then back to the commit form, where
    /// `.gitattributes` joins the changes.
    pub fn track_in_lfs(id: u64, patterns: Vec<String>, files: Vec<String>, cx: &mut dyn Host) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        crate::remote::spawn_bg(
            cx,
            move || corvene_git::track_in_lfs(git, &workdir, &patterns, &files),
            move |result, cx| {
                if let Err(err) = result {
                    Self::show_error("Could not track the files in Git LFS", &err, cx);
                }
                Self::refresh_repository(id, cx);
            },
        );
    }

    fn set_committing(id: u64, committing: bool, cx: &mut dyn Host) {
        Self::state(cx).update(cx, |s, cx| {
            s.repo_state_mut(id).committing = committing;
            cx.notify();
        });
    }
}

/// `784-suggest-lfs-tracking`: what `git lfs track` gets for `files`: `*.ext`
/// for each extension (in order, once), the path itself for a file without
/// one.
pub fn lfs_track_patterns(files: &[String]) -> Vec<String> {
    let mut patterns: Vec<String> = Vec::new();
    for file in files {
        let name = file.rsplit('/').next().unwrap_or(file);
        let pattern = match name.rsplit_once('.') {
            Some((stem, ext)) if !stem.is_empty() && !ext.is_empty() => format!("*.{ext}"),
            _ => file.clone(),
        };
        if !patterns.contains(&pattern) {
            patterns.push(pattern);
        }
    }
    patterns
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lfs_patterns_by_extension_or_path() {
        let files = [
            "videos/intro.mp4",
            "outro.mp4",
            "data/dump.tar.gz",
            "bin/blob",
            ".bigdotfile",
        ]
        .map(String::from);
        assert_eq!(
            lfs_track_patterns(&files),
            vec!["*.mp4", "*.gz", "bin/blob", ".bigdotfile"]
        );
    }
}
