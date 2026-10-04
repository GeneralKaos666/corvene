//! Port of GitHub Desktop's `app/test/unit/git/description-test.ts`.
//!
//! `getGitDescription(repositoryPath)` (`lib/git/description.ts`, which
//! prefills the Publish Repository dialog's description) is
//! `corvene_git::get_git_description`.

use corvene_git::get_git_description;
use corvene_test_support::setup_empty_repository;

// GHD: unit/git/description-test.ts › git/description › getGitDescription › returns empty for an initialized repository
#[test]
fn returns_empty_for_an_initialized_repository() {
    let repo = setup_empty_repository();
    let actual = get_git_description(repo.path());
    assert_eq!(actual, "");
}

// GHD: unit/git/description-test.ts › git/description › getGitDescription › returns empty when path is missing
#[test]
fn returns_empty_when_path_is_missing() {
    let repo = setup_empty_repository();
    let path = repo.join(".git").join("description");
    std::fs::remove_file(&path).unwrap();

    let actual = get_git_description(repo.path());
    assert_eq!(actual, "");
}

// GHD: unit/git/description-test.ts › git/description › getGitDescription › reads the custom text
#[test]
fn reads_the_custom_text() {
    let expected = "this is a repository description";
    let repo = setup_empty_repository();
    let path = repo.join(".git").join("description");
    std::fs::write(&path, expected).unwrap();

    let actual = get_git_description(repo.path());
    assert_eq!(actual, expected);
}
