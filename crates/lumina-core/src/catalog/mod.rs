mod albums;
mod media;
mod schema;
mod tags;
mod trash;

use anyhow::{bail, Result};
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::{MediaId, MediaItem, MediaType};

/// Bumped whenever the catalog layout changes. The ad-hoc pragma migrations are
/// idempotent, and this version is stamped into `PRAGMA user_version` so future
/// migrations can dispatch on it.
pub const SCHEMA_VERSION: i64 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeletionTarget {
    CatalogOnly,
    SystemTrash,
    PermanentDisk,
}

#[derive(Clone)]
pub struct Catalog {
    pub(crate) conn: Arc<Mutex<Connection>>,
    db_path: Option<PathBuf>,
}

impl Catalog {
    pub fn open(db_path: impl AsRef<Path>) -> Result<Self> {
        let db_path = db_path.as_ref();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        let catalog = Self {
            conn: Arc::new(Mutex::new(conn)),
            db_path: Some(db_path.to_path_buf()),
        };
        catalog.init_schema()?;
        Ok(catalog)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let catalog = Self {
            conn: Arc::new(Mutex::new(conn)),
            db_path: None,
        };
        catalog.init_schema()?;
        Ok(catalog)
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
    pub fn schema_version(&self) -> Result<i64> {
        let conn = self.conn.lock();
        Ok(conn.pragma_query_value(None, "user_version", |row| row.get(0))?)
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

    pub(crate) fn map_query_rows<P>(
        stmt: &mut rusqlite::Statement,
        params: P,
    ) -> Result<Vec<MediaItem>>
    where
        P: rusqlite::Params,
    {
        let rows = stmt.query_map(params, |row| {
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

            let media_type = match media_type_str.as_str() {
                "video" => MediaType::Video,
                "raw" => MediaType::Raw,
                _ => MediaType::Image,
            };

            Ok(MediaItem {
                id: MediaId(id as u64),
                path: PathBuf::from(file_path),
                parent_dir,
                file_name,
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
        })?;

        let mut items = Vec::new();
        for r in rows {
            items.push(r?);
        }
        Ok(items)
    }
}
