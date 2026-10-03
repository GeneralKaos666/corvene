//! One Corvene per data folder on Windows - GHD's
//! `app.requestSingleInstanceLock()` and `second-instance` event
//! (`main-process/main.ts`). The same contract as the Linux module
//! (`single_instance.rs`), over a named pipe instead of a Unix socket.
//!
//! The first instance creates `\\.\pipe\corvene-instance-<hash of the data
//! folder>` (`FILE_FLAG_FIRST_PIPE_INSTANCE`, so only one process can), which
//! harness instances with their own `CORVENE_DATA_DIR` do not share. A later
//! launch opens the pipe, sends the `x-corvene://` / `x-corvene-auth://`
//! URLs from its command line (the registry's `"%1"`, the command line
//! tool) or, without any, asks for the window, and exits. A pipe goes away
//! with its process: nothing is left behind by a crash.
//!
//! Wire format: one message per line, `url <url>` or `focus`.

use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::os::windows::io::FromRawHandle;

use tracing::{debug, info, warn};
use windows::Win32::Foundation::{
    ERROR_NO_DATA, ERROR_PIPE_CONNECTED, HANDLE, INVALID_HANDLE_VALUE,
};
use windows::Win32::Storage::FileSystem::{
    FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAGS_AND_ATTRIBUTES, PIPE_ACCESS_INBOUND,
};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE,
    PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_WAIT,
};
use windows::core::HSTRING;

/// What a later launch asked the running instance for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    Url(String),
    Focus,
}

/// The URL schemes Corvene handles (`possibleProtocols`).
pub const SCHEMES: &[&str] = &["x-corvene", "x-corvene-auth"];

/// The command line arguments that are Corvene URLs (GHD's
/// `handlePossibleProtocolLauncherArgs`, restricted to our schemes so no
/// argument is mistaken for one).
pub fn url_arguments(args: impl IntoIterator<Item = String>) -> Vec<String> {
    args.into_iter()
        .filter(|arg| {
            SCHEMES.iter().any(|scheme| {
                arg.strip_prefix(scheme)
                    .is_some_and(|r| r.starts_with("://"))
            })
        })
        .collect()
}

/// `\\.\pipe\corvene-instance-<hash of the data folder>`. Pipe names are
/// per machine; the data folder is per user, so the hash is too.
pub fn pipe_name() -> String {
    let data = crate::paths::app_support_dir();
    format!(
        r"\\.\pipe\corvene-instance-{:016x}",
        fnv1a(data.as_os_str().as_encoded_bytes())
    )
}

/// FNV-1a: stable across builds, unlike `DefaultHasher`.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &b| {
        (hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// The outcome of [`claim`].
pub enum Claim {
    /// This is the first instance; serve later launches with [`Listener::serve`].
    First(Listener),
    /// A running instance took the messages; this process should exit.
    Forwarded,
}

pub struct Listener {
    pipe: Option<Pipe>,
    name: String,
}

/// The server end of the pipe.
struct Pipe(HANDLE);

// SAFETY: a pipe handle may be used from any thread; the listener thread is
// its only user
unsafe impl Send for Pipe {}

/// Hand `urls` (or a focus request) to a running instance, else become the
/// instance. Errors creating the pipe are logged and leave this process
/// running on its own, as GHD does when the lock cannot be taken.
pub fn claim(urls: &[String]) -> Claim {
    claim_at(&pipe_name(), urls)
}

fn claim_at(name: &str, urls: &[String]) -> Claim {
    match create_pipe(name, true) {
        Ok(pipe) => Claim::First(Listener {
            pipe: Some(pipe),
            name: name.to_string(),
        }),
        // the pipe exists: another instance owns it
        Err(_) => match send(name, urls) {
            Ok(()) => {
                info!(urls = urls.len(), "handed over to the running instance");
                Claim::Forwarded
            }
            Err(err) => {
                warn!(%err, "the running instance did not take the launch");
                Claim::First(Listener {
                    pipe: None,
                    name: name.to_string(),
                })
            }
        },
    }
}

/// An inbound byte pipe that only this user's processes on this machine can
/// write to (the default security descriptor grants others read access at
/// most). `first`: fail when the name is taken.
fn create_pipe(name: &str, first: bool) -> windows::core::Result<Pipe> {
    let mut open_mode = PIPE_ACCESS_INBOUND;
    if first {
        open_mode |= FILE_FLAG_FIRST_PIPE_INSTANCE;
    }
    // SAFETY: plain handle creation; the name outlives the call
    let handle = unsafe {
        CreateNamedPipeW(
            &HSTRING::from(name),
            FILE_FLAGS_AND_ATTRIBUTES(open_mode.0),
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            0,
            4096,
            0,
            None,
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(windows::core::Error::from_thread());
    }
    Ok(Pipe(handle))
}

fn send(name: &str, urls: &[String]) -> std::io::Result<()> {
    let mut text = String::new();
    if urls.is_empty() {
        text.push_str("focus\n");
    }
    for url in urls {
        // a URL is one line; anything else would split the message
        if !url.contains('\n') {
            text.push_str(&format!("url {url}\n"));
        }
    }
    // the single pipe instance is busy while another launch talks to it
    let mut attempts = 0;
    let mut pipe = loop {
        match std::fs::OpenOptions::new().write(true).open(name) {
            Ok(pipe) => break pipe,
            Err(err) if attempts < 50 => {
                debug!(%err, "the instance pipe is busy");
                attempts += 1;
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            Err(err) => return Err(err),
        }
    };
    pipe.write_all(text.as_bytes())?;
    pipe.flush()
}

fn parse(line: &str) -> Option<Message> {
    match line.trim_end() {
        "focus" => Some(Message::Focus),
        other => {
            let url = other.strip_prefix("url ")?;
            url_arguments([url.to_string()]).pop().map(Message::Url)
        }
    }
}

impl Listener {
    /// Accept later launches on a background thread; each message goes to
    /// `on_message` (on that thread).
    pub fn serve(self, on_message: impl Fn(Message) + Send + 'static) {
        let Some(pipe) = self.pipe else {
            return;
        };
        debug!(name = %self.name, "single-instance pipe");
        let spawned = std::thread::Builder::new()
            .name("single-instance".into())
            .spawn(move || {
                let pipe = pipe;
                loop {
                    // SAFETY: waits for a client on the handle this thread owns
                    let connected = unsafe { ConnectNamedPipe(pipe.0, None) };
                    // a client that connected before this call
                    // (`ERROR_PIPE_CONNECTED`), or one that has already
                    // written and gone (`ERROR_NO_DATA`), is still read
                    if let Err(err) = connected
                        && err.code() != ERROR_PIPE_CONNECTED.to_hresult()
                        && err.code() != ERROR_NO_DATA.to_hresult()
                    {
                        warn!(%err, "the single-instance pipe stopped");
                        return;
                    }
                    // SAFETY: the `File` only borrows the handle: it is taken
                    // back below before it could be closed
                    let file = unsafe { File::from_raw_handle(pipe.0.0) };
                    let mut reader = BufReader::new(file);
                    let mut line = String::new();
                    while reader.read_line(&mut line).is_ok_and(|n| n > 0) {
                        match parse(&line) {
                            Some(message) => on_message(message),
                            None => warn!("ignored an unknown single-instance message"),
                        }
                        line.clear();
                    }
                    std::mem::forget(reader.into_inner());
                    // SAFETY: ends this client's connection on our handle
                    let _ = unsafe { DisconnectNamedPipe(pipe.0) };
                }
            });
        if let Err(err) = spawned {
            warn!(%err, "could not start the single-instance listener");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn picks_corvene_urls_only() {
        let args = [
            "--hidden",
            "x-corvene://openLocalRepo/tmp",
            "https://example.com",
            "x-corvene-auth://oauth?code=1",
            "x-corveneevil://x",
        ]
        .map(String::from);
        assert_eq!(
            url_arguments(args),
            [
                "x-corvene://openLocalRepo/tmp",
                "x-corvene-auth://oauth?code=1"
            ]
        );
    }

    #[test]
    fn a_second_launch_hands_over() {
        let name = format!(r"\\.\pipe\corvene-instance-test-{}", std::process::id());
        let Claim::First(listener) = claim_at(&name, &[]) else {
            panic!("the first launch must become the instance");
        };
        let (tx, rx) = mpsc::channel();
        listener.serve(move |m| {
            let _ = tx.send(m);
        });
        let url = "x-corvene://openLocalRepo/tmp".to_string();
        assert!(matches!(
            claim_at(&name, std::slice::from_ref(&url)),
            Claim::Forwarded
        ));
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(5)).unwrap(),
            Message::Url(url)
        );
        assert!(matches!(claim_at(&name, &[]), Claim::Forwarded));
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(5)).unwrap(),
            Message::Focus
        );
    }

    #[test]
    fn pipes_differ_per_data_folder() {
        assert_ne!(fnv1a(b"/a"), fnv1a(b"/b"));
    }
}
