use anyhow::Result;
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Transaction};

use super::keyword_ops::{refuse_alias_path, resolve_in};
use super::keywords::{filter_value, normalize_path, subtree_clause, subtree_params};
use super::Catalog;
use crate::{MediaId, MediaItem, SortSpec};

impl Catalog {
    /// Deletes existing FTS5 index rows for a set of media items before tag mutations.
    ///
    /// `media_search_fts` is contentless, so a 'delete' must pass exactly the
    /// values that were indexed: the same columns, in the same order, as the
    /// `media_fts_*` triggers in `schema.rs`. Leaving one out drops a token's
    /// whole entry for the row wherever else it appears.
    pub(crate) fn delete_fts_for_items(tx: &Transaction, ids: &[i64]) -> Result<()> {
        for chunk in ids.chunks(500) {
            let placeholders: Vec<String> = (1..=chunk.len()).map(|i| format!("?{}", i)).collect();
            let in_clause = placeholders.join(", ");
            let del_sql = format!(
                r#"
                INSERT INTO media_search_fts(
                    media_search_fts, rowid, file_name, parent_dir, city, admin1_region, country_code, make, model, lens, year, tags, caption
                )
                SELECT
                    'delete', m.id, m.file_name, m.parent_dir, m.city, m.admin1_region, m.country_code, m.make, m.model, m.lens, CAST(m.year AS TEXT),
                    COALESCE((SELECT GROUP_CONCAT(tag, ' ') FROM media_tags WHERE media_id = m.id), ''),
                    COALESCE((SELECT caption FROM media_captions WHERE media_id = m.id), '')
                FROM media_items m
                WHERE m.id IN ({});
                "#,
                in_clause
            );
            tx.execute(&del_sql, rusqlite::params_from_iter(chunk.iter()))?;
        }
        Ok(())
    }

    /// Re-inserts updated FTS5 index rows for a set of media items after tag
    /// mutations, with the same columns as the `media_fts_*` triggers.
    pub(crate) fn insert_fts_for_items(tx: &Transaction, ids: &[i64]) -> Result<()> {
        for chunk in ids.chunks(500) {
            let placeholders: Vec<String> = (1..=chunk.len()).map(|i| format!("?{}", i)).collect();
            let in_clause = placeholders.join(", ");
            let ins_sql = format!(
                r#"
                INSERT INTO media_search_fts(
                    rowid, file_name, parent_dir, city, admin1_region, country_code, make, model, lens, year, tags, caption
                )
                SELECT
                    m.id, m.file_name, m.parent_dir, m.city, m.admin1_region, m.country_code, m.make, m.model, m.lens, CAST(m.year AS TEXT),
                    COALESCE((SELECT GROUP_CONCAT(tag, ' ') FROM media_tags WHERE media_id = m.id), ''),
                    COALESCE((SELECT caption FROM media_captions WHERE media_id = m.id), '')
                FROM media_items m
                WHERE m.id IN ({});
                "#,
                in_clause
            );
            tx.execute(&ins_sql, rusqlite::params_from_iter(chunk.iter()))?;
        }
        Ok(())
    }

    /// Attaches one or more tags to a batch of media items, each stored as
    /// its [`normalize_path`] form, or the keyword it is an alias of (exact
    /// rows; no ancestor rows are added). Refuses a path below an alias.
    /// Refuses the whole batch, writing nothing, when any tag is empty or
    /// longer than [`crate::text::MAX_NAME_CHARS`].
    pub fn add_tags_batch(&self, ids: &[MediaId], tags: &[String]) -> Result<()> {
        if ids.is_empty() || tags.is_empty() {
            return Ok(());
        }

        let normalized_tags = tags
            .iter()
            .map(|t| normalize_path(t))
            .collect::<Result<Vec<String>>>()?;

        let raw_ids: Vec<i64> = ids.iter().map(|id| id.0 as i64).collect();
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        // An alias stores its keyword, never itself.
        let normalized_tags = normalized_tags
            .into_iter()
            .map(|tag| {
                let tag = resolve_in(&tx, tag)?;
                refuse_alias_path(&tx, &tag)?;
                Ok(tag)
            })
            .collect::<Result<Vec<String>>>()?;

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

    /// Removes one or more tags from a batch of media items. Exact rows:
    /// removing `a` leaves `a/x` alone (tag undo depends on it).
    pub fn remove_tags_batch(&self, ids: &[MediaId], tags: &[String]) -> Result<()> {
        if ids.is_empty() || tags.is_empty() {
            return Ok(());
        }

        // Both the text as given and its path form: a chip shows the stored
        // text, and a row saved before paths were normalized (`a / b`, or
        // over 100 characters) must stay removable by it.
        let mut normalized_tags: Vec<String> = Vec::new();
        for tag in tags {
            let exact = tag.trim().to_lowercase();
            if let Ok(path) = normalize_path(tag) {
                if path != exact {
                    normalized_tags.push(path);
                }
            }
            if !exact.is_empty() {
                normalized_tags.push(exact);
            }
        }

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
        Ok(rows.collect::<rusqlite::Result<_>>()?)
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
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Counts active media items that have no `media_tags` rows at all.
    ///
    /// Hidden and trashed items are excluded to match the other sidebar counts.
    /// The correlated `NOT EXISTS` uses `sqlite_autoindex_media_tags_1`, so this is
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
            SELECT {columns}
            FROM media_items m
            WHERE m.is_trashed = 0
              AND m.is_hidden = 0
              AND NOT EXISTS (
                  SELECT 1 FROM media_tags mt WHERE mt.media_id = m.id
              )
            ORDER BY {order}
            LIMIT ?1 OFFSET ?2;
            "#,
            columns = crate::sql::media_columns("m."),
            order = sort.order_by("m.")
        );
        let mut stmt = conn.prepare_cached(&sql)?;
        Self::map_query_rows(&mut stmt, params![limit as i64, offset as i64])
    }

    /// Queries media items carrying `tag` or a keyword below it, each once.
    pub fn get_items_by_tag(
        &self,
        tag: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<MediaItem>> {
        let clean = filter_value(tag);
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(&items_by_tag_sql())?;
        let mut values: Vec<Value> = subtree_params(&clean).into();
        values.extend([Value::Integer(limit as i64), Value::Integer(offset as i64)]);
        Self::map_query_rows(&mut stmt, params_from_iter(values))
    }
}

/// The `get_items_by_tag` query, one `?` per subtree value then limit and
/// offset. Shared with the plan test.
///
/// `IN`, not `EXISTS`: both avoid one row per matching tag, but `IN`
/// lets SQLite start from the tag range and sort the few hits instead of
/// walking the whole timeline with a correlated subquery per row (a rare
/// tag on 100k items: ~60 µs vs ~56 ms).
// ponytail: even with LIMIT 24 every match is read and sorted, so the cost
// grows with the tag's size (a 50k-item tag: tens of ms, on the UI thread in
// the tag manager, about 2x the old JOIN). Load the tag manager's grid off the
// UI thread if large tags make arrowing through it feel slow.
fn items_by_tag_sql() -> String {
    format!(
        r#"
            SELECT {columns}
            FROM media_items m
            WHERE m.id IN (SELECT media_id FROM media_tags WHERE {clause})
                AND m.is_trashed = 0 AND m.is_hidden = 0
            ORDER BY m.timestamp_epoch_ms DESC, m.id DESC
            LIMIT ? OFFSET ?;
            "#,
        columns = crate::sql::media_columns("m."),
        clause = subtree_clause("tag")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::MAX_NAME_CHARS;
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
    fn the_revision_moves_on_writes_and_not_on_reads() {
        let catalog = Catalog::open_in_memory().expect("open in-memory catalog");
        let id = catalog
            .insert_or_update_batch(&[media_item("/lib/a.jpg", 1_000, false, false)])
            .expect("insert media")[0]
            .id;
        let before = catalog.revision();
        catalog.get_tags_for_item(id).expect("read tags");
        catalog.get_item_by_id(id).expect("read item");
        assert_eq!(catalog.revision(), before, "reads leave the revision alone");
        catalog
            .add_tags_batch(&[id], &["trip".to_string()])
            .expect("add tag");
        let after_tag = catalog.revision();
        assert_ne!(after_tag, before, "a tag write moves the revision");
        catalog.set_title(id, Some("Lisbon")).expect("set title");
        assert_ne!(catalog.revision(), after_tag, "a title write moves it too");
        assert_eq!(
            catalog.revision_written_here(),
            catalog.revision(),
            "this thread's own write is recorded as its own"
        );

        let mine = catalog.revision_written_here();
        let other = catalog.clone();
        std::thread::spawn(move || other.set_title(id, Some("Porto")).expect("set title"))
            .join()
            .expect("writer thread");
        assert_ne!(
            catalog.revision(),
            mine,
            "another thread's write moves the revision"
        );
        assert_eq!(
            catalog.revision_written_here(),
            mine,
            "but is not recorded as this thread's write"
        );
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
            .add_tags_batch(&[id], &[dirty.to_string()])
            .expect("add tags");
        assert!(
            catalog.add_tags_batch(&[id], &["x".repeat(150)]).is_err(),
            "an over-long tag is refused: cutting it would store a different keyword"
        );
        assert_eq!(
            catalog.get_tags_for_item(id).expect("tags"),
            vec!["a b c".to_string()],
            "tags are cleaned to one line"
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
    fn items_by_tag_plan_drives_from_the_tag_range() {
        let catalog = Catalog::open_in_memory().expect("open in-memory catalog");
        let items: Vec<_> = (0..2_000)
            .map(|i| media_item(&format!("/lib/{i}.jpg"), i, false, false))
            .collect();
        let ids: Vec<_> = catalog
            .insert_or_update_batch(&items)
            .expect("insert media")
            .iter()
            .map(|it| it.id)
            .collect();
        catalog
            .add_tags_batch(&ids[..2], &["rare".to_string()])
            .expect("tag");
        catalog
            .add_tags_batch(&ids, &["common".to_string()])
            .expect("tag");
        // No ANALYZE: the app never runs it, so plan without statistics.
        // Inline the values (explain binds nothing): four times the tag,
        // then limit and offset.
        let mut values = ["'rare'"; 4].into_iter().chain(["24", "0"]);
        let sql: String = items_by_tag_sql()
            .chars()
            .map(|c| match c {
                '?' => values.next().expect("one value per ?").to_string(),
                c => c.to_string(),
            })
            .collect();
        let rows = catalog.explain_query_plan(&sql).expect("explain");
        let plan = rows.join(" | ");
        assert!(
            rows.iter().any(
                |r| r.starts_with("SEARCH media_tags USING INDEX idx_media_tags_tag")
                    && r.contains("tag>")
            ),
            "a rare tag must seek media_tags by tag range: {plan}"
        );
        assert!(
            !rows
                .iter()
                .any(|r| r.starts_with("SCAN m") || r.contains("CORRELATED")),
            "no timeline walk with a per-row subquery: {plan}"
        );
    }

    #[test]
    fn a_legacy_over_long_tag_stays_removable() {
        let catalog = Catalog::open_in_memory().expect("open in-memory catalog");
        let id = catalog
            .insert_or_update_batch(&[media_item("/lib/a.jpg", 1_000, false, false)])
            .expect("insert media")[0]
            .id;
        let legacy = "y".repeat(MAX_NAME_CHARS + 50);
        {
            // Seeded the way the catalog writes tags, keeping the index in step.
            let mut conn = catalog.conn.lock();
            let tx = conn.transaction().expect("begin");
            Catalog::delete_fts_for_items(&tx, &[id.0 as i64]).expect("unindex");
            tx.execute(
                "INSERT INTO media_tags (media_id, tag) VALUES (?1, ?2)",
                params![id.0 as i64, legacy],
            )
            .expect("seed a tag saved before paths were capped");
            Catalog::insert_fts_for_items(&tx, &[id.0 as i64]).expect("reindex");
            tx.commit().expect("commit");
        }
        catalog
            .remove_tags_batch(&[id], &[legacy])
            .expect("removing a legacy tag succeeds");
        assert!(
            catalog.get_tags_for_item(id).expect("tags").is_empty(),
            "a tag too long to normalize is still removed by its exact text"
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
            catalog
                .rename_tag("travel", "journey")
                .expect("rename tag")
                .removed
                .len(),
            1
        );
        assert_eq!(
            catalog.query_untagged_count().expect("count after rename"),
            1,
            "renaming a tag must not change untagged membership"
        );

        assert_eq!(
            catalog
                .merge_tags("journey", "trip")
                .expect("merge tag")
                .removed
                .len(),
            1
        );
        assert_eq!(
            catalog.query_untagged_count().expect("count after merge"),
            1,
            "merging tags must leave tagged items tagged"
        );

        assert_eq!(
            catalog
                .delete_tag_globally("trip")
                .expect("delete tag globally")
                .removed
                .len(),
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
