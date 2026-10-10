use anyhow::{Context, Result};
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::{MediaId, MediaItem, MediaType};

#[derive(Clone)]
pub struct Catalog {
    conn: Arc<Mutex<Connection>>,
    db_path: PathBuf,
}

impl Catalog {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let db_path = path.as_ref().to_path_buf();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create catalog parent directory: {:?}", parent))?;
        }

        let conn = Connection::open(&db_path)
            .with_context(|| format!("Failed to open SQLite database at {:?}", db_path))?;

        // Performance pragmas for 120 FPS workloads
        conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA temp_store = MEMORY;
            PRAGMA cache_size = -64000;
            "#,
        )?;

        let catalog = Self {
            conn: Arc::new(Mutex::new(conn)),
            db_path,
        };

        catalog.init_schema()?;
        Ok(catalog)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            "#,
        )?;
        let catalog = Self {
            conn: Arc::new(Mutex::new(conn)),
            db_path: PathBuf::from(":memory:"),
        };
        catalog.init_schema()?;
        Ok(catalog)
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS media_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT NOT NULL UNIQUE,
                media_type TEXT NOT NULL,
                width INTEGER NOT NULL,
                height INTEGER NOT NULL,
                duration_secs REAL NOT NULL DEFAULT 0.0,
                file_size INTEGER NOT NULL,
                mtime_epoch_ms INTEGER NOT NULL,
                timestamp_epoch_ms INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_media_timestamp ON media_items(timestamp_epoch_ms DESC);
            CREATE INDEX IF NOT EXISTS idx_media_path ON media_items(path);

            CREATE TABLE IF NOT EXISTS app_config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            "#,
        )?;
        Ok(())
    }

    pub fn insert_or_update_batch(&self, items: &[MediaItem]) -> Result<Vec<MediaItem>> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let mut result = Vec::with_capacity(items.len());

        {
            let mut stmt = tx.prepare(
                r#"
                INSERT INTO media_items (
                    path, media_type, width, height, duration_secs, file_size, mtime_epoch_ms, timestamp_epoch_ms
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                ON CONFLICT(path) DO UPDATE SET
                    media_type = excluded.media_type,
                    width = excluded.width,
                    height = excluded.height,
                    duration_secs = excluded.duration_secs,
                    file_size = excluded.file_size,
                    mtime_epoch_ms = excluded.mtime_epoch_ms,
                    timestamp_epoch_ms = excluded.timestamp_epoch_ms
                RETURNING id;
                "#,
            )?;

            for item in items {
                let path_str = item.path.to_string_lossy();
                let type_str = match item.media_type {
                    MediaType::Image => "image",
                    MediaType::Video => "video",
                };

                let id: i64 = stmt.query_row(
                    params![
                        path_str.as_ref(),
                        type_str,
                        item.width,
                        item.height,
                        item.duration_secs,
                        item.file_size,
                        item.mtime_epoch_ms,
                        item.timestamp_epoch_ms,
                    ],
                    |row| row.get(0),
                )?;

                let mut updated = item.clone();
                updated.id = MediaId(id as u64);
                result.push(updated);
            }
        }

        tx.commit()?;
        Ok(result)
    }

    pub fn total_count(&self) -> Result<usize> {
        let conn = self.conn.lock();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM media_items;",
            [],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    pub fn get_item_slice(&self, offset: usize, limit: usize) -> Result<Vec<MediaItem>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, path, media_type, width, height, duration_secs, file_size, mtime_epoch_ms, timestamp_epoch_ms
            FROM media_items
            ORDER BY timestamp_epoch_ms DESC, id DESC
            LIMIT ?1 OFFSET ?2;
            "#,
        )?;

        let rows = stmt.query_map(params![limit as i64, offset as i64], |row| {
            let id: i64 = row.get(0)?;
            let path_str: String = row.get(1)?;
            let media_type_str: String = row.get(2)?;
            let width: u32 = row.get(3)?;
            let height: u32 = row.get(4)?;
            let duration_secs: f64 = row.get(5)?;
            let file_size: u64 = row.get(6)?;
            let mtime_epoch_ms: i64 = row.get(7)?;
            let timestamp_epoch_ms: i64 = row.get(8)?;

            let media_type = if media_type_str == "video" {
                MediaType::Video
            } else {
                MediaType::Image
            };

            Ok(MediaItem {
                id: MediaId(id as u64),
                path: PathBuf::from(path_str),
                media_type,
                width,
                height,
                duration_secs,
                file_size,
                mtime_epoch_ms,
                timestamp_epoch_ms,
            })
        })?;

        let mut items = Vec::with_capacity(limit);
        for row in rows {
            items.push(row?);
        }
        Ok(items)
    }

    pub fn get_item_by_id(&self, id: MediaId) -> Result<Option<MediaItem>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, path, media_type, width, height, duration_secs, file_size, mtime_epoch_ms, timestamp_epoch_ms
            FROM media_items
            WHERE id = ?1;
            "#,
        )?;

        let mut rows = stmt.query_map(params![id.0 as i64], |row| {
            let id: i64 = row.get(0)?;
            let path_str: String = row.get(1)?;
            let media_type_str: String = row.get(2)?;
            let width: u32 = row.get(3)?;
            let height: u32 = row.get(4)?;
            let duration_secs: f64 = row.get(5)?;
            let file_size: u64 = row.get(6)?;
            let mtime_epoch_ms: i64 = row.get(7)?;
            let timestamp_epoch_ms: i64 = row.get(8)?;

            let media_type = if media_type_str == "video" {
                MediaType::Video
            } else {
                MediaType::Image
            };

            Ok(MediaItem {
                id: MediaId(id as u64),
                path: PathBuf::from(path_str),
                media_type,
                width,
                height,
                duration_secs,
                file_size,
                mtime_epoch_ms,
                timestamp_epoch_ms,
            })
        })?;

        match rows.next() {
            Some(res) => Ok(Some(res?)),
            None => Ok(None),
        }
    }

    pub fn remove_item_by_path(&self, path: &Path) -> Result<Option<MediaId>> {
        let conn = self.conn.lock();
        let path_str = path.to_string_lossy();
        let id_opt: Option<i64> = conn
            .query_row(
                "SELECT id FROM media_items WHERE path = ?1;",
                params![path_str.as_ref()],
                |row| row.get(0),
            )
            .ok();

        if let Some(id) = id_opt {
            conn.execute("DELETE FROM media_items WHERE id = ?1;", params![id])?;
            Ok(Some(MediaId(id as u64)))
        } else {
            Ok(None)
        }
    }

    pub fn remove_items_not_in(&self, valid_paths: &HashSet<PathBuf>) -> Result<Vec<MediaId>> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let mut removed = Vec::new();

        {
            let mut stmt = tx.prepare("SELECT id, path FROM media_items;")?;
            let rows = stmt.query_map([], |row| {
                let id: i64 = row.get(0)?;
                let path: String = row.get(1)?;
                Ok((id, PathBuf::from(path)))
            })?;

            for row in rows {
                let (id, path) = row?;
                if !valid_paths.contains(&path) {
                    removed.push(MediaId(id as u64));
                }
            }
        }

        for id in &removed {
            tx.execute("DELETE FROM media_items WHERE id = ?1;", params![id.0 as i64])?;
        }

        tx.commit()?;
        Ok(removed)
    }

    pub fn clear_all(&self) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM media_items;", [])?;
        Ok(())
    }

    pub fn get_config(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT value FROM app_config WHERE key = ?1;")?;
        let mut rows = stmt.query_map(params![key], |row| row.get(0))?;
        match rows.next() {
            Some(res) => Ok(Some(res?)),
            None => Ok(None),
        }
    }

    pub fn set_config(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            r#"
            INSERT INTO app_config (key, value) VALUES (?1, ?2)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value;
            "#,
            params![key, value],
        )?;
        Ok(())
    }
}
