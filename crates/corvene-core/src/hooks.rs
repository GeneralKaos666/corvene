//! The app's side of GHD's hooks interception (`corvene_git::hooks`):
//! `AppStore.onHookProgress` (the hook progress under the commit button),
//! `AppStore.onHookFailure` (the HookFailed dialog, whose Abort makes the
//! operation end without an error) and the commit's
//! `subscribeToCommitOutput` (the Committing changes dialog).

use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use corvene_git::hooks::{HookCallbacks, HookFailureResolution, HookProgress};
use corvene_git::{TerminalOutput, TerminalOutputCallback};

use crate::dispatcher::Dispatcher;
use crate::host::{AsyncCtx, Host};
use crate::state::Popup;

/// The HookFailed dialog's answer to the hook waiting for it (GHD's
/// `resolve`). Closing the dialog any other way aborts.
#[derive(Clone)]
pub struct HookFailureReply(async_channel::Sender<HookFailureResolution>);

impl HookFailureReply {
    pub fn send(&self, resolution: HookFailureResolution) {
        let _ = self.0.try_send(resolution);
    }
}

impl PartialEq for HookFailureReply {
    fn eq(&self, other: &Self) -> bool {
        self.0.same_channel(&other.0)
    }
}

impl Eq for HookFailureReply {}

impl std::fmt::Debug for HookFailureReply {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HookFailureReply")
    }
}

/// The output of the commit in progress (GHD `subscribeToCommitOutput`):
/// what git and its hooks wrote so far, which the Committing changes dialog
/// keeps showing once the commit is done.
#[derive(Clone, Default)]
pub struct CommitOutput(Arc<Mutex<Vec<u8>>>);

/// GHD's `terminalCapacity`: what a dialog opened late gets of the output.
const COMMIT_OUTPUT_CAPACITY: usize = 256 * 1024;

impl CommitOutput {
    /// Everything written so far, from `offset` on; the offset to read on
    /// from next time.
    pub fn read_from(&self, offset: usize) -> (Vec<u8>, usize) {
        let bytes = self.0.lock().map(|b| b.clone()).unwrap_or_default();
        let from = offset.min(bytes.len());
        (bytes[from..].to_vec(), bytes.len())
    }

    /// The last [`COMMIT_OUTPUT_CAPACITY`] bytes; where to read on from.
    pub fn tail(&self) -> (Vec<u8>, usize) {
        let bytes = self.0.lock().map(|b| b.clone()).unwrap_or_default();
        let from = bytes.len().saturating_sub(COMMIT_OUTPUT_CAPACITY);
        (bytes[from..].to_vec(), bytes.len())
    }

    fn push(&self, chunk: &[u8]) {
        if let Ok(mut bytes) = self.0.lock() {
            bytes.extend_from_slice(chunk);
        }
    }
}

impl PartialEq for CommitOutput {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for CommitOutput {}

impl std::fmt::Debug for CommitOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CommitOutput")
    }
}

enum HookEvent {
    Progress(HookProgress),
    Failure {
        hook_name: String,
        output: Vec<u8>,
        reply: HookFailureReply,
    },
}

/// The callbacks one operation hands git, and whether the user aborted it
/// in the HookFailed dialog (GHD's `aborted`).
pub struct HookUi {
    pub callbacks: HookCallbacks,
    pub aborted: Arc<AtomicBool>,
}

impl HookUi {
    pub fn aborted(&self) -> bool {
        self.aborted.load(Ordering::SeqCst)
    }
}

/// GHD `onHookFailure(() => (aborted = true))`, with `onHookProgress` for
/// the repository `id` when `progress` (only the commit reports progress).
pub fn hook_ui(id: u64, progress: bool, cx: &mut dyn Host) -> HookUi {
    let (tx, rx) = async_channel::unbounded::<HookEvent>();
    let aborted = Arc::new(AtomicBool::new(false));
    let on_hook_progress: Option<corvene_git::hooks::HookProgressFn> = progress.then(|| {
        let tx = tx.clone();
        Arc::new(move |p: HookProgress| {
            let _ = tx.send_blocking(HookEvent::Progress(p));
        }) as corvene_git::hooks::HookProgressFn
    });
    let on_hook_failure: corvene_git::hooks::HookFailureFn = {
        let aborted = aborted.clone();
        Arc::new(move |hook_name: &str, output: Vec<u8>| {
            let (reply_tx, reply_rx) = async_channel::bounded(1);
            let sent = tx.send_blocking(HookEvent::Failure {
                hook_name: hook_name.to_string(),
                output,
                reply: HookFailureReply(reply_tx),
            });
            let resolution = match sent {
                Ok(()) => reply_rx
                    .recv_blocking()
                    .unwrap_or(HookFailureResolution::Abort),
                Err(_) => HookFailureResolution::Abort,
            };
            if resolution == HookFailureResolution::Abort {
                aborted.store(true, Ordering::SeqCst);
            }
            resolution
        })
    };
    // ends once the operation dropped its callbacks
    cx.spawn(async move |cx: &mut AsyncCtx| {
        while let Ok(event) = rx.recv().await {
            cx.update(|cx| match event {
                HookEvent::Progress(p) => {
                    Dispatcher::state(cx).update(cx, |s, cx| {
                        s.repo_state_mut(id).hook_progress = Some(p);
                        cx.notify();
                    });
                }
                HookEvent::Failure {
                    hook_name,
                    output,
                    reply,
                } => Dispatcher::show_popup(
                    Popup::HookFailed {
                        hook_name,
                        terminal_output: String::from_utf8_lossy(&output).into_owned(),
                        reply,
                    },
                    cx,
                ),
            });
        }
    })
    .detach();
    HookUi {
        callbacks: HookCallbacks {
            on_hook_progress,
            on_hook_failure: Some(on_hook_failure),
        },
        aborted,
    }
}

/// Where a commit's terminal output goes: [`commit_output_callback`] on the
/// thread that runs git, [`CommitOutputSink::listen`] on the foreground.
pub struct CommitOutputSink {
    tx: async_channel::Sender<Option<Vec<u8>>>,
    rx: async_channel::Receiver<Option<Vec<u8>>>,
}

impl CommitOutputSink {
    pub fn new() -> Self {
        let (tx, rx) = async_channel::unbounded();
        Self { tx, rx }
    }

    pub fn sender(&self) -> async_channel::Sender<Option<Vec<u8>>> {
        self.tx.clone()
    }

    /// GHD's `onTerminalOutputAvailable: subscribeToCommitOutput =>
    /// repositoryStateCache.update(…)`: the repository `id` gets a
    /// [`CommitOutput`] once git runs, filled as it writes.
    pub fn listen(self, id: u64, cx: &mut dyn Host) {
        let Self { tx, rx } = self;
        drop(tx);
        cx.spawn(async move |cx: &mut AsyncCtx| {
            let mut output: Option<CommitOutput> = None;
            while let Ok(event) = rx.recv().await {
                let current = output.get_or_insert_with(|| {
                    let fresh = CommitOutput::default();
                    let shown = fresh.clone();
                    cx.update(|cx| {
                        Dispatcher::state(cx).update(cx, |s, cx| {
                            s.repo_state_mut(id).commit_output = Some(shown);
                            cx.notify();
                        })
                    });
                    fresh
                });
                if let Some(chunk) = event {
                    current.push(&chunk);
                    cx.update(|cx| Dispatcher::state(cx).update(cx, |_, cx| cx.notify()));
                }
            }
        })
        .detach();
    }
}

impl Default for CommitOutputSink {
    fn default() -> Self {
        Self::new()
    }
}

/// The `onTerminalOutputAvailable` callback for the thread that commits:
/// subscribes at once and sends every chunk to the foreground.
pub fn commit_output_callback(
    tx: async_channel::Sender<Option<Vec<u8>>>,
) -> TerminalOutputCallback {
    Rc::new(move |subscribe| {
        let _ = tx.send_blocking(None);
        let tx = tx.clone();
        // never unsubscribed: the command ends first
        let _unsubscribe = subscribe(Box::new(move |output| {
            let bytes = match output {
                TerminalOutput::Chunk(chunk) => chunk,
                TerminalOutput::Chunks(chunks) => chunks.concat(),
            };
            let _ = tx.send_blocking(Some(bytes));
        }));
    })
}
