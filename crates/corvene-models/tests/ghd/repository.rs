//! Port of GitHub Desktop's `app/test/unit/repository-test.ts`.
//!
//! GitHub Desktop's `Repository.name` (`models/repository.ts`: the GitHub
//! repository's name, else `Path.basename(path)`, else the whole path) is
//! `corvene_models::Repository::name` (the alias, else the GitHub
//! repository's name, else `corvene_models::dir_name`). `new
//! Repository(path, -1, null, false)` is `Repository::new(0, path)`: Corvene
//! ids are unsigned and the id plays no part in the name.

use corvene_models::Repository;

// GHD: unit/repository-test.ts › Repository › name › uses the last path component as the name
#[test]
fn uses_the_last_path_component_as_the_name() {
    let repo_path = "/some/cool/path";
    let repository = Repository::new(0, repo_path);
    assert_eq!(repository.name(), "path");
}

// GHD: unit/repository-test.ts › Repository › name › handles repository at root of the drive
#[test]
fn handles_repository_at_root_of_the_drive() {
    let repo_path = "T:\\";
    let repository = Repository::new(0, repo_path);
    assert_eq!(repository.name(), "T:\\");
}
