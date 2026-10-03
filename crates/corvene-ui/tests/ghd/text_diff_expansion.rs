//! Port of GitHub Desktop's `app/test/unit/text-diff-expansion-test.ts`.
//!
//! GitHub Desktop's `ui/diff/text-diff-expansion.ts` is Corvene's
//! `corvene_ui::diff_expansion`, which works on a list of expandable hunks
//! (`XHunk`, GitHub Desktop's `DiffHunk` with its `expansionType`) instead
//! of a whole `ITextDiff`:
//!
//! - `new DiffParser().parse(text)` is `corvene_git::parse_raw_diff`,
//! - `getTextDiffWithBottomDummyHunk(diff, hunks, numberOfOldLines,
//!   numberOfNewLines) ?? diff` is `from_hunks(hunks,
//!   Some(numberOfNewLines))`: the parsed hunks with their expansion types
//!   (GitHub Desktop's parser sets them) plus the dummy hunk when needed.
//!   Corvene derives the old line count from the hunks instead of taking
//!   `numberOfOldLines`,
//! - `expandTextDiffHunk(diff, diff.hunks[i], kind, newContentLines)` is
//!   `expand_hunk(&hunks, i, kind, &newContentLines,
//!   DEFAULT_DIFF_EXPANSION_STEP)` (GitHub Desktop's default `step`),
//! - `expandWholeTextDiff(diff, newContentLines)` is `expand_whole`,
//! - `hunk.header.{old,new}{StartLine,LineCount}` are `old_start`,
//!   `old_lines`, `new_start`, `new_lines`; a line's `type`,
//!   `oldLineNumber` and `newLineNumber` are `line.kind`, `line.old_line`
//!   and `line.new_line`; the `text` of a hunk header line is `line.text`
//!   (Corvene keeps the whole header text there, as GitHub Desktop does).

use std::ffi::OsString;

use corvene_core::DiffLineKind;
use corvene_git::parse_raw_diff;
use corvene_test_support::{create_temp_directory, exec};
use corvene_ui::diff_expansion::{
    DEFAULT_DIFF_EXPANSION_STEP, ExpansionKind, XHunk, expand_hunk, expand_whole, from_hunks,
};

/// GitHub Desktop's `ITestDiffInfo`.
struct TestDiffInfo {
    text_diff: Vec<XHunk>,
    new_content_lines: Vec<String>,
}

/// GitHub Desktop's `prepareDiff(numberOfLines, linesChanged)`.
fn prepare_diff(number_of_lines: usize, lines_changed: &[usize]) -> TestDiffInfo {
    let mut text_lines: Vec<String> = (0..number_of_lines)
        .map(|value| value.to_string())
        .collect();
    let original_contents = text_lines.join("\n");
    for &line in lines_changed {
        text_lines.insert(line, "added line".to_string());
    }
    let modified_contents = text_lines.join("\n");

    let content_folder = create_temp_directory();
    let content_folder_path = content_folder.path();

    std::fs::write(content_folder_path.join("original"), original_contents).unwrap();
    std::fs::write(content_folder_path.join("changed"), modified_contents).unwrap();

    // Generate diff with 3 lines of context
    let result = exec(
        [
            OsString::from("diff"),
            OsString::from("-U3"),
            content_folder_path.join("original").into_os_string(),
            content_folder_path.join("changed").into_os_string(),
        ],
        content_folder_path,
    );

    let diff = parse_raw_diff(result.stdout.as_bytes());
    // `IRawDiff.hunks`: none for Corvene's `Diff::Empty` / `Diff::Binary`
    let hunks = diff.hunks().unwrap_or(&[]);

    let result_diff = from_hunks(hunks, Some(number_of_lines + lines_changed.len()));

    TestDiffInfo {
        text_diff: result_diff,
        new_content_lines: text_lines,
    }
}

// GHD: unit/text-diff-expansion-test.ts › text-diff-expansion › adds a dummy hunk to the bottom to allow expansion when last hunk does not reach bottom
#[test]
fn adds_a_dummy_hunk_to_the_bottom_to_allow_expansion_when_last_hunk_does_not_reach_bottom() {
    let TestDiffInfo { text_diff, .. } = prepare_diff(100, &[30]);

    let last_hunk = &text_diff[text_diff.len() - 1];
    assert_eq!(last_hunk.lines.len(), 1);

    let first_line = &last_hunk.lines[0].line;
    assert_eq!(first_line.kind, DiffLineKind::Hunk);
    assert_eq!(first_line.text, "");
    assert_eq!(first_line.new_line, None);
    assert_eq!(first_line.old_line, None);
}

// GHD: unit/text-diff-expansion-test.ts › text-diff-expansion › does not add a dummy hunk to the bottom when last hunk reaches bottom
#[test]
fn does_not_add_a_dummy_hunk_to_the_bottom_when_last_hunk_reaches_bottom() {
    let TestDiffInfo { text_diff, .. } = prepare_diff(100, &[99]);
    let last_hunk = text_diff.last();
    assert!(last_hunk.is_some());
    let last_hunk = last_hunk.unwrap();
    assert_eq!(last_hunk.lines.len(), 6);
}

// GHD: unit/text-diff-expansion-test.ts › text-diff-expansion › expands the initial hunk without reaching the top
#[test]
fn expands_the_initial_hunk_without_reaching_the_top() {
    let TestDiffInfo {
        text_diff,
        new_content_lines,
    } = prepare_diff(100, &[30]);
    let expanded_diff = expand_hunk(
        &text_diff,
        0,
        ExpansionKind::Up,
        &new_content_lines,
        DEFAULT_DIFF_EXPANSION_STEP,
    );

    assert!(expanded_diff.is_some());
    let expanded_diff = expanded_diff.unwrap();

    let first_hunk = &expanded_diff[0];
    assert_eq!(first_hunk.old_start, 8);
    assert_eq!(first_hunk.old_lines, 26);
    assert_eq!(first_hunk.new_start, 8);
    assert_eq!(first_hunk.new_lines, 27);

    // Check the first line is still the header info
    assert_eq!(first_hunk.lines[0].line.kind, DiffLineKind::Hunk);
}

// GHD: unit/text-diff-expansion-test.ts › text-diff-expansion › expands the initial hunk reaching the top
#[test]
fn expands_the_initial_hunk_reaching_the_top() {
    let TestDiffInfo {
        text_diff,
        new_content_lines,
    } = prepare_diff(100, &[15]);
    let expanded_diff = expand_hunk(
        &text_diff,
        0,
        ExpansionKind::Up,
        &new_content_lines,
        DEFAULT_DIFF_EXPANSION_STEP,
    );

    assert!(expanded_diff.is_some());
    let expanded_diff = expanded_diff.unwrap();

    let first_hunk = &expanded_diff[0];
    assert_eq!(first_hunk.old_start, 1);
    assert_eq!(first_hunk.old_lines, 18);
    assert_eq!(first_hunk.new_start, 1);
    assert_eq!(first_hunk.new_lines, 19);

    // Check the first line is still the header info
    assert_eq!(first_hunk.lines[0].line.kind, DiffLineKind::Hunk);
}

// The last hunk is a dummy hunk to expand the bottom of the diff
// GHD: unit/text-diff-expansion-test.ts › text-diff-expansion › expands the second-to-last hunk without reaching the bottom
#[test]
fn expands_the_second_to_last_hunk_without_reaching_the_bottom() {
    let TestDiffInfo {
        text_diff,
        new_content_lines,
    } = prepare_diff(100, &[15]);
    let expanded_diff = expand_hunk(
        &text_diff,
        text_diff.len() - 2,
        ExpansionKind::Down,
        &new_content_lines,
        DEFAULT_DIFF_EXPANSION_STEP,
    );

    assert!(expanded_diff.is_some());
    let expanded_diff = expanded_diff.unwrap();

    let second_to_last_hunk = expanded_diff
        .len()
        .checked_sub(2)
        .map(|i| &expanded_diff[i]);
    assert!(second_to_last_hunk.is_some());
    let second_to_last_hunk = second_to_last_hunk.unwrap();

    assert_eq!(second_to_last_hunk.old_start, 13);
    assert_eq!(second_to_last_hunk.old_lines, 26);
    assert_eq!(second_to_last_hunk.new_start, 13);
    assert_eq!(second_to_last_hunk.new_lines, 27);
}

// GHD: unit/text-diff-expansion-test.ts › text-diff-expansion › expands the second-to-last hunk reaching the bottom
#[test]
fn expands_the_second_to_last_hunk_reaching_the_bottom() {
    let TestDiffInfo {
        text_diff,
        new_content_lines,
    } = prepare_diff(100, &[90]);
    let expanded_diff = expand_hunk(
        &text_diff,
        text_diff.len() - 2,
        ExpansionKind::Down,
        &new_content_lines,
        DEFAULT_DIFF_EXPANSION_STEP,
    );
    assert!(expanded_diff.is_some());
    let expanded_diff = expanded_diff.unwrap();

    let last_hunk = expanded_diff.last();
    assert!(last_hunk.is_some());
    let last_hunk = last_hunk.unwrap();

    assert_eq!(last_hunk.old_start, 88);
    assert_eq!(last_hunk.old_lines, 13);
    assert_eq!(last_hunk.new_start, 88);
    assert_eq!(last_hunk.new_lines, 14);
}

// GHD: unit/text-diff-expansion-test.ts › text-diff-expansion › merges hunks when the gap between them is shorter than the expansion size
#[test]
fn merges_hunks_when_the_gap_between_them_is_shorter_than_the_expansion_size() {
    let TestDiffInfo {
        text_diff,
        new_content_lines,
    } = prepare_diff(100, &[20, 10]);
    let expanded_diff = expand_hunk(
        &text_diff,
        0,
        ExpansionKind::Down,
        &new_content_lines,
        DEFAULT_DIFF_EXPANSION_STEP,
    );

    // Originally 3 hunks:
    // - First around line 10
    // - Second around line 20
    // - Third is the dummy hunk at the end
    assert_eq!(text_diff.len(), 3);

    assert!(expanded_diff.is_some());
    let expanded_diff = expanded_diff.unwrap();

    // After expanding the hunk, the first two hunks are merged
    assert_eq!(expanded_diff.len(), 2);

    let first_hunk = &expanded_diff[0];
    assert_eq!(first_hunk.old_start, 8);
    assert_eq!(first_hunk.old_lines, 16);
    assert_eq!(first_hunk.new_start, 8);
    assert_eq!(first_hunk.new_lines, 18);
}

// GHD: unit/text-diff-expansion-test.ts › text-diff-expansion › expands the whole file
#[test]
fn expands_the_whole_file() {
    let TestDiffInfo {
        text_diff,
        new_content_lines,
    } = prepare_diff(35, &[20, 17, 8, 7, 6]);

    let expanded_diff = expand_whole(text_diff, &new_content_lines);
    assert!(expanded_diff.is_some());
    let expanded_diff = expanded_diff.unwrap();
    assert_eq!(expanded_diff.len(), 1);

    let first_hunk = &expanded_diff[0];
    assert_eq!(first_hunk.lines.len(), 40 + 1); // +1 for the header

    let mut expected_new_line = 1;
    let mut expected_old_line = 1;

    // Make sure line numbers are consecutive as expected
    for line in first_hunk.lines.iter().map(|l| &l.line) {
        if line.kind == DiffLineKind::Add {
            assert_eq!(line.new_line, Some(expected_new_line));
            expected_new_line += 1;
        } else if line.kind == DiffLineKind::Delete {
            assert_eq!(line.old_line, Some(expected_old_line));
            expected_old_line += 1;
        } else if line.kind == DiffLineKind::Context {
            assert_eq!(line.new_line, Some(expected_new_line));
            expected_new_line += 1;
            assert_eq!(line.old_line, Some(expected_old_line));
            expected_old_line += 1;
        }
    }
}
