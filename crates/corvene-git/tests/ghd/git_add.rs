//! Port of GitHub Desktop's `app/test/unit/git/add-test.ts`.
//!
//! GitHub Desktop's `addConflictedFile(repository, file)` (`lib/git/add.ts`)
//! is `git add -- <file.path>`. Corvene runs that command inside
//! `corvene_git::stage_manual_conflict_resolution` and, on its own, as
//! `corvene_git::add_paths`, which [`add_conflicted_file`] calls.

use corvene_models::{FileStatusKind, WorkingDirectoryFileChange};
use corvene_test_support::{TestRepo, get_status_or_throw, git, setup_conflicted_repo};

/// GitHub Desktop's `addConflictedFile(repository, file)`.
fn add_conflicted_file(repository: &TestRepo, file: &WorkingDirectoryFileChange) {
    corvene_git::add_paths(git(), repository.path(), &[file.path.as_str()]).expect("git add");
}

// GHD: unit/git/add-test.ts › git/add › addConflictedFile › stages a conflicted file after manual resolution
#[test]
fn stages_a_conflicted_file_after_manual_resolution() {
    let repo = setup_conflicted_repo();

    // Get the conflicted status
    let before_status = get_status_or_throw(&repo);
    let conflicted_files: Vec<_> = before_status
        .files
        .iter()
        .filter(|f| f.status.kind == FileStatusKind::Conflicted)
        .collect();
    assert!(
        !conflicted_files.is_empty(),
        "Expected at least one conflicted file"
    );

    let file = conflicted_files[0];

    // Resolve the conflict by writing new content
    std::fs::write(repo.join(&file.path), "resolved content\n").unwrap();

    // Stage the resolved file
    add_conflicted_file(&repo, file);

    // Verify the file is no longer conflicted in status
    let after_status = get_status_or_throw(&repo);
    let still_conflicted = after_status
        .files
        .iter()
        .filter(|f| f.path == file.path && f.status.kind == FileStatusKind::Conflicted)
        .count();
    assert_eq!(
        still_conflicted, 0,
        "File should no longer be conflicted after staging"
    );
}
