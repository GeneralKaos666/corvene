//! Corvene `907-in-process-commit-files`: a commit's (or a range's) changed
//! files and line counts read by gitoxide in-process instead of
//! `git log -1 -m --first-parent -C -M --raw --numstat` / `git diff -C -M
//! --raw --numstat` (`log.rs`). Renames and copies use git's defaults (50 %
//! similarity, copies from the commit's modified files, at most 1,000
//! candidates). `None` (any error) means the caller runs git as before.
//!
//! Differences from git kept on purpose: a rename's similarity score is only
//! known when the content is identical (`R100`); other renames carry
//! gitoxide's estimate, which nothing reads.

use std::path::Path;

use corvene_models::ChangesetData;
use gix::bstr::ByteSlice;
use gix::diff::tree_with_rewrites::Change;
use gix::object::tree::EntryKind;
use gix::prelude::TreeDiffChangeExt;

/// The files changed from `oldest`'s first parent (the empty tree for a root
/// commit) to `newest`; for one commit both are its sha.
pub(crate) fn changed_files(workdir: &Path, oldest: &str, newest: &str) -> Option<ChangesetData> {
    let started = std::time::Instant::now();
    let repo = crate::handle::open(workdir).ok()?;
    let commit = |rev: &str| {
        repo.rev_parse_single(rev)
            .ok()?
            .object()
            .ok()?
            .peel_to_commit()
            .ok()
    };
    let new_tree = commit(newest)?.tree().ok()?;
    let old_tree = match commit(oldest)?.parent_ids().next() {
        Some(parent) => Some(parent.object().ok()?.peel_to_commit().ok()?.tree().ok()?),
        None => None,
    };
    let empty = repo.empty_tree();
    let old_tree = old_tree.as_ref().unwrap_or(&empty);
    let mut cache = repo.diff_resource_cache_for_tree_diff().ok()?;
    let mut changes = Vec::new();
    let outcome = gix::diff::tree_with_rewrites(
        gix::objs::TreeRefIter::from_bytes(&old_tree.data, old_tree.id.kind()),
        gix::objs::TreeRefIter::from_bytes(&new_tree.data, new_tree.id.kind()),
        &mut cache,
        &mut Default::default(),
        &repo.objects,
        |change: gix::diff::tree_with_rewrites::ChangeRef<'_>| {
            changes.push(change.into_owned());
            Ok(std::ops::ControlFlow::Continue(()))
        },
        gix::diff::tree_with_rewrites::Options {
            location: Some(gix::diff::tree::recorder::Location::Path),
            // Exact renames only: gitoxide's similarity measure is not git's
            // (a 50 % rename for one can be a delete and an add for the
            // other), so commits that may hold an inexact rename go to git
            // below. git's single `-C` reports no copies in practice (2.54)
            // and gitoxide drops the modified source of a copy: no copies.
            rewrites: Some(gix::diff::Rewrites {
                copies: None,
                percentage: None,
                limit: 0,
                ..Default::default()
            }),
        },
    )
    .map_err(|err| tracing::debug!(?err, "in-process commit files failed"))
    .ok()?;
    if outcome.is_some_and(|o| o.num_similarity_checks_skipped_for_rename_tracking_due_to_limit > 0)
        || !renames_are_settled(&changes)
    {
        return None;
    }
    cache.clear_resource_cache_keep_allocation();

    let mut data = ChangesetData::default();
    let mut files = Vec::new();
    for change in changes {
        let (letter, score, path, old_path, modes) = match &change {
            Change::Addition {
                location,
                entry_mode,
                ..
            } => ("A", None, location, None, (None, *entry_mode)),
            Change::Deletion {
                location,
                entry_mode,
                ..
            } => ("D", None, location, None, (Some(*entry_mode), *entry_mode)),
            Change::Modification {
                location,
                previous_entry_mode,
                entry_mode,
                ..
            } => {
                let letter = if kind_of(*previous_entry_mode) == kind_of(*entry_mode) {
                    "M"
                } else {
                    "T"
                };
                (
                    letter,
                    None,
                    location,
                    None,
                    (Some(*previous_entry_mode), *entry_mode),
                )
            }
            Change::Rewrite {
                source_location,
                source_entry_mode,
                location,
                entry_mode,
                diff,
                copy,
                ..
            } => (
                if *copy { "C" } else { "R" },
                Some(match diff {
                    None => 100,
                    Some(stats) => (stats.similarity * 100.0) as u8,
                }),
                location,
                Some(source_location),
                (Some(*source_entry_mode), *entry_mode),
            ),
        };
        // directories are walked, not listed
        if modes.1.is_tree() && modes.0.is_none_or(|m| m.is_tree()) {
            continue;
        }
        let is_submodule = modes.1.is_commit() || modes.0.is_some_and(|m| m.is_commit());
        let (added, deleted) = if is_submodule {
            // git counts the "Subproject commit <sha>" line on each side
            (
                u64::from(!matches!(change, Change::Deletion { .. })),
                u64::from(!matches!(change, Change::Addition { .. })),
            )
        } else {
            line_counts(&repo, &change, &mut cache)?
        };
        data.lines_added += added;
        data.lines_deleted += deleted;
        files.push(crate::log::committed_file(
            letter,
            score,
            path.to_str().ok()?.to_string(),
            match old_path {
                Some(old) => Some(old.to_str().ok()?.to_string()),
                None => None,
            },
            is_submodule,
            newest,
        ));
        cache.clear_resource_cache_keep_allocation();
    }
    // git's queue order: by (destination) path
    files.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));
    data.files = files;
    tracing::debug!(
        ms = started.elapsed().as_millis() as u64,
        files = data.files.len(),
        "in-process commit files"
    );
    Some(data)
}

/// `--numstat`'s added and deleted lines of a file change; binary files
/// (by attributes, or a NUL in the first 8000 bytes like git) count nothing.
/// gitoxide's own line counts drop line terminators, so a CRLF → LF
/// change or a final newline would count no lines where git counts them:
/// the lines are compared with their terminators here, as git's xdiff does.
fn line_counts(
    repo: &gix::Repository,
    change: &Change,
    cache: &mut gix::diff::blob::Platform,
) -> Option<(u64, u64)> {
    use gix::diff::blob::platform::prepare_diff::Operation;
    cache.options.skip_internal_diff_if_external_is_configured = false;
    let platform = change.attach(repo, repo).diff(cache).ok()?;
    let prepared = platform.resource_cache.prepare_diff().ok()?;
    match prepared.operation {
        Operation::InternalDiff { algorithm } => {
            let input = gix::diff::blob::InternedInput::new(
                prepared.old.intern_source(),
                prepared.new.intern_source(),
            );
            let diff = gix::diff::blob::Diff::compute(algorithm, &input);
            Some((
                u64::from(diff.count_additions()),
                u64::from(diff.count_removals()),
            ))
        }
        Operation::SourceOrDestinationIsBinary => Some((0, 0)),
        // a configured external diff (which git's numstat ignores) is off
        Operation::ExternalCommand { .. } => None,
    }
}

/// Whether git's rename detection could pair these changes differently from
/// gitoxide's exact-only one: an added and a deleted non-empty file left
/// unpaired (git may call them an inexact rename), or identical content on
/// more than one side of a rename (git picks among the candidates by name).
fn renames_are_settled(changes: &[Change]) -> bool {
    use std::collections::HashMap;
    let file = |mode: &gix::object::tree::EntryMode| mode.is_blob() || mode.is_link();
    let (mut added, mut deleted) = (false, false);
    let mut sources: HashMap<gix::ObjectId, usize> = HashMap::new();
    let mut destinations: HashMap<gix::ObjectId, usize> = HashMap::new();
    for change in changes {
        match change {
            Change::Addition { entry_mode, id, .. } if file(entry_mode) => {
                added |= !id.is_empty_blob();
                *destinations.entry(*id).or_default() += 1;
            }
            Change::Deletion { entry_mode, id, .. } if file(entry_mode) => {
                deleted |= !id.is_empty_blob();
                *sources.entry(*id).or_default() += 1;
            }
            Change::Rewrite { source_id, id, .. } => {
                *sources.entry(*source_id).or_default() += 1;
                *destinations.entry(*id).or_default() += 1;
            }
            _ => {}
        }
    }
    if added && deleted {
        return false;
    }
    changes.iter().all(|change| match change {
        Change::Rewrite { source_id, id, .. } => {
            sources.get(source_id) == Some(&1) && destinations.get(id) == Some(&1)
        }
        _ => true,
    })
}

/// File, symlink or submodule: a change between them is git's `T`.
fn kind_of(mode: gix::object::tree::EntryMode) -> EntryKind {
    match mode.kind() {
        EntryKind::BlobExecutable => EntryKind::Blob,
        kind => kind,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::Command;
    use std::sync::Arc;

    use super::*;

    fn run(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "T")
            .env("GIT_AUTHOR_EMAIL", "t@example.com")
            .env("GIT_COMMITTER_NAME", "T")
            .env("GIT_COMMITTER_EMAIL", "t@example.com")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    }

    fn summary(data: &ChangesetData) -> Vec<String> {
        let mut out: Vec<String> = data
            .files
            .iter()
            .map(|f| {
                format!(
                    "{} {} {:?} sub={}",
                    f.path, f.status.code, f.old_path, f.status.submodule
                )
            })
            .collect();
        out.push(format!("+{} -{}", data.lines_added, data.lines_deleted));
        out
    }

    /// `false` when gitoxide left the commit to git.
    fn assert_same(dir: &Path, shas: &[String]) -> bool {
        let git = Arc::new(crate::find_git().unwrap());
        let (cli, gix) = if shas.len() == 1 {
            (
                crate::log::get_changed_files(git, dir, &shas[0], false).unwrap(),
                changed_files(dir, &shas[0], &shas[0]),
            )
        } else {
            (
                crate::log::get_commit_range_changed_files(git, dir, shas, false).unwrap(),
                changed_files(dir, &shas[0], shas.last().unwrap()),
            )
        };
        let Some(gix) = gix else {
            return false;
        };
        assert_eq!(
            summary(&gix),
            summary(&cli),
            "{shas:?} in {}",
            dir.display()
        );
        for (a, b) in gix.files.iter().zip(&cli.files) {
            if a.status.score == Some(100) {
                assert_eq!(b.status.score, Some(100), "{}", a.path);
            }
        }
        true
    }

    fn head(dir: &Path) -> String {
        run(dir, &["rev-parse", "HEAD"]).trim().to_string()
    }

    #[test]
    fn matches_git_on_edge_cases() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        std::fs::create_dir(&root).unwrap();
        run(&root, &["init", "-q", "-b", "main"]);
        run(&root, &["config", "commit.gpgsign", "false"]);
        let lines = |n: usize, tag: &str| {
            (0..n)
                .map(|i| format!("{tag} line {i}\n"))
                .collect::<String>()
        };
        std::fs::write(root.join("a.txt"), lines(40, "a")).unwrap();
        std::fs::write(root.join("b.txt"), lines(40, "b")).unwrap();
        std::fs::write(root.join("bin.dat"), [0u8, 1, 2, 3, 0, 9]).unwrap();
        std::fs::write(root.join("exec.sh"), "#!/bin/sh\necho\n").unwrap();
        std::fs::write(root.join("empty.txt"), "").unwrap();
        std::fs::write(root.join("noeol.txt"), "one\ntwo").unwrap();
        std::fs::create_dir_all(root.join("dir/sub")).unwrap();
        std::fs::write(root.join("dir/sub/deep.txt"), lines(5, "deep")).unwrap();
        std::fs::write(root.join("dir-x.txt"), "x\n").unwrap();
        run(&root, &["add", "."]);
        run(&root, &["commit", "-q", "-m", "root"]);
        let first = head(&root);
        assert!(assert_same(&root, std::slice::from_ref(&first)));

        // modify, rename with edits, copy, delete, binary change, exec bit,
        // symlink type change, no-eol edit, a new directory
        std::fs::write(
            root.join("a.txt"),
            lines(40, "a").replace("a line 3\n", "changed\n"),
        )
        .unwrap();
        run(&root, &["mv", "b.txt", "b2.txt"]);
        std::fs::write(
            root.join("b2.txt"),
            lines(40, "b").replace("b line 7\n", "edited\n"),
        )
        .unwrap();
        std::fs::write(
            root.join("a-copy.txt"),
            lines(40, "a")
                .replace("a line 3\n", "changed\n")
                .replace("a line 9\n", "copy\n"),
        )
        .unwrap();
        std::fs::remove_file(root.join("empty.txt")).unwrap();
        std::fs::write(root.join("bin.dat"), [0u8, 7, 7, 7]).unwrap();
        std::fs::write(root.join("noeol.txt"), "one\nTWO").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(root.join("exec.sh"), std::fs::Permissions::from_mode(0o755))
                .unwrap();
            std::fs::remove_file(root.join("dir-x.txt")).unwrap();
            std::os::unix::fs::symlink("a.txt", root.join("dir-x.txt")).unwrap();
        }
        std::fs::create_dir_all(root.join("newdir/inner")).unwrap();
        std::fs::write(root.join("newdir/inner/n.txt"), "n\n").unwrap();
        run(&root, &["add", "-A"]);
        run(&root, &["commit", "-q", "-m", "second"]);
        let second = head(&root);
        // b.txt → b2.txt is an inexact rename: git decides
        assert!(!assert_same(&root, std::slice::from_ref(&second)));
        // from the root commit: additions only
        assert!(assert_same(&root, &[first.clone(), second.clone()]));
        // an exact rename and a deletion of empty files stay in-process
        run(&root, &["mv", "b2.txt", "b3.txt"]);
        run(&root, &["commit", "-q", "-m", "exact"]);
        assert!(assert_same(&root, &[head(&root)]));

        // a merge: compared with its first parent
        run(&root, &["checkout", "-q", "-b", "side", &first]);
        std::fs::write(root.join("side.txt"), "side\n").unwrap();
        run(&root, &["add", "."]);
        run(&root, &["commit", "-q", "-m", "side"]);
        run(&root, &["checkout", "-q", "main"]);
        run(&root, &["merge", "-q", "--no-edit", "side"]);
        let merge = head(&root);
        assert!(assert_same(&root, std::slice::from_ref(&merge)));
        assert!(!assert_same(&root, &[second, merge.clone()]));

        // a submodule added, moved on, removed
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        run(&sub, &["init", "-q", "-b", "main"]);
        run(&sub, &["config", "commit.gpgsign", "false"]);
        std::fs::write(sub.join("s.txt"), "s\n").unwrap();
        run(&sub, &["add", "."]);
        run(&sub, &["commit", "-q", "-m", "s"]);
        run(
            &root,
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
        run(&root, &["commit", "-q", "-m", "add sub"]);
        assert!(assert_same(&root, &[head(&root)]));
        let lib = root.join("lib");
        run(&lib, &["config", "commit.gpgsign", "false"]);
        std::fs::write(lib.join("s.txt"), "t\n").unwrap();
        run(&lib, &["commit", "-q", "-am", "t"]);
        run(&root, &["commit", "-q", "-am", "bump"]);
        assert!(assert_same(&root, &[head(&root)]));
        run(&root, &["rm", "-q", "lib"]);
        run(&root, &["commit", "-q", "-m", "drop sub"]);
        assert!(assert_same(&root, &[head(&root)]));
    }

    /// Commit `files` (name → bytes) on top of a fresh repository's first
    /// commit after `setup`, then compare that commit.
    #[track_caller]
    fn commit_and_compare(
        setup: &[(&str, &str)],
        attributes: &str,
        before: &[(&str, &[u8])],
        after: &[(&str, &[u8])],
    ) {
        let line = std::panic::Location::caller().line();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        run(root, &["init", "-q", "-b", "main"]);
        run(root, &["config", "commit.gpgsign", "false"]);
        for (key, value) in setup {
            run(root, &["config", key, value]);
        }
        std::fs::write(root.join(".gitattributes"), attributes).unwrap();
        for (name, bytes) in before {
            std::fs::write(root.join(name), bytes).unwrap();
        }
        run(root, &["add", "."]);
        run(root, &["commit", "-q", "-m", "before"]);
        for (name, bytes) in after {
            std::fs::write(root.join(name), bytes).unwrap();
        }
        run(root, &["add", "."]);
        run(root, &["commit", "-q", "-m", "after"]);
        assert!(assert_same(root, &[head(root)]), "line {line}: left to git");
    }

    /// The gitoxide-vs-git audit for commit files: diff configuration,
    /// attributes and content where line counts or kinds could differ.
    #[test]
    fn matches_git_in_edge_configurations() {
        let many = |tag: &str| {
            (0..300)
                .map(|i| format!("{tag} {}\n", i * 7 % 13))
                .collect::<String>()
        };
        let (a, b) = (many("x"), many("y"));
        let text: &[(&str, &[u8])] = &[("f.txt", b"one\ntwo\nthree\nfour\n")];
        let edited: &[(&str, &[u8])] = &[("f.txt", b"one\n2\nthree\nfour\nfive")];
        // diff algorithms git and gitoxide both know
        for algorithm in ["myers", "minimal", "patience", "histogram"] {
            commit_and_compare(
                &[("diff.algorithm", algorithm)],
                "",
                &[("f.txt", a.as_bytes())],
                &[("f.txt", b.as_bytes())],
            );
        }
        // line endings and the final newline
        commit_and_compare(
            &[],
            "",
            &[("f.txt", b"a\r\nb\r\n")],
            &[("f.txt", b"a\nb\n")],
        );
        commit_and_compare(&[], "", &[("f.txt", b"a\nb")], &[("f.txt", b"a\nb\n")]);
        commit_and_compare(
            &[],
            "* text=auto\n",
            &[("f.txt", b"a\r\nb\r\n")],
            &[("f.txt", b"a\r\nb\r\nc\r\n")],
        );
        commit_and_compare(&[("core.autocrlf", "true")], "", text, edited);
        // binary by content, by attribute, by driver, by size
        commit_and_compare(&[], "", &[("f.bin", b"a\0b\n")], &[("f.bin", b"a\0c\n")]);
        commit_and_compare(&[], "*.txt -diff\n", text, edited);
        commit_and_compare(&[], "*.txt binary\n", text, edited);
        commit_and_compare(
            &[("diff.nope.binary", "true")],
            "*.txt diff=nope\n",
            text,
            edited,
        );
        commit_and_compare(&[("core.bigFileThreshold", "10")], "", text, edited);
        // a NUL past git's first 8000 bytes is text
        let late_nul = |tail: &str| {
            let mut v = vec![b'a'; 9000];
            v.extend_from_slice(b"\n\0");
            v.extend_from_slice(tail.as_bytes());
            v
        };
        commit_and_compare(
            &[],
            "",
            &[("f.dat", &late_nul("x\n"))],
            &[("f.dat", &late_nul("y\n"))],
        );
        // a textconv driver and an external diff
        #[cfg(unix)]
        commit_and_compare(
            &[("diff.upper.textconv", "tr a-z A-Z <")],
            "*.txt diff=upper\n",
            text,
            edited,
        );
        commit_and_compare(&[("diff.external", "false")], "", text, edited);
        commit_and_compare(
            &[("diff.ext.command", "false")],
            "*.txt diff=ext\n",
            text,
            edited,
        );
        // a clean filter changes what is stored, not what is diffed
        #[cfg(unix)]
        commit_and_compare(
            &[
                ("filter.upper.clean", "tr a-z A-Z"),
                ("filter.upper.smudge", "cat"),
            ],
            "*.txt filter=upper\n",
            text,
            edited,
        );
        // empty files
        commit_and_compare(&[], "", &[("e.txt", b"")], &[("e.txt", b"now\n")]);
        commit_and_compare(&[], "", &[("e.txt", b"x\n")], &[("e.txt", b"")]);
        // object formats and ref storage
        for args in [
            &["--object-format=sha256"][..],
            &["--ref-format=reftable"][..],
        ] {
            let dir = tempfile::tempdir().unwrap();
            let mut full = vec!["init", "-q", "-b", "main"];
            full.extend_from_slice(args);
            if !Command::new("git")
                .args(&full)
                .current_dir(dir.path())
                .status()
                .unwrap()
                .success()
            {
                continue; // an older git
            }
            run(dir.path(), &["config", "commit.gpgsign", "false"]);
            std::fs::write(dir.path().join("a.txt"), "a\n").unwrap();
            run(dir.path(), &["add", "."]);
            run(dir.path(), &["commit", "-q", "-m", "a"]);
            // reftable: commits are found by sha, no refs read; sha256: left
            // to git. Either way the answer is git's
            assert_same(dir.path(), &[head(dir.path())]);
        }
    }

    /// Every commit of this repository's own history (renames, big diffs,
    /// binaries) against git.
    #[test]
    fn matches_git_on_this_repository() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        // CI checks out one commit
        if run(&root, &["rev-parse", "--is-shallow-repository"]).trim() != "false" {
            return;
        }
        let Ok(out) = Command::new("git")
            .args(["rev-list", "--max-count=150", "HEAD"])
            .current_dir(&root)
            .output()
        else {
            return;
        };
        let shas: Vec<String> = String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::to_string)
            .collect();
        let mut in_process = 0;
        for sha in &shas {
            in_process += usize::from(assert_same(&root, std::slice::from_ref(sha)));
        }
        for pair in shas.windows(4).step_by(10) {
            let range: Vec<String> = pair.iter().rev().cloned().collect();
            assert_same(&root, &range);
        }
        // most commits add no file while deleting another
        assert!(
            in_process * 10 >= shas.len() * 7,
            "{in_process} of {}",
            shas.len()
        );
    }

    /// `CORVENE_BENCH_REPO=<repo> cargo test --profile profiling -p corvene-git
    /// bench_changed_files -- --ignored --nocapture`: the last 50 commits.
    #[test]
    #[ignore]
    fn bench_changed_files() {
        let Some(repo) = std::env::var_os("CORVENE_BENCH_REPO") else {
            return;
        };
        let repo = std::path::PathBuf::from(repo);
        let shas: Vec<String> = run(&repo, &["rev-list", "--max-count=50", "HEAD"])
            .lines()
            .map(str::to_string)
            .collect();
        let git = Arc::new(crate::find_git().unwrap());
        for in_process in [false, true, false, true] {
            let started = std::time::Instant::now();
            for sha in &shas {
                crate::log::get_changed_files(git.clone(), &repo, sha, in_process).unwrap();
            }
            eprintln!(
                "in_process={in_process}: {} µs per commit",
                started.elapsed().as_micros() / shas.len() as u128
            );
        }
    }
}
