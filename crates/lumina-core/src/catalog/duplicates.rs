use anyhow::Result;
use rusqlite::params;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use super::merge_undo::{DuplicateMerge, KeeperSnapshot};
use super::Catalog;
use crate::duplicates::DuplicateGroup;
use crate::now_ms;
use crate::{MediaId, MediaItem};

// Every query here counts only items whose file is present: a missing file
// cannot be hashed, and merging one would trash metadata the user may want to
// reconnect with Locate.
impl Catalog {
    /// Returns media items that are unhashed but share a `file_size` with at
    /// least one other active item in the library (excluding 0-byte files).
    pub fn find_unhashed_duplicate_candidates(&self) -> Result<Vec<(MediaId, PathBuf, u64)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            r#"
            SELECT m.id, m.file_path, m.file_size
            FROM media_items m
            LEFT JOIN media_hashes h ON m.id = h.media_id
            WHERE m.is_trashed = 0 AND m.is_hidden = 0 AND NOT EXISTS (SELECT 1 FROM media_file_state s WHERE s.media_id = m.id AND s.missing_since_epoch_ms IS NOT NULL) AND m.file_size > 0
              AND h.media_id IS NULL
              AND m.file_size IN (
                  SELECT file_size FROM media_items
                  WHERE is_trashed = 0 AND is_hidden = 0 AND NOT EXISTS (SELECT 1 FROM media_file_state s WHERE s.media_id = media_items.id AND s.missing_since_epoch_ms IS NOT NULL) AND file_size > 0
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

        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Stores precomputed content hashes in a single transaction.
    /// Uses WHERE EXISTS to prevent foreign key errors if candidate items were
    /// deleted by external watcher/scan events before insertion.
    pub fn batch_insert_hashes(&self, entries: &[(MediaId, u64, String)]) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let now = now_ms();

        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(
                r#"
                INSERT OR REPLACE INTO media_hashes (media_id, file_size, content_hash, computed_epoch_ms)
                SELECT ?1, ?2, ?3, ?4
                WHERE EXISTS (SELECT 1 FROM media_items WHERE id = ?1);
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
            WHERE h.content_hash = ?1 AND m.is_trashed = 0 AND m.is_hidden = 0 AND NOT EXISTS (SELECT 1 FROM media_file_state s WHERE s.media_id = m.id AND s.missing_since_epoch_ms IS NOT NULL)
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
            WHERE file_size = ?1 AND is_trashed = 0 AND is_hidden = 0 AND NOT EXISTS (SELECT 1 FROM media_file_state s WHERE s.media_id = media_items.id AND s.missing_since_epoch_ms IS NOT NULL) AND file_size > 0;
            "#,
        )?;
        let rows = stmt.query_map(params![file_size as i64], |row| {
            let id: i64 = row.get(0)?;
            let path_str: String = row.get(1)?;
            Ok((MediaId(id as u64), PathBuf::from(path_str)))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Total number of duplicate items (across all groups with >= 2 items).
    pub fn query_duplicate_items_count(&self) -> Result<usize> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            r#"
            SELECT COUNT(*)
            FROM media_hashes h
            JOIN media_items m ON m.id = h.media_id
            WHERE m.is_trashed = 0 AND m.is_hidden = 0 AND NOT EXISTS (SELECT 1 FROM media_file_state s WHERE s.media_id = m.id AND s.missing_since_epoch_ms IS NOT NULL) AND m.file_size > 0
              AND h.content_hash IN (
                  SELECT content_hash FROM media_hashes
                  JOIN media_items ON media_items.id = media_hashes.media_id
                  WHERE media_items.is_trashed = 0 AND media_items.is_hidden = 0 AND NOT EXISTS (SELECT 1 FROM media_file_state s WHERE s.media_id = media_items.id AND s.missing_since_epoch_ms IS NOT NULL) AND media_items.file_size > 0
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
            WHERE m.is_trashed = 0 AND m.is_hidden = 0 AND NOT EXISTS (SELECT 1 FROM media_file_state s WHERE s.media_id = m.id AND s.missing_since_epoch_ms IS NOT NULL) AND m.file_size > 0
              AND h.content_hash IN (
                  SELECT content_hash FROM media_hashes
                  JOIN media_items ON media_items.id = media_hashes.media_id
                  WHERE media_items.is_trashed = 0 AND media_items.is_hidden = 0 AND NOT EXISTS (SELECT 1 FROM media_file_state s WHERE s.media_id = media_items.id AND s.missing_since_epoch_ms IS NOT NULL) AND media_items.file_size > 0
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
        Self::duplicate_groups_in(&self.conn.lock())
    }

    fn duplicate_groups_in(conn: &rusqlite::Connection) -> Result<Vec<DuplicateGroup>> {
        // 1. Get all duplicate hashes and their counts/sizes
        let mut group_stmt = conn.prepare_cached(
            r#"
            SELECT h.content_hash, m.file_size, COUNT(*) as cnt
            FROM media_hashes h
            JOIN media_items m ON m.id = h.media_id
            WHERE m.is_trashed = 0 AND m.is_hidden = 0 AND NOT EXISTS (SELECT 1 FROM media_file_state s WHERE s.media_id = m.id AND s.missing_since_epoch_ms IS NOT NULL) AND m.file_size > 0
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

        let hash_meta = hash_rows.collect::<rusqlite::Result<Vec<_>>>()?;
        drop(group_stmt);

        if hash_meta.is_empty() {
            return Ok(Vec::new());
        }

        // 2. Fetch all matching media items in one query using the shared row mapper
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
            WHERE m.is_trashed = 0 AND m.is_hidden = 0 AND NOT EXISTS (SELECT 1 FROM media_file_state s WHERE s.media_id = m.id AND s.missing_since_epoch_ms IS NOT NULL) AND m.file_size > 0
              AND h.content_hash IN (
                  SELECT content_hash FROM media_hashes
                  JOIN media_items ON media_items.id = media_hashes.media_id
                  WHERE media_items.is_trashed = 0 AND media_items.is_hidden = 0 AND NOT EXISTS (SELECT 1 FROM media_file_state s WHERE s.media_id = media_items.id AND s.missing_since_epoch_ms IS NOT NULL) AND media_items.file_size > 0
                  GROUP BY content_hash
                  HAVING COUNT(*) > 1
              )
            ORDER BY h.content_hash ASC, m.rating DESC, m.is_favorite DESC, m.timestamp_epoch_ms ASC, m.id ASC;
            "#,
        )?;

        let mut hash_to_items: std::collections::HashMap<String, Vec<MediaItem>> =
            std::collections::HashMap::new();

        let rows = item_stmt.query_map([], |row| {
            let hash: String = row.get(31)?;
            let item = Self::map_media_item_row(row)?;
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

    /// Merges one non-empty group within an active SQLite transaction and
    /// returns the keeper as it was before.
    pub(super) fn merge_duplicate_group_in_tx(
        tx: &rusqlite::Transaction,
        keep_id: MediaId,
        trash_ids: &[MediaId],
    ) -> Result<KeeperSnapshot> {
        let snapshot = KeeperSnapshot::take(tx, keep_id, trash_ids)?;
        Self::merge_metadata_in_tx(tx, keep_id, trash_ids)?;

        let mut trash_stmt =
            tx.prepare_cached("UPDATE media_items SET is_trashed = 1 WHERE id = ?1;")?;
        for trash_id in trash_ids {
            trash_stmt.execute(params![trash_id.0 as i64])?;
        }
        Ok(snapshot)
    }

    /// Copies the user metadata of `trash_ids` onto `keep_id`: tags (union),
    /// favorite (any), rating (highest), flag (highest: pick beats unflagged
    /// beats reject), album membership (union), and caption,
    /// colour label, title, rights and the edits row where the keeper has none.
    pub(crate) fn merge_metadata_in_tx(
        tx: &rusqlite::Transaction,
        keep_id: MediaId,
        trash_ids: &[MediaId],
    ) -> Result<()> {
        // 1. Merge favorite, rating, color label, title and flag onto keep_id
        {
            let mut read_stmt = tx.prepare_cached(
                "SELECT is_favorite, rating, color_label, title, flag FROM media_items WHERE id = ?1;",
            )?;
            let keeper_meta: Option<(i32, u8, u8, Option<String>, i64)> = read_stmt
                .query_row(params![keep_id.0 as i64], |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                })
                .ok();

            if let Some((init_fav, init_rating, init_color, init_title, init_flag)) = keeper_meta {
                let mut updated_fav = init_fav != 0;
                let mut updated_rating = init_rating;
                let mut updated_color = init_color;
                let mut updated_title = init_title;
                let mut updated_flag = init_flag;

                for trash_id in trash_ids {
                    if let Ok((fav, rating, color, title, flag)) =
                        read_stmt.query_row(params![trash_id.0 as i64], |row| {
                            Ok((
                                row.get::<_, i32>(0)?,
                                row.get::<_, u8>(1)?,
                                row.get::<_, u8>(2)?,
                                row.get::<_, Option<String>>(3)?,
                                row.get::<_, i64>(4)?,
                            ))
                        })
                    {
                        if fav != 0 {
                            updated_fav = true;
                        }
                        if rating > updated_rating {
                            updated_rating = rating;
                        }
                        if updated_color == 0 && color != 0 {
                            updated_color = color;
                        }
                        if updated_title.is_none() && title.is_some() {
                            updated_title = title;
                        }
                        updated_flag = updated_flag.max(flag);
                    }
                }

                let mut update_stmt = tx.prepare_cached(
                    r#"
                    UPDATE media_items
                    SET is_favorite = ?1, rating = ?2, color_label = ?3, title = ?4, flag = ?5
                    WHERE id = ?6;
                    "#,
                )?;
                update_stmt.execute(params![
                    updated_fav as i32,
                    updated_rating,
                    updated_color,
                    updated_title.as_deref(),
                    updated_flag,
                    keep_id.0 as i64
                ])?;
            }
        }

        // 2. Edits row (crop, exposure, auto-enhance) when keep_id has none.
        // The crop is in unrotated coordinates; the copied row brings its
        // rotation along when the keeper is unrotated, so the user sees the
        // same edit (the first duplicate with a row wins).
        {
            let mut rotate_stmt = tx.prepare_cached(
                r#"
                UPDATE media_items SET rotation_quarters =
                    (SELECT rotation_quarters FROM media_items WHERE id = ?2)
                WHERE id = ?1 AND rotation_quarters % 4 = 0;
                "#,
            )?;
            let mut edits_stmt = tx.prepare_cached(
                r#"
                INSERT OR IGNORE INTO media_edits
                    (media_id, crop_x, crop_y, crop_w, crop_h, exposure_ev, auto_enhance,
                     updated_epoch_ms)
                SELECT ?1, crop_x, crop_y, crop_w, crop_h, exposure_ev, auto_enhance,
                     updated_epoch_ms
                FROM media_edits WHERE media_id = ?2;
                "#,
            )?;
            for trash_id in trash_ids {
                let ids = params![keep_id.0 as i64, trash_id.0 as i64];
                if edits_stmt.execute(ids)? > 0 {
                    rotate_stmt.execute(ids)?;
                }
            }
        }

        // The search index is contentless, so its 'delete' must see the values
        // that were indexed. The UPDATEs above fire `media_fts_au`, which reads
        // tags and caption from the tables: they run before the copies, so
        // they saw the indexed ones. The copies below change only those tables,
        // so the keeper's row is removed before them and rebuilt after. No
        // media_items UPDATE may run between the two, or `media_fts_au` would
        // delete a row that is not indexed and corrupt the index.
        Self::delete_fts_for_items(tx, &[keep_id.0 as i64])?;
        // 3. Copy missing tags from trash_ids to keep_id
        {
            let mut tag_stmt = tx.prepare_cached(
                r#"
                INSERT OR IGNORE INTO media_tags (media_id, tag)
                SELECT ?1, tag FROM media_tags WHERE media_id = ?2;
                "#,
            )?;
            for trash_id in trash_ids {
                tag_stmt.execute(params![keep_id.0 as i64, trash_id.0 as i64])?;
            }
        }

        // 4. Copy missing caption from trash_ids to keep_id
        {
            let mut caption_stmt = tx.prepare_cached(
                r#"
                INSERT OR IGNORE INTO media_captions (media_id, caption)
                SELECT ?1, caption FROM media_captions WHERE media_id = ?2;
                "#,
            )?;
            for trash_id in trash_ids {
                caption_stmt.execute(params![keep_id.0 as i64, trash_id.0 as i64])?;
            }
        }

        // 5. Rights when keep_id has none (not indexed, so FTS order does not
        // matter here). The first trash row that has any wins.
        {
            let mut rights_stmt = tx.prepare_cached(
                r#"
                INSERT OR IGNORE INTO media_rights
                    (media_id, creator, copyright, usage_terms, contact_email, contact_url)
                SELECT ?1, creator, copyright, usage_terms, contact_email, contact_url
                FROM media_rights WHERE media_id = ?2;
                "#,
            )?;
            for trash_id in trash_ids {
                rights_stmt.execute(params![keep_id.0 as i64, trash_id.0 as i64])?;
            }
        }

        Self::insert_fts_for_items(tx, &[keep_id.0 as i64])?;

        // 6. Album membership: keep_id joins every album a trash_id is in.
        // Not indexed and no media_items row changes, so it sits outside the
        // FTS window. Must run before `fold_into` deletes `other`, whose rows
        // the cascade would drop; a merged duplicate is only trashed, so it
        // keeps its own row too.
        {
            let mut album_stmt = tx.prepare_cached(
                r#"
                INSERT OR IGNORE INTO album_items (album_id, media_id)
                SELECT album_id, ?1 FROM album_items WHERE media_id = ?2;
                "#,
            )?;
            for trash_id in trash_ids {
                album_stmt.execute(params![keep_id.0 as i64, trash_id.0 as i64])?;
            }
        }

        // 7. Key photos: an album whose cover was a merged copy now shows the
        // keeper, which step 6 just made a member. Before `fold_into` too,
        // whose delete would otherwise set the cover to NULL.
        {
            let mut cover_stmt = tx.prepare_cached(
                "UPDATE albums SET cover_media_id = ?1 WHERE cover_media_id = ?2;",
            )?;
            for trash_id in trash_ids {
                cover_stmt.execute(params![keep_id.0 as i64, trash_id.0 as i64])?;
            }
        }
        Ok(())
    }

    /// Merges duplicates into a keeper item within a single atomic SQLite
    /// transaction. Returns the keeper's metadata from before the merge, for
    /// undo ([`Catalog::undo_duplicate_merge`]); `None` when there was
    /// nothing to merge.
    pub fn merge_duplicate_group(
        &self,
        keep_id: MediaId,
        trash_ids: &[MediaId],
    ) -> Result<Option<KeeperSnapshot>> {
        if trash_ids.is_empty() {
            return Ok(None);
        }

        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let snapshot = Self::merge_duplicate_group_in_tx(&tx, keep_id, trash_ids)?;
        Self::rebuild_timeline_buckets(&tx)?;
        tx.commit()?;
        Ok(Some(snapshot))
    }

    /// Merges all duplicate groups in a single atomic transaction.
    /// `keepers` maps a content hash to the copy that group keeps; a group
    /// with no entry, or whose entry is not one of its items, keeps its
    /// heuristic pick (the first item). Groups whose hash is in `skipped`
    /// are left alone.
    /// Returns each keeper's metadata from before the merge and the copies
    /// it trashed, for undo. The groups are read inside the transaction, so
    /// a merge or undo that landed after the caller looked is seen.
    pub fn merge_all_duplicates(
        &self,
        keepers: &HashMap<String, MediaId>,
        skipped: &HashSet<String>,
    ) -> Result<DuplicateMerge> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let groups = Self::duplicate_groups_in(&tx)?;
        let mut merge = DuplicateMerge::default();
        for group in groups {
            if group.items.len() >= 2 && !skipped.contains(&group.hash) {
                let keep_id = keepers
                    .get(&group.hash)
                    .copied()
                    .filter(|id| group.items.iter().any(|i| i.id == *id))
                    .unwrap_or(group.items[0].id);
                let trash_ids: Vec<MediaId> = group
                    .items
                    .iter()
                    .map(|i| i.id)
                    .filter(|id| *id != keep_id)
                    .collect();
                merge
                    .keepers
                    .push(Self::merge_duplicate_group_in_tx(&tx, keep_id, &trash_ids)?);
            }
        }
        if !merge.keepers.is_empty() {
            Self::rebuild_timeline_buckets(&tx)?;
        }
        tx.commit()?;
        Ok(merge)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::tests_support::{album_members, video_item};
    use crate::catalog::{CropRect, Edits};
    use crate::search::StructuredQuery;
    use crate::Flag;

    fn plain(catalog: &Catalog, path: &str) -> MediaId {
        catalog.insert_or_update_batch(&[video_item(path)]).unwrap()[0].id
    }

    #[test]
    fn duplicate_merge_with_edits_and_tags_keeps_the_search_index() {
        let catalog = Catalog::open_in_memory().unwrap();
        let dup = plain(&catalog, "/lib/dup.mp4");
        catalog.add_tags_batch(&[dup], &["beach".into()]).unwrap();
        catalog.set_caption(dup, "sunset").unwrap();
        let edits = Edits {
            rotation: 1,
            crop: Some(CropRect {
                x: 0.25,
                y: 0.125,
                w: 0.5,
                h: 0.75,
            }),
            exposure_ev: -0.5,
            auto_enhance: true,
        };
        catalog.set_edits(&[(dup, edits)]).unwrap();
        // Catalogued after the duplicate's tags, with words of its own.
        let keep = plain(&catalog, "/lib/keep (edited).mp4");
        catalog
            .merge_duplicate_group(keep, &[dup])
            .expect("merging an edited, tagged duplicate must not corrupt the search index");
        let beach = |catalog: &Catalog| -> Vec<MediaId> {
            let found = catalog
                .search_structured(&StructuredQuery::parse("beach"), 10, 0)
                .unwrap();
            found.iter().map(|item| item.id).collect()
        };
        assert_eq!(beach(&catalog), vec![keep], "the merged tag is searchable");
        catalog
            .remove_tags_batch(&[keep], &["beach".into()])
            .unwrap();
        assert!(
            beach(&catalog).is_empty(),
            "the keeper is indexed once, so removing the tag removes the match"
        );
    }

    #[test]
    fn merge_all_keeps_the_chosen_keeper() {
        let catalog = Catalog::open_in_memory().unwrap();
        let a1 = plain(&catalog, "/lib/a1.mp4");
        let a2 = plain(&catalog, "/lib/a2.mp4");
        let b1 = plain(&catalog, "/lib/b1.mp4");
        let b2 = plain(&catalog, "/lib/b2.mp4");
        catalog
            .batch_insert_hashes(&[
                (a1, 1_000, "a".into()),
                (a2, 1_000, "a".into()),
                (b1, 1_000, "b".into()),
                (b2, 1_000, "b".into()),
            ])
            .unwrap();
        let heuristic: Vec<MediaId> = catalog
            .query_duplicate_groups()
            .unwrap()
            .iter()
            .map(|g| g.items[0].id)
            .collect();
        assert_eq!(
            heuristic,
            vec![a1, b1],
            "the heuristic keeps the first catalogued"
        );

        // Group a keeps its second copy; b's stale choice (not in b) is ignored.
        let keepers = HashMap::from([("a".to_string(), a2), ("b".to_string(), a1)]);
        let mut trashed = catalog
            .merge_all_duplicates(&keepers, &HashSet::new())
            .unwrap()
            .trashed();
        trashed.sort_by_key(|id| id.0);
        assert_eq!(
            trashed,
            vec![a1, b2],
            "the chosen keeper stays; the others go"
        );
        assert_eq!(
            catalog
                .find_items_by_hash("a")
                .unwrap()
                .iter()
                .map(|i| i.id)
                .collect::<Vec<_>>(),
            vec![a2],
            "group a keeps the copy the user chose"
        );
        assert_eq!(
            catalog
                .find_items_by_hash("b")
                .unwrap()
                .iter()
                .map(|i| i.id)
                .collect::<Vec<_>>(),
            vec![b1],
            "an id from another group falls back to the heuristic keeper"
        );
    }

    #[test]
    fn merge_all_leaves_skipped_groups() {
        let catalog = Catalog::open_in_memory().unwrap();
        let a1 = plain(&catalog, "/lib/a1.mp4");
        let a2 = plain(&catalog, "/lib/a2.mp4");
        let b1 = plain(&catalog, "/lib/b1.mp4");
        let b2 = plain(&catalog, "/lib/b2.mp4");
        catalog
            .batch_insert_hashes(&[
                (a1, 1_000, "a".into()),
                (a2, 1_000, "a".into()),
                (b1, 1_000, "b".into()),
                (b2, 1_000, "b".into()),
            ])
            .unwrap();
        let skipped = HashSet::from(["a".to_string()]);
        let trashed = catalog
            .merge_all_duplicates(&HashMap::new(), &skipped)
            .unwrap()
            .trashed();
        assert_eq!(trashed, vec![b2], "only the group not skipped merges");
        assert_eq!(
            catalog.find_items_by_hash("a").unwrap().len(),
            2,
            "a skipped group keeps every copy"
        );
    }

    #[test]
    fn merge_keeps_pick_over_reject() {
        let catalog = Catalog::open_in_memory().unwrap();
        let keep = plain(&catalog, "/lib/keep.mp4");
        let reject = plain(&catalog, "/lib/reject.mp4");
        let pick = plain(&catalog, "/lib/pick.mp4");
        catalog
            .set_flags(&[
                (keep, Flag::Reject),
                (reject, Flag::Reject),
                (pick, Flag::Pick),
            ])
            .unwrap();

        catalog
            .merge_duplicate_group(keep, &[reject, pick])
            .unwrap();

        assert_eq!(
            catalog.get_flags_map().unwrap().get(&keep),
            Some(&Flag::Pick),
            "the keeper takes the highest flag: pick beats unflagged beats reject"
        );
    }

    #[test]
    fn merge_does_not_lower_the_keepers_flag() {
        let catalog = Catalog::open_in_memory().unwrap();
        let keep = plain(&catalog, "/lib/keep.mp4");
        let dup = plain(&catalog, "/lib/dup.mp4");
        catalog.set_flags(&[(dup, Flag::Reject)]).unwrap();

        catalog.merge_duplicate_group(keep, &[dup]).unwrap();

        assert_eq!(
            catalog.get_flags_map().unwrap().get(&keep),
            None,
            "an unflagged keeper stays unflagged when the copy was rejected"
        );
    }

    #[test]
    fn merge_duplicates_moves_key_photo_to_keeper() {
        let catalog = Catalog::open_in_memory().unwrap();
        let keep = plain(&catalog, "/lib/keep.mp4");
        let dup = plain(&catalog, "/lib/dup.mp4");
        let other = plain(&catalog, "/lib/other.mp4");
        let album = catalog.create_album_with("Picks", &[dup, other]).unwrap();
        catalog.set_album_cover(album.id, Some(dup)).unwrap();

        catalog.merge_duplicate_group(keep, &[dup]).unwrap();

        assert_eq!(
            catalog.list_albums_with_counts().unwrap()[0]
                .0
                .cover_media_id,
            Some(keep),
            "a merged copy's key photo moves to the keeper"
        );
        assert_eq!(
            catalog.album_cards().unwrap()[0].cover,
            Some(keep),
            "the album shows the keeper, not the newest member"
        );
    }

    #[test]
    fn merge_duplicates_carries_album_membership() {
        let catalog = Catalog::open_in_memory().unwrap();
        let keep = plain(&catalog, "/lib/keep.mp4");
        let dup = plain(&catalog, "/lib/dup.mp4");
        catalog.add_tags_batch(&[dup], &["beach".into()]).unwrap();
        let album = catalog.create_album("Picks").unwrap();
        catalog.add_to_album(album.id, &[dup]).unwrap();

        catalog.merge_duplicate_group(keep, &[dup]).unwrap();

        assert_eq!(
            album_members(&catalog, album.id),
            vec![keep, dup],
            "the keeper joins the copy's album; the trashed copy keeps its own row"
        );
        assert_eq!(
            catalog.query_album_count(album.id).unwrap(),
            1,
            "only the keeper is active"
        );
        let found: Vec<MediaId> = catalog
            .search_structured(&StructuredQuery::parse("beach"), 10, 0)
            .unwrap()
            .iter()
            .map(|item| item.id)
            .collect();
        assert_eq!(found, vec![keep], "the search index stays consistent");
    }
}
