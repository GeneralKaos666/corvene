//! Port of GitHub Desktop's `app/test/unit/ui/helper-side-effect-surfaces-test.tsx`
//! (the `ConfigLockFileExists` cases).
//!
//! GitHub Desktop's `ConfigLockFileExists` (`ui/lib/config-lock-file-exists.tsx`,
//! shown by Settings › Git and Configure Git when saving the config fails
//! on an existing `config.lock`) offers to "delete the lock file and try
//! again": `onDeleteLockFile` unlinks the lock, treats a lock that is
//! already gone (`ENOENT`) as deleted and calls `onLockFileDeleted`, and
//! hands any other failure to `onError`. Corvene has no counterpart: it
//! reports `GitError::ConfigLockFileAlreadyExists` with a lead sentence
//! ("Another program is changing this repository's Git configuration; try
//! again in a moment.", `git_errors.rs`) and offers no deletion (its
//! `index_lock` removal, flag `265-remove-stale-index-lock`, handles
//! `index.lock` only). [`delete_config_lock_file`] is a stand-in: `Ok(())`
//! is `onLockFileDeleted`, `Err(e)` is `onError(e)`. Replace it with the
//! Corvene function once there is one and remove the `#[ignore]`s.
//!
//! GitHub Desktop mocks `fs/promises.unlink`; these tests use real files in a
//! temporary directory instead: a lock file that exists, one that does not
//! (`ENOENT`), and one in a read-only directory, which `unlink` refuses with
//! `EACCES` (GitHub Desktop's mock throws `EACCES`, "permission denied").
//! The recorded unlinked paths become "the file is gone afterwards".
//!
//! The theme case of the file is skipped in `tools/ghd-tests/skips/ui1.tsv`.

use std::io;
use std::path::{Path, PathBuf};

use corvene_test_support::create_temp_directory;

/// Stand-in for GitHub Desktop's `ConfigLockFileExists.onDeleteLockFile`
/// (`ui/lib/config-lock-file-exists.tsx`) for `lockFilePath`.
fn delete_config_lock_file(_lock_file_path: &Path) -> io::Result<()> {
    unimplemented!("Corvene has no ConfigLockFileExists (delete the config lock file and retry)")
}

/// `ConfigLockFileExists` with counting `onLockFileDeleted` / `onError`
/// callbacks.
#[derive(Default)]
struct Callbacks {
    deleted_count: usize,
    errors: Vec<io::Error>,
}

impl Callbacks {
    /// `fireEvent.click` on "delete the lock file".
    fn click_delete(&mut self, lock_file_path: &Path) {
        match delete_config_lock_file(lock_file_path) {
            Ok(()) => self.deleted_count += 1,
            Err(error) => self.errors.push(error),
        }
    }
}

/// Makes paths read-only, and writable again when dropped.
struct ReadOnly(Vec<PathBuf>);

impl ReadOnly {
    fn new(paths: Vec<PathBuf>) -> Self {
        for path in &paths {
            set_read_only(path, true);
        }
        Self(paths)
    }
}

impl Drop for ReadOnly {
    fn drop(&mut self) {
        for path in self.0.iter().rev() {
            if path.exists() {
                set_read_only(path, false);
            }
        }
    }
}

fn set_read_only(path: &Path, read_only: bool) {
    let mut permissions = std::fs::metadata(path).unwrap().permissions();
    permissions.set_readonly(read_only);
    std::fs::set_permissions(path, permissions).unwrap();
}

// GHD: unit/ui/helper-side-effect-surfaces-test.tsx › helper side-effect surfaces › deletes the config lock file and retries when deletion succeeds or file is already gone
#[test]
#[ignore = "ghd: missing: no ConfigLockFileExists delete-and-retry (ui/lib/config-lock-file-exists.tsx); Corvene only reports ConfigLockFileAlreadyExists with a lead sentence (git_errors.rs)"]
fn deletes_the_config_lock_file_and_retries_when_deletion_succeeds_or_file_is_already_gone() {
    let temp = create_temp_directory();
    // `lockFilePath="/tmp/repo.lock"`: unlinking succeeds
    let lock = temp.path().join("repo.lock");
    std::fs::write(&lock, "").unwrap();
    // `lockFilePath="/tmp/repo.missing.lock"`: unlinking fails with ENOENT
    let missing_lock = temp.path().join("repo.missing.lock");

    let mut callbacks = Callbacks::default();
    callbacks.click_delete(&lock);
    callbacks.click_delete(&missing_lock);

    // `deletedPaths` is both paths
    assert!(!lock.exists());
    assert!(!missing_lock.exists());
    assert_eq!(callbacks.deleted_count, 2);
    assert!(
        callbacks.errors.is_empty(),
        "unexpected errors: {:?}",
        callbacks.errors
    );
}

// GHD: unit/ui/helper-side-effect-surfaces-test.tsx › helper side-effect surfaces › reports config lock deletion failures other than ENOENT
#[test]
#[ignore = "ghd: missing: no ConfigLockFileExists delete-and-retry (ui/lib/config-lock-file-exists.tsx); Corvene only reports ConfigLockFileAlreadyExists with a lead sentence (git_errors.rs)"]
fn reports_config_lock_deletion_failures_other_than_enoent() {
    let temp = create_temp_directory();
    let dir = temp.path().join("locked");
    std::fs::create_dir(&dir).unwrap();
    let lock = dir.join("repo.lock");
    std::fs::write(&lock, "").unwrap();

    // unlink fails with EACCES: a read-only lock (Windows) in a read-only
    // directory (Unix); write access comes back before the temporary
    // directory is removed, even when the test fails
    let _read_only = ReadOnly::new(vec![lock.clone(), dir.clone()]);

    let mut callbacks = Callbacks::default();
    callbacks.click_delete(&lock);

    assert_eq!(callbacks.deleted_count, 0);
    // `errors` is `['permission denied']`, the EACCES error's message
    let kinds: Vec<io::ErrorKind> = callbacks.errors.iter().map(io::Error::kind).collect();
    assert_eq!(kinds, [io::ErrorKind::PermissionDenied]);
}
