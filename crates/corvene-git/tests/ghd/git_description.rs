//! Port of GitHub Desktop's `app/test/unit/git/description-test.ts`.
//!
//! Corvene has no equivalent of `getGitDescription` (GitHub Desktop's
//! `lib/git/description.ts`, which feeds the Publish Repository dialog), so
//! the cases call a stand-in and are ignored until it exists.

use std::path::Path;

use corvene_test_support::setup_empty_repository;

/// Stand-in for GitHub Desktop's `getGitDescription(repositoryPath)`:
/// `.git/description`, or `''` when it is missing or still git's default
/// text. Replace it with the `corvene_git` function once there is one and
/// remove the `#[ignore]`s.
fn get_git_description(_repository_path: &Path) -> String {
    unimplemented!("corvene_git has no getGitDescription")
}

// GHD: unit/git/description-test.ts › git/description › getGitDescription › returns empty for an initialized repository
#[test]
#[ignore = "ghd: missing: corvene_git has no getGitDescription (lib/git/description.ts)"]
fn returns_empty_for_an_initialized_repository() {
    let repo = setup_empty_repository();
    let actual = get_git_description(repo.path());
    assert_eq!(actual, "");
}

// GHD: unit/git/description-test.ts › git/description › getGitDescription › returns empty when path is missing
#[test]
#[ignore = "ghd: missing: corvene_git has no getGitDescription (lib/git/description.ts)"]
fn returns_empty_when_path_is_missing() {
    let repo = setup_empty_repository();
    let path = repo.join(".git").join("description");
    std::fs::remove_file(&path).unwrap();

    let actual = get_git_description(repo.path());
    assert_eq!(actual, "");
}

// GHD: unit/git/description-test.ts › git/description › getGitDescription › reads the custom text
#[test]
#[ignore = "ghd: missing: corvene_git has no getGitDescription (lib/git/description.ts)"]
fn reads_the_custom_text() {
    let expected = "this is a repository description";
    let repo = setup_empty_repository();
    let path = repo.join(".git").join("description");
    std::fs::write(&path, expected).unwrap();

    let actual = get_git_description(repo.path());
    assert_eq!(actual, expected);
}
