//! Port of GitHub Desktop's `app/test/unit/desktop-file-transport-test.ts`
//! (`main-process/desktop-file-transport.ts`).
//!
//! GitHub Desktop's `DesktopFileTransport({ logDirectory })` writes each log
//! message to `<logDirectory>/<UTC date>.desktop.<channel>.log`, opening a
//! new file when the date changes and keeping the newest 14
//! (`MaxRetainedLogFiles`). Corvene's is
//! `corvene_platform::desktop_file_transport::DesktopFileTransport` (the
//! writer behind `crates/corvene/src/logging.rs`); a debug build's channel
//! is `development`, as in GitHub Desktop's tests. The test file's
//! `info(transport, message)` is [`Info::info`], which takes the instant
//! GitHub Desktop's cases set with `t.mock.timers`.

use std::path::Path;
use std::time::SystemTime;

use corvene_platform::desktop_file_transport::DesktopFileTransport;
use corvene_test_support::{create_temp_directory, date_parse};

/// The test file's `info(transport, message)`: log `message` at `now`
/// (GitHub Desktop's `Date`).
trait Info {
    fn info(&mut self, message: &str, now: SystemTime);
}

impl Info for DesktopFileTransport {
    fn info(&mut self, message: &str, now: SystemTime) {
        self.log(message, now);
    }
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
