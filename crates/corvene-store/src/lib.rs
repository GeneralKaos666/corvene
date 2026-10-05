//! Persistence: one redb file holding JSON values keyed by string.
//! Typed accessors live in `corvene_core::persistence` (this crate stays
//! dependency-free so core can depend on it).
//!
//! [`Store::close`] at quit: redb records its allocator state only when the
//! database is dropped, and a process that exits without dropping it (GPUI
//! quits through `exit`) leaves a file that the next launch has to repair,
//! walking every page and committing durably before the first read.
//!
//! Background writes ([`Store::set_background_writes`], flag
//! `910-background-store-writes`): every commit is durable (redb's default
//! `Durability::Immediate`, an `F_FULLFSYNC` on macOS, 5-35 ms), so a save
//! made on the main thread stalls the frame. With background writes on, a
//! write lands in an in-memory overlay that reads consult first, and one
//! writer thread commits the overlay in batches, still durably, in the
//! order the writes were made: a key written again before its commit is
//! saved once with its newest value, and an older value never overwrites a
//! newer one. [`Store::close`] commits whatever is left before the database
//! is dropped. A crash loses at most the writes of the last few
//! milliseconds. `Durability::None` with a periodic durable commit was the
//! other option: it keeps the B-tree work on the calling thread, holds the
//! unpersisted pages in memory, loses everything since the last durable
//! commit on a crash, and its durable commit would still block a main
//! thread waiting on redb's write lock.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, RwLock};
use std::thread::JoinHandle;

use redb::{Database, ReadableDatabase, TableDefinition};
use serde::{Serialize, de::DeserializeOwned};

const KV: TableDefinition<&str, &[u8]> = TableDefinition::new("kv");
const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Db(#[from] redb::Error),
    #[error("database error: {0}")]
    DbOpen(#[from] redb::DatabaseError),
    #[error("transaction error: {0}")]
    Txn(#[from] redb::TransactionError),
    #[error("table error: {0}")]
    Table(#[from] redb::TableError),
    #[error("storage error: {0}")]
    Storage(#[from] redb::StorageError),
    #[error("commit error: {0}")]
    Commit(#[from] redb::CommitError),
    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("the store is closed")]
    Closed,
    /// An earlier background commit failed; its writes are retried with
    /// the next one.
    #[error("{0}")]
    Background(String),
}

pub type Result<T> = std::result::Result<T, StoreError>;

pub struct Store {
    shared: Arc<Shared>,
    writer: Mutex<Option<JoinHandle<()>>>,
    path: PathBuf,
}

struct Shared {
    /// `None` once [`Store::close`] has run
    db: RwLock<Option<Database>>,
    pending: Mutex<Pending>,
    /// the writer waits here for new writes or the close
    work: Condvar,
    /// [`Store::flush`] waits here for the writer
    done: Condvar,
    background: AtomicBool,
}

/// Writes not committed yet, newest value per key (`None` = removed).
#[derive(Default)]
struct Pending {
    entries: HashMap<String, (u64, Option<Vec<u8>>)>,
    /// generation of the newest write
    latest: u64,
    /// every write up to this generation has been committed or has failed
    attempted: u64,
    in_flight: bool,
    closing: bool,
    /// the last background commit's error, reported by the next write
    failed: Option<String>,
}

impl Pending {
    fn idle(&self) -> bool {
        self.entries.is_empty() && !self.in_flight
    }
}

impl Store {
    /// Open (or create) the database at `dir/corvene.redb`.
    pub fn open_in(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref();
        std::fs::create_dir_all(dir)?;
        Self::open(dir.join("corvene.redb"))
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let mut builder = Database::builder();
        builder.set_repair_callback(|session| {
            if session.progress() == 0.0 {
                tracing::info!("repairing the store, it was not closed cleanly");
            }
        });
        let db = builder.create(&path)?;
        let store = Self {
            shared: Arc::new(Shared {
                db: RwLock::new(Some(db)),
                pending: Mutex::new(Pending::default()),
                work: Condvar::new(),
                done: Condvar::new(),
                background: AtomicBool::new(false),
            }),
            writer: Mutex::new(None),
            path,
        };
        store.ensure_schema()?;
        Ok(store)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Commit writes on a background thread (`true`) or before
    /// [`Store::set_raw`] returns (`false`, the default). Turning it off
    /// waits for the writes already queued.
    pub fn set_background_writes(&self, on: bool) {
        if self.shared.background.swap(on, Ordering::SeqCst) == on {
            return;
        }
        if on {
            let mut writer = lock(&self.writer);
            if writer.is_none() && !self.shared.pending().closing {
                let shared = self.shared.clone();
                match std::thread::Builder::new()
                    .name("corvene-store".into())
                    .spawn(move || shared.write_loop())
                {
                    Ok(handle) => *writer = Some(handle),
                    Err(err) => {
                        tracing::warn!(%err, "could not start the store writer");
                        self.shared.background.store(false, Ordering::SeqCst);
                    }
                }
            }
        } else {
            self.flush();
        }
    }

    /// Wait until every write made so far has been committed (or failed).
    pub fn flush(&self) {
        let mut pending = self.shared.pending();
        let target = pending.latest;
        while pending.attempted < target && !pending.closing {
            pending = self
                .shared
                .done
                .wait(pending)
                .unwrap_or_else(|e| e.into_inner());
        }
    }

    /// Commit the queued writes, then close the database so the next open
    /// needs no repair. Reads and writes after this fail with
    /// [`StoreError::Closed`].
    pub fn close(&self) {
        let started = std::time::Instant::now();
        self.shared.pending().closing = true;
        self.shared.work.notify_all();
        self.shared.done.notify_all();
        let writer = lock(&self.writer).take();
        if let Some(writer) = writer
            && writer.join().is_err()
        {
            tracing::warn!("the store writer panicked");
        }
        let db = self
            .shared
            .db
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        if db.is_some() {
            drop(db);
            tracing::debug!(ms = started.elapsed().as_millis(), "store closed");
        }
    }

    fn ensure_schema(&self) -> Result<()> {
        let current: Option<u32> = self.get("meta.schema_version")?;
        if current != Some(SCHEMA_VERSION) {
            tracing::info!(from = ?current, to = SCHEMA_VERSION, "initialising store schema");
            self.set("meta.schema_version", &SCHEMA_VERSION)?;
        }
        Ok(())
    }

    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        match self.get_raw(key)? {
            Some(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            None => Ok(None),
        }
    }

    pub fn set<T: Serialize + ?Sized>(&self, key: &str, value: &T) -> Result<()> {
        self.set_raw(key, &serde_json::to_vec(value)?)
    }

    /// The stored bytes of `key`, whatever they hold.
    pub fn get_raw(&self, key: &str) -> Result<Option<Vec<u8>>> {
        {
            let pending = self.shared.pending();
            if pending.closing {
                return Err(StoreError::Closed);
            }
            if let Some((_, value)) = pending.entries.get(key) {
                return Ok(value.clone());
            }
        }
        self.shared.with_db(|db| {
            let txn = db.begin_read()?;
            let table = match txn.open_table(KV) {
                Ok(table) => table,
                Err(redb::TableError::TableDoesNotExist(_)) => return Ok(None),
                Err(err) => return Err(err.into()),
            };
            Ok(table.get(key)?.map(|guard| guard.value().to_vec()))
        })
    }

    /// Store `bytes` under `key` as they are.
    pub fn set_raw(&self, key: &str, bytes: &[u8]) -> Result<()> {
        self.write(key, Some(bytes))
    }

    pub fn remove(&self, key: &str) -> Result<()> {
        self.write(key, None)
    }

    fn write(&self, key: &str, value: Option<&[u8]>) -> Result<()> {
        let background = self.shared.background.load(Ordering::SeqCst);
        let mut pending = self.shared.pending();
        if pending.closing {
            return Err(StoreError::Closed);
        }
        if !background && pending.idle() {
            // holding `pending` keeps a background write from slipping in
            // between
            return self.shared.with_db(|db| commit(db, [(key, value)]));
        }
        pending.latest += 1;
        let generation = pending.latest;
        pending
            .entries
            .insert(key.to_owned(), (generation, value.map(<[u8]>::to_vec)));
        let failed = pending.failed.take();
        drop(pending);
        self.shared.work.notify_one();
        if !background {
            // turned off while writes were queued: keep their order
            self.flush();
        }
        match failed {
            Some(err) => Err(StoreError::Background(err)),
            None => Ok(()),
        }
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        self.close();
    }
}

impl Shared {
    fn pending(&self) -> MutexGuard<'_, Pending> {
        lock(&self.pending)
    }

    fn with_db<T>(&self, f: impl FnOnce(&Database) -> Result<T>) -> Result<T> {
        let db = self.db.read().unwrap_or_else(|e| e.into_inner());
        f(db.as_ref().ok_or(StoreError::Closed)?)
    }

    /// The writer thread: commit everything queued in one transaction,
    /// until [`Store::close`] (which gets one last commit).
    fn write_loop(&self) {
        loop {
            let (batch, upto) = {
                let mut pending = self.pending();
                while pending.attempted == pending.latest && !pending.closing {
                    pending = self.work.wait(pending).unwrap_or_else(|e| e.into_inner());
                }
                if pending.entries.is_empty() {
                    pending.attempted = pending.latest;
                    self.done.notify_all();
                    if pending.closing {
                        return;
                    }
                    continue;
                }
                pending.in_flight = true;
                let batch: Vec<(String, u64, Option<Vec<u8>>)> = pending
                    .entries
                    .iter()
                    .map(|(key, (generation, value))| (key.clone(), *generation, value.clone()))
                    .collect();
                (batch, pending.latest)
            };
            let started = std::time::Instant::now();
            let result = self.with_db(|db| {
                commit(
                    db,
                    batch
                        .iter()
                        .map(|(key, _, value)| (key.as_str(), value.as_deref())),
                )
            });
            let mut pending = self.pending();
            pending.in_flight = false;
            pending.attempted = upto;
            match result {
                Ok(()) => {
                    tracing::trace!(
                        keys = batch.len(),
                        ms = started.elapsed().as_millis(),
                        "store commit"
                    );
                    for (key, generation, _) in &batch {
                        // a newer write of the key waits for the next commit
                        if pending
                            .entries
                            .get(key)
                            .is_some_and(|(g, _)| g == generation)
                        {
                            pending.entries.remove(key);
                        }
                    }
                }
                Err(err) => {
                    tracing::warn!(%err, keys = batch.len(), "could not save to the store");
                    pending.failed = Some(err.to_string());
                }
            }
            // closing: no write can follow, and a failed batch is not
            // retried forever
            let finished = pending.closing && pending.attempted == pending.latest;
            drop(pending);
            self.done.notify_all();
            if finished {
                return;
            }
        }
    }
}

fn commit<'a>(
    db: &Database,
    writes: impl IntoIterator<Item = (&'a str, Option<&'a [u8]>)>,
) -> Result<()> {
    let txn = db.begin_write()?;
    {
        let mut table = txn.open_table(KV)?;
        for (key, value) in writes {
            match value {
                Some(bytes) => {
                    table.insert(key, bytes)?;
                }
                None => {
                    table.remove(key)?;
                }
            }
        }
    }
    txn.commit()?;
    Ok(())
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_round_trip_and_missing() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open_in(dir.path()).unwrap();
        let v: Option<String> = store.get("nope").unwrap();
        assert!(v.is_none());
        store.set("k", &vec![1u32, 2, 3]).unwrap();
        assert_eq!(store.get::<Vec<u32>>("k").unwrap(), Some(vec![1, 2, 3]));
        store.remove("k").unwrap();
        assert!(store.get::<Vec<u32>>("k").unwrap().is_none());
        assert_eq!(store.get::<u32>("meta.schema_version").unwrap(), Some(1));
    }

    #[test]
    fn closed_store_reopens_without_repair() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open_in(dir.path()).unwrap();
        store.set("k", &1u32).unwrap();
        store.close();
        assert!(matches!(store.get::<u32>("k"), Err(StoreError::Closed)));
        assert!(matches!(store.set("k", &2u32), Err(StoreError::Closed)));
        // a second close is a no-op
        store.close();

        let repaired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = repaired.clone();
        let mut builder = Database::builder();
        builder.set_repair_callback(move |_| flag.store(true, std::sync::atomic::Ordering::SeqCst));
        drop(builder.create(dir.path().join("corvene.redb")).unwrap());
        assert!(!repaired.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(
            Store::open_in(dir.path()).unwrap().get::<u32>("k").unwrap(),
            Some(1)
        );
    }

    #[test]
    fn background_writes_read_back_at_once_and_survive_close() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open_in(dir.path()).unwrap();
        store.set_background_writes(true);
        for i in 0..200u32 {
            store.set("k", &i).unwrap();
            assert_eq!(store.get::<u32>("k").unwrap(), Some(i));
            store.set(&format!("n{}", i % 7), &i).unwrap();
        }
        store.set("gone", &1u32).unwrap();
        store.remove("gone").unwrap();
        assert!(store.get::<u32>("gone").unwrap().is_none());
        store.close();

        let store = Store::open_in(dir.path()).unwrap();
        assert_eq!(store.get::<u32>("k").unwrap(), Some(199));
        for n in 0..7u32 {
            let last = (0..200u32).filter(|i| i % 7 == n).max();
            assert_eq!(store.get::<u32>(&format!("n{n}")).unwrap(), last);
        }
        assert!(store.get::<u32>("gone").unwrap().is_none());
    }

    #[test]
    fn writes_from_many_threads_keep_the_newest_value() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(Store::open_in(dir.path()).unwrap());
        store.set_background_writes(true);
        let threads: Vec<_> = (0..4u32)
            .map(|t| {
                let store = store.clone();
                std::thread::spawn(move || {
                    for i in 0..100u32 {
                        store.set(&format!("t{t}"), &i).unwrap();
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        store.flush();
        // switching back commits inline again, after the queue
        store.set_background_writes(false);
        store.set("t0", &1000u32).unwrap();
        store.close();
        let store = Store::open_in(dir.path()).unwrap();
        assert_eq!(store.get::<u32>("t0").unwrap(), Some(1000));
        for t in 1..4u32 {
            assert_eq!(store.get::<u32>(&format!("t{t}")).unwrap(), Some(99));
        }
    }

    #[test]
    fn dropping_the_store_commits_its_queue() {
        let dir = tempfile::tempdir().unwrap();
        {
            let store = Store::open_in(dir.path()).unwrap();
            store.set_background_writes(true);
            store.set("k", &7u32).unwrap();
        }
        let store = Store::open_in(dir.path()).unwrap();
        assert_eq!(store.get::<u32>("k").unwrap(), Some(7));
    }
}
