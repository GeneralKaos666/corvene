//! Port of GitHub Desktop's `app/test/unit/git/core-test.ts`.
//!
//! GitHub Desktop's `git(args, path, name, options)` (`lib/git/core.ts`) is
//! `corvene_git::GitCommand` (the only place Corvene spawns git): its
//! `successExitCodes` are [`GitCommand::allow_exit_code`] (which adds to
//! the accepted `0` where GitHub Desktop replaces it; no case runs into
//! that), a rejected promise is `Err(GitError)`, and `IGitResult.gitError`
//! is `corvene_git::known_git_error` of stderr (dugite's `parseError`,
//! `KnownGitError` standing for dugite's `GitError`), and its
//! `expectedErrors` are [`GitCommand::expected_errors`]. [`git`] is that
//! translation. GitHub Desktop's `parseConfigLockFilePathFromError(result)`
//! is `corvene_git::parse_config_lock_file_path_from_error(stderr, path)`
//! of the result's stderr and path ([`parse_config_lock_file_path_from_error`]).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use corvene_git::{GitCommand, GitError, KnownGitError, known_git_error};
use corvene_test_support::setup_fixture_repository;

/// The `IGitExecutionOptions` these cases pass.
#[derive(Clone, Debug, Default)]
struct GitExecutionOptions {
    expected_errors: Option<HashSet<KnownGitError>>,
    success_exit_codes: Option<HashSet<i32>>,
}

/// GitHub Desktop's `IGitResult`.
#[derive(Clone, Debug)]
struct GitResult {
    exit_code: i32,
    git_error: Option<KnownGitError>,
    path: PathBuf,
    #[allow(dead_code)]
    git_error_description: Option<String>,
    stderr: String,
    #[allow(dead_code)]
    stdout: String,
}

/// GitHub Desktop's `git(args, path, name, options)`.
fn git(
    args: &[&str],
    path: &Path,
    _name: &str,
    options: GitExecutionOptions,
) -> Result<GitResult, GitError> {
    let mut command = GitCommand::new(corvene_test_support::git())
        .args(args)
        .current_dir(path);
    if let Some(codes) = options.success_exit_codes {
        for code in codes {
            command = command.allow_exit_code(code);
        }
    }
    if let Some(expected_errors) = options.expected_errors {
        command = command.expected_errors(expected_errors);
    }
    let output = command.run()?;
    Ok(GitResult {
        exit_code: output.status.code().expect("git exited"),
        git_error: known_git_error(&output.stderr),
        path: path.to_path_buf(),
        git_error_description: None,
        stderr: output.stderr,
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
    })
}

/// GitHub Desktop's `parseConfigLockFilePathFromError(result)`.
fn parse_config_lock_file_path_from_error(result: &GitResult) -> Option<PathBuf> {
    corvene_git::parse_config_lock_file_path_from_error(&result.stderr, &result.path)
}

// GHD: unit/git/core-test.ts › git/core › error handling › does not throw for errors that were expected
#[test]
fn does_not_throw_for_errors_that_were_expected() {
    let test_repo = setup_fixture_repository("test-repo");

    let args = ["rev-list", "--left-right", "--count", "some-ref", "--"];
    let result = git(
        &args,
        test_repo.path(),
        "test",
        GitExecutionOptions {
            expected_errors: Some(HashSet::from([KnownGitError::BadRevision])),
            ..Default::default()
        },
    )
    .expect("an expected error does not throw");
    assert_eq!(result.git_error, Some(KnownGitError::BadRevision));
}

// GHD: unit/git/core-test.ts › git/core › error handling › throws for errors that were not expected
#[test]
fn throws_for_errors_that_were_not_expected() {
    let test_repo = setup_fixture_repository("test-repo");

    let args = ["rev-list", "--left-right", "--count", "some-ref", "--"];
    let result = git(
        &args,
        test_repo.path(),
        "test",
        GitExecutionOptions {
            expected_errors: Some(HashSet::from([KnownGitError::SSHKeyAuditUnverified])),
            ..Default::default()
        },
    );
    assert!(result.is_err(), "{result:?}");
}

// GHD: unit/git/core-test.ts › git/core › exit code handling › does not throw for exit codes that were expected
#[test]
fn does_not_throw_for_exit_codes_that_were_expected() {
    let repo = setup_fixture_repository("test-repo");
    let args = ["rev-list", "--left-right", "--count", "some-ref", "--"];
    let result = git(
        &args,
        repo.path(),
        "test",
        GitExecutionOptions {
            success_exit_codes: Some(HashSet::from([128])),
            ..Default::default()
        },
    )
    .expect("an expected exit code does not throw");
    assert_eq!(result.exit_code, 128);
}

// GHD: unit/git/core-test.ts › git/core › exit code handling › throws for exit codes that were not expected
#[test]
fn throws_for_exit_codes_that_were_not_expected() {
    let repo = setup_fixture_repository("test-repo");
    let args = ["rev-list", "--left-right", "--count", "some-ref", "--"];
    let result = git(
        &args,
        repo.path(),
        "test",
        GitExecutionOptions {
            success_exit_codes: Some(HashSet::from([2])),
            ..Default::default()
        },
    );
    assert!(result.is_err(), "{result:?}");
}

// GHD: unit/git/core-test.ts › git/core › config lock file error handling › can parse lock file path from stderr
#[test]
fn can_parse_lock_file_path_from_stderr() {
    let repo = setup_fixture_repository("test-repo");
    let repo_path = repo.path();

    let config_file_path = repo_path.join(".git").join("config");
    let config_lock_file_path = PathBuf::from(format!("{}.lock", config_file_path.display()));

    std::fs::copy(&config_file_path, &config_lock_file_path).unwrap();

    let args = ["config", "--local", "user.name", "niik"];
    let result = git(
        &args,
        repo_path,
        "test",
        GitExecutionOptions {
            expected_errors: Some(HashSet::from([KnownGitError::ConfigLockFileAlreadyExists])),
            ..Default::default()
        },
    )
    .expect("an expected error does not throw");

    assert_eq!(result.exit_code, 255);
    assert_eq!(
        result.git_error,
        Some(KnownGitError::ConfigLockFileAlreadyExists)
    );
    let parsed_path = parse_config_lock_file_path_from_error(&result);
    assert!(parsed_path.is_some());
    // `path.resolve(result.path, parsedPath)`
    let absolute_path = result.path.join(parsed_path.unwrap());
    assert_eq!(absolute_path, config_lock_file_path);
}

// GHD: unit/git/core-test.ts › git/core › config lock file error handling › normalizes paths
#[test]
fn normalizes_paths() {
    // GitHub Desktop compares the returned string exactly; a `PathBuf`
    // comparison would ignore the separators (on Windows `C:/…` equals
    // `C:\…`), which is what this case checks.
    fn as_string(path: Option<PathBuf>) -> Option<String> {
        path.map(|path| path.to_string_lossy().into_owned())
    }

    fn create_git_result(stderr: &str) -> GitResult {
        GitResult {
            exit_code: 255,
            git_error: Some(KnownGitError::ConfigLockFileAlreadyExists),
            path: PathBuf::from(if cfg!(windows) { "c:\\" } else { "/" }),
            git_error_description: None,
            stderr: stderr.to_string(),
            stdout: String::new(),
        }
    }

    #[cfg(windows)]
    {
        assert_eq!(
            as_string(parse_config_lock_file_path_from_error(&create_git_result(
                "error: could not lock config file C:/Users/markus/.gitconfig: File exists"
            ))),
            Some("C:\\Users\\markus\\.gitconfig.lock".to_string())
        );

        assert_eq!(
            as_string(parse_config_lock_file_path_from_error(&create_git_result(
                "error: could not lock config file C:\\Users\\markus\\.gitconfig: File exists"
            ))),
            Some("C:\\Users\\markus\\.gitconfig.lock".to_string())
        );
    }
    #[cfg(not(windows))]
    {
        assert_eq!(
            as_string(parse_config_lock_file_path_from_error(&create_git_result(
                "error: could not lock config file /Users/markus/.gitconfig: File exists"
            ))),
            Some("/Users/markus/.gitconfig.lock".to_string())
        );
    }
}
