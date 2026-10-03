//! Helpers shared by lane `rebase`'s ports of GitHub Desktop's rebase tests
//! (`rebase-full-test.ts`, `rebase/detect-conflict-test.ts`,
//! `rebase/progress-test.ts`).
//!
//! GitHub Desktop's `rebase` and `continueRebase` (`lib/git/rebase.ts`)
//! work out the commits their progress parser names themselves; Corvene's
//! `corvene_git::rebase` / `corvene_git::continue_rebase` take them from the
//! caller (the dispatcher passes `commits_between(base, HEAD)` from the
//! rebase preview, or a found rebase's `rebase_snapshot` commits). The
//! stand-ins below make the calls GitHub Desktop's functions make, with
//! GitHub Desktop's value of flag `834-rebase-keeps-hash-messages` (off).

use std::collections::BTreeMap;

use corvene_git::RebaseResult;
use corvene_models::{Branch, ManualConflictResolution, McoProgress, WorkingDirectoryFileChange};
use corvene_test_support::{TestRepo, git};

/// GitHub Desktop's `rebase(repository, baseBranch, targetBranch,
/// progressCallback?)`: `git -c rebase.backend=merge rebase <base> <target>`
/// (`corvene_git::rebase`). With a callback, the commits its progress names
/// are `getCommitsBetweenCommits(base.tip.sha, target.tip.sha)`
/// (`corvene_git::commits_between`), and a range git cannot resolve is
/// `RebaseResult.Error`. Past that point GitHub Desktop's `rebase` never
/// returns `RebaseResult.Error`: `git()` or `parseRebaseResult` throws
/// (`Unhandled result found`), which fails the test even where it ignores
/// the result. Corvene's `RebaseResult::Error(_)` from `corvene_git::rebase`
/// is that throw, so this panics on it.
pub fn rebase(
    repository: &TestRepo,
    base_branch: &Branch,
    target_branch: &Branch,
    progress_callback: Option<&mut dyn FnMut(McoProgress)>,
) -> RebaseResult {
    let mut commits = Vec::new();
    if progress_callback.is_some() {
        let found = corvene_git::commits_between(
            git(),
            repository.path(),
            base_branch.tip.as_deref().expect("base branch has a tip"),
            target_branch
                .tip
                .as_deref()
                .expect("target branch has a tip"),
        )
        .expect("getCommitsBetweenCommits");
        match found {
            Some(found) => commits = found,
            None => {
                return RebaseResult::Error(
                    "one or both of the refs do not exist in the repository".into(),
                );
            }
        }
    }
    let mut progress_callback = progress_callback;
    let result = corvene_git::rebase(
        git(),
        repository.path(),
        &base_branch.name,
        &target_branch.name,
        &commits,
        false,
        |p| {
            if let Some(callback) = progress_callback.as_mut() {
                callback(p);
            }
        },
    );
    if let RebaseResult::Error(error) = &result {
        panic!("rebase threw: Unhandled result found: {error}");
    }
    result
}

/// GitHub Desktop's `abortRebase(repository)` (`corvene_git::abort_rebase`;
/// an `Err` is GitHub Desktop's rejection).
pub fn abort_rebase(repository: &TestRepo) {
    corvene_git::abort_rebase(git(), repository.path()).expect("abortRebase");
}

/// GitHub Desktop's `continueRebase(repository, files, manualResolutions,
/// { progressCallback })` (`corvene_git::continue_rebase`). With a callback,
/// the commits its progress names are those of `getRebaseSnapshot`
/// (`corvene_git::rebase_snapshot`), and no snapshot is
/// `RebaseResult.Aborted`. GitHub Desktop's `continueRebase` never returns
/// `RebaseResult.Error`; it throws (`git()`, or `parseRebaseResult`'s
/// `Unhandled result found`). An `Err`, or an `Ok(RebaseResult::Error(_))`,
/// is that throw and panics.
pub fn continue_rebase(
    repository: &TestRepo,
    files: &[WorkingDirectoryFileChange],
    manual_resolutions: &BTreeMap<String, ManualConflictResolution>,
    progress_callback: Option<&mut dyn FnMut(McoProgress)>,
) -> RebaseResult {
    let mut commits = Vec::new();
    if progress_callback.is_some() {
        match corvene_git::rebase_snapshot(git(), repository.path()) {
            Some(snapshot) => commits = snapshot.commits,
            None => return RebaseResult::Aborted,
        }
    }
    let mut progress_callback = progress_callback;
    let result = corvene_git::continue_rebase(
        git(),
        repository.path(),
        files,
        manual_resolutions,
        &commits,
        false,
        |p| {
            if let Some(callback) = progress_callback.as_mut() {
                callback(p);
            }
        },
    )
    .expect("continueRebase");
    if let RebaseResult::Error(error) = &result {
        panic!("continueRebase threw: Unhandled result found: {error}");
    }
    result
}
