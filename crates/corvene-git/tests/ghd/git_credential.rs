//! Port of GitHub Desktop's `app/test/unit/git/credential-test.ts`.
//!
//! GitHub Desktop's `parseCredential(value)` / `formatCredential(credential)`
//! (`lib/git/credential.ts`) read and write git's credential helper
//! protocol (`key=value` lines, `key[]` arrays expanded to `key[0]`,
//! `key[1]`…) for its credential-helper trampoline and for `git credential
//! fill/approve/reject`. Corvene answers git through `GIT_ASKPASS` instead
//! (`corvene_git::AskpassEnv`, `crates/corvene/src/askpass.rs`) and never
//! speaks the credential protocol, so [`parse_credential`] and
//! [`format_credential`] stand in for both functions. GitHub Desktop's
//! `Map<string, string>` keeps insertion order; the stand-ins use a
//! `Vec<(String, String)>` of the entries in that order.

/// Stand-in for GitHub Desktop's `parseCredential(value)`: the entries of
/// the returned `Map`, in insertion order.
fn parse_credential(_value: &str) -> Vec<(String, String)> {
    unimplemented!("corvene_git has no parseCredential (lib/git/credential.ts)")
}

/// Stand-in for GitHub Desktop's `formatCredential(credential)`.
fn format_credential(_credential: &[(String, String)]) -> String {
    unimplemented!("corvene_git has no formatCredential (lib/git/credential.ts)")
}

fn entries(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

// GHD: unit/git/credential-test.ts › git/credential › parseCredential › expands arrays into numeric entries
#[test]
#[ignore = "ghd: missing: corvene_git has no parseCredential, Corvene uses GIT_ASKPASS instead of the credential helper protocol (lib/git/credential.ts)"]
fn expands_arrays_into_numeric_entries() {
    assert_eq!(
        parse_credential("wwwauth[]=foo\nwwwauth[]=bar"),
        entries(&[("wwwauth[0]", "foo"), ("wwwauth[1]", "bar")])
    );
}

// GHD: unit/git/credential-test.ts › git/credential › formatCredential › transforms numbered array entries into unnumbered
#[test]
#[ignore = "ghd: missing: corvene_git has no formatCredential, Corvene uses GIT_ASKPASS instead of the credential helper protocol (lib/git/credential.ts)"]
fn transforms_numbered_array_entries_into_unnumbered() {
    assert_eq!(
        format_credential(&entries(&[("wwwauth[0]", "foo"), ("wwwauth[1]", "bar")])),
        "wwwauth[]=foo\nwwwauth[]=bar\n"
    );
}
