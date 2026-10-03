//! Port of GitHub Desktop's `app/test/unit/git/gitignore-test.ts`.
//!
//! Corvene equivalents (`lib/git/gitignore.ts` → `corvene_git::ignore`):
//!
//! - `readGitIgnoreAtRoot(repository)` is `corvene_git::read_gitignore(path)`:
//!   `Ok(None)` is GitHub Desktop's `null`, an `Err` a rejection.
//! - `saveGitIgnore(repository, text)` is [`save_git_ignore`]: GitHub
//!   Desktop reads `core.autocrlf` itself (`formatGitIgnoreContents`);
//!   Corvene's `corvene_git::save_gitignore(path, text, autocrlf)` takes it
//!   from the caller, and [`save_git_ignore`] reads it as Repository
//!   Settings does before `Dispatcher::save_repository_settings` saves
//!   (`Dispatcher::open_repository_settings`: `config_value` of
//!   `core.autocrlf` equal to `true`, case-insensitively).
//! - `appendIgnoreRule(repository, patterns)` is
//!   `corvene_git::append_ignore_rules(path, patterns, false)` and
//!   `appendIgnoreFile(repository, paths)`
//!   `corvene_git::append_ignore_files(path, paths, false)`: `false` is the
//!   GitHub Desktop value of flag `717-ignore-skips-existing-rules`, as
//!   `Dispatcher::ignore_patterns` passes it.
//! - `escapeGitSpecialCharacters(pattern)` is
//!   `corvene_git::escape_gitignore_pattern(pattern)`.
//! - A rejection's message is the error's text ([`git_error_message`]).

use corvene_git::GitError;
use corvene_test_support::{
    TestRepo, exec, get_status_or_throw, git, git_error_message, setup_empty_repository,
    setup_local_config,
};

use crate::index_support::regex_test;

/// `readGitIgnoreAtRoot(repository)`.
fn read_git_ignore_at_root(repository: &TestRepo) -> Result<Option<String>, GitError> {
    corvene_git::read_gitignore(repository.path())
}

/// `saveGitIgnore(repository, text)`, with `core.autocrlf` read the way
/// Corvene's Repository Settings dialog reads it before saving.
fn save_git_ignore(repository: &TestRepo, text: &str) -> Result<(), GitError> {
    let autocrlf = corvene_git::config_value(git(), repository.path(), "core.autocrlf")
        .is_some_and(|v| v.eq_ignore_ascii_case("true"));
    corvene_git::save_gitignore(repository.path(), text, autocrlf)
}

/// `appendIgnoreRule(repository, patterns)`.
fn append_ignore_rule(repository: &TestRepo, patterns: &[&str]) -> Result<(), GitError> {
    let patterns: Vec<String> = patterns.iter().map(|p| p.to_string()).collect();
    corvene_git::append_ignore_rules(repository.path(), &patterns, false)
}

/// `appendIgnoreFile(repository, filePaths)`.
fn append_ignore_file(repository: &TestRepo, file_paths: &[&str]) -> Result<(), GitError> {
    let file_paths: Vec<String> = file_paths.iter().map(|p| p.to_string()).collect();
    corvene_git::append_ignore_files(repository.path(), &file_paths, false)
}

/// `fs.symlink(target, path)`.
#[cfg(unix)]
fn symlink(target: &std::path::Path, path: &std::path::Path) {
    std::os::unix::fs::symlink(target, path).unwrap();
}

/// `fs.symlink(target, path)` (a file link: the target is a file or missing).
#[cfg(windows)]
fn symlink(target: &std::path::Path, path: &std::path::Path) {
    std::os::windows::fs::symlink_file(target, path).unwrap();
}

const SYMBOLIC_LINK_ERROR: &str = "Cannot use a symbolic link as the root .gitignore file";

// GHD: unit/git/gitignore-test.ts › gitignore › readGitIgnoreAtRoot › returns null when .gitignore does not exist on disk
#[test]
fn returns_null_when_gitignore_does_not_exist_on_disk() {
    let repo = setup_empty_repository();

    let gitignore = read_git_ignore_at_root(&repo).expect("readGitIgnoreAtRoot");

    assert!(gitignore.is_none());
}

// GHD: unit/git/gitignore-test.ts › gitignore › readGitIgnoreAtRoot › reads contents from disk
#[test]
fn reads_contents_from_disk() {
    let repo = setup_empty_repository();
    let path = repo.path();

    let expected = "node_modules\nyarn-error.log\n";

    let ignore_file = path.join(".gitignore");
    std::fs::write(&ignore_file, expected).unwrap();

    let gitignore = read_git_ignore_at_root(&repo).expect("readGitIgnoreAtRoot");

    assert_eq!(gitignore.as_deref(), Some(expected));
}

// GHD: unit/git/gitignore-test.ts › gitignore › readGitIgnoreAtRoot › rejects a symbolic link
#[test]
#[ignore = "ghd: bug: read_gitignore reads through a symbolic link .gitignore (Ok(Some(\"target contents\"))); GHD rejects it (Cannot use a symbolic link as the root .gitignore file)"]
fn read_git_ignore_at_root_rejects_a_symbolic_link() {
    let repo = setup_empty_repository();
    let target_path = repo.join("target");

    std::fs::write(&target_path, "target contents").unwrap();
    symlink(&target_path, &repo.join(".gitignore"));

    let result = read_git_ignore_at_root(&repo);
    let err = result.expect_err("readGitIgnoreAtRoot rejects");
    let message = git_error_message(&err);
    assert!(regex_test(SYMBOLIC_LINK_ERROR, &message), "{message:?}");
}

// GHD: unit/git/gitignore-test.ts › gitignore › readGitIgnoreAtRoot › rejects a dangling symbolic link
#[test]
#[ignore = "ghd: bug: read_gitignore treats a dangling symbolic link .gitignore as missing (Ok(None)); GHD rejects it (Cannot use a symbolic link as the root .gitignore file)"]
fn read_git_ignore_at_root_rejects_a_dangling_symbolic_link() {
    let repo = setup_empty_repository();
    let target_path = repo.join("missing-target");

    symlink(&target_path, &repo.join(".gitignore"));

    let result = read_git_ignore_at_root(&repo);
    let err = result.expect_err("readGitIgnoreAtRoot rejects");
    let message = git_error_message(&err);
    assert!(regex_test(SYMBOLIC_LINK_ERROR, &message), "{message:?}");
}

// GHD: unit/git/gitignore-test.ts › gitignore › readGitIgnoreAtRoot › when autocrlf=true and safecrlf=true, appends CRLF to file
#[test]
fn when_autocrlf_true_and_safecrlf_true_appends_crlf_to_file() {
    let repo = setup_empty_repository();

    setup_local_config(
        &repo,
        [("core.autocrlf", "true"), ("core.safecrlf", "true")],
    );

    let path = repo.path();

    save_git_ignore(&repo, "node_modules").expect("saveGitIgnore");
    exec(["add", ".gitignore"], path);

    let commit = exec(["commit", "-m", "create the ignore file"], path);
    assert_eq!(commit.exit_code, 0, "{commit:?}");

    let contents = read_git_ignore_at_root(&repo).expect("readGitIgnoreAtRoot");
    let contents = contents.expect("contents !== null");
    assert!(contents.ends_with("\r\n"), "{contents:?}");
}

// GHD: unit/git/gitignore-test.ts › gitignore › readGitIgnoreAtRoot › when autocrlf=input, appends LF to file
#[test]
fn when_autocrlf_input_appends_lf_to_file() {
    let repo = setup_empty_repository();

    setup_local_config(
        &repo,
        [
            // ensure this repository only ever sticks to LF
            ("core.eol", "lf"),
            // do not do any conversion of line endings when committing
            ("core.autocrlf", "input"),
        ],
    );

    let path = repo.path();

    save_git_ignore(&repo, "node_modules").expect("saveGitIgnore");
    exec(["add", ".gitignore"], path);

    let commit = exec(["commit", "-m", "create the ignore file"], path);
    assert_eq!(commit.exit_code, 0, "{commit:?}");

    let contents = read_git_ignore_at_root(&repo).expect("readGitIgnoreAtRoot");
    let contents = contents.expect("contents !== null");
    assert!(contents.ends_with('\n'), "{contents:?}");
}

// GHD: unit/git/gitignore-test.ts › gitignore › saveGitIgnore › creates gitignore file when it doesn't exist
#[test]
fn creates_gitignore_file_when_it_doesnt_exist() {
    let repo = setup_empty_repository();

    save_git_ignore(&repo, "node_modules\n").expect("saveGitIgnore");

    let exists = repo.join(".gitignore").exists();

    assert!(exists);
}

// GHD: unit/git/gitignore-test.ts › gitignore › saveGitIgnore › rejects a symbolic link without modifying its target
#[test]
#[ignore = "ghd: bug: save_gitignore writes through a symbolic link .gitignore (Ok, target now `node_modules\\n`); GHD rejects it and leaves the target alone"]
fn save_git_ignore_rejects_a_symbolic_link_without_modifying_its_target() {
    let repo = setup_empty_repository();
    let target_path = repo.join("target");

    std::fs::write(&target_path, "target contents").unwrap();
    symlink(&target_path, &repo.join(".gitignore"));

    let result = save_git_ignore(&repo, "node_modules\n");
    let err = result.expect_err("saveGitIgnore rejects");
    let message = git_error_message(&err);
    assert!(regex_test(SYMBOLIC_LINK_ERROR, &message), "{message:?}");
    assert_eq!(
        std::fs::read_to_string(&target_path).unwrap(),
        "target contents"
    );
}

// GHD: unit/git/gitignore-test.ts › gitignore › saveGitIgnore › rejects a dangling symbolic link
#[test]
#[ignore = "ghd: bug: save_gitignore writes through a dangling symbolic link .gitignore (Ok, creates its target); GHD rejects it"]
fn save_git_ignore_rejects_a_dangling_symbolic_link() {
    let repo = setup_empty_repository();
    let target_path = repo.join("missing-target");

    symlink(&target_path, &repo.join(".gitignore"));

    let result = save_git_ignore(&repo, "node_modules\n");
    let err = result.expect_err("saveGitIgnore rejects");
    let message = git_error_message(&err);
    assert!(regex_test(SYMBOLIC_LINK_ERROR, &message), "{message:?}");
}

// GHD: unit/git/gitignore-test.ts › gitignore › saveGitIgnore › deletes gitignore file when no entries provided
#[test]
fn deletes_gitignore_file_when_no_entries_provided() {
    let repo = setup_empty_repository();
    let path = repo.path();

    let ignore_file = path.join(".gitignore");
    std::fs::write(&ignore_file, "node_modules\n").unwrap();

    // update gitignore file to be empty
    save_git_ignore(&repo, "").expect("saveGitIgnore");

    let exists = ignore_file.exists();
    assert!(!exists);
}

// GHD: unit/git/gitignore-test.ts › gitignore › saveGitIgnore › applies rule correctly to repository
#[test]
fn applies_rule_correctly_to_repository() {
    let repo = setup_empty_repository();

    let path = repo.path();

    save_git_ignore(&repo, "*.txt\n").expect("saveGitIgnore");
    exec(["add", ".gitignore"], path);
    exec(["commit", "-m", "create the ignore file"], path);

    // Create a txt file
    let file = repo.join("a.txt");

    std::fs::write(&file, "thrvbnmerkl;,iuw").unwrap();

    // Check status of repo
    let status = get_status_or_throw(&repo);
    let files = status.files;

    assert_eq!(files.len(), 0, "{files:?}");
}

// GHD: unit/git/gitignore-test.ts › gitignore › saveGitIgnore › escapes string with special git characters
#[test]
fn escapes_string_with_special_git_characters() {
    let unescaped_file_path = "[never]\\!gonna*give#you?_.up";
    let escaped_file_path = "\\[never\\]\\\\!gonna\\*give\\#you\\?_.up";

    let result = corvene_git::escape_gitignore_pattern(unescaped_file_path);
    assert_eq!(result, escaped_file_path);
}

// GHD: unit/git/gitignore-test.ts › gitignore › appendIgnoreRule › appends one rule
#[test]
fn appends_one_rule() {
    let repo = setup_empty_repository();

    setup_local_config(&repo, [("core.autocrlf", "true")]);

    let path = repo.path();

    let ignore_file = path.join(".gitignore");
    std::fs::write(&ignore_file, "node_modules\n").unwrap();

    append_ignore_rule(&repo, &["yarn-error.log"]).expect("appendIgnoreRule");

    let gitignore = std::fs::read(&ignore_file).unwrap();

    let expected = "node_modules\nyarn-error.log\n";
    assert_eq!(String::from_utf8(gitignore).unwrap(), expected);
}

// GHD: unit/git/gitignore-test.ts › gitignore › appendIgnoreRule › appends multiple rules
#[test]
fn appends_multiple_rules() {
    let repo = setup_empty_repository();

    setup_local_config(&repo, [("core.autocrlf", "true")]);

    let path = repo.path();

    let ignore_file = path.join(".gitignore");
    std::fs::write(&ignore_file, "node_modules\n").unwrap();

    append_ignore_rule(&repo, &["yarn-error.log", ".eslintcache", "dist/"])
        .expect("appendIgnoreRule");

    let gitignore = std::fs::read(&ignore_file).unwrap();

    let expected = "node_modules\nyarn-error.log\n.eslintcache\ndist/\n";
    assert_eq!(String::from_utf8(gitignore).unwrap(), expected);
}

// GHD: unit/git/gitignore-test.ts › gitignore › appendIgnoreRule › appends one file containing special characters
#[test]
fn appends_one_file_containing_special_characters() {
    let repo = setup_empty_repository();

    setup_local_config(&repo, [("core.autocrlf", "true")]);

    let path = repo.path();

    let ignore_file = path.join(".gitignore");
    std::fs::write(&ignore_file, "node_modules\n").unwrap();

    let file_to_ignore = "[never]!gonna*give#you?_.up";
    append_ignore_file(&repo, &[file_to_ignore]).expect("appendIgnoreFile");

    let gitignore = std::fs::read(&ignore_file).unwrap();

    let expected = concat!(
        "node_modules\n",
        "\\[never\\]\\!gonna\\*give\\#you\\?_.up\n"
    );
    assert_eq!(String::from_utf8(gitignore).unwrap(), expected);
}
