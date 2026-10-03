//! Port of GitHub Desktop's `app/test/unit/local-storage-test.ts`
//! (`lib/local-storage.ts`).
//!
//! GitHub Desktop keeps most preferences in the renderer's `localStorage`,
//! which holds strings only: `setBoolean` / `setNumber` write `'1'` / `'0'`
//! and decimal text, `getBoolean` / `getNumber` parse them back and fall
//! back to a default for a missing or malformed value. Corvene's counterpart
//! of `localStorage` is `corvene_store::Store` (one redb file of JSON values
//! by key):
//!
//! - `setBoolean(key, value)` / `setNumber(key, value)` is
//!   `Store::set(key, &value)`,
//! - `getBoolean(key)` / `getNumber(key)` is `Store::get::<T>(key)`, whose
//!   `Ok(None)` for a missing key is GitHub Desktop's `undefined`,
//! - `getBoolean(key, defaultValue)` / `getNumber(key, defaultValue)` is
//!   `Store::get::<T>(key)?.unwrap_or(default)`, the way Corvene's own
//!   callers supply a default (`corvene_core::persistence`, e.g.
//!   `next_repository_id`).
//!
//! A `beforeEach` clears `localStorage`; here every case opens a new store
//! in a temporary directory. The cases that write a raw string with
//! `localStorage.setItem` (`'blahblahblah'`, `'0'`, `'true'`, `'false'`) are
//! skipped (`tools/ghd-tests/skips/platform.tsv`): the store only holds
//! JSON values written by `Store::set`, so there is no string to parse.

use corvene_store::Store;
use corvene_test_support::create_temp_directory;
use tempfile::TempDir;

const BOOLEAN_KEY: &str = "some-boolean-key";
const NUMBER_KEY: &str = "some-number-key";

/// A fresh, empty store (`localStorage.clear()`), with the directory that
/// holds it.
fn empty_store() -> (TempDir, Store) {
    let dir = create_temp_directory();
    let store = Store::open_in(dir.path()).expect("open the store");
    (dir, store)
}

// GHD: unit/local-storage-test.ts › local storage › setBoolean › round-trips a true value
#[test]
fn round_trips_a_true_value() {
    let (_dir, store) = empty_store();
    let expected = true;
    store.set(BOOLEAN_KEY, &expected).unwrap();
    assert_eq!(store.get::<bool>(BOOLEAN_KEY).unwrap(), Some(expected));
}

// GHD: unit/local-storage-test.ts › local storage › setBoolean › round-trips a false value
#[test]
fn round_trips_a_false_value() {
    let (_dir, store) = empty_store();
    let expected = false;
    store.set(BOOLEAN_KEY, &expected).unwrap();
    assert_eq!(store.get::<bool>(BOOLEAN_KEY).unwrap(), Some(expected));
}

// GHD: unit/local-storage-test.ts › local storage › getBoolean parsing › returns default value when no key found
#[test]
fn get_boolean_returns_default_value_when_no_key_found() {
    let (_dir, store) = empty_store();
    let default_value = true;

    let actual = store
        .get::<bool>(BOOLEAN_KEY)
        .unwrap()
        .unwrap_or(default_value);

    assert_eq!(actual, default_value);
}

// GHD: unit/local-storage-test.ts › local storage › setNumber › round-trip a valid number
#[test]
fn round_trip_a_valid_number() {
    let (_dir, store) = empty_store();
    let expected: i64 = 12345;

    store.set(NUMBER_KEY, &expected).unwrap();

    assert_eq!(store.get::<i64>(NUMBER_KEY).unwrap(), Some(expected));
}

// GHD: unit/local-storage-test.ts › local storage › setNumber › round-trip zero and ignore default value
#[test]
fn round_trip_zero_and_ignore_default_value() {
    let (_dir, store) = empty_store();
    let expected: i64 = 0;
    let default_number: i64 = 1234;

    store.set(NUMBER_KEY, &expected).unwrap();

    assert_eq!(
        store
            .get::<i64>(NUMBER_KEY)
            .unwrap()
            .unwrap_or(default_number),
        expected
    );
}

// GHD: unit/local-storage-test.ts › local storage › getNumber parsing › returns default value when no key found
#[test]
fn get_number_returns_default_value_when_no_key_found() {
    let (_dir, store) = empty_store();
    let default_value: i64 = 3456;
    let actual = store
        .get::<i64>(NUMBER_KEY)
        .unwrap()
        .unwrap_or(default_value);
    assert_eq!(actual, default_value);
}
