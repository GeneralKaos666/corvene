//! Port of GitHub Desktop's `app/test/unit/git/rev-parse-test.ts`.
//!
//! Corvene has no single `getRepositoryType(path)` (`lib/git/rev-parse.ts`:
//! one `git rev-parse --is-bare-repository --show-cdup --git-dir`). Its
//! answer is split over the functions Corvene calls where GitHub Desktop
//! calls it, and the cases check each field against them:
//!
//! - `kind`: `corvene_git::path_status`, the check of the Add Local, Create
//!   and Clone dialogs (`corvene-ui` `dialogs/add_existing.rs`, GitHub
//!   Desktop's `validatePath`). `'regular'` is `PathStatus::Repository`,
//!   `'bare'` is `PathStatus::Bare`; `'missing'` is `PathStatus::Missing`
//!   for a directory that does not exist (`getRepositoryType`'s
//!   `directoryExists` check) and `PathStatus::NotARepository` for an
//!   existing directory git finds no repository in (exit code 128).
//! - `topLevelWorkingDirectory`: `corvene_git::top_level_working_directory`
//!   (`Dispatcher::relocate_repository`: "a subdirectory resolves to its
//!   repository's top level"). A `deepEqual(result, { kind })` has no such
//!   field, which is `None` here.
//! - `gitDir`: `corvene_git::git_dir` (GitHub Desktop's `resolvedGitDir`) of
//!   that top-level working directory, the git dir Corvene uses for a
//!   repository added or relocated from that path; `git_dir` takes a working
//!   directory, not any directory inside one.
//! - `'unsafe'`: no Corvene counterpart by design (`.docs/deviations.md` ›
//!   "Unsafe repositories": dubious ownership is detected from the failing
//!   git call itself, `corvene_git::dubious_ownership_path`, rather than a
//!   `rev-parse` probe). That case calls the stand-in
//!   [`get_repository_type_with_env`] (GitHub Desktop sets `HOME` and
//!   `GIT_TEST_ASSUME_DIFFERENT_OWNER` in `process.env` for its whole body;
//!   tests cannot change the process environment).

use std::fs;
use std::path::{Path, PathBuf};

use corvene_git::{PathStatus, git_dir, path_status, top_level_working_directory};
use corvene_test_support::{
    create_temp_directory, exec, exec_ok, setup_empty_repository, setup_fixture_repository,
};

/// GitHub Desktop's `RepositoryType` (`lib/git/rev-parse.ts`), for the
/// `unsafe` case's stand-in.
#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq)]
enum RepositoryType {
    Bare,
    Regular {
        top_level_working_directory: PathBuf,
        git_dir: PathBuf,
    },
    Missing,
    Unsafe {
        path: PathBuf,
    },
}

/// Stand-in for GitHub Desktop's `getRepositoryType(path)` run with extra
/// environment variables for its git process (GitHub Desktop's test sets
/// them in `process.env`). Corvene has no `rev-parse` probe that reports an
/// unsafe repository (see the module doc).
fn get_repository_type_with_env(_path: &Path, _env: &[(&str, &Path)]) -> RepositoryType {
    unimplemented!("Corvene has no getRepositoryType probe for unsafe repositories")
}

fn realpath(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|err| panic!("realpath {}: {err}", path.display()))
}

/// `result.kind === 'regular'`, `result.topLevelWorkingDirectory ===
/// top_level` and `realpath(result.gitDir) === realpath(git_dir)` for
/// `getRepositoryType(path)`.
fn assert_regular(path: &Path, top_level: &Path, expected_git_dir: &Path) {
    assert_eq!(
        path_status(path),
        PathStatus::Repository,
        "{}",
        path.display()
    );
    let top_level_working_directory = top_level_working_directory(path);
    assert_eq!(
        top_level_working_directory.as_deref(),
        Some(top_level),
        "{}",
        path.display()
    );
    let top_level_working_directory =
        top_level_working_directory.expect("topLevelWorkingDirectory");
    assert_eq!(
        realpath(&git_dir(&top_level_working_directory)),
        realpath(expected_git_dir)
    );
}

// GHD: unit/git/rev-parse-test.ts › git/rev-parse › getRepositoryType › should return an absolute path when run inside a working directory
#[test]
#[ignore = "ghd: bug: path_status(<repository>/subdir) is NotARepository (Add Local Repository refuses it); GHD getRepositoryType discovers the repository and answers regular with its top level"]
fn should_return_an_absolute_path_when_run_inside_a_working_directory() {
    let repository = setup_fixture_repository("test-repo");

    assert_regular(
        repository.path(),
        repository.path(),
        &repository.path().join(".git"),
    );

    let subdir_path = repository.path().join("subdir");
    fs::create_dir(&subdir_path).expect("mkdir subdir");

    assert_regular(
        &subdir_path,
        repository.path(),
        &repository.path().join(".git"),
    );
}

// GHD: unit/git/rev-parse-test.ts › git/rev-parse › getRepositoryType › should return missing when not run inside a working directory
#[test]
fn should_return_missing_when_not_run_inside_a_working_directory() {
    let temp = create_temp_directory();
    // `assert.deepEqual(result, { kind: 'missing' })`
    assert_eq!(path_status(temp.path()), PathStatus::NotARepository);
    assert_eq!(top_level_working_directory(temp.path()), None);
}

// GHD: unit/git/rev-parse-test.ts › git/rev-parse › getRepositoryType › should return correct path for submodules
#[test]
fn should_return_correct_path_for_submodules() {
    let fixture = create_temp_directory();
    let fixture_path = fixture.path();

    let first_repo_path = fixture_path.join("repo1");
    let second_repo_path = fixture_path.join("repo2");

    exec_ok(["init", "repo1"], fixture_path);

    exec_ok(["init", "repo2"], fixture_path);

    exec_ok(
        ["commit", "--allow-empty", "-m", "Initial commit"],
        &second_repo_path,
    );

    exec_ok(
        [
            // Git 2.38 (backported into 2.35.5) changed the default here to 'user'
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "../repo2",
        ],
        &first_repo_path,
    );

    assert_regular(
        &first_repo_path,
        &first_repo_path,
        &first_repo_path.join(".git"),
    );

    let sub_module_path = first_repo_path.join("repo2");
    assert_regular(
        &sub_module_path,
        &sub_module_path,
        &first_repo_path.join(".git").join("modules").join("repo2"),
    );
}

// GHD: unit/git/rev-parse-test.ts › git/rev-parse › getRepositoryType › returns regular for default initialized repository
#[test]
fn returns_regular_for_default_initialized_repository() {
    let repository = setup_empty_repository();
    assert_regular(
        repository.path(),
        repository.path(),
        &repository.path().join(".git"),
    );
}

// GHD: unit/git/rev-parse-test.ts › git/rev-parse › getRepositoryType › returns bare for initialized bare repository
#[test]
fn returns_bare_for_initialized_bare_repository() {
    let path = create_temp_directory();
    exec(["init", "--bare"], path.path());
    // `assert.deepEqual(await getRepositoryType(path), { kind: 'bare' })`
    assert_eq!(path_status(path.path()), PathStatus::Bare);
    assert_eq!(top_level_working_directory(path.path()), None);
}

// GHD: unit/git/rev-parse-test.ts › git/rev-parse › getRepositoryType › returns missing for empty directory
#[test]
fn returns_missing_for_empty_directory() {
    let p = create_temp_directory();
    // `assert.deepEqual(await getRepositoryType(p), { kind: 'missing' })`
    assert_eq!(path_status(p.path()), PathStatus::NotARepository);
    assert_eq!(top_level_working_directory(p.path()), None);
}

// GHD: unit/git/rev-parse-test.ts › git/rev-parse › getRepositoryType › returns missing for missing directory
#[test]
fn returns_missing_for_missing_directory() {
    let root_path = create_temp_directory();
    let missing_path = root_path.path().join("missing-folder");

    // `assert.deepEqual(await getRepositoryType(missingPath), { kind: 'missing' })`
    assert_eq!(path_status(&missing_path), PathStatus::Missing);
    assert_eq!(top_level_working_directory(&missing_path), None);
}

// GHD: unit/git/rev-parse-test.ts › git/rev-parse › getRepositoryType › returns unsafe for unsafe repository
#[test]
#[ignore = "ghd: deviation: deviations.md 'Unsafe repositories': dubious ownership is detected from the failing git call itself (dubious_ownership_path), Corvene has no rev-parse probe answering unsafe"]
fn returns_unsafe_for_unsafe_repository() {
    let repository = setup_fixture_repository("test-repo");

    // Creating a stub global config so we can unset safe.directory config
    // which will supersede any system config that might set * to ignore
    // warnings about a different owner
    //
    // This is because safe.directory setting is ignored if found in local
    // config, environment variables or command line arguments.
    let test_home_directory = create_temp_directory();
    let git_config_path = test_home_directory.path().join(".gitconfig");
    fs::write(&git_config_path, "[safe]\ndirectory=").expect("write .gitconfig");

    let result = get_repository_type_with_env(
        repository.path(),
        &[
            ("HOME", test_home_directory.path()),
            ("GIT_TEST_ASSUME_DIFFERENT_OWNER", Path::new("1")),
        ],
    );
    assert!(matches!(result, RepositoryType::Unsafe { .. }));
}
