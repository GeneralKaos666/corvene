//! Persistence: one redb file holding JSON values keyed by string.
//! Typed accessors live in `corvene_core::persistence` (this crate stays
//! dependency-free so core can depend on it).
//!
//! [`Store::close`] at quit: redb records its allocator state only when the
//! database is dropped, and a process that exits without dropping it (GPUI
//! quits through `exit`) leaves a file that the next launch has to repair,
//! walking every page and committing durably before the first read.

use std::path::{Path, PathBuf};
use std::sync::RwLock;

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
}

pub type Result<T> = std::result::Result<T, StoreError>;

pub struct Store {
    /// `None` once [`Store::close`] has run
    db: RwLock<Option<Database>>,
    path: PathBuf,
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
            db: RwLock::new(Some(db)),
            path,
        };
        store.ensure_schema()?;
        Ok(store)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Close the database so the next open needs no repair. Reads and
    /// writes after this fail with [`StoreError::Closed`].
    pub fn close(&self) {
        let db = self.db.write().unwrap_or_else(|e| e.into_inner()).take();
        if db.is_some() {
            let started = std::time::Instant::now();
            drop(db);
            tracing::debug!(ms = started.elapsed().as_millis(), "store closed");
        }
    }

    fn with_db<T>(&self, f: impl FnOnce(&Database) -> Result<T>) -> Result<T> {
        let db = self.db.read().unwrap_or_else(|e| e.into_inner());
        f(db.as_ref().ok_or(StoreError::Closed)?)
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
        self.with_db(|db| {
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
        self.with_db(|db| {
            let txn = db.begin_write()?;
            {
                let mut table = txn.open_table(KV)?;
                table.insert(key, bytes)?;
            }
            txn.commit()?;
            Ok(())
        })
    }

    pub fn remove(&self, key: &str) -> Result<()> {
        self.with_db(|db| {
            let txn = db.begin_write()?;
            {
                let mut table = txn.open_table(KV)?;
                table.remove(key)?;
            }
            txn.commit()?;
            Ok(())
        })
    }
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
}
