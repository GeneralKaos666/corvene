//! Git LFS pointer files and their local objects, read without git-lfs.
//!
//! A file stored in Git LFS is committed as a small pointer (`version
//! https://git-lfs.github.com/spec/v1`, `oid sha256:…`, `size …`); git-lfs
//! keeps the contents in `<git dir>/lfs/objects/<oid[0..2]>/<oid[2..4]>/<oid>`
//! (or under `lfs.storage`) once downloaded. GHD diffs the pointers: an LFS
//! image shows as three changed text lines.
//!
//! Corvene `795-lfs-image-previews` ([`resolve_lfs_images`]): an image whose
//! sides are pointers shows the images when their objects are local, else
//! the pointer diff with a note that the contents are not downloaded.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use corvene_models::{Diff, FileStatusKind, image_media_type};

use crate::detect::GitBinary;

/// A pointer is at most this large (git-lfs' own limit).
const MAX_POINTER_BYTES: usize = 1024;

/// What an LFS pointer file names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LfsPointer {
    /// The SHA-256 of the contents, 64 lowercase hex digits.
    pub oid: String,
    /// The contents' size in bytes.
    pub size: u64,
}

/// `bytes` as an LFS pointer file, when they are one.
pub fn lfs_pointer(bytes: &[u8]) -> Option<LfsPointer> {
    if bytes.len() > MAX_POINTER_BYTES {
        return None;
    }
    let text = std::str::from_utf8(bytes).ok()?;
    let mut lines = text.lines();
    let version = lines.next()?.strip_prefix("version ")?;
    if !version.starts_with("https://git-lfs.github.com/spec/")
        && !version.starts_with("https://hawser.github.com/spec/")
    {
        return None;
    }
    let (mut oid, mut size) = (None, None);
    for line in lines {
        if let Some(hex) = line.strip_prefix("oid sha256:") {
            oid = Some(hex.trim().to_string());
        } else if let Some(n) = line.strip_prefix("size ") {
            size = n.trim().parse().ok();
        }
    }
    let oid = oid.filter(|o| o.len() == 64 && o.bytes().all(|b| b.is_ascii_hexdigit()))?;
    Some(LfsPointer {
        oid: oid.to_ascii_lowercase(),
        size: size?,
    })
}

/// git-lfs' storage directory: `lfs.storage` (relative to the git dir) or
/// `<common git dir>/lfs`.
fn lfs_storage(git: Arc<GitBinary>, workdir: &Path) -> PathBuf {
    let common = crate::paths::common_dir(workdir);
    match crate::config_value(git, workdir, "lfs.storage") {
        Some(dir) if Path::new(&dir).is_absolute() => PathBuf::from(dir),
        Some(dir) => common.join(dir),
        None => common.join("lfs"),
    }
}

/// The downloaded contents `pointer` names; `None` when they are not local
/// (or do not have the size the pointer records).
pub fn lfs_object(git: Arc<GitBinary>, workdir: &Path, pointer: &LfsPointer) -> Option<Vec<u8>> {
    let path = lfs_storage(git, workdir)
        .join("objects")
        .join(&pointer.oid[0..2])
        .join(&pointer.oid[2..4])
        .join(&pointer.oid);
    let meta = std::fs::metadata(&path).ok()?;
    if meta.len() != pointer.size {
        return None;
    }
    std::fs::read(path).ok()
}

/// Whether a text diff's lines are those of LFS pointer files.
fn is_pointer_diff(hunks: &[corvene_models::DiffHunk]) -> bool {
    hunks.iter().flat_map(|h| h.lines.iter()).any(|l| {
        l.text.starts_with("oid sha256:")
            || l.text.starts_with("version https://git-lfs.github.com/spec/")
    })
}

/// Corvene `795-lfs-image-previews`: `diff` of the image at `path` with
/// sides stored as LFS pointers shown as images. `current` / `previous`
/// read the new and old sides' bytes (the working file or a blob): a side
/// that is a pointer becomes its local object. A pointer diff whose
/// objects are not all local stays a text diff, marked
/// `DiffWarnings::lfs_not_downloaded`. Any other diff is returned as it is.
pub fn resolve_lfs_images(
    git: Arc<GitBinary>,
    workdir: &Path,
    path: &str,
    kind: FileStatusKind,
    diff: Diff,
    current: impl FnOnce() -> Option<Vec<u8>>,
    previous: impl FnOnce() -> Option<Vec<u8>>,
) -> Diff {
    let Some(media_type) = image_media_type(path) else {
        return diff;
    };
    let resolve = |bytes: Vec<u8>| -> (bool, Option<Vec<u8>>) {
        match lfs_pointer(&bytes) {
            Some(pointer) => (true, lfs_object(git.clone(), workdir, &pointer)),
            None => (false, Some(bytes)),
        }
    };
    match diff {
        Diff::Text {
            hunks,
            mut warnings,
        } if is_pointer_diff(&hunks) => {
            let current = (kind != FileStatusKind::Deleted)
                .then(current)
                .flatten()
                .map(resolve);
            let previous = (!kind.is_new_or_untracked())
                .then(previous)
                .flatten()
                .map(resolve);
            let missing = |side: &Option<(bool, Option<Vec<u8>>)>, expected: bool| {
                expected && side.as_ref().is_none_or(|(_, bytes)| bytes.is_none())
            };
            if missing(&current, kind != FileStatusKind::Deleted)
                || missing(&previous, !kind.is_new_or_untracked())
            {
                warnings.lfs_not_downloaded = true;
                return Diff::Text { hunks, warnings };
            }
            crate::diff::image_diff_as(
                media_type,
                kind,
                || current.and_then(|(_, bytes)| bytes),
                || previous.and_then(|(_, bytes)| bytes),
            )
        }
        Diff::Image { previous, current } => {
            // git-lfs not installed: git compares a pointer with the image;
            // a pointer whose object is not local has nothing to draw
            let swap = |blob: Option<corvene_models::ImageBlob>| {
                let mut blob = blob?;
                if let Some(pointer) = lfs_pointer(&blob.bytes) {
                    blob.bytes = lfs_object(git.clone(), workdir, &pointer)?;
                }
                Some(blob)
            };
            Diff::Image {
                previous: swap(previous),
                current: swap(current),
            }
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OID: &str = "4d7a214614ab2935c943f9e0ff69d22eadbb8f32b1258daaa5e2ca24d17e2393";

    #[test]
    fn parses_pointers() {
        let text = format!("version https://git-lfs.github.com/spec/v1\noid sha256:{OID}\nsize 12345\n");
        assert_eq!(
            lfs_pointer(text.as_bytes()),
            Some(LfsPointer {
                oid: OID.to_string(),
                size: 12345
            })
        );
        assert_eq!(lfs_pointer(b"\x89PNG\r\n"), None);
        assert_eq!(lfs_pointer(b"version https://example.com\noid sha256:x\nsize 1\n"), None);
        let short = "version https://git-lfs.github.com/spec/v1\noid sha256:abc\nsize 1\n";
        assert_eq!(lfs_pointer(short.as_bytes()), None);
    }

    #[test]
    fn resolves_local_objects_into_an_image_diff() {
        use std::process::Command;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        assert!(
            Command::new("git")
                .args(["init", "-q"])
                .current_dir(path)
                .status()
                .unwrap()
                .success()
        );
        let png = b"\x89PNG\r\n\x1a\nnot really".to_vec();
        let object = path.join(".git/lfs/objects").join(&OID[0..2]).join(&OID[2..4]);
        std::fs::create_dir_all(&object).unwrap();
        std::fs::write(object.join(OID), &png).unwrap();
        let pointer = format!(
            "version https://git-lfs.github.com/spec/v1\noid sha256:{OID}\nsize {}\n",
            png.len()
        );
        let git = Arc::new(crate::find_git().unwrap());
        let diff = crate::parse_unified(&format!(
            "@@ -0,0 +1,3 @@\n+{}",
            pointer.lines().collect::<Vec<_>>().join("\n+")
        ));
        let resolved = resolve_lfs_images(
            git.clone(),
            path,
            "a.png",
            FileStatusKind::New,
            diff.clone(),
            || Some(pointer.clone().into_bytes()),
            || None,
        );
        match resolved {
            Diff::Image { previous, current } => {
                assert!(previous.is_none());
                assert_eq!(current.unwrap().bytes, png);
            }
            other => panic!("image expected, got {other:?}"),
        }
        // not downloaded: the pointer diff with the note
        let other = pointer.replace(OID, &"0".repeat(64));
        match resolve_lfs_images(
            git,
            path,
            "a.png",
            FileStatusKind::New,
            diff,
            || Some(other.into_bytes()),
            || None,
        ) {
            Diff::Text { warnings, .. } => assert!(warnings.lfs_not_downloaded),
            other => panic!("text expected, got {other:?}"),
        }
    }
}
