//! Signed-in accounts (GHD `app/src/lib/stores/accounts-store.ts`
//! `AccountsStore`), without gpui so the bookkeeping can be tested.
//!
//! GHD keeps the accounts in its `users` data store item and their tokens
//! in an `ISecureStore` (the keychain). In Corvene the list is
//! `AppState::accounts`, saved whole under the `accounts` key of
//! `corvene_store::Store` (`StoreExt::save_accounts`), and [`SecureStore`]
//! is the token store: [`Keychain`] in the app, anything else in tests.
//! Corvene's `Account` carries no token (it only ever goes to the secure
//! store), so [`add_account`] takes it beside the account.

use corvene_models::Account;
use corvene_store::Store;
use tracing::error;

use crate::persistence::StoreExt;

/// GHD `ISecureStore`: where account tokens live, one item per account.
/// Corvene's items are keyed by the account's host and login (GHD's by
/// `getKeyForAccount(account)` and login).
pub trait SecureStore {
    /// `setItem(key, login, token)`
    fn set_item(&self, host: &str, login: &str, token: &str) -> Result<(), String>;
    /// `deleteItem(key, login)`
    fn delete_item(&self, host: &str, login: &str) -> Result<(), String>;
}

/// The OS keychain (`corvene_platform::keychain`).
pub struct Keychain;

impl SecureStore for Keychain {
    fn set_item(&self, host: &str, login: &str, token: &str) -> Result<(), String> {
        corvene_platform::keychain::store_token(host, login, token).map_err(|e| e.to_string())
    }

    fn delete_item(&self, host: &str, login: &str) -> Result<(), String> {
        corvene_platform::keychain::delete_token(host, login).map_err(|e| e.to_string())
    }
}

/// Why an account could not be added or removed (GHD `emitError`).
#[derive(Debug, thiserror::Error)]
pub enum AccountsError {
    #[error("could not store the account token: {0}")]
    StoreToken(String),
    #[error("could not remove the account token: {0}")]
    RemoveToken(String),
}

/// GHD `sortAccounts`: GitHub.com accounts first, then Enterprise ones,
/// each in the order they were added.
pub fn sort_accounts(accounts: Vec<Account>) -> Vec<Account> {
    let mut accounts = accounts;
    // a stable sort keeps the insertion order within each group
    accounts.sort_by_key(|account| !account.is_dotcom());
    accounts
}

/// GHD `AccountsStore.addAccount`: store the token, put `account` in place
/// of the account with the same endpoint (or after the others), sort
/// GitHub.com first and save the list. Nothing changes when the token
/// cannot be stored.
pub fn add_account(
    store: &Store,
    accounts: &mut Vec<Account>,
    account: Account,
    token: &str,
    secure_store: &dyn SecureStore,
) -> Result<Account, AccountsError> {
    if let Err(err) = secure_store.set_item(&account.host(), &account.login, token) {
        error!(login = %account.login, %err, "Error adding account");
        return Err(AccountsError::StoreToken(err));
    }

    // `accountsByEndpoint`: one account per endpoint, the last one set
    // winning, in the place of the endpoint's first (a `Map` keeps the
    // first insertion's place)
    let mut by_endpoint: Vec<Account> = Vec::with_capacity(accounts.len() + 1);
    for x in std::mem::take(accounts)
        .into_iter()
        .chain([account.clone()])
    {
        match by_endpoint.iter_mut().find(|a| a.endpoint == x.endpoint) {
            Some(slot) => *slot = x,
            None => by_endpoint.push(x),
        }
    }
    *accounts = sort_accounts(by_endpoint);

    save(store, accounts);
    Ok(account)
}

/// GHD `AccountsStore.removeAccount`: delete the token, drop the account
/// (same endpoint and id) and save the list. Nothing changes when the token
/// cannot be deleted.
pub fn remove_account(
    store: &Store,
    accounts: &mut Vec<Account>,
    account: &Account,
    secure_store: &dyn SecureStore,
) -> Result<(), AccountsError> {
    if let Err(err) = secure_store.delete_item(&account.host(), &account.login) {
        error!(login = %account.login, %err, "Error removing account");
        return Err(AccountsError::RemoveToken(err));
    }
    accounts.retain(|a| !(a.endpoint == account.endpoint && a.id == account.id));
    save(store, accounts);
    Ok(())
}

/// GHD `save`: the accounts, without tokens, to the data store.
fn save(store: &Store, accounts: &[Account]) {
    if let Err(err) = store.save_accounts(accounts) {
        error!(?err, "could not save accounts");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(login: &str, endpoint: &str, id: u64) -> Account {
        Account {
            endpoint: endpoint.to_string(),
            id,
            login: login.to_string(),
            name: None,
            avatar_url: None,
            emails: Vec::new(),
            scopes: Vec::new(),
            plan: None,
            private_primary_email: false,
        }
    }

    #[test]
    fn github_com_accounts_sort_first_in_insertion_order() {
        let sorted = sort_accounts(vec![
            account("a", "https://ghe.one/api/v3", 1),
            account("b", "https://api.github.com", 2),
            account("c", "https://ghe.two/api/v3", 3),
        ]);
        let logins: Vec<&str> = sorted.iter().map(|a| a.login.as_str()).collect();
        assert_eq!(logins, ["b", "a", "c"]);
    }

    struct NoSecrets;

    impl SecureStore for NoSecrets {
        fn set_item(&self, _: &str, _: &str, _: &str) -> Result<(), String> {
            Ok(())
        }

        fn delete_item(&self, _: &str, _: &str) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn adding_keeps_one_account_per_endpoint_in_its_first_place() {
        let dir = tempfile::tempdir().expect("temp dir");
        let store = Store::open_in(dir.path()).expect("open the store");
        let mut accounts = vec![
            account("a", "https://ghe.one/api/v3", 1),
            account("b", "https://ghe.two/api/v3", 2),
            account("a2", "https://ghe.one/api/v3", 3),
        ];
        add_account(
            &store,
            &mut accounts,
            account("c", "https://ghe.one/api/v3", 4),
            "",
            &NoSecrets,
        )
        .expect("add the account");
        let logins: Vec<&str> = accounts.iter().map(|a| a.login.as_str()).collect();
        assert_eq!(logins, ["c", "b"]);
    }
}
