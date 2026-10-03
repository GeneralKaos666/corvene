//! GHD `AheadBehindStore` (`app/src/lib/stores/ahead-behind-store.ts`):
//! ahead/behind counts of commit ranges (`git rev-list --left-right --count
//! from...to`, `corvene_git::symmetric_ahead_behind`), computed in the
//! background one at a time, cached per repository and range (at most 2500
//! ranges, least recently used dropped first) and handed to a callback
//! unless the request was disposed first. A range that failed is never
//! retried. The compare branch list (`Dispatcher::load_compare_counts`)
//! asks it for every branch's tip against the current branch's tip; with
//! commit ids for `from` and `to` the cache never goes stale.
//!
//! GHD's callbacks run on the event loop. A Corvene callback runs on the
//! thread that owns the store: at once for a cached range, else from
//! [`AheadBehindStore::poll`] (or [`AheadBehindStore::wait`], which blocks
//! until every request has an answer) once the background worker has
//! counted it. The dispatcher waits for the worker on the background
//! executor ([`AheadBehindWaiter::wait_idle`]) and polls on the main thread.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};

use corvene_git::GitBinary;
use corvene_models::AheadBehind;
use tracing::error;

/// GHD's `QuickLRU({ maxSize: 2500 })`.
const MAX_CACHED_RANGES: usize = 2500;

/// GHD `getCacheKey(repository, from, to)`.
fn cache_key(repository: &Path, from: &str, to: &str) -> String {
    format!("{}:{from}:{to}", repository.display())
}

/// GHD's `IDisposable` (event-kit `Disposable`): disposing a request means
/// its callback never runs, and its count is not started when no other
/// request wants it.
#[derive(Clone, Debug, Default)]
pub struct Disposable(Arc<AtomicBool>);

impl Disposable {
    pub fn new() -> Self {
        Self::default()
    }

    /// GHD `dispose()`.
    pub fn dispose(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// GHD `disposed`.
    pub fn disposed(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

impl PartialEq for Disposable {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// A range waiting for the worker.
struct Job {
    key: String,
    git: Arc<GitBinary>,
    repository: PathBuf,
    from: String,
    to: String,
    /// The requests waiting for it; when all are disposed before it starts
    /// it is skipped.
    subscribers: Vec<Disposable>,
}

#[derive(Default)]
struct Inner {
    /// Range → count (`None`: it failed), with the tick of its last use.
    cache: HashMap<String, (Option<AheadBehind>, u64)>,
    tick: u64,
    /// GHD's `pLimit(1)` queue.
    queue: VecDeque<Job>,
    /// The range being counted (GHD `workers`).
    running: Option<String>,
    worker_alive: bool,
}

impl Inner {
    /// A cached count, marked as just used.
    fn get(&mut self, key: &str) -> Option<Option<AheadBehind>> {
        self.tick += 1;
        let tick = self.tick;
        self.cache.get_mut(key).map(|(value, used)| {
            *used = tick;
            *value
        })
    }

    fn insert(&mut self, key: String, value: Option<AheadBehind>) {
        self.tick += 1;
        self.cache.insert(key, (value, self.tick));
        if self.cache.len() > MAX_CACHED_RANGES
            && let Some(oldest) = self
                .cache
                .iter()
                .min_by_key(|(_, (_, used))| *used)
                .map(|(key, _)| key.clone())
        {
            self.cache.remove(&oldest);
        }
    }

    fn is_queued(&self, key: &str) -> bool {
        self.running.as_deref() == Some(key) || self.queue.iter().any(|j| j.key == key)
    }

    /// No range is being counted or waiting for a worker that runs.
    fn idle(&self) -> bool {
        self.running.is_none() && (self.queue.is_empty() || !self.worker_alive)
    }
}

/// The part of the store the background worker shares.
#[derive(Clone, Default)]
struct Shared(Arc<(Mutex<Inner>, Condvar)>);

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.0.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Queue `job` (or add its subscriber to the same range already queued)
    /// and make sure a worker runs.
    fn enqueue(&self, job: Job) {
        let mut inner = self.lock();
        if inner.running.as_deref() == Some(job.key.as_str()) {
            return;
        }
        if let Some(queued) = inner.queue.iter_mut().find(|j| j.key == job.key) {
            queued.subscribers.extend(job.subscribers);
            return;
        }
        inner.queue.push_back(job);
        if inner.worker_alive {
            return;
        }
        inner.worker_alive = true;
        drop(inner);
        let shared = self.clone();
        if let Err(err) = std::thread::Builder::new()
            .name("ahead-behind".into())
            .spawn(move || shared.work())
        {
            error!(%err, "could not start the ahead/behind worker");
            // count nothing rather than wait forever: the queued ranges fail
            let mut inner = self.lock();
            inner.worker_alive = false;
            while let Some(job) = inner.queue.pop_front() {
                inner.insert(job.key, None);
            }
            self.0.1.notify_all();
        }
    }

    /// The worker: count the queued ranges one after another.
    fn work(&self) {
        loop {
            let job = {
                let mut inner = self.lock();
                loop {
                    match inner.queue.pop_front() {
                        None => {
                            inner.worker_alive = false;
                            self.0.1.notify_all();
                            return;
                        }
                        // every caller aborted before git ran
                        Some(job) if job.subscribers.iter().all(Disposable::disposed) => {}
                        Some(job) => {
                            inner.running = Some(job.key.clone());
                            break job;
                        }
                    }
                }
            };
            let count =
                corvene_git::symmetric_ahead_behind(job.git, &job.repository, &job.from, &job.to)
                    .unwrap_or_else(|err| {
                        error!(%err, "failed calculating ahead/behind status");
                        None
                    });
            let mut inner = self.lock();
            inner.insert(job.key, count);
            inner.running = None;
            self.0.1.notify_all();
        }
    }

    fn wait_idle(&self) {
        let mut inner = self.lock();
        while !inner.idle() {
            inner = self.0.1.wait(inner).unwrap_or_else(PoisonError::into_inner);
        }
    }
}

/// A request whose range is still being counted.
struct Pending {
    key: String,
    git: Arc<GitBinary>,
    repository: PathBuf,
    from: String,
    to: String,
    disposable: Disposable,
    callback: Box<dyn FnOnce(AheadBehind)>,
}

/// GHD `AheadBehindStore` (see the module docs).
#[derive(Default)]
pub struct AheadBehindStore {
    shared: Shared,
    pending: Vec<Pending>,
}

/// A handle on an [`AheadBehindStore`]'s worker that can be sent to
/// another thread.
#[derive(Clone)]
pub struct AheadBehindWaiter(Shared);

impl AheadBehindWaiter {
    /// Block until no range is being counted or waiting to be.
    pub fn wait_idle(&self) {
        self.0.wait_idle();
    }
}

impl AheadBehindStore {
    /// `new AheadBehindStore()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// GHD `tryGetAheadBehind(repository, from, to)`: the cached count of
    /// `from...to`, if it has been counted (and did not fail).
    pub fn try_get_ahead_behind(
        &self,
        repository: &Path,
        from: &str,
        to: &str,
    ) -> Option<AheadBehind> {
        self.shared
            .lock()
            .get(&cache_key(repository, from, to))
            .flatten()
    }

    /// GHD `getAheadBehind(repository, from, to, callback)`: count
    /// `from...to` in the background and call `callback` with the result
    /// (at once when it is cached) unless the returned [`Disposable`] is
    /// disposed first. A failed count never calls back.
    pub fn get_ahead_behind(
        &mut self,
        git: Arc<GitBinary>,
        repository: &Path,
        from: &str,
        to: &str,
        callback: impl FnOnce(AheadBehind) + 'static,
    ) -> Disposable {
        let key = cache_key(repository, from, to);
        let disposable = Disposable::new();
        let cached = self.shared.lock().get(&key);
        match cached {
            // We failed loading on the last attempt in which case we won't retry
            Some(None) => return disposable,
            Some(Some(count)) => {
                callback(count);
                return disposable;
            }
            None => {}
        }
        self.shared.enqueue(Job {
            key: key.clone(),
            git: git.clone(),
            repository: repository.to_path_buf(),
            from: from.to_string(),
            to: to.to_string(),
            subscribers: vec![disposable.clone()],
        });
        self.pending.push(Pending {
            key,
            git,
            repository: repository.to_path_buf(),
            from: from.to_string(),
            to: to.to_string(),
            disposable: disposable.clone(),
            callback: Box::new(callback),
        });
        disposable
    }

    /// Run the callbacks of the requests whose range has been counted
    /// (dropping the disposed ones and those that failed); `true` while
    /// some are still waiting.
    pub fn poll(&mut self) -> bool {
        let mut ready = Vec::new();
        let mut requeue = Vec::new();
        {
            let mut inner = self.shared.lock();
            for pending in std::mem::take(&mut self.pending) {
                if pending.disposable.disposed() {
                    continue;
                }
                match inner.get(&pending.key) {
                    Some(Some(count)) => ready.push((pending.callback, count)),
                    Some(None) => {}
                    None => {
                        if !inner.is_queued(&pending.key) {
                            // counted, then dropped from the cache before
                            // this request saw it: count it again
                            requeue.push(Job {
                                key: pending.key.clone(),
                                git: pending.git.clone(),
                                repository: pending.repository.clone(),
                                from: pending.from.clone(),
                                to: pending.to.clone(),
                                subscribers: vec![pending.disposable.clone()],
                            });
                        }
                        self.pending.push(pending);
                    }
                }
            }
        }
        for job in requeue {
            self.shared.enqueue(job);
        }
        for (callback, count) in ready {
            callback(count);
        }
        !self.pending.is_empty()
    }

    /// Block until every request has its answer, running their callbacks.
    pub fn wait(&mut self) {
        loop {
            self.shared.wait_idle();
            if !self.poll() {
                return;
            }
        }
    }

    /// A [`AheadBehindWaiter`] for this store's worker.
    pub fn waiter(&self) -> AheadBehindWaiter {
        AheadBehindWaiter(self.shared.clone())
    }
}

impl std::fmt::Debug for AheadBehindStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AheadBehindStore")
            .field("pending", &self.pending.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_least_recently_used_range_is_dropped() {
        let mut inner = Inner::default();
        for i in 0..MAX_CACHED_RANGES {
            inner.insert(format!("k{i}"), None);
        }
        // use the oldest once, so the second oldest goes
        assert_eq!(inner.get("k0"), Some(None));
        inner.insert("new".into(), None);
        assert_eq!(inner.cache.len(), MAX_CACHED_RANGES);
        assert!(inner.cache.contains_key("k0"));
        assert!(!inner.cache.contains_key("k1"));
    }

    #[test]
    fn a_disposable_compares_by_identity() {
        let a = Disposable::new();
        assert_eq!(a, a.clone());
        assert_ne!(a, Disposable::new());
        a.dispose();
        assert!(a.disposed());
    }
}
