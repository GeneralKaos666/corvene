//! Events the activity's Java side hands to the native thread.
//!
//! A `NativeActivity` gets raw key and motion events but no text: the input
//! method only talks to a view with an `InputConnection`. The application's
//! activity provides one and forwards what the keyboard does through
//! [`post`] (from the Java UI thread); the event loop drains the queue.
//!
//! Not in gpui-mobile, which took text from key events alone.

use std::sync::Mutex;

/// Something the Java side reports.
#[derive(Debug, Clone)]
pub enum ActivityEvent {
    /// `InputConnection.commitText`: text to insert.
    CommitText(String),
    /// `InputConnection.setComposingText`: the text being composed, shown
    /// marked until it is committed.
    SetComposingText(String),
    /// `InputConnection.finishComposingText`.
    FinishComposing,
    /// `InputConnection.deleteSurroundingText`, in characters.
    DeleteSurrounding { before: usize, after: usize },
    /// `InputConnection.sendKeyEvent`: a key of the on-screen keyboard
    /// (Backspace, Enter, arrows). `key_code` and `meta_state` are
    /// `android.view.KeyEvent` values, `unicode` is `getUnicodeChar()`.
    Key {
        key_code: i32,
        down: bool,
        meta_state: i32,
        unicode: u32,
    },
    /// A URL the activity was opened with (`onNewIntent`).
    OpenUrl(String),
    /// The window's insets changed ([`set_window_insets`]).
    InsetsChanged,
}

/// What the system bars, a display cutout and the on-screen keyboard cover
/// at each edge of the window, in device pixels: left, top, right, bottom.
static WINDOW_INSETS: Mutex<Option<[i32; 4]>> = Mutex::new(None);

/// From the activity's `OnApplyWindowInsetsListener`. In an edge-to-edge
/// window (enforced for applications that target Android 16) NativeActivity's
/// content rectangle is the whole window and the keyboard no longer resizes
/// it; these insets say what is covered instead.
pub fn set_window_insets(left: i32, top: i32, right: i32, bottom: i32) {
    *WINDOW_INSETS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some([left, top, right, bottom]);
    post(ActivityEvent::InsetsChanged);
}

pub(crate) fn window_insets() -> [i32; 4] {
    WINDOW_INSETS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .unwrap_or_default()
}

static QUEUE: Mutex<Vec<ActivityEvent>> = Mutex::new(Vec::new());

/// Queues `event` for the native thread and wakes it. Callable from any
/// thread.
pub fn post(event: ActivityEvent) {
    QUEUE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push(event);
    if let Some(app) = super::jni::android_app() {
        app.create_waker().wake();
    }
}

pub(crate) fn drain() -> Vec<ActivityEvent> {
    std::mem::take(
        &mut *QUEUE
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()),
    )
}

type SoftKeyboardHandler = Box<dyn Fn(bool) + Send + Sync>;

static SOFT_KEYBOARD: Mutex<Option<SoftKeyboardHandler>> = Mutex::new(None);

/// Lets the application show (`true`) and hide the on-screen keyboard for
/// its input view. Without a handler `ANativeActivity_showSoftInput` is used.
pub fn set_soft_keyboard_handler(handler: impl Fn(bool) + Send + Sync + 'static) {
    *SOFT_KEYBOARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Box::new(handler));
}

pub(crate) fn show_soft_keyboard(show: bool) {
    let handler = SOFT_KEYBOARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    match handler.as_ref() {
        Some(handler) => handler(show),
        None => {
            if let Some(app) = super::jni::android_app() {
                if show {
                    app.show_soft_input(true);
                } else {
                    app.hide_soft_input(false);
                }
            }
        }
    }
}

/// Answers a folder or file prompt: the picked path, or `None` when the user
/// cancelled or nothing usable was picked.
pub type PathPromptReply = Box<dyn FnOnce(Option<std::path::PathBuf>) + Send>;

type PathPromptHandler = Box<dyn Fn(gpui::PathPromptOptions, PathPromptReply) + Send + Sync>;

static PATH_PROMPT: Mutex<Option<PathPromptHandler>> = Mutex::new(None);

/// Lets the application answer `Platform::prompt_for_paths`. Android's
/// pickers return content URIs, not paths, and what a picked document may be
/// used for is the application's business. Without a handler every prompt
/// answers `None`.
pub fn set_path_prompt_handler(
    handler: impl Fn(gpui::PathPromptOptions, PathPromptReply) + Send + Sync + 'static,
) {
    *PATH_PROMPT
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Box::new(handler));
}

pub(crate) fn prompt_for_paths(
    options: gpui::PathPromptOptions,
) -> futures::channel::oneshot::Receiver<anyhow::Result<Option<Vec<std::path::PathBuf>>>> {
    let (tx, rx) = futures::channel::oneshot::channel();
    let handler = PATH_PROMPT
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    match handler.as_ref() {
        Some(handler) => handler(
            options,
            Box::new(move |path| {
                tx.send(Ok(path.map(|path| vec![path]))).ok();
            }),
        ),
        None => {
            tx.send(Ok(None)).ok();
        }
    }
    rx
}

type UrlHandler = Box<dyn Fn(&str) -> bool + Send + Sync>;

static URL_HANDLER: Mutex<Option<UrlHandler>> = Mutex::new(None);

/// Lets the application open some URLs itself (`Platform::open_url`): the
/// handler returns true for a URL it took. Everything else goes to an
/// `ACTION_VIEW` intent.
pub fn set_url_handler(handler: impl Fn(&str) -> bool + Send + Sync + 'static) {
    *URL_HANDLER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Box::new(handler));
}

pub(crate) fn open_url(url: &str) {
    let handled = URL_HANDLER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_ref()
        .is_some_and(|handler| handler(url));
    if !handled {
        super::jni::open_url(url);
    }
}

type PathOpener = Box<dyn Fn(&std::path::Path, bool) + Send + Sync>;

static PATH_OPENER: Mutex<Option<PathOpener>> = Mutex::new(None);

/// Lets the application implement `Platform::open_with_system` (`reveal`
/// false) and `Platform::reveal_path` (`reveal` true), which need intents
/// only it can build.
pub fn set_path_opener(opener: impl Fn(&std::path::Path, bool) + Send + Sync + 'static) {
    *PATH_OPENER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Box::new(opener));
}

pub(crate) fn open_path(path: &std::path::Path, reveal: bool) {
    if let Some(opener) = PATH_OPENER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_ref()
    {
        opener(path, reveal);
    }
}
