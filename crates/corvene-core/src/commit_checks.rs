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

use crate::dispatcher::Dispatcher;
use crate::host::Host;
use crate::state::Popup;
use corvene_models::DiffSelectionType;

/// What the user already agreed to in this commit's dialogs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommitChecks {
    /// `OversizedFiles` › Commit Anyway.
    pub allow_oversized: bool,
}

impl Dispatcher {
    /// Commit the included changes, after the checks `checks` does not
    /// settle yet.
    pub fn commit_with(
        id: u64,
        summary: String,
        description: String,
        checks: CommitChecks,
        cx: &mut dyn Host,
    ) {
        if checks.allow_oversized {
            return Self::create_commit(id, summary, description, cx);
        }
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let (paths, committing) = {
            let s = Self::state(cx).read(cx);
            let rs = s.repo_states.get(&id);
            let paths: Vec<String> = rs
                .and_then(|rs| rs.status.as_deref())
                .map(|st| {
                    st.files
                        .iter()
                        .filter(|f| f.selection.kind() != DiffSelectionType::None)
                        .map(|f| f.path.clone())
                        .collect()
                })
                .unwrap_or_default();
            (paths, rs.is_some_and(|rs| rs.committing))
        };
        if committing {
            return;
        }
        if paths.is_empty() {
            return Self::create_commit(id, summary, description, cx);
        }
        let suggest_lfs = Self::state(cx)
            .read(cx)
            .flags
            .bool(crate::flags::ids::SUGGEST_LFS_TRACKING);
        Self::set_committing(id, true, cx);
        crate::remote::spawn_bg(
            cx,
            move || {
                let large =
                    corvene_git::large_file_paths(&workdir, &paths, corvene_git::RECEIVE_LIMIT);
                if large.is_empty() {
                    return (large, Vec::new());
                }
                let oversized =
                    corvene_git::files_not_tracked_by_lfs(git.clone(), &workdir, &large)
                        .unwrap_or(large);
                // `784-suggest-lfs-tracking`
                let patterns =
                    if suggest_lfs && !oversized.is_empty() && corvene_git::lfs_available(git) {
                        lfs_track_patterns(&oversized)
                    } else {
                        Vec::new()
                    };
                (oversized, patterns)
            },
            move |(oversized, lfs_patterns), cx| {
                Self::set_committing(id, false, cx);
                if oversized.is_empty() {
                    let checks = CommitChecks {
                        allow_oversized: true,
                    };
                    Self::commit_with(id, summary, description, checks, cx);
                } else {
                    Self::show_popup(
                        Popup::OversizedFiles {
                            repo: id,
                            files: oversized,
                            summary,
                            description,
                            lfs_patterns,
                        },
                        cx,
                    );
                }
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
