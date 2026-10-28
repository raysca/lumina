use anyhow::Result;
use rusqlite::{params, OptionalExtension};
use std::collections::HashMap;
use std::path::PathBuf;

use super::Catalog;
use crate::{MediaId, MediaItem, MediaType};

impl Catalog {
    pub fn insert_or_update_batch(&self, items: &[MediaItem]) -> Result<Vec<MediaItem>> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;

        let mut inserted = Vec::with_capacity(items.len());

        {
            let mut stmt = tx.prepare_cached(
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
                    file_size = excluded.file_size,
                    mtime_epoch_ms = excluded.mtime_epoch_ms,
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
                    exposure_time = excluded.exposure_time
                RETURNING id, is_favorite, is_hidden, is_trashed;
                "#,
            )?;

            for item in items {
                let media_type_str = match item.media_type {
                    MediaType::Image => "image",
                    MediaType::Video => "video",
                    MediaType::Raw => "raw",
                };

                let path_str = item.path.to_string_lossy().to_string();

                let (row_id, db_fav, db_hid, db_trash): (i64, i32, i32, i32) = stmt.query_row(
                    params![
                        path_str,
                        item.parent_dir,
                        item.file_name,
                        item.file_size,
                        item.mtime_epoch_ms,
                        media_type_str,
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
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )?;

                let mut updated = item.clone();
                updated.id = MediaId(row_id as u64);
                updated.is_favorite = db_fav != 0;
                updated.is_hidden = db_hid != 0;
                updated.is_trashed = db_trash != 0;
                inserted.push(updated);
            }
        }

        tx.commit()?;
        Ok(inserted)
    }

    /// Returns `(file_size, mtime_epoch_ms)` keyed by `PathBuf` for fast incremental rescans.
    pub fn get_file_signatures(&self) -> Result<HashMap<PathBuf, (u64, i64)>> {
        let conn = self.conn.lock();
        let mut stmt =
            conn.prepare_cached("SELECT file_path, file_size, mtime_epoch_ms FROM media_items;")?;
        let rows = stmt.query_map([], |row| {
            let path_str: String = row.get(0)?;
            let file_size: u64 = row.get(1)?;
            let mtime_epoch_ms: i64 = row.get(2)?;
            Ok((PathBuf::from(path_str), (file_size, mtime_epoch_ms)))
        })?;

        let mut map = HashMap::new();
        for r in rows {
            let (p, sig) = r?;
            map.insert(p, sig);
        }
        Ok(map)
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
        let mut stmt = conn.prepare_cached(
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
        let mut stmt = conn.prepare_cached(
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

        let mut rows = Self::map_query_rows(&mut stmt, params![id.0 as i64])?;
        Ok(rows.pop())
    }

    pub fn toggle_favorite(&self, id: MediaId, is_favorite: bool) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE media_items SET is_favorite = ?1 WHERE id = ?2;",
            params![is_favorite as i32, id.0 as i64],
        )?;
        Ok(())
    }

    pub fn batch_set_favorite(&self, ids: &[MediaId], is_favorite: bool) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        {
            let mut stmt =
                tx.prepare_cached("UPDATE media_items SET is_favorite = ?1 WHERE id = ?2;")?;
            let fav_val = is_favorite as i32;
            for id in ids {
                stmt.execute(params![fav_val, id.0 as i64])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn set_rating(&self, id: MediaId, rating: u8) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE media_items SET rating = ?1 WHERE id = ?2;",
            params![rating.min(5) as i32, id.0 as i64],
        )?;
        Ok(())
    }

    pub fn get_rating(&self, id: MediaId) -> Result<u8> {
        let conn = self.conn.lock();
        let r: Option<i32> = conn
            .query_row(
                "SELECT rating FROM media_items WHERE id = ?1;",
                params![id.0 as i64],
                |row| row.get(0),
            )
            .optional()?;
        Ok(r.unwrap_or(0).clamp(0, 5) as u8)
    }

    pub fn get_ratings_map(&self) -> Result<HashMap<MediaId, u8>> {
        let conn = self.conn.lock();
        let mut stmt =
            conn.prepare_cached("SELECT id, rating FROM media_items WHERE rating > 0;")?;
        let rows = stmt.query_map([], |row| {
            let id: i64 = row.get(0)?;
            let r: i32 = row.get(1)?;
            Ok((MediaId(id as u64), r.clamp(0, 5) as u8))
        })?;
        let mut map = HashMap::new();
        for row in rows {
            let (id, r) = row?;
            map.insert(id, r);
        }
        Ok(map)
    }

    pub fn set_color_label(&self, id: MediaId, label: u8) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE media_items SET color_label = ?1 WHERE id = ?2;",
            params![label.min(6) as i32, id.0 as i64],
        )?;
        Ok(())
    }

    pub fn get_color_labels_map(&self) -> Result<HashMap<MediaId, u8>> {
        let conn = self.conn.lock();
        let mut stmt =
            conn.prepare_cached("SELECT id, color_label FROM media_items WHERE color_label > 0;")?;
        let rows = stmt.query_map([], |row| {
            let id: i64 = row.get(0)?;
            let c: i32 = row.get(1)?;
            Ok((MediaId(id as u64), c.clamp(0, 6) as u8))
        })?;
        let mut map = HashMap::new();
        for row in rows {
            let (id, c) = row?;
            map.insert(id, c);
        }
        Ok(map)
    }

    pub fn set_caption(&self, id: MediaId, caption: &str) -> Result<()> {
        let conn = self.conn.lock();
        let media_id = id.0 as i64;
        let trimmed = caption.trim();

        // The caption currently reflected by the FTS index is needed to remove
        // the contentless FTS5 row before it is rewritten.
        let previous_caption: Option<String> = conn
            .query_row(
                "SELECT caption FROM media_captions WHERE media_id = ?1;",
                params![media_id],
                |row| row.get(0),
            )
            .optional()?;

        if trimmed.is_empty() {
            conn.execute(
                "DELETE FROM media_captions WHERE media_id = ?1;",
                params![media_id],
            )?;
        } else {
            conn.execute(
                "INSERT INTO media_captions (media_id, caption) VALUES (?1, ?2) ON CONFLICT(media_id) DO UPDATE SET caption = excluded.caption;",
                params![media_id, trimmed],
            )?;
        }

        Self::refresh_fts_caption_row(&conn, media_id, previous_caption.as_deref())?;
        Ok(())
    }

    /// Rewrites the contentless FTS5 row for `media_id` after a caption change.
    ///
    /// Contentless FTS5 rows can only be removed by supplying the column values
    /// that were originally indexed, so `previous_caption` must be the caption
    /// the FTS row was built from. The row is then reinserted using the current
    /// media metadata and caption, which keeps edits and clears searchable.
    fn refresh_fts_caption_row(
        conn: &rusqlite::Connection,
        media_id: i64,
        previous_caption: Option<&str>,
    ) -> Result<()> {
        let row = conn
            .query_row(
                r#"
                SELECT
                    m.file_name, m.parent_dir, m.city, m.admin1_region, m.country_code,
                    m.make, m.model, m.lens, CAST(m.year AS TEXT),
                    COALESCE((SELECT GROUP_CONCAT(tag, ' ') FROM media_tags WHERE media_id = m.id), ''),
                    COALESCE((SELECT caption FROM media_captions WHERE media_id = m.id), '')
                FROM media_items m
                WHERE m.id = ?1;
                "#,
                params![media_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, Option<String>>(6)?,
                        row.get::<_, Option<String>>(7)?,
                        row.get::<_, String>(8)?,
                        row.get::<_, String>(9)?,
                        row.get::<_, String>(10)?,
                    ))
                },
            )
            .optional()?;

        let Some((
            file_name,
            parent_dir,
            city,
            admin1_region,
            country_code,
            make,
            model,
            lens,
            year,
            tags,
            caption,
        )) = row
        else {
            return Ok(());
        };

        conn.execute(
            r#"
            INSERT INTO media_search_fts(
                media_search_fts, rowid, file_name, parent_dir, city, admin1_region, country_code, make, model, lens, year, tags, caption
            ) VALUES ('delete', ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12);
            "#,
            params![
                media_id,
                file_name,
                parent_dir,
                city,
                admin1_region,
                country_code,
                make,
                model,
                lens,
                year,
                tags,
                previous_caption.unwrap_or("")
            ],
        )?;

        conn.execute(
            r#"
            INSERT INTO media_search_fts(
                rowid, file_name, parent_dir, city, admin1_region, country_code, make, model, lens, year, tags, caption
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12);
            "#,
            params![
                media_id,
                file_name,
                parent_dir,
                city,
                admin1_region,
                country_code,
                make,
                model,
                lens,
                year,
                tags,
                caption
            ],
        )?;

        Ok(())
    }

    pub fn get_captions_map(&self) -> Result<HashMap<MediaId, String>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached("SELECT media_id, caption FROM media_captions;")?;
        let rows = stmt.query_map([], |row| {
            let id: i64 = row.get(0)?;
            let caption: String = row.get(1)?;
            Ok((MediaId(id as u64), caption))
        })?;
        let mut map = HashMap::new();
        for row in rows {
            let (id, c) = row?;
            map.insert(id, c);
        }
        Ok(map)
    }
}
