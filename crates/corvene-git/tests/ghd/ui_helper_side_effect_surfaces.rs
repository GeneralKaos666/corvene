//! Port of GitHub Desktop's `app/test/unit/ui/helper-side-effect-surfaces-test.tsx`
//! (the `ConfigLockFileExists` cases).
//!
//! GitHub Desktop's `ConfigLockFileExists` (`ui/lib/config-lock-file-exists.tsx`,
//! shown by Settings › Git and Configure Git when saving the config fails
//! on an existing `config.lock`) offers to "delete the lock file and try
//! again": `onDeleteLockFile` unlinks the lock, treats a lock that is
//! already gone (`ENOENT`) as deleted and calls `onLockFileDeleted`, and
//! hands any other failure to `onError`. Corvene's counterpart is
//! `corvene_git::delete_config_lock_file`, which the error dialog of a Git
//! configuration save offers (`Dispatcher::delete_config_lock_file`):
//! `Ok(())` is `onLockFileDeleted`, `Err(e)` is `onError(e)`.
//!
//! GitHub Desktop mocks `fs/promises.unlink`; these tests use real files in a
//! temporary directory instead: a lock file that exists, one that does not
//! (`ENOENT`), and one in a read-only directory, which `unlink` refuses with
//! `EACCES` (GitHub Desktop's mock throws `EACCES`, "permission denied").
//! Windows ignores a read-only directory and `remove_file` deletes a
//! read-only file there, so on Windows the lock is held open by a handle
//! that does not share deletion instead (a sharing violation).
//! The recorded unlinked paths become "the file is gone afterwards".
//!
//! The theme case of the file is skipped in `tools/ghd-tests/skips/ui1.tsv`.

use std::io;
use std::path::Path;
#[cfg(unix)]
use std::path::PathBuf;

use corvene_git::delete_config_lock_file;
use corvene_test_support::create_temp_directory;

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
#[cfg(unix)]
struct ReadOnly(Vec<PathBuf>);

#[cfg(unix)]
impl ReadOnly {
    fn new(paths: Vec<PathBuf>) -> Self {
        for path in &paths {
            set_read_only(path, true);
        }
        Self(paths)
    }
}

#[cfg(unix)]
impl Drop for ReadOnly {
    fn drop(&mut self) {
        for path in self.0.iter().rev() {
            if path.exists() {
                set_read_only(path, false);
            }
        }
    }
}

#[cfg(unix)]
fn set_read_only(path: &Path, read_only: bool) {
    let mut permissions = std::fs::metadata(path).unwrap().permissions();
    permissions.set_readonly(read_only);
    std::fs::set_permissions(path, permissions).unwrap();
}

// GHD: unit/ui/helper-side-effect-surfaces-test.tsx › helper side-effect surfaces › deletes the config lock file and retries when deletion succeeds or file is already gone
#[test]
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
fn reports_config_lock_deletion_failures_other_than_enoent() {
    let temp = create_temp_directory();
    let dir = temp.path().join("locked");
    std::fs::create_dir(&dir).unwrap();
    let lock = dir.join("repo.lock");
    std::fs::write(&lock, "").unwrap();

    // unlink fails with EACCES: a lock in a read-only directory; write
    // access comes back before the temporary directory is removed, even
    // when the test fails
    #[cfg(unix)]
    let _read_only = ReadOnly::new(vec![dir.clone()]);
    // Windows: a handle without FILE_SHARE_DELETE, closed before the
    // temporary directory is removed
    #[cfg(windows)]
    let _open = {
        use std::os::windows::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&lock)
            .unwrap()
    };

    let mut callbacks = Callbacks::default();
    callbacks.click_delete(&lock);

    assert_eq!(callbacks.deleted_count, 0);
    assert!(lock.exists());
    let kinds: Vec<io::ErrorKind> = callbacks.errors.iter().map(io::Error::kind).collect();
    // `errors` is `['permission denied']`, the EACCES error's message
    #[cfg(unix)]
    assert_eq!(kinds, [io::ErrorKind::PermissionDenied]);
    // the sharing violation has no `ErrorKind` of its own; it is reported
    #[cfg(windows)]
    assert!(
        kinds.len() == 1 && kinds[0] != io::ErrorKind::NotFound,
        "{:?}",
        callbacks.errors
    );
}
