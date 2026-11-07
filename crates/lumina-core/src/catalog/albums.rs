use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{params, OptionalExtension};

use super::Catalog;
use crate::search::{GeoIndex, StructuredQuery};
use crate::smart_albums::{MatchMode, SmartAlbum, SmartAlbumRule, SystemAlbum};
use crate::text::{clean_text, MAX_NAME_CHARS};
use crate::{MediaItem, SortSpec};

impl Catalog {
    /// High-speed full-text search over filename, city, region, country, camera model, year, and tags.
    pub fn search_fts(&self, query: &str, limit: usize) -> Result<Vec<MediaItem>> {
        let q = StructuredQuery::parse(query);
        self.search_structured(&q, limit, 0)
    }

    /// Executes a structured search query with optional FTS5 matching and metadata predicates.
    pub fn search_structured(
        &self,
        query: &StructuredQuery,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<MediaItem>> {
        self.search_structured_sorted(query, limit, offset, SortSpec::default())
    }

    pub fn search_structured_sorted(
        &self,
        query: &StructuredQuery,
        limit: usize,
        offset: usize,
        sort: SortSpec,
    ) -> Result<Vec<MediaItem>> {
        let (sql, params) = self.search_sql_sorted(query, limit, offset, sort)?;
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(&sql)?;
        Self::map_query_rows(&mut stmt, rusqlite::params_from_iter(params.iter()))
    }

    /// The SQL and parameters [`Self::search_structured_sorted`] runs.
    /// Public so plan tests can `EXPLAIN` the real query.
    ///
    /// A `within:` page either walks the sort index and filters the box, or
    /// seeks `idx_media_geo` and sorts the box's matches. With
    /// `cap = sqrt((offset + limit) * rows)`: a box with fewer than `cap`
    /// matches seeks the index (an index-only count decides). Otherwise a
    /// probe reads the first `cap` rows of the walk: if they already hold
    /// the page, the walk reads at most `cap` rows (proven only when
    /// `within:` is the sole filter: both probes test the box alone, so a
    /// dense box with a rare second filter can still walk far, no worse than
    /// that filter alone) and the box is written so the geo index cannot
    /// serve it; if not, the matches lie far down the sort order (an old
    /// trip) and sorting them is cheaper.
    pub fn search_sql_sorted(
        &self,
        query: &StructuredQuery,
        limit: usize,
        offset: usize,
        sort: SortSpec,
    ) -> Result<(String, Vec<rusqlite::types::Value>)> {
        let geo = if query.within.is_some() {
            self.geo_index_choice(query, limit, offset, sort)?
        } else {
            GeoIndex::Use
        };
        Ok(query.synthesize_query_geo(limit, offset, sort, geo))
    }

    /// The probe behind [`Self::search_sql_sorted`] for a `within:` query.
    fn geo_index_choice(
        &self,
        query: &StructuredQuery,
        limit: usize,
        offset: usize,
        sort: SortSpec,
    ) -> Result<GeoIndex> {
        let conn = self.conn.lock();
        // ponytail: max(rowid) stands in for the row count (O(1)); it
        // overcounts after deletes, which only raises the cap. Ceiling: the
        // walk probe reads `cap` rows of the sort index (~2.2k for page 0 at
        // 100k), and runs only for a box with at least `cap` matches; on a
        // deep page `cap` grows (~55k for a world box @30000), so the total
        // stays within ~2 x cap.
        // Follow-up, not done: a deep page of a dense box still sorts every
        // match (100k rows, ~8.9k in the box, @8500: 8.5 ms; `camera:` or
        // `iso:` at that depth take 25-30 ms); an id-only sort through a
        // covering geo index would cut it.
        let rows: i64 = conn
            .prepare_cached("SELECT coalesce(max(rowid), 0) FROM media_items;")?
            .query_row([], |row| row.get(0))?;
        let cap = (((offset + limit) as f64) * rows as f64).sqrt().max(1.0) as usize;
        let Some((probe, params)) = query.walk_probe_sql(cap, sort) else {
            return Ok(GeoIndex::Use);
        };
        // Fewer than `cap` matches: sorting them is already within the bound,
        // and this index-only count is far cheaper than the walk probe on a
        // deep page (which would read `cap` ~ 29k rows at 100k, @8500).
        if let Some((count_sql, count_params)) = query.box_count_sql(cap) {
            let matches: i64 = conn
                .prepare_cached(&count_sql)?
                .query_row(rusqlite::params_from_iter(count_params.iter()), |row| {
                    row.get(0)
                })
                .context("counting the within: box")?;
            if (matches as usize) < cap {
                return Ok(GeoIndex::Use);
            }
        }
        let found: i64 = conn
            .prepare_cached(&probe)?
            .query_row(rusqlite::params_from_iter(params.iter()), |row| row.get(0))
            .context("probing the within: box")?;
        Ok(if found as usize >= offset + limit {
            GeoIndex::Skip
        } else {
            GeoIndex::Use
        })
    }

    pub fn search_structured_count(&self, query: &StructuredQuery) -> Result<usize> {
        let (sql, params) = query.synthesize_count_query();
        let conn = self.conn.lock();
        let count: i64 =
            conn.query_row(&sql, rusqlite::params_from_iter(params.iter()), |row| {
                row.get(0)
            })?;
        Ok(count as usize)
    }

    /// Queries a built-in System Smart Album (Favorites, Videos, RAW, Screenshots, Places, etc.).
    pub fn query_system_album(
        &self,
        album: SystemAlbum,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<MediaItem>> {
        self.query_system_album_sorted(album, limit, offset, SortSpec::default())
    }

    pub fn query_system_album_sorted(
        &self,
        album: SystemAlbum,
        limit: usize,
        offset: usize,
        sort: SortSpec,
    ) -> Result<Vec<MediaItem>> {
        let (sql, params) = album.synthesize_query_sorted(limit, offset, sort);
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(&sql)?;
        Self::map_query_rows(&mut stmt, rusqlite::params_from_iter(params.iter()))
    }

    pub fn query_system_album_count(&self, album: SystemAlbum) -> Result<usize> {
        let (sql, params) = album.synthesize_count_query();
        let conn = self.conn.lock();
        let count: i64 =
            conn.query_row(&sql, rusqlite::params_from_iter(params.iter()), |row| {
                row.get(0)
            })?;
        Ok(count as usize)
    }

    /// Re-inserts a deleted album with its original id and creation time, so
    /// undoing a delete brings back the same album (and redo can delete it
    /// again by id).
    pub fn restore_smart_album(&self, album: &SmartAlbum) -> Result<()> {
        let conn = self.conn.lock();
        let rules_json = serde_json::to_string(&album.rules)?;
        let match_str = match album.match_mode {
            MatchMode::All => "all",
            MatchMode::Any => "any",
        };
        conn.execute(
            "INSERT INTO smart_albums (id, title, icon_name, match_mode, rules_json, created_at_epoch_ms, pinned)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7);",
            params![
                album.id,
                album.title,
                album.icon_name,
                match_str,
                rules_json,
                album.created_at_epoch_ms,
                album.pinned as i64
            ],
        )?;
        Ok(())
    }

    /// Creates and persists a user-defined Smart Album.
    pub fn create_smart_album(
        &self,
        title: &str,
        icon_name: &str,
        match_mode: MatchMode,
        rules: &[SmartAlbumRule],
    ) -> Result<SmartAlbum> {
        let title = clean_text(title, MAX_NAME_CHARS);
        let conn = self.conn.lock();
        let rules_json = serde_json::to_string(rules)?;
        let match_str = match match_mode {
            MatchMode::All => "all",
            MatchMode::Any => "any",
        };
        let now_ms = Utc::now().timestamp_millis();

        let id: i64 = conn.query_row(
            r#"
            INSERT INTO smart_albums (title, icon_name, match_mode, rules_json, created_at_epoch_ms)
            VALUES (?1, ?2, ?3, ?4, ?5)
            RETURNING id;
            "#,
            params![title, icon_name, match_str, rules_json, now_ms],
            |row| row.get(0),
        )?;

        Ok(SmartAlbum {
            id,
            title: title.to_string(),
            icon_name: icon_name.to_string(),
            match_mode,
            rules: rules.to_vec(),
            created_at_epoch_ms: now_ms,
            pinned: false,
        })
    }

    pub fn get_smart_album(&self, id: i64) -> Result<Option<SmartAlbum>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            r#"
            SELECT id, title, icon_name, match_mode, rules_json, created_at_epoch_ms, pinned
            FROM smart_albums
            WHERE id = ?1;
            "#,
        )?;

        let res = stmt
            .query_row(params![id], |row| {
                let id: i64 = row.get(0)?;
                let title: String = row.get(1)?;
                let icon_name: String = row.get(2)?;
                let match_str: String = row.get(3)?;
                let rules_json: String = row.get(4)?;
                let created_at: i64 = row.get(5)?;
                let pinned: i64 = row.get(6)?;

                let match_mode = if match_str == "any" {
                    MatchMode::Any
                } else {
                    MatchMode::All
                };
                let rules = decode_rules(id, &rules_json);

                Ok(SmartAlbum {
                    id,
                    title,
                    icon_name,
                    match_mode,
                    rules,
                    created_at_epoch_ms: created_at,
                    pinned: pinned != 0,
                })
            })
            .optional()?;

        Ok(res)
    }

    pub fn list_smart_albums(&self) -> Result<Vec<SmartAlbum>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            r#"
            SELECT id, title, icon_name, match_mode, rules_json, created_at_epoch_ms, pinned
            FROM smart_albums
            ORDER BY created_at_epoch_ms ASC, id ASC;
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            let id: i64 = row.get(0)?;
            let title: String = row.get(1)?;
            let icon_name: String = row.get(2)?;
            let match_str: String = row.get(3)?;
            let rules_json: String = row.get(4)?;
            let created_at: i64 = row.get(5)?;
            let pinned: i64 = row.get(6)?;

            let match_mode = if match_str == "any" {
                MatchMode::Any
            } else {
                MatchMode::All
            };
            let rules = decode_rules(id, &rules_json);

            Ok(SmartAlbum {
                id,
                title,
                icon_name,
                match_mode,
                rules,
                created_at_epoch_ms: created_at,
                pinned: pinned != 0,
            })
        })?;

        let mut albums = Vec::new();
        for r in rows {
            albums.push(r?);
        }
        Ok(albums)
    }

    pub fn update_smart_album(
        &self,
        id: i64,
        title: &str,
        icon_name: &str,
        match_mode: MatchMode,
        rules: &[SmartAlbumRule],
    ) -> Result<bool> {
        let title = clean_text(title, MAX_NAME_CHARS);
        let conn = self.conn.lock();
        let rules_json = serde_json::to_string(rules)?;
        let match_str = match match_mode {
            MatchMode::All => "all",
            MatchMode::Any => "any",
        };
        let affected = conn.execute(
            "UPDATE smart_albums
             SET title = ?1, icon_name = ?2, match_mode = ?3, rules_json = ?4
             WHERE id = ?5;",
            params![title, icon_name, match_str, rules_json, id],
        )?;
        Ok(affected > 0)
    }

    /// Pins or unpins a smart album; false when no smart album has `id`.
    pub fn set_smart_album_pinned(&self, id: i64, pinned: bool) -> Result<bool> {
        let conn = self.conn.lock();
        let affected = conn.execute(
            "UPDATE smart_albums SET pinned = ?1 WHERE id = ?2;",
            params![pinned as i64, id],
        )?;
        Ok(affected > 0)
    }

    pub fn delete_smart_album(&self, id: i64) -> Result<bool> {
        let conn = self.conn.lock();
        let affected = conn.execute("DELETE FROM smart_albums WHERE id = ?1;", params![id])?;
        Ok(affected > 0)
    }

    pub fn query_smart_album(
        &self,
        album: &SmartAlbum,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<MediaItem>> {
        self.query_smart_album_sorted(album, limit, offset, SortSpec::default())
    }

    pub fn query_smart_album_sorted(
        &self,
        album: &SmartAlbum,
        limit: usize,
        offset: usize,
        sort: SortSpec,
    ) -> Result<Vec<MediaItem>> {
        let (sql, params) = album.synthesize_query_sorted(limit, offset, sort)?;
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(&sql)?;
        Self::map_query_rows(&mut stmt, rusqlite::params_from_iter(params.iter()))
    }

    pub fn query_smart_album_count(&self, album: &SmartAlbum) -> Result<usize> {
        let (sql, params) = album.synthesize_count_query()?;
        let conn = self.conn.lock();
        let count: i64 =
            conn.query_row(&sql, rusqlite::params_from_iter(params.iter()), |row| {
                row.get(0)
            })?;
        Ok(count as usize)
    }
}

/// Decodes a stored album's rules. An unreadable `rules_json` (say, from a
/// newer build) still yields no rules, as before, but is logged instead of
/// passing silently.
fn decode_rules(album_id: i64, rules_json: &str) -> Vec<SmartAlbumRule> {
    serde_json::from_str(rules_json).unwrap_or_else(|err| {
        log::warn!("smart album {album_id}: rules could not be read, so it has no rules: {err}");
        Vec::new()
    })
}
