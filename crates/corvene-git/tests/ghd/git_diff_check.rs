//! Port of GitHub Desktop's `app/test/unit/git/diff-check-test.ts`.
//!
//! Corvene equivalent: `getFilesWithConflictMarkers(repositoryPath)`
//! (`lib/git/diff-check.ts`) is `corvene_git::conflict_marker_counts(git(),
//! path)` (the same `git diff --check`, read into a map of path to marker
//! count). GitHub Desktop's `Map` is compared with `deepStrictEqual`, which
//! ignores insertion order, so a `HashMap` stands for it.

use std::collections::HashMap;

use corvene_git::conflict_marker_counts as get_files_with_conflict_markers;
use corvene_test_support::{
    git, setup_conflicted_repo, setup_conflicted_repo_with_multiple_files, setup_empty_repository,
};

// GHD: unit/git/diff-check-test.ts › getFilesWithConflictMarkers › with one conflicted file › finds one conflicted file
#[test]
fn finds_one_conflicted_file() {
    let repository = setup_conflicted_repo();

    assert_eq!(
        get_files_with_conflict_markers(git(), repository.path()).expect("diff --check"),
        HashMap::from([("foo".to_string(), 3)])
    );
}

// GHD: unit/git/diff-check-test.ts › getFilesWithConflictMarkers › with one conflicted file › finds multiple conflicted files
#[test]
fn finds_multiple_conflicted_files() {
    let repository = setup_conflicted_repo_with_multiple_files();
    assert_eq!(
        get_files_with_conflict_markers(git(), repository.path()).expect("diff --check"),
        HashMap::from([
            ("baz".to_string(), 3),
            ("cat".to_string(), 3),
            ("foo".to_string(), 3),
        ])
    );
}

// GHD: unit/git/diff-check-test.ts › getFilesWithConflictMarkers › with no conflicted files › finds no conflicted files
#[test]
fn finds_no_conflicted_files() {
    let repository = setup_empty_repository();
    assert!(
        get_files_with_conflict_markers(git(), repository.path())
            .expect("diff --check")
            .is_empty()
    );
}
