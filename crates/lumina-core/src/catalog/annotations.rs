//! Metadata annotations: favorite, rating, colour label, rotation, title, timestamp
//! and location, split from the row store in `catalog::media`.

use anyhow::Result;
use rusqlite::{params, OptionalExtension};
use std::collections::HashMap;

use super::Catalog;
use crate::text::{clean_text, MAX_TEXT_CHARS};
use crate::{normalize_quarter_turns, MediaId};

impl Catalog {
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
        let mut map = HashMap::with_capacity(ids.len());
        // One read per 500 ids instead of a round-trip per id.
        for chunk in ids.chunks(500) {
            let marks = vec!["?"; chunk.len()].join(", ");
            let sql =
                format!("SELECT id, rotation_quarters FROM media_items WHERE id IN ({marks});");
            let mut stmt = conn.prepare_cached(&sql)?;
            let rows = stmt.query_map(
                rusqlite::params_from_iter(chunk.iter().map(|id| id.0 as i64)),
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i32>(1)?)),
            )?;
            for row in rows {
                let (id, quarters) = row?;
                map.insert(MediaId(id as u64), normalize_quarter_turns(quarters));
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
                if chrono::DateTime::from_timestamp_millis(epoch_ms).is_none() {
                    anyhow::bail!("invalid timestamp: {epoch_ms}");
                }
                // Capture time 0 is undated (year 0), as the scanner stores it.
                let (year, month, day) = crate::scanner::epoch_to_date_components(epoch_ms);
                stmt.execute(params![epoch_ms, year, month, day, id.0 as i64])?;
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
        let geocoder = crate::OfflineGeocoder::shared();
        let places: Vec<_> = changes
            .iter()
            .map(|(_, (lat, lon))| {
                lat.zip(*lon)
                    .and_then(|(lat, lon)| geocoder.reverse_geocode(lat, lon))
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
        let mut flags = HashMap::with_capacity(ids.len());
        for chunk in ids.chunks(500) {
            let marks = vec!["?"; chunk.len()].join(", ");
            let sql = format!(
                "SELECT id, date_edited, place_edited FROM media_items WHERE id IN ({marks});"
            );
            let mut stmt = conn.prepare_cached(&sql)?;
            let rows = stmt.query_map(
                rusqlite::params_from_iter(chunk.iter().map(|id| id.0 as i64)),
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, bool>(1)?,
                        row.get::<_, bool>(2)?,
                    ))
                },
            )?;
            for row in rows {
                let (id, date_edited, place_edited) = row?;
                flags.insert(MediaId(id as u64), (date_edited, place_edited));
            }
        }
        Ok(flags)
    }
}
