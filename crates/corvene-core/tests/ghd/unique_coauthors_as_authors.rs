//! Port of GitHub Desktop's `app/test/unit/unique-coauthors-as-authors-test.ts`.
//!
//! GitHub Desktop's `getUniqueCoauthorsAsAuthors(commits)`
//! (`lib/unique-coauthors-as-authors.ts`) is
//! `corvene_core::get_unique_coauthors_as_authors`: the `Co-Authored-By`
//! trailers of the commits (`Commit::co_authors`, parsed from
//! `Commit::trailers`) as known authors, one per distinct name and email. A
//! `KnownAuthor` is `Author::Known`.

use std::time::{SystemTime, UNIX_EPOCH};

use corvene_core::get_unique_coauthors_as_authors;
use corvene_models::{Author, Commit, CommitIdentity, Trailer};

/// `KnownAuthor.name`.
fn author_name(author: &Author) -> &str {
    match author {
        Author::Known { name, .. } => name,
        other => panic!("not a KnownAuthor: {other:?}"),
    }
}

/// `KnownAuthor.email`.
fn author_email(author: &Author) -> &str {
    match author {
        Author::Known { email, .. } => email,
        other => panic!("not a KnownAuthor: {other:?}"),
    }
}

/// The test file's `buildTestCommit(trailers)`: `new Commit('test', 'test',
/// 'test', 'test', author, author, [], trailers, [])` with the author `new
/// CommitIdentity('test', 'test', new Date())`.
fn build_test_commit(trailers: &[Trailer]) -> Commit {
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
        summary: "test".to_string(),
        body: "test".to_string(),
        author: author.clone(),
        committer: author,
        parents: Vec::new(),
        trailers: trailers.to_vec(),
        tags: Vec::new(),
        signature: None,
    }
}

/// The test file's `buildTestCoAuthorTrailer(email, name)`.
fn build_test_co_author_trailer(email: &str, name: &str) -> Trailer {
    ("Co-Authored-By".to_string(), format!("{name} <{email}>"))
}

// GHD: unit/unique-coauthors-as-authors-test.ts › getUniqueCoauthorsAsAuthors › can returns empty array for no coauthors
#[test]
fn can_returns_empty_array_for_no_coauthors() {
    let trailers = [(
        "Signed-Off-By".to_string(),
        "test <test@github.com>".to_string(),
    )];

    let commits = [
        build_test_commit(&trailers),
        build_test_commit(&[]),
        build_test_commit(&trailers),
    ];

    let co_authors = get_unique_coauthors_as_authors(&commits);
    assert_eq!(co_authors.len(), 0);
}

// GHD: unit/unique-coauthors-as-authors-test.ts › getUniqueCoauthorsAsAuthors › gets coauthor from commit with coauthor
#[test]
fn gets_coauthor_from_commit_with_coauthor() {
    let email = "tidy-dev@github.com";
    let name = "tidy-dev";
    let trailers = [build_test_co_author_trailer(email, name)];
    let commits = [build_test_commit(&trailers)];

    let co_authors = get_unique_coauthors_as_authors(&commits);
    assert_eq!(co_authors.len(), 1);
    let co_author = &co_authors[0];
    assert_eq!(author_email(co_author), email);
    assert_eq!(author_name(co_author), name);
}

// GHD: unit/unique-coauthors-as-authors-test.ts › getUniqueCoauthorsAsAuthors › does not return duplicate authors
#[test]
fn does_not_return_duplicate_authors() {
    let email = "tidy-dev@github.com";
    let name = "tidy-dev";
    let trailers = [build_test_co_author_trailer(email, name)];

    let commits = [
        build_test_commit(&trailers),
        build_test_commit(&trailers),
        build_test_commit(&trailers),
    ];

    let co_authors = get_unique_coauthors_as_authors(&commits);
    assert_eq!(co_authors.len(), 1);
    let co_author = &co_authors[0];
    assert_eq!(author_email(co_author), email);
    assert_eq!(author_name(co_author), name);
}

// GHD: unit/unique-coauthors-as-authors-test.ts › getUniqueCoauthorsAsAuthors › does not return duplicate authors when name is different and email is the same
#[test]
fn does_not_return_duplicate_authors_when_name_is_different_and_email_is_the_same() {
    let email = "tidy-dev@github.com";
    let name = "tidy-dev";
    let trailers = [build_test_co_author_trailer(email, name)];
    let trailers_diff_name = [build_test_co_author_trailer(email, &format!("{name}hello"))];

    let commits = [
        build_test_commit(&trailers),
        build_test_commit(&trailers),
        build_test_commit(&trailers_diff_name),
    ];

    let co_authors = get_unique_coauthors_as_authors(&commits);
    assert_eq!(co_authors.len(), 2);
}

// GHD: unit/unique-coauthors-as-authors-test.ts › getUniqueCoauthorsAsAuthors › does not return duplicate authors when email is different and name is the same
#[test]
fn does_not_return_duplicate_authors_when_email_is_different_and_name_is_the_same() {
    let email = "tidy-dev@github.com";
    let other_email = "sergiou87@github.com";
    let name = "tidy-dev";
    let trailers = [build_test_co_author_trailer(email, name)];
    let trailers_diff_email = [build_test_co_author_trailer(other_email, name)];

    let commits = [
        build_test_commit(&trailers),
        build_test_commit(&trailers),
        build_test_commit(&trailers_diff_email),
    ];

    let co_authors = get_unique_coauthors_as_authors(&commits);
    assert_eq!(co_authors.len(), 2);
}

// GHD: unit/unique-coauthors-as-authors-test.ts › getUniqueCoauthorsAsAuthors › can get multiple coauthors on multiple commits
#[test]
fn can_get_multiple_coauthors_on_multiple_commits() {
    let first_email = "tidy-dev@github.com";
    let first_name = "tidy-dev";
    let first_trailers = [build_test_co_author_trailer(first_email, first_name)];

    let second_email = "sergiou87@github.com";
    let second_name = "Sergio";
    let second_trailers = [build_test_co_author_trailer(second_email, second_name)];

    let commits = [
        build_test_commit(&first_trailers),
        build_test_commit(&[]),
        build_test_commit(&second_trailers),
    ];

    let co_authors = get_unique_coauthors_as_authors(&commits);
    assert_eq!(co_authors.len(), 2);
    let co_author_emails: Vec<&str> = co_authors.iter().map(author_email).collect();
    assert!(co_author_emails.contains(&first_email));
    assert!(co_author_emails.contains(&second_email));
}
