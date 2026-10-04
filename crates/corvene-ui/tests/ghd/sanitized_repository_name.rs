//! Port of GitHub Desktop's `app/test/unit/sanitized-repository-name-test.ts`.
//!
//! GitHub Desktop's `sanitizedRepositoryName`
//! (`ui/add-repository/sanitized-repository-name.ts`, used by the Publish
//! Repository dialog) is `corvene_ui::dialogs::sanitized_repository_name`
//! (`dialogs/remote_dialogs.rs`, used by Corvene's Publish dialog).

use corvene_ui::dialogs::sanitized_repository_name;

// GHD: unit/sanitized-repository-name-test.ts › sanitizedRepositoryName › leaves a good repo name alone
#[test]
fn leaves_a_good_repo_name_alone() {
    let repo_name = "this-is-fine";
    let result = sanitized_repository_name(repo_name);
    assert_eq!(result, "this-is-fine");
}

// GHD: unit/sanitized-repository-name-test.ts › sanitizedRepositoryName › replaces invalid characters with dashes
#[test]
fn replaces_invalid_characters_with_dashes() {
    let repo_name = ".this..is\\not fine:yo?|is-it";
    let result = sanitized_repository_name(repo_name);
    assert_eq!(result, ".this..is-not-fine-yo--is-it");
}

// GHD: unit/sanitized-repository-name-test.ts › sanitizedRepositoryName › replaces space with dashes
#[test]
fn replaces_space_with_dashes() {
    let repo_name = "repo space name";
    let result = sanitized_repository_name(repo_name);
    assert_eq!(result, "repo-space-name");
}

// GHD: unit/sanitized-repository-name-test.ts › sanitizedRepositoryName › replaces ending slash with dashes
#[test]
fn replaces_ending_slash_with_dashes() {
    let repo_name = "hello/";
    let result = sanitized_repository_name(repo_name);
    assert_eq!(result, "hello-");
}

// GHD: unit/sanitized-repository-name-test.ts › sanitizedRepositoryName › does not allow name to start with plus, replaces it with dashes
#[test]
fn does_not_allow_name_to_start_with_plus_replaces_it_with_dashes() {
    let repo_name = "++but-can-still-keep-the-rest";
    let result = sanitized_repository_name(repo_name);
    assert_eq!(result, "--but-can-still-keep-the-rest");
}

// GHD: unit/sanitized-repository-name-test.ts › sanitizedRepositoryName › allow name to start with minus
#[test]
fn allow_name_to_start_with_minus() {
    let repo_name = "--but-can-still-keep-the-rest";
    let result = sanitized_repository_name(repo_name);
    assert_eq!(result, repo_name);
}

// GHD: unit/sanitized-repository-name-test.ts › sanitizedRepositoryName › replaces slash in newlines with dash
#[test]
fn replaces_slash_in_newlines_with_dash() {
    let repo_name = "hello\\r\\nworld";
    let result = sanitized_repository_name(repo_name);
    assert_eq!(result, "hello-r-nworld");
}

// GHD: unit/sanitized-repository-name-test.ts › sanitizedRepositoryName › allow name to have dots
#[test]
fn allow_name_to_have_dots() {
    let repo_name = ".first.dot.is.ok";
    let result = sanitized_repository_name(repo_name);
    assert_eq!(result, repo_name);
}

// GHD: unit/sanitized-repository-name-test.ts › sanitizedRepositoryName › allows double dashes
#[test]
fn allows_double_dashes() {
    let repo_name = "repo--name";
    let result = sanitized_repository_name(repo_name);
    assert_eq!(result, repo_name);
}

// GHD: unit/sanitized-repository-name-test.ts › sanitizedRepositoryName › replaces one emoji with one dash
#[test]
fn replaces_one_emoji_with_one_dash() {
    let repo_name = "hello🐓world-repo";
    let result = sanitized_repository_name(repo_name);
    assert_eq!(result, "hello-world-repo");
}
