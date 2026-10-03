//! Port of GitHub Desktop's `app/test/unit/git/status-test.ts`.
//!
//! Corvene equivalents of GitHub Desktop's `lib/git/status.ts`:
//!
//! - `getStatus(repository)` is `corvene_git::get_status(git(), path, None)`
//!   (`getStatusOrThrow` is `get_status_or_throw`). GitHub Desktop's `null`
//!   (git exited with 128, "likely missing its .git directory") is
//!   `Err(GitError::Failed { code: Some(128), .. })`.
//! - `status.workingDirectory.files` is `WorkingDirectoryStatus::files`, a
//!   file's `path` is `WorkingDirectoryFileChange::path`.
//! - The `AppFileStatus` keys compared by `deepStrictEqual` map to:
//!   `kind` → `FileStatus::kind`; `oldPath` →
//!   `WorkingDirectoryFileChange::old_path`; `submoduleStatus` →
//!   `FileStatus::submodule_status` (`undefined` is `None`, and
//!   `FileStatus::submodule` false: GitHub Desktop has no submodule status
//!   for a path that is not a submodule);
//!   `conflictMarkerCount` → `FileStatus::conflict_markers` (a missing key,
//!   GitHub Desktop's manual conflict or a rename / copy, is `None`); a conflicted entry's
//!   `action` (`UnmergedEntrySummary`) → the porcelain code
//!   `FileStatus::code` it is parsed from in GitHub Desktop's
//!   `status-parser.ts` (`BothModified` is `UU`, `BothAdded` is `AA`,
//!   `DeletedByThem` is `UD`); `entry.us` / `entry.them` →
//!   `FileStatus::us()` / `them()` (`GitStatusEntry.UpdatedButUnmerged` is
//!   `GitStatusEntry::Unmerged`). Corvene has no
//!   `renameIncludesModifications` ([`rename_includes_modifications`] is a
//!   stand-in, checked last so the other keys still run under `--ignored`).
//! - `isConflictedFile(s) && isManualConflict(s)` is
//!   `FileStatus::is_manual_conflict` (which checks the kind itself; GitHub
//!   Desktop's `isConflictedFile` only narrows the type for
//!   `isManualConflict`).

use corvene_git::GitError;
use corvene_models::{FileStatus, FileStatusKind, GitStatusEntry, SubmoduleStatus};
use corvene_test_support::{
    DEFAULT_STRING_LENGTH, append_file, conflicted_count, exec, generate_string,
    get_status_or_throw, git, setup_conflicted_repo_with_multiple_files, setup_empty_directory,
    setup_empty_repository, setup_fixture_repository, setup_local_config, write_file,
};

/// Stand-in for GitHub Desktop's `renameIncludesModifications`
/// (`CopiedOrRenamedFileStatus`, `models/status.ts`; set by
/// `convertToAppStatus` in `lib/git/status.ts`: `false` for copies, and for
/// renames `true` when the working tree is modified or the score is below
/// 100). Replace this with the Corvene field once there is one and remove
/// the `#[ignore]`s.
fn rename_includes_modifications(_status: &FileStatus) -> bool {
    unimplemented!("corvene_models::FileStatus has no renameIncludesModifications")
}

// ---------------------------------------------------------------------------
// with conflicted repo
// ---------------------------------------------------------------------------

// GHD: unit/git/status-test.ts › git/status › getStatus › with conflicted repo › parses conflicted files with markers
#[test]
fn parses_conflicted_files_with_markers() {
    let repository = setup_conflicted_repo_with_multiple_files();

    let status = get_status_or_throw(&repository);
    let files = &status.files;
    assert_eq!(files.len(), 5);
    assert_eq!(conflicted_count(files), 4);

    let foo_file = files.iter().find(|f| f.path == "foo").expect("foo");
    // { kind: Conflicted, entry: { kind: 'conflicted', action: BothModified,
    //   them: UpdatedButUnmerged, us: UpdatedButUnmerged,
    //   submoduleStatus: undefined }, conflictMarkerCount: 3 }
    assert_eq!(foo_file.status.kind, FileStatusKind::Conflicted);
    assert_eq!(foo_file.status.code, "UU");
    assert_eq!(foo_file.status.them(), GitStatusEntry::Unmerged);
    assert_eq!(foo_file.status.us(), GitStatusEntry::Unmerged);
    assert_eq!(foo_file.status.submodule_status, None);
    assert!(!foo_file.status.submodule);
    assert_eq!(foo_file.status.conflict_markers, Some(3));

    let baz_file = files.iter().find(|f| f.path == "baz").expect("baz");
    // { kind: Conflicted, entry: { kind: 'conflicted', action: BothAdded,
    //   them: Added, us: Added, submoduleStatus: undefined },
    //   conflictMarkerCount: 3 }
    assert_eq!(baz_file.status.kind, FileStatusKind::Conflicted);
    assert_eq!(baz_file.status.code, "AA");
    assert_eq!(baz_file.status.them(), GitStatusEntry::Added);
    assert_eq!(baz_file.status.us(), GitStatusEntry::Added);
    assert_eq!(baz_file.status.submodule_status, None);
    assert!(!baz_file.status.submodule);
    assert_eq!(baz_file.status.conflict_markers, Some(3));

    let cat_file = files.iter().find(|f| f.path == "cat").expect("cat");
    // { kind: Conflicted, entry: { kind: 'conflicted', action: BothAdded,
    //   them: Added, us: Added, submoduleStatus: undefined },
    //   conflictMarkerCount: 3 }
    assert_eq!(cat_file.status.kind, FileStatusKind::Conflicted);
    assert_eq!(cat_file.status.code, "AA");
    assert_eq!(cat_file.status.them(), GitStatusEntry::Added);
    assert_eq!(cat_file.status.us(), GitStatusEntry::Added);
    assert_eq!(cat_file.status.submodule_status, None);
    assert!(!cat_file.status.submodule);
    assert_eq!(cat_file.status.conflict_markers, Some(3));
}

// GHD: unit/git/status-test.ts › git/status › getStatus › with conflicted repo › parses conflicted files without markers
#[test]
fn parses_conflicted_files_without_markers() {
    let repository = setup_conflicted_repo_with_multiple_files();

    let status = get_status_or_throw(&repository);
    let files = &status.files;
    assert_eq!(files.len(), 5);
    assert_eq!(conflicted_count(files), 4);

    let bar_file = files.iter().find(|f| f.path == "bar").expect("bar");
    // { kind: Conflicted, entry: { kind: 'conflicted', action: DeletedByThem,
    //   us: UpdatedButUnmerged, them: Deleted, submoduleStatus: undefined } }
    // (no conflictMarkerCount)
    assert_eq!(bar_file.status.kind, FileStatusKind::Conflicted);
    assert_eq!(bar_file.status.code, "UD");
    assert_eq!(bar_file.status.us(), GitStatusEntry::Unmerged);
    assert_eq!(bar_file.status.them(), GitStatusEntry::Deleted);
    assert_eq!(bar_file.status.submodule_status, None);
    assert!(!bar_file.status.submodule);
    assert_eq!(bar_file.status.conflict_markers, None);
}

// GHD: unit/git/status-test.ts › git/status › getStatus › with conflicted repo › parses conflicted files resulting from popping a stash
#[test]
fn parses_conflicted_files_resulting_from_popping_a_stash() {
    let repository = setup_empty_repository();
    let readme = repository.join("README.md");
    write_file(readme.clone(), "");
    exec(["add", "README.md"], repository.path());
    exec(["commit", "-m", "initial commit"], repository.path());

    // write a change to the readme into the stash
    append_file(&readme, generate_string(DEFAULT_STRING_LENGTH));
    exec(["stash"], repository.path());

    // write a different change to the README and commit it
    append_file(&readme, generate_string(DEFAULT_STRING_LENGTH));
    exec(["commit", "-am", "later commit"], repository.path());

    // pop the stash to introduce a conflict into the index
    exec(["stash", "pop"], repository.path());

    let status = get_status_or_throw(&repository);
    let files = &status.files;
    assert_eq!(files.len(), 1);

    assert_eq!(conflicted_count(files), 1);
}

// GHD: unit/git/status-test.ts › git/status › getStatus › with conflicted repo › parses resolved files
#[test]
fn parses_resolved_files() {
    let repository = setup_conflicted_repo_with_multiple_files();
    let file_path = repository.join("foo");

    write_file(file_path, "b1b2");
    let status = get_status_or_throw(&repository);
    let files = &status.files;

    assert_eq!(files.len(), 5);

    // all files are now considered conflicted
    assert_eq!(conflicted_count(files), 4);

    let file = files.iter().find(|f| f.path == "foo").expect("foo");
    // { kind: Conflicted, entry: { kind: 'conflicted', action: BothModified,
    //   them: UpdatedButUnmerged, us: UpdatedButUnmerged,
    //   submoduleStatus: undefined }, conflictMarkerCount: 0 }
    assert_eq!(file.status.kind, FileStatusKind::Conflicted);
    assert_eq!(file.status.code, "UU");
    assert_eq!(file.status.them(), GitStatusEntry::Unmerged);
    assert_eq!(file.status.us(), GitStatusEntry::Unmerged);
    assert_eq!(file.status.submodule_status, None);
    assert!(!file.status.submodule);
    assert_eq!(file.status.conflict_markers, Some(0));
}

// ---------------------------------------------------------------------------
// with conflicted images repo
// ---------------------------------------------------------------------------

// GHD: unit/git/status-test.ts › git/status › getStatus › with conflicted images repo › parses conflicted image file on merge
#[test]
#[ignore = "ghd: bug: a both-modified binary image is a text conflict (conflict_markers Some(0)) not a manual one: binary_paths runs diff --numstat -- <paths> (0 0 for an unmerged binary), GHD getBinaryPaths diffs against MERGE_HEAD (- -)"]
fn parses_conflicted_image_file_on_merge() {
    let repository = setup_fixture_repository("detect-conflict-in-binary-file");
    exec(["checkout", "make-a-change"], repository.path());

    exec(["merge", "master"], repository.path());

    let status = get_status_or_throw(&repository);
    let files = &status.files;
    assert_eq!(files.len(), 1);

    let file = &files[0];
    assert_eq!(file.status.kind, FileStatusKind::Conflicted);
    assert!(file.status.is_manual_conflict(), "{:?}", file.status);
}

// GHD: unit/git/status-test.ts › git/status › getStatus › with conflicted images repo › parses conflicted image file on merge after removing
#[test]
fn parses_conflicted_image_file_on_merge_after_removing() {
    let repository = setup_fixture_repository("detect-conflict-in-binary-file");

    exec(["rm", "my-cool-image.png"], repository.path());
    exec(["commit", "-am", "removed the image"], repository.path());

    exec(["merge", "master"], repository.path());

    let status = get_status_or_throw(&repository);
    let files = &status.files;
    assert_eq!(files.len(), 1);

    let file = &files[0];
    assert_eq!(file.status.kind, FileStatusKind::Conflicted);
    assert!(file.status.is_manual_conflict(), "{:?}", file.status);
}

// ---------------------------------------------------------------------------
// with unconflicted repo
// ---------------------------------------------------------------------------

// GHD: unit/git/status-test.ts › git/status › getStatus › with unconflicted repo › parses changed files
#[test]
fn parses_changed_files() {
    let repository = setup_fixture_repository("test-repo");

    write_file(repository.join("README.md"), "Hi world\n");

    let status = get_status_or_throw(&repository);
    let files = &status.files;
    assert_eq!(files.len(), 1);

    let file = &files[0];
    assert_eq!(file.path, "README.md");
    assert_eq!(file.status.kind, FileStatusKind::Modified);
}

// GHD: unit/git/status-test.ts › git/status › getStatus › with unconflicted repo › returns an empty array when there are no changes
#[test]
fn returns_an_empty_array_when_there_are_no_changes() {
    let repository = setup_fixture_repository("test-repo");

    let status = get_status_or_throw(&repository);
    let files = &status.files;
    assert_eq!(files.len(), 0);
}

// GHD: unit/git/status-test.ts › git/status › getStatus › with unconflicted repo › reflects renames
#[test]
#[ignore = "ghd: missing: corvene_models::FileStatus has no renameIncludesModifications (models/status.ts, convertToAppStatus in lib/git/status.ts)"]
fn reflects_renames() {
    let repo = setup_empty_repository();

    write_file(repo.join("foo"), "foo\n");

    exec(["add", "foo"], repo.path());
    exec(["commit", "-m", "Initial commit"], repo.path());
    exec(["mv", "foo", "bar"], repo.path());

    let status = get_status_or_throw(&repo);
    let files = &status.files;

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, "bar");
    // { kind: Renamed, oldPath: 'foo', renameIncludesModifications: false,
    //   submoduleStatus: undefined }
    assert_eq!(files[0].status.kind, FileStatusKind::Renamed);
    assert_eq!(files[0].old_path.as_deref(), Some("foo"));
    assert_eq!(files[0].status.submodule_status, None);
    assert!(!files[0].status.submodule);
    // (no conflictMarkerCount key)
    assert_eq!(files[0].status.conflict_markers, None);
    assert!(!rename_includes_modifications(&files[0].status));
}

// GHD: unit/git/status-test.ts › git/status › getStatus › with unconflicted repo › reflects copies
#[test]
#[ignore = "ghd: missing: corvene_models::FileStatus has no renameIncludesModifications (models/status.ts, convertToAppStatus in lib/git/status.ts)"]
fn reflects_copies() {
    let repository = setup_fixture_repository("copy-detection-status");

    // Git 2.18 now uses a new config value to handle detecting copies, so
    // users who have this enabled will see this. For reference, Desktop does
    // not enable this by default.
    setup_local_config(&repository, [("status.renames", "copies")]);

    exec(["add", "."], repository.path());

    let status = get_status_or_throw(&repository);
    let files = &status.files;

    assert_eq!(files.len(), 2);

    assert_eq!(files[0].status.kind, FileStatusKind::Modified);
    assert_eq!(files[0].path, "CONTRIBUTING.md");

    assert_eq!(files[1].path, "docs/OVERVIEW.md");
    // { kind: Copied, oldPath: 'CONTRIBUTING.md', submoduleStatus: undefined,
    //   renameIncludesModifications: false }
    assert_eq!(files[1].status.kind, FileStatusKind::Copied);
    assert_eq!(files[1].old_path.as_deref(), Some("CONTRIBUTING.md"));
    assert_eq!(files[1].status.submodule_status, None);
    assert!(!files[1].status.submodule);
    // (no conflictMarkerCount key)
    assert_eq!(files[1].status.conflict_markers, None);
    assert!(!rename_includes_modifications(&files[1].status));
}

// GHD: unit/git/status-test.ts › git/status › getStatus › with unconflicted repo › returns null for directory without a .git directory
#[test]
fn returns_null_for_directory_without_a_git_directory() {
    let repository = setup_empty_directory();
    let status = corvene_git::get_status(git(), repository.path(), None);
    assert!(
        matches!(
            status,
            Err(GitError::Failed {
                code: Some(128),
                ..
            })
        ),
        "expected null (git exited with 128), got {status:?}"
    );
}

// ---------------------------------------------------------------------------
// with submodules
// ---------------------------------------------------------------------------

/// The changes GitHub Desktop's `checkSubmoduleChanges` expects.
struct SubmoduleChanges {
    modified_changes: bool,
    untracked_changes: bool,
    commit_changed: bool,
}

// GHD: unit/git/status-test.ts › git/status › getStatus › with submodules › returns the submodule status
#[test]
fn returns_the_submodule_status() {
    let repository = setup_fixture_repository("submodule-basic-setup");

    let submodule_path = repository.join("foo").join("submodule");
    let check_submodule_changes = |changes: SubmoduleChanges| {
        let status = get_status_or_throw(&repository);
        let files = &status.files;
        assert_eq!(files.len(), 1);

        let file = &files[0];
        assert_eq!(file.path, "foo/submodule");
        assert_eq!(file.status.kind, FileStatusKind::Modified);
        let submodule_status: Option<SubmoduleStatus> = file.status.submodule_status;
        assert_eq!(
            submodule_status.map(|s| s.modified_changes),
            Some(changes.modified_changes)
        );
        assert_eq!(
            submodule_status.map(|s| s.untracked_changes),
            Some(changes.untracked_changes)
        );
        assert_eq!(
            submodule_status.map(|s| s.commit_changed),
            Some(changes.commit_changed)
        );
    };

    // Modify README.md file. Now the submodule has modified changes.
    write_file(submodule_path.join("README.md"), "hello world\n");
    check_submodule_changes(SubmoduleChanges {
        modified_changes: true,
        untracked_changes: false,
        commit_changed: false,
    });

    // Create untracked file in submodule. Now the submodule has both
    // modified and untracked changes.
    write_file(submodule_path.join("test"), "test\n");
    check_submodule_changes(SubmoduleChanges {
        modified_changes: true,
        untracked_changes: true,
        commit_changed: false,
    });

    // Commit the changes within the submodule. Now the submodule has commit
    // changes.
    exec(["add", "."], &submodule_path);
    exec(["commit", "-m", "changes"], &submodule_path);
    check_submodule_changes(SubmoduleChanges {
        modified_changes: false,
        untracked_changes: false,
        commit_changed: true,
    });
}
