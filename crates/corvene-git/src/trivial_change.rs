//! Corvene `1319-trivial-change-icon` (desktop/desktop#17573): which
//! modified files changed only their mode, and which `.patch` / `.diff`
//! files changed only patch metadata (`index` lines, hunk header line
//! numbers, the `From <sha>` line, the git version trailer), as happens to
//! a stack of patches when an earlier one is edited. Read in-process with
//! gitoxide; only candidates (a mode change, a patch file) load contents.
//!
//! GitHub Desktop shows every modified file with the same icon.

use std::collections::BTreeMap;
use std::path::Path;

use corvene_models::{
    CommittedFileChange, FileStatusKind, TrivialChange, WorkingDirectoryFileChange,
};

/// Lists with more modified files than this are not classified.
const MAX_FILES: usize = 2_000;
/// Files larger than this are not read.
const MAX_BYTES: u64 = 4 * 1024 * 1024;

/// Whether `path` is a patch file whose metadata lines can change alone.
pub fn is_patch_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".patch") || lower.ends_with(".diff")
}

fn is_hex(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// `index 1a2b3c4..5d6e7f8[ 100644]`
fn is_index_line(line: &str) -> bool {
    let Some(rest) = line.strip_prefix("index ") else {
        return false;
    };
    let ids = rest.split(' ').next().unwrap_or("");
    ids.split_once("..")
        .is_some_and(|(a, b)| is_hex(a) && is_hex(b))
}

/// `@@ -1,2 +3,4 @@ context` → `@@ @@ context`.
fn hunk_header(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("@@ -")?;
    let (ranges, context) = rest.split_once(" @@")?;
    let (old, new) = ranges.split_once(" +")?;
    let range = |r: &str| {
        r.split(',')
            .all(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
    };
    (range(old) && range(new)).then_some(context)
}

/// `From <40 hex> Mon Sep 17 00:00:00 2001`
fn is_from_line(line: &str) -> bool {
    line.strip_prefix("From ")
        .and_then(|rest| rest.split(' ').next())
        .is_some_and(|sha| sha.len() >= 40 && is_hex(sha))
}

/// `2.39.2` or `2.39.2.windows.1` (the line under a `-- ` signature).
fn is_version_line(line: &str) -> bool {
    let mut parts = line.split('.');
    parts
        .next()
        .is_some_and(|major| !major.is_empty() && major.bytes().all(|b| b.is_ascii_digit()))
        && line.contains('.')
        && parts.all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_alphanumeric()))
}

/// The lines of a patch with its metadata blanked out.
fn normalised(text: &str) -> Vec<std::borrow::Cow<'_, str>> {
    let mut out = Vec::new();
    let mut after_signature = false;
    for line in text.lines() {
        let line = line.strip_suffix('\r').unwrap_or(line);
        let normal: std::borrow::Cow<str> = if is_index_line(line) {
            "index".into()
        } else if let Some(context) = hunk_header(line) {
            format!("@@ @@{context}").into()
        } else if is_from_line(line) {
            "From".into()
        } else if after_signature && is_version_line(line.trim()) {
            "version".into()
        } else {
            line.into()
        };
        after_signature = line == "-- " || line == "--";
        out.push(normal);
    }
    out
}

/// Whether two versions of a patch differ only in patch metadata.
pub fn patch_metadata_only(old: &[u8], new: &[u8]) -> bool {
    if old == new {
        return false;
    }
    let (old, new) = (String::from_utf8_lossy(old), String::from_utf8_lossy(new));
    normalised(&old) == normalised(&new)
}

/// The trivial changes among a commit's (or a stash's) modified files:
/// `old_rev` and `new_rev` are the two sides (`<sha>^` and `<sha>`).
pub fn classify_committed(
    workdir: &Path,
    old_rev: &str,
    new_rev: &str,
    files: &[CommittedFileChange],
) -> BTreeMap<String, TrivialChange> {
    let mut out = BTreeMap::new();
    let modified: Vec<&CommittedFileChange> = files
        .iter()
        .filter(|f| f.status.kind == FileStatusKind::Modified && !f.status.submodule)
        .collect();
    if modified.is_empty() || modified.len() > MAX_FILES {
        return out;
    }
    let Some(repo) = crate::handle::open_trusted(workdir) else {
        return out;
    };
    let tree = |rev: &str| {
        repo.rev_parse_single(rev)
            .ok()?
            .object()
            .ok()?
            .peel_to_tree()
            .ok()
    };
    let (Some(old_tree), Some(new_tree)) = (tree(old_rev), tree(new_rev)) else {
        return out;
    };
    for file in modified {
        let old_path = file.old_path.as_deref().unwrap_or(&file.path);
        let (Ok(Some(old)), Ok(Some(new))) = (
            old_tree.lookup_entry_by_path(old_path),
            new_tree.lookup_entry_by_path(&file.path),
        ) else {
            continue;
        };
        if !old.mode().is_blob() || !new.mode().is_blob() {
            continue;
        }
        if old.object_id() == new.object_id() {
            if old.mode() != new.mode() {
                out.insert(file.path.clone(), TrivialChange::ModeOnly);
            }
            continue;
        }
        if !is_patch_path(&file.path) {
            continue;
        }
        let load = |id: gix::ObjectId| -> Option<Vec<u8>> {
            let object = repo.find_object(id).ok()?;
            (object.data.len() as u64 <= MAX_BYTES).then(|| object.detach().data)
        };
        if let (Some(a), Some(b)) = (load(old.object_id()), load(new.object_id()))
            && patch_metadata_only(&a, &b)
        {
            out.insert(file.path.clone(), TrivialChange::PatchMetadata);
        }
    }
    out
}

/// The trivial changes among the working directory's modified files,
/// `HEAD` against the file on disk.
pub fn classify_working(
    workdir: &Path,
    files: &[WorkingDirectoryFileChange],
) -> BTreeMap<String, TrivialChange> {
    let mut out = BTreeMap::new();
    let modified: Vec<&WorkingDirectoryFileChange> = files
        .iter()
        .filter(|f| f.status.kind == FileStatusKind::Modified && !f.status.submodule)
        .collect();
    if modified.is_empty() || modified.len() > MAX_FILES {
        return out;
    }
    let Some(repo) = crate::handle::open_trusted(workdir) else {
        return out;
    };
    let Some(tree) = repo
        .head_commit()
        .ok()
        .and_then(|commit| commit.tree().ok())
    else {
        return out;
    };
    // `core.fileMode=false`: git ignores the executable bit on disk
    let file_mode = repo
        .config_snapshot()
        .boolean("core.fileMode")
        .unwrap_or(cfg!(unix));
    for file in modified {
        let patch = is_patch_path(&file.path);
        let Ok(Some(entry)) = tree.lookup_entry_by_path(&file.path) else {
            continue;
        };
        if !entry.mode().is_blob() {
            continue;
        }
        let disk = workdir.join(&file.path);
        let Ok(meta) = std::fs::symlink_metadata(&disk) else {
            continue;
        };
        if !meta.is_file() || meta.len() > MAX_BYTES {
            continue;
        }
        let head_exec = entry.mode().is_executable();
        let disk_exec = if file_mode {
            executable(&meta)
        } else {
            head_exec
        };
        let mode_changed = head_exec != disk_exec;
        if !mode_changed && !patch {
            continue;
        }
        let Some(head) = repo
            .find_object(entry.object_id())
            .ok()
            .map(|o| o.detach().data)
        else {
            continue;
        };
        let Ok(disk) = std::fs::read(&disk) else {
            continue;
        };
        if mode_changed && head == disk {
            out.insert(file.path.clone(), TrivialChange::ModeOnly);
        } else if patch && !mode_changed && patch_metadata_only(&head, &disk) {
            out.insert(file.path.clone(), TrivialChange::PatchMetadata);
        }
    }
    out
}

#[cfg(unix)]
fn executable(meta: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn executable(_meta: &std::fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD: &str = "From 1111111111111111111111111111111111111111 Mon Sep 17 00:00:00 2001\n\
        From: A <a@b.c>\n\
        Subject: [PATCH] Fix\n\
        \n\
        diff --git a/x b/x\n\
        index 1111111..2222222 100644\n\
        --- a/x\n\
        +++ b/x\n\
        @@ -10,2 +10,2 @@ fn main() {\n\
        -a\n\
        +b\n\
        -- \n\
        2.39.2\n";

    #[test]
    fn metadata_only_changes() {
        let new = OLD
            .replace(
                "1111111111111111111111111111111111111111",
                "2222222222222222222222222222222222222222",
            )
            .replace("1111111..2222222", "3333333..4444444")
            .replace("-10,2 +10,2", "-12,2 +12,2")
            .replace("2.39.2", "2.40.0.windows.1");
        assert!(patch_metadata_only(OLD.as_bytes(), new.as_bytes()));
    }

    #[test]
    fn content_changes_are_not_metadata() {
        let new = OLD.replace("+b", "+c");
        assert!(!patch_metadata_only(OLD.as_bytes(), new.as_bytes()));
        let new = OLD.replace("fn main() {", "fn other() {");
        assert!(!patch_metadata_only(OLD.as_bytes(), new.as_bytes()));
        assert!(!patch_metadata_only(OLD.as_bytes(), OLD.as_bytes()));
    }

    #[test]
    fn line_shapes() {
        assert!(is_index_line("index 1a2b..3c4d"));
        assert!(!is_index_line("index of things"));
        assert_eq!(hunk_header("@@ -1 +1 @@"), Some(""));
        assert_eq!(hunk_header("@@ -1,0 +2,3 @@ x"), Some(" x"));
        assert_eq!(hunk_header("@@ text @@"), None);
        assert!(is_version_line("2.39.2"));
        assert!(!is_version_line("Thanks"));
        assert!(is_patch_path("0001-fix.PATCH"));
        assert!(!is_patch_path("src/patch.rs"));
    }
}
