//! GHD `ConfigLockFileExists.onDeleteLockFile`
//! (`ui/lib/config-lock-file-exists.tsx`): when saving the Git
//! configuration fails on an existing `config.lock`
//! (`parse_config_lock_file_path_from_error`), the user can delete the lock
//! and try again.

use std::io;
use std::path::Path;

/// Delete the configuration lock file at `lock_file_path`. A lock that is
/// already gone counts as deleted (GHD ignores `ENOENT`); any other failure
/// is returned (GHD's `onError`).
pub fn delete_config_lock_file(lock_file_path: &Path) -> io::Result<()> {
    match std::fs::remove_file(lock_file_path) {
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}
