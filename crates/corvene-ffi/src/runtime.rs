//! The run loop and the host services over the Kotlin callbacks.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use corvene_core::host::{HostServices, PathPromptOptions, Task};

use crate::api::HostEvents;

/// `HostServices` that ask Kotlin. A path prompt is a request id Kotlin
/// answers with `Corvene::paths_picked`.
pub struct Services {
    events: Arc<dyn HostEvents>,
    next_request: AtomicU64,
    pending: Mutex<HashMap<u64, async_channel::Sender<Option<Vec<PathBuf>>>>>,
}

impl Services {
    pub fn new(events: Arc<dyn HostEvents>) -> Self {
        Services {
            events,
            next_request: AtomicU64::new(1),
            pending: Mutex::new(HashMap::new()),
        }
    }

    /// Kotlin's answer to `HostEvents::pick_paths`.
    pub fn paths_picked(&self, request: u64, paths: Option<Vec<String>>) {
        let sender = self
            .pending
            .lock()
            .ok()
            .and_then(|mut p| p.remove(&request));
        if let Some(sender) = sender {
            let _ = sender.try_send(paths.map(|p| p.into_iter().map(PathBuf::from).collect()));
        }
    }
}

impl HostServices for Services {
    fn prompt_for_paths(&self, options: PathPromptOptions) -> Task<Option<Vec<PathBuf>>> {
        let request = self.next_request.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = async_channel::bounded(1);
        if let Ok(mut pending) = self.pending.lock() {
            pending.insert(request, tx);
        }
        self.events.pick_paths(
            request,
            options.directories,
            options.multiple,
            options.prompt,
        );
        Task::from_future(async move { rx.recv().await.unwrap_or(None) })
    }

    fn write_to_clipboard(&self, text: String) {
        self.events.write_clipboard(text);
    }

    fn open_url(&self, url: &str) {
        self.events.open_url(url.to_string());
    }

    fn reveal_path(&self, path: &Path) {
        self.events
            .open_path(path.to_string_lossy().into_owned(), true);
    }

    fn open_with_system(&self, path: &Path) {
        self.events
            .open_path(path.to_string_lossy().into_owned(), false);
    }

    fn quit(&self) {
        self.events.quit();
    }
}

/// Logging to logcat (Android) or stderr, once per process.
pub fn init_logging() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        #[cfg(target_os = "android")]
        {
            android_logger::init_once(
                android_logger::Config::default()
                    .with_max_level(log_level())
                    .with_tag("corvene"),
            );
            let _ = tracing::subscriber::set_global_default(
                tracing_subscriber::fmt()
                    .with_env_filter(
                        tracing_subscriber::EnvFilter::try_from_env("CORVENE_LOG")
                            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
                    )
                    .with_writer(LogcatWriter)
                    .without_time()
                    .with_ansi(false)
                    .finish(),
            );
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = tracing_subscriber::fmt()
                .with_env_filter(
                    tracing_subscriber::EnvFilter::try_from_env("CORVENE_LOG")
                        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
                )
                .try_init();
        }
    });
}

#[cfg(target_os = "android")]
fn log_level() -> log::LevelFilter {
    match std::env::var("CORVENE_LOG").as_deref() {
        Ok("trace") => log::LevelFilter::Trace,
        Ok("debug") => log::LevelFilter::Debug,
        _ => log::LevelFilter::Info,
    }
}

/// `tracing` lines to logcat through the `log` crate.
#[cfg(target_os = "android")]
struct LogcatWriter;

#[cfg(target_os = "android")]
impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogcatWriter {
    type Writer = LogcatLine;
    fn make_writer(&'a self) -> Self::Writer {
        LogcatLine(Vec::new())
    }
}

#[cfg(target_os = "android")]
struct LogcatLine(Vec<u8>);

#[cfg(target_os = "android")]
impl std::io::Write for LogcatLine {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(target_os = "android")]
impl Drop for LogcatLine {
    fn drop(&mut self) {
        let line = String::from_utf8_lossy(&self.0);
        let line = line.trim_end();
        if !line.is_empty() {
            log::info!(target: "corvene", "{line}");
        }
    }
}
