//! Port of GitHub Desktop's `app/test/unit/cloning-repository-test.ts`.
//!
//! GitHub Desktop's `CloningRepository(path, url)`
//! (`models/cloning-repository.ts`) is Corvene's `corvene_core::CloneState`
//! (`AppState::cloning`, the clone in progress).
//!
//! `CloningRepository.name` (`Path.basename(url, '.git')`) titles the
//! cloning view ("Cloning desktop", `ui/cloning-repository.tsx`) and is
//! `CloneState::name`, which Corvene's cloning view
//! (`corvene_ui::cloning_view`) titles itself with too.
//! (`corvene_git::repository_name_from_url` computes a similar name for the
//! clone dialog's default folder, not for a clone in progress.)
//!
//! `CloningRepository.id` is `CloneState::id`, unique per clone
//! (`CloneState::new`), so the clones `AppState::cloning` holds can be told
//! apart. `CloningRepository.hash` is an equality key; `CloneState` is
//! compared with `PartialEq` and has no such string, so that case is skipped
//! (`tools/ghd-tests/skips/stores.tsv`).

use crate::cloning_support::cloning_repository;

// GHD: unit/cloning-repository-test.ts › CloningRepository › name › provides the name of the repository being cloned
#[test]
fn provides_the_name_of_the_repository_being_cloned() {
    let repository = cloning_repository(
        "C:/some/path/to/desktop",
        "https://github.com/desktop/desktop",
    );

    assert_eq!(repository.name(), "desktop");
}

// GHD: unit/cloning-repository-test.ts › CloningRepository › name › extracts the repo name from the url not the path
#[test]
fn extracts_the_repo_name_from_the_url_not_the_path() {
    let repository =
        cloning_repository("C:/some/path/to/repo", "https://github.com/desktop/desktop");

    assert_eq!(repository.name(), "desktop");
}

// GHD: unit/cloning-repository-test.ts › CloningRepository › name › extracts the repo name without git suffix
#[test]
fn extracts_the_repo_name_without_git_suffix() {
    let repository = cloning_repository(
        "C:/some/path/to/repo",
        "https://github.com/desktop/desktop.git",
    );

    assert_eq!(repository.name(), "desktop");
}

// GHD: unit/cloning-repository-test.ts › CloningRepository › identity › generates unique IDs
#[test]
fn generates_unique_ids() {
    let first_repository = cloning_repository("/tmp/a", "https://github.com/owner/a.git");
    let second_repository = cloning_repository("/tmp/b", "https://github.com/owner/b.git");

    assert_ne!(first_repository.id, second_repository.id);
}
