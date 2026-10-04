//! Port of GitHub Desktop's `app/test/unit/commit-identity-test.ts`.
//!
//! GitHub Desktop's `CommitIdentity.parseIdentity(identity)`
//! (`models/commit-identity.ts`) reads a git ident string (`NAME <EMAIL>
//! SECONDS TZ`); `getCommits` (`lib/git/log.ts`) uses it for every commit's
//! author and committer. Corvene has no ident-string parser of its own:
//! `corvene_git::get_commits` decodes the `author` header of the commit
//! object with gitoxide and turns it into a `corvene_models::CommitIdentity`
//! (`corvene_git::log`, `identity`). [`parse_identity`] therefore writes a
//! commit whose `author` header is the ident string and reads it back with
//! `get_commits`, which runs exactly that code.
//!
//! GitHub Desktop's `tzOffset` is in minutes, Corvene's `offset` in seconds
//! east of UTC (`90` minutes is `90 * 60`); GitHub Desktop's `date` is
//! `CommitIdentity::date()`.

use std::time::{Duration, UNIX_EPOCH};

use corvene_models::CommitIdentity;
use corvene_test_support::{ExecOptions, exec_with, setup_empty_repository};

/// GitHub Desktop's `CommitIdentity.parseIdentity(identity)`, through
/// Corvene's commit reader (see the module doc).
fn parse_identity(identity: &str) -> CommitIdentity {
    let repo = setup_empty_repository();
    let hash_object = |kind: &str, contents: String| {
        let result = exec_with(
            ["hash-object", "-t", kind, "-w", "--literally", "--stdin"],
            repo.path(),
            ExecOptions {
                stdin: Some(contents.into_bytes()),
                ..ExecOptions::default()
            },
        );
        assert_eq!(result.exit_code, 0, "hash-object: {}", result.stderr);
        result.stdout.trim().to_string()
    };
    let tree = hash_object("tree", String::new());
    let sha = hash_object(
        "commit",
        format!("tree {tree}\nauthor {identity}\ncommitter {identity}\n\nparseIdentity\n"),
    );
    let commits = corvene_git::get_commits(repo.path(), &sha, 0, 1).expect("getCommits");
    assert_eq!(commits.len(), 1, "the commit written for {identity:?}");
    commits.into_iter().next().unwrap().author
}

// GHD: unit/commit-identity-test.ts › CommitIdentity › #parseIdent › understands a normal ident string
#[test]
fn understands_a_normal_ident_string() {
    let identity = parse_identity("Markus Olsson <markus@github.com> 1475670580 +0200");
    assert_eq!(identity.name, "Markus Olsson");
    assert_eq!(identity.email, "markus@github.com");
    // `new Date('2016-10-05T12:29:40.000Z')`
    let expected = gix::date::parse("2016-10-05T12:29:40+00:00", None).unwrap();
    assert_eq!(
        identity.date(),
        UNIX_EPOCH + Duration::from_secs(expected.seconds as u64)
    );
}

// GHD: unit/commit-identity-test.ts › CommitIdentity › #parseIdent › parses timezone information
#[test]
fn parses_timezone_information() {
    let identity1 = parse_identity("Markus Olsson <markus@github.com> 1475670580 +0130");
    assert_eq!(identity1.offset, 90 * 60);

    let identity2 = parse_identity("Markus Olsson <markus@github.com> 1475670580 -0245");
    assert_eq!(identity2.offset, -165 * 60);
}

// GHD: unit/commit-identity-test.ts › CommitIdentity › #parseIdent › parses even if the email address isn't a normal email
#[test]
fn parses_even_if_the_email_address_isnt_a_normal_email() {
    let identity = parse_identity("Markus Olsson <Markus Olsson> 1475670580 +0200");
    assert_eq!(identity.name, "Markus Olsson");
    assert_eq!(identity.email, "Markus Olsson");
}

// GHD: unit/commit-identity-test.ts › CommitIdentity › #parseIdent › parses even if the email address is broken
#[test]
fn parses_even_if_the_email_address_is_broken() {
    // https://github.com/git/git/blob/3ef7618e616e023cf04180e30d77c9fa5310f964/ident.c#L292-L296
    let identity = parse_identity("Markus Olsson <Markus >Olsson> 1475670580 +0200");
    assert_eq!(identity.name, "Markus Olsson");
    assert_eq!(identity.email, "Markus >Olsson");
}
