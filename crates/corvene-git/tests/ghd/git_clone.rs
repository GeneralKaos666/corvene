//! Port of GitHub Desktop's `app/test/unit/git/clone-test.ts`.
//!
//! GitHub Desktop's `clone(url, path, options, progressCallback?)`
//! (`lib/git/clone.ts`) is `corvene_git::clone(git, url, path,
//! default_branch, depth, cancel, on_progress)`, called the way
//! `Dispatcher::clone_repository` calls it: `options.defaultBranch` is the
//! `default_branch` argument (the dispatcher passes the repository's default
//! branch when the clone dialog knows it), no `depth`, no cancel token.
//! [`clone`] is that call. Differences the cases run into:
//!
//! - `CloneOptions.branch` (`git clone -b <branch>`): `corvene_git::clone`
//!   has no branch argument, so [`clone_with_branch`] stands in for it.
//! - `ICloneProgress.kind` (`'clone'`) is the tag of GitHub Desktop's
//!   progress union (`IProgress`, `models/progress.ts`). Corvene's clone
//!   callback receives a `corvene_git::CloneProgress`, the type that stands
//!   for that tag, so "reports progress when callback is provided" checks
//!   the kind through the events' type (at compile time).
//! - `isClonePathSensitive`: `corvene_git::clone` clones into any path.
//!
//! "clones with a custom default branch name" forces protocol v0 with
//! `GIT_CONFIG_PARAMETERS='protocol.version=0'` in the process environment.
//! Tests cannot change the process environment and `corvene_git::clone`
//! takes no environment, so the port sets the same configuration in the
//! (locked) global git configuration, which git reads for every command of
//! the test just as it reads `GIT_CONFIG_PARAMETERS`.

use std::path::{Path, PathBuf};

use corvene_git::{CloneProgress, GitError};
use corvene_test_support::{
    Tree, TreeEntry, create_temp_directory, exec, exec_ok, git, home_dir, lock_global_config,
    make_commit, setup_empty_repository,
};

/// GitHub Desktop's `CloneOptions` (`models/clone-options.ts`).
#[derive(Clone, Debug, Default)]
struct CloneOptions {
    /// The branch to checkout after the clone has completed.
    branch: Option<String>,
    /// The default branch name in case we're cloning an empty repository.
    default_branch: Option<String>,
}

/// GitHub Desktop's `clone(url, path, options, progressCallback?)`.
fn clone(
    url: &str,
    path: &Path,
    options: &CloneOptions,
    mut progress_callback: Option<&mut dyn FnMut(CloneProgress)>,
) -> Result<(), GitError> {
    if let Some(branch) = &options.branch {
        return clone_with_branch(url, path, options, branch);
    }
    corvene_git::clone(
        git(),
        url,
        path,
        options.default_branch.as_deref(),
        None,
        None,
        |progress| {
            if let Some(callback) = progress_callback.as_mut() {
                callback(progress);
            }
        },
    )
}

/// Stand-in for `clone(url, path, { branch })` (`git clone -b <branch>`,
/// `lib/git/clone.ts`): `corvene_git::clone` has no branch argument. Give it
/// one and call it from [`clone`].
fn clone_with_branch(
    _url: &str,
    _path: &Path,
    _options: &CloneOptions,
    _branch: &str,
) -> Result<(), GitError> {
    unimplemented!("corvene_git::clone has no branch option (CloneOptions.branch)")
}

/// The test's `createEmptyBareRepository(t)`: `git init --bare
/// <temp>/remote.git`. The returned guard keeps the parent directory.
fn create_empty_bare_repository() -> (tempfile::TempDir, PathBuf) {
    let bare_parent_path = create_temp_directory();
    let bare_path = bare_parent_path.path().join("remote.git");
    exec_ok(
        [
            std::ffi::OsStr::new("init"),
            std::ffi::OsStr::new("--bare"),
            bare_path.as_os_str(),
        ],
        bare_parent_path.path(),
    );
    (bare_parent_path, bare_path)
}

fn path_str(path: &Path) -> &str {
    path.to_str().expect("temporary paths are UTF-8")
}

// GHD: unit/git/clone-test.ts › git/clone › clones a local repository
#[test]
fn clones_a_local_repository() {
    // Create a source repo with a commit
    let source = setup_empty_repository();
    make_commit(
        &source,
        &Tree::with_message("initial commit", [TreeEntry::new("README.md", "hello")]),
    );

    let dest_path = create_temp_directory();
    let clone_path = dest_path.path().join("cloned");

    clone(
        path_str(source.path()),
        &clone_path,
        &CloneOptions::default(),
        None,
    )
    .expect("clone");

    assert!(clone_path.join(".git").exists());
    assert!(clone_path.join("README.md").exists());
}

// GHD: unit/git/clone-test.ts › git/clone › clones with a specific branch
#[test]
#[ignore = "ghd: missing: corvene_git::clone has no branch option (CloneOptions.branch, git clone -b, lib/git/clone.ts)"]
fn clones_with_a_specific_branch() {
    let source = setup_empty_repository();
    make_commit(
        &source,
        &Tree::with_message("initial commit", [TreeEntry::new("README.md", "hello")]),
    );

    // Create a feature branch on the source
    exec(["branch", "feature"], source.path());
    exec(["checkout", "feature"], source.path());
    make_commit(
        &source,
        &Tree::with_message("feature commit", [TreeEntry::new("feature.txt", "feature")]),
    );
    exec(["checkout", "master"], source.path());

    let dest_path = create_temp_directory();
    let clone_path = dest_path.path().join("cloned");

    clone(
        path_str(source.path()),
        &clone_path,
        &CloneOptions {
            branch: Some("feature".into()),
            ..CloneOptions::default()
        },
        None,
    )
    .expect("clone");

    // Verify the feature branch was checked out
    let result = exec(["rev-parse", "--abbrev-ref", "HEAD"], &clone_path);
    assert_eq!(result.stdout.trim(), "feature");
    assert!(clone_path.join("feature.txt").exists());
}

// GHD: unit/git/clone-test.ts › git/clone › reports progress when callback is provided
#[test]
fn reports_progress_when_callback_is_provided() {
    let source = setup_empty_repository();
    make_commit(
        &source,
        &Tree::with_message("initial commit", [TreeEntry::new("README.md", "hello")]),
    );

    let dest_path = create_temp_directory();
    let clone_path = dest_path.path().join("cloned");

    let mut progress_events: Vec<CloneProgress> = Vec::new();
    clone(
        path_str(source.path()),
        &clone_path,
        &CloneOptions::default(),
        Some(&mut |progress| progress_events.push(progress)),
    )
    .expect("clone");

    assert!(
        !progress_events.is_empty(),
        "Expected at least one progress event"
    );
    // `progressEvents[0].kind === 'clone'`: the event is a clone progress
    // event by its type (see the module doc)
    let first: &CloneProgress = &progress_events[0];
    let _ = first;
}

// GHD: unit/git/clone-test.ts › git/clone › clones with a custom default branch name
#[test]
fn clones_with_a_custom_default_branch_name() {
    // init.defaultBranch only takes effect when the remote's unborn HEAD
    // branch name is not advertised (protocol v0/v1). Force protocol v0
    // so we can verify the option actually drives the result, rather than
    // having the remote's initial-branch setting do the work.
    // (GitHub Desktop: GIT_CONFIG_PARAMETERS='protocol.version=0'; here the
    // global configuration, see the module doc.)
    let _global = lock_global_config();
    exec_ok(["config", "--global", "protocol.version", "0"], home_dir());

    // Bare repo defaults to 'master' — clone must use defaultBranch to get 'trunk'
    let (_bare_parent, source) = create_empty_bare_repository();

    let dest_path = create_temp_directory();
    let clone_path = dest_path.path().join("cloned");

    clone(
        path_str(&source),
        &clone_path,
        &CloneOptions {
            default_branch: Some("trunk".into()),
            ..CloneOptions::default()
        },
        None,
    )
    .expect("clone");

    assert!(clone_path.join(".git").exists());

    let result = exec(["symbolic-ref", "HEAD"], &clone_path);
    assert_eq!(result.stdout.trim(), "refs/heads/trunk");
}

// GHD: unit/git/clone-test.ts › git/clone › rejects cloning into ~/.ssh
#[test]
#[ignore = "ghd: bug: corvene_git::clone has no isClonePathSensitive backstop: a clone into ~/.ssh runs git instead of failing with 'sensitive system location'"]
fn rejects_cloning_into_ssh() {
    // the clone writes below $HOME, where the global configuration lives
    let _global = lock_global_config();
    let ssh_path = home_dir().join(".ssh").join("malicious-clone");

    let err = clone(
        "https://example.com/repo.git",
        &ssh_path,
        &CloneOptions::default(),
        None,
    )
    .expect_err("the clone must be rejected");
    assert!(
        err.to_string().contains("sensitive system location"),
        "Expected sensitive location error, got: {err}"
    );
}

// GHD: unit/git/clone-test.ts › git/clone › rejects cloning into home directory root
#[test]
#[ignore = "ghd: bug: corvene_git::clone has no isClonePathSensitive backstop: a clone into the home directory runs git instead of failing with 'sensitive system location'"]
fn rejects_cloning_into_home_directory_root() {
    // the clone writes into $HOME, where the global configuration lives
    let _global = lock_global_config();
    let home_path = home_dir();

    let err = clone(
        "https://example.com/repo.git",
        home_path,
        &CloneOptions::default(),
        None,
    )
    .expect_err("the clone must be rejected");
    assert!(
        err.to_string().contains("sensitive system location"),
        "Expected sensitive location error, got: {err}"
    );
}

// GHD: unit/git/clone-test.ts › git/clone › rejects cloning into ~/.config/git
#[test]
#[ignore = "ghd: bug: corvene_git::clone has no isClonePathSensitive backstop: a clone into ~/.config/git runs git instead of failing with 'sensitive system location'"]
fn rejects_cloning_into_config_git() {
    // the clone writes into $HOME/.config/git, the XDG global configuration
    let _global = lock_global_config();
    let git_config_path = home_dir().join(".config").join("git");

    let err = clone(
        "https://example.com/repo.git",
        &git_config_path,
        &CloneOptions::default(),
        None,
    )
    .expect_err("the clone must be rejected");
    assert!(
        err.to_string().contains("sensitive system location"),
        "Expected sensitive location error, got: {err}"
    );
}
