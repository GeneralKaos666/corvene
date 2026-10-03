//! Port of GitHub Desktop's `app/test/unit/git/log-test.ts`.
//!
//! Corvene equivalents of GitHub Desktop's `lib/git/log.ts`:
//!
//! - `getCommits(repository, 'HEAD', n)` is `corvene_git::get_commits(path,
//!   "HEAD", 0, n)` (an in-process walk with gitoxide rather than `git
//!   log`); `shortSha` is `Commit::short_sha()`.
//! - `getChangedFiles(repository, sha)` is `corvene_git::get_changed_files`
//!   (the same `git log -C -M -m -1 --first-parent --raw --numstat -z`).
//! - GitHub Desktop's committed file status `{ kind, oldPath,
//!   submoduleStatus, renameIncludesModifications }` is Corvene's
//!   `FileStatus::kind`, `CommittedFileChange::old_path` and
//!   `FileStatus::submodule_status`; Corvene keeps git's similarity score
//!   (`FileStatus::score`) but has no `renameIncludesModifications`
//!   ([`rename_includes_modifications`] is a stand-in).

use corvene_models::{FileStatus, FileStatusKind};
use corvene_test_support::{git, setup_fixture_repository, setup_local_config};

/// Stand-in for GitHub Desktop's `renameIncludesModifications`
/// (`CopiedOrRenamedFileStatus`, `models/status.ts`; set by `mapStatus` in
/// `lib/git/log.ts`: `true` for a rename whose score is not 100, `false`
/// for copies). Replace this with the Corvene field once there is one and
/// remove the `#[ignore]`s.
fn rename_includes_modifications(_status: &FileStatus) -> bool {
    unimplemented!("corvene_models::FileStatus has no renameIncludesModifications")
}

// GHD: unit/git/log-test.ts › git/log › getCommits › loads history
#[test]
fn loads_history() {
    let repository = setup_fixture_repository("test-repo-with-tags");

    let commits = corvene_git::get_commits(repository.path(), "HEAD", 0, 100).expect("getCommits");
    assert_eq!(commits.len(), 5);

    let first_commit = &commits[commits.len() - 1];
    assert_eq!(first_commit.summary, "first");
    assert_eq!(first_commit.sha, "7cd6640e5b6ca8dbfd0b33d0281ebe702127079c");
    assert_eq!(first_commit.short_sha(), "7cd6640");
}

// GHD: unit/git/log-test.ts › git/log › getCommits › handles repository with HEAD file on disk
#[test]
fn handles_repository_with_head_file_on_disk() {
    let repo = setup_fixture_repository("repository-with-HEAD-file");
    let commits = corvene_git::get_commits(repo.path(), "HEAD", 0, 100).expect("getCommits");
    assert_eq!(commits.len(), 2);
}

// GHD: unit/git/log-test.ts › git/log › getCommits › handles repository with signed commit and log.showSignature set
#[test]
fn handles_repository_with_signed_commit_and_log_show_signature_set() {
    let repository = setup_fixture_repository("just-doing-some-signing");

    // ensure the default config is to try and show signatures
    // this should be overriden by the `getCommits` function as it may not
    // have a valid GPG agent configured
    setup_local_config(&repository, [("log.showSignature", "true")]);

    let commits = corvene_git::get_commits(repository.path(), "HEAD", 0, 100).expect("getCommits");

    assert_eq!(commits.len(), 1);
    assert_eq!(commits[0].sha, "415e4987158c49c383ce7114e0ef00ebf4b070c1");
    assert_eq!(commits[0].short_sha(), "415e498");
}

// GHD: unit/git/log-test.ts › git/log › getCommits › parses tags
#[test]
#[ignore = "ghd: bug: get_commits orders Commit::tags by ref name; GHD keeps git log %D order (got [less-important, tentative], expected [tentative, less-important])"]
fn parses_tags() {
    let repository = setup_fixture_repository("test-repo-with-tags");

    let commits = corvene_git::get_commits(repository.path(), "HEAD", 0, 100).expect("getCommits");
    assert_eq!(commits.len(), 5);

    assert_eq!(commits[0].tags, vec!["important"]);
    assert_eq!(commits[1].tags, vec!["tentative", "less-important"]);
    assert_eq!(commits[2].tags.len(), 0);
}

// GHD: unit/git/log-test.ts › git/log › getChangedFiles › loads the files changed in the commit
#[test]
fn loads_the_files_changed_in_the_commit() {
    let repository = setup_fixture_repository("test-repo-with-tags");

    let changeset_data = corvene_git::get_changed_files(
        git(),
        repository.path(),
        "7cd6640e5b6ca8dbfd0b33d0281ebe702127079c",
    )
    .expect("getChangedFiles");
    assert_eq!(changeset_data.files.len(), 1);
    assert_eq!(changeset_data.files[0].path, "README.md");
    assert_eq!(changeset_data.files[0].status.kind, FileStatusKind::New);
}

// GHD: unit/git/log-test.ts › git/log › getChangedFiles › detects renames
#[test]
#[ignore = "ghd: missing: corvene_models::FileStatus has no renameIncludesModifications (models/status.ts, mapStatus in lib/git/log.ts)"]
fn detects_renames() {
    let repository = setup_fixture_repository("rename-history-detection");

    let first = corvene_git::get_changed_files(git(), repository.path(), "55bdecb")
        .expect("getChangedFiles");
    assert_eq!(first.files.len(), 1);

    assert_eq!(first.files[0].path, "NEWER.md");
    // `{ kind: Renamed, oldPath: 'NEW.md', submoduleStatus: undefined,
    // renameIncludesModifications: true }`
    assert_eq!(first.files[0].status.kind, FileStatusKind::Renamed);
    assert_eq!(first.files[0].old_path.as_deref(), Some("NEW.md"));
    assert_eq!(first.files[0].status.submodule_status, None);
    assert!(rename_includes_modifications(&first.files[0].status));

    let second = corvene_git::get_changed_files(git(), repository.path(), "c898ca8")
        .expect("getChangedFiles");
    assert_eq!(second.files.len(), 1);

    assert_eq!(second.files[0].path, "NEW.md");
    // `{ kind: Renamed, oldPath: 'OLD.md', submoduleStatus: undefined,
    // renameIncludesModifications: false }`
    assert_eq!(second.files[0].status.kind, FileStatusKind::Renamed);
    assert_eq!(second.files[0].old_path.as_deref(), Some("OLD.md"));
    assert_eq!(second.files[0].status.submodule_status, None);
    assert!(!rename_includes_modifications(&second.files[0].status));
}

// GHD: unit/git/log-test.ts › git/log › getChangedFiles › detect copies
#[test]
#[ignore = "ghd: missing: corvene_models::FileStatus has no renameIncludesModifications (models/status.ts, mapStatus in lib/git/log.ts)"]
fn detect_copies() {
    let repository = setup_fixture_repository("copies-history-detection");

    // ensure the test repository is configured to detect copies
    setup_local_config(&repository, [("diff.renames", "copies")]);

    let changeset_data = corvene_git::get_changed_files(git(), repository.path(), "a500bf415")
        .expect("getChangedFiles");
    assert_eq!(changeset_data.files.len(), 2);

    assert_eq!(changeset_data.files[0].path, "duplicate-with-edits.md");
    // `{ kind: Copied, oldPath: 'initial.md', renameIncludesModifications:
    // false, submoduleStatus: undefined }`
    assert_eq!(changeset_data.files[0].status.kind, FileStatusKind::Copied);
    assert_eq!(
        changeset_data.files[0].old_path.as_deref(),
        Some("initial.md")
    );
    assert!(!rename_includes_modifications(
        &changeset_data.files[0].status
    ));
    assert_eq!(changeset_data.files[0].status.submodule_status, None);

    assert_eq!(changeset_data.files[1].path, "duplicate.md");
    // the same status
    assert_eq!(changeset_data.files[1].status.kind, FileStatusKind::Copied);
    assert_eq!(
        changeset_data.files[1].old_path.as_deref(),
        Some("initial.md")
    );
    assert!(!rename_includes_modifications(
        &changeset_data.files[1].status
    ));
    assert_eq!(changeset_data.files[1].status.submodule_status, None);
}

// GHD: unit/git/log-test.ts › git/log › getChangedFiles › handles commit when HEAD exists on disk
#[test]
fn handles_commit_when_head_exists_on_disk() {
    let repository = setup_fixture_repository("test-repo-with-tags");

    let changeset_data =
        corvene_git::get_changed_files(git(), repository.path(), "HEAD").expect("getChangedFiles");
    assert_eq!(changeset_data.files.len(), 1);
    assert_eq!(changeset_data.files[0].path, "README.md");
    assert_eq!(
        changeset_data.files[0].status.kind,
        FileStatusKind::Modified
    );
}

// GHD: unit/git/log-test.ts › git/log › detects submodule changes within commits
#[test]
fn detects_submodule_changes_within_commits() {
    let repository = setup_fixture_repository("submodule-basic-setup");

    let changeset_data =
        corvene_git::get_changed_files(git(), repository.path(), "HEAD").expect("getChangedFiles");
    assert_eq!(changeset_data.files.len(), 2);
    assert_eq!(changeset_data.files[1].path, "foo/submodule");
    assert!(changeset_data.files[1].status.submodule_status.is_some());
}
