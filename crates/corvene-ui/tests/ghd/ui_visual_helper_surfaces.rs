//! Port of GitHub Desktop's
//! `app/test/unit/ui/visual-helper-surfaces-test.tsx` (the branch name
//! warnings; `Octicon` / `createOcticonElement` and `Donut` are skipped in
//! `tools/ghd-tests/skips/ui2.tsv`).
//!
//! GitHub Desktop's `ui/lib/branch-name-warnings.tsx`:
//!
//! - `renderBranchHasRemoteWarning(branch)` (Rename Branch): "This branch
//!   is tracking <upstream> and renaming this branch will not change the
//!   branch name on the remote." when the branch has an upstream. Corvene
//!   draws the same warning inline in `RenameBranchDialog::render`
//!   (`crates/corvene-ui/src/dialogs/branch_dialogs.rs`, from
//!   `Branch::upstream_short`).
//! - `renderBranchNameExistsOnRemoteWarning(sanitizedName, branches)`
//!   (Create Branch): "A branch named <name> already exists on the remote."
//!   when a remote branch has that name without its remote. Corvene's
//!   Create Branch dialog has no such warning (only the local "A branch
//!   named … already exists." error).
//!
//! [`render_branch_has_remote_warning`] and
//! [`render_branch_name_exists_on_remote_warning`] are stand-ins returning
//! the warning's text, `None` when GitHub Desktop renders nothing.
//!
//! Types: GitHub Desktop's `Branch(name, upstream, tip, type, ref)` is
//! `corvene_core::Branch`, whose `upstream` is the full ref
//! (`refs/remotes/origin/main` for GitHub Desktop's `origin/main`) and
//! whose `remote_name` is left unset (GitHub Desktop splits a remote
//! branch's name at the first `/`, as Corvene does without it). Not ported
//! (React DOM only): the `.warning-helper-text` class, here one `Some` per
//! rendered warning.

use corvene_core::{Branch, BranchKind};

/// GitHub Desktop's `createBranch(name, upstream, type, ref)` with the tip
/// `abc123`.
fn create_branch(name: &str, upstream: Option<&str>, kind: BranchKind, full_name: &str) -> Branch {
    Branch {
        name: name.to_string(),
        kind,
        full_name: full_name.to_string(),
        tip: Some("abc123".to_string()),
        upstream: upstream.map(|upstream| format!("refs/remotes/{upstream}")),
        tip_time: None,
        remote_name: None,
    }
}

/// Stand-in for GitHub Desktop's `renderBranchHasRemoteWarning(branch)`:
/// the warning's text, `None` when nothing is rendered. Replace it with the
/// Corvene function once there is one and remove the `#[ignore]`s.
fn render_branch_has_remote_warning(_branch: &Branch) -> Option<String> {
    unimplemented!("RenameBranchDialog::render draws the tracking warning inline")
}

/// Stand-in for GitHub Desktop's
/// `renderBranchNameExistsOnRemoteWarning(sanitizedName, branches)`: the
/// warning's text, `None` when nothing is rendered. Replace it with the
/// Corvene function once there is one and remove the `#[ignore]`s.
fn render_branch_name_exists_on_remote_warning(
    _sanitized_name: &str,
    _branches: &[Branch],
) -> Option<String> {
    unimplemented!("Corvene's Create Branch dialog has no \"already exists on the remote\" warning")
}

// GHD: unit/ui/visual-helper-surfaces-test.tsx › visual helper surfaces › renders remote-tracking and duplicate-name branch warnings when applicable
#[test]
#[ignore = "ghd: missing: no renderBranchHasRemoteWarning / renderBranchNameExistsOnRemoteWarning (branch-name-warnings.tsx); Rename Branch draws the first inline, Create Branch lacks the second"]
fn renders_remote_tracking_and_duplicate_name_branch_warnings_when_applicable() {
    let remote_tracking_branch = create_branch(
        "main",
        Some("origin/main"),
        BranchKind::Local,
        "refs/heads/main",
    );
    let remote_branch = create_branch(
        "origin/feature",
        None,
        BranchKind::Remote,
        "refs/remotes/origin/feature",
    );

    let warnings = [
        render_branch_has_remote_warning(&remote_tracking_branch),
        render_branch_name_exists_on_remote_warning("feature", &[remote_branch]),
    ];
    let text: String = warnings.iter().flatten().map(String::as_str).collect();

    assert_eq!(warnings.iter().flatten().count(), 2);
    assert!(text.contains("This branch is tracking"));
    assert!(text.contains("origin/main"));
    assert!(text.contains("A branch named"));
    assert!(text.contains("feature"));
}

// GHD: unit/ui/visual-helper-surfaces-test.tsx › visual helper surfaces › returns no branch warning markup when warning conditions are absent
#[test]
#[ignore = "ghd: missing: no renderBranchHasRemoteWarning / renderBranchNameExistsOnRemoteWarning (branch-name-warnings.tsx); Rename Branch draws the first inline, Create Branch lacks the second"]
fn returns_no_branch_warning_markup_when_warning_conditions_are_absent() {
    let local_branch = create_branch("topic", None, BranchKind::Local, "refs/heads/topic");
    let remote_branch = create_branch(
        "origin/feature",
        None,
        BranchKind::Remote,
        "refs/remotes/origin/feature",
    );

    let warnings = [
        render_branch_has_remote_warning(&local_branch),
        render_branch_name_exists_on_remote_warning("other", &[remote_branch]),
    ];
    let text: String = warnings.iter().flatten().map(String::as_str).collect();

    assert_eq!(text, "");
    // `queryByText(/branch named/i)`
    assert!(!text.to_lowercase().contains("branch named"));
}
