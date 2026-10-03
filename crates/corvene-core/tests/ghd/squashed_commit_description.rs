//! Port of GitHub Desktop's `app/test/unit/squashed-commit-description-test.ts`.
//!
//! GitHub Desktop's `getSquashedCommitDescription(commits, squashOnto)`
//! (`lib/squash/squashed-commit-description.ts`) prefills the squash
//! dialog's description: the target commit's body, then each squashed
//! commit's summary and body, all trimmed and without their
//! `Co-Authored-By` trailers (`Commit.bodyNoCoAuthors`), joined by blank
//! lines. Corvene builds that text inline in `Dispatcher::request_squash`
//! (`corvene-core/src/mco.rs`), from `rs.commits` of the repository state,
//! so there is nothing to call with the cases' commits, and
//! `corvene_models::Commit` has no trailers. The cases call a stand-in and
//! are ignored until a function exists.
//!
//! [`TestCommit`] is GitHub Desktop's `Commit` as the test builds it: a
//! `corvene_models::Commit` plus the trailers it is given, which Corvene's
//! `Commit` cannot hold yet.

use std::time::{SystemTime, UNIX_EPOCH};

use corvene_models::{Commit, CommitIdentity};

/// GitHub Desktop's `ITrailer` (`{ token, value }`).
type Trailer = (String, String);

/// GitHub Desktop's `Commit`: Corvene's `Commit` and the trailers GitHub
/// Desktop's carries (see the module doc).
struct TestCommit {
    #[allow(dead_code)]
    commit: Commit,
    #[allow(dead_code)]
    trailers: Vec<Trailer>,
}

/// Stand-in for GitHub Desktop's `getSquashedCommitDescription(commits,
/// squashOnto)`. Replace it with the Corvene function once there is one
/// (then `TestCommit` becomes `Commit`) and remove the `#[ignore]`s.
fn get_squashed_commit_description(_commits: &[TestCommit], _squash_onto: &TestCommit) -> String {
    unimplemented!(
        "Corvene builds the squashed description inline in Dispatcher::request_squash; no function takes commits"
    )
}

/// The test file's `buildTestCommit(summary, body, trailers)`: `new
/// Commit('test', 'test', summary, body, author, author, [], trailers, [])`
/// with the author `new CommitIdentity('test', 'test', new Date())`.
fn build_test_commit(summary: &str, body: &str, trailers: &[Trailer]) -> TestCommit {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let author = CommitIdentity {
        name: "test".to_string(),
        email: "test".to_string(),
        seconds: now,
        offset: 0,
    };
    TestCommit {
        commit: Commit {
            sha: "test".to_string(),
            summary: summary.to_string(),
            body: body.to_string(),
            author: author.clone(),
            committer: author,
            parents: Vec::new(),
            tags: Vec::new(),
        },
        trailers: trailers.to_vec(),
    }
}

/// The describe's `mockCoAuthorTrailers`.
fn mock_co_author_trailers() -> Vec<Trailer> {
    vec![("Co-Authored-By".to_string(), "test <test>".to_string())]
}

// GHD: unit/squashed-commit-description-test.ts › getSquashedCommitDescription › builds squashed commit descriptions - no coauthors provided
#[test]
#[ignore = "ghd: missing: getSquashedCommitDescription (lib/squash/squashed-commit-description.ts) is inline in Dispatcher::request_squash (corvene-core/src/mco.rs), not callable; corvene_models::Commit has no trailers"]
fn builds_squashed_commit_descriptions_no_coauthors_provided() {
    let commits = [
        build_test_commit("summary1", "desc1", &[]),
        build_test_commit("summary2", "desc2", &[]),
    ];

    let squash_onto = build_test_commit("ontoSummary", "ontoDesc", &[]);

    let desc = get_squashed_commit_description(&commits, &squash_onto);
    assert_eq!(desc, "ontoDesc\n\nsummary1\n\ndesc1\n\nsummary2\n\ndesc2");
}

// GHD: unit/squashed-commit-description-test.ts › getSquashedCommitDescription › builds squashed commit descriptions that do not include coauthors
#[test]
#[ignore = "ghd: missing: getSquashedCommitDescription (lib/squash/squashed-commit-description.ts) is inline in Dispatcher::request_squash (corvene-core/src/mco.rs), not callable; corvene_models::Commit has no trailers"]
fn builds_squashed_commit_descriptions_that_do_not_include_coauthors() {
    let mock_co_author_trailers = mock_co_author_trailers();
    let commits = [
        build_test_commit("summary1", "desc1", &mock_co_author_trailers),
        build_test_commit("summary2", "desc2", &mock_co_author_trailers),
    ];

    let squash_onto = build_test_commit("ontoSummary", "ontoDesc", &mock_co_author_trailers);

    let desc = get_squashed_commit_description(&commits, &squash_onto);
    assert_eq!(desc, "ontoDesc\n\nsummary1\n\ndesc1\n\nsummary2\n\ndesc2");
}

// GHD: unit/squashed-commit-description-test.ts › getSquashedCommitDescription › builds squashed commit descriptions with whitespace trimmed
#[test]
#[ignore = "ghd: missing: getSquashedCommitDescription (lib/squash/squashed-commit-description.ts) is inline in Dispatcher::request_squash (corvene-core/src/mco.rs), not callable; corvene_models::Commit has no trailers"]
fn builds_squashed_commit_descriptions_with_whitespace_trimmed() {
    let mock_co_author_trailers = mock_co_author_trailers();
    let commits = [
        build_test_commit("summary1    ", "desc1   ", &mock_co_author_trailers),
        build_test_commit("summary2\n", "desc2\n", &mock_co_author_trailers),
    ];

    let squash_onto = build_test_commit("ontoSummary", "ontoDesc  \n", &mock_co_author_trailers);

    let desc = get_squashed_commit_description(&commits, &squash_onto);
    assert_eq!(desc, "ontoDesc\n\nsummary1\n\ndesc1\n\nsummary2\n\ndesc2");
}
