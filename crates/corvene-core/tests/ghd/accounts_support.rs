//! Helpers shared by `corvene-core`'s ports of GitHub Desktop's account
//! tests (`unit/accounts-store-test.ts`, `unit/find-account-test.ts`,
//! `unit/stores/sign-in-store-test.ts`).
//!
//! GitHub Desktop's `AccountsStore` (`lib/stores/accounts-store.ts`) keeps
//! the accounts in an `IDataStore` (`localStorage`, item `users`) and their
//! tokens in an `ISecureStore` (the keychain); its tests use
//! `InMemoryStore` / `AsyncInMemoryStore` (`app/test/helpers/stores`).
//! Corvene keeps the accounts in `AppState::accounts`, persisted whole under
//! the key `accounts` of `corvene_store::Store` by `StoreExt::save_accounts`
//! and read back at launch by `StoreExt::accounts` (`Dispatcher::init`);
//! tokens live in the OS keychain (`corvene_platform::keychain`), which
//! takes no injected store, so there is no `AsyncInMemoryStore` here.
//!
//! - [`InMemoryStore`] is a `corvene_store::Store` in a temporary directory,
//!   with GitHub Desktop's `setItem` / `getItem` over JSON values;
//! - [`AccountsStore::get_all`] is `StoreExt::accounts`, the read
//!   `Dispatcher::init` does (GitHub Desktop's `getAll` after
//!   `loadFromStore`);
//! - [`AccountsStore::add_account`] is a stand-in: Corvene adds an account in
//!   `Dispatcher::finish_sign_in` (replace the account of the same endpoint,
//!   `save_accounts`), inside a gpui update after fetching the user and
//!   writing the keychain.

use std::sync::Arc;

use corvene_core::Account;
use corvene_core::persistence::StoreExt;
use corvene_github::Endpoint;
use corvene_store::Store;
use tempfile::TempDir;

/// GitHub Desktop's `users` item of the data store is Corvene's `accounts`
/// key.
pub const USERS: &str = "accounts";

/// GitHub Desktop's `InMemoryStore` (`IDataStore`): a `corvene_store::Store`
/// in a temporary directory. Clones share the store.
#[derive(Clone)]
pub struct InMemoryStore {
    store: Arc<Store>,
    _dir: Arc<TempDir>,
}

impl InMemoryStore {
    pub fn new() -> Self {
        let dir = corvene_test_support::create_temp_directory();
        let store = Store::open_in(dir.path()).expect("open the store");
        Self {
            store: Arc::new(store),
            _dir: Arc::new(dir),
        }
    }

    /// `setItem(key, JSON.stringify(value))`.
    pub fn set_item(&self, key: &str, value: &serde_json::Value) {
        self.store.set(key, value).expect("write the store");
    }

    /// `JSON.parse(getItem(key))`.
    pub fn get_item(&self, key: &str) -> serde_json::Value {
        self.store
            .get::<serde_json::Value>(key)
            .expect("read the store")
            .expect("the item is set")
    }
}

/// GitHub Desktop's `AccountsStore` over an `InMemoryStore` (see the module
/// docs).
pub struct AccountsStore {
    data_store: InMemoryStore,
}

impl AccountsStore {
    /// `new AccountsStore(dataStore, new AsyncInMemoryStore())`.
    pub fn new(data_store: InMemoryStore) -> Self {
        Self { data_store }
    }

    /// `getAll()`: the accounts as loaded from the data store.
    pub fn get_all(&self) -> Vec<Account> {
        self.data_store.store.accounts().expect("read the accounts")
    }

    /// Stand-in for `addAccount(account)`: store the token, replace the
    /// account of the same endpoint, sort GitHub.com first and save. Corvene
    /// does this in `Dispatcher::finish_sign_in` (needs a gpui `App`, the
    /// network and the OS keychain); replace this once there is a gpui-free
    /// call and remove the `#[ignore]`s.
    pub fn add_account(&self, _account: Account) -> Option<Account> {
        unimplemented!("no gpui-free AccountsStore.addAccount (Dispatcher::finish_sign_in)")
    }
}

/// GitHub Desktop's `new Account(login, endpoint, token, emails, avatarURL,
/// id, name, plan)`. The token is dropped: Corvene keeps it in the keychain,
/// never in `Account`.
#[allow(clippy::too_many_arguments)]
pub fn new_account(
    login: &str,
    endpoint: &str,
    _token: &str,
    emails: &[&str],
    avatar_url: &str,
    id: u64,
    name: &str,
    plan: &str,
) -> Account {
    Account {
        endpoint: endpoint.to_string(),
        id,
        login: login.to_string(),
        name: Some(name.to_string()),
        avatar_url: Some(avatar_url.to_string()),
        emails: emails.iter().map(|e| e.to_string()).collect(),
        scopes: Vec::new(),
        plan: Some(plan.to_string()),
        private_primary_email: false,
    }
}

/// GitHub Desktop's `getDotComAPIEndpoint()` (`lib/api.ts`):
/// `Endpoint::github_com().api_base`.
pub fn get_dot_com_api_endpoint() -> String {
    Endpoint::github_com().api_base
}

/// GitHub Desktop's `getEnterpriseAPIURL(endpoint)` (`lib/api.ts`):
/// `Endpoint::enterprise(endpoint, false).api_base`.
pub fn get_enterprise_api_url(endpoint: &str) -> String {
    Endpoint::enterprise(endpoint, false)
        .expect("a valid Enterprise address")
        .api_base
}
