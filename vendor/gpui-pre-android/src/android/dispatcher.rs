//! Android task dispatcher.
//!
//! * Foreground tasks are queued and the main thread's `ALooper` is woken, so
//!   `AndroidApp::poll_events` returns and the event loop runs them on the
//!   thread that owns the `ANativeWindow` and the input queue.
//! * Background tasks run on a thread pool fed by GPUI's priority queue.
//! * Delayed tasks wait on a timer thread.
//!
//! Ported from gpui-mobile's `src/android/dispatcher.rs`. gpui-pre 0.3.7
//! differences: priorities are honoured, the timer has its own thread
//! (gpui-mobile fired delayed tasks from a polling main loop, which no longer
//! spins), the profiler is told about every task, and the wake-up is
//! android-activity's waker instead of a pipe with a looper callback, which
//! that crate reports as a spurious poll result.

use std::{
    cmp::Reverse,
    collections::BinaryHeap,
    sync::{Arc, Condvar, Mutex},
    thread,
    time::{Duration, Instant},
};

use android_activity::{AndroidApp, AndroidAppWaker};
use gpui::{
    profiler, PlatformDispatcher, Priority, PriorityQueueReceiver, PriorityQueueSender,
    RunnableVariant,
};

const MIN_THREADS: usize = 2;

pub(crate) fn run(runnable: RunnableVariant) {
    let location = runnable.metadata().location;
    let spawned = runnable.metadata().spawned;
    profiler::update_running_task(spawned, location);
    runnable.run();
    profiler::save_task_timing();
}

struct Timer {
    due: Instant,
    sequence: u64,
    runnable: RunnableVariant,
}

impl PartialEq for Timer {
    fn eq(&self, other: &Self) -> bool {
        self.due == other.due && self.sequence == other.sequence
    }
}
impl Eq for Timer {}
impl PartialOrd for Timer {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Timer {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.due, self.sequence).cmp(&(other.due, other.sequence))
    }
}

#[derive(Default)]
struct Timers {
    heap: Mutex<(BinaryHeap<Reverse<Timer>>, u64)>,
    changed: Condvar,
}

pub struct AndroidDispatcher {
    main_sender: PriorityQueueSender<RunnableVariant>,
    waker: AndroidAppWaker,
    background_sender: PriorityQueueSender<RunnableVariant>,
    timers: Arc<Timers>,
    main_thread_id: thread::ThreadId,
}

impl AndroidDispatcher {
    /// Must be called on the thread `android_main` runs on. Returns the
    /// dispatcher and the foreground queue the event loop drains.
    pub(crate) fn new(app: &AndroidApp) -> (Arc<Self>, PriorityQueueReceiver<RunnableVariant>) {
        let (main_sender, main_receiver) = PriorityQueueReceiver::new();

        let (background_sender, background_receiver) = PriorityQueueReceiver::new();
        let threads =
            thread::available_parallelism().map_or(MIN_THREADS, |n| n.get().max(MIN_THREADS));
        for i in 0..threads {
            let receiver: PriorityQueueReceiver<RunnableVariant> = background_receiver.clone();
            thread::Builder::new()
                .name(format!("Worker-{i}"))
                .spawn(move || {
                    for runnable in receiver.iter() {
                        run(runnable);
                    }
                })
                .expect("failed to spawn a worker thread");
        }

        let timers = Arc::new(Timers::default());
        thread::Builder::new()
            .name("Timer".to_owned())
            .spawn({
                let timers = timers.clone();
                move || timer_loop(&timers)
            })
            .expect("failed to spawn the timer thread");

        let dispatcher = Arc::new(Self {
            main_sender,
            waker: app.create_waker(),
            background_sender,
            timers,
            main_thread_id: thread::current().id(),
        });
        (dispatcher, main_receiver)
    }
}

fn timer_loop(timers: &Timers) {
    let mut guard = timers.heap.lock().unwrap_or_else(|e| e.into_inner());
    loop {
        let now = Instant::now();
        match guard.0.peek() {
            Some(Reverse(timer)) if timer.due <= now => {
                let Some(Reverse(timer)) = guard.0.pop() else {
                    continue;
                };
                drop(guard);
                run(timer.runnable);
                guard = timers.heap.lock().unwrap_or_else(|e| e.into_inner());
            }
            Some(Reverse(timer)) => {
                let wait = timer.due - now;
                guard = timers
                    .changed
                    .wait_timeout(guard, wait)
                    .unwrap_or_else(|e| e.into_inner())
                    .0;
            }
            None => {
                guard = timers
                    .changed
                    .wait(guard)
                    .unwrap_or_else(|e| e.into_inner());
            }
        }
    }
}

impl PlatformDispatcher for AndroidDispatcher {
    fn is_main_thread(&self) -> bool {
        thread::current().id() == self.main_thread_id
    }

    fn dispatch(&self, runnable: RunnableVariant, priority: Priority) {
        if let Err(err) = self.background_sender.send(priority, runnable) {
            // the pool is gone (shutdown); dropping would cancel the task
            std::mem::forget(err);
        }
    }

    fn dispatch_on_main_thread(&self, runnable: RunnableVariant, priority: Priority) {
        match self.main_sender.send(priority, runnable) {
            Ok(()) => self.waker.wake(),
            // The runnable may wrap a `!Send` future, which must not be
            // dropped on this thread; the app is shutting down anyway.
            Err(err) => std::mem::forget(err),
        }
    }

    fn dispatch_after(&self, duration: Duration, runnable: RunnableVariant) {
        let mut guard = self.timers.heap.lock().unwrap_or_else(|e| e.into_inner());
        guard.1 += 1;
        let sequence = guard.1;
        guard.0.push(Reverse(Timer {
            due: Instant::now() + duration,
            sequence,
            runnable,
        }));
        drop(guard);
        self.timers.changed.notify_one();
    }

    fn spawn_realtime(&self, f: Box<dyn FnOnce() + Send>) {
        thread::Builder::new()
            .name("Realtime".to_owned())
            .spawn(f)
            .expect("failed to spawn a realtime thread");
    }
}
