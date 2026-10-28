use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, OptionalExtension};

use super::Catalog;
use crate::search::StructuredQuery;
use crate::smart_albums::{MatchMode, SmartAlbum, SmartAlbumRule, SystemAlbum};
use crate::MediaItem;

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
        let (sql, params) = query.synthesize_query(limit, offset);
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(&sql)?;
        Self::map_query_rows(&mut stmt, rusqlite::params_from_iter(params.iter()))
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
        let (sql, params) = album.synthesize_query(limit, offset);
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

    /// Creates and persists a user-defined Smart Album.
    pub fn create_smart_album(
        &self,
        title: &str,
        icon_name: &str,
        match_mode: MatchMode,
        rules: &[SmartAlbumRule],
    ) -> Result<SmartAlbum> {
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
        })
    }

    pub fn get_smart_album(&self, id: i64) -> Result<Option<SmartAlbum>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            r#"
            SELECT id, title, icon_name, match_mode, rules_json, created_at_epoch_ms
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

                let match_mode = if match_str == "any" {
                    MatchMode::Any
                } else {
                    MatchMode::All
                };
                let rules: Vec<SmartAlbumRule> =
                    serde_json::from_str(&rules_json).unwrap_or_default();

                Ok(SmartAlbum {
                    id,
                    title,
                    icon_name,
                    match_mode,
                    rules,
                    created_at_epoch_ms: created_at,
                })
            })
            .optional()?;

        Ok(res)
    }

    pub fn list_smart_albums(&self) -> Result<Vec<SmartAlbum>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            r#"
            SELECT id, title, icon_name, match_mode, rules_json, created_at_epoch_ms
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

            let match_mode = if match_str == "any" {
                MatchMode::Any
            } else {
                MatchMode::All
            };
            let rules: Vec<SmartAlbumRule> = serde_json::from_str(&rules_json).unwrap_or_default();

            Ok(SmartAlbum {
                id,
                title,
                icon_name,
                match_mode,
                rules,
                created_at_epoch_ms: created_at,
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
        let (sql, params) = album.synthesize_query(limit, offset)?;
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
