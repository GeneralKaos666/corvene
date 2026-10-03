//! Port of GitHub Desktop's `app/test/unit/git/for-each-ref-test.ts`.
//!
//! Corvene equivalents:
//!
//! - `getBranches(repository)` (`lib/git/for-each-ref.ts`): the branches of
//!   `corvene_git::open_repository` ([`get_branches`]). `BranchType.Local` is
//!   `BranchKind::Local`; `Branch.upstream` is `Branch::upstream` (`None`
//!   when there is none); `Branch.tip.sha` is `Branch::tip`.
//! - `getBranchesDifferingFromUpstream(repository)`: none. Corvene inlines a
//!   variant of it (strictly behind only) in
//!   `corvene_git::fast_forward_branches`; [`get_branches_differing_from_upstream`]
//!   is a stand-in returning GitHub Desktop's `ITrackingBranch`
//!   ([`TrackingBranch`]).

use std::path::Path;

use corvene_models::BranchKind;
use corvene_test_support::{
    setup_empty_directory, setup_empty_repository, setup_fixture_repository,
};

use crate::refs_support::get_branches;

/// GitHub Desktop's `ITrackingBranch` (`models/branch.ts`).
#[allow(dead_code)]
struct TrackingBranch {
    reference: String,
    sha: String,
    upstream_ref: String,
    upstream_sha: String,
}

/// Stand-in for GitHub Desktop's
/// `getBranchesDifferingFromUpstream(repository)` (`lib/git/for-each-ref.ts`):
/// the local branches other than the current one whose tip differs from
/// their upstream's. Replace it with the `corvene_git` function once there
/// is one and remove the `#[ignore]`.
fn get_branches_differing_from_upstream(_repository: &Path) -> Vec<TrackingBranch> {
    unimplemented!("corvene_git has no getBranchesDifferingFromUpstream")
}

// GHD: unit/git/for-each-ref-test.ts › git/for-each-ref › getBranches › fetches branches using for-each-ref
#[test]
fn fetches_branches_using_for_each_ref() {
    let repository = setup_fixture_repository("repo-with-many-refs");

    let branches: Vec<_> = get_branches(repository.path(), &[])
        .into_iter()
        .filter(|b| b.kind == BranchKind::Local)
        .collect();

    assert_eq!(branches.len(), 3);

    let commit_with_body = &branches[0];
    assert_eq!(commit_with_body.name, "commit-with-long-description");
    assert!(commit_with_body.upstream.is_none());
    assert_eq!(
        commit_with_body.tip.as_deref(),
        Some("dfa96676b65e1c0ed43ca25492252a5e384c8efd")
    );

    let commit_no_body = &branches[1];
    assert_eq!(commit_no_body.name, "commit-with-no-body");
    assert!(commit_no_body.upstream.is_none());
    assert_eq!(
        commit_no_body.tip.as_deref(),
        Some("49ec1e05f39eef8d1ab6200331a028fb3dd96828")
    );

    let master = &branches[2];
    assert_eq!(master.name, "master");
    assert!(master.upstream.is_none());
    assert_eq!(
        master.tip.as_deref(),
        Some("b9ccfc3307240b86447bca2bd6c51a4bb4ade493")
    );
}

// GHD: unit/git/for-each-ref-test.ts › git/for-each-ref › getBranches › should return empty list for empty repo
#[test]
fn should_return_empty_list_for_empty_repo() {
    let repo = setup_empty_repository();
    let branches = get_branches(repo.path(), &[]);
    assert_eq!(branches.len(), 0);
}

// GHD: unit/git/for-each-ref-test.ts › git/for-each-ref › getBranches › should return empty list for directory without a .git directory
#[test]
fn should_return_empty_list_for_directory_without_a_git_directory() {
    let repo = setup_empty_directory();
    let status = get_branches(repo.path(), &[]);
    assert_eq!(status.len(), 0);
}

// GHD: unit/git/for-each-ref-test.ts › git/for-each-ref › getBranchesDifferingFromUpstream › filters branches differing from upstream using for-each-ref
#[test]
#[ignore = "ghd: missing: corvene_git has no getBranchesDifferingFromUpstream (lib/git/for-each-ref.ts); fast_forward_branches inlines a behind-only variant"]
fn filters_branches_differing_from_upstream_using_for_each_ref() {
    let repository = setup_fixture_repository("repo-with-non-updated-branches");

    let branches = get_branches_differing_from_upstream(repository.path());

    let branch_refs: Vec<&str> = branches.iter().map(|b| b.reference.as_str()).collect();
    assert_eq!(branch_refs.len(), 3);

    // All branches that are behind and/or ahead must be included
    assert!(branch_refs.contains(&"refs/heads/branch-behind"));
    assert!(branch_refs.contains(&"refs/heads/branch-ahead"));
    assert!(branch_refs.contains(&"refs/heads/branch-ahead-and-behind"));

    // `main` is the current branch, and shouldn't be included
    assert!(!branch_refs.contains(&"refs/heads/main"));

    // Branches that are up to date shouldn't be included
    assert!(!branch_refs.contains(&"refs/heads/branch-up-to-date"));
}
