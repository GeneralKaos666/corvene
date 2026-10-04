//! Port of GitHub Desktop's `app/test/unit/find-forked-remotes-to-prune-test.ts`.
//!
//! Corvene equivalent: `findForkedRemotesToPrune(remotes, openPRs,
//! allBranches)` (`lib/stores/helpers/find-forked-remotes-to-prune.ts`) is
//! `corvene_core::pull_requests::forked_remotes_to_prune`, which returns the
//! names of the remotes to prune (GitHub Desktop returns the `IRemote`s and
//! the test maps them to names with `getNamesFromRemotes`). It compares a
//! pull request's head repository clone URL with a remote's URL through
//! `corvene_models::url_matches_remote` (GitHub Desktop: string equality).
//!
//! GitHub Desktop's models are the `corvene_models` types (re-exported by
//! `corvene_core`): `IRemote` is `Remote`, `PullRequest` / `PullRequestRef`
//! are `PullRequest` / `PullRequestRef` (`gitHubRepository` is
//! `repository`, `ref` is `ref_name`, the `Date` is `created_at` as the
//! API's ISO-8601 text), `GitHubRepository` is `GitHubRepository`
//! (`corvene_test_support::git_hub_repo_fixture` for
//! `helpers/github-repo-builder.ts` `gitHubRepoFixture`), and `Branch` is
//! `Branch` (`upstream` holds the full `refs/remotes/...` name Corvene
//! reads from git; the tip's author date is `tip_time`).

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use corvene_core::pull_requests::forked_remotes_to_prune;
use corvene_core::{Branch, BranchKind, GitHubRepository, PullRequest, PullRequestRef, Remote};
use corvene_test_support::{GitHubRepoFixtureOptions, git_hub_repo_fixture, to_iso_string};

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time after the epoch")
        .as_secs() as i64
}

/// `new Date()` as the ISO-8601 text Corvene keeps for a pull request
/// (whole seconds, as the API writes it).
fn iso8601_now() -> String {
    let iso = to_iso_string(UNIX_EPOCH + Duration::from_secs(now_secs() as u64));
    format!("{}Z", &iso[..19])
}

fn create_sample_pull_request(
    git_hub_repository: GitHubRepository,
    user_name: &str,
    base_branch_name: &str,
) -> PullRequest {
    let created_at = iso8601_now();
    PullRequest {
        number: 1,
        title: "desktop".to_string(),
        created_at: created_at.clone(),
        updated_at: created_at,
        head: PullRequestRef {
            ref_name: "main".to_string(),
            sha: "deadbeef".to_string(),
            repository: Some(git_hub_repository.clone()),
        },
        base: PullRequestRef {
            ref_name: base_branch_name.to_string(),
            sha: "deadbeef".to_string(),
            repository: Some(git_hub_repository),
        },
        author: user_name.to_string(),
        draft: false,
        body: "sample body".to_string(),
    }
}

fn create_sample_branch(name: &str, upstream: Option<&str>) -> Branch {
    Branch {
        name: name.to_string(),
        kind: BranchKind::Local,
        full_name: String::new(),
        tip: Some("300acef".to_string()),
        upstream: upstream.map(|u| format!("refs/remotes/{u}")),
        tip_time: Some(now_secs()),
        tip_author: None,
        remote_name: None,
    }
}

const TEST_USER_NAME: &str = "sergiou87";

const ORIGIN_REMOTE: &str = "origin";
const NON_GIT_HUB_DESKTOP_REMOTE: &str = "non-github-desktop-remote";
const GIT_HUB_DESKTOP_REMOTE_WITH_LOCAL_BRANCH: &str = "github-desktop-niik";
/// `` `github-desktop-${TestUserName}` ``
const GIT_HUB_DESKTOP_REMOTE_WITH_PULL_REQUEST: &str = "github-desktop-sergiou87";

fn remotes() -> Vec<Remote> {
    vec![
        Remote {
            name: ORIGIN_REMOTE.to_string(),
            url: "https://github.com/desktop/desktop.git".to_string(),
        },
        Remote {
            name: NON_GIT_HUB_DESKTOP_REMOTE.to_string(),
            url: "https://github.com/fakeuser/desktop.git".to_string(),
        },
        Remote {
            name: GIT_HUB_DESKTOP_REMOTE_WITH_LOCAL_BRANCH.to_string(),
            url: "https://github.com/niik/desktop.git".to_string(),
        },
        Remote {
            name: GIT_HUB_DESKTOP_REMOTE_WITH_PULL_REQUEST.to_string(),
            url: format!("https://github.com/{TEST_USER_NAME}/desktop.git"),
        },
    ]
}

// GHD: unit/find-forked-remotes-to-prune-test.ts › findForkedRemotesToPrune › never prunes remotes not created by the app
#[test]
fn never_prunes_remotes_not_created_by_the_app() {
    let names = forked_remotes_to_prune(&remotes(), &[], &[]);

    assert_ne!(names.len(), 0, "Expected names to be empty");
    assert!(!names.iter().any(|n| n == ORIGIN_REMOTE));
    assert!(!names.iter().any(|n| n == NON_GIT_HUB_DESKTOP_REMOTE));
}

// GHD: unit/find-forked-remotes-to-prune-test.ts › findForkedRemotesToPrune › never prunes remotes with local branches
#[test]
fn never_prunes_remotes_with_local_branches() {
    let all_branches = [create_sample_branch(
        "app-store-refactor",
        Some(&format!(
            "{GIT_HUB_DESKTOP_REMOTE_WITH_LOCAL_BRANCH}/app-store-refactor"
        )),
    )];

    let remotes_to_prune = forked_remotes_to_prune(&remotes(), &[], &all_branches);

    assert!(
        !remotes_to_prune
            .iter()
            .any(|n| n == GIT_HUB_DESKTOP_REMOTE_WITH_LOCAL_BRANCH)
    );
}

// GHD: unit/find-forked-remotes-to-prune-test.ts › findForkedRemotesToPrune › never prunes remotes with pull requests
#[test]
fn never_prunes_remotes_with_pull_requests() {
    let fork_repository = git_hub_repo_fixture(GitHubRepoFixtureOptions {
        owner: TEST_USER_NAME,
        name: "desktop",
        ..Default::default()
    });
    let open_prs = [create_sample_pull_request(
        fork_repository,
        TEST_USER_NAME,
        "my-cool-feature",
    )];

    let remotes_to_prune = forked_remotes_to_prune(&remotes(), &open_prs, &[]);

    assert!(
        !remotes_to_prune
            .iter()
            .any(|n| n == GIT_HUB_DESKTOP_REMOTE_WITH_PULL_REQUEST)
    );
}

// GHD: unit/find-forked-remotes-to-prune-test.ts › findForkedRemotesToPrune › prunes remotes without pull requests or local branches
#[test]
fn prunes_remotes_without_pull_requests_or_local_branches() {
    let remote_names = forked_remotes_to_prune(&remotes(), &[], &[]);

    assert!(
        remote_names
            .iter()
            .any(|n| n == GIT_HUB_DESKTOP_REMOTE_WITH_PULL_REQUEST)
    );
    assert!(
        remote_names
            .iter()
            .any(|n| n == GIT_HUB_DESKTOP_REMOTE_WITH_LOCAL_BRANCH)
    );
}
