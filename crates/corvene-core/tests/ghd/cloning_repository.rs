//! Port of GitHub Desktop's `app/test/unit/cloning-repository-test.ts`.
//!
//! GitHub Desktop's `CloningRepository(path, url)`
//! (`models/cloning-repository.ts`) is Corvene's `corvene_core::CloneState`
//! (`AppState::cloning`, the clone in progress).
//!
//! `CloningRepository.name` (`Path.basename(url, '.git')`) titles GitHub
//! Desktop's cloning view ("Cloning desktop", `ui/cloning-repository.tsx`).
//! `CloneState` has no name and Corvene's cloning view
//! (`corvene_ui::cloning_view`) titles itself with the path instead
//! ("Cloning C:/some/path/to/repo"), so [`name`] is a stand-in.
//! (`corvene_git::repository_name_from_url` computes the same basename, but
//! for the clone dialog's default folder name, not for a clone in progress,
//! so it is not this case's subject.)
//!
//! Corvene tracks a single clone (`AppState::cloning` is an `Option`), so a
//! `CloneState` has no id: [`id`] is a stand-in. `CloningRepository.hash` is
//! an equality key; `CloneState` is compared with `PartialEq` and has no
//! such string, so that case is skipped (`tools/ghd-tests/skips/stores.tsv`).

use std::path::PathBuf;

use corvene_core::CloneState;

/// `new CloningRepository(path, url)`: a clone that has not reported any
/// progress yet.
fn cloning_repository(path: &str, url: &str) -> CloneState {
    CloneState {
        url: url.to_string(),
        path: PathBuf::from(path),
        description: String::new(),
        value: None,
        cancel: corvene_git::CancelToken::new(),
    }
}

/// Stand-in for GitHub Desktop's `CloningRepository.name`, the repository
/// name the cloning view shows (`Path.basename(url, '.git')`). `CloneState`
/// has none and the cloning view shows the path; replace this once it has
/// one and remove the `#[ignore]`s.
fn name(_repository: &CloneState) -> String {
    unimplemented!("CloneState has no name (the cloning view shows the path)")
}

/// Stand-in for GitHub Desktop's `CloningRepository.id` (unique per
/// clone, so several clones can run and be told apart). Corvene keeps one
/// `CloneState` without an id; replace this once clones have one and remove
/// the `#[ignore]`.
fn id(_repository: &CloneState) -> u64 {
    unimplemented!("CloneState has no id (Corvene tracks a single clone)")
}

// GHD: unit/cloning-repository-test.ts › CloningRepository › name › provides the name of the repository being cloned
#[test]
#[ignore = "ghd: missing: CloneState has no name (GHD CloningRepository.name, models/cloning-repository.ts: Path.basename(url, '.git')); Corvene's cloning view title shows the path where GHD's shows that name"]
fn provides_the_name_of_the_repository_being_cloned() {
    let repository = cloning_repository(
        "C:/some/path/to/desktop",
        "https://github.com/desktop/desktop",
    );

    assert_eq!(name(&repository), "desktop");
}

// GHD: unit/cloning-repository-test.ts › CloningRepository › name › extracts the repo name from the url not the path
#[test]
#[ignore = "ghd: missing: CloneState has no name (GHD CloningRepository.name, models/cloning-repository.ts: Path.basename(url, '.git')); Corvene's cloning view title shows the path where GHD's shows that name"]
fn extracts_the_repo_name_from_the_url_not_the_path() {
    let repository =
        cloning_repository("C:/some/path/to/repo", "https://github.com/desktop/desktop");

    assert_eq!(name(&repository), "desktop");
}

// GHD: unit/cloning-repository-test.ts › CloningRepository › name › extracts the repo name without git suffix
#[test]
#[ignore = "ghd: missing: CloneState has no name (GHD CloningRepository.name, models/cloning-repository.ts: Path.basename(url, '.git')); Corvene's cloning view title shows the path where GHD's shows that name"]
fn extracts_the_repo_name_without_git_suffix() {
    let repository = cloning_repository(
        "C:/some/path/to/repo",
        "https://github.com/desktop/desktop.git",
    );

    assert_eq!(name(&repository), "desktop");
}

// GHD: unit/cloning-repository-test.ts › CloningRepository › identity › generates unique IDs
#[test]
#[ignore = "ghd: missing: CloneState has no id; Corvene tracks one clone in AppState::cloning (an Option) where GHD's CloningRepositoriesStore runs several, each with a unique CloningRepository.id"]
fn generates_unique_ids() {
    let first_repository = cloning_repository("/tmp/a", "https://github.com/owner/a.git");
    let second_repository = cloning_repository("/tmp/b", "https://github.com/owner/b.git");

    assert_ne!(id(&first_repository), id(&second_repository));
}
