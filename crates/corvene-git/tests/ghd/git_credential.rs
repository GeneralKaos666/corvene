//! Port of GitHub Desktop's `app/test/unit/git/credential-test.ts`.
//!
//! GitHub Desktop's `parseCredential(value)` / `formatCredential(credential)`
//! (`lib/git/credential.ts`) read and write git's credential helper
//! protocol (`key=value` lines, `key[]` arrays expanded to `key[0]`,
//! `key[1]`…) for its credential-helper trampoline and for `git credential
//! fill/approve/reject`. Corvene answers git through `GIT_ASKPASS` instead
//! (`corvene_git::AskpassEnv`, `crates/corvene/src/askpass.rs`); the two
//! functions are `corvene_git::parse_credential` and
//! `corvene_git::format_credential`. GitHub Desktop's `Map<string, string>`
//! keeps insertion order; Corvene's credential is a `Vec<(String, String)>`
//! of the entries in that order. `formatCredential` throws for a value it
//! cannot write, `format_credential` returns an `Err`.

use corvene_git::{format_credential, parse_credential};

fn entries(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

// GHD: unit/git/credential-test.ts › git/credential › parseCredential › expands arrays into numeric entries
#[test]
fn expands_arrays_into_numeric_entries() {
    assert_eq!(
        parse_credential("wwwauth[]=foo\nwwwauth[]=bar"),
        entries(&[("wwwauth[0]", "foo"), ("wwwauth[1]", "bar")])
    );
}

// GHD: unit/git/credential-test.ts › git/credential › formatCredential › transforms numbered array entries into unnumbered
#[test]
fn transforms_numbered_array_entries_into_unnumbered() {
    assert_eq!(
        format_credential(&entries(&[("wwwauth[0]", "foo"), ("wwwauth[1]", "bar")]))
            .expect("formatCredential"),
        "wwwauth[]=foo\nwwwauth[]=bar\n"
    );
}
