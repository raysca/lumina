use anyhow::Result;
use chrono::Datelike;
use rusqlite::{params, OptionalExtension};
use std::collections::HashMap;
use std::path::PathBuf;

use super::Catalog;
use crate::{normalize_quarter_turns, MediaId, MediaItem, MediaType, SortSpec};

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
                    is_favorite, is_hidden, is_trashed, title, added_epoch_ms
                ) VALUES (
                    ?1, ?2, ?3, ?4, ?5,
                    ?6, ?7, ?8, ?9,
                    ?10, ?11, ?12, ?13,
                    ?14, ?15, ?16,
                    ?17, ?18, ?19,
                    ?20, ?21, ?22, ?23, ?24, ?25, ?26,
                    ?27, ?28, ?29, ?30,
                    CAST(unixepoch('subsec') * 1000 AS INTEGER)
                )
                -- added_epoch_ms is set once, on first insert, never on update.
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
                RETURNING id, is_favorite, is_hidden, is_trashed, title;
                "#,
            )?;

            for item in items {
                let media_type_str = match item.media_type {
                    MediaType::Image => "image",
                    MediaType::Video => "video",
                    MediaType::Raw => "raw",
                };

                let path_str = item.path.to_string_lossy().to_string();

                let (row_id, db_fav, db_hid, db_trash, db_title): (
                    i64,
                    i32,
                    i32,
                    i32,
                    Option<String>,
                ) = stmt.query_row(
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
                        item.title,
                    ],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                        ))
                    },
                )?;

                let mut updated = item.clone();
                updated.id = MediaId(row_id as u64);
                updated.is_favorite = db_fav != 0;
                updated.is_hidden = db_hid != 0;
                updated.is_trashed = db_trash != 0;
                updated.title = db_title;
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

    /// Fills `width`/`height` of videos stored without a size, touching no
    /// other column (a full re-insert would overwrite edited dates and
    /// places). Files are measured with the catalog lock released.
    /// Only MP4/MOV/M4V files carry the headers `measure` reads.
    /// ponytail: one whose `moov` lies outside the scanned head/tail windows is
    /// re-read on every scan; walk the box tree if that shows up in profiles.
    pub fn fill_video_sizes(
        &self,
        measure: impl Fn(&std::path::Path) -> Option<(u32, u32)>,
    ) -> Result<usize> {
        let sizeless: Vec<(i64, String)> = {
            let conn = self.conn.lock();
            let mut stmt = conn.prepare(
                "SELECT id, file_path FROM media_items
                 WHERE media_type = 'video' AND (width = 0 OR height = 0)
                   AND (lower(file_path) LIKE '%.mp4' OR lower(file_path) LIKE '%.mov'
                        OR lower(file_path) LIKE '%.m4v');",
            )?;
            let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        let sizes: Vec<(i64, (u32, u32))> = sizeless
            .into_iter()
            .filter_map(|(id, path)| Some((id, measure(std::path::Path::new(&path))?)))
            .collect();
        if sizes.is_empty() {
            return Ok(0);
        }
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        for (id, (w, h)) in &sizes {
            tx.execute(
                "UPDATE media_items SET width = ?1, height = ?2 WHERE id = ?3;",
                params![w, h, id],
            )?;
        }
        tx.commit()?;
        Ok(sizes.len())
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
        self.get_item_slice_sorted(offset, limit, SortSpec::default())
    }

    pub fn get_item_slice_sorted(
        &self,
        offset: usize,
        limit: usize,
        sort: SortSpec,
    ) -> Result<Vec<MediaItem>> {
        let conn = self.conn.lock();
        let sql = format!(
            r#"
            SELECT 
                id, file_path, parent_dir, file_name, file_size, mtime_epoch_ms,
                media_type, width, height, duration_ms,
                timestamp_epoch_ms, year, month, day,
                latitude, longitude, altitude,
                country_code, admin1_region, city,
                make, model, lens, focal_length, f_number, iso, exposure_time,
                is_favorite, is_hidden, is_trashed, title
            FROM media_items
            WHERE is_trashed = 0 AND is_hidden = 0
            ORDER BY {}
            LIMIT ?1 OFFSET ?2;
            "#,
            sort.order_by("")
        );
        let mut stmt = conn.prepare_cached(&sql)?;

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
                is_favorite, is_hidden, is_trashed, title
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

    /// When the item first entered the catalog; `None` for rows added before
    /// schema v9 or unknown ids.
    pub fn get_added_epoch_ms(&self, id: MediaId) -> Result<Option<i64>> {
        let conn = self.conn.lock();
        let added: Option<Option<i64>> = conn
            .query_row(
                "SELECT added_epoch_ms FROM media_items WHERE id = ?1;",
                params![id.0 as i64],
                |row| row.get(0),
            )
            .optional()?;
        Ok(added.flatten())
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

    /// Sets the persisted clockwise quarter-turn rotation for every id.
    ///
    /// Only the catalog column changes; the original file is never rewritten.
    /// The update is one transaction and normalizes `quarters` to 0..=3. An
    /// empty id list is a no-op, and ids with no row are skipped without
    /// failing the batch.
    pub fn set_rotation(&self, ids: &[MediaId], quarters: u8) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let normalized = normalize_quarter_turns(quarters as i32);
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        {
            let mut stmt =
                tx.prepare_cached("UPDATE media_items SET rotation_quarters = ?1 WHERE id = ?2;")?;
            for id in ids {
                stmt.execute(params![normalized as i32, id.0 as i64])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Persisted clockwise quarter turns for `id` (0 when unset or absent).
    pub fn get_rotation(&self, id: MediaId) -> Result<u8> {
        let conn = self.conn.lock();
        let quarters: Option<i32> = conn
            .query_row(
                "SELECT rotation_quarters FROM media_items WHERE id = ?1;",
                params![id.0 as i64],
                |row| row.get(0),
            )
            .optional()?;
        Ok(normalize_quarter_turns(quarters.unwrap_or(0)))
    }

    pub fn set_title(&self, id: MediaId, title: Option<&str>) -> Result<()> {
        let conn = self.conn.lock();
        let trimmed = title.map(str::trim).filter(|value| !value.is_empty());
        match trimmed {
            Some(value) => {
                conn.execute(
                    "UPDATE media_items SET title = ?1 WHERE id = ?2;",
                    params![value, id.0 as i64],
                )?;
            }
            None => {
                conn.execute(
                    "UPDATE media_items SET title = NULL WHERE id = ?1;",
                    params![id.0 as i64],
                )?;
            }
        }
        Ok(())
    }

    /// Updates the capture timestamp and derived year/month/day, then rebuilds
    /// timeline buckets so year/month scrubbing reflects the edit immediately.
    pub fn set_timestamp(&self, id: MediaId, epoch_ms: i64) -> Result<()> {
        let Some(datetime) = chrono::DateTime::from_timestamp_millis(epoch_ms) else {
            anyhow::bail!("invalid timestamp: {epoch_ms}");
        };
        {
            let conn = self.conn.lock();
            conn.execute(
                "UPDATE media_items
                 SET timestamp_epoch_ms = ?1, year = ?2, month = ?3, day = ?4
                 WHERE id = ?5;",
                params![
                    epoch_ms,
                    datetime.year(),
                    datetime.month(),
                    datetime.day(),
                    id.0 as i64
                ],
            )?;
        }
        self.compute_timeline_buckets()
    }

    pub fn set_location(
        &self,
        id: MediaId,
        latitude: Option<f64>,
        longitude: Option<f64>,
    ) -> Result<()> {
        // The place columns are derived from the coordinates (as at scan
        // time), so they move with them: re-geocoded on edit, cleared on
        // removal. The FTS update trigger reindexes them.
        static GEOCODER: std::sync::OnceLock<crate::OfflineGeocoder> = std::sync::OnceLock::new();
        let place = latitude.zip(longitude).and_then(|(lat, lon)| {
            GEOCODER
                .get_or_init(crate::OfflineGeocoder::new)
                .reverse_geocode(lat, lon)
        });
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE media_items SET latitude = ?1, longitude = ?2, city = ?3, admin1_region = ?4, country_code = ?5 WHERE id = ?6;",
            params![
                latitude,
                longitude,
                place.as_ref().map(|p| &p.city),
                place.as_ref().map(|p| &p.admin1_region),
                place.as_ref().map(|p| &p.country_code),
                id.0 as i64
            ],
        )?;
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SCHEMA_VERSION;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_catalog(prefix: &str) -> (PathBuf, PathBuf) {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("lumina_{prefix}_{nanos}"));
        std::fs::create_dir_all(&dir).expect("temp catalog dir must be creatable");
        let db = dir.join("catalog.db");
        (dir, db)
    }

    fn sample_item(index: usize) -> MediaItem {
        let file_name = format!("IMG_{index:04}.HEIC");
        MediaItem {
            id: MediaId(0),
            path: PathBuf::from(format!("/library/{file_name}")),
            parent_dir: "/library".to_string(),
            file_name,
            title: None,
            file_size: 4_500_000,
            mtime_epoch_ms: 1_642_200_000_000,
            media_type: MediaType::Image,
            width: 4032,
            height: 3024,
            duration_ms: None,
            timestamp_epoch_ms: 1_642_200_000_000,
            year: 2022,
            month: 1,
            day: 15,
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

    fn user_version(catalog: &Catalog) -> i64 {
        catalog
            .conn
            .lock()
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .expect("user_version must be readable")
    }

    #[test]
    fn legacy_smart_albums_table_is_migrated_and_keeps_albums() {
        let (dir, db) = temp_catalog("legacy_smart_albums");
        drop(Catalog::open(&db).expect("catalog must open"));
        // The first smart-album engine's layout, as found in real catalogs.
        {
            let conn = rusqlite::Connection::open(&db).unwrap();
            conn.execute_batch(
                r#"
                DROP TABLE smart_albums;
                CREATE TABLE smart_albums (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    title TEXT NOT NULL,
                    icon_name TEXT NOT NULL DEFAULT 'folder-heart',
                    match_mode TEXT NOT NULL CHECK(match_mode IN ('all', 'any')),
                    rule_definition_json TEXT NOT NULL,
                    created_at_epoch_ms INTEGER NOT NULL
                );
                INSERT INTO smart_albums (title, match_mode, rule_definition_json, created_at_epoch_ms)
                VALUES ('Sunsets', 'all', '[{"field":"tag","operator":"equals","value":"sunset"}]', 1);
                PRAGMA user_version = 7;
                "#,
            )
            .unwrap();
        }

        let catalog = Catalog::open(&db).expect("catalog must reopen and migrate");
        assert_eq!(user_version(&catalog), SCHEMA_VERSION);
        let albums = catalog
            .list_smart_albums()
            .expect("legacy albums must list");
        assert_eq!(albums.len(), 1, "the saved album survives the migration");
        assert_eq!(albums[0].title, "Sunsets");
        assert_eq!(albums[0].rules.len(), 1);
        catalog
            .create_smart_album("New", "folder", crate::MatchMode::Any, &albums[0].rules)
            .expect("saving must work on a migrated catalog");
        catalog
            .update_smart_album(
                albums[0].id,
                "Renamed",
                "folder",
                crate::MatchMode::All,
                &[],
            )
            .expect("editing must work on a migrated catalog");
        drop(catalog);
        Catalog::open(&db).expect("the migration is idempotent");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn legacy_search_triggers_are_dropped_and_their_stale_entries_rebuilt() {
        let (dir, db) = temp_catalog("legacy_fts");
        let catalog = Catalog::open(&db).expect("catalog must open");
        let mut item = sample_item(1);
        item.city = Some("Lisbon".to_string());
        let id = catalog.insert_or_update_batch(&[item]).unwrap()[0].id;
        drop(catalog);
        // A legacy trigger and the stale entry it left behind for the row.
        {
            let conn = rusqlite::Connection::open(&db).unwrap();
            conn.execute_batch(&format!(
                "CREATE TRIGGER trg_media_update AFTER UPDATE ON media_items BEGIN SELECT 1; END;
                 INSERT INTO media_search_fts(rowid, city) VALUES ({}, 'Oldtown');",
                id.0
            ))
            .unwrap();
        }

        let catalog = Catalog::open(&db).expect("catalog must reopen and migrate");
        let triggers: i64 = catalog
            .conn
            .lock()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name LIKE 'trg_media_%'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(triggers, 0, "legacy search triggers must be dropped");
        assert!(
            catalog.search_fts("Oldtown", 10).unwrap().is_empty(),
            "stale legacy index entries must not match after the upgrade"
        );
        assert_eq!(catalog.search_fts("Lisbon", 10).unwrap().len(), 1);

        drop(catalog);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rotation_migration_is_idempotent_and_roundtrips() {
        let (dir, db) = temp_catalog("rotation_migration");
        let catalog = Catalog::open(&db).expect("catalog must open");
        assert_eq!(
            user_version(&catalog),
            SCHEMA_VERSION,
            "a new catalog must be stamped with the current schema version"
        );

        let inserted = catalog
            .insert_or_update_batch(&[sample_item(1)])
            .expect("sample item must insert");
        let id = inserted[0].id;
        assert_eq!(
            catalog.get_rotation(id).expect("rotation read must work"),
            0,
            "new items start unrotated"
        );
        catalog
            .set_rotation(&[id], 1)
            .expect("rotation write must succeed");
        assert_eq!(catalog.get_rotation(id).expect("rotation read"), 1);

        // Simulate a catalog created before display rotation existed.
        catalog
            .conn
            .lock()
            .execute("ALTER TABLE media_items DROP COLUMN rotation_quarters;", [])
            .expect("pre-migration schema must be reproducible");
        drop(catalog);

        let catalog = Catalog::open(&db).expect("catalog must reopen and migrate");
        assert_eq!(
            user_version(&catalog),
            SCHEMA_VERSION,
            "the migration must stamp the bumped schema version"
        );
        assert_eq!(
            catalog
                .get_rotation(id)
                .expect("rotation read after migration"),
            0,
            "the migration must default existing rows to 0"
        );
        catalog
            .set_rotation(&[id], 2)
            .expect("rotation write after migration");
        drop(catalog);

        // Reopening runs the same probes again; the migration must be idempotent.
        let catalog = Catalog::open(&db).expect("catalog must reopen twice");
        assert_eq!(
            catalog.get_rotation(id).expect("rotation must persist"),
            2,
            "rotation must survive a reopen"
        );
        drop(catalog);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn set_rotation_is_transactional_normalized_and_survives_rescan() {
        let (dir, db) = temp_catalog("rotation_set");
        let catalog = Catalog::open(&db).expect("catalog must open");
        let inserted = catalog
            .insert_or_update_batch(&[sample_item(1), sample_item(2)])
            .expect("sample items must insert");
        let ids: Vec<MediaId> = inserted.iter().map(|item| item.id).collect();

        assert!(
            catalog.set_rotation(&[], 3).is_ok(),
            "an empty id list must be a successful no-op"
        );

        catalog
            .set_rotation(&ids, 7)
            .expect("out-of-range turns must normalize, not fail");
        assert_eq!(catalog.get_rotation(ids[0]).expect("rotation read"), 3);
        assert_eq!(catalog.get_rotation(ids[1]).expect("rotation read"), 3);

        // A rescan re-inserts the same rows through ON CONFLICT; the update path
        // must not reset the user's rotation.
        let mut rescanned = inserted.clone();
        for item in &mut rescanned {
            item.file_size += 1;
        }
        catalog
            .insert_or_update_batch(&rescanned)
            .expect("rescan must update rows");
        assert_eq!(
            catalog
                .get_rotation(ids[0])
                .expect("rotation read after rescan"),
            3,
            "a rescan must preserve the stored rotation"
        );

        // Unknown ids must not abort the batch for known ones.
        catalog
            .set_rotation(&[ids[0], MediaId(9_999)], 2)
            .expect("mixed id list must succeed");
        assert_eq!(catalog.get_rotation(ids[0]).expect("rotation read"), 2);
        assert_eq!(
            catalog
                .get_rotation(MediaId(9_999))
                .expect("missing id reads 0"),
            0
        );

        // The shared media mapper must still read the full row after the column
        // was appended to the table.
        let item = catalog
            .get_item_by_id(ids[0])
            .expect("item lookup must work")
            .expect("inserted item must exist");
        assert_eq!(item.file_name, "IMG_0001.HEIC");
        assert_eq!(item.file_size, rescanned[0].file_size);
        assert_eq!(item.is_favorite, rescanned[0].is_favorite);

        drop(catalog);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
