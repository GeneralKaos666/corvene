//! Port of GitHub Desktop's `app/test/unit/remove-remote-prefix-test.ts`.
//!
//! Corvene has no free `removeRemotePrefix(name)` (`lib/remove-remote-prefix.ts`:
//! everything after the first `/`, or `null` without one). Its counterpart
//! is `corvene_models::Branch::name_without_remote`, GitHub Desktop's
//! `Branch.nameWithoutRemote` (`removeRemotePrefix(name) || name` for a
//! remote branch), which the first two cases call on a remote branch named
//! like the input ([`remote_branch`]); `remote_name: None` is the split at
//! the first `/` (flag `256-remote-names-with-slashes` off, as in the
//! github-desktop preset). It falls back to the whole name instead of
//! `null`, so the third case calls the stand-in [`remove_remote_prefix`]
//! and is ignored until a function returning `None` exists.

use corvene_models::{Branch, BranchKind};

/// A remote-tracking branch whose short name is `name`.
fn remote_branch(name: &str) -> Branch {
    Branch {
        name: name.to_string(),
        kind: BranchKind::Remote,
        full_name: format!("refs/remotes/{name}"),
        tip: None,
        upstream: None,
        tip_time: None,
        remote_name: None,
    }
}

/// Stand-in for GitHub Desktop's `removeRemotePrefix(name)`
/// (`lib/remove-remote-prefix.ts`). Replace it with the Corvene function
/// once there is one and remove the `#[ignore]`.
fn remove_remote_prefix(_name: &str) -> Option<String> {
    unimplemented!("Corvene has no removeRemotePrefix")
}

// GHD: unit/remove-remote-prefix-test.ts › removeRemotePrefix › removes the remote prefix
#[test]
fn removes_the_remote_prefix() {
    let branch = remote_branch("origin/test");
    let name = branch.name_without_remote();
    assert_eq!(name, "test");
}

// GHD: unit/remove-remote-prefix-test.ts › removeRemotePrefix › removes only the remote prefix and not any subsequent /'s
#[test]
fn removes_only_the_remote_prefix_and_not_any_subsequent_slashes() {
    let branch = remote_branch("origin/test/name");
    let name = branch.name_without_remote();
    assert_eq!(name, "test/name");
}

// GHD: unit/remove-remote-prefix-test.ts › removeRemotePrefix › returns null if there is no remote prefix
#[test]
#[ignore = "ghd: missing: no removeRemotePrefix (lib/remove-remote-prefix.ts) returning None; Branch::name_without_remote falls back to the whole name"]
fn returns_null_if_there_is_no_remote_prefix() {
    let name = remove_remote_prefix("name");
    assert!(name.is_none());
}
