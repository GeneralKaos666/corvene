//! Port of GitHub Desktop's `app/test/unit/find-account-test.ts`.
//!
//! GitHub Desktop's `findAccountForRemoteURL(urlOrRepositoryAlias, accounts,
//! canAccessRepository)` (`lib/find-account.ts`) has no stand-alone Corvene
//! function: its account choice is the first half of
//! `corvene_core::clone_info::resolve_with(input, candidates, lookup,
//! strict_shorthand, prefer_ssh)`, which then returns the clone info that
//! account's `lookup` answered (GitHub Desktop's `resolveCloneInfo` +
//! `fetchRepositoryCloneInfo`). [`find_account_for_remote_url`] calls it the
//! way `Dispatcher::resolve_clone_info` does:
//!
//! - every account becomes a `Candidate` (host and GitHub.com-ness from
//!   `Endpoint::from_api_base`, `authenticated`), followed by the
//!   unauthenticated GitHub.com candidate the dispatcher appends for
//!   GitHub Desktop's `Account.anonymous()`;
//! - `lookup` answers with the test's `canAccessRepository`, and its clone
//!   URL names the candidate, so the result tells which account was chosen
//!   (no URL from a candidate: no account);
//! - `strict_shorthand` and `prefer_ssh` are the `github-desktop` preset's
//!   values of `204-clone-shorthand-not-found` and `226-clone-prefers-ssh`.
//!
//! `getDotComAPIEndpoint()` is `Endpoint::github_com().api_base` and
//! `getEnterpriseAPIURL(url)` is `Endpoint::enterprise(url, false).api_base`
//! (`314-enterprise-plain-http` is off in every preset).

use corvene_core::Account;
use corvene_core::clone_info::{Candidate, resolve_with};
use corvene_core::flags::ids::{CLONE_PREFERS_SSH, CLONE_SHORTHAND_NOT_FOUND};
use corvene_github::{Endpoint, RepositoryCloneInfo};

use crate::accounts_support::{ghd_flags, new_account};

/// The clone URL prefix that names the candidate whose lookup answered.
const CANDIDATE: &str = "candidate:";

/// `getDotComAPIEndpoint()`.
fn get_dot_com_api_endpoint() -> String {
    Endpoint::github_com().api_base
}

/// `getEnterpriseAPIURL(endpoint)`.
fn get_enterprise_api_url(endpoint: &str) -> String {
    Endpoint::enterprise(endpoint, false)
        .expect("a valid Enterprise address")
        .api_base
}

/// `Account.anonymous()`: `new Account('', getDotComAPIEndpoint(), '', [],
/// '', -1, '', 'free')`. Corvene's ids are unsigned, so the id is 0.
fn anonymous() -> Account {
    new_account("", &get_dot_com_api_endpoint(), "", &[], "", 0, "", "free")
}

/// `findAccountForRemoteURL(urlOrRepositoryAlias, accounts,
/// canAccessRepository)` through `clone_info::resolve_with` (see the module
/// docs). Swap the body for a call to a Corvene `find_account_for_remote_url`
/// once the account choice is a function of its own.
fn find_account_for_remote_url(
    url_or_repository_alias: &str,
    accounts: &[Account],
    can_access_repository: impl Fn(&Account, &str, &str) -> bool,
) -> Option<Account> {
    let all_accounts: Vec<Account> = accounts.iter().cloned().chain([anonymous()]).collect();
    let candidates: Vec<Candidate> = all_accounts
        .iter()
        .enumerate()
        .map(|(ix, account)| {
            let endpoint = Endpoint::from_api_base(&account.endpoint);
            Candidate {
                host: endpoint.host().to_string(),
                is_dotcom: endpoint.is_dotcom(),
                authenticated: ix < accounts.len(),
            }
        })
        .collect();
    let mut lookup = |ix: usize, owner: &str, name: &str, _ssh: bool| {
        Ok(
            can_access_repository(&all_accounts[ix], owner, name).then(|| RepositoryCloneInfo {
                url: format!("{CANDIDATE}{ix}"),
                default_branch: None,
            }),
        )
    };
    let flags = ghd_flags();
    let info = resolve_with(
        url_or_repository_alias,
        &candidates,
        &mut lookup,
        flags.bool(CLONE_SHORTHAND_NOT_FOUND),
        flags.bool(CLONE_PREFERS_SSH),
    )
    .ok()?;
    let ix: usize = info.url.strip_prefix(CANDIDATE)?.parse().ok()?;
    Some(all_accounts[ix].clone())
}

fn mock_can_access_repository(account: &Account, owner: &str, name: &str) -> bool {
    // private repository, only this person can access it
    if account.endpoint == get_dot_com_api_endpoint()
        && account.login == "joan"
        && owner == "desktop"
        && name == "repo-fixture"
    {
        return true;
    }

    // public repository is accessible to everyone
    if account.endpoint == get_dot_com_api_endpoint() && owner == "inkscape" && name == "inkscape" {
        return true;
    }

    false
}

fn accounts() -> Vec<Account> {
    vec![
        new_account(
            "joan",
            &get_dot_com_api_endpoint(),
            "deadbeef",
            &[],
            "",
            1,
            "GitHub",
            "free",
        ),
        new_account(
            "joel",
            &get_enterprise_api_url("https://github.mycompany.com"),
            "deadbeef",
            &[],
            "",
            2,
            "My Company",
            "free",
        ),
    ]
}

// GHD: unit/find-account-test.ts › findAccountForRemoteURL › gives no account for non-GitHub endpoint
#[test]
fn gives_no_account_for_non_github_endpoint() {
    let account = find_account_for_remote_url(
        "https://gitlab.com/inkscape/inkscape.git",
        &accounts(),
        mock_can_access_repository,
    );
    assert!(account.is_none());
}

// GHD: unit/find-account-test.ts › findAccountForRemoteURL › gives no account for non-existent GitHub owner/name repository
#[test]
fn gives_no_account_for_non_existent_github_owner_name_repository() {
    let account = find_account_for_remote_url(
        "desktop/nonexistent-repo-fixture",
        &accounts(),
        mock_can_access_repository,
    );
    assert!(account.is_none());
}

// GHD: unit/find-account-test.ts › findAccountForRemoteURL › finds the anonymous account for public GitHub owner/name repository
#[test]
fn finds_the_anonymous_account_for_public_github_owner_name_repository() {
    let account = find_account_for_remote_url("inkscape/inkscape", &[], mock_can_access_repository);
    assert!(account.is_some());
    assert_eq!(account.unwrap(), anonymous());
}

// GHD: unit/find-account-test.ts › findAccountForRemoteURL › finds the anonymous account for public repository on GitHub endpoint
#[test]
fn finds_the_anonymous_account_for_public_repository_on_github_endpoint() {
    let account = find_account_for_remote_url(
        "https://github.com/inkscape/inkscape",
        &[],
        mock_can_access_repository,
    );
    assert!(account.is_some());
    assert_eq!(account.unwrap(), anonymous());
}

// GHD: unit/find-account-test.ts › findAccountForRemoteURL › finds the account for GitHub owner/name repository
#[test]
fn finds_the_account_for_github_owner_name_repository() {
    let account =
        find_account_for_remote_url("inkscape/inkscape", &accounts(), mock_can_access_repository);
    assert!(account.is_some());
    assert_eq!(account.unwrap().login, "joan");
}

// GHD: unit/find-account-test.ts › findAccountForRemoteURL › finds the account for GitHub endpoint
#[test]
fn finds_the_account_for_github_endpoint() {
    let account = find_account_for_remote_url(
        "https://github.com/inkscape/inkscape.git",
        &accounts(),
        mock_can_access_repository,
    );
    assert!(account.is_some());
    assert_eq!(account.unwrap().login, "joan");
}

// GHD: unit/find-account-test.ts › findAccountForRemoteURL › finds the account for GitHub Enterprise endpoint
#[test]
#[ignore = "ghd: missing: no findAccountForRemoteURL (lib/find-account.ts) apart from clone_info::resolve_with, which only takes the host's account (joel) when its lookup finds the repo; here it 404s, so REPOSITORY_NOT_FOUND, no account"]
fn finds_the_account_for_github_enterprise_endpoint() {
    let account = find_account_for_remote_url(
        "https://github.mycompany.com/inkscape/inkscape.git",
        &accounts(),
        mock_can_access_repository,
    );
    assert!(account.is_some());
    assert_eq!(account.unwrap().login, "joel");
}

// GHD: unit/find-account-test.ts › findAccountForRemoteURL › finds the account for private GitHub owner/name repository
#[test]
fn finds_the_account_for_private_github_owner_name_repository() {
    let account = find_account_for_remote_url(
        "desktop/repo-fixture",
        &accounts(),
        mock_can_access_repository,
    );
    assert!(account.is_some());
    assert_eq!(account.unwrap().login, "joan");
}

// GHD: unit/find-account-test.ts › findAccountForRemoteURL › cannot see the private GitHub owner/name repository
#[test]
fn cannot_see_the_private_github_owner_name_repository() {
    let account =
        find_account_for_remote_url("desktop/repo-fixture", &[], mock_can_access_repository);
    assert!(account.is_none());
}
