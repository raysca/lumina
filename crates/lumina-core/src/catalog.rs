use anyhow::{Context, Result};
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::{MediaId, MediaItem, MediaType, TimelineBucket};
use crate::search::StructuredQuery;
use crate::smart_albums::{MatchMode, SmartAlbum, SmartAlbumRule, SystemAlbum};

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
            PRAGMA mmap_size = 268435456;
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
            PRAGMA temp_store = MEMORY;
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

        let has_old_schema: bool = {
            let mut stmt = conn.prepare("PRAGMA table_info(media_items);")?;
            let mut rows = stmt.query([])?;
            let mut has_items_table = false;
            let mut has_trashed_col = false;
            while let Some(row) = rows.next()? {
                has_items_table = true;
                let col_name: String = row.get(1)?;
                if col_name == "is_trashed" {
                    has_trashed_col = true;
                }
            }
            has_items_table && !has_trashed_col
        };

        if has_old_schema {
            conn.execute_batch(
                r#"
                DROP TABLE IF EXISTS media_items;
                DROP TABLE IF EXISTS media_search_fts;
                DROP TABLE IF EXISTS timeline_buckets;
                DROP TABLE IF EXISTS smart_albums;
                "#,
            )?;
        }

        conn.execute_batch(
            r#"
            -- Core Media Catalog v2
            CREATE TABLE IF NOT EXISTS media_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_path TEXT NOT NULL UNIQUE,
                parent_dir TEXT NOT NULL,
                file_name TEXT NOT NULL,
                file_size INTEGER NOT NULL,
                mtime_epoch_ms INTEGER NOT NULL,

                -- Dimensions & Media Classification
                media_type TEXT NOT NULL CHECK(media_type IN ('image', 'video', 'raw')),
                width INTEGER NOT NULL,
                height INTEGER NOT NULL,
                duration_ms INTEGER DEFAULT NULL,

                -- EXIF Temporal & Spatial Attributes
                timestamp_epoch_ms INTEGER NOT NULL,
                year INTEGER NOT NULL,
                month INTEGER NOT NULL,
                day INTEGER NOT NULL,
                latitude REAL DEFAULT NULL,
                longitude REAL DEFAULT NULL,
                altitude REAL DEFAULT NULL,

                -- Reverse Geocoded Metadata
                country_code TEXT DEFAULT NULL,
                admin1_region TEXT DEFAULT NULL,
                city TEXT DEFAULT NULL,

                -- Camera Hardware Metadata
                make TEXT DEFAULT NULL,
                model TEXT DEFAULT NULL,
                lens TEXT DEFAULT NULL,
                focal_length REAL DEFAULT NULL,
                f_number REAL DEFAULT NULL,
                iso INTEGER DEFAULT NULL,
                exposure_time TEXT DEFAULT NULL,

                -- State Flags
                is_favorite INTEGER NOT NULL DEFAULT 0 CHECK(is_favorite IN (0, 1)),
                is_hidden INTEGER NOT NULL DEFAULT 0 CHECK(is_hidden IN (0, 1)),
                is_trashed INTEGER NOT NULL DEFAULT 0 CHECK(is_trashed IN (0, 1))
            );

            -- Performance Indexes for 120 FPS Scrolling & Hierarchy
            CREATE INDEX IF NOT EXISTS idx_timeline_active 
                ON media_items(timestamp_epoch_ms DESC) 
                WHERE is_trashed = 0 AND is_hidden = 0;

            CREATE INDEX IF NOT EXISTS idx_folder_listing 
                ON media_items(parent_dir, file_name) 
                WHERE is_trashed = 0;

            CREATE INDEX IF NOT EXISTS idx_geo_lookup 
                ON media_items(latitude, longitude) 
                WHERE latitude IS NOT NULL;

            -- Temporal Pre-aggregation for Instant Year/Month/Day Jump Lists
            CREATE TABLE IF NOT EXISTS timeline_buckets (
                bucket_type TEXT NOT NULL CHECK(bucket_type IN ('year', 'month', 'day')),
                bucket_key TEXT NOT NULL,
                start_timestamp INTEGER NOT NULL,
                end_timestamp INTEGER NOT NULL,
                item_count INTEGER NOT NULL,
                cover_media_id INTEGER,
                PRIMARY KEY (bucket_type, bucket_key),
                FOREIGN KEY(cover_media_id) REFERENCES media_items(id) ON DELETE SET NULL
            );

            -- Tier 1: Full-Text Search (SQLite FTS5)
            CREATE VIRTUAL TABLE IF NOT EXISTS media_search_fts USING fts5(
                file_name,
                parent_dir,
                city,
                admin1_region,
                country_code,
                model,
                content='media_items',
                content_rowid='id'
            );

            -- Automatic FTS5 Synchronization Triggers
            CREATE TRIGGER IF NOT EXISTS trg_media_insert AFTER INSERT ON media_items BEGIN
                INSERT INTO media_search_fts(rowid, file_name, parent_dir, city, admin1_region, country_code, model)
                VALUES (new.id, new.file_name, new.parent_dir, new.city, new.admin1_region, new.country_code, new.model);
            END;

            CREATE TRIGGER IF NOT EXISTS trg_media_delete AFTER DELETE ON media_items BEGIN
                INSERT INTO media_search_fts(media_search_fts, rowid, file_name, parent_dir, city, admin1_region, country_code, model)
                VALUES('delete', old.id, old.file_name, old.parent_dir, old.city, old.admin1_region, old.country_code, old.model);
            END;

            CREATE TRIGGER IF NOT EXISTS trg_media_update AFTER UPDATE ON media_items BEGIN
                INSERT INTO media_search_fts(media_search_fts, rowid, file_name, parent_dir, city, admin1_region, country_code, model)
                VALUES('delete', old.id, old.file_name, old.parent_dir, old.city, old.admin1_region, old.country_code, old.model);
                INSERT INTO media_search_fts(rowid, file_name, parent_dir, city, admin1_region, country_code, model)
                VALUES (new.id, new.file_name, new.parent_dir, new.city, new.admin1_region, new.country_code, new.model);
            END;

            -- Smart Albums Storage
            CREATE TABLE IF NOT EXISTS smart_albums (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                icon_name TEXT NOT NULL DEFAULT 'folder-heart',
                match_mode TEXT NOT NULL CHECK(match_mode IN ('all', 'any')),
                rule_definition_json TEXT NOT NULL,
                created_at_epoch_ms INTEGER NOT NULL
            );

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
                    file_path, parent_dir, file_name, file_size, mtime_epoch_ms,
                    media_type, width, height, duration_ms,
                    timestamp_epoch_ms, year, month, day,
                    latitude, longitude, altitude,
                    country_code, admin1_region, city,
                    make, model, lens, focal_length, f_number, iso, exposure_time,
                    is_favorite, is_hidden, is_trashed
                ) VALUES (
                    ?1, ?2, ?3, ?4, ?5,
                    ?6, ?7, ?8, ?9,
                    ?10, ?11, ?12, ?13,
                    ?14, ?15, ?16,
                    ?17, ?18, ?19,
                    ?20, ?21, ?22, ?23, ?24, ?25, ?26,
                    ?27, ?28, ?29
                )
                ON CONFLICT(file_path) DO UPDATE SET
                    parent_dir = excluded.parent_dir,
                    file_name = excluded.file_name,
                    file_size = excluded.file_size,
                    mtime_epoch_ms = excluded.mtime_epoch_ms,
                    media_type = excluded.media_type,
                    width = excluded.width,
                    height = excluded.height,
                    duration_ms = excluded.duration_ms,
                    timestamp_epoch_ms = excluded.timestamp_epoch_ms,
                    year = excluded.year,
                    month = excluded.month,
                    day = excluded.day,
                    latitude = excluded.latitude,
                    longitude = excluded.longitude,
                    altitude = excluded.altitude,
                    country_code = excluded.country_code,
                    admin1_region = excluded.admin1_region,
                    city = excluded.city,
                    make = excluded.make,
                    model = excluded.model,
                    lens = excluded.lens,
                    focal_length = excluded.focal_length,
                    f_number = excluded.f_number,
                    iso = excluded.iso,
                    exposure_time = excluded.exposure_time,
                    is_favorite = excluded.is_favorite,
                    is_hidden = excluded.is_hidden,
                    is_trashed = excluded.is_trashed
                RETURNING id;
                "#,
            )?;

            for item in items {
                let path_str = item.path.to_string_lossy();
                let type_str = match item.media_type {
                    MediaType::Image => "image",
                    MediaType::Video => "video",
                    MediaType::Raw => "raw",
                };

                let id: i64 = stmt.query_row(
                    params![
                        path_str.as_ref(),
                        item.parent_dir,
                        item.file_name,
                        item.file_size,
                        item.mtime_epoch_ms,
                        type_str,
                        item.width,
                        item.height,
                        item.duration_ms,
                        item.timestamp_epoch_ms,
                        item.year,
                        item.month,
                        item.day,
                        item.latitude,
                        item.longitude,
                        item.altitude,
                        item.country_code,
                        item.admin1_region,
                        item.city,
                        item.make,
                        item.model,
                        item.lens,
                        item.focal_length,
                        item.f_number,
                        item.iso,
                        item.exposure_time,
                        item.is_favorite as i32,
                        item.is_hidden as i32,
                        item.is_trashed as i32,
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

    pub fn compute_timeline_buckets(&self) -> Result<()> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;

        // Clear existing buckets
        tx.execute("DELETE FROM timeline_buckets;", [])?;

        // Compute Year buckets
        tx.execute(
            r#"
            INSERT INTO timeline_buckets (bucket_type, bucket_key, start_timestamp, end_timestamp, item_count, cover_media_id)
            SELECT 
                'year' AS bucket_type,
                CAST(year AS TEXT) AS bucket_key,
                MIN(timestamp_epoch_ms) AS start_timestamp,
                MAX(timestamp_epoch_ms) AS end_timestamp,
                COUNT(*) AS item_count,
                MAX(id) AS cover_media_id
            FROM media_items
            WHERE is_trashed = 0 AND is_hidden = 0
            GROUP BY year
            ORDER BY year DESC;
            "#,
            [],
        )?;

        // Compute Month buckets (YYYY-MM)
        tx.execute(
            r#"
            INSERT INTO timeline_buckets (bucket_type, bucket_key, start_timestamp, end_timestamp, item_count, cover_media_id)
            SELECT 
                'month' AS bucket_type,
                printf('%04d-%02d', year, month) AS bucket_key,
                MIN(timestamp_epoch_ms) AS start_timestamp,
                MAX(timestamp_epoch_ms) AS end_timestamp,
                COUNT(*) AS item_count,
                MAX(id) AS cover_media_id
            FROM media_items
            WHERE is_trashed = 0 AND is_hidden = 0
            GROUP BY year, month
            ORDER BY year DESC, month DESC;
            "#,
            [],
        )?;

        // Compute Day buckets (YYYY-MM-DD)
        tx.execute(
            r#"
            INSERT INTO timeline_buckets (bucket_type, bucket_key, start_timestamp, end_timestamp, item_count, cover_media_id)
            SELECT 
                'day' AS bucket_type,
                printf('%04d-%02d-%02d', year, month, day) AS bucket_key,
                MIN(timestamp_epoch_ms) AS start_timestamp,
                MAX(timestamp_epoch_ms) AS end_timestamp,
                COUNT(*) AS item_count,
                MAX(id) AS cover_media_id
            FROM media_items
            WHERE is_trashed = 0 AND is_hidden = 0
            GROUP BY year, month, day
            ORDER BY year DESC, month DESC, day DESC;
            "#,
            [],
        )?;

        tx.commit()?;
        Ok(())
    }

    pub fn get_timeline_buckets(&self, bucket_type: &str) -> Result<Vec<TimelineBucket>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            r#"
            SELECT bucket_type, bucket_key, start_timestamp, end_timestamp, item_count, cover_media_id
            FROM timeline_buckets
            WHERE bucket_type = ?1
            ORDER BY start_timestamp DESC;
            "#,
        )?;

        let rows = stmt.query_map(params![bucket_type], |row| {
            let b_type: String = row.get(0)?;
            let b_key: String = row.get(1)?;
            let start: i64 = row.get(2)?;
            let end: i64 = row.get(3)?;
            let count: i64 = row.get(4)?;
            let cover_id: Option<i64> = row.get(5)?;

            Ok(TimelineBucket {
                bucket_type: b_type,
                bucket_key: b_key,
                start_timestamp: start,
                end_timestamp: end,
                item_count: count as usize,
                cover_media_id: cover_id.map(|id| MediaId(id as u64)),
            })
        })?;

        let mut buckets = Vec::new();
        for r in rows {
            buckets.push(r?);
        }
        Ok(buckets)
    }

    pub fn total_count(&self) -> Result<usize> {
        let conn = self.conn.lock();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM media_items WHERE is_trashed = 0 AND is_hidden = 0;",
            [],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    pub fn get_item_slice(&self, offset: usize, limit: usize) -> Result<Vec<MediaItem>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            r#"
            SELECT 
                id, file_path, parent_dir, file_name, file_size, mtime_epoch_ms,
                media_type, width, height, duration_ms,
                timestamp_epoch_ms, year, month, day,
                latitude, longitude, altitude,
                country_code, admin1_region, city,
                make, model, lens, focal_length, f_number, iso, exposure_time,
                is_favorite, is_hidden, is_trashed
            FROM media_items
            WHERE is_trashed = 0 AND is_hidden = 0
            ORDER BY timestamp_epoch_ms DESC, id DESC
            LIMIT ?1 OFFSET ?2;
            "#,
        )?;

        Self::map_query_rows(&mut stmt, params![limit as i64, offset as i64])
    }

    pub fn get_item_by_id(&self, id: MediaId) -> Result<Option<MediaItem>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            r#"
            SELECT 
                id, file_path, parent_dir, file_name, file_size, mtime_epoch_ms,
                media_type, width, height, duration_ms,
                timestamp_epoch_ms, year, month, day,
                latitude, longitude, altitude,
                country_code, admin1_region, city,
                make, model, lens, focal_length, f_number, iso, exposure_time,
                is_favorite, is_hidden, is_trashed
            FROM media_items
            WHERE id = ?1;
            "#,
        )?;

        let mut items = Self::map_query_rows(&mut stmt, params![id.0 as i64])?;
        Ok(items.pop())
    }

    pub fn search_fts(&self, query: &str, limit: usize) -> Result<Vec<MediaItem>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            r#"
            SELECT 
                m.id, m.file_path, m.parent_dir, m.file_name, m.file_size, m.mtime_epoch_ms,
                m.media_type, m.width, m.height, m.duration_ms,
                m.timestamp_epoch_ms, m.year, m.month, m.day,
                m.latitude, m.longitude, m.altitude,
                m.country_code, m.admin1_region, m.city,
                m.make, m.model, m.lens, m.focal_length, m.f_number, m.iso, m.exposure_time,
                m.is_favorite, m.is_hidden, m.is_trashed
            FROM media_search_fts f
            JOIN media_items m ON f.rowid = m.id
            WHERE media_search_fts MATCH ?1 AND m.is_trashed = 0 AND m.is_hidden = 0
            ORDER BY m.timestamp_epoch_ms DESC
            LIMIT ?2;
            "#,
        )?;

        Self::map_query_rows(&mut stmt, params![query, limit as i64])
    }

    pub fn search_structured(&self, query: &StructuredQuery, limit: usize, offset: usize) -> Result<Vec<MediaItem>> {
        let (sql, params) = query.synthesize_query(limit, offset);
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(&sql)?;
        Self::map_query_rows(&mut stmt, rusqlite::params_from_iter(&params))
    }

    pub fn search_structured_count(&self, query: &StructuredQuery) -> Result<usize> {
        let (sql, params) = query.synthesize_count_query();
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(&sql)?;
        let count: i64 = stmt.query_row(rusqlite::params_from_iter(&params), |row| row.get(0))?;
        Ok(count as usize)
    }

    pub fn query_system_album(&self, album: SystemAlbum, limit: usize, offset: usize) -> Result<Vec<MediaItem>> {
        let (sql, params) = album.synthesize_query(limit, offset);
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(&sql)?;
        Self::map_query_rows(&mut stmt, rusqlite::params_from_iter(&params))
    }

    pub fn query_system_album_count(&self, album: SystemAlbum) -> Result<usize> {
        let (sql, params) = album.synthesize_count_query();
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(&sql)?;
        let count: i64 = stmt.query_row(rusqlite::params_from_iter(&params), |row| row.get(0))?;
        Ok(count as usize)
    }

    pub fn create_smart_album(
        &self,
        title: &str,
        icon_name: &str,
        match_mode: MatchMode,
        rules: &[SmartAlbumRule],
    ) -> Result<SmartAlbum> {
        let conn = self.conn.lock();
        let match_mode_str = match match_mode {
            MatchMode::All => "all",
            MatchMode::Any => "any",
        };
        let rules_json = serde_json::to_string(rules)?;
        let now_ms = chrono::Utc::now().timestamp_millis();

        conn.execute(
            r#"
            INSERT INTO smart_albums (title, icon_name, match_mode, rule_definition_json, created_at_epoch_ms)
            VALUES (?1, ?2, ?3, ?4, ?5);
            "#,
            params![title, icon_name, match_mode_str, rules_json, now_ms],
        )?;

        let id = conn.last_insert_rowid();
        Ok(SmartAlbum {
            id,
            title: title.to_string(),
            icon_name: icon_name.to_string(),
            match_mode,
            rules: rules.to_vec(),
            created_at_epoch_ms: now_ms,
        })
    }

    pub fn get_smart_album(&self, id: i64) -> Result<Option<SmartAlbum>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, title, icon_name, match_mode, rule_definition_json, created_at_epoch_ms
            FROM smart_albums
            WHERE id = ?1;
            "#,
        )?;

        let album = stmt.query_row(params![id], |row| {
            let id: i64 = row.get(0)?;
            let title: String = row.get(1)?;
            let icon_name: String = row.get(2)?;
            let match_mode_str: String = row.get(3)?;
            let rule_json: String = row.get(4)?;
            let created_at: i64 = row.get(5)?;
            Ok((id, title, icon_name, match_mode_str, rule_json, created_at))
        });

        match album {
            Ok((id, title, icon_name, match_mode_str, rule_json, created_at)) => {
                let match_mode = if match_mode_str == "any" {
                    MatchMode::Any
                } else {
                    MatchMode::All
                };
                let rules: Vec<SmartAlbumRule> = serde_json::from_str(&rule_json).unwrap_or_default();
                Ok(Some(SmartAlbum {
                    id,
                    title,
                    icon_name,
                    match_mode,
                    rules,
                    created_at_epoch_ms: created_at,
                }))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn list_smart_albums(&self) -> Result<Vec<SmartAlbum>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, title, icon_name, match_mode, rule_definition_json, created_at_epoch_ms
            FROM smart_albums
            ORDER BY id ASC;
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            let id: i64 = row.get(0)?;
            let title: String = row.get(1)?;
            let icon_name: String = row.get(2)?;
            let match_mode_str: String = row.get(3)?;
            let rule_json: String = row.get(4)?;
            let created_at: i64 = row.get(5)?;
            Ok((id, title, icon_name, match_mode_str, rule_json, created_at))
        })?;

        let mut albums = Vec::new();
        for r in rows {
            let (id, title, icon_name, match_mode_str, rule_json, created_at) = r?;
            let match_mode = if match_mode_str == "any" {
                MatchMode::Any
            } else {
                MatchMode::All
            };
            let rules: Vec<SmartAlbumRule> = serde_json::from_str(&rule_json).unwrap_or_default();
            albums.push(SmartAlbum {
                id,
                title,
                icon_name,
                match_mode,
                rules,
                created_at_epoch_ms: created_at,
            });
        }
        Ok(albums)
    }

    pub fn delete_smart_album(&self, id: i64) -> Result<bool> {
        let conn = self.conn.lock();
        let affected = conn.execute("DELETE FROM smart_albums WHERE id = ?1;", params![id])?;
        Ok(affected > 0)
    }

    pub fn query_smart_album(&self, album: &SmartAlbum, limit: usize, offset: usize) -> Result<Vec<MediaItem>> {
        let (sql, params) = album.synthesize_query(limit, offset)?;
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(&sql)?;
        Self::map_query_rows(&mut stmt, rusqlite::params_from_iter(&params))
    }

    pub fn query_smart_album_count(&self, album: &SmartAlbum) -> Result<usize> {
        let (sql, params) = album.synthesize_count_query()?;
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(&sql)?;
        let count: i64 = stmt.query_row(rusqlite::params_from_iter(&params), |row| row.get(0))?;
        Ok(count as usize)
    }

    pub fn toggle_favorite(&self, id: MediaId, is_favorite: bool) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE media_items SET is_favorite = ?1 WHERE id = ?2;",
            params![is_favorite as i32, id.0 as i64],
        )?;
        Ok(())
    }

    pub fn set_trashed(&self, id: MediaId, is_trashed: bool) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE media_items SET is_trashed = ?1 WHERE id = ?2;",
            params![is_trashed as i32, id.0 as i64],
        )?;
        Ok(())
    }

    pub fn remove_item_by_path(&self, path: &Path) -> Result<Option<MediaId>> {
        let conn = self.conn.lock();
        let path_str = path.to_string_lossy();
        let id_opt: Option<i64> = conn
            .query_row(
                "SELECT id FROM media_items WHERE file_path = ?1;",
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
            let mut stmt = tx.prepare("SELECT id, file_path FROM media_items;")?;
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
        conn.execute("DELETE FROM timeline_buckets;", [])?;
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

    fn map_query_rows(
        stmt: &mut rusqlite::Statement,
        params: impl rusqlite::Params,
    ) -> Result<Vec<MediaItem>> {
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

            let duration_secs = duration_ms.map(|ms| ms as f64 / 1000.0).unwrap_or(0.0);

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
                duration_secs,
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
