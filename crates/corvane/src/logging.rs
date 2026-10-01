//! tracing setup: stderr in debug builds (Android: logcat, always), daily-rolled
//! file in ~/Library/Logs/Corvane.

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

pub fn init() -> Option<WorkerGuard> {
    let filter = EnvFilter::try_from_env("CORVANE_LOG").unwrap_or_else(|_| {
        // Android: every line is a write to logcat on the thread that
        // logs; a release keeps to `info` (a refresh alone logs a dozen
        // git commands at `debug`)
        EnvFilter::new(if cfg!(target_os = "android") && !cfg!(debug_assertions) {
            "info"
        } else {
            "info,corvane=debug"
        })
    });

    let logs_dir = corvane_platform::paths::logs_dir();
    let file_layer = std::fs::create_dir_all(&logs_dir).ok().map(|_| {
        let appender = tracing_appender::rolling::daily(&logs_dir, "corvane.log");
        let (writer, guard) = tracing_appender::non_blocking(appender);
        (fmt::layer().with_ansi(false).with_writer(writer), guard)
    });

    let registry = tracing_subscriber::registry().with(filter);
    // Android has no stderr to read: logcat takes its place, in every build
    #[cfg(target_os = "android")]
    let registry = registry.with(
        fmt::layer()
            .with_ansi(false)
            .without_time()
            .with_writer(logcat::Writer::default),
    );
    match file_layer {
        Some((layer, guard)) => {
            registry
                .with(layer)
                .with(
                    (cfg!(debug_assertions) && !cfg!(target_os = "android"))
                        .then(|| fmt::layer().with_writer(std::io::stderr)),
                )
                .init();
            Some(guard)
        }
        None => {
            registry
                .with(
                    (!cfg!(target_os = "android"))
                        .then(|| fmt::layer().with_writer(std::io::stderr)),
                )
                .init();
            None
        }
    }
}

/// Lines for `adb logcat -s corvane`.
#[cfg(target_os = "android")]
mod logcat {
    use std::ffi::{CString, c_char, c_int};
    use std::io;

    const ANDROID_LOG_INFO: c_int = 4;

    #[link(name = "log")]
    unsafe extern "C" {
        fn __android_log_write(prio: c_int, tag: *const c_char, text: *const c_char) -> c_int;
    }

    /// Collects one formatted event and writes it when dropped.
    #[derive(Default)]
    pub struct Writer(Vec<u8>);

    impl io::Write for Writer {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl Drop for Writer {
        fn drop(&mut self) {
            let mut text = std::mem::take(&mut self.0);
            text.retain(|byte| *byte != 0);
            let Ok(text) = CString::new(text) else {
                return;
            };
            // SAFETY: both strings are NUL-terminated and live for the call
            unsafe { __android_log_write(ANDROID_LOG_INFO, c"corvane".as_ptr(), text.as_ptr()) };
        }
    }
}
