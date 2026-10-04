//! Port of GitHub Desktop's `app/test/unit/group-branches-test.ts`.
//!
//! Corvene equivalent: `groupBranches(defaultBranch, currentBranch,
//! allBranches, recentBranches)` (`ui/branches/group-branches.ts`) is
//! `corvene_ui::branch_list::group_branches(branches, default_branch,
//! recent, query, newest_first)`, which takes the default branch and the
//! recent branches by name (as Corvene stores them), never used the
//! current branch (GitHub Desktop's ignores it too), filters by `query`
//! (GitHub Desktop's `FilterList` filters afterwards; `""` here) and sorts
//! Other Branches newest first when `newest_first` is set (flag
//! `848-branch-list-sort-by-date`, off in the github-desktop preset, so
//! `false`).
//!
//! A group's `identifier` (`'default'`, `'recent'`, `'other'`) is the
//! group's `title`, the label GitHub Desktop's `getGroupLabel`
//! (`ui/branches/branch-list.tsx`) shows for it ([`group_label`]); an
//! item's `branch` is an entry of `BranchGroup::branches`. GitHub Desktop's
//! `Branch` is `corvene_core::Branch` (the `corvene_models` type): the tip's
//! author date is `tip_time`, `ref: ''` an empty `full_name`.

use std::time::{SystemTime, UNIX_EPOCH};

use corvene_core::{Branch, BranchKind};
use corvene_ui::branch_list::group_branches;

/// GitHub Desktop's `getGroupLabel(identifier)` (`ui/branches/branch-list.tsx`).
fn group_label(identifier: &str) -> &'static str {
    let darwin = cfg!(target_os = "macos");
    match identifier {
        "default" if darwin => "Default Branch",
        "default" => "Default branch",
        "recent" if darwin => "Recent Branches",
        "recent" => "Recent branches",
        "other" if darwin => "Other Branches",
        "other" => "Other branches",
        _ => panic!("unknown group identifier {identifier}"),
    }
}

// GHD: unit/group-branches-test.ts › Branches grouping › should group branches
#[test]
fn should_group_branches() {
    // `new CommitIdentity('Hubot', 'hubot@github.com', new Date())`
    let author_date = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time after the epoch")
        .as_secs() as i64;

    let branch = |name: &str| Branch {
        name: name.to_string(),
        kind: BranchKind::Local,
        full_name: String::new(),
        tip: Some("300acef".to_string()),
        upstream: None,
        tip_time: Some(author_date),
        tip_author: None,
        remote_name: None,
    };

    let current_branch = branch("master");
    let default_branch = branch("master");
    let recent_branches = [branch("some-recent-branch")];
    let other_branch = branch("other-branch");

    let all_branches = [
        current_branch.clone(),
        recent_branches[0].clone(),
        other_branch.clone(),
    ];

    let recent_names: Vec<String> = recent_branches.iter().map(|b| b.name.clone()).collect();
    let groups = group_branches(
        &all_branches,
        Some(default_branch.name.as_str()),
        &recent_names,
        "",
        false,
    );
    assert_eq!(groups.len(), 3);

    assert_eq!(groups[0].title, group_label("default"));
    let items = &groups[0].branches;
    assert_eq!(items[0], default_branch);

    assert_eq!(groups[1].title, group_label("recent"));
    let items = &groups[1].branches;
    assert_eq!(items[0], recent_branches[0]);

    assert_eq!(groups[2].title, group_label("other"));
    let items = &groups[2].branches;
    assert_eq!(items[0], other_branch);
}
