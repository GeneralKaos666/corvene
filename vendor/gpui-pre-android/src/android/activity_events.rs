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
