//! Port of GitHub Desktop's `app/test/unit/progress/git-test.ts`.
//!
//! - `GitProgressParser` (`lib/progress/git.ts`) is
//!   `corvene_git::ProgressParser`: `new(steps)` takes `(title, weight)`
//!   pairs, and `parse(line)` returns `Some((percent, text))` for GitHub
//!   Desktop's `{ kind: 'progress', percent }` and `None` for its
//!   `{ kind: 'context' }`.
//! - `parse(line)` (same file) is `corvene_git::parse_progress_line`, whose
//!   `ProgressLine` has GitHub Desktop's `IGitProgressInfo` fields except
//!   `percent` (the integer before `%`). The cases that compare a whole
//!   `IGitProgressInfo` check every other field against `ProgressLine` and
//!   `percent` through the stand-in [`progress_percent`], so they are
//!   ignored until `ProgressLine` has that field.
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

/// Stand-in for the `percent` field of GitHub Desktop's `IGitProgressInfo`
/// (`lib/progress/git.ts`: the `14` of `14% (159/1133)`, `undefined` when
/// the line has no total), which `corvene_git::remote_ops::ProgressLine`
/// does not have. Replace it with the field once there is one.
fn progress_percent(_info: &ProgressLine) -> Option<u32> {
    unimplemented!("corvene_git::remote_ops::ProgressLine has no percent field")
}

// GHD: unit/progress/git-test.ts › GitProgressParser › requires at least one step
#[test]
#[ignore = "ghd: bug: ProgressParser::new(&[]) accepts an empty step list; GHD GitProgressParser throws 'must specify at least one step'"]
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
#[ignore = "ghd: bug: ProgressParser computes percent in f32: Receiving objects 99% (166741/167587) at weight 0.5 gives 0.997476, GHD's f64 0.99747593 rounds to 0.9974759"]
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
#[ignore = "ghd: bug: ProgressParser computes percent in f32: Receiving objects 99% (166741/167587) at weight 0.5 gives 0.997476, GHD's f64 0.99747593 rounds to 0.9974759"]
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
#[ignore = "ghd: missing: corvene_git ProgressLine has no percent field (GHD IGitProgressInfo.percent, lib/progress/git.ts parse)"]
fn parses_progress_with_no_total() {
    let result = parse_progress_line("remote: Counting objects: 167587");

    assert_eq!(
        result,
        Some(ProgressLine {
            title: "remote: Counting objects".into(),
            text: "remote: Counting objects: 167587".into(),
            value: 167587,
            done: false,
            total: None,
        })
    );
    assert_eq!(result.as_ref().map(progress_percent), Some(None));
}

// GHD: unit/progress/git-test.ts › GitProgressParser › parses final progress with no total
#[test]
#[ignore = "ghd: missing: corvene_git ProgressLine has no percent field (GHD IGitProgressInfo.percent, lib/progress/git.ts parse)"]
fn parses_final_progress_with_no_total() {
    let result = parse_progress_line("remote: Counting objects: 167587, done.");

    assert_eq!(
        result,
        Some(ProgressLine {
            title: "remote: Counting objects".into(),
            text: "remote: Counting objects: 167587, done.".into(),
            value: 167587,
            done: true,
            total: None,
        })
    );
    assert_eq!(result.as_ref().map(progress_percent), Some(None));
}

// GHD: unit/progress/git-test.ts › GitProgressParser › parses progress with total
#[test]
#[ignore = "ghd: missing: corvene_git ProgressLine has no percent field (GHD IGitProgressInfo.percent, lib/progress/git.ts parse)"]
fn parses_progress_with_total() {
    let result = parse_progress_line("remote: Compressing objects:  72% (16/22)");

    assert_eq!(
        result,
        Some(ProgressLine {
            title: "remote: Compressing objects".into(),
            text: "remote: Compressing objects:  72% (16/22)".into(),
            value: 16,
            done: false,
            total: Some(22),
        })
    );
    assert_eq!(result.as_ref().map(progress_percent), Some(Some(72)));
}

// GHD: unit/progress/git-test.ts › GitProgressParser › parses final with total
#[test]
#[ignore = "ghd: missing: corvene_git ProgressLine has no percent field (GHD IGitProgressInfo.percent, lib/progress/git.ts parse)"]
fn parses_final_with_total() {
    let result = parse_progress_line("remote: Compressing objects: 100% (22/22), done.");

    assert_eq!(
        result,
        Some(ProgressLine {
            title: "remote: Compressing objects".into(),
            text: "remote: Compressing objects: 100% (22/22), done.".into(),
            value: 22,
            done: true,
            total: Some(22),
        })
    );
    assert_eq!(result.as_ref().map(progress_percent), Some(Some(100)));
}

// GHD: unit/progress/git-test.ts › GitProgressParser › parses with total and throughput
#[test]
#[ignore = "ghd: missing: corvene_git ProgressLine has no percent field (GHD IGitProgressInfo.percent, lib/progress/git.ts parse)"]
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
            total: Some(167587),
        })
    );
    assert_eq!(result.as_ref().map(progress_percent), Some(Some(99)));
}

// GHD: unit/progress/git-test.ts › GitProgressParser › parses final with total and throughput
#[test]
#[ignore = "ghd: missing: corvene_git ProgressLine has no percent field (GHD IGitProgressInfo.percent, lib/progress/git.ts parse)"]
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
            total: Some(167587),
        })
    );
    assert_eq!(result.as_ref().map(progress_percent), Some(Some(100)));
}

// GHD: unit/progress/git-test.ts › GitProgressParser › does not parse things that aren't progress
#[test]
fn does_not_parse_things_that_arent_progress() {
    let result = parse_progress_line(
        "remote: Total 167587 (delta 19), reused 11 (delta 11), pack-reused 167554         ",
    );
    assert!(result.is_none());
}
