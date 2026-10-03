//! The log file writer - GHD `DesktopFileTransport`
//! (`app/src/main-process/desktop-file-transport.ts`, GHD's stand-in for
//! winston-daily-rotate-file): each message goes to
//! `<log directory>/<UTC date>.desktop.<channel>.log`, a file opened (and
//! created) on the first write of a day, and opening one prunes the
//! directory down to the newest [`MAX_RETAINED_LOG_FILES`].
//!
//! The binary's tracing setup (`crates/corvene/src/logging.rs`) writes
//! formatted events through the [`std::io::Write`] impl, dated by the
//! transport's clock (the system clock unless [`DesktopFileTransport::with_clock`]
//! replaces it); [`DesktopFileTransport::log`] is GHD's `log(info)` for one
//! message at a given instant.
//!
//! GHD sorts the log file `Dirent`s with a bare `sort()`, which compares
//! them all as `"[object Object]"` and so prunes in `readdir` order; the
//! intent (and its test) is the oldest dates first, which Corvene sorts by
//! name for. GHD swallows every file error (printing it in development
//! builds); `log` does the same, the `Write` impl returns them.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// GHD `MaxRetainedLogFiles`.
pub const MAX_RETAINED_LOG_FILES: usize = 14;

/// GHD `__RELEASE_CHANNEL__` as Corvene builds it: `development` for debug
/// builds, `production` for releases.
pub const RELEASE_CHANNEL: &str = if cfg!(debug_assertions) {
    "development"
} else {
    "production"
};

/// Node's `os.EOL`.
const EOL: &str = if cfg!(windows) { "\r\n" } else { "\n" };

type Clock = Box<dyn Fn() -> SystemTime + Send>;

/// GHD `DesktopFileTransport`: see the module docs.
pub struct DesktopFileTransport {
    log_directory: PathBuf,
    /// GHD `fileSuffix`: `.desktop.<channel>.log`
    file_suffix: String,
    /// The open file and its path (GHD `stream`, `stream.path`).
    stream: Option<(PathBuf, File)>,
    clock: Clock,
}

impl DesktopFileTransport {
    /// `new DesktopFileTransport({ logDirectory })`, for this build's
    /// [`RELEASE_CHANNEL`].
    pub fn new(log_directory: impl Into<PathBuf>) -> Self {
        Self::for_channel(log_directory, RELEASE_CHANNEL)
    }

    /// A transport whose files carry another release channel's suffix.
    pub fn for_channel(log_directory: impl Into<PathBuf>, channel: &str) -> Self {
        Self {
            log_directory: log_directory.into(),
            file_suffix: format!(".desktop.{channel}.log"),
            stream: None,
            clock: Box::new(SystemTime::now),
        }
    }

    /// Date the [`Write`] impl's writes with `clock` instead of the system
    /// clock.
    pub fn with_clock(mut self, clock: impl Fn() -> SystemTime + Send + 'static) -> Self {
        self.clock = Box::new(clock);
        self
    }

    /// GHD `log(info)`: write `message` and the platform's line ending to
    /// the file of `now`'s UTC date. Errors are swallowed, as GHD does.
    pub fn log(&mut self, message: &str, now: SystemTime) {
        let line = format!("{message}{EOL}");
        if let Err(err) = self.write_at(line.as_bytes(), now)
            && cfg!(debug_assertions)
        {
            eprintln!("DesktopFileTransport: write {err}");
        }
    }

    /// `transport.close()`: end the open file.
    pub fn close(mut self) {
        if let Some((_, mut file)) = self.stream.take() {
            let _ = file.flush();
        }
    }

    /// GHD `getFilePath`: `<log directory>/<YYYY-MM-DD><suffix>`.
    fn file_path(&self, now: SystemTime) -> PathBuf {
        let (year, month, day, _) = crate::crash_reports::utc_civil(now);
        self.log_directory
            .join(format!("{year:04}-{month:02}-{day:02}{}", self.file_suffix))
    }

    /// Open the file for `now` when it is not the open one (closing that and
    /// pruning the directory), then append `bytes`.
    fn write_at(&mut self, bytes: &[u8], now: SystemTime) -> std::io::Result<()> {
        let path = self.file_path(now);
        if self.stream.as_ref().is_none_or(|(open, _)| *open != path) {
            if let Some((_, mut old)) = self.stream.take() {
                let _ = old.flush();
            }
            let file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)?;
            self.stream = Some((path, file));
            prune_directory(&self.log_directory, &self.file_suffix);
        }
        match &mut self.stream {
            Some((_, file)) => file.write_all(bytes),
            None => Ok(()),
        }
    }
}

impl Write for DesktopFileTransport {
    /// One formatted event: its `\n` becomes the platform's line ending.
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let now = (self.clock)();
        match buf.strip_suffix(b"\n") {
            Some(line) if EOL != "\n" && !line.ends_with(b"\r") => {
                let mut bytes = line.to_vec();
                bytes.extend_from_slice(EOL.as_bytes());
                self.write_at(&bytes, now)?;
            }
            _ => self.write_at(buf, now)?,
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match &mut self.stream {
            Some((_, file)) => file.flush(),
            None => Ok(()),
        }
    }
}

/// GHD `pathRe`: a name ending in `YYYY-MM-DD<suffix>`.
fn is_log_file_name(name: &str, suffix: &str) -> bool {
    let Some(stem) = name.strip_suffix(suffix) else {
        return false;
    };
    let bytes = stem.as_bytes();
    let Some(date) = bytes.len().checked_sub(10).map(|start| &bytes[start..]) else {
        return false;
    };
    date.iter().enumerate().all(|(i, b)| match i {
        4 | 7 => *b == b'-',
        _ => b.is_ascii_digit(),
    })
}

/// GHD `pruneDirectory`: past [`MAX_RETAINED_LOG_FILES`] log files, remove
/// the oldest so that one fewer than the maximum remain (GHD's
/// `all.length - MaxRetainedLogFiles + 1`); the next day's file brings the
/// count back to the maximum.
fn prune_directory(dir: &Path, suffix: &str) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut all: Vec<String> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|name| is_log_file_name(name, suffix))
        .collect();
    if all.len() <= MAX_RETAINED_LOG_FILES {
        return;
    }
    all.sort();
    let end = all.len() - MAX_RETAINED_LOG_FILES + 1;
    for name in &all[..end] {
        let _ = std::fs::remove_file(dir.join(name));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn log_file_names_need_a_date_and_the_suffix() {
        let suffix = ".desktop.production.log";
        assert!(is_log_file_name(
            "2022-03-01.desktop.production.log",
            suffix
        ));
        assert!(is_log_file_name(
            "x2022-03-01.desktop.production.log",
            suffix
        ));
        assert!(!is_log_file_name(
            "2022-03-01.desktop.development.log",
            suffix
        ));
        assert!(!is_log_file_name("22-03-01.desktop.production.log", suffix));
        assert!(!is_log_file_name(
            "2022_03_01.desktop.production.log",
            suffix
        ));
    }

    #[test]
    fn writes_go_to_the_clock_s_day() {
        let dir = tempfile::tempdir().unwrap();
        // 2022-03-10T10:00:00Z
        let at = UNIX_EPOCH + Duration::from_secs(1_646_906_400);
        let mut transport =
            DesktopFileTransport::for_channel(dir.path(), "test").with_clock(move || at);
        transport.write_all(b"one\n").unwrap();
        transport.flush().unwrap();
        let text = std::fs::read_to_string(dir.path().join("2022-03-10.desktop.test.log")).unwrap();
        assert_eq!(text, format!("one{EOL}"));
    }
}
