//! Port of GitHub Desktop's `app/test/unit/git/git-attributes-test.ts`.
//!
//! Corvene equivalent: `writeGitAttributes(path)`
//! (`ui/add-repository/git-attributes.ts`, which Create Repository calls
//! after `git init`) is the `.gitattributes` step of
//! `corvene_git::init_repository`, fed `corvene_core::templates::GIT_ATTRIBUTES`
//! exactly as `Dispatcher::create_repository` does
//! ([`write_git_attributes`]). The repository already exists, as in GitHub
//! Desktop's test, so no default branch is passed (`git init` re-initialises
//! it and leaves `HEAD` alone); `init_repository` also commits what it wrote,
//! which the test does not look at.

use std::path::Path;

use corvene_git::InitOptions;
use corvene_test_support::{git, setup_empty_repository};

/// GitHub Desktop's `writeGitAttributes(path)` through Corvene's
/// `init_repository` with only the `.gitattributes` contents set.
fn write_git_attributes(path: &Path) {
    corvene_git::init_repository(
        git(),
        InitOptions {
            path: path.to_path_buf(),
            default_branch: None,
            description: None,
            readme: false,
            gitignore: None,
            license: None,
            git_attributes: Some(corvene_core::templates::GIT_ATTRIBUTES.to_string()),
            keep_existing: false,
        },
    )
    .expect("writeGitAttributes");
}

// GHD: unit/git/git-attributes-test.ts › git/git-attributes › writeGitAttributes › initializes a .gitattributes file
#[test]
fn initializes_a_gitattributes_file() {
    let repo = setup_empty_repository();
    write_git_attributes(repo.path());
    let expected_path = repo.join(".gitattributes");
    let contents = std::fs::read_to_string(&expected_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", expected_path.display()));
    assert!(contents.contains("* text=auto"));
}
