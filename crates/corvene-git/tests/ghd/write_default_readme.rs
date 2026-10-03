//! Port of GitHub Desktop's `app/test/unit/write-default-readme-test.ts`.
//!
//! GitHub Desktop's `writeDefaultReadme(path, name, description?)`
//! (`ui/add-repository/write-default-readme.ts`) writes `README.md` for
//! "Initialize this repository with a README". Corvene writes that file
//! inside `corvene_git::init_repository` when `InitOptions::readme` is set,
//! with the repository folder's name as the name, so the cases create the
//! repository in a folder named `some-repository` (the name GitHub Desktop
//! passes) inside the temporary directory and read that folder's
//! `README.md`. The other options are the ones the dispatcher's Create
//! Repository passes, minus the templates (`.gitattributes`, `.gitignore`,
//! license) that do not touch the README.

use std::path::Path;

use corvene_git::{InitOptions, configured_default_branch, init_repository};
use corvene_test_support::{create_temp_directory, git, lock_global_config};

/// GitHub Desktop's `writeDefaultReadme(path, name, description)`:
/// `init_repository` with a README for the folder `path/name`. Returns the
/// folder the README is written to.
fn write_default_readme(path: &Path, name: &str, description: Option<&str>) -> std::path::PathBuf {
    let target = path.join(name);
    init_repository(
        git(),
        InitOptions {
            path: target.clone(),
            default_branch: Some(configured_default_branch(git())),
            description: description.map(str::to_string),
            readme: true,
            gitignore: None,
            license: None,
            git_attributes: None,
            keep_existing: false,
        },
    )
    .expect("init_repository");
    target
}

// GHD: unit/write-default-readme-test.ts › repository setup › writeDefaultReadme › writes a default README without a description
#[test]
fn writes_a_default_readme_without_a_description() {
    let _global = lock_global_config();
    let temp = create_temp_directory();

    let directory = write_default_readme(temp.path(), "some-repository", None);
    let file = directory.join("README.md");

    let text = std::fs::read_to_string(file).unwrap();
    assert_eq!(text, "# some-repository\n");
}

// GHD: unit/write-default-readme-test.ts › repository setup › writeDefaultReadme › writes a README with description when provided
#[test]
fn writes_a_readme_with_description_when_provided() {
    let _global = lock_global_config();
    let temp = create_temp_directory();

    let directory = write_default_readme(
        temp.path(),
        "some-repository",
        Some("description goes here"),
    );
    let file = directory.join("README.md");

    let text = std::fs::read_to_string(file).unwrap();
    assert_eq!(text, "# some-repository\ndescription goes here\n");
}
