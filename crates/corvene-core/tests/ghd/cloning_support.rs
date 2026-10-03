//! Helpers shared by `corvene-core`'s ports of GitHub Desktop's cloning
//! tests (`unit/cloning-repository-test.ts`,
//! `unit/cloning-repositories-store-test.ts`).

use std::path::PathBuf;

use corvene_core::CloneState;

/// GitHub Desktop's `new CloningRepository(path, url)`
/// (`models/cloning-repository.ts`): Corvene's `CloneState` of a clone that
/// has not reported any progress yet.
pub fn cloning_repository(path: &str, url: &str) -> CloneState {
    CloneState {
        url: url.to_string(),
        path: PathBuf::from(path),
        description: String::new(),
        value: None,
        cancel: corvene_git::CancelToken::new(),
    }
}
