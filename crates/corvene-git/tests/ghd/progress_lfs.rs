//! Port of GitHub Desktop's `app/test/unit/progress/lfs-test.ts`.
//!
//! Corvene has no equivalent of `GitLFSProgressParser`
//! (`lib/progress/lfs.ts`, which reads the `GIT_LFS_PROGRESS` file of a
//! clone, checkout, fetch, pull or push and reports per-file transfer
//! progress), so the cases use a stand-in and are ignored until it exists.
//! Its `parse` answers like `corvene_git::ProgressParser::parse`:
//! `Some((percent, text))` for GitHub Desktop's `{ kind: 'progress' }`,
//! `None` for its `{ kind: 'context' }`.

/// Stand-in for GitHub Desktop's `GitLFSProgressParser`. Replace it with
/// the `corvene_git` parser once there is one and remove the `#[ignore]`s.
struct GitLfsProgressParser;

impl GitLfsProgressParser {
    /// `new GitLFSProgressParser()`.
    fn new() -> Self {
        Self
    }

    /// `parse(line)`: a `<direction> <current>/<total files>
    /// <downloaded>/<total> <name>` line is progress, anything else context.
    fn parse(&mut self, _line: &str) -> Option<(f32, String)> {
        unimplemented!("corvene_git has no GitLFSProgressParser")
    }
}

/// The `beforeEach` of `#parse`.
fn setup() -> GitLfsProgressParser {
    GitLfsProgressParser::new()
}

// GHD: unit/progress/lfs-test.ts › GitLFSProgressParser › #parse › understands valid lines
#[test]
#[ignore = "ghd: missing: corvene_git has no GitLFSProgressParser (lib/progress/lfs.ts)"]
fn understands_valid_lines() {
    let mut parser = setup();
    let result = parser.parse("download 1/2 5/300 my cool image.jpg");
    assert!(result.is_some(), "kind: expected 'progress'");
}

// GHD: unit/progress/lfs-test.ts › GitLFSProgressParser › #parse › ignores lines it doesn't understand
#[test]
#[ignore = "ghd: missing: corvene_git has no GitLFSProgressParser (lib/progress/lfs.ts)"]
fn ignores_lines_it_doesnt_understand() {
    let mut parser = setup();
    let result = parser.parse("All this happened, more or less.");
    assert!(result.is_none(), "kind: expected 'context'");
}
