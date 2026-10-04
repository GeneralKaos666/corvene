//! Corvene `906-in-process-status`: the working directory status read by
//! gitoxide in-process instead of `git status --porcelain=2` (`status.rs`).
//! The result is assembled from the same pieces the porcelain parser uses
//! (`XY` codes, the submodule field, the branch headers), so everything
//! after parsing is shared. Any error (or anything gitoxide cannot answer
//! the way git does) returns `None` and the caller runs git as before.
//!
//! It must give what `status.rs` gives from git, field for field, so the
//! files go through the same `FileMap` (GHD `buildStatusMap`) in git's
//! order, and `status.rs` adds the same conflict details. Checked by the
//! tests below and by every GHD test that reads a status
//! (`corvene_test_support::get_status_or_throw`).
//!
//! With `786-worktree-rename-detection` a deleted tracked file and a similar
//! untracked file are one rename (`.R`, which porcelain v2 only prints for
//! intent-to-add entries): gitoxide's index-worktree rewrites, 50 %
//! similarity.
//!
//! Left to git: inexact or ambiguous staged renames and copies
//! ([`StagedRenames`], `status.renames=copies`), `diff.ignoreSubmodules`
//! ([`submodule_config_matches`]), untracked paths at or below a submodule
//! (a file or a plain directory in its place), submodules missing from
//! `.gitmodules` and ignored submodules whose directory is gone
//! ([`submodules_seen_alike`]), an intent-to-add entry at a path `HEAD`
//! has ([`intent_to_add_seen_alike`]), upstreams gitoxide maps
//! differently, paths that are not UTF-8, SHA-256 and reftable
//! repositories, a `.gitmodules` with conflict markers, and any gitoxide
//! error.

use std::collections::HashMap;
use std::path::Path;

use corvene_models::{AheadBehind, WorkingDirectoryStatus};
use gix::bstr::ByteSlice;
use gix::status::index_worktree::Item as WorktreeItem;
use gix::status::plumbing::index_as_worktree::{Change, Conflict, EntryStatus};

use crate::status::{IgnoreSubmodules, StatusOptions};

/// One path's porcelain v2 record before it becomes a file change.
#[derive(Default)]
struct Record {
    /// index (`X`) and worktree (`Y`) columns, `.` when unchanged
    x: Option<char>,
    y: Option<char>,
    /// a conflict's two-letter code (`UU`, `AA`, …), which replaces `XY`
    conflict: Option<&'static str>,
    /// `S<c><m><u>` for a submodule, `N...` otherwise
    sub: Option<String>,
    /// a submodule in `HEAD`, the index or the working tree: git's field is
    /// `S...` even with nothing to say about the submodule itself (a staged
    /// submodule commit, a removed submodule directory)
    gitlink: bool,
    old_path: Option<String>,
    score: Option<u8>,
    untracked: bool,
}

/// The status of `workdir`, or `None` to fall back to git.
pub(crate) fn status(
    workdir: &Path,
    options: StatusOptions,
    hide_untracked: bool,
) -> Option<WorkingDirectoryStatus> {
    status_pass(workdir, options, hide_untracked, false)
}

/// [`status`]; `track_rewrites` pairs deleted and untracked files
/// (`786-worktree-rename-detection`). gitoxide hashes every untracked file
/// for that, so it is a second pass, run only when the first one found both
/// a deleted tracked file and an untracked one.
fn status_pass(
    workdir: &Path,
    options: StatusOptions,
    hide_untracked: bool,
    track_rewrites: bool,
) -> Option<WorkingDirectoryStatus> {
    let repo = crate::handle::open_trusted(workdir)?;
    let started = std::time::Instant::now();
    let untracked = if hide_untracked {
        gix::status::UntrackedFiles::None
    } else {
        gix::status::UntrackedFiles::Files
    };
    let submodules = match options.ignore_submodules {
        IgnoreSubmodules::AsConfigured => {
            if !submodule_config_matches(&repo) {
                return None;
            }
            gix::status::Submodule::AsConfigured { check_dirty: false }
        }
        IgnoreSubmodules::Dirty => gix::status::Submodule::Given {
            ignore: gix::submodule::config::Ignore::Dirty,
            check_dirty: false,
        },
        IgnoreSubmodules::All => gix::status::Submodule::Given {
            ignore: gix::submodule::config::Ignore::All,
            check_dirty: false,
        },
    };
    let renames = tree_index_renames(&repo)?;
    let renames_off = matches!(renames, gix::status::tree_index::TrackRenames::Disabled);
    // `status.showUntrackedFiles=no` makes `status()` drop the directory
    // walk, which `untracked_files` then cannot turn back on; GHD's
    // `--untracked-files=all` lists them anyway
    let walk = (!hide_untracked)
        .then(|| repo.dirwalk_options())
        .transpose()
        .ok()?
        .map(|walk| walk.emit_untracked(gix::dir::walk::EmissionMode::Matching));
    let iter = repo
        .status(gix::progress::Discard)
        .and_then(|platform| {
            platform
                .untracked_files(untracked)
                .index_worktree_options_mut(|options| {
                    if walk.is_some() {
                        options.dirwalk_options = walk;
                    }
                })
                .index_worktree_submodules(submodules)
                // git status does not pair deleted and untracked files;
                // `786-worktree-rename-detection` does
                .index_worktree_rewrites(track_rewrites.then_some(gix::diff::Rewrites {
                    copies: None,
                    percentage: Some(0.5),
                    limit: 1000,
                    track_empty: false,
                }))
                .tree_index_track_renames(renames)
                .into_iter(Vec::<gix::bstr::BString>::new())
        })
        .map_err(|err| tracing::debug!(?err, "in-process status failed"))
        .ok()?;

    let mut records: HashMap<String, Record> = HashMap::new();
    let mut worktree_rename_sources: Vec<String> = Vec::new();
    let mut intent_to_add = Vec::new();
    let mut staged = StagedRenames::default();
    for item in iter {
        let item = item
            .map_err(|err| tracing::debug!(?err, "in-process status failed"))
            .ok()?;
        match item {
            gix::status::Item::IndexWorktree(item) => match item {
                WorktreeItem::Modification {
                    entry,
                    rela_path,
                    status,
                    ..
                } => {
                    let path = rela_path.to_str().ok()?.to_string();
                    let record = records.entry(path.clone()).or_default();
                    record.gitlink |= entry.mode == gix::index::entry::Mode::COMMIT;
                    // also when the file is gone again, which git shows as
                    // `.D` and gitoxide as a removal
                    if entry
                        .flags
                        .contains(gix::index::entry::Flags::INTENT_TO_ADD)
                    {
                        intent_to_add.push(path.clone());
                    }
                    match status {
                        EntryStatus::Conflict { summary, .. } => {
                            record.conflict = Some(conflict_code(summary));
                        }
                        EntryStatus::Change(change) => match change {
                            Change::Removed => record.y = Some('D'),
                            Change::Type { worktree_mode } => {
                                record.gitlink |= worktree_mode == gix::index::entry::Mode::COMMIT;
                                record.y = Some('T');
                            }
                            Change::Modification { .. } => record.y = Some('M'),
                            Change::SubmoduleModification(sub) => {
                                let field = submodule_field(&sub);
                                if field != "S..." {
                                    record.y = Some('M');
                                }
                                record.sub = Some(field);
                            }
                        },
                        EntryStatus::IntentToAdd => record.y = Some('A'),
                        EntryStatus::NeedsUpdate(_) => {}
                    }
                }
                WorktreeItem::DirectoryContents { entry, .. } => {
                    if !matches!(entry.status, gix::dir::entry::Status::Untracked) {
                        continue;
                    }
                    let mut path = entry.rela_path.to_str().ok()?.to_string();
                    // git lists an untracked repository (or an empty
                    // directory it was told to show) with a trailing slash
                    if matches!(
                        entry.disk_kind,
                        Some(gix::dir::entry::Kind::Directory | gix::dir::entry::Kind::Repository)
                    ) {
                        path.push('/');
                    }
                    records.entry(path).or_default().untracked = true;
                }
                // `786-worktree-rename-detection`: a deleted tracked file and
                // the untracked one it became (git: `.D` and `??`)
                WorktreeItem::Rewrite {
                    source,
                    dirwalk_entry,
                    diff,
                    copy,
                    ..
                } => {
                    let gix::status::index_worktree::RewriteSource::RewriteFromIndex {
                        source_entry,
                        source_rela_path,
                        ..
                    } = source
                    else {
                        return None;
                    };
                    if copy
                        || !track_rewrites
                        || source_entry.mode == gix::index::entry::Mode::COMMIT
                        || source_entry
                            .flags
                            .contains(gix::index::entry::Flags::INTENT_TO_ADD)
                    {
                        return None;
                    }
                    let path = dirwalk_entry.rela_path.to_str().ok()?.to_string();
                    let record = records.entry(path).or_default();
                    record.y = Some('R');
                    record.old_path = Some(source_rela_path.to_str().ok()?.to_string());
                    record.score = Some(
                        diff.map_or(100, |d| (d.similarity * 100.).round().clamp(0., 100.) as u8),
                    );
                    worktree_rename_sources.push(record.old_path.clone()?);
                }
            },
            gix::status::Item::TreeIndex(change) => {
                use gix::diff::index::ChangeRef;
                match change {
                    ChangeRef::Addition {
                        location,
                        entry_mode,
                        id,
                        ..
                    } => {
                        staged.added(entry_mode, &id);
                        let record = records
                            .entry(location.to_str().ok()?.to_string())
                            .or_default();
                        record.x = Some('A');
                        record.gitlink |= entry_mode == gix::index::entry::Mode::COMMIT;
                    }
                    ChangeRef::Deletion {
                        location,
                        entry_mode,
                        id,
                        ..
                    } => {
                        staged.deleted(entry_mode, &id);
                        let record = records
                            .entry(location.to_str().ok()?.to_string())
                            .or_default();
                        record.x = Some('D');
                        record.gitlink |= entry_mode == gix::index::entry::Mode::COMMIT;
                    }
                    ChangeRef::Modification {
                        location,
                        previous_entry_mode,
                        entry_mode,
                        ..
                    } => {
                        // file, symlink or submodule; an executable bit
                        // alone is git's `M`
                        let kind = |mode: gix::index::entry::Mode| {
                            mode.to_tree_entry_mode().map(|m| match m.kind() {
                                gix::object::tree::EntryKind::BlobExecutable => {
                                    gix::object::tree::EntryKind::Blob
                                }
                                kind => kind,
                            })
                        };
                        let kind_changed = kind(previous_entry_mode) != kind(entry_mode);
                        let record = records
                            .entry(location.to_str().ok()?.to_string())
                            .or_default();
                        record.x = Some(if kind_changed { 'T' } else { 'M' });
                        record.gitlink |= previous_entry_mode == gix::index::entry::Mode::COMMIT
                            || entry_mode == gix::index::entry::Mode::COMMIT;
                    }
                    ChangeRef::Rewrite {
                        source_location,
                        source_entry_mode,
                        source_id,
                        location,
                        entry_mode,
                        id,
                        copy,
                        ..
                    } => {
                        let record = records
                            .entry(location.to_str().ok()?.to_string())
                            .or_default();
                        record.x = Some(if copy { 'C' } else { 'R' });
                        record.old_path = Some(source_location.to_str().ok()?.to_string());
                        record.score = (source_id == id).then_some(100);
                        // a moved submodule (`R. S...`)
                        record.gitlink |= source_entry_mode == gix::index::entry::Mode::COMMIT
                            || entry_mode == gix::index::entry::Mode::COMMIT;
                        staged.renamed(&source_id, &id);
                    }
                }
            }
        }
    }
    if !renames_off && !staged.settled() {
        return None;
    }
    // `786-worktree-rename-detection`: something to pair, a deleted file
    // and an untracked one
    if options.worktree_renames
        && !track_rewrites
        && records.values().any(|r| r.untracked)
        && records
            .values()
            .any(|r| r.y == Some('D') && !r.gitlink && r.conflict.is_none())
    {
        return status_pass(workdir, options, hide_untracked, true);
    }
    // a renamed file with staged changes of its own is left to git (two
    // entries would describe one path)
    if worktree_rename_sources.iter().any(|source| {
        records
            .get(source)
            .is_some_and(|r| r.x.is_some() || r.y.is_some())
    }) {
        return None;
    }
    for record in records.values_mut() {
        // gitoxide's walk also lists the working file of a conflict without
        // our side (`DU`, `UA`) as untracked; git only as unmerged
        record.untracked &= record.conflict.is_none();
    }
    if !submodules_seen_alike(&repo, &records, options.ignore_submodules)
        || !intent_to_add_seen_alike(&repo, &intent_to_add)
    {
        return None;
    }
    // an intent-to-add entry is an empty blob in the index: git shows `.A`
    for path in intent_to_add {
        if let Some(record) = records.get_mut(&path)
            && record.x == Some('A')
        {
            record.x = None;
        }
    }

    let mut status = WorkingDirectoryStatus::default();
    head_info(&repo, &mut status)?;
    // git's order: changed files, then unmerged ones, then untracked ones,
    // each by path bytewise; `FileMap` keeps it (GHD `buildStatusMap`)
    let mut records: Vec<(String, Record)> = records.into_iter().collect();
    records.sort_unstable_by(|(a, ra), (b, rb)| {
        (ra.untracked, ra.conflict.is_some(), a).cmp(&(rb.untracked, rb.conflict.is_some(), b))
    });
    let mut files = crate::status::FileMap::default();
    for (path, record) in records {
        if record.untracked {
            files.push(&path, None, "??", "N...", None);
            continue;
        }
        let sub = record
            .sub
            .unwrap_or_else(|| if record.gitlink { "S..." } else { "N..." }.to_string());
        let code = match record.conflict {
            Some(code) => code.to_string(),
            None => {
                if record.x.is_none() && record.y.is_none() {
                    continue;
                }
                format!("{}{}", record.x.unwrap_or('.'), record.y.unwrap_or('.'))
            }
        };
        files.push(&path, record.old_path, &code, &sub, record.score);
    }
    files.finish(&mut status);
    tracing::debug!(
        ms = started.elapsed().as_millis() as u64,
        files = status.files.len(),
        "in-process status"
    );
    Some(status)
}

/// Whether gitoxide sees the index's submodules as git does. Left to git:
/// an untracked path at or below a submodule (a file in its place is git's
/// type change `.T S...`, and git lists nothing inside a directory there
/// that is not a repository; gitoxide's walk lists both as untracked); a
/// submodule without an entry in `.gitmodules` unless submodules are
/// ignored (git still reads its checkout, `S.MU`, gitoxide does not); a
/// submodule whose directory is gone while it is ignored (`all`, by
/// [`IgnoreSubmodules::All`] or its own setting): git hides the removal
/// (git `diff.c` `diff_addremove`), gitoxide lists it (`.D`).
fn submodules_seen_alike(
    repo: &gix::Repository,
    records: &HashMap<String, Record>,
    ignore: IgnoreSubmodules,
) -> bool {
    let Ok(index) = repo.index_or_empty() else {
        return false;
    };
    let mut gitlinks = std::collections::HashSet::new();
    for entry in index.entries() {
        if entry.mode == gix::index::entry::Mode::COMMIT {
            let Ok(path) = entry.path(&index).to_str() else {
                return false;
            };
            gitlinks.insert(path);
        }
    }
    if gitlinks.is_empty() {
        return true;
    }
    let at_or_below_gitlink = |path: &str| {
        let path = path.trim_end_matches('/');
        gitlinks.contains(path)
            || path
                .match_indices('/')
                .any(|(ix, _)| gitlinks.contains(&path[..ix]))
    };
    if records
        .iter()
        .any(|(path, r)| r.untracked && at_or_below_gitlink(path))
    {
        return false;
    }
    let removed = || {
        records
            .iter()
            .filter(|(_, r)| r.gitlink && r.y == Some('D'))
    };
    if ignore == IgnoreSubmodules::All {
        return removed().next().is_none();
    }
    // each submodule's path and own `ignore`
    let Ok(submodules) = repo.submodules() else {
        return false;
    };
    let mut configured: HashMap<String, Option<gix::submodule::config::Ignore>> = HashMap::new();
    for submodule in submodules.into_iter().flatten() {
        let (Ok(path), Ok(own)) = (submodule.path(), submodule.ignore()) else {
            return false;
        };
        let Ok(path) = path.to_str() else {
            return false;
        };
        configured.insert(path.to_string(), own);
    }
    gitlinks.iter().all(|path| configured.contains_key(*path))
        && (ignore != IgnoreSubmodules::AsConfigured
            || removed().all(|(path, _)| {
                configured.get(path) != Some(&Some(gix::submodule::config::Ignore::All))
            }))
}

/// Whether gitoxide reads the intent-to-add entries at `paths` as git
/// does: git leaves such an entry out of the `HEAD` → index comparison, so
/// a path `HEAD` has is deleted there (`DA`); gitoxide leaves out both
/// sides (`.A`). Such a path is left to git.
fn intent_to_add_seen_alike(repo: &gix::Repository, paths: &[String]) -> bool {
    if paths.is_empty() {
        return true;
    }
    let Ok(head) = repo.head() else {
        return false;
    };
    if head.is_unborn() {
        return true;
    }
    let Ok(tree) = repo.head_tree() else {
        return false;
    };
    paths
        .iter()
        .all(|path| matches!(tree.lookup_entry_by_path(path), Ok(None)))
}

/// Whether git's own submodule settings are ones gitoxide's `AsConfigured`
/// applies the same way: each submodule's `ignore` (repository
/// configuration, then `.gitmodules`) only. git uses
/// `diff.ignoreSubmodules` for a submodule without its own setting (git
/// `diff.c` `set_diffopt_flags_from_submodule_config`), gitoxide puts it
/// over every submodule's own setting: when it is set, git answers.
fn submodule_config_matches(repo: &gix::Repository) -> bool {
    repo.config_snapshot()
        .string("diff.ignoreSubmodules")
        .is_none()
}

/// git's rename detection between `HEAD` and the index, exact renames only:
/// gitoxide's similarity measure is not git's, so an index that may hold an
/// inexact rename is left to git ([`StagedRenames`]). `None` when git would
/// look for copies (`status.renames=copies`), which gitoxide reports
/// differently.
fn tree_index_renames(repo: &gix::Repository) -> Option<gix::status::tree_index::TrackRenames> {
    use gix::status::tree_index::TrackRenames;
    let config = repo.config_snapshot();
    let value = config
        .string("status.renames")
        .or_else(|| config.string("diff.renames"))
        .map(|v| v.to_str_lossy().to_ascii_lowercase());
    match value.as_deref() {
        Some("copies" | "copy") => None,
        Some("false" | "no" | "off" | "0") => Some(TrackRenames::Disabled),
        _ => Some(TrackRenames::Given(gix::diff::Rewrites {
            copies: None,
            percentage: None,
            limit: 0,
            // git pairs empty files too (`R100`)
            track_empty: true,
        })),
    }
}

/// Whether the exact-only rename pairing of `HEAD` → index is the one git
/// makes: no staged addition and deletion of non-empty files left unpaired
/// (git may call them an inexact rename) and no content on more than one
/// side of a rename (git picks among such candidates by name).
#[derive(Default)]
struct StagedRenames {
    added: bool,
    deleted: bool,
    sources: HashMap<gix::ObjectId, usize>,
    destinations: HashMap<gix::ObjectId, usize>,
    pairs: Vec<(gix::ObjectId, gix::ObjectId)>,
}

impl StagedRenames {
    fn is_file(mode: gix::index::entry::Mode) -> bool {
        matches!(
            mode,
            gix::index::entry::Mode::FILE
                | gix::index::entry::Mode::FILE_EXECUTABLE
                | gix::index::entry::Mode::SYMLINK
        )
    }

    fn added(&mut self, mode: gix::index::entry::Mode, id: &gix::oid) {
        if Self::is_file(mode) {
            self.added |= !id.is_empty_blob();
            *self.destinations.entry(id.to_owned()).or_default() += 1;
        }
    }

    fn deleted(&mut self, mode: gix::index::entry::Mode, id: &gix::oid) {
        if Self::is_file(mode) {
            self.deleted |= !id.is_empty_blob();
            *self.sources.entry(id.to_owned()).or_default() += 1;
        }
    }

    fn renamed(&mut self, source: &gix::oid, destination: &gix::oid) {
        *self.sources.entry(source.to_owned()).or_default() += 1;
        *self.destinations.entry(destination.to_owned()).or_default() += 1;
        self.pairs.push((source.to_owned(), destination.to_owned()));
    }

    fn settled(&self) -> bool {
        !(self.added && self.deleted)
            && self.pairs.iter().all(|(source, destination)| {
                self.sources.get(source) == Some(&1)
                    && self.destinations.get(destination) == Some(&1)
            })
    }
}

/// git's `XY` for an unmerged path (`wt-status.c`).
fn conflict_code(conflict: Conflict) -> &'static str {
    match conflict {
        Conflict::BothDeleted => "DD",
        Conflict::AddedByUs => "AU",
        Conflict::DeletedByThem => "UD",
        Conflict::AddedByThem => "UA",
        Conflict::DeletedByUs => "DU",
        Conflict::BothAdded => "AA",
        Conflict::BothModified => "UU",
    }
}

/// `S<c><m><u>`: the checked-out commit differs from the recorded one, the
/// submodule has changes to tracked files, it has untracked files.
fn submodule_field(sub: &gix::submodule::Status) -> String {
    let commit = sub.index_id.is_some()
        && sub.checked_out_head_id.is_some()
        && sub.checked_out_head_id != sub.index_id;
    let (mut modified, mut untracked) = (false, false);
    for change in sub.changes.iter().flatten() {
        match change {
            gix::status::Item::IndexWorktree(WorktreeItem::DirectoryContents { entry, .. }) => {
                if matches!(entry.status, gix::dir::entry::Status::Untracked) {
                    untracked = true;
                }
            }
            gix::status::Item::IndexWorktree(WorktreeItem::Modification {
                status: EntryStatus::NeedsUpdate(_),
                ..
            }) => {}
            _ => modified = true,
        }
    }
    format!(
        "S{}{}{}",
        if commit { 'C' } else { '.' },
        if modified { 'M' } else { '.' },
        if untracked { 'U' } else { '.' }
    )
}

/// The `# branch.*` headers: tip, branch, upstream and ahead/behind.
fn head_info(repo: &gix::Repository, status: &mut WorkingDirectoryStatus) -> Option<()> {
    let head = repo.head().ok()?;
    status.current_tip = head.id().map(|id| id.to_string());
    let Some(name) = head.referent_name().map(|name| name.to_owned()) else {
        // detached
        return Some(());
    };
    status.branch = Some(name.shorten().to_str().ok()?.to_string());
    let short = name.shorten().to_str().ok()?.to_string();
    let config = repo.config_snapshot();
    let merge = config.string(format!("branch.{short}.merge").as_str());
    let remote = config.string(format!("branch.{short}.remote").as_str());
    let tracking: gix::refs::FullName = match (&merge, &remote) {
        (None, _) => return Some(()),
        // `branch.<name>.remote = .`: the upstream is a local branch
        (Some(merge), Some(remote)) if remote.as_slice() == b"." => {
            merge.as_bstr().try_into().ok()?
        }
        _ => match repo
            .branch_remote_tracking_ref_name(name.as_ref(), gix::remote::Direction::Fetch)
        {
            Some(Ok(tracking)) => tracking,
            // configured, but not the way gitoxide maps it: let git say
            _ => return None,
        },
    };
    status.upstream = Some(tracking.shorten().to_str().ok()?.to_string());
    let (Some(tip), Some(upstream)) = (
        head.id().map(|id| id.detach()),
        repo.try_find_reference(tracking.as_ref())
            .ok()
            .flatten()
            .and_then(|mut r| r.peel_to_id().ok().map(|id| id.detach())),
    ) else {
        return Some(());
    };
    status.ahead_behind = Some(ahead_behind(repo, tip, upstream)?);
    Some(())
}

/// `rev-list --left-right --count tip...upstream`.
fn ahead_behind(
    repo: &gix::Repository,
    tip: gix::ObjectId,
    upstream: gix::ObjectId,
) -> Option<AheadBehind> {
    if tip == upstream {
        return Some(AheadBehind::default());
    }
    let count = |from: gix::ObjectId, hidden: gix::ObjectId| -> Option<u64> {
        let walk = repo.rev_walk([from]).with_hidden([hidden]).all().ok()?;
        let mut n = 0u64;
        for info in walk {
            info.ok()?;
            n += 1;
        }
        Some(n)
    };
    Some(AheadBehind {
        ahead: count(tip, upstream)? as _,
        behind: count(upstream, tip)? as _,
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::Command;
    use std::sync::Arc;

    use super::*;
    use crate::status::get_status_with;

    fn git(dir: &Path, args: &[&str]) -> bool {
        Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "T")
            .env("GIT_AUTHOR_EMAIL", "t@example.com")
            .env("GIT_COMMITTER_NAME", "T")
            .env("GIT_COMMITTER_EMAIL", "t@example.com")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap()
            .status
            .success()
    }

    fn run(dir: &Path, args: &[&str]) {
        assert!(git(dir, args), "git {args:?}");
    }

    fn init(dir: &Path) {
        run(dir, &["init", "-q", "-b", "main"]);
        run(dir, &["config", "commit.gpgsign", "false"]);
        run(dir, &["config", "protocol.file.allow", "always"]);
    }

    #[test]
    fn worktree_renames_pair_a_deleted_and_an_untracked_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        init(root);
        let text: String = (1..=20).map(|i| format!("line {i}\n")).collect();
        std::fs::write(root.join("a.txt"), &text).unwrap();
        std::fs::write(root.join("keep.txt"), "keep\n").unwrap();
        run(root, &["add", "."]);
        run(root, &["commit", "-q", "-m", "init"]);
        std::fs::remove_file(root.join("a.txt")).unwrap();
        std::fs::write(root.join("b.txt"), text.replace("line 7\n", "line seven\n")).unwrap();
        let git_binary = Arc::new(crate::find_git().unwrap());
        let options = StatusOptions {
            in_process: true,
            worktree_renames: true,
            ..Default::default()
        };
        let status = get_status_with(git_binary.clone(), root, options).unwrap();
        assert_eq!(status.files.len(), 1, "{:?}", summary(&status));
        let file = &status.files[0];
        assert_eq!(file.path, "b.txt");
        assert_eq!(file.old_path.as_deref(), Some("a.txt"));
        assert_eq!(file.status.kind, corvene_models::FileStatusKind::Renamed);
        assert_eq!(file.status.code, ".R");
        // the diff: the old blob against the working file
        let diff = crate::diff::working_directory_diff(
            git_binary.clone(),
            root,
            file,
            false,
            false,
            false,
            None,
        )
        .unwrap();
        let corvene_models::Diff::Text { hunks, .. } = diff else {
            panic!("{diff:?}");
        };
        assert_eq!(hunks.len(), 1);
        // committing it stages both paths
        crate::commit::unstage_all(git_binary.clone(), root).unwrap();
        crate::commit::stage_files(git_binary.clone(), root, &status.files).unwrap();
        crate::commit::commit(
            git_binary.clone(),
            root,
            "rename\n",
            &crate::commit::CommitOptions::default(),
        )
        .unwrap();
        let tree = Command::new("git")
            .args(["ls-tree", "--name-only", "HEAD"])
            .current_dir(root)
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&tree.stdout), "b.txt\nkeep.txt\n");
        // off (git's view): a deletion and an untracked file
        std::fs::rename(root.join("b.txt"), root.join("c.txt")).unwrap();
        let off = get_status_with(
            git_binary,
            root,
            StatusOptions {
                in_process: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(off.files.len(), 2, "{:?}", summary(&off));
    }

    /// What both implementations must agree on: per file, in git's order,
    /// the path, `XY`, submodule field and source path; the branch headers.
    fn summary(status: &WorkingDirectoryStatus) -> Vec<String> {
        let mut out: Vec<String> = status
            .files
            .iter()
            .map(|f| {
                let sub = f.status.submodule_status.map(|s| {
                    format!(
                        "S{}{}{}",
                        if s.commit_changed { 'C' } else { '.' },
                        if s.modified_changes { 'M' } else { '.' },
                        if s.untracked_changes { 'U' } else { '.' }
                    )
                });
                format!(
                    "{} {} {:?} {:?} {:?}",
                    f.path, f.status.code, sub, f.old_path, f.status.kind
                )
            })
            .collect();
        out.push(format!(
            "branch={:?} tip={:?} upstream={:?} ab={:?}",
            status.branch,
            status.current_tip,
            status.upstream,
            status.ahead_behind.map(|ab| (ab.ahead, ab.behind))
        ));
        out
    }

    /// The in-process status ([`crate::status::get_status_in_process`])
    /// and git's, both finished by `status.rs`; `None` when gitoxide left
    /// the status to git.
    fn both(
        dir: &Path,
        options: StatusOptions,
    ) -> (WorkingDirectoryStatus, Option<WorkingDirectoryStatus>) {
        let git = Arc::new(crate::find_git().unwrap());
        let options = StatusOptions {
            in_process: false,
            ..options
        };
        let cli = get_status_with(git.clone(), dir, options).unwrap();
        let gix = crate::status::get_status_in_process(git, dir, options);
        (cli, gix)
    }

    /// Everything must be equal: files in git's order with their codes,
    /// selections, scores and conflict details, the branch headers and
    /// [`WorkingDirectoryStatus::hidden_index_entries`]. The summary
    /// first, for a readable failure.
    #[track_caller]
    fn assert_equal(gix: &WorkingDirectoryStatus, cli: &WorkingDirectoryStatus, dir: &Path) {
        assert_eq!(summary(gix), summary(cli), "in {}", dir.display());
        assert_eq!(gix, cli, "in {}", dir.display());
    }

    #[track_caller]
    fn assert_same(dir: &Path, options: StatusOptions) -> WorkingDirectoryStatus {
        let (cli, gix) = both(dir, options);
        assert_equal(&gix.expect("in-process status"), &cli, dir);
        cli
    }

    /// `true`: gitoxide answered and agreed with git; `false`: it left the
    /// status to git.
    #[track_caller]
    fn compare(dir: &Path, options: StatusOptions) -> bool {
        let (cli, gix) = both(dir, options);
        let Some(gix) = gix else {
            return false;
        };
        assert_equal(&gix, &cli, dir);
        true
    }

    #[test]
    fn matches_git_status() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        std::fs::create_dir(&root).unwrap();
        init(&root);
        // a clean repository, unborn first
        assert_same(&root, StatusOptions::default());
        for (name, body) in [
            ("a.txt", "one\n"),
            ("b.txt", "two\n"),
            ("gone.txt", "bye\n"),
            ("staged-gone.txt", "bye\n"),
            (
                "move-me.txt",
                "the same content for a rename\nline two\nline three\n",
            ),
            ("exec.sh", "#!/bin/sh\n"),
            ("link-target.txt", "t\n"),
            ("dir with space/ü.txt", "unicode\n"),
            ("conflict.txt", "base\n"),
        ] {
            let path = root.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, body).unwrap();
        }
        std::fs::write(root.join(".gitignore"), "ignored.log\nbuild/\n").unwrap();
        run(&root, &["add", "."]);
        run(&root, &["commit", "-q", "-m", "init"]);
        assert_same(&root, StatusOptions::default());

        // conflict on a side branch
        run(&root, &["checkout", "-q", "-b", "side"]);
        std::fs::write(root.join("conflict.txt"), "side\n").unwrap();
        run(&root, &["commit", "-q", "-am", "side"]);
        run(&root, &["checkout", "-q", "main"]);
        std::fs::write(root.join("conflict.txt"), "main\n").unwrap();
        run(&root, &["commit", "-q", "-am", "main"]);
        assert!(!git(&root, &["merge", "-q", "side"]));

        std::fs::write(root.join("a.txt"), "changed\n").unwrap();
        std::fs::write(root.join("b.txt"), "staged\n").unwrap();
        run(&root, &["add", "b.txt"]);
        std::fs::write(root.join("b.txt"), "staged then changed\n").unwrap();
        std::fs::remove_file(root.join("gone.txt")).unwrap();
        run(&root, &["rm", "-q", "staged-gone.txt"]);
        run(&root, &["mv", "move-me.txt", "moved.txt"]);
        // empty: a staged addition of content next to the staged deletion
        // would leave the pairing to git (see `StagedRenames`)
        std::fs::write(root.join("new-staged.txt"), "").unwrap();
        run(&root, &["add", "new-staged.txt"]);
        std::fs::write(root.join("ita.txt"), "intent\n").unwrap();
        run(&root, &["add", "-N", "ita.txt"]);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(root.join("exec.sh"), std::fs::Permissions::from_mode(0o755))
                .unwrap();
            std::fs::remove_file(root.join("link-target.txt")).unwrap();
            std::os::unix::fs::symlink("a.txt", root.join("link-target.txt")).unwrap();
        }
        std::fs::create_dir_all(root.join("untracked/deep")).unwrap();
        std::fs::write(root.join("untracked/deep/x.txt"), "x\n").unwrap();
        std::fs::write(root.join("untracked/y.txt"), "y\n").unwrap();
        std::fs::write(root.join("ignored.log"), "i\n").unwrap();
        std::fs::create_dir_all(root.join("build")).unwrap();
        std::fs::write(root.join("build/out.o"), "o\n").unwrap();
        std::fs::create_dir_all(root.join("empty-dir")).unwrap();
        // an untracked repository inside
        let nested = root.join("nested");
        std::fs::create_dir(&nested).unwrap();
        init(&nested);
        std::fs::write(nested.join("n.txt"), "n\n").unwrap();
        run(&nested, &["add", "."]);
        run(&nested, &["commit", "-q", "-m", "n"]);
        assert_same(&root, StatusOptions::default());
    }

    #[test]
    fn matches_git_upstream_and_submodules() {
        let dir = tempfile::tempdir().unwrap();
        let (remote, work, sub) = (
            dir.path().join("remote.git"),
            dir.path().join("work"),
            dir.path().join("sub"),
        );
        std::fs::create_dir(&sub).unwrap();
        init(&sub);
        std::fs::write(sub.join("s.txt"), "s\n").unwrap();
        run(&sub, &["add", "."]);
        run(&sub, &["commit", "-q", "-m", "s"]);

        run(
            dir.path(),
            &[
                "init",
                "-q",
                "--bare",
                "-b",
                "main",
                remote.to_str().unwrap(),
            ],
        );
        std::fs::create_dir(&work).unwrap();
        init(&work);
        std::fs::write(work.join("a.txt"), "a\n").unwrap();
        run(&work, &["add", "."]);
        run(&work, &["commit", "-q", "-m", "a"]);
        run(
            &work,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        run(&work, &["push", "-q", "-u", "origin", "main"]);
        assert_same(&work, StatusOptions::default());

        // ahead 2, behind 1
        let other = dir.path().join("other");
        run(
            dir.path(),
            &[
                "clone",
                "-q",
                remote.to_str().unwrap(),
                other.to_str().unwrap(),
            ],
        );
        run(&other, &["config", "commit.gpgsign", "false"]);
        std::fs::write(other.join("o.txt"), "o\n").unwrap();
        run(&other, &["add", "."]);
        run(&other, &["commit", "-q", "-m", "o"]);
        run(&other, &["push", "-q"]);
        run(&work, &["fetch", "-q"]);
        for n in 0..2 {
            std::fs::write(work.join(format!("w{n}.txt")), "w\n").unwrap();
            run(&work, &["add", "."]);
            run(&work, &["commit", "-q", "-m", "w"]);
        }
        assert_same(&work, StatusOptions::default());

        // an upstream branch that is gone
        run(&work, &["checkout", "-q", "-b", "feature"]);
        run(&work, &["push", "-q", "-u", "origin", "feature"]);
        run(&work, &["push", "-q", "origin", "--delete", "feature"]);
        run(&work, &["fetch", "-q", "--prune"]);
        assert_same(&work, StatusOptions::default());
        run(&work, &["checkout", "-q", "main"]);

        // a submodule: clean, then new commit, modified and untracked inside
        run(
            &work,
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                "-q",
                sub.to_str().unwrap(),
                "lib",
            ],
        );
        run(&work, &["commit", "-q", "-m", "sub"]);
        assert_same(&work, StatusOptions::default());
        let lib = work.join("lib");
        run(&lib, &["config", "commit.gpgsign", "false"]);
        std::fs::write(lib.join("s.txt"), "changed\n").unwrap();
        assert_same(&work, StatusOptions::default());
        std::fs::write(lib.join("u.txt"), "u\n").unwrap();
        assert_same(&work, StatusOptions::default());
        run(&lib, &["commit", "-q", "-am", "moved on"]);
        assert_same(&work, StatusOptions::default());
        for ignore in [IgnoreSubmodules::Dirty, IgnoreSubmodules::All] {
            assert_same(
                &work,
                StatusOptions {
                    ignore_submodules: ignore,
                    ..Default::default()
                },
            );
        }

        // detached
        run(&work, &["checkout", "-q", "--detach", "HEAD~1"]);
        assert_same(&work, StatusOptions::default());
    }

    /// A repository with one commit of a few files.
    fn fresh(init_args: &[&str]) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        let mut args = vec!["init", "-q", "-b", "main"];
        args.extend_from_slice(init_args);
        args.push(root.to_str().unwrap());
        run(dir.path(), &args);
        run(&root, &["config", "commit.gpgsign", "false"]);
        for (name, body) in [
            ("a.txt", "a\nb\nc\n"),
            ("B.txt", "upper\n"),
            ("crlf.txt", "one\r\ntwo\r\n"),
            ("dir/x.txt", "x\n"),
            ("dir/y.txt", "y\n"),
        ] {
            let path = root.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, body).unwrap();
        }
        run(&root, &["add", "."]);
        run(&root, &["commit", "-q", "-m", "init"]);
        (dir, root)
    }

    /// The gitoxide-vs-git audit: configurations and index states where the
    /// two could disagree. Each either matches git or is left to git.
    #[test]
    fn matches_git_in_edge_configurations() {
        let o = StatusOptions::default();
        // core.fileMode=false: an exec bit change is no change
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let (_d, r) = fresh(&[]);
            run(&r, &["config", "core.fileMode", "false"]);
            std::fs::set_permissions(r.join("a.txt"), std::fs::Permissions::from_mode(0o755))
                .unwrap();
            assert!(compare(&r, o));
        }
        // assume-unchanged and skip-worktree entries hide their changes
        {
            let (_d, r) = fresh(&[]);
            run(&r, &["update-index", "--assume-unchanged", "a.txt"]);
            run(&r, &["update-index", "--skip-worktree", "dir/x.txt"]);
            std::fs::write(r.join("a.txt"), "changed\n").unwrap();
            std::fs::write(r.join("dir/x.txt"), "changed\n").unwrap();
            assert!(compare(&r, o));
        }
        // sparse checkout (cone): files outside are skip-worktree and absent
        {
            let (_d, r) = fresh(&[]);
            run(&r, &["sparse-checkout", "set", "--cone", "dir"]);
            std::fs::write(r.join("dir/y.txt"), "changed\n").unwrap();
            assert!(compare(&r, o));
            run(&r, &["sparse-checkout", "disable"]);
            // a sparse index (`dir/` outside the cone, one entry): whatever
            // gitoxide answers must be git's
            run(
                &r,
                &[
                    "sparse-checkout",
                    "set",
                    "--cone",
                    "--sparse-index",
                    "other",
                ],
            );
            std::fs::write(r.join("a.txt"), "changed\n").unwrap();
            std::fs::write(r.join("new.txt"), "new\n").unwrap();
            compare(&r, o);
            run(&r, &["sparse-checkout", "disable"]);
        }
        // a case-only rename on disk with core.ignoreCase
        {
            let (_d, r) = fresh(&[]);
            run(&r, &["config", "core.ignoreCase", "true"]);
            std::fs::rename(r.join("B.txt"), r.join("b-tmp")).unwrap();
            std::fs::rename(r.join("b-tmp"), r.join("b.txt")).unwrap();
            assert!(compare(&r, o));
        }
        // untracked names in NFD (macOS keeps what was written)
        {
            let (_d, r) = fresh(&[]);
            std::fs::write(r.join("cafe\u{301}.txt"), "nfd\n").unwrap();
            std::fs::write(r.join("na\u{ef}ve.txt"), "nfc\n").unwrap();
            assert!(compare(&r, o));
            run(&r, &["config", "core.precomposeUnicode", "false"]);
            assert!(compare(&r, o));
        }
        // line endings: autocrlf, text=auto renormalisation, a clean filter
        {
            let (_d, r) = fresh(&[]);
            run(&r, &["config", "core.autocrlf", "true"]);
            assert!(compare(&r, o));
            std::fs::write(r.join(".gitattributes"), "*.txt text=auto\n").unwrap();
            assert!(compare(&r, o));
            // `tr` is not on every Windows runner
            #[cfg(unix)]
            {
                std::fs::write(r.join(".gitattributes"), "*.txt filter=upper\n").unwrap();
                run(&r, &["config", "filter.upper.clean", "tr a-z A-Z"]);
                run(&r, &["config", "filter.upper.smudge", "cat"]);
                assert!(compare(&r, o));
            }
        }
        // index formats and extensions git may write
        for (key, value) in [
            ("index.version", "4"),
            ("core.splitIndex", "true"),
            ("core.untrackedCache", "true"),
            ("index.skipHash", "true"),
            ("feature.manyFiles", "true"),
        ] {
            let (_d, r) = fresh(&[]);
            run(&r, &["config", key, value]);
            std::fs::write(r.join("a.txt"), "changed\n").unwrap();
            std::fs::write(r.join("new.txt"), "n\n").unwrap();
            run(&r, &["add", "new.txt"]);
            git(&r, &["update-index", "-q", "--refresh"]);
            run(&r, &["status", "--porcelain"]);
            std::fs::write(r.join("untracked.txt"), "u\n").unwrap();
            assert!(compare(&r, o), "{key}={value}");
        }
        // staged renames: exact in-process, inexact or ambiguous left to git
        {
            let (_d, r) = fresh(&[]);
            run(&r, &["mv", "a.txt", "renamed.txt"]);
            assert!(compare(&r, o));
            std::fs::write(r.join("renamed.txt"), "a\nb\nc\nd\n").unwrap();
            run(&r, &["add", "renamed.txt"]);
            assert!(!compare(&r, o));
            run(&r, &["reset", "-q", "--hard"]);
            std::fs::copy(r.join("dir/x.txt"), r.join("copy.txt")).unwrap();
            run(&r, &["add", "copy.txt"]);
            run(&r, &["rm", "-q", "dir/x.txt"]);
            assert!(compare(&r, o));
            // git looks for copies: left to git; no renames: in-process
            run(&r, &["config", "status.renames", "copies"]);
            assert!(!compare(&r, o));
            run(&r, &["config", "status.renames", "false"]);
            assert!(compare(&r, o));
        }
        // empty files: git pairs a moved one too (`R100`); several alike
        // are left to git, which picks among them by name
        {
            let (_d, r) = fresh(&[]);
            std::fs::write(r.join("e1"), "").unwrap();
            std::fs::write(r.join("e2"), "").unwrap();
            run(&r, &["add", "."]);
            run(&r, &["commit", "-q", "-m", "empty"]);
            run(&r, &["mv", "e1", "moved-e1"]);
            let s = assert_same(&r, o);
            assert_eq!(s.files[0].status.code, "R.");
            run(&r, &["mv", "e2", "moved-e2"]);
            assert!(!compare(&r, o));
        }
        // a staged deletion next to an unrelated staged addition
        {
            let (_d, r) = fresh(&[]);
            run(&r, &["rm", "-q", "a.txt"]);
            std::fs::write(r.join("other.txt"), "other content\n").unwrap();
            run(&r, &["add", "other.txt"]);
            assert!(!compare(&r, o));
        }
        // an unborn branch with staged files
        {
            let dir = tempfile::tempdir().unwrap();
            init(dir.path());
            std::fs::write(dir.path().join("first.txt"), "f\n").unwrap();
            run(dir.path(), &["add", "."]);
            std::fs::write(dir.path().join("second.txt"), "s\n").unwrap();
            assert!(compare(dir.path(), o));
        }
        // tracked files under an ignored directory, ignored-only and
        // symlinked directories, a nested `.git` file
        {
            let (_d, r) = fresh(&[]);
            std::fs::write(r.join(".gitignore"), "dir/\n*.log\nonly-ignored/\n").unwrap();
            std::fs::write(r.join("dir/x.txt"), "changed\n").unwrap();
            std::fs::write(r.join("dir/new.txt"), "ignored\n").unwrap();
            std::fs::create_dir(r.join("only-ignored")).unwrap();
            std::fs::write(r.join("only-ignored/z.log"), "z\n").unwrap();
            std::fs::create_dir(r.join("mixed")).unwrap();
            std::fs::write(r.join("mixed/m.log"), "m\n").unwrap();
            std::fs::write(r.join("mixed/m.txt"), "m\n").unwrap();
            #[cfg(unix)]
            std::os::unix::fs::symlink("dir", r.join("dir-link")).unwrap();
            std::fs::create_dir(r.join("gitfile")).unwrap();
            std::fs::write(r.join("gitfile/.git"), "gitdir: /nonexistent\n").unwrap();
            assert!(compare(&r, o));
        }
        // a tracked directory replaced by a symlink to a directory with the
        // same files (git: deleted, and the link untracked)
        #[cfg(unix)]
        {
            let (d, r) = fresh(&[]);
            let elsewhere = d.path().join("elsewhere");
            std::fs::create_dir(&elsewhere).unwrap();
            std::fs::write(elsewhere.join("x.txt"), "x\n").unwrap();
            std::fs::write(elsewhere.join("y.txt"), "y\n").unwrap();
            std::fs::remove_dir_all(r.join("dir")).unwrap();
            std::os::unix::fs::symlink(&elsewhere, r.join("dir")).unwrap();
            compare(&r, o);
        }
        // a repository inside an untracked directory, and a linked worktree
        // inside the repository (both `<path>/` for git)
        {
            let (_d, r) = fresh(&[]);
            let nested = r.join("untracked/nested");
            std::fs::create_dir_all(&nested).unwrap();
            init(&nested);
            std::fs::write(r.join("untracked/file.txt"), "f\n").unwrap();
            run(&r, &["worktree", "add", "-q", "inner-wt", "-b", "inner"]);
            compare(&r, o);
        }
        // a linked worktree
        {
            let (d, r) = fresh(&[]);
            let wt = d.path().join("wt");
            run(
                &r,
                &["worktree", "add", "-q", wt.to_str().unwrap(), "-b", "wt"],
            );
            std::fs::write(wt.join("a.txt"), "in worktree\n").unwrap();
            assert!(compare(&wt, o));
        }
        // upstreams: a local branch, a non-default fetch refspec, no
        // ahead/behind wanted
        {
            let (_d, r) = fresh(&[]);
            run(&r, &["branch", "base"]);
            run(&r, &["checkout", "-q", "-b", "topic", "--track", "base"]);
            std::fs::write(r.join("t.txt"), "t\n").unwrap();
            run(&r, &["add", "."]);
            run(&r, &["commit", "-q", "-m", "t"]);
            assert!(compare(&r, o));
            run(&r, &["config", "status.aheadBehind", "false"]);
            assert!(compare(&r, o));
        }
        // a merge in progress with every conflict resolved and staged
        {
            let (_d, r) = fresh(&[]);
            run(&r, &["checkout", "-q", "-b", "side"]);
            std::fs::write(r.join("a.txt"), "side\n").unwrap();
            run(&r, &["commit", "-q", "-am", "side"]);
            run(&r, &["checkout", "-q", "main"]);
            std::fs::write(r.join("a.txt"), "main\n").unwrap();
            run(&r, &["commit", "-q", "-am", "main"]);
            assert!(!git(&r, &["merge", "-q", "side"]));
            std::fs::write(r.join("a.txt"), "resolved\n").unwrap();
            run(&r, &["add", "a.txt"]);
            assert!(compare(&r, o));
        }
        // `status.showUntrackedFiles=no`, respected (flag
        // `respect-show-untracked-files`) or not
        {
            let (_d, r) = fresh(&[]);
            std::fs::write(r.join("a.txt"), "changed\n").unwrap();
            std::fs::write(r.join("untracked.txt"), "u\n").unwrap();
            run(&r, &["config", "status.showUntrackedFiles", "no"]);
            for respect in [false, true] {
                let options = StatusOptions {
                    respect_show_untracked_files: respect,
                    ..o
                };
                assert_eq!(
                    assert_same(&r, options).files.len(),
                    2 - usize::from(respect)
                );
            }
        }
        // object formats and ref storage gitoxide may not read
        for args in [
            &["--object-format=sha256"][..],
            &["--ref-format=reftable"][..],
        ] {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path().join("repo");
            let mut full = vec!["init", "-q", "-b", "main"];
            full.extend_from_slice(args);
            full.push(root.to_str().unwrap());
            if !git(dir.path(), &full) {
                continue; // an older git
            }
            run(&root, &["config", "commit.gpgsign", "false"]);
            std::fs::write(root.join("a.txt"), "a\n").unwrap();
            run(&root, &["add", "."]);
            run(&root, &["commit", "-q", "-m", "a"]);
            std::fs::write(root.join("a.txt"), "b\n").unwrap();
            // gitoxide (sha1 build) reads neither: left to git
            assert!(!compare(&root, o));
        }
    }

    /// The rules `status.rs` follows to match GHD's `getStatus` (git's order,
    /// `buildStatusMap`, conflict details), which the in-process status
    /// must follow too.
    #[test]
    fn matches_git_on_ghd_status_rules() {
        let o = StatusOptions::default();
        let paths = |s: &WorkingDirectoryStatus| -> Vec<String> {
            s.files
                .iter()
                .map(|f| format!("{} {}", f.status.code, f.path))
                .collect()
        };
        // git's order: changed files, then unmerged ones, then untracked
        // ones, each bytewise (upper case first, `-` and `.` before `/`)
        {
            let (_d, r) = fresh(&[]);
            run(&r, &["checkout", "-q", "-b", "side"]);
            std::fs::write(r.join("dir/y.txt"), "side\n").unwrap();
            run(&r, &["commit", "-q", "-am", "side"]);
            run(&r, &["checkout", "-q", "main"]);
            std::fs::write(r.join("dir/y.txt"), "main\n").unwrap();
            run(&r, &["commit", "-q", "-am", "main"]);
            assert!(!git(&r, &["merge", "-q", "side"]));
            std::fs::write(r.join("a.txt"), "changed\n").unwrap();
            std::fs::write(r.join("B.txt"), "changed\n").unwrap();
            std::fs::write(r.join("Z.txt"), "z\n").unwrap();
            run(&r, &["add", "Z.txt"]);
            for name in ["dir-x", "dir.x", "dir/new.txt", "_u", "C.txt"] {
                std::fs::write(r.join(name), "u\n").unwrap();
            }
            let s = assert_same(&r, o);
            assert_eq!(
                paths(&s),
                [
                    ".M B.txt",
                    "A. Z.txt",
                    ".M a.txt",
                    "UU dir/y.txt",
                    "?? C.txt",
                    "?? _u",
                    "?? dir-x",
                    "?? dir.x",
                    "?? dir/new.txt"
                ]
            );
        }
        // added in the index, then deleted: left out, but noted
        {
            let (_d, r) = fresh(&[]);
            std::fs::write(r.join("gone.txt"), "g\n").unwrap();
            run(&r, &["add", "gone.txt"]);
            std::fs::remove_file(r.join("gone.txt")).unwrap();
            std::fs::write(r.join("a.txt"), "changed\n").unwrap();
            let s = assert_same(&r, o);
            assert!(s.hidden_index_entries);
            assert_eq!(paths(&s), [".M a.txt"]);
        }
        // a staged deletion and an untracked file at the same path: only
        // the untracked one, among the untracked files
        {
            let (_d, r) = fresh(&[]);
            run(&r, &["rm", "-q", "--cached", "B.txt"]);
            std::fs::write(r.join("a.txt"), "changed\n").unwrap();
            std::fs::write(r.join("0.txt"), "untracked\n").unwrap();
            let s = assert_same(&r, o);
            assert_eq!(paths(&s), [".M a.txt", "?? 0.txt", "?? B.txt"]);
        }
        // intent to add: `.A`, and `.D` once the file is gone again
        {
            let (_d, r) = fresh(&[]);
            std::fs::write(r.join("ita.txt"), "intent\n").unwrap();
            std::fs::write(r.join("ita-gone.txt"), "intent\n").unwrap();
            run(&r, &["add", "-N", "ita.txt", "ita-gone.txt"]);
            std::fs::remove_file(r.join("ita-gone.txt")).unwrap();
            let s = assert_same(&r, o);
            assert_eq!(paths(&s), [".D ita-gone.txt", ".A ita.txt"]);
            // at a path `HEAD` has: git's `DA` (the entry is left out of
            // `HEAD` against the index), gitoxide's `.A`: left to git
            run(&r, &["rm", "-q", "--cached", "a.txt"]);
            run(&r, &["add", "-N", "a.txt"]);
            assert!(!compare(&r, o));
        }
        // renames: with and without changes in the working tree
        // (`rename_includes_modifications`), a staged executable bit is
        // `M` (and a staged symlink in place of a file `T`)
        {
            let (_d, r) = fresh(&[]);
            run(&r, &["mv", "a.txt", "renamed.txt"]);
            run(&r, &["mv", "B.txt", "moved.txt"]);
            std::fs::write(r.join("moved.txt"), "upper\nand more\n").unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(
                    r.join("crlf.txt"),
                    std::fs::Permissions::from_mode(0o755),
                )
                .unwrap();
                std::fs::remove_file(r.join("dir/x.txt")).unwrap();
                std::os::unix::fs::symlink("y.txt", r.join("dir/x.txt")).unwrap();
                run(&r, &["add", "crlf.txt", "dir/x.txt"]);
            }
            let s = assert_same(&r, o);
            let moved = s.files.iter().find(|f| f.path == "moved.txt").unwrap();
            assert!(moved.status.rename_includes_modifications());
            let renamed = s.files.iter().find(|f| f.path == "renamed.txt").unwrap();
            assert!(!renamed.status.rename_includes_modifications());
            #[cfg(unix)]
            {
                let codes = paths(&s);
                assert!(codes.contains(&"M. crlf.txt".to_string()), "{codes:?}");
                assert!(codes.contains(&"T. dir/x.txt".to_string()), "{codes:?}");
            }
        }
        // untracked repositories keep git's trailing slash, which sorts
        // after `-` and `.`
        {
            let (_d, r) = fresh(&[]);
            let nested = r.join("nested");
            std::fs::create_dir(&nested).unwrap();
            init(&nested);
            std::fs::write(nested.join("n.txt"), "n\n").unwrap();
            std::fs::write(r.join("nested-a.txt"), "a\n").unwrap();
            std::fs::write(r.join("nested.txt"), "b\n").unwrap();
            let s = assert_same(&r, o);
            assert_eq!(
                paths(&s),
                ["?? nested-a.txt", "?? nested.txt", "?? nested/"]
            );
        }
        // conflicts: text with markers, resolved, binary on a merge and on
        // a rebase, deleted by them, and from popping a stash
        {
            let (_d, r) = fresh(&[]);
            let binary = |tag: u8| vec![0u8, 1, 2, tag, 0, 9];
            std::fs::write(r.join("img.bin"), binary(0)).unwrap();
            std::fs::write(r.join("doomed.txt"), "base\n").unwrap();
            run(&r, &["add", "."]);
            run(&r, &["commit", "-q", "-m", "base"]);
            run(&r, &["checkout", "-q", "-b", "side"]);
            std::fs::write(r.join("a.txt"), "side\n").unwrap();
            std::fs::write(r.join("dir/x.txt"), "side\n").unwrap();
            std::fs::write(r.join("img.bin"), binary(1)).unwrap();
            run(&r, &["rm", "-q", "doomed.txt"]);
            std::fs::write(r.join("both.txt"), "side\n").unwrap();
            run(&r, &["add", "."]);
            run(&r, &["commit", "-q", "-m", "side"]);
            run(&r, &["checkout", "-q", "main"]);
            std::fs::write(r.join("a.txt"), "main\n").unwrap();
            std::fs::write(r.join("dir/x.txt"), "main\n").unwrap();
            std::fs::write(r.join("img.bin"), binary(2)).unwrap();
            std::fs::write(r.join("doomed.txt"), "main\n").unwrap();
            std::fs::write(r.join("both.txt"), "main\n").unwrap();
            run(&r, &["add", "."]);
            run(&r, &["commit", "-q", "-m", "main"]);
            assert!(!git(&r, &["merge", "-q", "side"]));
            // one resolved in an editor, still unmerged
            std::fs::write(r.join("dir/x.txt"), "resolved\n").unwrap();
            let s = assert_same(&r, o);
            let markers: Vec<(String, Option<u32>)> = s
                .files
                .iter()
                .map(|f| {
                    (
                        format!("{} {}", f.status.code, f.path),
                        f.status.conflict_markers,
                    )
                })
                .collect();
            assert_eq!(
                markers,
                [
                    ("UU a.txt".to_string(), Some(3)),
                    ("AA both.txt".to_string(), Some(3)),
                    ("UU dir/x.txt".to_string(), Some(0)),
                    ("UD doomed.txt".to_string(), None),
                    ("UU img.bin".to_string(), None),
                ]
            );
            run(&r, &["merge", "--abort"]);
            // the same as a rebase (binary files against `REBASE_HEAD`)
            run(&r, &["checkout", "-q", "side"]);
            assert!(!git(&r, &["rebase", "-q", "main"]));
            assert_same(&r, o);
            run(&r, &["rebase", "--abort"]);
            // popping a stash onto a changed file
            run(&r, &["checkout", "-q", "main"]);
            std::fs::write(r.join("a.txt"), "stashed\n").unwrap();
            run(&r, &["stash", "-q"]);
            std::fs::write(r.join("a.txt"), "committed\n").unwrap();
            run(&r, &["commit", "-q", "-am", "later"]);
            assert!(!git(&r, &["stash", "pop", "-q"]));
            let s = assert_same(&r, o);
            assert_eq!(paths(&s), ["UU a.txt"]);
        }
        // conflicts with one side missing, whose working file gitoxide's
        // walk also finds: deleted by us, and a rename on both sides (both
        // deleted, added by us, added by them)
        {
            let (_d, r) = fresh(&[]);
            run(&r, &["checkout", "-q", "-b", "side"]);
            std::fs::write(r.join("a.txt"), "side\n").unwrap();
            run(&r, &["mv", "B.txt", "side-name.txt"]);
            run(&r, &["commit", "-q", "-am", "side"]);
            run(&r, &["checkout", "-q", "main"]);
            run(&r, &["rm", "-q", "a.txt"]);
            run(&r, &["mv", "B.txt", "main-name.txt"]);
            run(&r, &["commit", "-q", "-am", "main"]);
            assert!(!git(&r, &["merge", "-q", "side"]));
            let s = assert_same(&r, o);
            let codes = paths(&s);
            assert!(codes.contains(&"DU a.txt".to_string()), "{codes:?}");
            assert!(codes.iter().all(|c| !c.starts_with("??")), "{codes:?}");
        }
    }

    /// Submodule states git names `S...` without a change inside: staged,
    /// moved, removed, or no longer in the index. Left to git: a file or a
    /// plain directory in its place, its directory gone while it is
    /// ignored, and a repository added without `.gitmodules`.
    #[test]
    fn matches_git_on_submodule_entries() {
        let o = StatusOptions::default();
        let dir = tempfile::tempdir().unwrap();
        let (sub, work) = (dir.path().join("sub"), dir.path().join("work"));
        std::fs::create_dir(&sub).unwrap();
        init(&sub);
        std::fs::write(sub.join("s.txt"), "s\n").unwrap();
        run(&sub, &["add", "."]);
        run(&sub, &["commit", "-q", "-m", "s"]);
        std::fs::create_dir(&work).unwrap();
        init(&work);
        std::fs::write(work.join("a.txt"), "a\n").unwrap();
        run(&work, &["add", "."]);
        run(&work, &["commit", "-q", "-m", "a"]);
        run(
            &work,
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                "-q",
                sub.to_str().unwrap(),
                "lib",
            ],
        );
        let codes = |s: &WorkingDirectoryStatus| -> Vec<String> {
            s.files
                .iter()
                .map(|f| format!("{} {} {:?}", f.status.code, f.path, f.selection.kind()))
                .collect()
        };
        // staged, not yet committed
        let s = assert_same(&work, o);
        assert!(
            s.files
                .iter()
                .any(|f| f.path == "lib" && f.status.submodule)
        );
        run(&work, &["commit", "-q", "-m", "sub"]);
        // a new commit inside, then staged (nothing to commit: none
        // selected, as GHD's `buildStatusMap`)
        let lib = work.join("lib");
        run(&lib, &["config", "commit.gpgsign", "false"]);
        std::fs::write(lib.join("s.txt"), "t\n").unwrap();
        run(&lib, &["commit", "-q", "-am", "t"]);
        assert_same(&work, o);
        run(&work, &["add", "lib"]);
        let s = assert_same(&work, o);
        assert_eq!(codes(&s), ["M. lib None"]);
        // a staged submodule commit set to be ignored: git still lists it
        for value in ["all", "dirty"] {
            run(&work, &["config", "submodule.lib.ignore", value]);
            assert_same(&work, o);
            run(&work, &["config", "--unset", "submodule.lib.ignore"]);
        }
        std::fs::write(lib.join("u.txt"), "u\n").unwrap();
        assert_same(&work, o);
        std::fs::remove_file(lib.join("u.txt")).unwrap();
        run(&work, &["reset", "-q"]);
        run(&lib, &["reset", "-q", "--hard", "HEAD~1"]);
        // the directory gone, then a file in its place
        let aside = dir.path().join("lib-aside");
        std::fs::rename(&lib, &aside).unwrap();
        let s = assert_same(&work, o);
        assert_eq!(codes(&s), [".D lib All"]);
        // an empty directory in its place is no change
        std::fs::create_dir(&lib).unwrap();
        assert_eq!(codes(&assert_same(&work, o)), Vec::<String>::new());
        std::fs::remove_dir(&lib).unwrap();
        // a file: git's `.T S...`, which gitoxide lists as untracked
        std::fs::write(&lib, "file\n").unwrap();
        assert!(!compare(&work, o));
        std::fs::remove_file(&lib).unwrap();
        // a plain directory with a file: git lists nothing inside, gitoxide
        // the file as untracked
        std::fs::create_dir(&lib).unwrap();
        std::fs::write(lib.join("x.txt"), "x\n").unwrap();
        assert!(!compare(&work, o));
        std::fs::remove_dir_all(&lib).unwrap();
        // the directory gone while the submodule is ignored: git hides the
        // removal (unless only its changes are ignored), gitoxide does not
        let flagged = |ignore_submodules| StatusOptions {
            ignore_submodules,
            ..o
        };
        assert_same(&work, flagged(IgnoreSubmodules::Dirty));
        assert!(!compare(&work, flagged(IgnoreSubmodules::All)));
        for file in [None, Some(".gitmodules")] {
            let config = |args: &[&str]| {
                let mut all = vec!["config"];
                all.extend(file.map(|f| ["-f", f]).into_iter().flatten());
                all.extend_from_slice(args);
                run(&work, &all);
            };
            config(&["submodule.lib.ignore", "all"]);
            assert!(!compare(&work, o));
            config(&["submodule.lib.ignore", "dirty"]);
            assert_same(&work, o);
            config(&["--unset", "submodule.lib.ignore"]);
        }
        std::fs::rename(&aside, &lib).unwrap();
        // what the configuration hides: `diff.ignoreSubmodules` (for git
        // the default of submodules without their own setting, for
        // gitoxide over it: left to git) and `submodule.<name>.ignore` in
        // `.gitmodules` or the repository's configuration
        std::fs::write(lib.join("s.txt"), "dirty\n").unwrap();
        std::fs::write(lib.join("u.txt"), "u\n").unwrap();
        for value in ["all", "dirty", "untracked", "none"] {
            run(&work, &["config", "diff.ignoreSubmodules", value]);
            assert!(!compare(&work, o));
            run(&work, &["config", "submodule.lib.ignore", "all"]);
            assert!(!compare(&work, o));
            run(&work, &["config", "--unset", "submodule.lib.ignore"]);
            // `--ignore-submodules` wins over both
            for ignore in [IgnoreSubmodules::Dirty, IgnoreSubmodules::All] {
                assert_same(&work, flagged(ignore));
            }
        }
        run(&work, &["config", "--unset", "diff.ignoreSubmodules"]);
        for ignore in [IgnoreSubmodules::Dirty, IgnoreSubmodules::All] {
            assert_same(&work, flagged(ignore));
        }
        for value in ["all", "dirty", "untracked"] {
            run(&work, &["config", "submodule.lib.ignore", value]);
            assert_same(&work, o);
            run(&work, &["config", "--unset", "submodule.lib.ignore"]);
            run(
                &work,
                &["config", "-f", ".gitmodules", "submodule.lib.ignore", value],
            );
            assert_same(&work, o);
            run(
                &work,
                &[
                    "config",
                    "-f",
                    ".gitmodules",
                    "--unset",
                    "submodule.lib.ignore",
                ],
            );
        }
        run(&lib, &["checkout", "-q", "s.txt"]);
        std::fs::remove_file(lib.join("u.txt")).unwrap();
        // moved: git's `R. S...` (and `.gitmodules` changed)
        run(&work, &["mv", "lib", "lib2"]);
        let s = assert_same(&work, o);
        assert!(
            s.files
                .iter()
                .any(|f| f.path == "lib2" && f.status.code == "R." && f.status.submodule)
        );
        run(&work, &["mv", "lib2", "lib"]);
        assert_same(&work, o);
        // `.gitmodules` in conflict, with changes in the submodule (git
        // still lists them)
        run(&work, &["checkout", "-q", "-b", "side"]);
        run(
            &work,
            &[
                "config",
                "-f",
                ".gitmodules",
                "submodule.lib.branch",
                "side",
            ],
        );
        run(&work, &["commit", "-q", "-am", "side"]);
        run(&work, &["checkout", "-q", "main"]);
        run(
            &work,
            &[
                "config",
                "-f",
                ".gitmodules",
                "submodule.lib.branch",
                "main",
            ],
        );
        run(&work, &["commit", "-q", "-am", "main"]);
        assert!(!git(&work, &["merge", "-q", "side"]));
        std::fs::write(lib.join("s.txt"), "dirty\n").unwrap();
        // gitoxide fails on the conflict markers in `.gitmodules`: git
        assert!(!compare(&work, o));
        run(&lib, &["checkout", "-q", "s.txt"]);
        run(&work, &["merge", "--abort"]);
        // out of the index: deleted, and an untracked repository
        run(&work, &["rm", "-q", "--cached", "lib"]);
        let s = assert_same(&work, o);
        assert_eq!(codes(&s), ["D. lib All", "?? lib/ All"]);

        // a repository added without `.gitmodules`: git reads its checkout
        // (`.M S.MU` once changed inside), gitoxide finds no submodule
        // there, so it is left to git even while clean
        let inner = work.join("inner");
        std::fs::create_dir(&inner).unwrap();
        init(&inner);
        std::fs::write(inner.join("i.txt"), "i\n").unwrap();
        run(&inner, &["add", "."]);
        run(&inner, &["commit", "-q", "-m", "i"]);
        run(&work, &["add", "inner"]);
        assert!(!compare(&work, o));
        run(&work, &["commit", "-q", "-m", "inner"]);
        std::fs::write(inner.join("i.txt"), "dirty\n").unwrap();
        std::fs::write(inner.join("u.txt"), "u\n").unwrap();
        assert!(!compare(&work, o));
        assert!(!compare(&work, flagged(IgnoreSubmodules::Dirty)));
        assert_same(&work, flagged(IgnoreSubmodules::All));
    }

    /// Random edits, staging, renames and commits on a handful of paths,
    /// the status compared after each step: whatever gitoxide answers must
    /// be git's (a fixed seed, so a failure repeats).
    #[test]
    fn matches_git_on_random_changes() {
        let o = StatusOptions::default();
        let names = [
            "a", "b.txt", "c", "d/e", "d/f", "d-x", "d.x", "g/h/i", "x y", "ü",
        ];
        let contents = ["", "same\n", "same\n", "one\n", "two\nlines\n"];
        let (mut answered, mut steps) = (0, 0);
        for seed in [7u64, 1234, 98765] {
            let mut state = seed;
            let mut next = |n: usize| {
                state = state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                ((state >> 33) as usize) % n
            };
            let (_d, r) = fresh(&[]);
            for _ in 0..30 {
                let name = names[next(names.len())];
                let path = r.join(name);
                match next(11) {
                    0 | 1 => {
                        if let Some(parent) = path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        let _ = std::fs::remove_file(&path);
                        let _ = std::fs::write(&path, contents[next(contents.len())]);
                    }
                    2 => {
                        let _ = std::fs::remove_file(&path);
                    }
                    3 => {
                        git(&r, &["add", "-A", "--", name]);
                    }
                    4 => {
                        git(&r, &["rm", "-q", "--cached", "--", name]);
                    }
                    5 => {
                        git(&r, &["add", "-N", "--", name]);
                    }
                    6 => {
                        git(&r, &["mv", "--", name, names[next(names.len())]]);
                    }
                    7 => {
                        git(&r, &["add", "-A"]);
                        git(&r, &["commit", "-q", "-m", "step"]);
                    }
                    8 => {
                        git(&r, &["reset", "-q"]);
                    }
                    #[cfg(unix)]
                    9 => {
                        use std::os::unix::fs::PermissionsExt;
                        let _ =
                            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755));
                    }
                    #[cfg(unix)]
                    10 => {
                        let _ = std::fs::remove_file(&path);
                        let _ = std::os::unix::fs::symlink("a", &path);
                    }
                    _ => {}
                }
                steps += 1;
                answered += usize::from(compare(&r, o));
            }
        }
        // most states have no unpaired staged addition and deletion
        assert!(answered * 2 >= steps, "{answered} of {steps}");
    }

    /// `CORVENE_BENCH_REPO=<repo> cargo test --profile profiling -p corvene-git
    /// bench_status -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn bench_status() {
        let Some(repo) = std::env::var_os("CORVENE_BENCH_REPO") else {
            return;
        };
        let repo = std::path::PathBuf::from(repo);
        let git = Arc::new(crate::find_git().unwrap());
        let time = |in_process: bool| {
            let mut runs: Vec<u128> = (0..7)
                .map(|_| {
                    let started = std::time::Instant::now();
                    let options = StatusOptions {
                        in_process,
                        ..Default::default()
                    };
                    let status = get_status_with(git.clone(), &repo, options).unwrap();
                    assert!(!status.files.is_empty() || status.branch.is_some());
                    started.elapsed().as_micros()
                })
                .collect();
            runs.sort();
            runs
        };
        eprintln!("git status µs: {:?}", time(false));
        eprintln!("in-process µs: {:?}", time(true));
    }
}
