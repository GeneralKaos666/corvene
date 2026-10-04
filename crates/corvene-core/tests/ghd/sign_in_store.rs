//! Port of GitHub Desktop's `app/test/unit/sign-in-store-test.ts`.
//!
//! GitHub Desktop's `SignInStore` (`lib/stores/sign-in-store.ts`) is
//! `corvene_core::sign_in::SignInStore`, the state machine behind
//! `AppState::sign_in_store` and the sign-in dialog; its state is
//! `SignInState` (a `kind` with the fields of each step), `getState()`
//! returns it by reference. `AccountsStore` is
//! `crate::accounts_support::AccountsStore`, which the store reads its
//! accounts from (`AccountsSource`) and whose `addAccount` is
//! `corvene_core::accounts::add_account`. `getDotComAPIEndpoint()` is
//! `Endpoint::github_com().api_base`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::accounts_support::{
    AccountsStore, InMemoryStore, USERS, get_dot_com_api_endpoint, new_account,
};
use corvene_core::Account;
use corvene_core::sign_in::{SignInResult, SignInState, SignInStep, SignInStore};

/// `createAccountsStore(accounts = [])`.
fn create_accounts_store(accounts: &[Account]) -> AccountsStore {
    let data_store = InMemoryStore::new();
    if !accounts.is_empty() {
        let serialized = serde_json::to_value(accounts).expect("serialize the accounts");
        data_store.set_item(USERS, &serialized);
    }
    AccountsStore::new(data_store)
}

/// `createDotComAccount(login = 'octocat')`.
fn create_dot_com_account(login: Option<&str>) -> Account {
    let login = login.unwrap_or("octocat");
    new_account(
        login,
        &get_dot_com_api_endpoint(),
        "test-token",
        &[],
        "https://avatars.githubusercontent.com/u/1",
        1,
        login,
        "free",
    )
}

/// `createEnterpriseAccount(login = 'enterprise-user', endpoint =
/// 'https://github.example.com/api/v3')`.
fn create_enterprise_account(login: Option<&str>, endpoint: Option<&str>) -> Account {
    let login = login.unwrap_or("enterprise-user");
    let endpoint = endpoint.unwrap_or("https://github.example.com/api/v3");
    new_account(login, endpoint, "ent-token", &[], "", 2, login, "free")
}

/// `beforeEach`: `accountsStore = createAccountsStore(); signInStore = new
/// SignInStore(accountsStore)`.
fn setup() -> (Rc<AccountsStore>, SignInStore) {
    let accounts_store = Rc::new(create_accounts_store(&[]));
    let sign_in_store = SignInStore::new(accounts_store.clone());
    (accounts_store, sign_in_store)
}

// GHD: unit/sign-in-store-test.ts › SignInStore › initial state › starts with null state
#[test]
fn starts_with_null_state() {
    let (_accounts_store, sign_in_store) = setup();
    assert_eq!(sign_in_store.get_state(), None);
}

// GHD: unit/sign-in-store-test.ts › SignInStore › beginDotComSignIn › transitions to Authentication step when no existing account
#[test]
fn transitions_to_authentication_step_when_no_existing_account() {
    let (_accounts_store, mut sign_in_store) = setup();
    sign_in_store.begin_dot_com_sign_in(None);
    let state = sign_in_store.get_state();
    assert_ne!(state, None);
    assert_eq!(
        state.as_ref().map(|s| s.kind),
        Some(SignInStep::Authentication)
    );
    if let Some(state) = state.filter(|s| s.kind == SignInStep::Authentication) {
        assert_eq!(state.endpoint, Some(get_dot_com_api_endpoint()));
        assert_eq!(state.error, None);
        assert!(!state.loading);
    }
}

// GHD: unit/sign-in-store-test.ts › SignInStore › beginDotComSignIn › transitions to ExistingAccountWarning when a dotcom account exists
#[test]
fn transitions_to_existing_account_warning_when_a_dotcom_account_exists() {
    let (_accounts_store, _sign_in_store) = setup();
    let existing_account = create_dot_com_account(None);
    let accounts_store = Rc::new(create_accounts_store(&[]));
    let mut sign_in_store = SignInStore::new(accounts_store.clone());

    accounts_store.add_account(existing_account);

    sign_in_store.begin_dot_com_sign_in(None);
    let state = sign_in_store.get_state();
    assert_ne!(state, None);
    assert_eq!(
        state.map(|s| s.kind),
        Some(SignInStep::ExistingAccountWarning)
    );
}

// GHD: unit/sign-in-store-test.ts › SignInStore › beginDotComSignIn › calls resultCallback when provided
#[test]
fn calls_result_callback_when_provided() {
    let (_accounts_store, mut sign_in_store) = setup();
    let callback_called = Rc::new(Cell::new(false));
    sign_in_store.begin_dot_com_sign_in(Some(Box::new({
        let callback_called = callback_called.clone();
        move |_| callback_called.set(true)
    })));

    // Reset triggers the callback with 'cancelled'
    sign_in_store.reset();
    assert!(callback_called.get());
}

// GHD: unit/sign-in-store-test.ts › SignInStore › beginEnterpriseSignIn › transitions to EndpointEntry step
#[test]
fn transitions_to_endpoint_entry_step() {
    let (_accounts_store, mut sign_in_store) = setup();
    sign_in_store.begin_enterprise_sign_in(None);
    let state = sign_in_store.get_state();
    assert_ne!(state, None);
    assert_eq!(state.map(|s| s.kind), Some(SignInStep::EndpointEntry));
}

// GHD: unit/sign-in-store-test.ts › SignInStore › beginEnterpriseSignIn › sets initial state correctly
#[test]
fn sets_initial_state_correctly() {
    let (_accounts_store, mut sign_in_store) = setup();
    sign_in_store.begin_enterprise_sign_in(None);
    let state = sign_in_store.get_state();
    if let Some(state) = state.filter(|s| s.kind == SignInStep::EndpointEntry) {
        assert_eq!(state.error, None);
        assert!(!state.loading);
    }
}

// GHD: unit/sign-in-store-test.ts › SignInStore › beginEnterpriseSignIn › resets previous state before starting
#[test]
fn resets_previous_state_before_starting() {
    let (_accounts_store, mut sign_in_store) = setup();
    // Start a dotcom sign-in first
    sign_in_store.begin_dot_com_sign_in(None);
    assert_eq!(
        sign_in_store.get_state().map(|s| s.kind),
        Some(SignInStep::Authentication)
    );

    // Starting enterprise sign-in should replace that state
    sign_in_store.begin_enterprise_sign_in(None);
    assert_eq!(
        sign_in_store.get_state().map(|s| s.kind),
        Some(SignInStep::EndpointEntry)
    );
}

// GHD: unit/sign-in-store-test.ts › SignInStore › setEndpoint › transitions to Authentication step for valid enterprise URL
#[test]
fn transitions_to_authentication_step_for_valid_enterprise_url() {
    let (_accounts_store, mut sign_in_store) = setup();
    sign_in_store.begin_enterprise_sign_in(None);
    sign_in_store.set_endpoint("https://github.example.com");

    let state = sign_in_store.get_state();
    assert_eq!(state.map(|s| s.kind), Some(SignInStep::Authentication));
}

// GHD: unit/sign-in-store-test.ts › SignInStore › setEndpoint › redirects to dotcom flow for github.com URLs
#[test]
fn redirects_to_dotcom_flow_for_github_com_urls() {
    let (_accounts_store, mut sign_in_store) = setup();
    sign_in_store.begin_enterprise_sign_in(None);
    sign_in_store.set_endpoint("https://github.com");

    let state = sign_in_store.get_state();
    // Should redirect to the Authentication step with the dotcom endpoint
    assert_eq!(
        state.as_ref().map(|s| s.kind),
        Some(SignInStep::Authentication)
    );
    if let Some(state) = state.filter(|s| s.kind == SignInStep::Authentication) {
        assert_eq!(state.endpoint, Some(get_dot_com_api_endpoint()));
    }
}

// GHD: unit/sign-in-store-test.ts › SignInStore › setEndpoint › redirects to dotcom flow for api.github.com URLs
#[test]
fn redirects_to_dotcom_flow_for_api_github_com_urls() {
    let (_accounts_store, mut sign_in_store) = setup();
    sign_in_store.begin_enterprise_sign_in(None);
    sign_in_store.set_endpoint("https://api.github.com");

    let state = sign_in_store.get_state();
    assert_eq!(
        state.as_ref().map(|s| s.kind),
        Some(SignInStep::Authentication)
    );
    if let Some(state) = state.filter(|s| s.kind == SignInStep::Authentication) {
        assert_eq!(state.endpoint, Some(get_dot_com_api_endpoint()));
    }
}

// GHD: unit/sign-in-store-test.ts › SignInStore › setEndpoint › sets error for non-HTTPS URL
#[test]
fn sets_error_for_non_https_url() {
    let (_accounts_store, mut sign_in_store) = setup();
    sign_in_store.begin_enterprise_sign_in(None);
    sign_in_store.set_endpoint("http://github.example.com");

    let state = sign_in_store.get_state();
    assert_eq!(
        state.as_ref().map(|s| s.kind),
        Some(SignInStep::EndpointEntry)
    );
    if let Some(state) = state.filter(|s| s.kind == SignInStep::EndpointEntry) {
        assert_ne!(state.error, None);
        assert!(!state.loading);
    }
}

// GHD: unit/sign-in-store-test.ts › SignInStore › setEndpoint › shows ExistingAccountWarning if enterprise account exists
#[test]
fn shows_existing_account_warning_if_enterprise_account_exists() {
    let (_accounts_store, _sign_in_store) = setup();
    let endpoint = "https://github.example.com/api/v3";
    let existing_account = create_enterprise_account(Some("user"), Some(endpoint));
    let accounts_store = Rc::new(create_accounts_store(&[]));
    let mut sign_in_store = SignInStore::new(accounts_store.clone());

    accounts_store.add_account(existing_account);

    sign_in_store.begin_enterprise_sign_in(None);
    sign_in_store.set_endpoint("https://github.example.com");

    let state = sign_in_store.get_state();
    assert_eq!(
        state.map(|s| s.kind),
        Some(SignInStep::ExistingAccountWarning)
    );
}

// GHD: unit/sign-in-store-test.ts › SignInStore › reset › clears the state back to null
#[test]
fn clears_the_state_back_to_null() {
    let (_accounts_store, mut sign_in_store) = setup();
    sign_in_store.begin_dot_com_sign_in(None);
    assert_ne!(sign_in_store.get_state(), None);

    sign_in_store.reset();
    assert_eq!(sign_in_store.get_state(), None);
}

// GHD: unit/sign-in-store-test.ts › SignInStore › reset › calls resultCallback with cancelled
#[test]
fn calls_result_callback_with_cancelled() {
    let (_accounts_store, mut sign_in_store) = setup();
    let result: Rc<RefCell<Option<SignInResult>>> = Rc::new(RefCell::new(None));
    sign_in_store.begin_dot_com_sign_in(Some(Box::new({
        let result = result.clone();
        move |r| *result.borrow_mut() = Some(r)
    })));

    sign_in_store.reset();
    let result = result.borrow();
    assert!(result.is_some());
    assert_eq!(result.as_ref().unwrap().kind(), "cancelled");
}

// GHD: unit/sign-in-store-test.ts › SignInStore › onDidUpdate › emits updates when state changes
#[test]
fn emits_updates_when_state_changes() {
    let (_accounts_store, mut sign_in_store) = setup();
    let states: Rc<RefCell<Vec<Option<SignInState>>>> = Rc::new(RefCell::new(Vec::new()));
    sign_in_store.on_did_update({
        let states = states.clone();
        move |state| states.borrow_mut().push(state)
    });

    sign_in_store.begin_dot_com_sign_in(None);
    let states = states.borrow();
    assert_eq!(states.len(), 1);
    assert_eq!(
        states[0].as_ref().map(|s| s.kind),
        Some(SignInStep::Authentication)
    );
}

// GHD: unit/sign-in-store-test.ts › SignInStore › onDidUpdate › emits null when reset
#[test]
fn emits_null_when_reset() {
    let (_accounts_store, mut sign_in_store) = setup();
    let states: Rc<RefCell<Vec<Option<SignInState>>>> = Rc::new(RefCell::new(Vec::new()));
    sign_in_store.on_did_update({
        let states = states.clone();
        move |state| states.borrow_mut().push(state)
    });

    sign_in_store.begin_dot_com_sign_in(None);
    sign_in_store.reset();

    // Should have: cancelled callback + null state + possibly more
    // (`states[states.length - 1]` is `undefined`, which `assert.equal`
    // takes for `null`, when nothing was emitted)
    let last_state = states.borrow().last().cloned().flatten();
    assert_eq!(last_state, None);
}
