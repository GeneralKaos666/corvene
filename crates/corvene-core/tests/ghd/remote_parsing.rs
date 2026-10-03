//! Port of GitHub Desktop's `app/test/unit/remote-parsing-test.ts`.
//!
//! GitHub Desktop's `parseRemote(url)` (`lib/remote-parsing.ts`) returns the
//! protocol, host name, owner and name of a remote URL, or `null`. In
//! Corvene that job is split: `corvene_models::split_remote` (documented as
//! GHD `parseRemote`) only cuts a URL into host and path, and
//! `corvene_core::clone_info::parse_repository_identifier` (GHD
//! `parseRepositoryIdentifier`, which calls `parseRemote`) takes the owner
//! and name from that path. GitHub Desktop's `parseRepositoryIdentifier`
//! returns `parseRemote`'s host name, owner and name for a URL it parses
//! and falls back to the `owner/name` shorthand (host name `null`)
//! otherwise; Corvene's does the same. [`parse_remote`] is therefore
//! `parse_repository_identifier` without its shorthand fallback: the
//! identifiers that came from a remote URL (a host name) and nothing else.
//! No case checks `protocol`.

use corvene_core::clone_info::parse_repository_identifier;

/// GitHub Desktop's `IGitRemoteURL` (the fields these cases check).
#[derive(Debug)]
struct GitRemoteUrl {
    hostname: String,
    owner: String,
    name: String,
}

/// GitHub Desktop's `parseRemote(url)`, see the module doc.
fn parse_remote(url: &str) -> Option<GitRemoteUrl> {
    let identifier = parse_repository_identifier(url)?;
    let hostname = identifier.hostname?;
    Some(GitRemoteUrl {
        hostname,
        owner: identifier.owner,
        name: identifier.name,
    })
}

// GHD: unit/remote-parsing-test.ts › URL remote parsing › parses HTTPS URLs with a trailing git suffix
#[test]
fn parses_https_urls_with_a_trailing_git_suffix() {
    let remote = parse_remote("https://github.com/hubot/repo.git");
    assert!(remote.is_some());
    let remote = remote.unwrap();
    assert_eq!(remote.hostname, "github.com");
    assert_eq!(remote.owner, "hubot");
    assert_eq!(remote.name, "repo");
}

// GHD: unit/remote-parsing-test.ts › URL remote parsing › parses HTTPS URLs with a trailing -git suffix
#[test]
fn parses_https_urls_with_a_trailing_dash_git_suffix() {
    let remote = parse_remote("https://github.com/hubot/repo-git");
    assert!(remote.is_some());
    let remote = remote.unwrap();
    assert_eq!(remote.hostname, "github.com");
    assert_eq!(remote.owner, "hubot");
    assert_eq!(remote.name, "repo-git");
}

// GHD: unit/remote-parsing-test.ts › URL remote parsing › parses HTTPS URLs with a trailing -git and .git suffixes
#[test]
fn parses_https_urls_with_a_trailing_dash_git_and_dot_git_suffixes() {
    let remote = parse_remote("https://github.com/hubot/repo-git.git");
    assert!(remote.is_some());
    let remote = remote.unwrap();
    assert_eq!(remote.hostname, "github.com");
    assert_eq!(remote.owner, "hubot");
    assert_eq!(remote.name, "repo-git");
}

// GHD: unit/remote-parsing-test.ts › URL remote parsing › parses HTTPS URLs without a trailing git suffix
#[test]
fn parses_https_urls_without_a_trailing_git_suffix() {
    let remote = parse_remote("https://github.com/hubot/repo");
    assert!(remote.is_some());
    let remote = remote.unwrap();
    assert_eq!(remote.hostname, "github.com");
    assert_eq!(remote.owner, "hubot");
    assert_eq!(remote.name, "repo");
}

// GHD: unit/remote-parsing-test.ts › URL remote parsing › parses HTTPS URLs with a trailing slash
#[test]
fn parses_https_urls_with_a_trailing_slash() {
    let remote = parse_remote("https://github.com/hubot/repo/");
    assert!(remote.is_some());
    let remote = remote.unwrap();
    assert_eq!(remote.hostname, "github.com");
    assert_eq!(remote.owner, "hubot");
    assert_eq!(remote.name, "repo");
}

// GHD: unit/remote-parsing-test.ts › URL remote parsing › parses HTTPS URLs which include a username
#[test]
fn parses_https_urls_which_include_a_username() {
    let remote = parse_remote("https://monalisa@github.com/hubot/repo.git");
    assert!(remote.is_some());
    let remote = remote.unwrap();
    assert_eq!(remote.hostname, "github.com");
    assert_eq!(remote.owner, "hubot");
    assert_eq!(remote.name, "repo");
}

// GHD: unit/remote-parsing-test.ts › URL remote parsing › parses SSH URLs
#[test]
fn parses_ssh_urls() {
    let remote = parse_remote("git@github.com:hubot/repo.git");
    assert!(remote.is_some());
    let remote = remote.unwrap();
    assert_eq!(remote.hostname, "github.com");
    assert_eq!(remote.owner, "hubot");
    assert_eq!(remote.name, "repo");
}

// GHD: unit/remote-parsing-test.ts › URL remote parsing › parses SSH URLs with custom username
#[test]
fn parses_ssh_urls_with_custom_username() {
    let remote = parse_remote("niik@niik.ghe.com:hubot/repo.git");
    assert!(remote.is_some());
    let remote = remote.unwrap();
    assert_eq!(remote.hostname, "niik.ghe.com");
    assert_eq!(remote.owner, "hubot");
    assert_eq!(remote.name, "repo");
}

// GHD: unit/remote-parsing-test.ts › URL remote parsing › parses SSH URLs without the git suffix
#[test]
fn parses_ssh_urls_without_the_git_suffix() {
    let remote = parse_remote("git@github.com:hubot/repo");
    assert!(remote.is_some());
    let remote = remote.unwrap();
    assert_eq!(remote.hostname, "github.com");
    assert_eq!(remote.owner, "hubot");
    assert_eq!(remote.name, "repo");
}

// GHD: unit/remote-parsing-test.ts › URL remote parsing › parses SSH URLs without the git suffix but with -git suffix
#[test]
fn parses_ssh_urls_without_the_git_suffix_but_with_dash_git_suffix() {
    let remote = parse_remote("git@github.com:hubot/repo-git");
    assert!(remote.is_some());
    let remote = remote.unwrap();
    assert_eq!(remote.hostname, "github.com");
    assert_eq!(remote.owner, "hubot");
    assert_eq!(remote.name, "repo-git");
}

// GHD: unit/remote-parsing-test.ts › URL remote parsing › parses SSH URLs with the .git suffix and -git suffix
#[test]
fn parses_ssh_urls_with_the_dot_git_suffix_and_dash_git_suffix() {
    let remote = parse_remote("git@github.com:hubot/repo-git.git");
    assert!(remote.is_some());
    let remote = remote.unwrap();
    assert_eq!(remote.hostname, "github.com");
    assert_eq!(remote.owner, "hubot");
    assert_eq!(remote.name, "repo-git");
}

// GHD: unit/remote-parsing-test.ts › URL remote parsing › parses SSH URLs with a trailing slash
#[test]
fn parses_ssh_urls_with_a_trailing_slash() {
    let remote = parse_remote("git@github.com:hubot/repo/");
    assert!(remote.is_some());
    let remote = remote.unwrap();
    assert_eq!(remote.hostname, "github.com");
    assert_eq!(remote.owner, "hubot");
    assert_eq!(remote.name, "repo");
}

// GHD: unit/remote-parsing-test.ts › URL remote parsing › parses git URLs
#[test]
fn parses_git_urls() {
    let remote = parse_remote("git:github.com/hubot/repo.git");
    assert!(remote.is_some());
    let remote = remote.unwrap();
    assert_eq!(remote.hostname, "github.com");
    assert_eq!(remote.owner, "hubot");
    assert_eq!(remote.name, "repo");
}

// GHD: unit/remote-parsing-test.ts › URL remote parsing › parses git URLs without the git suffix
#[test]
fn parses_git_urls_without_the_git_suffix() {
    let remote = parse_remote("git:github.com/hubot/repo");
    assert!(remote.is_some());
    let remote = remote.unwrap();
    assert_eq!(remote.hostname, "github.com");
    assert_eq!(remote.owner, "hubot");
    assert_eq!(remote.name, "repo");
}

// GHD: unit/remote-parsing-test.ts › URL remote parsing › parses git URLs with a trailing slash
#[test]
fn parses_git_urls_with_a_trailing_slash() {
    let remote = parse_remote("git:github.com/hubot/repo/");
    assert!(remote.is_some());
    let remote = remote.unwrap();
    assert_eq!(remote.hostname, "github.com");
    assert_eq!(remote.owner, "hubot");
    assert_eq!(remote.name, "repo");
}

// GHD: unit/remote-parsing-test.ts › URL remote parsing › parses SSH URLs with the ssh prefix
#[test]
fn parses_ssh_urls_with_the_ssh_prefix() {
    let remote = parse_remote("ssh://git@github.com/hubot/repo");
    assert!(remote.is_some());
    let remote = remote.unwrap();
    assert_eq!(remote.hostname, "github.com");
    assert_eq!(remote.owner, "hubot");
    assert_eq!(remote.name, "repo");
}

// GHD: unit/remote-parsing-test.ts › URL remote parsing › parses SSH URLs with the ssh prefix and trailing slash
#[test]
fn parses_ssh_urls_with_the_ssh_prefix_and_trailing_slash() {
    let remote = parse_remote("ssh://git@github.com/hubot/repo/");
    assert!(remote.is_some());
    let remote = remote.unwrap();
    assert_eq!(remote.hostname, "github.com");
    assert_eq!(remote.owner, "hubot");
    assert_eq!(remote.name, "repo");
}

// GHD: unit/remote-parsing-test.ts › URL remote parsing › does not parse invalid HTTP URLs when missing repo name
#[test]
fn does_not_parse_invalid_http_urls_when_missing_repo_name() {
    let remote = parse_remote("https://github.com/someuser//");
    assert!(remote.is_none());
}

// GHD: unit/remote-parsing-test.ts › URL remote parsing › does not parse invalid SSH URLs when missing repo name
#[test]
fn does_not_parse_invalid_ssh_urls_when_missing_repo_name() {
    let remote = parse_remote("git@github.com:hubot/");
    assert!(remote.is_none());
}

// GHD: unit/remote-parsing-test.ts › URL remote parsing › does not parse invalid git URLs when missing repo name
#[test]
fn does_not_parse_invalid_git_urls_when_missing_repo_name() {
    let remote = parse_remote("git:github.com/hubot/");
    assert!(remote.is_none());
}

// GHD: unit/remote-parsing-test.ts › URL remote parsing › does not parse invalid HTTP URLs when missing repo owner
#[test]
fn does_not_parse_invalid_http_urls_when_missing_repo_owner() {
    let remote = parse_remote("https://github.com//somerepo");
    assert!(remote.is_none());
}

// GHD: unit/remote-parsing-test.ts › URL remote parsing › does not parse invalid SSH URLs when missing repo owner
#[test]
fn does_not_parse_invalid_ssh_urls_when_missing_repo_owner() {
    let remote = parse_remote("git@github.com:/somerepo");
    assert!(remote.is_none());
}

// GHD: unit/remote-parsing-test.ts › URL remote parsing › does not parse invalid git URLs when missing repo owner
#[test]
fn does_not_parse_invalid_git_urls_when_missing_repo_owner() {
    let remote = parse_remote("git:github.com/hubot/");
    assert!(remote.is_none());
}
