//! Port of GitHub Desktop's `app/test/unit/git/apply-test.ts`.
//!
//! It lives in `corvene-ui` because `findInteractiveDiffRange` does: the
//! rest is `corvene-git`.
//!
//! Corvene equivalents:
//!
//! - `discardChangesFromSelection(repository, filePath, diff, selection)`
//!   (`lib/git/apply.ts`) is `corvene_git::format_patch_to_discard_changes`
//!   followed, unless the patch is empty, by
//!   `corvene_git::discard_changes_from_selection`, the two calls
//!   `Dispatcher::discard_selection` makes ([`discard_changes_from_selection`]).
//! - `getWorkingDirectoryDiff(repository, file)` (`lib/git/diff.ts`) is
//!   `corvene_git::working_directory_diff(git, path, file, false, false,
//!   false, None)`: whitespace shown, flags `743-renamed-diff-against-head`
//!   and `749-binary-diff-as-text` at their GitHub Desktop value (off), no
//!   cancel token (`764-cancel-stale-diffs`). The
//!   file is a modified `WorkingDirectoryFileChange` with nothing staged,
//!   which is what `modified-file.md` is in `repo-with-changes`.
//! - `findInteractiveDiffRange(hunks, index)` (`ui/diff/diff-explorer.ts`):
//!   Corvene's diff view computes the same block of consecutive added and
//!   removed lines for each row (`diff_view_rows::build_rows` over
//!   `diff_expansion::from_hunks(hunks, None)`, the unexpanded diff;
//!   `Row::group` is `(from, to - from + 1)`), see
//!   [`find_interactive_diff_range`].
//! - `diff.text` (the raw diff below its header) has no Corvene field;
//!   `Diff` holds the same text parsed (every hunk header and line), so
//!   `diff.text === previousDiff.text` compares the two `Diff`s, and a diff
//!   whose `text` is `''` with no hunks is `Diff::Empty`, Corvene's diff of
//!   an unchanged file.
//! - The test's `getDifference` uses jsdiff's `structuredPatch`, a test
//!   dependency of GitHub Desktop, ported in [`get_difference`].

use corvene_core::{
    Diff, DiffHunk, DiffSelection, FileStatus, FileStatusKind, GitStatusEntry,
    WorkingDirectoryFileChange,
};
use corvene_test_support::{TestRepo, git, setup_fixture_repository};
use corvene_ui::diff_expansion::from_hunks;
use corvene_ui::diff_view_rows::build_rows;

/// GitHub Desktop's `IDiffRange` (without its `type`).
struct DiffRange {
    from: u32,
    to: u32,
}

/// `findInteractiveDiffRange(hunks, index)`: the run of added / removed
/// lines around `index`, as Corvene's diff view groups them. GitHub
/// Desktop also returns a range around a context line, where Corvene's
/// rows have no group (`None` here); the cases only ask about changed
/// lines.
fn find_interactive_diff_range(hunks: &[DiffHunk], index: u32) -> Option<DiffRange> {
    let rows = build_rows(&from_hunks(hunks, None));
    let row = rows.iter().find(|row| row.original == Some(index))?;
    let (from, len) = row.group?;
    Some(DiffRange {
        from,
        to: from + len - 1,
    })
}

/// The describe's `getDiff(repository, filePath)`.
fn get_diff(repository: &TestRepo, file_path: &str) -> Diff {
    let file = WorkingDirectoryFileChange {
        path: file_path.to_string(),
        old_path: None,
        status: FileStatus {
            kind: FileStatusKind::Modified,
            index: GitStatusEntry::Unchanged,
            working_tree: GitStatusEntry::Modified,
            score: None,
            code: String::new(),
            submodule: false,
            submodule_status: None,
            conflict_markers: None,
        },
        selection: DiffSelection::none(),
    };
    corvene_git::working_directory_diff(git(), repository.path(), &file, false, false, false, None)
        .expect("getWorkingDirectoryDiff")
}

/// `diff.hunks` of the `ITextDiff` GitHub Desktop casts `getDiff`'s result
/// to.
fn hunks(diff: &Diff) -> &[DiffHunk] {
    diff.hunks()
        .unwrap_or_else(|| panic!("expected a text diff, got {diff:?}"))
}

/// `discardChangesFromSelection(repository, filePath, diff, selection)`.
fn discard_changes_from_selection(
    repository: &TestRepo,
    file_path: &str,
    diff: &Diff,
    selection: &DiffSelection,
) {
    let Some(patch) =
        corvene_git::format_patch_to_discard_changes(file_path, hunks(diff), selection)
    else {
        // When the patch is null we don't need to apply it since it will be a noop.
        return;
    };
    corvene_git::discard_changes_from_selection(git(), repository.path(), &patch)
        .expect("discardChangesFromSelection");
}

fn read_file(repository: &TestRepo, file_path: &str) -> String {
    std::fs::read_to_string(repository.join(file_path)).unwrap()
}

// GHD: unit/git/apply-test.ts › git/apply › discardChangesFromSelection() › does not change the file when an empty selection is passed
#[test]
fn does_not_change_the_file_when_an_empty_selection_is_passed() {
    let repository = setup_fixture_repository("repo-with-changes");

    let file_path = "modified-file.md";
    let previous_diff = get_diff(&repository, file_path);

    discard_changes_from_selection(
        &repository,
        file_path,
        &previous_diff,
        &DiffSelection::none(),
    );

    let diff = get_diff(&repository, file_path);

    // `diff.text === previousDiff.text`
    assert_eq!(diff, previous_diff);
}

// GHD: unit/git/apply-test.ts › git/apply › discardChangesFromSelection() › discards all file changes when a full selection is passed
#[test]
fn discards_all_file_changes_when_a_full_selection_is_passed() {
    let repository = setup_fixture_repository("repo-with-changes");

    let file_path = "modified-file.md";
    discard_changes_from_selection(
        &repository,
        file_path,
        &get_diff(&repository, file_path),
        &DiffSelection::all(),
    );

    let diff = get_diff(&repository, file_path);

    // Check that the file has no local changes.
    // `diff.text === ''`
    assert_eq!(diff, Diff::Empty);
    // `diff.hunks.length === 0`
    assert_eq!(diff.hunks().map_or(0, <[DiffHunk]>::len), 0);
}

// GHD: unit/git/apply-test.ts › git/apply › discardChangesFromSelection() › re-adds a single removed line
#[test]
fn re_adds_a_single_removed_line() {
    let repository = setup_fixture_repository("repo-with-changes");

    let file_path = "modified-file.md";
    let selection = DiffSelection::none().with_line(4, true);

    let previous_contents = read_file(&repository, file_path);

    discard_changes_from_selection(
        &repository,
        file_path,
        &get_diff(&repository, file_path),
        &selection,
    );

    let file_contents = read_file(&repository, file_path);

    assert_eq!(
        get_difference(&previous_contents, &file_contents),
        "@@ -7,0 +7,1 @@\n\
         +Aliquam leo ipsum, laoreet sed libero at, mollis pulvinar arcu. Nullam porttitor"
    );
}

// GHD: unit/git/apply-test.ts › git/apply › discardChangesFromSelection() › re-adds a removed hunk
#[test]
fn re_adds_a_removed_hunk() {
    let repository = setup_fixture_repository("repo-with-changes");

    let file_path = "modified-file.md";
    let diff = get_diff(&repository, file_path);
    let hunk_range = find_interactive_diff_range(hunks(&diff), 4);
    let hunk_range = hunk_range.expect("hunkRange !== null");

    let selection = DiffSelection::none().with_range(
        hunk_range.from,
        hunk_range.to - hunk_range.from + 1,
        true,
    );

    let previous_contents = read_file(&repository, file_path);

    discard_changes_from_selection(&repository, file_path, &diff, &selection);

    let file_contents = read_file(&repository, file_path);

    assert_eq!(
        get_difference(&previous_contents, &file_contents),
        "@@ -7,0 +7,4 @@\n\
         +Aliquam leo ipsum, laoreet sed libero at, mollis pulvinar arcu. Nullam porttitor\n\
         +nisl eget hendrerit vestibulum. Curabitur ornare id neque ac tristique. Cras in\n\
         +eleifend mi.\n\
         +"
    );
}

// GHD: unit/git/apply-test.ts › git/apply › discardChangesFromSelection() › removes an added line
#[test]
fn removes_an_added_line() {
    let repository = setup_fixture_repository("repo-with-changes");

    let file_path = "modified-file.md";
    let selection = DiffSelection::none().with_line(16, true);

    let previous_contents = read_file(&repository, file_path);

    discard_changes_from_selection(
        &repository,
        file_path,
        &get_diff(&repository, file_path),
        &selection,
    );

    let file_contents = read_file(&repository, file_path);

    assert_eq!(
        get_difference(&previous_contents, &file_contents),
        "@@ -21,1 +21,0 @@\n\
         -nisl eget hendrerit vestibulum. Curabitur ornare id neque ac tristique. Cras in"
    );
}

// GHD: unit/git/apply-test.ts › git/apply › discardChangesFromSelection() › removes an added hunk
#[test]
fn removes_an_added_hunk() {
    let repository = setup_fixture_repository("repo-with-changes");

    let file_path = "modified-file.md";
    let diff = get_diff(&repository, file_path);
    let hunk_range = find_interactive_diff_range(hunks(&diff), 16);
    let hunk_range = hunk_range.expect("hunkRange !== null");
    let selection = DiffSelection::none().with_range(
        hunk_range.from,
        hunk_range.to - hunk_range.from + 1,
        true,
    );

    let previous_contents = read_file(&repository, file_path);

    discard_changes_from_selection(
        &repository,
        file_path,
        &get_diff(&repository, file_path),
        &selection,
    );

    let file_contents = read_file(&repository, file_path);

    assert_eq!(
        get_difference(&previous_contents, &file_contents),
        "@@ -20,4 +20,0 @@\n\
         -Aliquam leo ipsum, laoreet sed libero at, mollis pulvinar arcu. Nullam porttitor\n\
         -nisl eget hendrerit vestibulum. Curabitur ornare id neque ac tristique. Cras in\n\
         -eleifend mi.\n\
         -"
    );
}

// ---- getDifference: jsdiff 8's `structuredPatch` with `{ context: 0 }` ----

/// A run of the edit script (jsdiff's change object without its value).
#[derive(Clone, Copy, Debug)]
struct Component {
    count: usize,
    added: bool,
    removed: bool,
}

/// A path of jsdiff's Myers search: the last old position reached and the
/// runs that led there.
#[derive(Clone, Debug)]
struct SearchPath {
    old_pos: isize,
    components: Vec<Component>,
}

/// jsdiff's `addToPath`: one more added or removed token.
fn add_to_path(path: &SearchPath, added: bool, removed: bool, old_pos_inc: isize) -> SearchPath {
    let mut components = path.components.clone();
    match components.last_mut() {
        Some(last) if last.added == added && last.removed == removed => last.count += 1,
        _ => components.push(Component {
            count: 1,
            added,
            removed,
        }),
    }
    SearchPath {
        old_pos: path.old_pos + old_pos_inc,
        components,
    }
}

/// jsdiff's `extractCommon`: follow the diagonal while tokens match;
/// returns the new position.
fn extract_common(path: &mut SearchPath, new: &[&str], old: &[&str], diagonal: isize) -> isize {
    let (new_len, old_len) = (new.len() as isize, old.len() as isize);
    let mut old_pos = path.old_pos;
    let mut new_pos = old_pos - diagonal;
    let mut common = 0;
    while new_pos + 1 < new_len
        && old_pos + 1 < old_len
        && old[(old_pos + 1) as usize] == new[(new_pos + 1) as usize]
    {
        new_pos += 1;
        old_pos += 1;
        common += 1;
    }
    if common > 0 {
        path.components.push(Component {
            count: common,
            added: false,
            removed: false,
        });
    }
    path.old_pos = old_pos;
    new_pos
}

/// jsdiff's `Diff.diff` (Myers, as jsdiff explores it) over line tokens.
fn diff_tokens(old: &[&str], new: &[&str]) -> Vec<Component> {
    use std::collections::HashMap;

    let (new_len, old_len) = (new.len() as isize, old.len() as isize);
    let mut first = SearchPath {
        old_pos: -1,
        components: Vec::new(),
    };
    let new_pos = extract_common(&mut first, new, old, 0);
    if first.old_pos + 1 >= old_len && new_pos + 1 >= new_len {
        return first.components;
    }
    let mut best: HashMap<isize, SearchPath> = HashMap::new();
    best.insert(0, first);
    let (mut min_diagonal, mut max_diagonal) = (isize::MIN, isize::MAX);
    for edit_length in 1..=(new_len + old_len) {
        let mut diagonal = min_diagonal.max(-edit_length);
        while diagonal <= max_diagonal.min(edit_length) {
            let remove_path = best.remove(&(diagonal - 1));
            let add_path = best.get(&(diagonal + 1)).cloned();
            let can_add = add_path.as_ref().is_some_and(|p| {
                let add_path_new_pos = p.old_pos - diagonal;
                0 <= add_path_new_pos && add_path_new_pos < new_len
            });
            let can_remove = remove_path
                .as_ref()
                .is_some_and(|p| p.old_pos + 1 < old_len);
            if !can_add && !can_remove {
                best.remove(&diagonal);
                diagonal += 2;
                continue;
            }
            let take_add = match (&remove_path, &add_path) {
                _ if !can_remove => true,
                (Some(remove), Some(add)) => can_add && remove.old_pos < add.old_pos,
                _ => false,
            };
            let mut base = if take_add {
                add_to_path(add_path.as_ref().expect("can_add"), true, false, 0)
            } else {
                add_to_path(remove_path.as_ref().expect("can_remove"), false, true, 1)
            };
            let new_pos = extract_common(&mut base, new, old, diagonal);
            if base.old_pos + 1 >= old_len && new_pos + 1 >= new_len {
                return base.components;
            }
            if base.old_pos + 1 >= old_len {
                max_diagonal = max_diagonal.min(diagonal - 1);
            }
            if new_pos + 1 >= new_len {
                min_diagonal = min_diagonal.max(diagonal + 1);
            }
            best.insert(diagonal, base);
            diagonal += 2;
        }
    }
    unreachable!("an edit script never exceeds old + new tokens")
}

/// The test's `getDifference(before, after)`: `structuredPatch('before',
/// 'after', …, { context: 0 })` of the two texts with CRLF folded to LF,
/// each hunk as `@@ -oldStart,oldLines +newStart,newLines @@` and its
/// lines, joined by `\n`.
fn get_difference(before: &str, after: &str) -> String {
    let before = before.replace("\r\n", "\n");
    let after = after.replace("\r\n", "\n");
    // jsdiff's line tokenizer: each line with its newline
    let old: Vec<&str> = before.split_inclusive('\n').collect();
    let new: Vec<&str> = after.split_inclusive('\n').collect();

    struct Hunk {
        old_start: usize,
        old_lines: usize,
        new_start: usize,
        new_lines: usize,
        lines: Vec<String>,
    }

    let mut hunks: Vec<Hunk> = Vec::new();
    let (mut old_range_start, mut new_range_start) = (0usize, 0usize);
    let mut cur_range: Vec<String> = Vec::new();
    let (mut old_line, mut new_line) = (1usize, 1usize);
    let (mut old_pos, mut new_pos) = (0usize, 0usize);
    let mut components = diff_tokens(&old, &new);
    // jsdiff appends an empty value to make cleanup easier
    components.push(Component {
        count: 0,
        added: false,
        removed: false,
    });
    for component in components {
        let lines: &[&str] = if component.removed {
            let lines = &old[old_pos..old_pos + component.count];
            old_pos += component.count;
            lines
        } else {
            let lines = &new[new_pos..new_pos + component.count];
            new_pos += component.count;
            if !component.added {
                old_pos += component.count;
            }
            lines
        };
        if component.added || component.removed {
            if old_range_start == 0 {
                // no previous context with `context: 0`
                old_range_start = old_line;
                new_range_start = new_line;
            }
            let prefix = if component.added { '+' } else { '-' };
            cur_range.extend(lines.iter().map(|line| format!("{prefix}{line}")));
            if component.added {
                new_line += lines.len();
            } else {
                old_line += lines.len();
            }
        } else {
            if old_range_start != 0 {
                // with `context: 0` every run of identical lines ends a hunk
                hunks.push(Hunk {
                    old_start: old_range_start,
                    old_lines: old_line - old_range_start,
                    new_start: new_range_start,
                    new_lines: new_line - new_range_start,
                    lines: std::mem::take(&mut cur_range),
                });
                old_range_start = 0;
                new_range_start = 0;
            }
            old_line += lines.len();
            new_line += lines.len();
        }
    }
    // the newline of each line goes, a line without one gets jsdiff's marker
    for hunk in &mut hunks {
        let mut lines = Vec::with_capacity(hunk.lines.len());
        for line in hunk.lines.drain(..) {
            match line.strip_suffix('\n') {
                Some(stripped) => lines.push(stripped.to_string()),
                None => {
                    lines.push(line);
                    lines.push("\\ No newline at end of file".to_string());
                }
            }
        }
        hunk.lines = lines;
    }
    hunks
        .iter()
        .flat_map(|hunk| {
            std::iter::once(format!(
                "@@ -{},{} +{},{} @@",
                hunk.old_start, hunk.old_lines, hunk.new_start, hunk.new_lines
            ))
            .chain(hunk.lines.iter().cloned())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// [`get_difference`] against outputs of jsdiff 8.0.4's `structuredPatch`.
#[test]
fn get_difference_matches_structured_patch() {
    assert_eq!(get_difference("a\nb\n", "a\nb\n"), "");
    assert_eq!(
        get_difference("x\ny\n", "p\nq\nr\n"),
        "@@ -1,2 +1,3 @@\n-x\n-y\n+p\n+q\n+r"
    );
    assert_eq!(
        get_difference("a\n\nb\n", "a\n\nX\n\nb\n"),
        "@@ -3,0 +3,2 @@\n+X\n+"
    );
    assert_eq!(get_difference("a\nc\n", "a\nb\nc\n"), "@@ -2,0 +2,1 @@\n+b");
    assert_eq!(get_difference("a\nb\nc\n", "a\nc\n"), "@@ -2,1 +2,0 @@\n-b");
    assert_eq!(
        get_difference("a\nb\nc\n", "a\nB\nc\nd"),
        "@@ -2,1 +2,1 @@\n-b\n+B\n@@ -4,0 +4,1 @@\n+d\n\\ No newline at end of file"
    );
}
