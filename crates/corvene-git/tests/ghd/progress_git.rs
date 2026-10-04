//! Port of GitHub Desktop's `app/test/unit/progress/git-test.ts`.
//!
//! - `GitProgressParser` (`lib/progress/git.ts`) is
//!   `corvene_git::ProgressParser`: `new(steps)` takes `(title, weight)`
//!   pairs, and `parse(line)` returns `Some((percent, text))` for GitHub
//!   Desktop's `{ kind: 'progress', percent }` and `None` for its
//!   `{ kind: 'context' }`.
//! - `parse(line)` (same file) is `corvene_git::parse_progress_line`, whose
//!   `ProgressLine` has GitHub Desktop's `IGitProgressInfo` fields
//!   (`percent`, the integer before `%`, is `None` for GitHub Desktop's
//!   `undefined`).
//! - GitHub Desktop computes `percent` in JavaScript numbers (`f64`);
//!   Corvene's parser answers `f32`. The expected values are GitHub
//!   Desktop's expressions evaluated in `f64`, as GitHub Desktop evaluates
//!   them, then rounded to `f32`: GitHub Desktop's exact value in Corvene's
//!   type.
//! - "requires at least one step": GitHub Desktop's constructor throws, the
//!   Rust equivalent is a panic.

use std::panic::{AssertUnwindSafe, catch_unwind};

use corvene_git::remote_ops::ProgressLine;
use corvene_git::{ProgressParser, parse_progress_line};

// GHD: unit/progress/git-test.ts › GitProgressParser › requires at least one step
#[test]
fn requires_at_least_one_step() {
    let result = catch_unwind(AssertUnwindSafe(|| ProgressParser::new(&[])));
    assert!(result.is_err());
}

// GHD: unit/progress/git-test.ts › GitProgressParser › parses progress with one step
#[test]
fn parses_progress_with_one_step() {
    let mut parser = ProgressParser::new(&[("remote: Compressing objects", 1.0)]);

    assert_eq!(
        parser
            .parse("remote: Compressing objects:  72% (16/22)")
            .map(|(percent, _)| percent),
        Some((16.0f64 / 22.0) as f32)
    );
}

// GHD: unit/progress/git-test.ts › GitProgressParser › parses progress with several steps
#[test]
fn parses_progress_with_several_steps() {
    let mut parser = ProgressParser::new(&[
        ("remote: Compressing objects", 0.5),
        ("Receiving objects", 0.5),
    ]);

    let result = parser.parse("remote: Compressing objects:  72% (16/22)");

    assert!(result.is_some(), "kind: expected 'progress'");
    assert_eq!(
        result.map(|(percent, _)| percent),
        Some((16.0f64 / 22.0 / 2.0) as f32)
    );

    let result = parser.parse("Receiving objects:  99% (166741/167587), 267.24 MiB | 2.40 MiB/s");

    assert!(result.is_some(), "kind: expected 'progress'");
    assert_eq!(
        result.map(|(percent, _)| percent),
        Some((0.5f64 + 166741.0 / 167587.0 / 2.0) as f32)
    );
}

// GHD: unit/progress/git-test.ts › GitProgressParser › enforces ordering of steps
#[test]
fn enforces_ordering_of_steps() {
    let mut parser = ProgressParser::new(&[
        ("remote: Compressing objects", 0.5),
        ("Receiving objects", 0.5),
    ]);

    let result = parser.parse("remote: Compressing objects:  72% (16/22)");

    assert!(result.is_some(), "kind: expected 'progress'");
    assert_eq!(
        result.map(|(percent, _)| percent),
        Some((16.0f64 / 22.0 / 2.0) as f32)
    );

    let result = parser.parse("Receiving objects:  99% (166741/167587), 267.24 MiB | 2.40 MiB/s");

    assert!(result.is_some(), "kind: expected 'progress'");
    assert_eq!(
        result.map(|(percent, _)| percent),
        Some((0.5f64 + 166741.0 / 167587.0 / 2.0) as f32)
    );

    let result = parser.parse("remote: Compressing objects:  72% (16/22)");

    assert!(result.is_none(), "kind: expected 'context'");
}

// GHD: unit/progress/git-test.ts › GitProgressParser › parses progress with no total
#[test]
fn parses_progress_with_no_total() {
    let result = parse_progress_line("remote: Counting objects: 167587");

    assert_eq!(
        result,
        Some(ProgressLine {
            title: "remote: Counting objects".into(),
            text: "remote: Counting objects: 167587".into(),
            value: 167587,
            done: false,
            percent: None,
            total: None,
        })
    );
    assert_eq!(result.as_ref().map(|info| info.percent), Some(None));
}

// GHD: unit/progress/git-test.ts › GitProgressParser › parses final progress with no total
#[test]
fn parses_final_progress_with_no_total() {
    let result = parse_progress_line("remote: Counting objects: 167587, done.");

    assert_eq!(
        result,
        Some(ProgressLine {
            title: "remote: Counting objects".into(),
            text: "remote: Counting objects: 167587, done.".into(),
            value: 167587,
            done: true,
            percent: None,
            total: None,
        })
    );
    assert_eq!(result.as_ref().map(|info| info.percent), Some(None));
}

// GHD: unit/progress/git-test.ts › GitProgressParser › parses progress with total
#[test]
fn parses_progress_with_total() {
    let result = parse_progress_line("remote: Compressing objects:  72% (16/22)");

    assert_eq!(
        result,
        Some(ProgressLine {
            title: "remote: Compressing objects".into(),
            text: "remote: Compressing objects:  72% (16/22)".into(),
            value: 16,
            done: false,
            percent: Some(72),
            total: Some(22),
        })
    );
    assert_eq!(result.as_ref().map(|info| info.percent), Some(Some(72)));
}

// GHD: unit/progress/git-test.ts › GitProgressParser › parses final with total
#[test]
fn parses_final_with_total() {
    let result = parse_progress_line("remote: Compressing objects: 100% (22/22), done.");

    assert_eq!(
        result,
        Some(ProgressLine {
            title: "remote: Compressing objects".into(),
            text: "remote: Compressing objects: 100% (22/22), done.".into(),
            value: 22,
            done: true,
            percent: Some(100),
            total: Some(22),
        })
    );
    assert_eq!(result.as_ref().map(|info| info.percent), Some(Some(100)));
}

// GHD: unit/progress/git-test.ts › GitProgressParser › parses with total and throughput
#[test]
fn parses_with_total_and_throughput() {
    let result =
        parse_progress_line("Receiving objects:  99% (166741/167587), 267.24 MiB | 2.40 MiB/s");

    assert_eq!(
        result,
        Some(ProgressLine {
            title: "Receiving objects".into(),
            text: "Receiving objects:  99% (166741/167587), 267.24 MiB | 2.40 MiB/s".into(),
            value: 166741,
            done: false,
            percent: Some(99),
            total: Some(167587),
        })
    );
    assert_eq!(result.as_ref().map(|info| info.percent), Some(Some(99)));
}

// GHD: unit/progress/git-test.ts › GitProgressParser › parses final with total and throughput
#[test]
fn parses_final_with_total_and_throughput() {
    let result = parse_progress_line(
        "Receiving objects: 100% (167587/167587), 279.67 MiB | 2.43 MiB/s, done.",
    );

    assert_eq!(
        result,
        Some(ProgressLine {
            title: "Receiving objects".into(),
            text: "Receiving objects: 100% (167587/167587), 279.67 MiB | 2.43 MiB/s, done.".into(),
            value: 167587,
            done: true,
            percent: Some(100),
            total: Some(167587),
        })
    );
    assert_eq!(result.as_ref().map(|info| info.percent), Some(Some(100)));
}

// GHD: unit/progress/git-test.ts › GitProgressParser › does not parse things that aren't progress
#[test]
fn does_not_parse_things_that_arent_progress() {
    let result = parse_progress_line(
        "remote: Total 167587 (delta 19), reused 11 (delta 11), pack-reused 167554         ",
    );
    assert!(result.is_none());
}
