use anyhow::Result;
use rusqlite::params;
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use super::Catalog;
use crate::duplicates::DuplicateGroup;
use crate::{MediaId, MediaItem};

impl Catalog {
    /// Returns media items that are unhashed but share a `file_size` with at
    /// least one other active item in the library.
    pub fn find_unhashed_duplicate_candidates(&self) -> Result<Vec<(MediaId, PathBuf, u64)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            r#"
            SELECT m.id, m.file_path, m.file_size
            FROM media_items m
            LEFT JOIN media_hashes h ON m.id = h.media_id
            WHERE m.is_trashed = 0 AND m.is_hidden = 0
              AND h.media_id IS NULL
              AND m.file_size IN (
                  SELECT file_size FROM media_items
                  WHERE is_trashed = 0 AND is_hidden = 0
                  GROUP BY file_size
                  HAVING COUNT(*) > 1
              );
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            let id: i64 = row.get(0)?;
            let path_str: String = row.get(1)?;
            let size: u64 = row.get(2)?;
            Ok((MediaId(id as u64), PathBuf::from(path_str), size))
        })?;

        let mut candidates = Vec::new();
        for r in rows {
            candidates.push(r?);
        }
        Ok(candidates)
    }

    /// Stores precomputed content hashes in a single transaction.
    pub fn batch_insert_hashes(&self, entries: &[(MediaId, u64, String)]) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(
                r#"
                INSERT OR REPLACE INTO media_hashes (media_id, file_size, content_hash, computed_epoch_ms)
                VALUES (?1, ?2, ?3, ?4);
                "#,
            )?;
            for (id, size, hash) in entries {
                stmt.execute(params![id.0 as i64, *size as i64, hash, now])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Returns active items that match a specific content hash.
    pub fn find_items_by_hash(&self, content_hash: &str) -> Result<Vec<MediaItem>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            r#"
            SELECT
                m.id, m.file_path, m.parent_dir, m.file_name, m.file_size, m.mtime_epoch_ms,
                m.media_type, m.width, m.height, m.duration_ms,
                m.timestamp_epoch_ms, m.year, m.month, m.day,
                m.latitude, m.longitude, m.altitude,
                m.country_code, m.admin1_region, m.city,
                m.make, m.model, m.lens, m.focal_length, m.f_number, m.iso, m.exposure_time,
                m.is_favorite, m.is_hidden, m.is_trashed, m.title
            FROM media_items m
            JOIN media_hashes h ON m.id = h.media_id
            WHERE h.content_hash = ?1 AND m.is_trashed = 0 AND m.is_hidden = 0
            ORDER BY m.timestamp_epoch_ms ASC, m.id ASC;
            "#,
        )?;
        Self::map_query_rows(&mut stmt, params![content_hash])
    }

    /// Returns active items that match a specific file size.
    pub fn find_candidates_by_size(&self, file_size: u64) -> Result<Vec<(MediaId, PathBuf)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            r#"
            SELECT id, file_path
            FROM media_items
            WHERE file_size = ?1 AND is_trashed = 0 AND is_hidden = 0;
            "#,
        )?;
        let rows = stmt.query_map(params![file_size as i64], |row| {
            let id: i64 = row.get(0)?;
            let path_str: String = row.get(1)?;
            Ok((MediaId(id as u64), PathBuf::from(path_str)))
        })?;
        let mut items = Vec::new();
        for r in rows {
            items.push(r?);
        }
        Ok(items)
    }

    /// Total number of duplicate items (across all groups with >= 2 items).
    pub fn query_duplicate_items_count(&self) -> Result<usize> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            r#"
            SELECT COUNT(*)
            FROM media_hashes h
            JOIN media_items m ON m.id = h.media_id
            WHERE m.is_trashed = 0 AND m.is_hidden = 0
              AND h.content_hash IN (
                  SELECT content_hash FROM media_hashes
                  JOIN media_items ON media_items.id = media_hashes.media_id
                  WHERE media_items.is_trashed = 0 AND media_items.is_hidden = 0
                  GROUP BY content_hash
                  HAVING COUNT(*) > 1
              );
            "#,
        )?;
        let count: i64 = stmt.query_row([], |row| row.get(0))?;
        Ok(count as usize)
    }

    /// Returns a slice of duplicate items ordered by duplicate group hash,
    /// so items belonging to the same set are adjacent.
    pub fn query_duplicate_items_slice(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<MediaItem>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            r#"
            SELECT
                m.id, m.file_path, m.parent_dir, m.file_name, m.file_size, m.mtime_epoch_ms,
                m.media_type, m.width, m.height, m.duration_ms,
                m.timestamp_epoch_ms, m.year, m.month, m.day,
                m.latitude, m.longitude, m.altitude,
                m.country_code, m.admin1_region, m.city,
                m.make, m.model, m.lens, m.focal_length, m.f_number, m.iso, m.exposure_time,
                m.is_favorite, m.is_hidden, m.is_trashed, m.title
            FROM media_items m
            JOIN media_hashes h ON m.id = h.media_id
            WHERE m.is_trashed = 0 AND m.is_hidden = 0
              AND h.content_hash IN (
                  SELECT content_hash FROM media_hashes
                  JOIN media_items ON media_items.id = media_hashes.media_id
                  WHERE media_items.is_trashed = 0 AND media_items.is_hidden = 0
                  GROUP BY content_hash
                  HAVING COUNT(*) > 1
              )
            ORDER BY h.content_hash ASC, m.timestamp_epoch_ms ASC, m.id ASC
            LIMIT ?1 OFFSET ?2;
            "#,
        )?;
        Self::map_query_rows(&mut stmt, params![limit as i64, offset as i64])
    }

    /// Groups duplicate items into [`DuplicateGroup`] structures.
    pub fn query_duplicate_groups(&self) -> Result<Vec<DuplicateGroup>> {
        let conn = self.conn.lock();
        // 1. Get all duplicate hashes and their counts/sizes
        let mut group_stmt = conn.prepare_cached(
            r#"
            SELECT h.content_hash, m.file_size, COUNT(*) as cnt
            FROM media_hashes h
            JOIN media_items m ON m.id = h.media_id
            WHERE m.is_trashed = 0 AND m.is_hidden = 0
            GROUP BY h.content_hash
            HAVING COUNT(*) > 1
            ORDER BY (COUNT(*) - 1) * m.file_size DESC, h.content_hash ASC;
            "#,
        )?;

        let hash_rows = group_stmt.query_map([], |row| {
            let hash: String = row.get(0)?;
            let file_size: u64 = row.get(1)?;
            let count: i64 = row.get(2)?;
            Ok((hash, file_size, count as usize))
        })?;

        let mut hash_meta = Vec::new();
        for r in hash_rows {
            hash_meta.push(r?);
        }
        drop(group_stmt);

        if hash_meta.is_empty() {
            return Ok(Vec::new());
        }

        // 2. Fetch all matching media items in one query
        let mut item_stmt = conn.prepare_cached(
            r#"
            SELECT
                m.id, m.file_path, m.parent_dir, m.file_name, m.file_size, m.mtime_epoch_ms,
                m.media_type, m.width, m.height, m.duration_ms,
                m.timestamp_epoch_ms, m.year, m.month, m.day,
                m.latitude, m.longitude, m.altitude,
                m.country_code, m.admin1_region, m.city,
                m.make, m.model, m.lens, m.focal_length, m.f_number, m.iso, m.exposure_time,
                m.is_favorite, m.is_hidden, m.is_trashed, m.title,
                h.content_hash
            FROM media_items m
            JOIN media_hashes h ON m.id = h.media_id
            WHERE m.is_trashed = 0 AND m.is_hidden = 0
              AND h.content_hash IN (
                  SELECT content_hash FROM media_hashes
                  JOIN media_items ON media_items.id = media_hashes.media_id
                  WHERE media_items.is_trashed = 0 AND media_items.is_hidden = 0
                  GROUP BY content_hash
                  HAVING COUNT(*) > 1
              )
            ORDER BY h.content_hash ASC, m.rating DESC, m.is_favorite DESC, m.timestamp_epoch_ms ASC, m.id ASC;
            "#,
        )?;

        let mut hash_to_items: std::collections::HashMap<String, Vec<MediaItem>> =
            std::collections::HashMap::new();

        let rows = item_stmt.query_map([], |row| {
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
            let hash: String = row.get(31)?;

            let media_type = match media_type_str.as_str() {
                "video" => crate::MediaType::Video,
                "raw" => crate::MediaType::Raw,
                _ => crate::MediaType::Image,
            };

            let item = MediaItem {
                id: MediaId(id as u64),
                path: PathBuf::from(file_path),
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
            };
            Ok((hash, item))
        })?;

        for r in rows {
            let (hash, item) = r?;
            hash_to_items.entry(hash).or_default().push(item);
        }

        let mut groups = Vec::with_capacity(hash_meta.len());
        for (hash, file_size, _) in hash_meta {
            if let Some(items) = hash_to_items.remove(&hash) {
                if items.len() >= 2 {
                    let potential_reclaimable_bytes = (items.len() - 1) as u64 * file_size;
                    groups.push(DuplicateGroup {
                        hash,
                        file_size,
                        items,
                        potential_reclaimable_bytes,
                    });
                }
            }
        }

        Ok(groups)
    }

    /// Merges duplicates into a keeper item:
    /// - Collects tags from `trash_ids` and copies missing tags to `keep_id`.
    /// - Sets `is_favorite = true` on `keep_id` if any trash item was favorite.
    /// - Updates title if `keep_id` has none and a trash item does.
    /// - Upgrades rating if any trash item has a higher rating.
    /// - Soft-deletes `trash_ids` (`batch_set_trashed`).
    pub fn merge_duplicate_group(&self, keep_id: MediaId, trash_ids: &[MediaId]) -> Result<()> {
        if trash_ids.is_empty() {
            return Ok(());
        }

        // 1. Gather tags from trash_ids
        let mut tags_to_add: HashSet<String> = HashSet::new();
        for id in trash_ids {
            let tags = self.get_tags_for_item(*id)?;
            for tag in tags {
                tags_to_add.insert(tag);
            }
        }

        // Add missing tags to keep_id
        if !tags_to_add.is_empty() {
            let existing_tags: HashSet<String> =
                self.get_tags_for_item(keep_id)?.into_iter().collect();
            let new_tags: Vec<String> = tags_to_add
                .into_iter()
                .filter(|t| !existing_tags.contains(t))
                .collect();
            if !new_tags.is_empty() {
                self.add_tags_batch(&[keep_id], &new_tags)?;
            }
        }

        // 2. Fetch metadata of keep_id and trash_ids to merge favorites/ratings/titles
        let keep_item = self.get_item_by_id(keep_id)?;
        if let Some(keeper) = keep_item {
            let mut updated_rating = self.get_rating(keep_id)?;
            let mut updated_favorite = keeper.is_favorite;
            let mut updated_title = keeper.title.clone();

            for trash_id in trash_ids {
                if let Some(item) = self.get_item_by_id(*trash_id)? {
                    if item.is_favorite {
                        updated_favorite = true;
                    }
                    let rating = self.get_rating(*trash_id)?;
                    if rating > updated_rating {
                        updated_rating = rating;
                    }
                    if updated_title.is_none() && item.title.is_some() {
                        updated_title = item.title;
                    }
                }
            }

            if updated_favorite != keeper.is_favorite {
                self.batch_set_favorite(&[keep_id], updated_favorite)?;
            }
            if updated_rating > 0 {
                self.set_rating(keep_id, updated_rating)?;
            }
            if updated_title != keeper.title {
                self.set_title(keep_id, updated_title.as_deref())?;
            }
        }

        // 3. Move trash_ids to trash
        self.batch_set_trashed(trash_ids, true)?;
        Ok(())
    }
}
