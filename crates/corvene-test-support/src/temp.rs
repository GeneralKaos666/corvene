//! Port of `app/test/helpers/temp.ts`.

use tempfile::TempDir;

use crate::env::init;

/// GitHub Desktop's `createTempDirectory(t)`: a new empty directory
/// `<temp>/desktop-test-XXXXXX`, removed when the returned guard is dropped
/// (GitHub Desktop removes it in `t.after`). Keep the guard alive for the
/// whole test.
///
/// # Panics
///
/// When the directory cannot be created.
pub fn create_temp_directory() -> TempDir {
    init();
    tempfile::Builder::new()
        .prefix("desktop-test-")
        .tempdir()
        .expect("create a temporary directory")
}
