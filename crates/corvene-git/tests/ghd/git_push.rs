//! Port of GitHub Desktop's `app/test/unit/git/push-test.ts`.
//!
//! GitHub Desktop's `push(repository, remote, localBranch, remoteBranch,
//! tagsToPush, options?, progressCallback?)` (`lib/git/push.ts`) is
//! `corvene_git::push(git, path, remote.name, local_branch, remote_branch,
//! tags, force_with_lease, askpass, on_progress)`: `tagsToPush: null` is
//! `&[]` (both add no tag), `options.forceWithLease` is `force_with_lease`,
//! no askpass, and a progress callback that drops every event where GitHub
//! Desktop passes none (Corvene always passes `--progress`, which only
//! changes what git writes to stderr).
//!
//! GitHub Desktop's progress callback receives `IPushProgress` (`kind:
//! 'push'`, title, value, remote, branch), first with an initial event sent
//! before git runs. That is `corvene_git::push_with_progress`
//! ([`push_with_progress_callback`]); `corvene_git::push` reports `(percent,
//! description)` of the same events minus the initial one.

use std::ffi::OsStr;
use std::path::Path;

use corvene_git::GitError;
use corvene_models::Remote;
use corvene_test_support::{
    TestRepo, Tree, TreeEntry, create_bare_upstream, exec, git, make_commit, setup_empty_repository,
};

/// `{ name: 'origin', url: barePath }`.
fn origin(bare_path: &Path) -> Remote {
    Remote {
        name: "origin".into(),
        url: bare_path.to_string_lossy().into_owned(),
    }
}

/// `git remote add origin <barePath>`.
fn add_origin(repo: &TestRepo, bare_path: &Path) {
    exec(
        [
            OsStr::new("remote"),
            OsStr::new("add"),
            OsStr::new("origin"),
            bare_path.as_os_str(),
        ],
        repo.path(),
    );
}

/// GitHub Desktop's `push(repo, remote, localBranch, remoteBranch, null,
/// options)` without a progress callback.
fn push(
    repo: &TestRepo,
    remote: &Remote,
    local_branch: &str,
    remote_branch: Option<&str>,
    force_with_lease: bool,
) -> Result<(), GitError> {
    corvene_git::push(
        git(),
        repo.path(),
        &remote.name,
        local_branch,
        remote_branch,
        &[],
        force_with_lease,
        None,
        &mut |_, _| {},
    )
}

/// The part of GitHub Desktop's `IPushProgress` (`models/progress.ts`) the
/// test reads.
struct PushProgress {
    kind: &'static str,
}

/// GitHub Desktop's `push(repository, remote, localBranch, remoteBranch,
/// null, undefined, progressCallback)`: `corvene_git::push_with_progress`,
/// whose callback receives `IPushProgress` (`corvene_git::PushProgress`).
fn push_with_progress_callback(
    repo: &TestRepo,
    remote: &Remote,
    local_branch: &str,
    remote_branch: Option<&str>,
    progress_callback: &mut dyn FnMut(PushProgress),
) -> Result<(), GitError> {
    corvene_git::push_with_progress(
        git(),
        repo.path(),
        &remote.name,
        local_branch,
        remote_branch,
        &[],
        false,
        None,
        &mut |progress| {
            progress_callback(PushProgress {
                kind: progress.kind,
            })
        },
    )
}

// GHD: unit/git/push-test.ts › git/push › pushes commits to a local remote
#[test]
fn pushes_commits_to_a_local_remote() {
    let repo = setup_empty_repository();
    make_commit(
        &repo,
        &Tree::with_message("initial commit", [TreeEntry::new("README.md", "initial")]),
    );

    // Create a bare upstream and add it as origin
    let bare_path = create_bare_upstream(&repo);
    add_origin(&repo, bare_path.path());

    // Create a new commit to push
    make_commit(
        &repo,
        &Tree::with_message(
            "add new file",
            [TreeEntry::new("new-file.txt", "new content")],
        ),
    );

    let remote = origin(bare_path.path());
    push(&repo, &remote, "master", Some("master"), false)
        .unwrap_or_else(|err| panic!("push: {err}"));

    // Verify the bare upstream received the commit
    let result = exec(["log", "--oneline"], bare_path.path());
    assert!(result.stdout.contains("add new file"));
}

// GHD: unit/git/push-test.ts › git/push › pushes with --set-upstream for a new branch
#[test]
fn pushes_with_set_upstream_for_a_new_branch() {
    let repo = setup_empty_repository();
    make_commit(
        &repo,
        &Tree::with_message("initial commit", [TreeEntry::new("README.md", "initial")]),
    );

    let bare_path = create_bare_upstream(&repo);
    add_origin(&repo, bare_path.path());

    // Create and switch to a new branch
    exec(["checkout", "-b", "new-branch"], repo.path());
    make_commit(
        &repo,
        &Tree::with_message(
            "branch commit",
            [TreeEntry::new("branch-file.txt", "branch content")],
        ),
    );

    let remote = origin(bare_path.path());

    // Push with remoteBranch=null should set upstream
    push(&repo, &remote, "new-branch", None, false).unwrap_or_else(|err| panic!("push: {err}"));

    // Verify the branch exists on the bare upstream
    let result = exec(["rev-parse", "--verify", "new-branch"], bare_path.path());
    assert_eq!(result.exit_code, 0);

    let upstream = exec(
        [
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            "@{upstream}",
        ],
        repo.path(),
    );
    assert_eq!(upstream.stdout.trim(), "origin/new-branch");
}

// GHD: unit/git/push-test.ts › git/push › pushes with force-with-lease
#[test]
fn pushes_with_force_with_lease() {
    let repo = setup_empty_repository();
    make_commit(
        &repo,
        &Tree::with_message("initial commit", [TreeEntry::new("README.md", "initial")]),
    );

    make_commit(
        &repo,
        &Tree::with_message(
            "commit before rewrite",
            [TreeEntry::new("README.md", "before rewrite")],
        ),
    );

    let bare_path = create_bare_upstream(&repo);
    add_origin(&repo, bare_path.path());

    let remote = origin(bare_path.path());
    push(&repo, &remote, "master", Some("master"), false)
        .unwrap_or_else(|err| panic!("push: {err}"));

    exec(["reset", "--hard", "HEAD~1"], repo.path());
    std::fs::write(repo.join("README.md"), "rewritten history").unwrap();
    exec(["add", "README.md"], repo.path());
    exec(["commit", "-m", "rewritten commit"], repo.path());

    assert!(push(&repo, &remote, "master", Some("master"), false).is_err());

    push(&repo, &remote, "master", Some("master"), true)
        .unwrap_or_else(|err| panic!("push --force-with-lease: {err}"));

    // Verify the bare upstream has the rewritten commit
    let result = exec(["log", "--oneline", "-1"], bare_path.path());
    assert!(result.stdout.contains("rewritten commit"));
}

// GHD: unit/git/push-test.ts › git/push › reports progress when callback is provided
#[test]
fn reports_progress_when_callback_is_provided() {
    let repo = setup_empty_repository();
    make_commit(
        &repo,
        &Tree::with_message("initial commit", [TreeEntry::new("README.md", "initial")]),
    );

    let bare_path = create_bare_upstream(&repo);
    add_origin(&repo, bare_path.path());

    make_commit(
        &repo,
        &Tree::with_message("new commit", [TreeEntry::new("file.txt", "content")]),
    );

    let remote = origin(bare_path.path());
    let mut progress_events: Vec<PushProgress> = Vec::new();

    push_with_progress_callback(&repo, &remote, "master", Some("master"), &mut |progress| {
        progress_events.push(PushProgress {
            kind: progress.kind,
        })
    })
    .unwrap_or_else(|err| panic!("push: {err}"));

    // At minimum we should get the initial progress event
    assert!(
        !progress_events.is_empty(),
        "Expected at least one progress event"
    );
    assert_eq!(progress_events[0].kind, "push");
}
