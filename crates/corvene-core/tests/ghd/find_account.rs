//! Port of GitHub Desktop's `app/test/unit/find-account-test.ts`.
//!
//! GitHub Desktop's `findAccountForRemoteURL(urlOrRepositoryAlias, accounts,
//! canAccessRepository)` (`lib/find-account.ts`) is
//! `corvene_core::clone_info::find_account_for_remote_url(input, candidates,
//! can_access)`, which picks the index of a `Candidate` the way
//! `Dispatcher::resolve_clone_info` builds them (through
//! `clone_info::resolve_with`): [`find_account_for_remote_url`] makes every
//! account a `Candidate` (host and GitHub.com-ness from
//! `Endpoint::from_api_base`, `authenticated`), followed by the
//! unauthenticated GitHub.com candidate the dispatcher appends for GitHub
//! Desktop's `Account.anonymous()`, and answers `can_access` with the test's
//! `canAccessRepository`.
//!
//! `getDotComAPIEndpoint()` is `Endpoint::github_com().api_base` and
//! `getEnterpriseAPIURL(url)` is `Endpoint::enterprise(url, false).api_base`
//! (`314-enterprise-plain-http` is off in every preset).

use corvene_core::Account;
use corvene_core::clone_info::Candidate;
use corvene_github::Endpoint;

use crate::accounts_support::{get_dot_com_api_endpoint, get_enterprise_api_url, new_account};

/// `Account.anonymous()`: `new Account('', getDotComAPIEndpoint(), '', [],
/// '', -1, '', 'free')`. Corvene's ids are unsigned, so the id is 0.
fn anonymous() -> Account {
    new_account("", &get_dot_com_api_endpoint(), "", &[], "", 0, "", "free")
}

/// `findAccountForRemoteURL(urlOrRepositoryAlias, accounts,
/// canAccessRepository)` (see the module docs).
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
                login: (ix < accounts.len()).then(|| account.login.clone()),
            }
        })
        .collect();
    let ix = corvene_core::clone_info::find_account_for_remote_url(
        url_or_repository_alias,
        &candidates,
        &mut |ix, owner, name| can_access_repository(&all_accounts[ix], owner, name),
    )?;
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
