//! A [`Host`] without a UI toolkit: one "corvene-main" thread owns the
//! `AppState` and runs the main-thread futures from an inbox, a small pool
//! of std threads runs the background futures, and one thread keeps the
//! timers. The Android library runs the dispatcher on it; the tests run
//! the dispatcher on it without a window.
//!
//! Only the loop thread touches the `LocalHost`; the futures it polls reach
//! it through a thread-local pointer that is set for the duration of each
//! poll, which is what makes [`AsyncCtx::update`] work inside them.

use std::cell::{Cell, RefCell};
use std::collections::{BinaryHeap, HashMap};
use std::future::Future;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::time::{Duration, Instant};

use super::{
    AsyncCtx, AsyncHost, Host, HostServices, LocalFuture, NoServices, SendFuture, StateCx, Task,
};
use crate::state::AppState;

type TaskId = u64;

/// What the loop thread processes.
pub enum Msg {
    /// Runs on the loop thread with the host.
    Run(Box<dyn FnOnce(&mut LocalHost) + Send>),
    /// A local future's waker fired.
    Poll(TaskId),
    /// Ends [`run`].
    Stop,
}

/// Posts work to the loop from any thread.
#[derive(Clone)]
pub struct LoopHandle {
    tx: Sender<Msg>,
}

impl LoopHandle {
    /// Fire and forget.
    pub fn post(&self, f: impl FnOnce(&mut LocalHost) + Send + 'static) {
        let _ = self.tx.send(Msg::Run(Box::new(f)));
    }

    /// Runs `f` on the loop thread and resolves to its result.
    pub fn query<R: Send + 'static>(
        &self,
        f: impl FnOnce(&mut LocalHost) -> R + Send + 'static,
    ) -> impl Future<Output = R> + Send + 'static {
        let (tx, rx) = async_channel::bounded(1);
        self.post(move |host| {
            let _ = tx.try_send(f(host));
        });
        async move {
            match rx.recv().await {
                Ok(value) => value,
                Err(_) => std::future::pending().await,
            }
        }
    }

    /// Runs `f` on the loop thread and blocks until it is done.
    pub fn query_blocking<R: Send + 'static>(
        &self,
        f: impl FnOnce(&mut LocalHost) -> R + Send + 'static,
    ) -> Option<R> {
        let (tx, rx) = mpsc::channel();
        self.post(move |host| {
            let _ = tx.send(f(host));
        });
        rx.recv().ok()
    }

    pub fn stop(&self) {
        let _ = self.tx.send(Msg::Stop);
    }
}

/// The host. Build one with [`LocalHost::new`], then hand it to [`run`] on
/// the thread that is to become the main thread.
pub struct LocalHost {
    state: Option<AppState>,
    version: u64,
    dirty: bool,
    tx: Sender<Msg>,
    tasks: RefCell<HashMap<TaskId, LocalFuture>>,
    next_task: Cell<TaskId>,
    pool: ThreadPool,
    timers: TimerThread,
    services: Arc<dyn HostServices + Send + Sync>,
    on_changed: Box<dyn FnMut(u64) + Send>,
}

impl LocalHost {
    /// `on_changed(version)` runs on the loop thread after every message
    /// that notified the state.
    pub fn new(
        services: Arc<dyn HostServices + Send + Sync>,
        on_changed: impl FnMut(u64) + Send + 'static,
    ) -> (LocalHost, Receiver<Msg>) {
        let (tx, rx) = mpsc::channel();
        let threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(2)
            .clamp(2, 4);
        let host = LocalHost {
            state: None,
            version: 0,
            dirty: false,
            tx: tx.clone(),
            tasks: RefCell::new(HashMap::new()),
            next_task: Cell::new(1),
            pool: ThreadPool::new(threads),
            timers: TimerThread::new(tx),
            services,
            on_changed: Box::new(on_changed),
        };
        (host, rx)
    }

    /// A host with no services and no observer (tests).
    pub fn bare() -> (LocalHost, Receiver<Msg>) {
        Self::new(Arc::new(NoServices), |_| {})
    }

    pub fn handle(&self) -> LoopHandle {
        LoopHandle {
            tx: self.tx.clone(),
        }
    }

    /// Bumps after every notified update.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// `true` while local futures are registered.
    pub fn has_pending_tasks(&self) -> bool {
        !self.tasks.borrow().is_empty()
    }

    fn poll_task(&mut self, id: TaskId) {
        // Taken out of the map while it runs so a spawn from inside the
        // future can register itself.
        let Some(mut fut) = self.tasks.borrow_mut().remove(&id) else {
            return;
        };
        let waker = Waker::from(Arc::new(TaskWaker {
            id,
            tx: self.tx.clone(),
        }));
        let mut cx = Context::from_waker(&waker);
        let done = with_current(self, || fut.as_mut().poll(&mut cx).is_ready());
        if !done {
            self.tasks.borrow_mut().insert(id, fut);
        }
    }

    fn after_message(&mut self) {
        if self.dirty {
            self.dirty = false;
            self.version += 1;
            let version = self.version;
            (self.on_changed)(version);
        }
    }
}

thread_local! {
    static CURRENT: Cell<*mut LocalHost> = const { Cell::new(std::ptr::null_mut()) };
}

/// Runs `f` with `CURRENT` pointing at `host` so a nested [`AsyncCtx::update`]
/// reaches it. Nested calls (an update inside a poll) keep the same pointer.
fn with_current<R>(host: &mut LocalHost, f: impl FnOnce() -> R) -> R {
    let previous = CURRENT.with(|c| c.replace(host as *mut LocalHost));
    let result = f();
    CURRENT.with(|c| c.set(previous));
    result
}

/// Runs the loop on the current thread until [`LoopHandle::stop`].
pub fn run(mut host: LocalHost, rx: Receiver<Msg>) {
    for msg in rx {
        match msg {
            Msg::Run(f) => {
                let host_ptr: *mut LocalHost = &mut host;
                with_current(&mut host, || {
                    // SAFETY: `with_current` set the pointer from this very
                    // `&mut host`, and nothing else reaches the host while
                    // `f` runs on this thread.
                    f(unsafe { &mut *host_ptr });
                });
            }
            Msg::Poll(id) => host.poll_task(id),
            Msg::Stop => break,
        }
        host.after_message();
    }
}

/// Starts the loop on a new thread named `name`; the host is built there
/// because the local futures it holds are not `Send`.
pub fn spawn_loop(
    name: &str,
    services: Arc<dyn HostServices + Send + Sync>,
    on_changed: impl FnMut(u64) + Send + 'static,
) -> std::io::Result<LoopHandle> {
    let (handle_tx, handle_rx) = mpsc::channel();
    std::thread::Builder::new()
        .name(name.to_string())
        .spawn(move || {
            let (host, rx) = LocalHost::new(services, on_changed);
            let _ = handle_tx.send(host.handle());
            run(host, rx);
        })?;
    handle_rx
        .recv()
        .map_err(|_| std::io::Error::other("the host thread ended before it started"))
}

/// Runs messages until the inbox is empty and no timer is due within
/// `settle` (tests: "run until idle").
pub fn run_until_idle(host: &mut LocalHost, rx: &Receiver<Msg>, settle: Duration) {
    loop {
        match rx.recv_timeout(settle) {
            Ok(Msg::Run(f)) => {
                let host_ptr: *mut LocalHost = host;
                with_current(host, || {
                    // SAFETY: as in `run`.
                    f(unsafe { &mut *host_ptr });
                });
            }
            Ok(Msg::Poll(id)) => host.poll_task(id),
            Ok(Msg::Stop) | Err(_) => return,
        }
        host.after_message();
    }
}

struct TaskWaker {
    id: TaskId,
    tx: Sender<Msg>,
}

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        let _ = self.tx.send(Msg::Poll(self.id));
    }
}

impl Host for LocalHost {
    fn install_state(&mut self, state: AppState) {
        self.state = Some(state);
        self.dirty = true;
    }

    fn state_ref(&self) -> &AppState {
        match &self.state {
            Some(state) => state,
            None => panic!("Dispatcher::init has not run"),
        }
    }

    fn update_state(&mut self, f: &mut dyn FnMut(&mut AppState, &mut StateCx)) {
        let Some(state) = self.state.as_mut() else {
            panic!("Dispatcher::init has not run");
        };
        let mut scx = StateCx::default();
        f(state, &mut scx);
        if scx.notified() {
            self.dirty = true;
        }
    }

    fn has_state(&self) -> bool {
        self.state.is_some()
    }

    fn spawn_local(&self, fut: LocalFuture) {
        let id = self.next_task.get();
        self.next_task.set(id + 1);
        self.tasks.borrow_mut().insert(id, fut);
        // First poll happens on a later message, never inside this call.
        let _ = self.tx.send(Msg::Poll(id));
    }

    fn spawn_background(&self, fut: SendFuture) {
        self.pool.spawn(fut);
    }

    fn timer(&self, after: Duration) -> Task<()> {
        let (tx, rx) = async_channel::bounded::<()>(1);
        self.timers.add(after, move || {
            let _ = tx.try_send(());
        });
        Task::from_future(async move {
            let _ = rx.recv().await;
        })
    }

    fn async_ctx(&self) -> AsyncCtx {
        AsyncCtx::new(LocalAsync)
    }

    fn services(&self) -> &dyn HostServices {
        self.services.as_ref()
    }

    #[cfg(feature = "gpui")]
    fn gpui_app(&mut self) -> Option<&mut gpui_kit::App> {
        None
    }
}

/// The [`AsyncHost`] of a `LocalHost`: valid only on the loop thread while
/// a message runs, which is the only place local futures are polled.
#[derive(Clone)]
struct LocalAsync;

impl AsyncHost for LocalAsync {
    fn with_host(&self, f: &mut dyn FnMut(&mut dyn Host)) {
        let ptr = CURRENT.with(|c| c.get());
        if ptr.is_null() {
            panic!("AsyncCtx used off the host's loop thread");
        }
        // SAFETY: `CURRENT` is set by `with_current` around the poll or
        // message that owns `&mut LocalHost`; this runs inside it on the
        // same thread, and the host's own methods never re-enter here with
        // another live borrow.
        let host: &mut LocalHost = unsafe { &mut *ptr };
        f(host);
    }

    fn box_clone(&self) -> Box<dyn AsyncHost> {
        Box::new(LocalAsync)
    }
}

/// Background work: each thread polls one future to completion with a
/// parking waker (every background future core spawns is blocking work
/// wrapped in `async move`).
struct ThreadPool {
    tx: Sender<SendFuture>,
}

impl ThreadPool {
    fn new(threads: usize) -> Self {
        let (tx, rx) = mpsc::channel::<SendFuture>();
        let rx = Arc::new(Mutex::new(rx));
        for i in 0..threads {
            let rx = rx.clone();
            let _ = std::thread::Builder::new()
                .name(format!("corvene-bg-{i}"))
                .spawn(move || {
                    loop {
                        let job = match rx.lock() {
                            Ok(rx) => rx.recv(),
                            Err(_) => return,
                        };
                        let Ok(fut) = job else { return };
                        block_on(fut);
                    }
                });
        }
        ThreadPool { tx }
    }

    fn spawn(&self, fut: SendFuture) {
        let _ = self.tx.send(fut);
    }
}

struct ParkWaker {
    thread: std::thread::Thread,
}

impl Wake for ParkWaker {
    fn wake(self: Arc<Self>) {
        self.thread.unpark();
    }
}

fn block_on(mut fut: SendFuture) {
    let waker = Waker::from(Arc::new(ParkWaker {
        thread: std::thread::current(),
    }));
    let mut cx = Context::from_waker(&waker);
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(()) => return,
            Poll::Pending => std::thread::park(),
        }
    }
}

struct TimerEntry {
    due: Instant,
    seq: u64,
    fire: Box<dyn FnOnce() + Send>,
}

impl PartialEq for TimerEntry {
    fn eq(&self, other: &Self) -> bool {
        self.due == other.due && self.seq == other.seq
    }
}
impl Eq for TimerEntry {}
impl PartialOrd for TimerEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for TimerEntry {
    // BinaryHeap is a max-heap: the earliest due time is the greatest.
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.due.cmp(&self.due).then(other.seq.cmp(&self.seq))
    }
}

struct TimerThread {
    inner: Arc<(Mutex<TimerState>, Condvar)>,
}

struct TimerState {
    heap: BinaryHeap<TimerEntry>,
    seq: u64,
}

impl TimerThread {
    fn new(_loop_tx: Sender<Msg>) -> Self {
        let inner = Arc::new((
            Mutex::new(TimerState {
                heap: BinaryHeap::new(),
                seq: 0,
            }),
            Condvar::new(),
        ));
        let thread_inner = inner.clone();
        let _ = std::thread::Builder::new()
            .name("corvene-timers".into())
            .spawn(move || {
                let (lock, cv) = &*thread_inner;
                let mut state = match lock.lock() {
                    Ok(s) => s,
                    Err(_) => return,
                };
                loop {
                    let now = Instant::now();
                    if let Some(entry) = state.heap.peek() {
                        if entry.due <= now {
                            if let Some(entry) = state.heap.pop() {
                                drop(state);
                                (entry.fire)();
                                state = match lock.lock() {
                                    Ok(s) => s,
                                    Err(_) => return,
                                };
                            }
                            continue;
                        }
                        let wait = entry.due - now;
                        state = match cv.wait_timeout(state, wait) {
                            Ok((s, _)) => s,
                            Err(_) => return,
                        };
                    } else {
                        state = match cv.wait(state) {
                            Ok(s) => s,
                            Err(_) => return,
                        };
                    }
                }
            });
        TimerThread { inner }
    }

    fn add(&self, after: Duration, fire: impl FnOnce() + Send + 'static) {
        let (lock, cv) = &*self.inner;
        if let Ok(mut state) = lock.lock() {
            state.seq += 1;
            let seq = state.seq;
            state.heap.push(TimerEntry {
                due: Instant::now() + after,
                seq,
                fire: Box::new(fire),
            });
            cv.notify_one();
        }
    }
}

/// Services for a host whose platform calls go to a sink (tests).
#[derive(Default)]
pub struct RecordingServices {
    pub opened_urls: Mutex<Vec<String>>,
    pub clipboard: Mutex<Vec<String>>,
    pub picked: Mutex<Option<Vec<PathBuf>>>,
}

impl HostServices for RecordingServices {
    fn prompt_for_paths(&self, _options: super::PathPromptOptions) -> Task<Option<Vec<PathBuf>>> {
        Task::ready(self.picked.lock().ok().and_then(|p| p.clone()))
    }
    fn write_to_clipboard(&self, text: String) {
        if let Ok(mut c) = self.clipboard.lock() {
            c.push(text);
        }
    }
    fn open_url(&self, url: &str) {
        if let Ok(mut u) = self.opened_urls.lock() {
            u.push(url.to_string());
        }
    }
    fn reveal_path(&self, _path: &std::path::Path) {}
    fn open_with_system(&self, _path: &std::path::Path) {}
    fn quit(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timer_and_background_tasks_resolve_on_the_loop() {
        let (mut host, rx) = LocalHost::bare();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen2 = seen.clone();
        let host_ref: &dyn Host = &host;
        host_ref.spawn(async move |cx: &mut AsyncCtx| {
            let value = cx.background_executor().spawn(async { 41 + 1 }).await;
            cx.background_executor()
                .timer(Duration::from_millis(20))
                .await;
            if let Ok(mut s) = seen2.lock() {
                s.push(value);
            }
        });
        run_until_idle(&mut host, &rx, Duration::from_millis(200));
        assert_eq!(seen.lock().map(|s| s.clone()).unwrap_or_default(), vec![42]);
        assert!(!host.has_pending_tasks());
    }

    #[test]
    fn query_runs_on_the_loop_thread() {
        let handle = spawn_loop("corvene-main", Arc::new(NoServices), |_| {}).unwrap();
        let name = handle
            .query_blocking(|_| std::thread::current().name().map(str::to_string))
            .flatten();
        assert_eq!(name.as_deref(), Some("corvene-main"));
        handle.stop();
    }
}
