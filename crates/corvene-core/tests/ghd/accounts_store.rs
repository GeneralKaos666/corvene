//! Port of GitHub Desktop's `app/test/unit/accounts-store-test.ts`.
//!
//! `AccountsStore` is `crate::accounts_support::AccountsStore` (see there):
//! `getAll` is `StoreExt::accounts`, the read `Dispatcher::init` does at
//! launch, over a `corvene_store::Store` in a temporary directory, and
//! `addAccount` is `corvene_core::accounts::add_account`, which
//! `Dispatcher::finish_sign_in` calls.
//!
//! The persisted `users` JSON uses Corvene's field names (`avatar_url`) and
//! leaves out `token`, which Corvene never writes to the store (it lives in
//! the keychain).

use serde_json::json;

use crate::accounts_support::{AccountsStore, InMemoryStore, USERS, new_account};

/// `beforeEach`: `new AccountsStore(new InMemoryStore(), new
/// AsyncInMemoryStore())`.
fn accounts_store() -> AccountsStore {
    AccountsStore::new(InMemoryStore::new())
}

// GHD: unit/accounts-store-test.ts › AccountsStore › adding a new user › contains the added user
#[test]
fn contains_the_added_user() {
    let accounts_store = accounts_store();
    let new_account_login = "joan";
    accounts_store.add_account(new_account(
        new_account_login,
        "",
        "deadbeef",
        &[],
        "",
        1,
        "",
        "free",
    ));

    let users = accounts_store.get_all();
    assert_eq!(users[0].login, new_account_login);
}

// GHD: unit/accounts-store-test.ts › AccountsStore › loading persisted users › migrates .ghe.com users still using /api/v3 to api. subdomain
#[test]
fn migrates_ghe_com_users_still_using_api_v3_to_api_subdomain() {
    let _accounts_store = accounts_store();
    let data_store = InMemoryStore::new();
    data_store.set_item(
        USERS,
        &json!([
            {
                "login": "joan",
                "endpoint": "https://whatever.ghe.com/api/v3",
                "emails": [],
                "avatar_url": "",
                "id": 1,
                "name": "",
                "plan": "free",
            },
        ]),
    );
    let accounts_store = AccountsStore::new(data_store.clone());

    let users = accounts_store.get_all();
    assert_eq!(users[0].login, "joan");
    assert_eq!(users[0].endpoint, "https://api.whatever.ghe.com/");

    let persisted_users = data_store.get_item(USERS);
    assert_eq!(persisted_users[0]["login"], "joan");
    assert_eq!(
        persisted_users[0]["endpoint"],
        "https://api.whatever.ghe.com/"
    );
}

// GHD: unit/accounts-store-test.ts › AccountsStore › loading persisted users › does NOT migrate GHE users already using the api. subdomain
#[test]
fn does_not_migrate_ghe_users_already_using_the_api_subdomain() {
    let _accounts_store = accounts_store();
    let data_store = InMemoryStore::new();
    data_store.set_item(
        USERS,
        &json!([
            {
                "login": "joan",
                "endpoint": "https://api.whatever.ghe.com/",
                "emails": [],
                "avatar_url": "",
                "id": 1,
                "name": "",
                "plan": "free",
            },
        ]),
    );
    let accounts_store = AccountsStore::new(data_store.clone());

    let users = accounts_store.get_all();
    assert_eq!(users[0].login, "joan");
    assert_eq!(users[0].endpoint, "https://api.whatever.ghe.com/");

    let persisted_users = data_store.get_item(USERS);
    assert_eq!(persisted_users[0]["login"], "joan");
    assert_eq!(
        persisted_users[0]["endpoint"],
        "https://api.whatever.ghe.com/"
    );
}

// GHD: unit/accounts-store-test.ts › AccountsStore › loading persisted users › does NOT migrate GHES users still using /api/v3 to api. subdomain
#[test]
fn does_not_migrate_ghes_users_still_using_api_v3_to_api_subdomain() {
    let _accounts_store = accounts_store();
    let data_store = InMemoryStore::new();
    data_store.set_item(
        USERS,
        &json!([
            {
                "login": "joan",
                "endpoint": "https://my-company-repos.com/api/v3",
                "emails": [],
                "avatar_url": "",
                "id": 1,
                "name": "",
                "plan": "free",
            },
        ]),
    );
    let accounts_store = AccountsStore::new(data_store.clone());

    let users = accounts_store.get_all();
    assert_eq!(users[0].login, "joan");
    assert_eq!(users[0].endpoint, "https://my-company-repos.com/api/v3");

    let persisted_users = data_store.get_item(USERS);
    assert_eq!(persisted_users[0]["login"], "joan");
    assert_eq!(
        persisted_users[0]["endpoint"],
        "https://my-company-repos.com/api/v3"
    );
}
