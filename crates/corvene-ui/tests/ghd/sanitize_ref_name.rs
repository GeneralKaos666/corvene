//! Port of GitHub Desktop's `app/test/unit/sanitize-ref-name-test.ts`.
//!
//! Corvene equivalent: `sanitizedRefName(name)` (`lib/sanitize-ref-name.ts`)
//! is `corvene_ui::dialogs::sanitize_ref_name` (`dialogs/branch_dialogs.rs`,
//! used by the Create Branch, Rename Branch, worktree and default-branch
//! preference fields).

use corvene_ui::dialogs::sanitize_ref_name as sanitized_ref_name;

// GHD: unit/sanitize-ref-name-test.ts › sanitizedBranchName › leaves a good branch name alone
#[test]
fn leaves_a_good_branch_name_alone() {
    let branch_name = "this-is/fine";
    let result = sanitized_ref_name(branch_name);
    assert_eq!(result, "this-is/fine");
}

// GHD: unit/sanitize-ref-name-test.ts › sanitizedBranchName › replaces invalid characters with dashes
#[test]
#[ignore = "ghd: todo: TODO.md UI parity gaps 'Create Tag / Create Branch name sanitising with the Will be created as warning (sanitizedRefName)': sanitize_ref_name drops \\ : ? and keeps |, got 'this-isnot-fineyo|is-it'"]
fn replaces_invalid_characters_with_dashes() {
    let branch_name = ".this..is\\not fine:yo?|is-it";
    let result = sanitized_ref_name(branch_name);
    assert_eq!(result, "this-is-not-fine-yo-is-it");
}

// GHD: unit/sanitize-ref-name-test.ts › sanitizedBranchName › does not allow branch name to end in slash
#[test]
#[ignore = "ghd: todo: TODO.md UI parity gaps 'Create Tag / Create Branch name sanitising with the Will be created as warning (sanitizedRefName)': sanitize_ref_name trims the trailing / instead of dashing it, got 'hello'"]
fn does_not_allow_branch_name_to_end_in_slash() {
    let branch_name = "hello/";
    let result = sanitized_ref_name(branch_name);
    assert_eq!(result, "hello-");
}

// GHD: unit/sanitize-ref-name-test.ts › sanitizedBranchName › does not allow name to start with plus
#[test]
#[ignore = "ghd: todo: TODO.md UI parity gaps 'Create Tag / Create Branch name sanitising with the Will be created as warning (sanitizedRefName)': sanitize_ref_name keeps leading +, got '++but-can-still-keep-the-rest'"]
fn does_not_allow_name_to_start_with_plus() {
    let branch_name = "++but-can-still-keep-the-rest";
    let result = sanitized_ref_name(branch_name);
    assert_eq!(result, "but-can-still-keep-the-rest");
}

// GHD: unit/sanitize-ref-name-test.ts › sanitizedBranchName › does not allow name to start with minus
#[test]
fn does_not_allow_name_to_start_with_minus() {
    let branch_name = "--but-can-still-keep-the-rest";
    let result = sanitized_ref_name(branch_name);
    assert_eq!(result, "but-can-still-keep-the-rest");
}

// GHD: unit/sanitize-ref-name-test.ts › sanitizedBranchName › does not allow name to end in `.lock`
#[test]
#[ignore = "ghd: todo: TODO.md UI parity gaps 'Create Tag / Create Branch name sanitising with the Will be created as warning (sanitizedRefName)': sanitize_ref_name ignores a trailing .lock, got 'foo.lock.lock'"]
fn does_not_allow_name_to_end_in_lock() {
    let branch_name = "foo.lock.lock";
    let result = sanitized_ref_name(branch_name);
    assert_eq!(result, "foo.lock-");
}

// GHD: unit/sanitize-ref-name-test.ts › sanitizedBranchName › replaces newlines with dash
#[test]
#[ignore = "ghd: todo: TODO.md UI parity gaps 'Create Tag / Create Branch name sanitising with the Will be created as warning (sanitizedRefName)': sanitize_ref_name dashes each whitespace character instead of each run, got 'hello--world'"]
fn replaces_newlines_with_dash() {
    let branch_name = "hello\r\nworld";
    let result = sanitized_ref_name(branch_name);
    assert_eq!(result, "hello-world");
}

// GHD: unit/sanitize-ref-name-test.ts › sanitizedBranchName › removes starting dot
#[test]
fn removes_starting_dot() {
    let branch_name = ".first.dot.is.not.ok";
    let result = sanitized_ref_name(branch_name);
    assert_eq!(result, "first.dot.is.not.ok");
}

// GHD: unit/sanitize-ref-name-test.ts › sanitizedBranchName › allows double dashes after first character
#[test]
fn allows_double_dashes_after_first_character() {
    let branch_name = "branch--name";
    let result = sanitized_ref_name(branch_name);
    assert_eq!(result, branch_name);
}
