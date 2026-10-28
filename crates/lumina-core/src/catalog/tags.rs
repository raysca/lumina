use anyhow::Result;
use rusqlite::{params, Transaction};

use super::Catalog;
use crate::{MediaId, MediaItem};

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

    /// Attaches one or more normalized tags to a batch of media items.
    pub fn add_tags_batch(&self, ids: &[MediaId], tags: &[String]) -> Result<()> {
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

    /// Retrieves distinct tags across a batch of media items (useful for multi-select inspector).
    pub fn get_tags_for_items_batch(&self, ids: &[MediaId]) -> Result<Vec<String>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.conn.lock();
        let mut set = std::collections::BTreeSet::new();
        for chunk in ids.chunks(500) {
            let placeholders: Vec<String> = (1..=chunk.len()).map(|i| format!("?{}", i)).collect();
            let sql = format!(
                "SELECT DISTINCT tag FROM media_tags WHERE media_id IN ({}) ORDER BY tag ASC;",
                placeholders.join(", ")
            );
            let raw_ids: Vec<i64> = chunk.iter().map(|id| id.0 as i64).collect();
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(rusqlite::params_from_iter(raw_ids.iter()), |row| {
                row.get::<_, String>(0)
            })?;
            for r in rows {
                set.insert(r?);
            }
        }
        Ok(set.into_iter().collect())
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

    /// Globally renames a tag across all media items (handles collision by merging).
    pub fn rename_tag(&self, old_tag: &str, new_tag: &str) -> Result<usize> {
        let old_clean = old_tag.trim().to_lowercase();
        let new_clean = new_tag.trim().to_lowercase();
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
                m.is_favorite, m.is_hidden, m.is_trashed
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
