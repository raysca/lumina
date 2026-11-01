use anyhow::Result;
use rusqlite::{params, Transaction};

use super::Catalog;
use crate::text::{clean_text, MAX_NAME_CHARS};
use crate::{MediaId, MediaItem, SortSpec};

impl Catalog {
    /// Deletes existing FTS5 index rows for a set of media items before tag mutations.
    fn delete_fts_for_items(tx: &Transaction, ids: &[i64]) -> Result<()> {
        for chunk in ids.chunks(500) {
            let placeholders: Vec<String> = (1..=chunk.len()).map(|i| format!("?{}", i)).collect();
            let in_clause = placeholders.join(", ");
            let del_sql = format!(
                r#"
                INSERT INTO media_search_fts(
                    media_search_fts, rowid, file_name, parent_dir, city, admin1_region, country_code, make, model, lens, year, tags
                )
                SELECT
                    'delete', m.id, m.file_name, m.parent_dir, m.city, m.admin1_region, m.country_code, m.make, m.model, m.lens, CAST(m.year AS TEXT),
                    COALESCE((SELECT GROUP_CONCAT(tag, ' ') FROM media_tags WHERE media_id = m.id), '')
                FROM media_items m
                WHERE m.id IN ({});
                "#,
                in_clause
            );
            tx.execute(&del_sql, rusqlite::params_from_iter(chunk.iter()))?;
        }
        Ok(())
    }

    /// Re-inserts updated FTS5 index rows for a set of media items after tag mutations.
    fn insert_fts_for_items(tx: &Transaction, ids: &[i64]) -> Result<()> {
        for chunk in ids.chunks(500) {
            let placeholders: Vec<String> = (1..=chunk.len()).map(|i| format!("?{}", i)).collect();
            let in_clause = placeholders.join(", ");
            let ins_sql = format!(
                r#"
                INSERT INTO media_search_fts(
                    rowid, file_name, parent_dir, city, admin1_region, country_code, make, model, lens, year, tags
                )
                SELECT
                    m.id, m.file_name, m.parent_dir, m.city, m.admin1_region, m.country_code, m.make, m.model, m.lens, CAST(m.year AS TEXT),
                    COALESCE((SELECT GROUP_CONCAT(tag, ' ') FROM media_tags WHERE media_id = m.id), '')
                FROM media_items m
                WHERE m.id IN ({});
                "#,
                in_clause
            );
            tx.execute(&ins_sql, rusqlite::params_from_iter(chunk.iter()))?;
        }
        Ok(())
    }

    /// Attaches one or more normalized tags to a batch of media items. Tags
    /// are cleaned to one line of at most [`MAX_NAME_CHARS`] and lowercased.
    pub fn add_tags_batch(&self, ids: &[MediaId], tags: &[String]) -> Result<()> {
        if ids.is_empty() || tags.is_empty() {
            return Ok(());
        }

        let normalized_tags: Vec<String> = tags
            .iter()
            .map(|t| clean_text(t, MAX_NAME_CHARS).to_lowercase())
            .filter(|t| !t.is_empty())
            .collect();

        if normalized_tags.is_empty() {
            return Ok(());
        }

        let raw_ids: Vec<i64> = ids.iter().map(|id| id.0 as i64).collect();
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;

        Self::delete_fts_for_items(&tx, &raw_ids)?;

        {
            let mut stmt = tx.prepare_cached(
                "INSERT OR IGNORE INTO media_tags (media_id, tag) VALUES (?1, ?2);",
            )?;
            for &id in &raw_ids {
                for tag in &normalized_tags {
                    stmt.execute(params![id, tag])?;
                }
            }
        }

        Self::insert_fts_for_items(&tx, &raw_ids)?;

        tx.commit()?;
        Ok(())
    }

    /// Removes one or more tags from a batch of media items.
    pub fn remove_tags_batch(&self, ids: &[MediaId], tags: &[String]) -> Result<()> {
        if ids.is_empty() || tags.is_empty() {
            return Ok(());
        }

        let normalized_tags: Vec<String> = tags
            .iter()
            .map(|t| t.trim().to_lowercase())
            .filter(|t| !t.is_empty())
            .collect();

        if normalized_tags.is_empty() {
            return Ok(());
        }

        let raw_ids: Vec<i64> = ids.iter().map(|id| id.0 as i64).collect();
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;

        Self::delete_fts_for_items(&tx, &raw_ids)?;

        {
            let mut stmt =
                tx.prepare_cached("DELETE FROM media_tags WHERE media_id = ?1 AND tag = ?2;")?;
            for &id in &raw_ids {
                for tag in &normalized_tags {
                    stmt.execute(params![id, tag])?;
                }
            }
        }

        Self::insert_fts_for_items(&tx, &raw_ids)?;

        tx.commit()?;
        Ok(())
    }

    /// Retrieves all tags assigned to a single media item.
    pub fn get_tags_for_item(&self, id: MediaId) -> Result<Vec<String>> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare_cached("SELECT tag FROM media_tags WHERE media_id = ?1 ORDER BY tag ASC;")?;
        let rows = stmt.query_map(params![id.0 as i64], |row| row.get::<_, String>(0))?;
        let mut tags = Vec::new();
        for r in rows {
            tags.push(r?);
        }
        Ok(tags)
    }

    /// For each tag on any of `ids`, how many of those items carry it, ordered
    /// by tag. The batch inspector uses this to tell tags on every selected
    /// item from tags on only some of them.
    pub fn tag_counts_for_items(&self, ids: &[MediaId]) -> Result<Vec<(String, usize)>> {
        let conn = self.conn.lock();
        let mut counts = std::collections::BTreeMap::<String, usize>::new();
        // Chunks keep each statement under SQLite's bound-parameter limit; a
        // (media_id, tag) row is unique, so per-chunk counts add up exactly.
        for chunk in ids.chunks(500) {
            let placeholders: Vec<String> = (1..=chunk.len()).map(|i| format!("?{}", i)).collect();
            let sql = format!(
                "SELECT tag, COUNT(*) FROM media_tags WHERE media_id IN ({}) GROUP BY tag;",
                placeholders.join(", ")
            );
            let raw_ids: Vec<i64> = chunk.iter().map(|id| id.0 as i64).collect();
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(rusqlite::params_from_iter(raw_ids.iter()), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize))
            })?;
            for r in rows {
                let (tag, n) = r?;
                *counts.entry(tag).or_default() += n;
            }
        }
        Ok(counts.into_iter().collect())
    }

    /// The subset of `ids` that carry `tag`. Tag undo uses it to record only
    /// the items an add or remove actually changes.
    pub fn ids_with_tag(&self, ids: &[MediaId], tag: &str) -> Result<Vec<MediaId>> {
        let conn = self.conn.lock();
        let mut found = Vec::new();
        for chunk in ids.chunks(500) {
            // ?1 is the tag; the ids follow from ?2.
            let placeholders: Vec<String> =
                (2..=chunk.len() + 1).map(|i| format!("?{}", i)).collect();
            let sql = format!(
                "SELECT media_id FROM media_tags WHERE tag = ?1 AND media_id IN ({});",
                placeholders.join(", ")
            );
            let mut values: Vec<rusqlite::types::Value> = vec![tag.to_string().into()];
            values.extend(
                chunk
                    .iter()
                    .map(|id| rusqlite::types::Value::from(id.0 as i64)),
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(rusqlite::params_from_iter(values.iter()), |row| {
                row.get::<_, i64>(0)
            })?;
            for r in rows {
                found.push(MediaId(r? as u64));
            }
        }
        Ok(found)
    }

    /// Returns all distinct tags in the catalog, ordered alphabetically.
    pub fn get_all_tags(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock();
        let mut stmt =
            conn.prepare_cached("SELECT DISTINCT tag FROM media_tags ORDER BY tag ASC;")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut tags = Vec::new();
        for r in rows {
            tags.push(r?);
        }
        Ok(tags)
    }

    /// Returns all tags along with their active (non-trashed, non-hidden) item counts.
    pub fn query_tag_counts(&self) -> Result<Vec<(String, usize)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            r#"
            SELECT mt.tag, COUNT(mt.media_id) as cnt
            FROM media_tags mt
            JOIN media_items m ON m.id = mt.media_id
            WHERE m.is_trashed = 0 AND m.is_hidden = 0
            GROUP BY mt.tag
            ORDER BY cnt DESC, mt.tag ASC;
            "#,
        )?;
        let rows = stmt.query_map([], |row| {
            let tag: String = row.get(0)?;
            let count: i64 = row.get(1)?;
            Ok((tag, count as usize))
        })?;
        let mut counts = Vec::new();
        for r in rows {
            counts.push(r?);
        }
        Ok(counts)
    }

    /// Globally renames a tag across all media items (handles collision by
    /// merging). The new name is cleaned like [`Self::add_tags_batch`]; the
    /// old one is only trimmed, so a tag stored before cleaning still matches.
    pub fn rename_tag(&self, old_tag: &str, new_tag: &str) -> Result<usize> {
        let old_clean = old_tag.trim().to_lowercase();
        let new_clean = clean_text(new_tag, MAX_NAME_CHARS).to_lowercase();
        if old_clean.is_empty() || new_clean.is_empty() || old_clean == new_clean {
            return Ok(0);
        }

        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;

        let affected_ids: Vec<i64> = {
            let mut stmt = tx.prepare("SELECT media_id FROM media_tags WHERE tag = ?1;")?;
            let rows = stmt.query_map(params![old_clean], |row| row.get::<_, i64>(0))?;
            let mut ids = Vec::new();
            for r in rows {
                ids.push(r?);
            }
            ids
        };

        if affected_ids.is_empty() {
            return Ok(0);
        }

        Self::delete_fts_for_items(&tx, &affected_ids)?;

        // Insert new tag where not already present, then remove old tag
        tx.execute(
            "INSERT OR IGNORE INTO media_tags (media_id, tag) SELECT media_id, ?1 FROM media_tags WHERE tag = ?2;",
            params![new_clean, old_clean],
        )?;
        let count = tx.execute("DELETE FROM media_tags WHERE tag = ?1;", params![old_clean])?;

        // Cascade tag rename to any SmartAlbum rules referencing old_clean
        let albums_rows: Vec<(i64, String)> = {
            let mut stmt = tx.prepare("SELECT id, rules_json FROM smart_albums;")?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?;
            let mut list = Vec::new();
            for r in rows {
                list.push(r?);
            }
            list
        };

        for (album_id, rules_json) in albums_rows {
            if let Ok(mut rules) =
                serde_json::from_str::<Vec<crate::smart_albums::SmartAlbumRule>>(&rules_json)
            {
                let mut modified = false;
                for rule in &mut rules {
                    if rule.field == crate::smart_albums::RuleField::Tag {
                        if let crate::smart_albums::RuleValue::Text(ref val) = rule.value {
                            if val.trim().to_lowercase() == old_clean {
                                rule.value =
                                    crate::smart_albums::RuleValue::Text(new_clean.clone());
                                modified = true;
                            }
                        }
                    }
                }
                if modified {
                    if let Ok(new_json) = serde_json::to_string(&rules) {
                        tx.execute(
                            "UPDATE smart_albums SET rules_json = ?1 WHERE id = ?2;",
                            params![new_json, album_id],
                        )?;
                    }
                }
            }
        }

        Self::insert_fts_for_items(&tx, &affected_ids)?;

        tx.commit()?;
        Ok(count)
    }

    /// Globally deletes a tag from all media items.
    pub fn delete_tag_globally(&self, tag: &str) -> Result<usize> {
        let clean = tag.trim().to_lowercase();
        if clean.is_empty() {
            return Ok(0);
        }

        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;

        let affected_ids: Vec<i64> = {
            let mut stmt = tx.prepare("SELECT media_id FROM media_tags WHERE tag = ?1;")?;
            let rows = stmt.query_map(params![clean], |row| row.get::<_, i64>(0))?;
            let mut ids = Vec::new();
            for r in rows {
                ids.push(r?);
            }
            ids
        };

        if affected_ids.is_empty() {
            return Ok(0);
        }

        Self::delete_fts_for_items(&tx, &affected_ids)?;

        let count = tx.execute("DELETE FROM media_tags WHERE tag = ?1;", params![clean])?;

        Self::insert_fts_for_items(&tx, &affected_ids)?;

        tx.commit()?;
        Ok(count)
    }

    /// Merges `source_tag` into `target_tag` across all media items.
    pub fn merge_tags(&self, source_tag: &str, target_tag: &str) -> Result<usize> {
        self.rename_tag(source_tag, target_tag)
    }

    /// Counts active media items that have no `media_tags` rows at all.
    ///
    /// Hidden and trashed items are excluded to match the other sidebar counts.
    /// The correlated `NOT EXISTS` uses `idx_media_tags_media_id`, so this is
    /// one aggregate query and never a full scan of the tag table.
    pub fn query_untagged_count(&self) -> Result<usize> {
        let conn = self.conn.lock();
        let count: i64 = conn.query_row(
            r#"
            SELECT COUNT(*)
            FROM media_items m
            WHERE m.is_trashed = 0
              AND m.is_hidden = 0
              AND NOT EXISTS (
                  SELECT 1 FROM media_tags mt WHERE mt.media_id = m.id
              );
            "#,
            [],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    /// Queries a page of active media items that have no `media_tags` rows.
    ///
    /// Uses the same media column list and `map_query_rows` mapper as the rest
    /// of the catalog so callers receive complete [`MediaItem`]s, and applies
    /// the caller's [`SortSpec`] instead of re-sorting a page in the UI.
    pub fn query_untagged_sorted(
        &self,
        limit: usize,
        offset: usize,
        sort: SortSpec,
    ) -> Result<Vec<MediaItem>> {
        let conn = self.conn.lock();
        let sql = format!(
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
            WHERE m.is_trashed = 0
              AND m.is_hidden = 0
              AND NOT EXISTS (
                  SELECT 1 FROM media_tags mt WHERE mt.media_id = m.id
              )
            ORDER BY {}
            LIMIT ?1 OFFSET ?2;
            "#,
            sort.order_by("m.")
        );
        let mut stmt = conn.prepare_cached(&sql)?;
        Self::map_query_rows(&mut stmt, params![limit as i64, offset as i64])
    }

    /// Queries media items matching a specific tag.
    pub fn get_items_by_tag(
        &self,
        tag: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<MediaItem>> {
        let clean = tag.trim().to_lowercase();
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
            JOIN media_tags mt ON mt.media_id = m.id
            WHERE mt.tag = ?1 AND m.is_trashed = 0 AND m.is_hidden = 0
            ORDER BY m.timestamp_epoch_ms DESC, m.id DESC
            LIMIT ?2 OFFSET ?3;
            "#,
        )?;
        Self::map_query_rows(&mut stmt, params![clean, limit as i64, offset as i64])
    }
}

#[cfg(test)]
mod untagged_media_tests {
    use super::*;
    use crate::{MediaType, SortDirection, SortField};

    fn media_item(path: &str, timestamp_epoch_ms: i64, hidden: bool, trashed: bool) -> MediaItem {
        let path = std::path::PathBuf::from(path);
        let parent_dir = path
            .parent()
            .map(|parent| parent.to_string_lossy().into_owned())
            .unwrap_or_default();
        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        MediaItem {
            id: MediaId(0),
            path,
            parent_dir,
            file_name,
            title: None,
            file_size: 1024,
            mtime_epoch_ms: timestamp_epoch_ms,
            media_type: MediaType::Image,
            width: 640,
            height: 480,
            duration_ms: None,
            timestamp_epoch_ms,
            year: 2026,
            month: 1,
            day: 1,
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
            is_hidden: hidden,
            is_trashed: trashed,
        }
    }

    #[test]
    fn every_text_write_stores_one_clean_line() {
        let catalog = Catalog::open_in_memory().expect("open in-memory catalog");
        let id = catalog
            .insert_or_update_batch(&[media_item("/lib/a.jpg", 1_000, false, false)])
            .expect("insert media")[0]
            .id;
        let dirty = "  a\u{202E}b\n\u{FFFF} c  ";

        catalog.set_title(id, Some(dirty)).expect("set title");
        let item = catalog.get_item_by_id(id).expect("read").expect("item");
        assert_eq!(item.title.as_deref(), Some("a b c"), "title is cleaned");

        catalog.set_caption(id, dirty).expect("set caption");
        let captions = catalog.get_captions_map().expect("captions");
        assert_eq!(
            captions[&id], "a b\nc",
            "caption is cleaned, keeping its line break"
        );

        catalog
            .add_tags_batch(&[id], &[dirty.to_string(), "x".repeat(150)])
            .expect("add tags");
        let mut tags = catalog.get_tags_for_item(id).expect("tags");
        tags.sort();
        assert_eq!(
            tags,
            vec!["a b c".to_string(), "x".repeat(crate::text::MAX_NAME_CHARS)],
            "tags are cleaned and capped"
        );

        catalog
            .rename_tag("a b c", "Trip\u{2028}2026")
            .expect("rename");
        assert!(
            catalog
                .get_tags_for_item(id)
                .expect("tags")
                .contains(&"trip 2026".to_string()),
            "a renamed tag is cleaned"
        );

        let album = catalog
            .create_smart_album(dirty, "folder", crate::MatchMode::All, &[])
            .expect("create album");
        assert_eq!(album.title, "a b c", "smart album title is cleaned");
        let stored = catalog
            .get_smart_album(album.id)
            .expect("read album")
            .expect("album");
        assert_eq!(stored.title, "a b c", "the stored album title is cleaned");
        catalog
            .update_smart_album(
                album.id,
                "Edited\t\u{FEFF}",
                "folder",
                crate::MatchMode::All,
                &[],
            )
            .expect("update album");
        let stored = catalog
            .get_smart_album(album.id)
            .expect("read")
            .expect("album");
        assert_eq!(stored.title, "Edited", "an edited album title is cleaned");

        catalog
            .set_titles(&[(id, Some("batch\u{2028}title "))])
            .expect("batch titles");
        let item = catalog.get_item_by_id(id).expect("read").expect("item");
        assert_eq!(
            item.title.as_deref(),
            Some("batch title"),
            "batch titles are cleaned"
        );

        catalog
            .set_captions(&[(id, "Para\u{202E} one\n\nPara two\n")])
            .expect("batch captions");
        assert_eq!(
            catalog.get_captions_map().expect("captions")[&id],
            "Para one\n\nPara two",
            "captions keep their paragraphs but lose disguising characters"
        );
    }

    #[test]
    fn untagged_count_and_query_exclude_tagged_hidden_and_trashed() {
        let catalog = Catalog::open_in_memory().expect("open in-memory catalog");
        let inserted = catalog
            .insert_or_update_batch(&[
                media_item("/lib/untagged_a.jpg", 1_000, false, false),
                media_item("/lib/tagged.jpg", 2_000, false, false),
                media_item("/lib/hidden.jpg", 3_000, true, false),
                media_item("/lib/trashed.jpg", 4_000, false, true),
                media_item("/lib/untagged_b.jpg", 5_000, false, false),
            ])
            .expect("insert media");

        catalog
            .add_tags_batch(&[inserted[1].id], &["travel".to_string()])
            .expect("tag one item");

        assert_eq!(
            catalog.query_untagged_count().expect("count untagged"),
            2,
            "only active, untagged media may be counted"
        );

        let default_page = catalog
            .query_untagged_sorted(10, 0, SortSpec::default())
            .expect("query untagged");
        assert_eq!(
            default_page.iter().map(|item| item.id).collect::<Vec<_>>(),
            vec![inserted[4].id, inserted[0].id],
            "untagged media must follow the requested date-desc order"
        );

        let by_name = catalog
            .query_untagged_sorted(
                10,
                0,
                SortSpec {
                    field: SortField::FileName,
                    direction: SortDirection::Ascending,
                },
            )
            .expect("query untagged by name");
        assert_eq!(
            by_name.iter().map(|item| item.id).collect::<Vec<_>>(),
            vec![inserted[0].id, inserted[4].id],
            "the caller's SortSpec must be honored, not a fixed SQL order"
        );

        let second_page = catalog
            .query_untagged_sorted(1, 1, SortSpec::default())
            .expect("page untagged");
        assert_eq!(second_page.len(), 1, "limit and offset must page");
        assert_eq!(second_page[0].id, inserted[0].id);

        assert!(
            catalog
                .query_untagged_sorted(10, 2, SortSpec::default())
                .expect("page past the end")
                .is_empty(),
            "an offset past the last untagged item must return an empty page"
        );
    }

    #[test]
    fn untagged_count_tracks_add_remove_rename_merge_and_delete() {
        let catalog = Catalog::open_in_memory().expect("open in-memory catalog");
        let inserted = catalog
            .insert_or_update_batch(&[
                media_item("/lib/first.jpg", 1_000, false, false),
                media_item("/lib/second.jpg", 2_000, false, false),
            ])
            .expect("insert media");
        let first = inserted[0].id;

        assert_eq!(
            catalog.query_untagged_count().expect("initial count"),
            2,
            "a fresh catalog has every item untagged"
        );

        catalog
            .add_tags_batch(&[first], &["travel".to_string()])
            .expect("add tag");
        assert_eq!(
            catalog.query_untagged_count().expect("count after add"),
            1,
            "tagging an item must remove it from the untagged count"
        );

        catalog
            .add_tags_batch(&[first], &["vacation".to_string()])
            .expect("add second tag");
        assert_eq!(
            catalog
                .query_untagged_count()
                .expect("count after second add"),
            1,
            "extra tags must not change the untagged count"
        );

        catalog
            .remove_tags_batch(&[first], &["vacation".to_string()])
            .expect("remove tag");
        assert_eq!(
            catalog.query_untagged_count().expect("count after remove"),
            1,
            "removing one of two tags keeps the item tagged"
        );

        assert_eq!(
            catalog.rename_tag("travel", "journey").expect("rename tag"),
            1
        );
        assert_eq!(
            catalog.query_untagged_count().expect("count after rename"),
            1,
            "renaming a tag must not change untagged membership"
        );

        assert_eq!(catalog.merge_tags("journey", "trip").expect("merge tag"), 1);
        assert_eq!(
            catalog.query_untagged_count().expect("count after merge"),
            1,
            "merging tags must leave tagged items tagged"
        );

        assert_eq!(
            catalog
                .delete_tag_globally("trip")
                .expect("delete tag globally"),
            1
        );
        assert_eq!(
            catalog.query_untagged_count().expect("count after delete"),
            2,
            "deleting the last tag must return the item to untagged"
        );
        assert_eq!(
            catalog
                .query_untagged_sorted(10, 0, SortSpec::default())
                .expect("query after delete")
                .len(),
            2,
            "the sorted query must agree with the refreshed count"
        );
    }
}
