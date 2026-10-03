//! Port of GitHub Desktop's `app/test/unit/patch-formatter-test.ts`.
//!
//! Corvene equivalents:
//!
//! - `formatPatch(file, diff)` (`lib/patch-formatter.ts`) is
//!   `corvene_git::format_patch(file, hunks)`, which takes the hunks of the
//!   text diff and returns `None` where GitHub Desktop throws "Could not
//!   generate a patch, no changes" (no case here selects nothing).
//! - `getWorkingDirectoryDiff(repository, file)` (`lib/git/diff.ts`) is
//!   `corvene_git::working_directory_diff(git, path, file, false, false,
//!   false)`: whitespace shown, and flags `743-renamed-diff-against-head`
//!   and `749-binary-diff-as-text` at their GitHub Desktop value (off).
//! - The test's `parseDiff` (`DiffParser.parse` and `convertDiff` of a
//!   modified `file.txt`) is `corvene_git::parse_unified`, which, like
//!   GitHub Desktop's parser, reads a patch whose header has no `diff --git`
//!   line.
//! - `new WorkingDirectoryFileChange(path, { kind }, selection)` is a
//!   `WorkingDirectoryFileChange` of that kind ([`file_change`]); GitHub
//!   Desktop's status carries nothing but the kind, so the other fields are
//!   those of an unstaged change (index unchanged, working tree modified),
//!   which is what `modified-file.md` is in `repo-with-changes`.
//! - `DiffSelection.fromInitialSelection(All | None)` is
//!   `DiffSelection::all()` / `none()`, `withRangeSelection` `with_range`,
//!   `withLineSelection` `with_line`.
//! - `hunk.unifiedDiffEnd` is `corvene_test_support::unified_diff_end`:
//!   Corvene's `DiffHunk` has no such field.
//! - GitHub Desktop's `DiffLine.text` keeps the `+` / `-` / ` ` prefix;
//!   Corvene's drops it and `DiffLine::kind` says which it was.

use corvene_git::{format_patch, parse_unified};
use corvene_models::{
    Diff, DiffHunk, DiffLineKind, DiffSelection, FileStatusKind, GitStatusEntry,
    WorkingDirectoryFileChange,
};
use corvene_test_support::{
    get_working_directory_diff, setup_fixture_repository, unified_diff_end,
    working_directory_file_change,
};

/// `new WorkingDirectoryFileChange(path, { kind }, selection)` for an
/// unstaged change (see the module doc).
fn file_change(
    path: &str,
    kind: FileStatusKind,
    selection: DiffSelection,
) -> WorkingDirectoryFileChange {
    let mut file = working_directory_file_change(path, kind, selection);
    file.status.working_tree = GitStatusEntry::Modified;
    file
}

/// The hunks of a diff after `assert.equal(diff.kind, DiffType.Text)`.
fn text_hunks(diff: Diff) -> Vec<DiffHunk> {
    match diff {
        Diff::Text { hunks, .. } => hunks,
        other => panic!("expected a text diff, got {other:?}"),
    }
}

/// The test's `parseDiff(diff)`.
fn parse_diff(diff: &str) -> Vec<DiffHunk> {
    text_hunks(parse_unified(diff))
}

/// `formatPatch(file, diff)`.
fn format_patch_or_throw(file: &WorkingDirectoryFileChange, hunks: &[DiffHunk]) -> String {
    format_patch(file, hunks).expect("Could not generate a patch, no changes")
}

// GHD: unit/patch-formatter-test.ts › patch formatting › formatPatchesForModifiedFile › creates right patch when first hunk is selected
#[test]
fn creates_right_patch_when_first_hunk_is_selected() {
    let repository = setup_fixture_repository("repo-with-changes");

    let modified_file = "modified-file.md";

    let unselected_file = DiffSelection::none();
    let file = file_change(modified_file, FileStatusKind::Modified, unselected_file);

    let diff = get_working_directory_diff(&repository, &file);

    let text_diff = text_hunks(diff);
    let second = &text_diff[1];

    let selection = DiffSelection::all().with_range(
        second.unified_diff_start,
        unified_diff_end(second) - second.unified_diff_start,
        false,
    );

    let updated_file = file_change(modified_file, FileStatusKind::Modified, selection);

    let patch = format_patch_or_throw(&updated_file, &text_diff);

    assert!(patch.contains("--- a/modified-file.md\n"), "{patch}");
    assert!(patch.contains("+++ b/modified-file.md\n"), "{patch}");
    assert!(patch.contains("@@ -4,10 +4,6 @@"), "{patch}");
}

// GHD: unit/patch-formatter-test.ts › patch formatting › formatPatchesForModifiedFile › creates right patch when second hunk is selected
#[test]
fn creates_right_patch_when_second_hunk_is_selected() {
    let repository = setup_fixture_repository("repo-with-changes");

    let modified_file = "modified-file.md";
    let unselected_file = DiffSelection::none();
    let file = file_change(modified_file, FileStatusKind::Modified, unselected_file);

    let diff = get_working_directory_diff(&repository, &file);

    let text_diff = text_hunks(diff);
    let first = &text_diff[0];

    let selection = DiffSelection::all().with_range(
        first.unified_diff_start,
        unified_diff_end(first) - first.unified_diff_start,
        false,
    );

    let updated_file = file_change(modified_file, FileStatusKind::Modified, selection);

    let patch = format_patch_or_throw(&updated_file, &text_diff);

    assert!(patch.contains("--- a/modified-file.md\n"), "{patch}");
    assert!(patch.contains("+++ b/modified-file.md\n"), "{patch}");
    assert!(patch.contains("@@ -21,6 +17,10 @@"), "{patch}");
}

// GHD: unit/patch-formatter-test.ts › patch formatting › formatPatchesForModifiedFile › creates right patch when first and third hunk is selected
#[test]
fn creates_right_patch_when_first_and_third_hunk_is_selected() {
    let repository = setup_fixture_repository("repo-with-changes");

    let modified_file = "modified-file.md";

    let unselected_file = DiffSelection::none();
    let file = file_change(modified_file, FileStatusKind::Modified, unselected_file);

    let diff = get_working_directory_diff(&repository, &file);

    let text_diff = text_hunks(diff);
    let second = &text_diff[1];

    let selection = DiffSelection::all().with_range(
        second.unified_diff_start,
        unified_diff_end(second) - second.unified_diff_start,
        false,
    );
    let updated_file = file_change(modified_file, FileStatusKind::Modified, selection);

    let patch = format_patch_or_throw(&updated_file, &text_diff);

    assert!(patch.contains("--- a/modified-file.md\n"), "{patch}");
    assert!(patch.contains("+++ b/modified-file.md\n"), "{patch}");
    assert!(patch.contains("@@ -31,3 +31,8 @@"), "{patch}");
}

// GHD: unit/patch-formatter-test.ts › patch formatting › formatPatchesForModifiedFile › creates the right patch when an addition is selected but preceding deletions aren't
#[test]
fn creates_the_right_patch_when_an_addition_is_selected_but_preceding_deletions_arent() {
    let repository = setup_fixture_repository("repo-with-changes");

    let modified_file = "modified-file.md";
    std::fs::write(repository.join(modified_file), "line 1\n").unwrap();

    let unselected_file = DiffSelection::none();
    let file = file_change(modified_file, FileStatusKind::Modified, unselected_file);

    let diff = get_working_directory_diff(&repository, &file);

    let text_diff = text_hunks(diff);

    let mut selection = DiffSelection::all();
    let hunk = &text_diff[0];
    for (index, line) in hunk.lines.iter().enumerate() {
        let absolute_index = hunk.unified_diff_start + index as u32;
        // `line.text === '+line 1'`
        if line.kind == DiffLineKind::Add && line.text == "line 1" {
            selection = selection.with_line(absolute_index, true);
        } else {
            selection = selection.with_line(absolute_index, false);
        }
    }

    let updated_file = file_change(modified_file, FileStatusKind::Modified, selection);

    let patch = format_patch_or_throw(&updated_file, &text_diff);
    let expected_patch = concat!(
        "--- a/modified-file.md\n",
        "+++ b/modified-file.md\n",
        "@@ -1,33 +1,34 @@\n",
        " Lorem ipsum dolor sit amet, consectetur adipiscing elit. Cras mi urna,\n",
        " ullamcorper sit amet tellus eget, congue ornare leo. Donec dapibus sem quis sem\n",
        " commodo, id ultricies ligula varius. Vestibulum ante ipsum primis in faucibus\n",
        " orci luctus et ultrices posuere cubilia Curae; Maecenas efficitur lacus ac\n",
        " tortor placerat facilisis. Ut sed ex tortor. Duis consectetur at ex vel mattis.\n",
        " \n",
        " Aliquam leo ipsum, laoreet sed libero at, mollis pulvinar arcu. Nullam porttitor\n",
        " nisl eget hendrerit vestibulum. Curabitur ornare id neque ac tristique. Cras in\n",
        " eleifend mi.\n",
        " \n",
        " Donec sit amet posuere nibh, sed laoreet nisl. Pellentesque a consectetur\n",
        " turpis. Curabitur varius ex nisi, vitae vestibulum augue cursus sit amet. Morbi\n",
        " non vestibulum velit. Integer consectetur lacus vitae erat pellentesque\n",
        " tincidunt. Nullam id nunc rhoncus, ultrices orci bibendum, blandit orci. Morbi\n",
        " vitae accumsan metus, et cursus diam. Sed mi augue, sollicitudin imperdiet\n",
        " semper ac, scelerisque vitae nulla. Nam cursus est massa, et tincidunt lectus\n",
        " consequat vel. Nunc commodo elementum metus, vel pellentesque est efficitur sit\n",
        " amet. Aliquam rhoncus, diam vel pulvinar eleifend, massa tellus lobortis elit,\n",
        " quis cursus justo tellus vel magna. Quisque placerat nunc non nibh porttitor,\n",
        " vel sagittis nisl rutrum. Proin enim augue, condimentum sit amet suscipit id,\n",
        " tempor a ligula. Proin pretium ipsum vel nulla sollicitudin mollis. Morbi\n",
        " elementum neque id tellus gravida rhoncus.\n",
        " \n",
        " Ut fringilla, orci id consequat sodales, tellus tellus interdum risus, eleifend\n",
        " vestibulum velit nunc sit amet nulla. Ut tristique, diam ut rhoncus commodo,\n",
        " libero tellus maximus ex, vel rutrum mauris purus vel enim. Donec accumsan nulla\n",
        "  id purus lacinia venenatis. Phasellus convallis ex et vulputate aliquet. Ut\n",
        "  porttitor diam magna, vel porttitor tortor ornare et. Suspendisse eleifend\n",
        "  sagittis tempus. Pellentesque mollis dolor id lectus lobortis vulputate. Etiam\n",
        "  eu lacus sit amet mauris ornare dictum. Integer erat nisi, semper ut augue\n",
        "  vitae, cursus pulvinar lorem. Suspendisse potenti. Mauris eleifend elit ac\n",
        "  sodales posuere. Cras ultrices, ex in porta volutpat, libero sapien blandit\n",
        "  urna, ac porta justo leo sed magna.\n",
        "+line 1\n",
    );
    assert_eq!(patch, expected_patch);
}

// GHD: unit/patch-formatter-test.ts › patch formatting › formatPatchesForModifiedFile › doesn't include unselected added lines as context
#[test]
fn doesnt_include_unselected_added_lines_as_context() {
    let raw_diff = [
        "--- a/file.md",
        "+++ b/file.md",
        "@@ -10,2 +10,4 @@",
        " context",
        "+added line 1",
        "+added line 2",
        " context",
    ]
    .join("\n");

    let diff = parse_diff(&raw_diff);

    // Select the second added line
    let selection = DiffSelection::none().with_line(3, true);

    let file = file_change("file.md", FileStatusKind::Modified, selection);
    let patch = format_patch_or_throw(&file, &diff);

    assert_eq!(
        patch,
        "--- a/file.md\n+++ b/file.md\n@@ -10,2 +10,3 @@\n context\n+added line 2\n context\n"
    );
}

// GHD: unit/patch-formatter-test.ts › patch formatting › formatPatchesForModifiedFile › rewrites hunk header when necessary
#[test]
fn rewrites_hunk_header_when_necessary() {
    let raw_diff = [
        "--- /dev/null",
        "+++ b/file.md",
        "@@ -0,2 +1,2 @@",
        "+added line 1",
        "+added line 2",
    ]
    .join("\n");
    let diff = parse_diff(&raw_diff);

    // Select the second added line
    let selection = DiffSelection::none().with_line(2, true);

    let file = file_change("file.md", FileStatusKind::New, selection);
    let patch = format_patch_or_throw(&file, &diff);

    assert!(patch.contains("@@ -0,0 +1 @@"), "{patch}");
    assert!(patch.contains("+added line 2"), "{patch}");
}

// GHD: unit/patch-formatter-test.ts › patch formatting › formatPatchesForModifiedFile › includes empty context lines
#[test]
fn includes_empty_context_lines() {
    let raw_diff = [
        "--- a/file.md",
        "+++ b/file.md",
        "@@ -1 +1,2 @@",
        " ",
        "+added line 2",
    ]
    .join("\n");
    let diff = parse_diff(&raw_diff);

    // Select the second added line
    let selection = DiffSelection::none().with_line(2, true);

    let file = file_change("file.md", FileStatusKind::Modified, selection);
    let patch = format_patch_or_throw(&file, &diff);

    assert!(patch.contains("@@ -1 +1,2 @@"), "{patch}");
    assert!(patch.contains(' '), "{patch}");
    assert!(patch.contains("+added line 2"), "{patch}");
}

// GHD: unit/patch-formatter-test.ts › patch formatting › formatPatchesForModifiedFile › creates the right patch when a `No newline` marker is involved
#[test]
fn creates_the_right_patch_when_a_no_newline_marker_is_involved() {
    let raw_diff = [
        "--- a/file.md",
        "+++ b/file.md",
        "@@ -23,5 +24,5 @@ and more stuff",
        " ",
        " ",
        " ",
        "-",
        "-and fun stuff? I dnno",
        "\\ No newline at end of file",
        "+and fun stuff? I dnno",
        "+it could be,",
    ]
    .join("\n");
    let diff = parse_diff(&raw_diff);

    // Select the second added line
    let selection = DiffSelection::none().with_line(7, true);

    let file = file_change("file.md", FileStatusKind::Modified, selection);

    let patch = format_patch_or_throw(&file, &diff);

    assert!(patch.contains("\\ No newline at end of file"), "{patch}");
    assert!(patch.contains("+it could be"), "{patch}");
}
