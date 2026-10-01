//! Display refresh ticks from `AChoreographer`, so frames start with the
//! display's own cadence (60, 90 or 120 Hz) instead of a 16.7 ms timer.
//!
//! A choreographer delivers its callbacks through the looper of the thread
//! that asked. The main thread's looper belongs to android-activity, which
//! logs an error for every callback it did not register itself, so the
//! choreographer lives on a small thread of its own; each tick sets a flag
//! and wakes the event loop.
//!
//! Not in gpui-mobile, which rendered from a polling loop.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};

use android_activity::AndroidAppWaker;

struct Shared {
    /// The event loop wants the next tick.
    requested: AtomicBool,
    /// A callback is posted and has not fired yet.
    posted: AtomicBool,
    /// A tick arrived that the event loop has not consumed.
    ticked: AtomicBool,
    waker: AndroidAppWaker,
}

pub(crate) struct Vsync {
    shared: Arc<Shared>,
    looper: Looper,
}

struct Looper(*mut ndk_sys::ALooper);

// SAFETY: `ALooper_wake` may be called from any thread; the pointer holds a
// reference (`ALooper_acquire`) for as long as this value lives
unsafe impl Send for Looper {}

unsafe extern "C" fn tick(_frame_time_nanos: std::ffi::c_long, data: *mut c_void) {
    // SAFETY: `data` is the `Shared` the thread below keeps alive forever
    let shared = unsafe { &*data.cast::<Shared>() };
    shared.posted.store(false, Ordering::SeqCst);
    shared.ticked.store(true, Ordering::SeqCst);
    shared.waker.wake();
}

impl Vsync {
    /// Starts the choreographer thread; `None` when the system gives no
    /// choreographer (the event loop then keeps its timer).
    pub(crate) fn start(waker: AndroidAppWaker) -> Option<Self> {
        let shared = Arc::new(Shared {
            requested: AtomicBool::new(false),
            posted: AtomicBool::new(false),
            ticked: AtomicBool::new(false),
            waker,
        });
        let (sender, receiver) = mpsc::channel::<Option<Looper>>();
        let thread_shared = shared.clone();
        std::thread::Builder::new()
            .name("vsync".into())
            .spawn(move || {
                // SAFETY: plain NDK calls on this thread's own looper and
                // choreographer; `tick` gets a pointer that outlives it
                unsafe {
                    let looper = ndk_sys::ALooper_prepare(0);
                    let choreographer = ndk_sys::AChoreographer_getInstance();
                    if looper.is_null() || choreographer.is_null() {
                        let _ = sender.send(None);
                        return;
                    }
                    ndk_sys::ALooper_acquire(looper);
                    let _ = sender.send(Some(Looper(looper)));
                    let data = Arc::into_raw(thread_shared.clone())
                        .cast_mut()
                        .cast::<c_void>();
                    loop {
                        if thread_shared.requested.swap(false, Ordering::SeqCst)
                            && !thread_shared.posted.swap(true, Ordering::SeqCst)
                        {
                            ndk_sys::AChoreographer_postFrameCallback(
                                choreographer,
                                Some(tick),
                                data,
                            );
                        }
                        // returns for a wake (`request`) and after callbacks
                        ndk_sys::ALooper_pollOnce(
                            -1,
                            std::ptr::null_mut(),
                            std::ptr::null_mut(),
                            std::ptr::null_mut(),
                        );
                    }
                }
            })
            .ok()?;
        let looper = receiver.recv().ok()??;
        Some(Self { shared, looper })
    }

    /// Asks for the next refresh tick (once, however often it is called
    /// before the tick).
    pub(crate) fn request(&self) {
        if self.shared.posted.load(Ordering::SeqCst) {
            return;
        }
        self.shared.requested.store(true, Ordering::SeqCst);
        // SAFETY: see `Looper`
        unsafe { ndk_sys::ALooper_wake(self.looper.0) };
    }

    /// Whether a tick arrived since the last call.
    pub(crate) fn take_tick(&self) -> bool {
        self.shared.ticked.swap(false, Ordering::SeqCst)
    }
}
