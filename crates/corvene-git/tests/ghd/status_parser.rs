//! Port of GitHub Desktop's `app/test/unit/status-parser-test.ts`.
//!
//! Corvene equivalent: `parsePorcelainStatus(output)`
//! (`lib/status-parser.ts`) is `corvene_git::parse_porcelain_v2(output)`.
//! GitHub Desktop returns a flat list of status entries and `# …` headers;
//! Corvene parses the same `git status --porcelain=2 -z` output straight
//! into a `WorkingDirectoryStatus`:
//!
//! - an `IStatusEntry` is a `WorkingDirectoryFileChange` in
//!   `WorkingDirectoryStatus::files` (git's order): `path` is `path`,
//!   `oldPath` is `old_path`, `statusCode` is `status.code`, and the
//!   submodule status code (`S<c><m><u>`, `N...` for no submodule) is
//!   `status.submodule` (`S`) with `status.submodule_status`'s
//!   `commit_changed` (`C`), `modified_changes` (`M`) and
//!   `untracked_changes` (`U`);
//! - the `IStatusHeader`s are parsed into fields (what GitHub Desktop's
//!   `parseStatusHeader` in `lib/git/status.ts` does with them):
//!   `branch.oid <sha>` is `current_tip`, `branch.head <name>` is `branch`,
//!   `branch.upstream <name>` is `upstream`, `branch.ab +A -B` is
//!   `ahead_behind`;
//! - the list's length is [`entries_len`]: the files plus the headers
//!   parsed into fields.

use corvene_models::{AheadBehind, SubmoduleStatus, WorkingDirectoryStatus};

fn parse(input: &str) -> WorkingDirectoryStatus {
    corvene_git::parse_porcelain_v2(input.as_bytes())
}

/// GitHub Desktop's `entries.length`: the status entries (files) plus the
/// `# branch.*` headers, which Corvene keeps as fields.
fn entries_len(status: &WorkingDirectoryStatus) -> usize {
    let headers = [
        status.current_tip.is_some(),
        status.branch.is_some(),
        status.upstream.is_some(),
        status.ahead_behind.is_some(),
    ];
    status.files.len() + headers.iter().filter(|h| **h).count()
}

// GHD: unit/status-parser-test.ts › parsePorcelainStatus › parses a standard status
#[test]
fn parses_a_standard_status() {
    let entries = parse(
        &([
            "1 .D N... 100644 100644 000000 e69de29bb2d1d6434b8b29ae775ad8c2e48c5391 e69de29bb2d1d6434b8b29ae775ad8c2e48c5391 deleted",
            "1 .M N... 100644 100644 100644 e69de29bb2d1d6434b8b29ae775ad8c2e48c5391 e69de29bb2d1d6434b8b29ae775ad8c2e48c5391 modified",
            "? untracked",
        ]
        .join("\0")
            + "\0"),
    );

    assert_eq!(entries_len(&entries), 3);

    let mut i = 0;
    assert_eq!(entries.files[i].status.code, ".D");
    assert_eq!(entries.files[i].path, "deleted");
    i += 1;

    assert_eq!(entries.files[i].status.code, ".M");
    assert_eq!(entries.files[i].path, "modified");
    i += 1;

    assert_eq!(entries.files[i].status.code, "??");
    assert_eq!(entries.files[i].path, "untracked");
}

// GHD: unit/status-parser-test.ts › parsePorcelainStatus › parses renames
#[test]
fn parses_renames() {
    let entries = parse(
        &([
            "2 R. N... 100644 100644 100644 2de0487c2d3e977f5f560b746833f9d7f9a054fd 2de0487c2d3e977f5f560b746833f9d7f9a054fd R100 new\0old",
            "2 RM N... 100644 100644 100644 a3cba7afce66ef37a228e094273c27141db21f36 a3cba7afce66ef37a228e094273c27141db21f36 R100 to\0from",
        ]
        .join("\0")
            + "\0"),
    );

    assert_eq!(entries_len(&entries), 2);

    let mut i = 0;

    assert_eq!(entries.files[i].status.code, "R.");
    assert_eq!(entries.files[i].path, "new");
    assert_eq!(entries.files[i].old_path.as_deref(), Some("old"));
    i += 1;

    assert_eq!(entries.files[i].status.code, "RM");
    assert_eq!(entries.files[i].path, "to");
    assert_eq!(entries.files[i].old_path.as_deref(), Some("from"));
}

// GHD: unit/status-parser-test.ts › parsePorcelainStatus › ignores ignored files
#[test]
fn ignores_ignored_files() {
    // We don't run status with --ignored so this shouldn't be a problem
    // but we test it all the same

    let entries = parse(&(["! foo"].join("\0") + "\0"));

    assert_eq!(entries_len(&entries), 0);
}

// GHD: unit/status-parser-test.ts › parsePorcelainStatus › parses status headers
#[test]
fn parses_status_headers() {
    // We don't run status with --ignored so this shouldn't be a problem
    // but we test it all the same

    let entries = parse(
        &([
            "# branch.oid 2de0487c2d3e977f5f560b746833f9d7f9a054fd",
            "# branch.head master",
            "# branch.upstream origin/master",
            "# branch.ab +1 -0",
        ]
        .join("\0")
            + "\0"),
    );

    assert_eq!(entries_len(&entries), 4);

    // 'branch.oid 2de0487c2d3e977f5f560b746833f9d7f9a054fd'
    assert_eq!(
        entries.current_tip.as_deref(),
        Some("2de0487c2d3e977f5f560b746833f9d7f9a054fd")
    );
    // 'branch.head master'
    assert_eq!(entries.branch.as_deref(), Some("master"));
    // 'branch.upstream origin/master'
    assert_eq!(entries.upstream.as_deref(), Some("origin/master"));
    // 'branch.ab +1 -0'
    assert_eq!(
        entries.ahead_behind,
        Some(AheadBehind {
            ahead: 1,
            behind: 0
        })
    );
}

// GHD: unit/status-parser-test.ts › parsePorcelainStatus › parses a path which includes a newline
#[test]
fn parses_a_path_which_includes_a_newline() {
    let x = "1 D. N... 100644 000000 000000 dc9fb24e86f7445720b39dcb39a7fc0e410d9583 0000000000000000000000000000000000000000 ProjectSID/Images.xcassets/iPhone 67/Status Center/Report X68 Y461\n      /.DS_Store";
    let entries = parse(x);

    assert_eq!(entries_len(&entries), 1);

    let expected_path =
        "ProjectSID/Images.xcassets/iPhone 67/Status Center/Report X68 Y461\n      /.DS_Store";

    assert_eq!(entries.files[0].path, expected_path);
    assert_eq!(entries.files[0].status.code, "D.");
}

// GHD: unit/status-parser-test.ts › parsePorcelainStatus › parses a typechange
#[test]
fn parses_a_typechange() {
    let x = "1 .T N... 120000 120000 100755 6165716e8b408ad09b51d1a37aa1ef50e7f84376 6165716e8b408ad09b51d1a37aa1ef50e7f84376 pdf_linux-x64/lib/libQt5Core.so.5";
    let entries = parse(x);

    assert_eq!(entries_len(&entries), 1);

    assert_eq!(entries.files[0].path, "pdf_linux-x64/lib/libQt5Core.so.5");
    assert_eq!(entries.files[0].status.code, ".T");
}

// GHD: unit/status-parser-test.ts › parsePorcelainStatus › parses submodule changes
#[test]
fn parses_submodule_changes() {
    let x = "1 .M SCMU 100644 100644 100644 e69de29bb2d1d6434b8b29ae775ad8c2e48c5391 e69de29bb2d1d6434b8b29ae775ad8c2e48c5391 submodule/submodule";
    let entries = parse(x);
    assert_eq!(entries_len(&entries), 1);
    assert_eq!(entries.files[0].path, "submodule/submodule");
    // submoduleStatusCode 'SCMU'
    assert!(entries.files[0].status.submodule);
    assert_eq!(
        entries.files[0].status.submodule_status,
        Some(SubmoduleStatus {
            commit_changed: true,
            modified_changes: true,
            untracked_changes: true,
        })
    );
}
