//! Port of GitHub Desktop's
//! `app/test/unit/parse-files-to-be-overwritten-test.ts`.
//!
//! GitHub Desktop's `parseFilesToBeOverwritten(errorMessage)`
//! (`ui/lib/parse-files-to-be-overwritten.ts`) is
//! `corvene_git::files_that_would_be_overwritten(stderr)`.
//!
//! The tests run git through GitHub Desktop's `git(args, path, name, {
//! expectedErrors })` (`lib/git/core.ts`): exit code 0 returns with no
//! `gitError`; otherwise dugite's `parseError` of stderr (then stdout) is the
//! `gitError`, and the call returns when that error is expected and throws
//! when it is not. [`git_expecting`] does the same with
//! `corvene_git::known_git_error` (dugite's `parseError`); a plain `git()`
//! call is `exec_ok`.

use corvene_git::KnownGitError;
use corvene_test_support::{clone_local_repository, exec, exec_ok, setup_empty_repository};

/// What GitHub Desktop's `git()` resolves to (`IGitResult`), as far as the
/// tests read it.
struct GitResult {
    stderr: String,
    git_error: Option<KnownGitError>,
}

/// GitHub Desktop's `git(args, path, name, { expectedErrors })`.
fn git_expecting(
    args: &[&str],
    path: &std::path::Path,
    expected_errors: &[KnownGitError],
) -> GitResult {
    let result = exec(args, path);
    if result.exit_code == 0 {
        return GitResult {
            stderr: result.stderr,
            git_error: None,
        };
    }
    let git_error = corvene_git::known_git_error(&result.stderr)
        .or_else(|| corvene_git::known_git_error(&result.stdout));
    match git_error {
        Some(error) if expected_errors.contains(&error) => GitResult {
            stderr: result.stderr,
            git_error,
        },
        _ => panic!(
            "`git {}` exited with an unexpected code: {}.\n{}\n(The error was parsed as {git_error:?})",
            args.join(" "),
            result.exit_code,
            result.stderr
        ),
    }
}

// GHD: unit/parse-files-to-be-overwritten-test.ts › parseFilesToBeOverwritten › parses files from pull error
#[test]
fn parses_files_from_pull_error() {
    let parent = setup_empty_repository();
    std::fs::write(parent.join("a"), "1").unwrap();
    std::fs::write(parent.join("b"), "2").unwrap();
    exec_ok(["add", "a", "b"], parent.path());
    exec_ok(["commit", "-m", "initial"], parent.path());

    std::fs::write(parent.join("a"), "3").unwrap();
    std::fs::write(parent.join("b"), "4").unwrap();
    exec_ok(["add", "a", "b"], parent.path());
    exec_ok(["commit", "-m", "second"], parent.path());

    let fork = clone_local_repository(&parent);
    exec_ok(["reset", "HEAD^"], fork.path());
    let result = git_expecting(
        &["pull"],
        fork.path(),
        &[KnownGitError::MergeWithLocalChanges],
    );

    assert_eq!(result.git_error, Some(KnownGitError::MergeWithLocalChanges));
    assert_eq!(
        corvene_git::files_that_would_be_overwritten(&result.stderr),
        ["a", "b"]
    );
}

// GHD: unit/parse-files-to-be-overwritten-test.ts › parseFilesToBeOverwritten › isn't able to parse files from pull rebase error
#[test]
fn isnt_able_to_parse_files_from_pull_rebase_error() {
    let parent = setup_empty_repository();
    std::fs::write(parent.join("a"), "1").unwrap();
    std::fs::write(parent.join("b"), "2").unwrap();
    exec_ok(["add", "a", "b"], parent.path());
    exec_ok(["commit", "-m", "initial"], parent.path());

    std::fs::write(parent.join("a"), "3").unwrap();
    std::fs::write(parent.join("b"), "4").unwrap();
    exec_ok(["add", "a", "b"], parent.path());
    exec_ok(["commit", "-m", "second"], parent.path());

    let fork = clone_local_repository(&parent);
    exec_ok(["reset", "HEAD^"], fork.path());
    let result = git_expecting(
        &["pull", "--rebase"],
        fork.path(),
        &[KnownGitError::RebaseWithLocalChanges],
    );

    assert_eq!(
        result.git_error,
        Some(KnownGitError::RebaseWithLocalChanges)
    );
    assert_eq!(
        corvene_git::files_that_would_be_overwritten(&result.stderr),
        Vec::<String>::new()
    );
}

// GHD: unit/parse-files-to-be-overwritten-test.ts › parseFilesToBeOverwritten › parses files from merge error
#[test]
fn parses_files_from_merge_error() {
    let repo = setup_empty_repository();
    std::fs::write(repo.join("a"), "1").unwrap();
    std::fs::write(repo.join("b"), "2").unwrap();
    exec_ok(["add", "a", "b"], repo.path());
    exec_ok(["commit", "-m", "initial"], repo.path());

    std::fs::write(repo.join("a"), "3").unwrap();
    std::fs::write(repo.join("b"), "4").unwrap();
    exec_ok(["add", "a", "b"], repo.path());
    exec_ok(["commit", "-m", "second"], repo.path());

    exec_ok(["reset", "HEAD^"], repo.path());
    let result = git_expecting(
        &["merge", "HEAD@{1}"],
        repo.path(),
        &[KnownGitError::MergeWithLocalChanges],
    );

    assert_eq!(result.git_error, Some(KnownGitError::MergeWithLocalChanges));
    assert_eq!(
        corvene_git::files_that_would_be_overwritten(&result.stderr),
        ["a", "b"]
    );
}

// GHD: unit/parse-files-to-be-overwritten-test.ts › parseFilesToBeOverwritten › parses files from checkout error
#[test]
fn parses_files_from_checkout_error() {
    let repo = setup_empty_repository();
    std::fs::write(repo.join("a"), "1").unwrap();
    std::fs::write(repo.join("b"), "2").unwrap();
    exec_ok(["add", "a", "b"], repo.path());
    exec_ok(["commit", "-m", "initial"], repo.path());

    exec_ok(["branch", "feature-branch"], repo.path());

    std::fs::write(repo.join("a"), "3").unwrap();
    std::fs::write(repo.join("b"), "4").unwrap();

    exec_ok(["commit", "-am", "second"], repo.path());

    exec_ok(["checkout", "feature-branch"], repo.path());

    std::fs::write(repo.join("a"), "5").unwrap();
    std::fs::write(repo.join("b"), "6").unwrap();

    let result = git_expecting(
        &["checkout", "master"],
        repo.path(),
        &[KnownGitError::LocalChangesOverwritten],
    );

    assert_eq!(
        result.git_error,
        Some(KnownGitError::LocalChangesOverwritten)
    );
    assert_eq!(
        corvene_git::files_that_would_be_overwritten(&result.stderr),
        ["a", "b"]
    );
}
