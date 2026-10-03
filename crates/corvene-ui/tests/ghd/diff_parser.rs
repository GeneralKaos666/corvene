//! Port of GitHub Desktop's `app/test/unit/diff-parser-test.ts`.
//!
//! GitHub Desktop's `new DiffParser().parse(text)` (`lib/diff-parser.ts`)
//! is Corvene's `corvene_git::parse_raw_diff` (the parser every diff goes
//! through: it skips the header up to the first hunk, detects binary
//! patches and parses the hunks with `parse_unified`) followed by
//! `corvene_ui::diff_expansion::from_hunks(hunks, None)`. GitHub Desktop's
//! parser hands out hunks that carry `unifiedDiffEnd` and lines that carry
//! `originalLineNumber`; Corvene's parser keeps only each hunk's
//! `unified_diff_start` and its lines, and `from_hunks` (what the diff view
//! builds from every parsed diff) turns them into `XHunk`s, whose
//! `unified_diff_end()` and `XLine::original` are those two values. Without
//! file contents (`None`) `from_hunks` maps the hunks one to one and adds no
//! expansion dummy hunk. That is why this file is a `corvene-ui` test.
//!
//! How GitHub Desktop's `IRawDiff`, `DiffHunk` and `DiffLine` fields are
//! read ([`parse`]):
//!
//! - `diff.hunks` is [`RawDiff::hunks`] (none for `Diff::Empty` /
//!   `Diff::Binary`), `diff.isBinary` is [`RawDiff::is_binary`],
//! - `hunk.header.{old,new}{StartLine,LineCount}` are `old_start`,
//!   `old_lines`, `new_start`, `new_lines`; `hunk.unifiedDiffStart` and
//!   `hunk.unifiedDiffEnd` are `unified_diff_start` and `unified_diff_end()`,
//! - `line.text` is [`ghd_text`]: GitHub Desktop's `DiffLine.text` is the
//!   line as the unified diff prints it, marker included (`+foo`), while
//!   Corvene's `DiffLine.text` is the text after the marker (GitHub
//!   Desktop's `DiffLine.content`) and the marker is its `kind`; a hunk
//!   header line keeps its whole text in both,
//! - `line.type`, `line.oldLineNumber`, `line.newLineNumber` and
//!   `line.noTrailingNewLine` are the `XLine`'s `line.kind`, `line.old_line`,
//!   `line.new_line` and `line.no_trailing_newline`; `line.originalLineNumber`
//!   is `XLine::original`.

use corvene_core::{Diff, DiffLineKind};
use corvene_git::parse_raw_diff;
use corvene_test_support::ghd_text;
use corvene_ui::diff_expansion::{XHunk, from_hunks};

/// GitHub Desktop's `IRawDiff`, as these tests read it.
struct RawDiff {
    diff: Diff,
    hunks: Vec<XHunk>,
}

impl RawDiff {
    /// `IRawDiff.isBinary`.
    fn is_binary(&self) -> bool {
        self.diff == Diff::Binary
    }
}

/// `new DiffParser().parse(text)`.
fn parse(text: &str) -> RawDiff {
    let diff = parse_raw_diff(text.as_bytes());
    // `IRawDiff.hunks`: none for Corvene's `Diff::Empty` / `Diff::Binary`
    let hunks = from_hunks(diff.hunks().unwrap_or(&[]), None);
    RawDiff { diff, hunks }
}

// Atom doesn't like lines with just one space and tries to
// de-indent so when copying- and pasting diff contents the
// space signalling that the line is a context line gets lost.
//
// This function reinstates that space and makes us all
// feel a little bit sad.
fn reinstate_spaces_at_the_start_of_blank_lines(text: &str) -> String {
    text.replace("\n\n", "\n \n")
}

// GHD: unit/diff-parser-test.ts › DiffParser › parses changed files
#[test]
fn parses_changed_files() {
    let diff_text = r#"diff --git a/app/src/lib/diff-parser.ts b/app/src/lib/diff-parser.ts
index e1d4871..3bd3ee0 100644
--- a/app/src/lib/diff-parser.ts
+++ b/app/src/lib/diff-parser.ts
@@ -18,6 +18,7 @@ export function parseRawDiff(lines: ReadonlyArray<string>): Diff {

     let numberOfUnifiedDiffLines = 0

+
     while (prefixFound) {

       // trim any preceding text
@@ -71,12 +72,9 @@ export function parseRawDiff(lines: ReadonlyArray<string>): Diff {
         diffSections.push(new DiffSection(range, diffLines, startDiffSection, endDiffSection))
       } else {
         const diffBody = diffTextBuffer
-
         let startDiffSection: number = 0
         let endDiffSection: number = 0
-
         const diffLines = diffBody.split('\n')
-
         if (diffSections.length === 0) {
           startDiffSection = 0
           endDiffSection = diffLines.length
@@ -84,10 +82,8 @@ export function parseRawDiff(lines: ReadonlyArray<string>): Diff {
           startDiffSection = numberOfUnifiedDiffLines
           endDiffSection = startDiffSection + diffLines.length
         }
-
         diffSections.push(new DiffSection(range, diffLines, startDiffSection, endDiffSection))
       }
     }
-
     return new Diff(diffSections)
 }
    "#;

    let diff = parse(&reinstate_spaces_at_the_start_of_blank_lines(diff_text));
    assert_eq!(diff.hunks.len(), 3);

    let mut hunk = &diff.hunks[0];
    assert_eq!(hunk.unified_diff_start, 0);
    assert_eq!(hunk.unified_diff_end(), 7);

    let mut lines = &hunk.lines;
    assert_eq!(lines.len(), 8);

    let mut i = 0;
    assert_eq!(
        ghd_text(&lines[i].line),
        "@@ -18,6 +18,7 @@ export function parseRawDiff(lines: ReadonlyArray<string>): Diff {"
    );
    assert_eq!(lines[i].line.kind, DiffLineKind::Hunk);
    assert_eq!(lines[i].line.old_line, None);
    assert_eq!(lines[i].line.new_line, None);
    i += 1;

    assert_eq!(ghd_text(&lines[i].line), " ");
    assert_eq!(lines[i].line.kind, DiffLineKind::Context);
    assert_eq!(lines[i].line.old_line, Some(18));
    assert_eq!(lines[i].line.new_line, Some(18));
    i += 1;

    assert_eq!(
        ghd_text(&lines[i].line),
        "     let numberOfUnifiedDiffLines = 0"
    );
    assert_eq!(lines[i].line.kind, DiffLineKind::Context);
    assert_eq!(lines[i].line.old_line, Some(19));
    assert_eq!(lines[i].line.new_line, Some(19));
    i += 1;

    assert_eq!(ghd_text(&lines[i].line), " ");
    assert_eq!(lines[i].line.kind, DiffLineKind::Context);
    assert_eq!(lines[i].line.old_line, Some(20));
    assert_eq!(lines[i].line.new_line, Some(20));
    i += 1;

    assert_eq!(ghd_text(&lines[i].line), "+");
    assert_eq!(lines[i].line.kind, DiffLineKind::Add);
    assert_eq!(lines[i].line.old_line, None);
    assert_eq!(lines[i].line.new_line, Some(21));
    i += 1;

    assert_eq!(ghd_text(&lines[i].line), "     while (prefixFound) {");
    assert_eq!(lines[i].line.kind, DiffLineKind::Context);
    assert_eq!(lines[i].line.old_line, Some(21));
    assert_eq!(lines[i].line.new_line, Some(22));

    hunk = &diff.hunks[1];
    assert_eq!(hunk.unified_diff_start, 8);
    assert_eq!(hunk.unified_diff_end(), 20);

    lines = &hunk.lines;
    assert_eq!(lines.len(), 13);
}

// GHD: unit/diff-parser-test.ts › DiffParser › parses new files
#[test]
fn parses_new_files() {
    let diff_text = r#"diff --git a/testste b/testste
new file mode 100644
index 0000000..f13588b
--- /dev/null
+++ b/testste
@@ -0,0 +1 @@
+asdfasdf
"#;

    let diff = parse(diff_text);
    assert_eq!(diff.hunks.len(), 1);

    let hunk = &diff.hunks[0];
    assert_eq!(hunk.unified_diff_start, 0);
    assert_eq!(hunk.unified_diff_end(), 1);

    let lines = &hunk.lines;
    assert_eq!(lines.len(), 2);

    let mut i = 0;
    assert_eq!(ghd_text(&lines[i].line), "@@ -0,0 +1 @@");
    assert_eq!(lines[i].line.kind, DiffLineKind::Hunk);
    assert_eq!(lines[i].line.old_line, None);
    assert_eq!(lines[i].line.new_line, None);
    i += 1;

    assert_eq!(ghd_text(&lines[i].line), "+asdfasdf");
    assert_eq!(lines[i].line.kind, DiffLineKind::Add);
    assert_eq!(lines[i].line.old_line, None);
    assert_eq!(lines[i].line.new_line, Some(1));
}

// GHD: unit/diff-parser-test.ts › DiffParser › parses files containing @@
#[test]
fn parses_files_containing_at_at() {
    let diff_text = r#"diff --git a/test.txt b/test.txt
index 24219cc..bf711a5 100644
--- a/test.txt
+++ b/test.txt
@@ -1 +1 @@
-foo @@
+@@ foo
"#;

    let diff = parse(diff_text);
    assert_eq!(diff.hunks.len(), 1);

    let hunk = &diff.hunks[0];
    assert_eq!(hunk.unified_diff_start, 0);
    assert_eq!(hunk.unified_diff_end(), 2);

    let lines = &hunk.lines;
    assert_eq!(lines.len(), 3);

    let mut i = 0;
    assert_eq!(ghd_text(&lines[i].line), "@@ -1 +1 @@");
    assert_eq!(lines[i].line.kind, DiffLineKind::Hunk);
    assert_eq!(lines[i].line.old_line, None);
    assert_eq!(lines[i].line.new_line, None);
    i += 1;

    assert_eq!(ghd_text(&lines[i].line), "-foo @@");
    assert_eq!(lines[i].line.kind, DiffLineKind::Delete);
    assert_eq!(lines[i].line.old_line, Some(1));
    assert_eq!(lines[i].line.new_line, None);
    i += 1;

    assert_eq!(ghd_text(&lines[i].line), "+@@ foo");
    assert_eq!(lines[i].line.kind, DiffLineKind::Add);
    assert_eq!(lines[i].line.old_line, None);
    assert_eq!(lines[i].line.new_line, Some(1));
}

// GHD: unit/diff-parser-test.ts › DiffParser › parses new files without a newline at end of file
#[test]
fn parses_new_files_without_a_newline_at_end_of_file() {
    let diff_text = r#"diff --git a/test2.txt b/test2.txt
new file mode 100644
index 0000000..faf7da1
--- /dev/null
+++ b/test2.txt
@@ -0,0 +1 @@
+asdasdasd
\ No newline at end of file
"#;

    let diff = parse(diff_text);
    assert_eq!(diff.hunks.len(), 1);

    let hunk = &diff.hunks[0];
    assert_eq!(hunk.unified_diff_start, 0);
    assert_eq!(hunk.unified_diff_end(), 1);

    let lines = &hunk.lines;
    assert_eq!(lines.len(), 2);

    let mut i = 0;
    assert_eq!(ghd_text(&lines[i].line), "@@ -0,0 +1 @@");
    assert_eq!(lines[i].line.kind, DiffLineKind::Hunk);
    assert_eq!(lines[i].line.old_line, None);
    assert_eq!(lines[i].line.new_line, None);
    assert!(!lines[i].line.no_trailing_newline);
    i += 1;

    assert_eq!(ghd_text(&lines[i].line), "+asdasdasd");
    assert_eq!(lines[i].line.kind, DiffLineKind::Add);
    assert_eq!(lines[i].line.old_line, None);
    assert_eq!(lines[i].line.new_line, Some(1));
    assert!(lines[i].line.no_trailing_newline);
}

// GHD: unit/diff-parser-test.ts › DiffParser › parses diffs that adds newline to end of file
#[test]
fn parses_diffs_that_adds_newline_to_end_of_file() {
    let diff_text = r#"diff --git a/test2.txt b/test2.txt
index 1910281..257cc56 100644
--- a/test2.txt
+++ b/test2.txt
@@ -1 +1 @@
-foo
\ No newline at end of file
+foo
"#;
    let diff = parse(diff_text);
    assert_eq!(diff.hunks.len(), 1);

    let hunk = &diff.hunks[0];
    assert_eq!(hunk.unified_diff_start, 0);
    assert_eq!(hunk.unified_diff_end(), 2);

    let lines = &hunk.lines;
    assert_eq!(lines.len(), 3);

    let mut i = 0;
    assert_eq!(ghd_text(&lines[i].line), "@@ -1 +1 @@");
    assert_eq!(lines[i].line.kind, DiffLineKind::Hunk);
    assert_eq!(lines[i].line.old_line, None);
    assert_eq!(lines[i].line.new_line, None);
    assert!(!lines[i].line.no_trailing_newline);
    i += 1;

    assert_eq!(ghd_text(&lines[i].line), "-foo");
    assert_eq!(lines[i].line.kind, DiffLineKind::Delete);
    assert_eq!(lines[i].line.old_line, Some(1));
    assert_eq!(lines[i].line.new_line, None);
    assert_eq!(lines[i].original, Some(1));
    assert!(lines[i].line.no_trailing_newline);
    i += 1;

    assert_eq!(ghd_text(&lines[i].line), "+foo");
    assert_eq!(lines[i].line.kind, DiffLineKind::Add);
    assert_eq!(lines[i].line.old_line, None);
    assert_eq!(lines[i].line.new_line, Some(1));
    assert_eq!(lines[i].original, Some(2));
    assert!(!lines[i].line.no_trailing_newline);
}

// GHD: unit/diff-parser-test.ts › DiffParser › parses diffs where neither file version has a trailing newline
#[test]
fn parses_diffs_where_neither_file_version_has_a_trailing_newline() {
    // echo -n 'foo' >  test
    // git add -A && git commit -m foo
    // echo -n 'bar' > test
    // git diff test
    let diff_text = r#"diff --git a/test b/test
index 1910281..ba0e162 100644
--- a/test
+++ b/test
@@ -1 +1 @@
-foo
\ No newline at end of file
+bar
\ No newline at end of file
"#;
    let diff = parse(diff_text);
    assert_eq!(diff.hunks.len(), 1);

    let hunk = &diff.hunks[0];
    assert_eq!(hunk.unified_diff_start, 0);
    assert_eq!(hunk.unified_diff_end(), 2);

    let lines = &hunk.lines;
    assert_eq!(lines.len(), 3);

    let mut i = 0;
    assert_eq!(ghd_text(&lines[i].line), "@@ -1 +1 @@");
    assert_eq!(lines[i].line.kind, DiffLineKind::Hunk);
    assert_eq!(lines[i].line.old_line, None);
    assert_eq!(lines[i].line.new_line, None);
    assert!(!lines[i].line.no_trailing_newline);
    i += 1;

    assert_eq!(ghd_text(&lines[i].line), "-foo");
    assert_eq!(lines[i].line.kind, DiffLineKind::Delete);
    assert_eq!(lines[i].line.old_line, Some(1));
    assert_eq!(lines[i].line.new_line, None);
    assert_eq!(lines[i].original, Some(1));
    assert!(lines[i].line.no_trailing_newline);
    i += 1;

    assert_eq!(ghd_text(&lines[i].line), "+bar");
    assert_eq!(lines[i].line.kind, DiffLineKind::Add);
    assert_eq!(lines[i].line.old_line, None);
    assert_eq!(lines[i].line.new_line, Some(1));
    assert_eq!(lines[i].original, Some(2));
    assert!(lines[i].line.no_trailing_newline);
}

// GHD: unit/diff-parser-test.ts › DiffParser › parses binary diffs
#[test]
fn parses_binary_diffs() {
    let diff_text = r#"diff --git a/IMG_2306.CR2 b/IMG_2306.CR2
new file mode 100644
index 0000000..4bf3a64
Binary files /dev/null and b/IMG_2306.CR2 differ
"#;
    let diff = parse(diff_text);
    assert_eq!(diff.hunks.len(), 0);
    assert!(diff.is_binary());
}

// GHD: unit/diff-parser-test.ts › DiffParser › parses diff of empty file
#[test]
fn parses_diff_of_empty_file() {
    // To produce this output, do
    // touch foo
    // git diff --no-index --patch-with-raw -z -- /dev/null foo
    let diff_text = r#"new file mode 100644
index 0000000..e69de29
"#;

    let diff = parse(diff_text);
    assert_eq!(diff.hunks.len(), 0);
}

// GHD: unit/diff-parser-test.ts › DiffParser › parses hunk headers with omitted line counts from new file
#[test]
fn parses_hunk_headers_with_omitted_line_counts_from_new_file() {
    let diff_text = r#"diff --git a/testste b/testste
new file mode 100644
index 0000000..f13588b
--- /dev/null
+++ b/testste
@@ -0,0 +1 @@
+asdfasdf
"#;

    let diff = parse(diff_text);
    assert_eq!(diff.hunks.len(), 1);

    let hunk = &diff.hunks[0];
    assert_eq!(hunk.old_start, 0);
    assert_eq!(hunk.old_lines, 0);
    assert_eq!(hunk.new_start, 1);
    assert_eq!(hunk.new_lines, 1);
}

// GHD: unit/diff-parser-test.ts › DiffParser › parses hunk headers with omitted line counts from old file
#[test]
fn parses_hunk_headers_with_omitted_line_counts_from_old_file() {
    let diff_text = r#"diff --git a/testste b/testste
new file mode 100644
index 0000000..f13588b
--- /dev/null
+++ b/testste
@@ -1 +0,0 @@
-asdfasdf
"#;

    let diff = parse(diff_text);
    assert_eq!(diff.hunks.len(), 1);

    let hunk = &diff.hunks[0];
    assert_eq!(hunk.old_start, 1);
    assert_eq!(hunk.old_lines, 1);
    assert_eq!(hunk.new_start, 0);
    assert_eq!(hunk.new_lines, 0);
}
