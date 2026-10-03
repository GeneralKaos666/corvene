//! Port of GitHub Desktop's `app/test/unit/desktop-file-transport-test.ts`
//! (`main-process/desktop-file-transport.ts`).
//!
//! GitHub Desktop's `DesktopFileTransport({ logDirectory })` writes each log
//! message to `<logDirectory>/<UTC date>.desktop.<channel>.log`, opening a
//! new file when the date changes and keeping the newest 14
//! (`MaxRetainedLogFiles`). Corvene's log file is set up in
//! `crates/corvene/src/logging.rs` (`tracing_appender::rolling::daily` in
//! `corvene_platform::paths::logs_dir()`): it lives in the binary crate,
//! which integration tests cannot reach, and takes neither a directory nor
//! a clock. [`DesktopFileTransport`] is a stand-in; its `info` takes the
//! instant GitHub Desktop's cases set with `t.mock.timers`.

use std::path::Path;
use std::time::SystemTime;

use corvene_test_support::{create_temp_directory, date_parse};

/// Stand-in for GitHub Desktop's `DesktopFileTransport`
/// (`main-process/desktop-file-transport.ts`). Replace it with the Corvene
/// type once there is one and remove the `#[ignore]`s.
struct DesktopFileTransport;

impl DesktopFileTransport {
    /// `new DesktopFileTransport({ logDirectory })`
    fn new(_log_directory: &Path) -> Self {
        unimplemented!(
            "Corvene has no DesktopFileTransport (main-process/desktop-file-transport.ts)"
        )
    }

    /// The test file's `info(transport, message)`, written at `now` (GitHub
    /// Desktop's `Date`).
    fn info(&mut self, _message: &str, _now: SystemTime) {
        unimplemented!(
            "Corvene has no DesktopFileTransport (main-process/desktop-file-transport.ts)"
        )
    }

    /// `transport.close()`
    fn close(self) {}
}

fn read_dir_names(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect()
}

/// Node's `os.EOL`.
const EOL: &str = if cfg!(windows) { "\r\n" } else { "\n" };

// GHD: unit/desktop-file-transport-test.ts › DesktopFileTransport › creates a file on demand
#[test]
#[ignore = "ghd: missing: no DesktopFileTransport (main-process/desktop-file-transport.ts); logging.rs is in the binary crate and takes no directory"]
fn creates_a_file_on_demand() {
    let d = create_temp_directory();
    let mut transport = DesktopFileTransport::new(d.path());

    assert_eq!(read_dir_names(d.path()).len(), 0);
    transport.info("heyo", SystemTime::now());
    let files = read_dir_names(d.path());
    assert_eq!(files.len(), 1);
    assert_eq!(
        std::fs::read_to_string(d.path().join(&files[0])).unwrap(),
        format!("heyo{EOL}")
    );

    transport.close();
}

// GHD: unit/desktop-file-transport-test.ts › DesktopFileTransport › creates a file for each day
#[test]
#[ignore = "ghd: missing: no DesktopFileTransport (main-process/desktop-file-transport.ts); logging.rs is in the binary crate, no injectable clock"]
fn creates_a_file_for_each_day() {
    let d = create_temp_directory();
    let mut transport = DesktopFileTransport::new(d.path());

    assert_eq!(read_dir_names(d.path()).len(), 0);

    transport.info("heyo", date_parse("2022-03-10T10:00:00.000Z"));

    transport.info("heyo", date_parse("2022-03-11T11:00:00.000Z"));

    assert_eq!(read_dir_names(d.path()).len(), 2);

    transport.close();
}

// GHD: unit/desktop-file-transport-test.ts › DesktopFileTransport › retains a maximum of 14 log files
#[test]
#[ignore = "ghd: missing: no DesktopFileTransport (main-process/desktop-file-transport.ts); logging.rs keeps every daily file, no injectable clock"]
fn retains_a_maximum_of_14_log_files() {
    let d = create_temp_directory();
    let mut transport = DesktopFileTransport::new(d.path());

    let dates = [
        "2022-03-01T10:00:00.000Z",
        "2022-03-02T10:00:00.000Z",
        "2022-03-03T10:00:00.000Z",
        "2022-03-04T10:00:00.000Z",
        "2022-03-05T10:00:00.000Z",
        "2022-03-06T10:00:00.000Z",
        "2022-03-07T10:00:00.000Z",
        "2022-03-08T10:00:00.000Z",
        "2022-03-09T10:00:00.000Z",
        "2022-03-10T10:00:00.000Z",
        "2022-03-11T10:00:00.000Z",
        "2022-03-12T10:00:00.000Z",
        "2022-03-13T10:00:00.000Z",
        "2022-03-14T10:00:00.000Z",
        "2022-03-15T10:00:00.000Z",
        "2022-03-16T10:00:00.000Z",
        "2022-03-17T10:00:00.000Z",
        "2022-03-18T10:00:00.000Z",
        "2022-03-19T10:00:00.000Z",
        "2022-03-20T10:00:00.000Z",
    ];

    assert_eq!(read_dir_names(d.path()).len(), 0);
    for date in dates {
        transport.info("heyo", date_parse(date));
    }

    let mut retained_files = read_dir_names(d.path());

    // Retains the newest files (ISO date is lexicographically sortable)
    retained_files.sort();
    assert_eq!(
        retained_files,
        [
            "2022-03-07.desktop.development.log",
            "2022-03-08.desktop.development.log",
            "2022-03-09.desktop.development.log",
            "2022-03-10.desktop.development.log",
            "2022-03-11.desktop.development.log",
            "2022-03-12.desktop.development.log",
            "2022-03-13.desktop.development.log",
            "2022-03-14.desktop.development.log",
            "2022-03-15.desktop.development.log",
            "2022-03-16.desktop.development.log",
            "2022-03-17.desktop.development.log",
            "2022-03-18.desktop.development.log",
            "2022-03-19.desktop.development.log",
            "2022-03-20.desktop.development.log",
        ]
    );

    transport.close();
}
