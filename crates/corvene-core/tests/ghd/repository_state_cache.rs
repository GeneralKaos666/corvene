//! Port of GitHub Desktop's `app/test/unit/repository-state-cache-test.ts`.
//!
//! GitHub Desktop's `RepositoryStateCache` (`lib/stores/repository-state-cache.ts`)
//! keeps an `IRepositoryState` per repository; `update*State(repository, fn)`
//! merges what `fn` returns into it and `get(repository)` reads it. In
//! Corvene that state is `AppState::repo_states`: `AppState::repo_state_mut`
//! creates or returns a repository's `RepositoryState` and the dispatcher
//! writes its fields in place, so an update here sets the fields the
//! updater function returns ([`crate::stores_support::app_state`] builds
//! the `AppState`). Where each part of `IRepositoryState` lives in Corvene:
//!
//! - `branchesState.openPullRequests` / `isLoadingPullRequests`: the
//!   `PullRequestCache` of the repository's GitHub repository in
//!   `AppState::pull_requests` (keyed by `cache_key`), read through
//!   `AppState::pull_requests_for` / `pull_requests_loading`. Corvene keys
//!   pull requests by GitHub repository, so the repository is given the
//!   GitHub repository its pull request belongs to (GitHub Desktop's has
//!   none, its cache is keyed by the local repository).
//! - `changesState.workingDirectory` / `showCoAuthoredBy`:
//!   `RepositoryState::status` / `show_co_authored_by`. Corvene keeps the
//!   commit message in the commit form's text inputs (`corvene-ui`), not in
//!   the repository state, so `commitMessage` goes through the stand-ins
//!   [`set_commit_message`] / [`commit_message`].
//! - `compareState.formState` / `commitSHAs`: `RepositoryState::compare`'s
//!   `form` / `commits` (whole commits, so a commit with that sha). The
//!   compare branch filter text lives in the compare view's input, so
//!   `filterText` goes through the stand-ins [`set_compare_filter_text`] /
//!   [`compare_filter_text`].
//!
//! - `gitHubRepoFixture({ name, owner })` is
//!   `corvene_test_support::git_hub_repo_fixture`.
//! - `createSamplePullRequest(gitHubRepository)` is
//!   [`create_sample_pull_request`].

use std::path::PathBuf;

use corvene_core::compare::CompareForm;
use corvene_core::pull_requests::cache_key;
use corvene_core::state::RepositoryState;
use corvene_core::{
    Commit, CommitIdentity, DiffSelection, FileStatus, FileStatusKind, GitHubRepository,
    GitStatusEntry, PullRequest, PullRequestRef, Repository, WorkingDirectoryFileChange,
    WorkingDirectoryStatus,
};

use corvene_test_support::{GitHubRepoFixtureOptions, git_hub_repo_fixture};

use crate::stores_support::app_state;

/// Stand-in for the `commitMessage` of GitHub Desktop's `IChangesState`
/// (`updateChangesState(repository, () => ({ commitMessage }))`). Corvene's
/// commit message lives in the commit form, not in `RepositoryState`;
/// replace this once the repository state carries it and remove the
/// `#[ignore]`.
fn set_commit_message(_state: &mut RepositoryState, _summary: &str, _description: Option<&str>) {
    unimplemented!("RepositoryState has no commit message")
}

/// Stand-in for reading `changesState.commitMessage.summary`.
fn commit_message(_state: &RepositoryState) -> (String, Option<String>) {
    unimplemented!("RepositoryState has no commit message")
}

/// Stand-in for the `filterText` of GitHub Desktop's `ICompareState`.
/// Corvene's compare filter text lives in the compare view's input, not in
/// `CompareState`; replace this once it does and remove the `#[ignore]`.
fn set_compare_filter_text(_state: &mut RepositoryState, _filter_text: &str) {
    unimplemented!("CompareState has no filter text")
}

/// Stand-in for reading `compareState.filterText`.
fn compare_filter_text(_state: &RepositoryState) -> String {
    unimplemented!("CompareState has no filter text")
}

/// `new Repository('/something/path', 1, null, false)`.
fn repository() -> Repository {
    Repository::new(1, PathBuf::from("/something/path"))
}

/// `createSamplePullRequest(gitHubRepository)`: `new PullRequest(new Date(),
/// 'something', 1, head, base, 'shiftkey', false, 'something body')`. The
/// creation date (`new Date()`) is not read by the case.
fn create_sample_pull_request(github_repository: &GitHubRepository) -> PullRequest {
    PullRequest {
        number: 1,
        title: "something".to_string(),
        created_at: "2026-01-01T00:00:00Z".to_string(),
        updated_at: "2026-01-01T00:00:00Z".to_string(),
        head: PullRequestRef {
            ref_name: "refs/heads/master".to_string(),
            sha: "deadbeef".to_string(),
            repository: Some(github_repository.clone()),
        },
        base: PullRequestRef {
            ref_name: "refs/heads/my-cool-feature".to_string(),
            sha: "deadbeef".to_string(),
            repository: Some(github_repository.clone()),
        },
        author: "shiftkey".to_string(),
        draft: false,
        body: "something body".to_string(),
    }
}

// GHD: unit/repository-state-cache-test.ts › RepositoryStateCache › can update branches state for a repository
#[test]
fn can_update_branches_state_for_a_repository() {
    let mut repository = repository();
    let github_repository = git_hub_repo_fixture(GitHubRepoFixtureOptions {
        name: "desktop",
        owner: "desktop",
        ..Default::default()
    });
    let first_pull_request = create_sample_pull_request(&github_repository);
    // Corvene keys pull requests by GitHub repository (see the module docs)
    repository.github = Some(github_repository.clone());

    let mut cache = app_state();
    cache.repositories.push(repository.clone());

    // cache.updateBranchesState(repository, () => ({ openPullRequests, isLoadingPullRequests }))
    let branches_state = cache
        .pull_requests
        .entry(cache_key(&github_repository))
        .or_default();
    branches_state.pull_requests = vec![first_pull_request];
    branches_state.loading = true;

    assert!(cache.pull_requests_loading(repository.id));
    assert_eq!(cache.pull_requests_for(repository.id).len(), 1);
}

// GHD: unit/repository-state-cache-test.ts › RepositoryStateCache › can update changes state for a repository
#[test]
#[ignore = "ghd: missing: RepositoryState has no commit message (changesState.commitMessage); Corvene keeps it in the corvene-ui commit form inputs"]
fn can_update_changes_state_for_a_repository() {
    let repository = repository();
    let files = vec![WorkingDirectoryFileChange {
        path: "README.md".to_string(),
        old_path: None,
        status: FileStatus {
            kind: FileStatusKind::New,
            index: GitStatusEntry::Added,
            working_tree: GitStatusEntry::Unchanged,
            score: None,
            code: "A.".to_string(),
            submodule: false,
            submodule_status: None,
            conflict_markers: None,
        },
        selection: DiffSelection::all(),
    }];

    let summary = "Hello world!";

    let mut cache = app_state();

    // cache.updateChangesState(repository, () => ({ workingDirectory, commitMessage, showCoAuthoredBy }))
    let changes_state = cache.repo_state_mut(repository.id);
    changes_state.status = Some(WorkingDirectoryStatus {
        files,
        ..Default::default()
    });
    set_commit_message(changes_state, summary, None);
    changes_state.show_co_authored_by = true;

    let changes_state = cache.repo_state_mut(repository.id);
    let working_directory = changes_state.status.as_ref().expect("workingDirectory");
    assert_eq!(working_directory.include_all(), Some(true));
    assert_eq!(working_directory.files.len(), 1);
    assert!(changes_state.show_co_authored_by);
    assert_eq!(commit_message(changes_state).0, summary);
}

// GHD: unit/repository-state-cache-test.ts › RepositoryStateCache › can update compare state for a repository
#[test]
#[ignore = "ghd: missing: CompareState has no filter text (compareState.filterText); Corvene keeps it in the corvene-ui compare branch list input"]
fn can_update_compare_state_for_a_repository() {
    let repository = repository();
    let filter_text = "my-cool-branch";

    let mut cache = app_state();

    // cache.updateCompareState(repository, () => ({ formState: { kind: History }, filterText, commitSHAs }))
    let state = cache.repo_state_mut(repository.id);
    state.compare.form = CompareForm::History;
    set_compare_filter_text(state, filter_text);
    state.compare.commits = vec![commit_with_sha("deadbeef")];

    let state = cache.repo_state_mut(repository.id);
    assert_eq!(state.compare.form, CompareForm::History);
    assert_eq!(compare_filter_text(state), filter_text);
    assert_eq!(state.compare.commits.len(), 1);
}

/// A commit known only by its sha (`commitSHAs: ['deadbeef']`).
fn commit_with_sha(sha: &str) -> Commit {
    let identity = CommitIdentity {
        name: String::new(),
        email: String::new(),
        seconds: 0,
        offset: 0,
    };
    Commit {
        sha: sha.to_string(),
        summary: String::new(),
        body: String::new(),
        author: identity.clone(),
        committer: identity,
        parents: Vec::new(),
        tags: Vec::new(),
    }
}
