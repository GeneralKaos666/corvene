//! Port of GitHub Desktop's `app/test/unit/progress/clone-test.ts`.
//!
//! GitHub Desktop's `CloneProgressParser` (`lib/progress/clone.ts`, a
//! `GitProgressParser` with the clone steps) is
//! `corvene_git::CloneProgressParser`, which `corvene_git::clone` feeds
//! every `--progress` line. The cases that parse one line with a fresh
//! parser call `corvene_git::parse_clone_progress` (a fresh parser's
//! `parse`). `CloneProgress::value` is `Some(percent)` for GitHub Desktop's
//! `{ kind: 'progress', percent }` and `None` for its `{ kind: 'context' }`.
//!
//! - GitHub Desktop's "understands …" cases assert `parse(line) !== null`,
//!   which its parser always satisfies (it returns a progress or a context
//!   object); `parse_clone_progress` returns a `CloneProgress`, never
//!   nothing, which the type already guarantees.
//! - Percentages are JavaScript numbers (`f64`) in GitHub Desktop and `f32`
//!   in Corvene. The expected values are GitHub Desktop's expressions
//!   evaluated in `f64`, as GitHub Desktop evaluates them, then rounded to
//!   `f32`: GitHub Desktop's exact value in Corvene's type.

use corvene_git::{CloneProgress, CloneProgressParser, parse_clone_progress};

// GHD: unit/progress/clone-test.ts › CloneProgressParser › #parse › understands receiving object
#[test]
fn understands_receiving_object() {
    let _result: CloneProgress =
        parse_clone_progress("Receiving objects:  17% (4808/28282), 3.30 MiB | 1.29 MiB/s");
}

// GHD: unit/progress/clone-test.ts › CloneProgressParser › #parse › understands resolving deltas
#[test]
fn understands_resolving_deltas() {
    let _result: CloneProgress = parse_clone_progress("Resolving deltas:  89% (18063/20263)");
}

// GHD: unit/progress/clone-test.ts › CloneProgressParser › #parse › understands checking out files
#[test]
fn understands_checking_out_files() {
    let _result: CloneProgress = parse_clone_progress("Checking out files: 100% (579/579)");
}

// GHD: unit/progress/clone-test.ts › CloneProgressParser › #parse › understands remote compression
#[test]
fn understands_remote_compression() {
    let _result: CloneProgress = parse_clone_progress("remote: Compressing objects:  45% (10/22)");
}

// GHD: unit/progress/clone-test.ts › CloneProgressParser › #parse › understands relative weights
#[test]
fn understands_relative_weights() {
    let compressing = parse_clone_progress("remote: Compressing objects:  45% (10/22)");
    assert!(compressing.value.is_some(), "kind: expected 'progress'");
    assert_eq!(compressing.value, Some(((10.0f64 / 22.0) * 0.1) as f32));

    let receiving =
        parse_clone_progress("Receiving objects:  17% (4808/28282), 3.30 MiB | 1.29 MiB/s");
    assert!(receiving.value.is_some(), "kind: expected 'progress'");
    assert_eq!(
        receiving.value,
        Some((0.1f64 + (4808.0 / 28282.0) * 0.6) as f32)
    );

    let resolving = parse_clone_progress("Resolving deltas:  89% (18063/20263)");
    assert!(resolving.value.is_some(), "kind: expected 'progress'");
    assert_eq!(
        resolving.value,
        Some((0.7f64 + (18063.0 / 20263.0) * 0.1) as f32)
    );

    let checking_out = parse_clone_progress("Checking out files: 100% (579/579)");
    assert!(checking_out.value.is_some(), "kind: expected 'progress'");
    assert_eq!(
        checking_out.value,
        Some((0.8f64 + (579.0 / 579.0) * 0.2) as f32)
    );
}

// GHD: unit/progress/clone-test.ts › CloneProgressParser › #parse › ignores wrong order
#[test]
fn ignores_wrong_order() {
    let mut parser = CloneProgressParser::new();

    let final_progress = parser.parse("Checking out files: 100% (579/579)");
    let early_progress = parser.parse("Receiving objects:   1% (283/28282)");

    assert!(early_progress.value.is_none(), "kind: expected 'context'");
    assert!(final_progress.value.is_some(), "kind: expected 'progress'");
}

// GHD: unit/progress/clone-test.ts › CloneProgressParser › #parse › ignores lines it doesn't understand
#[test]
fn ignores_lines_it_doesnt_understand() {
    assert!(
        parse_clone_progress("Counting objects: 28282, done.")
            .value
            .is_none(),
        "kind: expected 'context'"
    );
}
