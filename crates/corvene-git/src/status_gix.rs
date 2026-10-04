//! Corvene `906-in-process-status`: the working directory status read by
//! gitoxide in-process instead of `git status --porcelain=2` (`status.rs`).
//! The result is assembled from the same pieces the porcelain parser uses
//! (`XY` codes, the submodule field, the branch headers), so everything
//! after parsing is shared. Any error (or anything gitoxide cannot answer
//! the way git does) returns `None` and the caller runs git as before.
//!
//! Differences from git kept on purpose: a rename's similarity score is only
//! known when the content is identical (`R100`); other renames carry none.

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
    let repo = crate::handle::open_trusted(workdir)?;
    let started = std::time::Instant::now();
    let untracked = if hide_untracked {
        gix::status::UntrackedFiles::None
    } else {
        gix::status::UntrackedFiles::Files
    };
    let submodules = match options.ignore_submodules {
        IgnoreSubmodules::AsConfigured => {
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
    let iter = repo
        .status(gix::progress::Discard)
        .and_then(|platform| {
            platform
                .untracked_files(untracked)
                .index_worktree_submodules(submodules)
                // git status does not pair deleted and untracked files
                .index_worktree_rewrites(None)
                .tree_index_track_renames(renames)
                .into_iter(Vec::<gix::bstr::BString>::new())
        })
        .map_err(|err| tracing::debug!(?err, "in-process status failed"))
        .ok()?;

    let mut records: HashMap<String, Record> = HashMap::new();
    let mut intent_to_add = Vec::new();
    let mut staged = StagedRenames::default();
    for item in iter {
        let item = item
            .map_err(|err| tracing::debug!(?err, "in-process status failed"))
            .ok()?;
        match item {
            gix::status::Item::IndexWorktree(item) => match item {
                WorktreeItem::Modification {
                    rela_path, status, ..
                } => {
                    let path = rela_path.to_str().ok()?.to_string();
                    let record = records.entry(path.clone()).or_default();
                    match status {
                        EntryStatus::Conflict { summary, .. } => {
                            record.conflict = Some(conflict_code(summary));
                        }
                        EntryStatus::Change(change) => match change {
                            Change::Removed => record.y = Some('D'),
                            Change::Type { .. } => record.y = Some('T'),
                            Change::Modification { .. } => record.y = Some('M'),
                            Change::SubmoduleModification(sub) => {
                                let field = submodule_field(&sub);
                                if field != "S..." {
                                    record.y = Some('M');
                                }
                                record.sub = Some(field);
                            }
                        },
                        EntryStatus::IntentToAdd => {
                            record.y = Some('A');
                            intent_to_add.push(path);
                        }
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
                // rewrites are off for the worktree
                WorktreeItem::Rewrite { .. } => return None,
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
                        records
                            .entry(location.to_str().ok()?.to_string())
                            .or_default()
                            .x = Some('A');
                    }
                    ChangeRef::Deletion {
                        location,
                        entry_mode,
                        id,
                        ..
                    } => {
                        staged.deleted(entry_mode, &id);
                        records
                            .entry(location.to_str().ok()?.to_string())
                            .or_default()
                            .x = Some('D');
                    }
                    ChangeRef::Modification {
                        location,
                        previous_entry_mode,
                        entry_mode,
                        ..
                    } => {
                        let kind_changed =
                            previous_entry_mode.to_tree_entry_mode().map(|m| m.kind())
                                != entry_mode.to_tree_entry_mode().map(|m| m.kind());
                        records
                            .entry(location.to_str().ok()?.to_string())
                            .or_default()
                            .x = Some(if kind_changed { 'T' } else { 'M' });
                    }
                    ChangeRef::Rewrite {
                        source_location,
                        source_id,
                        location,
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
                        staged.renamed(&source_id, &id);
                    }
                }
            }
        }
    }
    if !renames_off && !staged.settled() {
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
        let sub = record.sub.unwrap_or_else(|| "N...".to_string());
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
            track_empty: false,
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

    fn assert_same(dir: &Path, options: StatusOptions) {
        let git = Arc::new(crate::find_git().unwrap());
        let cli = get_status_with(git.clone(), dir, options).unwrap();
        let gix = status(dir, options, false).expect("in-process status");
        assert_eq!(summary(&gix), summary(&cli), "in {}", dir.display());
        // identical content renames carry git's score
        for file in &gix.files {
            if let Some(score) = file.status.score {
                let theirs = cli.files.iter().find(|f| f.path == file.path).unwrap();
                assert_eq!(Some(score), theirs.status.score);
            }
        }
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

    /// `Some(true)`: gitoxide answered and agreed with git; `Some(false)`:
    /// it left the status to git.
    fn compare(dir: &Path, options: StatusOptions) -> bool {
        let git = Arc::new(crate::find_git().unwrap());
        let cli = get_status_with(git.clone(), dir, options).unwrap();
        let Some(gix) = status(dir, options, false) else {
            return false;
        };
        assert_eq!(summary(&gix), summary(&cli), "in {}", dir.display());
        true
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
