use anyhow::Result;
use chrono::Datelike;
use rusqlite::{params, OptionalExtension};
use std::collections::HashMap;
use std::path::PathBuf;

use super::files::{file_inode, PRESENT};
use super::Catalog;
use crate::text::{clean_caption, clean_text, MAX_TEXT_CHARS};
use crate::{normalize_quarter_turns, MediaId, MediaItem, MediaType, SortSpec};

/// One geotagged item for the map, without the rest of its row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeoPoint {
    pub id: MediaId,
    pub lat: f32,
    pub lon: f32,
}

impl Catalog {
    /// Upserts scanned items by path; scans and the watcher both land here.
    /// A date or place the user edited (`date_edited` / `place_edited`) is
    /// kept over the file's, and so is a duration while a rescan of the
    /// unchanged file fails to probe one. Returned items carry the scanned
    /// metadata plus the row's id, favorite/hidden/trashed, title and
    /// duration; read the row for edited values.
    /// ponytail: undoing an edit keeps its flag, so that item then ignores
    /// a later in-file date change; clear the flag on undo if that matters.
    ///
    /// Ingesting a file marks it present and records its inode. A path new
    /// to the catalog first takes over the newest missing row with the same
    /// inode and size: the file was moved or renamed, and keeps its metadata.
    pub fn insert_or_update_batch(&self, items: &[MediaItem]) -> Result<Vec<MediaItem>> {
        // Stat before locking: no file IO under the catalog lock.
        let inodes: Vec<Option<u64>> = items.iter().map(|item| file_inode(&item.path)).collect();
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let inserted = Self::upsert_in_tx(&tx, items, &inodes)?;
        tx.commit()?;
        Ok(inserted)
    }

    /// Upserts one network download and its source attribution in a single
    /// transaction, so an item never lands without the URL it came from.
    pub fn insert_with_source(
        &self,
        item: &MediaItem,
        source_url: &str,
        uploader: Option<&str>,
    ) -> Result<MediaItem> {
        let inodes = [file_inode(&item.path)];
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let mut inserted = Self::upsert_in_tx(&tx, std::slice::from_ref(item), &inodes)?
            .pop()
            .ok_or_else(|| anyhow::anyhow!("the catalog returned no row for {:?}", item.path))?;
        // The watcher may have catalogued the new file first; an upsert keeps
        // that row's favorite and title, so set the user's choices here.
        tx.execute(
            "UPDATE media_items SET is_favorite = ?1, title = ?2 WHERE id = ?3;",
            params![item.is_favorite as i32, item.title, inserted.id.0 as i64],
        )?;
        inserted.is_favorite = item.is_favorite;
        inserted.title = item.title.clone();
        Self::record_source_in(&tx, inserted.id, source_url, uploader, true)?;
        tx.commit()?;
        Ok(inserted)
    }

    pub(super) fn upsert_in_tx(
        tx: &rusqlite::Transaction,
        items: &[MediaItem],
        inodes: &[Option<u64>],
    ) -> Result<Vec<MediaItem>> {
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
                -- A date or place the user edited outlives the file's own.
                -- A failed duration probe (0 or NULL; container formats the
                -- header parser can't read fall back to Spotlight, which may
                -- not have indexed the file) keeps the stored duration while
                -- the file is unchanged. SET reads the old row on the right.
                ON CONFLICT(file_path) DO UPDATE SET
                    file_size = excluded.file_size,
                    mtime_epoch_ms = excluded.mtime_epoch_ms,
                    width = excluded.width,
                    height = excluded.height,
                    duration_ms = CASE
                        WHEN COALESCE(excluded.duration_ms, 0) = 0
                         AND file_size = excluded.file_size
                         AND mtime_epoch_ms = excluded.mtime_epoch_ms
                        THEN duration_ms ELSE excluded.duration_ms END,
                    timestamp_epoch_ms = CASE WHEN date_edited THEN timestamp_epoch_ms ELSE excluded.timestamp_epoch_ms END,
                    year = CASE WHEN date_edited THEN year ELSE excluded.year END,
                    month = CASE WHEN date_edited THEN month ELSE excluded.month END,
                    day = CASE WHEN date_edited THEN day ELSE excluded.day END,
                    latitude = CASE WHEN place_edited THEN latitude ELSE excluded.latitude END,
                    longitude = CASE WHEN place_edited THEN longitude ELSE excluded.longitude END,
                    altitude = excluded.altitude,
                    country_code = CASE WHEN place_edited THEN country_code ELSE excluded.country_code END,
                    admin1_region = CASE WHEN place_edited THEN admin1_region ELSE excluded.admin1_region END,
                    city = CASE WHEN place_edited THEN city ELSE excluded.city END,
                    make = excluded.make,
                    model = excluded.model,
                    lens = excluded.lens,
                    focal_length = excluded.focal_length,
                    f_number = excluded.f_number,
                    iso = excluded.iso,
                    exposure_time = excluded.exposure_time
                RETURNING id, is_favorite, is_hidden, is_trashed, title, duration_ms;
                "#,
            )?;

            let mut reconnect = tx.prepare_cached(
                r#"
                UPDATE media_items SET file_path = ?1, parent_dir = ?2, file_name = ?3
                WHERE id = (
                    SELECT s.media_id FROM media_file_state s
                    JOIN media_items m ON m.id = s.media_id
                    WHERE s.missing_since_epoch_ms IS NOT NULL
                      AND s.file_inode = ?4 AND m.file_size = ?5
                    ORDER BY s.missing_since_epoch_ms DESC LIMIT 1
                )
                AND NOT EXISTS (SELECT 1 FROM media_items WHERE file_path = ?1);
                "#,
            )?;
            let mut mark_present = tx.prepare_cached(
                r#"
                INSERT INTO media_file_state (media_id, file_inode, missing_since_epoch_ms)
                VALUES (?1, ?2, NULL)
                ON CONFLICT(media_id) DO UPDATE SET
                    file_inode = COALESCE(excluded.file_inode, file_inode),
                    missing_since_epoch_ms = NULL;
                "#,
            )?;

            for (item, inode) in items.iter().zip(inodes) {
                let media_type_str = match item.media_type {
                    MediaType::Image => "image",
                    MediaType::Video => "video",
                    MediaType::Raw => "raw",
                };

                let path_str = item.path.to_string_lossy().to_string();
                if let Some(inode) = inode {
                    reconnect.execute(params![
                        path_str,
                        item.parent_dir,
                        item.file_name,
                        *inode as i64,
                        item.file_size
                    ])?;
                }

                let mut updated = item.clone();
                let row_id: i64 = stmt.query_row(
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
                        updated.is_favorite = row.get::<_, i32>(1)? != 0;
                        updated.is_hidden = row.get::<_, i32>(2)? != 0;
                        updated.is_trashed = row.get::<_, i32>(3)? != 0;
                        updated.title = row.get(4)?;
                        updated.duration_ms = row.get(5)?;
                        row.get(0)
                    },
                )?;

                mark_present.execute(params![row_id, inode.map(|inode| inode as i64)])?;

                updated.id = MediaId(row_id as u64);
                inserted.push(updated);
            }
        }

        Ok(inserted)
    }

    /// Returns `(file_size, mtime_epoch_ms)` keyed by `PathBuf` for fast incremental rescans.
    /// Missing rows are left out, so a file restored at its old path is
    /// ingested again (which marks it present) instead of skipped.
    pub fn get_file_signatures(&self) -> Result<HashMap<PathBuf, (u64, i64)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(&format!(
            "SELECT m.file_path, m.file_size, m.mtime_epoch_ms FROM media_items m WHERE {PRESENT};"
        ))?;
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

    /// Every active (not trashed, not hidden) item with a latitude and a
    /// longitude, in id order.
    pub fn geo_points(&self) -> Result<Vec<GeoPoint>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT id, latitude, longitude FROM media_items \
             WHERE latitude IS NOT NULL AND longitude IS NOT NULL \
             AND is_trashed = 0 AND is_hidden = 0 ORDER BY id;",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(GeoPoint {
                id: MediaId(row.get::<_, i64>(0)? as u64),
                lat: row.get::<_, f64>(1)? as f32,
                lon: row.get::<_, f64>(2)? as f32,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
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

    /// The rows for `ids` in one read per 500 ids (under SQLite's variable
    /// limit), in `ids` order; ids with no row are left out. Trashed and
    /// hidden rows are returned too: callers check the flags.
    pub fn get_items_by_ids(&self, ids: &[MediaId]) -> Result<Vec<MediaItem>> {
        let mut found: HashMap<MediaId, MediaItem> = HashMap::with_capacity(ids.len());
        let conn = self.conn.lock();
        for chunk in ids.chunks(500) {
            let marks = vec!["?"; chunk.len()].join(", ");
            let sql = format!(
                "SELECT id, file_path, parent_dir, file_name, file_size, mtime_epoch_ms, \
                 media_type, width, height, duration_ms, \
                 timestamp_epoch_ms, year, month, day, \
                 latitude, longitude, altitude, \
                 country_code, admin1_region, city, \
                 make, model, lens, focal_length, f_number, iso, exposure_time, \
                 is_favorite, is_hidden, is_trashed, title \
                 FROM media_items WHERE id IN ({marks});"
            );
            let mut stmt = conn.prepare(&sql)?;
            let params = rusqlite::params_from_iter(chunk.iter().map(|id| id.0 as i64));
            found.extend(
                Self::map_query_rows(&mut stmt, params)?
                    .into_iter()
                    .map(|item| (item.id, item)),
            );
        }
        Ok(ids.iter().filter_map(|id| found.remove(id)).collect())
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
        self.set_ratings(&[(id, rating)])
    }

    /// Sets each item's rating (clamped to 0..=5) in one transaction.
    pub fn set_ratings(&self, changes: &[(MediaId, u8)]) -> Result<()> {
        self.update_each(
            "UPDATE media_items SET rating = ?1 WHERE id = ?2;",
            changes.iter().map(|&(id, rating)| (id, rating.min(5))),
        )
    }

    /// Runs `sql` (value ?1, id ?2) for every row in one transaction, so a
    /// batch edit is all-or-nothing and commits once.
    pub(super) fn update_each<T: rusqlite::ToSql>(
        &self,
        sql: &str,
        rows: impl IntoIterator<Item = (MediaId, T)>,
    ) -> Result<()> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(sql)?;
            for (id, value) in rows {
                stmt.execute(params![value, id.0 as i64])?;
            }
        }
        tx.commit()?;
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
        self.set_color_labels(&[(id, label)])
    }

    /// Sets each item's color label (clamped to 0..=6) in one transaction.
    pub fn set_color_labels(&self, changes: &[(MediaId, u8)]) -> Result<()> {
        self.update_each(
            "UPDATE media_items SET color_label = ?1 WHERE id = ?2;",
            changes.iter().map(|&(id, label)| (id, label.min(6))),
        )
    }

    /// One item's color label (0 when none or unknown).
    pub fn get_color_label(&self, id: MediaId) -> Result<u8> {
        let conn = self.conn.lock();
        let label: Option<i32> = conn
            .query_row(
                "SELECT color_label FROM media_items WHERE id = ?1;",
                params![id.0 as i64],
                |row| row.get(0),
            )
            .optional()?;
        Ok(label.unwrap_or(0).clamp(0, 6) as u8)
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

    /// Sets each item's own clockwise quarter turns in one transaction (batch
    /// rotate and its undo). Only the catalog column changes; the original
    /// file is never rewritten. Values normalize to 0..=3, and ids with no
    /// row are skipped without failing the batch.
    pub fn set_rotations(&self, changes: &[(MediaId, u8)]) -> Result<()> {
        self.update_each(
            "UPDATE media_items SET rotation_quarters = ?1 WHERE id = ?2;",
            changes
                .iter()
                .map(|&(id, quarters)| (id, normalize_quarter_turns(quarters as i32) as i32)),
        )
    }

    /// Each id's persisted rotation; ids with no row are left out, so a
    /// caller never records a change for a purged item.
    pub fn get_rotations(&self, ids: &[MediaId]) -> Result<HashMap<MediaId, u8>> {
        let conn = self.conn.lock();
        let mut stmt =
            conn.prepare_cached("SELECT rotation_quarters FROM media_items WHERE id = ?1;")?;
        let mut map = HashMap::with_capacity(ids.len());
        for id in ids {
            let quarters: Option<i32> = stmt
                .query_row(params![id.0 as i64], |row| row.get(0))
                .optional()?;
            if let Some(quarters) = quarters {
                map.insert(*id, normalize_quarter_turns(quarters));
            }
        }
        Ok(map)
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
        self.set_titles(&[(id, title)])
    }

    /// Sets each item's title in one transaction; blank clears it. Titles
    /// are cleaned to one line (see [`crate::text::clean_text`]).
    pub fn set_titles(&self, changes: &[(MediaId, Option<&str>)]) -> Result<()> {
        self.update_each(
            "UPDATE media_items SET title = ?1 WHERE id = ?2;",
            changes.iter().map(|&(id, title)| {
                let title = title.map(|value| clean_text(value, MAX_TEXT_CHARS));
                (id, title.filter(|value| !value.is_empty()))
            }),
        )
    }

    /// Updates the capture timestamp and derived year/month/day, then rebuilds
    /// timeline buckets so year/month scrubbing reflects the edit immediately.
    pub fn set_timestamp(&self, id: MediaId, epoch_ms: i64) -> Result<()> {
        self.set_timestamps(&[(id, epoch_ms)])
    }

    /// [`Self::set_timestamp`] for many items: the dates and one bucket
    /// rebuild commit together (a rebuild per item made large batches take
    /// minutes, and a failed rebuild must not leave the dates half-applied).
    pub fn set_timestamps(&self, changes: &[(MediaId, i64)]) -> Result<()> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(
                "UPDATE media_items
                 SET timestamp_epoch_ms = ?1, year = ?2, month = ?3, day = ?4,
                     date_edited = 1
                 WHERE id = ?5;",
            )?;
            for &(id, epoch_ms) in changes {
                let Some(datetime) = chrono::DateTime::from_timestamp_millis(epoch_ms) else {
                    anyhow::bail!("invalid timestamp: {epoch_ms}");
                };
                stmt.execute(params![
                    epoch_ms,
                    datetime.year(),
                    datetime.month(),
                    datetime.day(),
                    id.0 as i64
                ])?;
            }
        }
        Self::rebuild_timeline_buckets(&tx)?;
        tx.commit()?;
        Ok(())
    }

    pub fn set_location(
        &self,
        id: MediaId,
        latitude: Option<f64>,
        longitude: Option<f64>,
    ) -> Result<()> {
        self.set_locations(&[(id, (latitude, longitude))])
    }

    /// Sets each item's (latitude, longitude) in one transaction.
    pub fn set_locations(&self, changes: &[(MediaId, crate::Coordinates)]) -> Result<()> {
        // The place columns are derived from the coordinates (as at scan
        // time), so they move with them: re-geocoded on edit, cleared on
        // removal. The FTS update trigger reindexes them. Geocoding runs
        // before the catalog lock is taken.
        static GEOCODER: std::sync::OnceLock<crate::OfflineGeocoder> = std::sync::OnceLock::new();
        let places: Vec<_> = changes
            .iter()
            .map(|(_, (lat, lon))| {
                lat.zip(*lon).and_then(|(lat, lon)| {
                    GEOCODER
                        .get_or_init(crate::OfflineGeocoder::new)
                        .reverse_geocode(lat, lon)
                })
            })
            .collect();
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(
                "UPDATE media_items SET latitude = ?1, longitude = ?2, city = ?3, admin1_region = ?4, country_code = ?5, place_edited = 1 WHERE id = ?6;",
            )?;
            for ((id, (latitude, longitude)), place) in changes.iter().zip(&places) {
                stmt.execute(params![
                    latitude,
                    longitude,
                    place.as_ref().map(|p| &p.city),
                    place.as_ref().map(|p| &p.admin1_region),
                    place.as_ref().map(|p| &p.country_code),
                    id.0 as i64
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// `(date_edited, place_edited)` for each id that has a row: whether the
    /// user (or an XMP import) set the date or place, so sidecars carry them.
    pub fn edited_flags(&self, ids: &[MediaId]) -> Result<HashMap<MediaId, (bool, bool)>> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare_cached("SELECT date_edited, place_edited FROM media_items WHERE id = ?1;")?;
        let mut flags = HashMap::with_capacity(ids.len());
        for &id in ids {
            let mut rows = stmt.query(params![id.0 as i64])?;
            if let Some(row) = rows.next()? {
                flags.insert(id, (row.get(0)?, row.get(1)?));
            }
        }
        Ok(flags)
    }

    pub fn set_caption(&self, id: MediaId, caption: &str) -> Result<()> {
        self.set_captions(&[(id, caption)])
    }

    /// Sets each item's caption in one transaction; blank removes it.
    /// Captions are cleaned, keeping line breaks (see [`crate::text::clean_caption`]).
    pub fn set_captions(&self, changes: &[(MediaId, &str)]) -> Result<()> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        for &(id, caption) in changes {
            Self::write_caption(&tx, id, caption)?;
        }
        tx.commit()?;
        Ok(())
    }

    fn write_caption(conn: &rusqlite::Connection, id: MediaId, caption: &str) -> Result<()> {
        let media_id = id.0 as i64;
        let cleaned = clean_caption(caption, MAX_TEXT_CHARS);
        let trimmed = cleaned.as_str();

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

        Self::refresh_fts_caption_row(conn, media_id, previous_caption.as_deref())?;
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

    /// One item's caption, if it has one.
    pub fn get_caption(&self, id: MediaId) -> Result<Option<String>> {
        let conn = self.conn.lock();
        Ok(conn
            .query_row(
                "SELECT caption FROM media_captions WHERE media_id = ?1;",
                params![id.0 as i64],
                |row| row.get(0),
            )
            .optional()?)
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

    /// Records the web address an imported sidecar names (`dc:source`).
    /// This alone never makes an item a download.
    pub fn record_media_source(
        &self,
        media_id: MediaId,
        source_url: &str,
        uploader: Option<&str>,
    ) -> Result<()> {
        Self::record_source_in(&self.conn.lock(), media_id, source_url, uploader, false)
    }

    fn record_source_in(
        conn: &rusqlite::Connection,
        media_id: MediaId,
        source_url: &str,
        uploader: Option<&str>,
        downloaded: bool,
    ) -> Result<()> {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        conn.execute(
            r#"
            INSERT INTO media_sources (media_id, source_url, uploader, imported_epoch_ms, downloaded)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(media_id) DO UPDATE SET
                -- A sidecar imported onto a download keeps it a download,
                -- with its address, uploader and download date.
                source_url = CASE
                    WHEN media_sources.downloaded = 1 AND excluded.downloaded = 0
                    THEN media_sources.source_url
                    ELSE excluded.source_url END,
                uploader = CASE
                    WHEN media_sources.downloaded = 1 AND excluded.downloaded = 0
                    THEN media_sources.uploader
                    ELSE excluded.uploader END,
                imported_epoch_ms = CASE
                    WHEN media_sources.downloaded = 1 AND excluded.downloaded = 0
                    THEN media_sources.imported_epoch_ms
                    ELSE excluded.imported_epoch_ms END,
                downloaded = MAX(COALESCE(media_sources.downloaded, 0), excluded.downloaded);
            "#,
            params![media_id.0 as i64, source_url, uploader, now_ms, downloaded as i64],
        )?;
        Ok(())
    }

    /// Returns `(source_url, uploader, imported_epoch_ms)` for an item if it was imported from the network.
    pub fn get_media_source(
        &self,
        media_id: MediaId,
    ) -> Result<Option<(String, Option<String>, i64)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT source_url, uploader, imported_epoch_ms FROM media_sources WHERE media_id = ?1;",
        )?;
        let mut rows = stmt.query(params![media_id.0 as i64])?;
        if let Some(row) = rows.next()? {
            let url: String = row.get(0)?;
            let uploader: Option<String> = row.get(1)?;
            let epoch_ms: i64 = row.get(2)?;
            Ok(Some((url, uploader, epoch_ms)))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::StructuredQuery;
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

    /// Upserts `item` and returns (the returned item's, the stored row's) duration.
    fn upsert_duration(catalog: &Catalog, item: &MediaItem) -> (Option<u64>, Option<u64>) {
        let returned = catalog
            .insert_or_update_batch(std::slice::from_ref(item))
            .expect("upsert must succeed")
            .remove(0);
        let stored = catalog.get_item_by_id(returned.id).unwrap().unwrap();
        (returned.duration_ms, stored.duration_ms)
    }

    fn sample_video(duration_ms: Option<u64>) -> MediaItem {
        let mut item = sample_item(0);
        item.media_type = MediaType::Video;
        item.duration_ms = duration_ms;
        item
    }

    #[test]
    fn rescan_with_failed_probe_keeps_duration_of_unchanged_file() {
        let catalog = Catalog::open_in_memory().unwrap();
        upsert_duration(&catalog, &sample_video(Some(42_000)));
        for failed in [Some(0), None] {
            assert_eq!(
                upsert_duration(&catalog, &sample_video(failed)),
                (Some(42_000), Some(42_000)),
                "a failed probe ({failed:?}) of an unchanged file must keep the stored duration"
            );
        }
    }

    #[test]
    fn rescan_of_changed_file_takes_new_duration() {
        let catalog = Catalog::open_in_memory().unwrap();
        upsert_duration(&catalog, &sample_video(Some(42_000)));
        let mut changed = sample_video(Some(50_000));
        changed.file_size += 1;
        changed.mtime_epoch_ms += 1_000;
        assert_eq!(
            upsert_duration(&catalog, &changed),
            (Some(50_000), Some(50_000)),
            "a changed file's successful probe must replace the duration"
        );
    }

    #[test]
    fn failed_probe_of_file_with_new_size_or_mtime_takes_zero() {
        for (what, size_delta, mtime_delta) in [("size", 1, 0), ("mtime", 0, 1_000)] {
            let catalog = Catalog::open_in_memory().unwrap();
            upsert_duration(&catalog, &sample_video(Some(42_000)));
            let mut changed = sample_video(Some(0));
            changed.file_size += size_delta;
            changed.mtime_epoch_ms += mtime_delta;
            assert_eq!(
                upsert_duration(&catalog, &changed),
                (Some(0), Some(0)),
                "a file with only a new {what} that no longer probes must not keep the old duration"
            );
        }
    }

    #[test]
    fn recently_added_counts_last_30_days() {
        let catalog = Catalog::open_in_memory().unwrap();
        let stored = catalog
            .insert_or_update_batch(&(0..6).map(sample_item).collect::<Vec<_>>())
            .unwrap();
        let day_ms = 86_400_000_i64;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        let conn = catalog.conn.lock();
        let set = |id: MediaId, added: Option<i64>| {
            conn.execute(
                "UPDATE media_items SET added_epoch_ms = ?1 WHERE id = ?2",
                params![added, id.0],
            )
            .unwrap();
        };
        set(stored[0].id, Some(now - day_ms));
        set(stored[1].id, Some(now - 29 * day_ms));
        set(stored[2].id, Some(now - 31 * day_ms));
        set(stored[3].id, None);
        set(stored[4].id, Some(now - 29 * day_ms));
        set(stored[5].id, Some(now - day_ms));
        conn.execute(
            "UPDATE media_items SET is_trashed = 1 WHERE id = ?1",
            params![stored[1].id.0],
        )
        .unwrap();
        conn.execute(
            "UPDATE media_items SET is_hidden = 1 WHERE id = ?1",
            params![stored[5].id.0],
        )
        .unwrap();
        drop(conn);
        assert_eq!(
            catalog
                .query_system_album_count(crate::SystemAlbum::RecentlyAdded)
                .unwrap(),
            2,
            "items added 1 and 29 days ago count; 31 days, trashed, hidden and undated rows do not"
        );
    }

    #[test]
    fn items_by_ids_keeps_order_skips_unknown_and_spans_chunks() {
        let (dir, db) = temp_catalog("items_by_ids");
        let catalog = Catalog::open(&db).expect("catalog must open");
        let items: Vec<MediaItem> = (0..1_201).map(sample_item).collect();
        let stored = catalog.insert_or_update_batch(&items).unwrap();
        let mut ids: Vec<MediaId> = stored.iter().map(|i| i.id).rev().collect();
        ids.insert(3, MediaId(999_999));
        let got = catalog.get_items_by_ids(&ids).unwrap();
        assert_eq!(
            got.len(),
            1_201,
            "every known id across three chunks, unknown left out"
        );
        let want: Vec<MediaId> = ids.iter().copied().filter(|id| id.0 != 999_999).collect();
        let got_ids: Vec<MediaId> = got.iter().map(|i| i.id).collect();
        assert_eq!(got_ids, want, "rows come back in the order asked");
        assert_eq!(
            got[0].file_name,
            stored.last().unwrap().file_name,
            "full rows are mapped"
        );
        assert!(catalog.get_items_by_ids(&[]).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(dir);
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
            catalog
                .search_structured(&StructuredQuery::parse("Oldtown"), 10, 0)
                .unwrap()
                .is_empty(),
            "stale legacy index entries must not match after the upgrade"
        );
        assert_eq!(
            catalog
                .search_structured(&StructuredQuery::parse("Lisbon"), 10, 0)
                .unwrap()
                .len(),
            1
        );

        drop(catalog);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn fts_repair_migration_runs_once() {
        let (dir, db) = temp_catalog("fts_repair");
        let catalog = Catalog::open(&db).expect("catalog must open");
        let id = catalog.insert_or_update_batch(&[sample_item(1)]).unwrap()[0].id;
        catalog.set_caption(id, "kyoto trip").unwrap();
        drop(catalog);
        // Replay "add tag kyoto, remove it" through the pre-v16 helper SQL,
        // which left the caption column out of the contentless 'delete'.
        let break_index = |version: i64| {
            let conn = rusqlite::Connection::open(&db).unwrap();
            let old_delete = format!(
                "INSERT INTO media_search_fts(media_search_fts, rowid, file_name, parent_dir, city, admin1_region, country_code, make, model, lens, year, tags)
                 SELECT 'delete', m.id, m.file_name, m.parent_dir, m.city, m.admin1_region, m.country_code, m.make, m.model, m.lens, CAST(m.year AS TEXT),
                     COALESCE((SELECT GROUP_CONCAT(tag, ' ') FROM media_tags WHERE media_id = m.id), '')
                 FROM media_items m WHERE m.id = {};",
                id.0
            );
            let old_insert = format!(
                "INSERT INTO media_search_fts(rowid, file_name, parent_dir, city, admin1_region, country_code, make, model, lens, year, tags)
                 SELECT m.id, m.file_name, m.parent_dir, m.city, m.admin1_region, m.country_code, m.make, m.model, m.lens, CAST(m.year AS TEXT),
                     COALESCE((SELECT GROUP_CONCAT(tag, ' ') FROM media_tags WHERE media_id = m.id), '')
                 FROM media_items m WHERE m.id = {};",
                id.0
            );
            conn.execute_batch(&format!(
                "{old_delete} INSERT INTO media_tags (media_id, tag) VALUES ({0}, 'kyoto'); {old_insert}
                 {old_delete} DELETE FROM media_tags WHERE media_id = {0}; {old_insert}
                 PRAGMA user_version = {1};",
                id.0, version
            ))
            .unwrap();
        };
        break_index(15);
        let ids = |catalog: &Catalog| -> Vec<MediaId> {
            catalog
                .search_structured(&StructuredQuery::parse("kyoto"), 10, 0)
                .unwrap()
                .into_iter()
                .map(|item| item.id)
                .collect()
        };

        let catalog = Catalog::open(&db).expect("catalog must reopen and repair");
        assert_eq!(
            ids(&catalog),
            vec![id],
            "the repair must reindex caption words lost by the old tag helpers"
        );
        assert_eq!(user_version(&catalog), SCHEMA_VERSION);
        drop(catalog);

        let catalog = Catalog::open(&db).expect("catalog must reopen after the repair");
        assert_eq!(ids(&catalog), vec![id], "the repaired index must persist");
        assert_eq!(user_version(&catalog), SCHEMA_VERSION);
        assert_eq!(
            catalog
                .get_captions_map()
                .unwrap()
                .get(&id)
                .map(String::as_str),
            Some("kyoto trip"),
            "the repair must keep catalog rows"
        );
        drop(catalog);

        // At the current version the rebuild must not run again: a mismatch
        // injected now survives a reopen, which proves the repair is one-time.
        break_index(SCHEMA_VERSION);
        let catalog = Catalog::open(&db).expect("catalog must reopen at the current version");
        assert!(
            ids(&catalog).is_empty(),
            "the index rebuild must run only below v16, not on every open"
        );
        drop(catalog);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn migration_creates_tag_aliases_idempotently() {
        let (dir, db) = temp_catalog("tag_aliases_migration");
        let catalog = Catalog::open(&db).expect("catalog must open");
        // Simulate a catalog from before aliases existed.
        catalog
            .conn
            .lock()
            .execute_batch("DROP TABLE tag_aliases; PRAGMA user_version = 15;")
            .expect("pre-migration schema must be reproducible");
        drop(catalog);

        let catalog = Catalog::open(&db).expect("catalog must reopen and migrate");
        assert_eq!(user_version(&catalog), SCHEMA_VERSION);
        catalog
            .set_alias("nyc", "places/usa/new york")
            .expect("the migrated table takes aliases");
        drop(catalog);

        let catalog = Catalog::open(&db).expect("catalog must reopen twice");
        assert_eq!(user_version(&catalog), SCHEMA_VERSION);
        assert_eq!(
            catalog.aliases().expect("aliases"),
            vec![("nyc".to_string(), "places/usa/new york".to_string())],
            "reopening keeps alias rows"
        );
        drop(catalog);
        let _ = std::fs::remove_dir_all(&dir);
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
            .set_rotations(&[(id, 1)])
            .expect("rotation write must succeed");
        assert_eq!(catalog.get_rotation(id).expect("rotation read"), 1);

        // Simulate a catalog created before display rotation existed.
        catalog
            .conn
            .lock()
            .execute_batch(
                "DROP INDEX idx_media_rotated;
                 ALTER TABLE media_items DROP COLUMN rotation_quarters;",
            )
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
            .set_rotations(&[(id, 2)])
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
    fn set_rotations_stores_each_value_and_reads_back_known_ids() {
        let (dir, db) = temp_catalog("rotation_per_item");
        let catalog = Catalog::open(&db).expect("catalog must open");
        let inserted = catalog
            .insert_or_update_batch(&[sample_item(1), sample_item(2)])
            .expect("sample items must insert");
        let (a, b) = (inserted[0].id, inserted[1].id);
        let missing = MediaId(9_999);

        catalog
            .set_rotations(&[(a, 1), (b, 6), (missing, 2)])
            .expect("per-item rotations must save, skipping unknown ids");
        let read = catalog
            .get_rotations(&[a, b, missing])
            .expect("rotations must read");
        assert_eq!(read.get(&a), Some(&1), "each item keeps its own value");
        assert_eq!(read.get(&b), Some(&2), "values normalize to 0..=3");
        assert!(
            !read.contains_key(&missing),
            "an unknown id is left out so callers never record it"
        );

        drop(catalog);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn set_rotations_is_transactional_normalized_and_survives_rescan() {
        let (dir, db) = temp_catalog("rotation_set");
        let catalog = Catalog::open(&db).expect("catalog must open");
        let inserted = catalog
            .insert_or_update_batch(&[sample_item(1), sample_item(2)])
            .expect("sample items must insert");
        let ids: Vec<MediaId> = inserted.iter().map(|item| item.id).collect();

        assert!(
            catalog.set_rotations(&[]).is_ok(),
            "an empty id list must be a successful no-op"
        );

        catalog
            .set_rotations(&[(ids[0], 7), (ids[1], 7)])
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
            .set_rotations(&[(ids[0], 2), (MediaId(9_999), 2)])
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
