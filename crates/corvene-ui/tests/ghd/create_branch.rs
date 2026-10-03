//! Port of GitHub Desktop's `app/test/unit/create-branch-test.ts`.
//!
//! Corvene has no equivalent of `getStartPoint(props, preferred)`
//! (`lib/create-branch.ts`): the Create a Branch dialog
//! (`corvene_ui::dialogs::CreateBranchDialog`, `dialogs/branch_dialogs.rs`)
//! picks its start point inline while rendering, from a private
//! `StartPoint` enum (default branch, current branch and, with flag
//! `843-create-branch-from-any-branch`, another branch) and the tip, with
//! no `Head` or `UpstreamDefaultBranch` choice to return. The cases call
//! the stand-in [`get_start_point`] (with [`StartPoint`] and [`BranchInfo`]
//! for GitHub Desktop's types) and are ignored until the function exists.
//!
//! GitHub Desktop's `Branch` objects are `corvene_core::Branch` (the
//! `corvene_models` type): `ref: ''` is an empty `full_name`, `tip.sha` is
//! `tip`, `upstream: null` is `None`; `TipState.Valid` / `TipState.Detached`
//! are `Tip::Valid` / `Tip::Detached`.

use corvene_core::{Branch, BranchKind, Tip};

/// GitHub Desktop's `StartPoint` (`models/branch.ts`).
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StartPoint {
    CurrentBranch,
    DefaultBranch,
    Head,
    UpstreamDefaultBranch,
}

/// GitHub Desktop's `BranchInfo` argument of `getStartPoint`.
#[allow(dead_code)]
struct BranchInfo<'a> {
    tip: &'a Tip,
    default_branch: Option<&'a Branch>,
    upstream_default_branch: Option<&'a Branch>,
}

/// Stand-in for GitHub Desktop's `getStartPoint(props, preferred)`
/// (`lib/create-branch.ts`). Replace it with the Corvene function once
/// there is one and remove the `#[ignore]`s.
fn get_start_point(_props: &BranchInfo, _preferred: StartPoint) -> StartPoint {
    unimplemented!("Corvene has no getStartPoint")
}

const STUB_TIP: &str = "deadbeef";

fn local_branch(name: &str) -> Branch {
    Branch {
        name: name.to_string(),
        kind: BranchKind::Local,
        full_name: String::new(),
        tip: Some(STUB_TIP.to_string()),
        upstream: None,
        tip_time: None,
        remote_name: None,
    }
}

fn default_branch() -> Branch {
    local_branch("my-default-branch")
}

fn some_other_branch() -> Branch {
    local_branch("some-other-branch")
}

/// `upstreamDefaultBranch = null`
const UPSTREAM_DEFAULT_BRANCH: Option<&Branch> = None;

/// The `action(startPoint)` of each `describe`: `getStartPoint({ tip,
/// defaultBranch, upstreamDefaultBranch }, startPoint)`.
fn action(tip: &Tip, start_point: StartPoint) -> StartPoint {
    let default_branch = default_branch();
    get_start_point(
        &BranchInfo {
            tip,
            default_branch: Some(&default_branch),
            upstream_default_branch: UPSTREAM_DEFAULT_BRANCH,
        },
        start_point,
    )
}

fn default_branch_tip() -> Tip {
    Tip::Valid {
        branch: default_branch(),
    }
}

fn non_default_branch_tip() -> Tip {
    Tip::Valid {
        branch: some_other_branch(),
    }
}

fn detached_tip() -> Tip {
    Tip::Detached {
        sha: "deadbeef".to_string(),
    }
}

// GHD: unit/create-branch-test.ts › create-branch/getStartPoint › for default branch › returns current HEAD when HEAD requested
#[test]
#[ignore = "ghd: missing: Corvene has no getStartPoint (lib/create-branch.ts); CreateBranchDialog picks its start point inline"]
fn for_default_branch_returns_current_head_when_head_requested() {
    let tip = default_branch_tip();
    assert_eq!(action(&tip, StartPoint::Head), StartPoint::Head);
}

// GHD: unit/create-branch-test.ts › create-branch/getStartPoint › for default branch › chooses current branch when current branch requested
#[test]
#[ignore = "ghd: missing: Corvene has no getStartPoint (lib/create-branch.ts); CreateBranchDialog picks its start point inline"]
fn for_default_branch_chooses_current_branch_when_current_branch_requested() {
    let tip = default_branch_tip();
    assert_eq!(
        action(&tip, StartPoint::CurrentBranch),
        StartPoint::CurrentBranch
    );
}

// GHD: unit/create-branch-test.ts › create-branch/getStartPoint › for default branch › chooses default branch when default branch requested
#[test]
#[ignore = "ghd: missing: Corvene has no getStartPoint (lib/create-branch.ts); CreateBranchDialog picks its start point inline"]
fn for_default_branch_chooses_default_branch_when_default_branch_requested() {
    let tip = default_branch_tip();
    assert_eq!(
        action(&tip, StartPoint::DefaultBranch),
        StartPoint::DefaultBranch
    );
}

// GHD: unit/create-branch-test.ts › create-branch/getStartPoint › for a non-default branch › returns current HEAD when HEAD requested
#[test]
#[ignore = "ghd: missing: Corvene has no getStartPoint (lib/create-branch.ts); CreateBranchDialog picks its start point inline"]
fn for_a_non_default_branch_returns_current_head_when_head_requested() {
    let tip = non_default_branch_tip();
    assert_eq!(action(&tip, StartPoint::Head), StartPoint::Head);
}

// GHD: unit/create-branch-test.ts › create-branch/getStartPoint › for a non-default branch › chooses current branch when current branch requested
#[test]
#[ignore = "ghd: missing: Corvene has no getStartPoint (lib/create-branch.ts); CreateBranchDialog picks its start point inline"]
fn for_a_non_default_branch_chooses_current_branch_when_current_branch_requested() {
    let tip = non_default_branch_tip();
    assert_eq!(
        action(&tip, StartPoint::CurrentBranch),
        StartPoint::CurrentBranch
    );
}

// GHD: unit/create-branch-test.ts › create-branch/getStartPoint › for a non-default branch › chooses default branch when default branch requested
#[test]
#[ignore = "ghd: missing: Corvene has no getStartPoint (lib/create-branch.ts); CreateBranchDialog picks its start point inline"]
fn for_a_non_default_branch_chooses_default_branch_when_default_branch_requested() {
    let tip = non_default_branch_tip();
    assert_eq!(
        action(&tip, StartPoint::DefaultBranch),
        StartPoint::DefaultBranch
    );
}

// GHD: unit/create-branch-test.ts › create-branch/getStartPoint › for detached HEAD › returns current HEAD when HEAD requested
#[test]
#[ignore = "ghd: missing: Corvene has no getStartPoint (lib/create-branch.ts); CreateBranchDialog picks its start point inline"]
fn for_detached_head_returns_current_head_when_head_requested() {
    let tip = detached_tip();
    assert_eq!(action(&tip, StartPoint::Head), StartPoint::Head);
}

// GHD: unit/create-branch-test.ts › create-branch/getStartPoint › for detached HEAD › returns current HEAD when current branch requested
#[test]
#[ignore = "ghd: missing: Corvene has no getStartPoint (lib/create-branch.ts); CreateBranchDialog picks its start point inline"]
fn for_detached_head_returns_current_head_when_current_branch_requested() {
    let tip = detached_tip();
    assert_eq!(action(&tip, StartPoint::CurrentBranch), StartPoint::Head);
}

// GHD: unit/create-branch-test.ts › create-branch/getStartPoint › for detached HEAD › returns current HEAD when default branch requested
#[test]
#[ignore = "ghd: missing: Corvene has no getStartPoint (lib/create-branch.ts); CreateBranchDialog picks its start point inline"]
fn for_detached_head_returns_current_head_when_default_branch_requested() {
    let tip = detached_tip();
    assert_eq!(action(&tip, StartPoint::DefaultBranch), StartPoint::Head);
}
