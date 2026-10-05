//! Port of GitHub Desktop's `app/test/unit/squashed-commit-description-test.ts`.
//!
//! GitHub Desktop's `getSquashedCommitDescription(commits, squashOnto)`
//! (`lib/squash/squashed-commit-description.ts`) prefills the squash
//! dialog's description: the target commit's body, then each squashed
//! commit's summary and body, all trimmed and without their
//! `Co-Authored-By` trailers (`Commit.bodyNoCoAuthors`), joined by blank
//! lines. It is `corvene_core::mco::get_squashed_commit_description`, which
//! `Dispatcher::request_squash` calls; GitHub Desktop's `Commit` is
//! `corvene_models::Commit` with its `trailers`.

use std::time::{SystemTime, UNIX_EPOCH};

use corvene_core::mco::get_squashed_commit_description;
use corvene_models::{Commit, CommitIdentity, Trailer};

/// The test file's `buildTestCommit(summary, body, trailers)`: `new
/// Commit('test', 'test', summary, body, author, author, [], trailers, [])`
/// with the author `new CommitIdentity('test', 'test', new Date())`.
fn build_test_commit(summary: &str, body: &str, trailers: &[Trailer]) -> Commit {
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
    Commit {
        sha: "test".to_string(),
        summary: summary.to_string(),
        body: body.to_string(),
        author: author.clone(),
        committer: author,
        parents: Vec::new(),
        trailers: trailers.to_vec(),
        tags: Vec::new(),
        signature: None,
    }
}

/// The describe's `mockCoAuthorTrailers`.
fn mock_co_author_trailers() -> Vec<Trailer> {
    vec![("Co-Authored-By".to_string(), "test <test>".to_string())]
}

// GHD: unit/squashed-commit-description-test.ts › getSquashedCommitDescription › builds squashed commit descriptions - no coauthors provided
#[test]
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
