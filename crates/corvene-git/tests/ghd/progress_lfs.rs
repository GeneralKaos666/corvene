//! Port of GitHub Desktop's `app/test/unit/progress/lfs-test.ts`.
//!
//! GitHub Desktop's `GitLFSProgressParser` (`lib/progress/lfs.ts`, which
//! reads the `GIT_LFS_PROGRESS` file of a fetch, pull or push and reports
//! per-file transfer progress) is `corvene_git::GitLfsProgressParser`. Its
//! `parse` answers like `corvene_git::ProgressParser::parse`:
//! `Some((percent, text))` for GitHub Desktop's `{ kind: 'progress' }`,
//! `None` for its `{ kind: 'context' }`.

use corvene_git::GitLfsProgressParser;

/// The `beforeEach` of `#parse`.
fn setup() -> GitLfsProgressParser {
    GitLfsProgressParser::new()
}

// GHD: unit/progress/lfs-test.ts › GitLFSProgressParser › #parse › understands valid lines
#[test]
fn understands_valid_lines() {
    let mut parser = setup();
    let result = parser.parse("download 1/2 5/300 my cool image.jpg");
    assert!(result.is_some(), "kind: expected 'progress'");
}

// GHD: unit/progress/lfs-test.ts › GitLFSProgressParser › #parse › ignores lines it doesn't understand
#[test]
fn ignores_lines_it_doesnt_understand() {
    let mut parser = setup();
    let result = parser.parse("All this happened, more or less.");
    assert!(result.is_none(), "kind: expected 'context'");
}
