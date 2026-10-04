//! Port of `app/test/helpers/fixture.ts`.

use std::path::{Path, PathBuf};

/// This crate's copy of GitHub Desktop's `app/test/fixtures`.
pub fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

/// GitHub Desktop's `getFixturePath(...paths)`: `paths` joined onto
/// [`fixtures_dir`]. Read-only: copy a fixture repository with
/// [`setup_fixture_repository`](crate::setup_fixture_repository) before
/// running git in it.
pub fn get_fixture_path<I, P>(paths: I) -> PathBuf
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    let mut path = fixtures_dir();
    for part in paths {
        path.push(part);
    }
    path
}
