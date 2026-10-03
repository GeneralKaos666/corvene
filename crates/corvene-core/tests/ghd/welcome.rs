//! Port of GitHub Desktop's `app/test/unit/welcome-test.ts`.
//!
//! GitHub Desktop keeps "the welcome flow was shown" in `localStorage` under
//! `has-shown-welcome-flow` (`lib/welcome.ts`, read with `getBoolean`, written
//! with `setBoolean` as `'1'` / `'0'`). Corvene keeps it as
//! `Settings::welcome_completed` (documented as GHD `hasShownWelcomeFlow`)
//! inside the `settings` JSON value of its `corvene_store::Store`:
//!
//! - a fresh store (`localStorage.removeItem(key)` in `beforeEach`) is a new
//!   `Store` in a temporary directory,
//! - `localStorage.setItem(key, v)` writes `v` as the stored
//!   `welcome_completed` field: `'1'` / `'0'` (what `setBoolean` writes for
//!   true / false) become the JSON `true` / `false` Corvene writes, the
//!   non-numeric `'a'` stays the string `"a"`,
//! - `hasShownWelcomeFlow()` is `StoreExt::settings(store)?.welcome_completed`
//!   (`corvene_core::persistence`), the read Corvene makes at launch. GitHub
//!   Desktop's `getBoolean` falls back to the default for that one key when
//!   the stored value is not `'1'`/`'0'`/`'true'`/`'false'`; Corvene's
//!   `StoreExt::settings` fails for the whole record instead, and only the
//!   binary (`crates/corvene/src/main.rs`: `store.settings().unwrap_or_default()`,
//!   unreachable from here) turns that into the defaults for every setting.
//!   That fallback is not repeated here, so the non-numeric case is ignored,
//! - `markWelcomeFlowComplete()` is `Dispatcher::complete_welcome`, which
//!   needs a gpui `App`; there is no store-level entry point, so that case
//!   calls a stand-in and is ignored.

use corvene_core::persistence::StoreExt;
use corvene_store::Store;
use serde_json::{Value, json};
use tempfile::TempDir;

/// A fresh store: GitHub Desktop's `localStorage.removeItem(key)`.
fn fresh_store() -> (Store, TempDir) {
    let dir = corvene_test_support::create_temp_directory();
    let store = Store::open_in(dir.path()).expect("open the store");
    (store, dir)
}

/// GitHub Desktop's `localStorage.setItem('has-shown-welcome-flow', value)`.
fn set_stored_value(store: &Store, value: Value) {
    store
        .set("settings", &json!({ "welcome_completed": value }))
        .expect("write the settings");
}

/// GitHub Desktop's `localStorage.getItem('has-shown-welcome-flow')`.
fn stored_value(store: &Store) -> Option<Value> {
    store
        .get::<Value>("settings")
        .expect("read the settings")
        .and_then(|settings| settings.get("welcome_completed").cloned())
}

/// GitHub Desktop's `hasShownWelcomeFlow()`: the stored settings'
/// `welcome_completed` (see the module doc for the binary's fallback).
fn has_shown_welcome_flow(store: &Store) -> bool {
    store
        .settings()
        .expect("StoreExt::settings")
        .welcome_completed
}

/// Stand-in for GitHub Desktop's `markWelcomeFlowComplete()`
/// (`lib/welcome.ts`). Corvene's is `Dispatcher::complete_welcome(cx)`,
/// which needs a gpui `App`; replace this with a store-level function once
/// there is one and remove the `#[ignore]`.
fn mark_welcome_flow_complete(_store: &Store) {
    unimplemented!("Dispatcher::complete_welcome needs a gpui App; no store-level entry point")
}

// GHD: unit/welcome-test.ts › Welcome › hasShownWelcomeFlow › defaults to false when no value found
#[test]
fn defaults_to_false_when_no_value_found() {
    let (store, _dir) = fresh_store();
    assert!(!has_shown_welcome_flow(&store));
}

// GHD: unit/welcome-test.ts › Welcome › hasShownWelcomeFlow › returns false for some non-numeric value
#[test]
#[ignore = "ghd: bug: StoreExt::settings fails on the non-boolean welcome_completed \"a\" (main.rs then resets every setting), GHD getBoolean gives false for that key only"]
fn returns_false_for_some_non_numeric_value() {
    let (store, _dir) = fresh_store();
    set_stored_value(&store, json!("a"));
    assert!(!has_shown_welcome_flow(&store));
}

// GHD: unit/welcome-test.ts › Welcome › hasShownWelcomeFlow › returns false when zero found
#[test]
fn returns_false_when_zero_found() {
    let (store, _dir) = fresh_store();
    set_stored_value(&store, json!(false));
    assert!(!has_shown_welcome_flow(&store));
}

// GHD: unit/welcome-test.ts › Welcome › hasShownWelcomeFlow › returns true when one found
#[test]
fn returns_true_when_one_found() {
    let (store, _dir) = fresh_store();
    set_stored_value(&store, json!(true));
    assert!(has_shown_welcome_flow(&store));
}

// GHD: unit/welcome-test.ts › Welcome › markWelcomeFlowComplete › sets localStorage to 1
#[test]
#[ignore = "ghd: missing: markWelcomeFlowComplete (lib/welcome.ts) is Dispatcher::complete_welcome, which needs a gpui App; no store-level function"]
fn sets_local_storage_to_1() {
    let (store, _dir) = fresh_store();
    mark_welcome_flow_complete(&store);
    let value = stored_value(&store);
    assert_eq!(value, Some(json!(true)));
}
