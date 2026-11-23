//! The catalog: schema, rows, queries and the mutations above them.
//!
//! Naming convention for `Catalog` methods: `query_*` is a filtered read whose
//! cost depends on the library (SQL, possibly a scan); `get_*` is a direct
//! lookup of known rows; `batch_*`/`set_*` write many or one row. A method that
//! does work beyond its name (hashing, disk IO) says so in its name and doc.

mod albums;
mod annotations;
mod captions;
mod capture_dates;
mod dims;
pub use albums::{Album, AlbumCard, AlbumStats};
mod downloads;
mod duplicates;
mod merge_undo;
pub use merge_undo::{DuplicateMerge, KeeperSnapshot, MergeUndo};
mod edits;
mod flags;
pub use edits::{CropRect, Edits};
pub use flags::RejectedItems;
mod view_scope;
pub use view_scope::{DateSpan, DayPlace, LayoutCell, ViewScope, ViewTotals};
pub(crate) mod files;
mod keyword_ops;
pub mod keywords;
pub use keyword_ops::TagChange;
pub use tag_undo::TagReplay;
mod sources;
pub use downloads::{DownloadRow, DownloadState};
pub use files::MissingChange;
pub use sources::PriorDownload;
mod media;
pub use media::GeoPoint;
pub(crate) mod paths;
mod places;
pub use places::PlaceCount;
mod rights;
pub use rights::Rights;
mod roots;
pub use roots::LibraryRoot;
mod schema;
mod search;
mod smart;
mod storage;
pub use storage::StorageSummary;
mod tag_undo;
mod tags;
mod templates;
pub use templates::TemplateChange;
mod trash;
pub use trash::{DeleteOutcome, DeletedItem};

use anyhow::{bail, Result};
use parking_lot::{Mutex, MutexGuard};
use rusqlite::{params, Connection, OptionalExtension};
use std::fs;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::{MediaId, MediaItem, MediaType};

/// Bumped whenever the catalog layout changes. The ad-hoc pragma migrations are
/// idempotent, and this version is stamped into `PRAGMA user_version` so future
/// migrations can dispatch on it.
pub const SCHEMA_VERSION: i64 = 27;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeletionTarget {
    CatalogOnly,
    SystemTrash,
    PermanentDisk,
}

/// The catalog connection plus a revision that moves whenever a write lands,
/// readable without taking the lock (the UI checks it every frame).
///
/// Pure reads go through [`Db::read`]: a small pool of extra connections to
/// the same file. In WAL mode each reads its own committed snapshot, so a
/// page fetch does not wait behind a long write batch on the one writer.
pub(crate) struct Db {
    /// Idle read connections, at most [`MAX_IDLE_READERS`]. Declared before
    /// the writer so they close first and the writer does the final WAL
    /// checkpoint.
    readers: Mutex<Vec<Connection>>,
    conn: Mutex<Connection>,
    revision: AtomicU64,
    /// The catalog file readers open; `None` for an in-memory catalog, whose
    /// reads share the writer (another connection would see another database).
    path: Option<PathBuf>,
}

/// Read connections kept open between reads. More may be open at once under
/// load; those close when returned.
const MAX_IDLE_READERS: usize = 4;

impl Db {
    fn new(conn: Connection, path: Option<PathBuf>) -> Self {
        Self {
            readers: Mutex::new(Vec::new()),
            conn: Mutex::new(conn),
            revision: AtomicU64::new(0),
            path,
        }
    }

    pub(crate) fn lock(&self) -> DbGuard<'_> {
        #[cfg(feature = "test-support")]
        LOCKS_HERE.with(|locks| locks.set(locks.get() + 1));
        DbGuard {
            conn: self.conn.lock(),
            revision: &self.revision,
        }
    }

    /// A connection for a pure read (never a write, never a read whose result
    /// a write then depends on: that belongs in one writer transaction). It
    /// sees every write committed before it starts. Falls back to the writer
    /// for an in-memory catalog, or if a read connection cannot be opened.
    pub(crate) fn read(&self) -> ReadGuard<'_> {
        let Some(path) = &self.path else {
            return ReadGuard::Writer(self.lock());
        };
        #[cfg(feature = "test-support")]
        LOCKS_HERE.with(|locks| locks.set(locks.get() + 1));
        let idle = self.readers.lock().pop();
        let conn = match idle {
            Some(conn) => conn,
            None => match schema::open_reader(path) {
                Ok(conn) => conn,
                Err(err) => {
                    log::warn!("catalog read connection unavailable, using the writer: {err:#}");
                    return ReadGuard::Writer(self.lock_uncounted());
                }
            },
        };
        ReadGuard::Pooled {
            conn: Some(conn),
            pool: &self.readers,
        }
    }

    /// [`Self::lock`] without counting it again (the caller already did).
    fn lock_uncounted(&self) -> DbGuard<'_> {
        DbGuard {
            conn: self.conn.lock(),
            revision: &self.revision,
        }
    }
}

/// A connection for a pure read; see [`Db::read`].
pub(crate) enum ReadGuard<'a> {
    Pooled {
        /// Always `Some` until dropped.
        conn: Option<Connection>,
        pool: &'a Mutex<Vec<Connection>>,
    },
    Writer(DbGuard<'a>),
}

impl Deref for ReadGuard<'_> {
    type Target = Connection;
    fn deref(&self) -> &Connection {
        match self {
            ReadGuard::Pooled { conn, .. } => conn.as_ref().expect("held until drop"),
            ReadGuard::Writer(guard) => guard,
        }
    }
}

impl Drop for ReadGuard<'_> {
    fn drop(&mut self) {
        if let ReadGuard::Pooled { conn, pool } = self {
            if let Some(conn) = conn.take() {
                let mut idle = pool.lock();
                if idle.len() < MAX_IDLE_READERS {
                    idle.push(conn);
                }
            }
        }
    }
}

/// Locked connection. On release it records SQLite's running count of changed
/// rows, so every write path bumps the revision without opting in.
pub(crate) struct DbGuard<'a> {
    conn: MutexGuard<'a, Connection>,
    revision: &'a AtomicU64,
}

impl Deref for DbGuard<'_> {
    type Target = Connection;
    fn deref(&self) -> &Connection {
        &self.conn
    }
}

impl DerefMut for DbGuard<'_> {
    fn deref_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }
}

/// Catalog lock calls made from the current thread, so tests can check that
/// the UI thread never takes (and so never waits on) the lock.
#[cfg(feature = "test-support")]
pub(crate) fn locks_on_this_thread() -> u64 {
    LOCKS_HERE.with(|locks| locks.get())
}

#[cfg(feature = "test-support")]
thread_local! {
    static LOCKS_HERE: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

thread_local! {
    /// Revision after the latest write made from this thread.
    static WRITTEN_HERE: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

impl Drop for DbGuard<'_> {
    fn drop(&mut self) {
        // ponytail: one trivial query per unlock (~µs); a write hook would
        // need unsafe FFI.
        let changes = self
            .conn
            .prepare_cached("SELECT total_changes()")
            .and_then(|mut stmt| stmt.query_row([], |row| row.get::<_, i64>(0)));
        match changes {
            Ok(changes) => {
                let changes = changes as u64;
                if self.revision.swap(changes, Ordering::AcqRel) != changes {
                    WRITTEN_HERE.with(|written| written.set(changes));
                }
            }
            Err(err) => log::warn!("catalog revision unavailable: {err}"),
        }
    }
}

#[derive(Clone)]
pub struct Catalog {
    pub(crate) conn: Arc<Db>,
    db_path: Option<PathBuf>,
    /// The root an open library registered, shared by every clone so a scan
    /// writes paths relative to it without re-reading the row per item.
    root: Arc<Mutex<Option<roots::RootContext>>>,
}

impl Catalog {
    pub fn open(db_path: impl AsRef<Path>) -> Result<Self> {
        let db_path = db_path.as_ref();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        let catalog = Self {
            conn: Arc::new(Db::new(conn, Some(db_path.to_path_buf()))),
            db_path: Some(db_path.to_path_buf()),
            root: Arc::new(Mutex::new(None)),
        };
        catalog.init_schema()?;
        Ok(catalog)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let catalog = Self {
            conn: Arc::new(Db::new(conn, None)),
            db_path: None,
            root: Arc::new(Mutex::new(None)),
        };
        catalog.init_schema()?;
        Ok(catalog)
    }

    /// Moves whenever a write lands; equal values mean nothing changed in
    /// between. Never takes the catalog lock.
    pub fn revision(&self) -> u64 {
        self.conn.revision.load(Ordering::Acquire)
    }

    /// [`Self::revision`] after the latest write made from the calling thread
    /// (0 if none), so a thread can tell its own writes from others'.
    pub fn revision_written_here(&self) -> u64 {
        WRITTEN_HERE.with(|written| written.get())
    }

    /// Holds the catalog lock while `f` runs, to test that callers never wait
    /// on it.
    #[cfg(feature = "test-support")]
    pub fn with_lock_held<R>(&self, f: impl FnOnce() -> R) -> R {
        let _guard = self.conn.lock();
        f()
    }

    /// Reads an application config key-value pair from SQLite.
    pub fn get_config(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock();
        let val = conn
            .query_row(
                "SELECT value FROM app_config WHERE key = ?1",
                params![key],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        Ok(val)
    }

    /// Persists an application config key-value pair into SQLite.
    pub fn set_config(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO app_config (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value;",
            params![key, value],
        )?;
        Ok(())
    }

    /// Returns the schema version stamped in `PRAGMA user_version`.
    /// Test-support entry point: migration tests assert the pragma.
    #[doc(hidden)]
    pub fn schema_version(&self) -> Result<i64> {
        let conn = self.conn.lock();
        Ok(conn.pragma_query_value(None, "user_version", |row| row.get(0))?)
    }

    /// The catalog file behind this handle, when it is file-backed. Deleting a
    /// library needs the path; an in-memory catalog has none.
    pub fn db_path(&self) -> Option<&Path> {
        self.db_path.as_deref()
    }

    /// Runs SQLite's fast integrity check (bounded to one error).
    pub fn quick_check(&self) -> Result<()> {
        let conn = self.conn.lock();
        let result: String = conn.query_row("PRAGMA quick_check(1);", [], |row| row.get(0))?;
        if result == "ok" {
            Ok(())
        } else {
            bail!("catalog integrity check failed: {result}");
        }
    }

    /// Creates a consistent single-generation backup next to the catalog file,
    /// at most once per database modification (newer DB than backup).
    pub fn daily_backup(&self) -> Result<Option<PathBuf>> {
        let Some(db_path) = self.db_path.as_ref() else {
            return Ok(None);
        };
        if !db_path.exists() {
            return Ok(None);
        }
        let backup_path = PathBuf::from(format!("{}.bak", db_path.display()));

        if let Ok(backup_meta) = fs::metadata(&backup_path) {
            if backup_meta.len() > 0 {
                if let (Ok(db_modified), Ok(backup_modified)) =
                    (fs::metadata(db_path)?.modified(), backup_meta.modified())
                {
                    if backup_modified >= db_modified {
                        return Ok(None);
                    }
                }
            }
        }

        let _ = fs::remove_file(&backup_path);
        let conn = self.conn.lock();
        conn.execute(
            "VACUUM INTO ?1;",
            params![backup_path.to_string_lossy().to_string()],
        )?;
        Ok(Some(backup_path))
    }

    /// Returns the SQLite EXPLAIN QUERY PLAN rows for `sql`.
    /// Test-support entry point: plan assertions in the query tests.
    #[doc(hidden)]
    pub fn explain_query_plan(&self, sql: &str) -> Result<Vec<String>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}"))?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(3))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub(crate) fn map_media_item_row(row: &rusqlite::Row) -> rusqlite::Result<MediaItem> {
        let id: i64 = row.get(0)?;
        let file_path: String = row.get(1)?;
        let parent_dir: String = row.get(2)?;
        let file_name: String = row.get(3)?;
        let file_size: u64 = row.get(4)?;
        let mtime_epoch_ms: i64 = row.get(5)?;
        let media_type_str: String = row.get(6)?;
        let width: u32 = row.get(7)?;
        let height: u32 = row.get(8)?;
        let duration_ms: Option<u64> = row.get(9)?;
        let timestamp_epoch_ms: i64 = row.get(10)?;
        let year: i32 = row.get(11)?;
        let month: u32 = row.get(12)?;
        let day: u32 = row.get(13)?;
        let latitude: Option<f64> = row.get(14)?;
        let longitude: Option<f64> = row.get(15)?;
        let altitude: Option<f64> = row.get(16)?;
        let country_code: Option<String> = row.get(17)?;
        let admin1_region: Option<String> = row.get(18)?;
        let city: Option<String> = row.get(19)?;
        let make: Option<String> = row.get(20)?;
        let model: Option<String> = row.get(21)?;
        let lens: Option<String> = row.get(22)?;
        let focal_length: Option<f64> = row.get(23)?;
        let f_number: Option<f64> = row.get(24)?;
        let iso: Option<u32> = row.get(25)?;
        let exposure_time: Option<String> = row.get(26)?;
        let is_favorite: i32 = row.get(27)?;
        let is_hidden: i32 = row.get(28)?;
        let is_trashed: i32 = row.get(29)?;
        let title: Option<String> = row.get(30)?;

        let media_type = media_type_from_db(&media_type_str);

        Ok(MediaItem {
            id: MediaId(id as u64),
            path: paths::decode(&file_path),
            parent_dir,
            file_name,
            title,
            file_size,
            mtime_epoch_ms,
            media_type,
            width,
            height,
            duration_ms,
            timestamp_epoch_ms,
            year,
            month,
            day,
            latitude,
            longitude,
            altitude,
            country_code,
            admin1_region,
            city,
            make,
            model,
            lens,
            focal_length,
            f_number,
            iso,
            exposure_time,
            is_favorite: is_favorite != 0,
            is_hidden: is_hidden != 0,
            is_trashed: is_trashed != 0,
        })
    }

    pub(crate) fn map_query_rows<P>(
        stmt: &mut rusqlite::Statement,
        params: P,
    ) -> Result<Vec<MediaItem>>
    where
        P: rusqlite::Params,
    {
        let rows = stmt.query_map(params, Self::map_media_item_row)?;

        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}

/// Maps a stored `media_items.media_type` value; anything unknown is an image.
pub(crate) fn media_type_from_db(value: &str) -> MediaType {
    match value {
        "video" => MediaType::Video,
        "raw" => MediaType::Raw,
        _ => MediaType::Image,
    }
}

#[cfg(test)]
pub(crate) mod tests_support {
    use super::Catalog;
    use crate::{MediaId, MediaItem, MediaType};
    use std::path::PathBuf;

    /// Every member of album `id`, trashed and hidden ones included, by id.
    pub(crate) fn album_members(catalog: &Catalog, id: i64) -> Vec<MediaId> {
        let conn = catalog.conn.lock();
        let mut stmt = conn
            .prepare("SELECT media_id FROM album_items WHERE album_id = ?1 ORDER BY media_id;")
            .expect("album_items must be readable");
        let rows = stmt
            .query_map([id], |row| row.get::<_, i64>(0))
            .expect("album members must be readable");
        rows.map(|row| MediaId(row.expect("member id") as u64))
            .collect()
    }

    /// A catalogue-ready video at `path`.
    pub(crate) fn video_item(path: &str) -> MediaItem {
        let path = PathBuf::from(path);
        MediaItem {
            id: MediaId(0),
            parent_dir: path.parent().unwrap().to_string_lossy().to_string(),
            file_name: path.file_name().unwrap().to_string_lossy().to_string(),
            path,
            title: None,
            file_size: 1_000,
            mtime_epoch_ms: 1,
            media_type: MediaType::Video,
            width: 640,
            height: 360,
            duration_ms: Some(1_000),
            timestamp_epoch_ms: 1,
            year: 2026,
            month: 11,
            day: 3,
            latitude: None,
            longitude: None,
            altitude: None,
            country_code: None,
            admin1_region: None,
            city: None,
            make: None,
            model: None,
            lens: None,
            focal_length: None,
            f_number: None,
            iso: None,
            exposure_time: None,
            is_favorite: false,
            is_hidden: false,
            is_trashed: false,
        }
    }
}
