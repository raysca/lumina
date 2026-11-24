use anyhow::{Context, Result};
use rusqlite::{params, OptionalExtension};

use super::Catalog;
use crate::now_ms;
use crate::smart_albums::{MatchMode, SmartAlbum, SmartAlbumRule, SystemAlbum};
use crate::text::{clean_text, MAX_NAME_CHARS};
use crate::{MediaItem, SortSpec};

impl Catalog {
    /// Queries a built-in System Smart Album (Favorites, Videos, RAW, Screenshots, Places, etc.).
    /// Test-support entry point (integration tests build catalogs directly).
    #[doc(hidden)]
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
        let conn = self.conn.read();
        let mut stmt = conn.prepare_cached(&sql)?;
        Self::map_query_rows(&mut stmt, rusqlite::params_from_iter(params.iter()))
    }

    pub fn query_system_album_count(&self, album: SystemAlbum) -> Result<usize> {
        let (sql, params) = album.synthesize_count_query();
        let conn = self.conn.read();
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
        let now_ms = now_ms();

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
            title,
            icon_name: icon_name.to_string(),
            match_mode,
            rules: rules.to_vec(),
            created_at_epoch_ms: now_ms,
            pinned: false,
        })
    }

    /// Test-support entry point.
    #[doc(hidden)]
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

        Ok(rows.collect::<rusqlite::Result<_>>()?)
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
        let conn = self.conn.read();
        let mut stmt = conn.prepare_cached(&sql)?;
        Self::map_query_rows(&mut stmt, rusqlite::params_from_iter(params.iter()))
    }

    pub fn query_smart_album_count(&self, album: &SmartAlbum) -> Result<usize> {
        let (sql, params) = album.synthesize_count_query()?;
        let conn = self.conn.read();
        let count: i64 =
            conn.query_row(&sql, rusqlite::params_from_iter(params.iter()), |row| {
                row.get(0)
            })?;
        Ok(count as usize)
    }

    /// Every camera in the library as stored `(make, model)`, "" for a
    /// missing part, for the smart album "Camera is" list. Spellings are
    /// kept apart ("Canon" / "canon"): the builder groups them under one
    /// name and the rule matches each stored pair exactly.
    /// Trashed and hidden items count, so the list does not shift as items
    /// move in and out of Trash.
    pub fn distinct_cameras(&self) -> Result<Vec<(String, String)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(DISTINCT_CAMERAS_SQL)?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("list cameras")
    }
}

/// Served by `idx_media_camera (make, model)` as a covering index: one pass
/// over the index, no table rows and no temp b-tree for DISTINCT or ORDER BY.
/// `trim` drops a camera of only spaces, which would be a blank option.
const DISTINCT_CAMERAS_SQL: &str = "SELECT DISTINCT make, model FROM media_items \
     WHERE trim(make) <> '' OR trim(model) <> '' ORDER BY make, model";

/// Decodes a stored album's rules. An unreadable `rules_json` (say, from a
/// newer build) still yields no rules, as before, but is logged instead of
/// passing silently.
fn decode_rules(album_id: i64, rules_json: &str) -> Vec<SmartAlbumRule> {
    serde_json::from_str(rules_json).unwrap_or_else(|err| {
        log::warn!("smart album {album_id}: rules could not be read, so it has no rules: {err}");
        Vec::new()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinct_cameras_reads_only_the_camera_index() {
        let catalog = Catalog::open_in_memory().expect("open catalog");
        let plan = catalog
            .explain_query_plan(DISTINCT_CAMERAS_SQL)
            .expect("plan")
            .join(" | ");
        assert!(
            plan.contains("USING COVERING INDEX idx_media_camera"),
            "the camera list reads the (make, model) index, not the table: {plan}"
        );
        assert!(
            !plan.contains("TEMP B-TREE"),
            "the index already orders and de-duplicates the pairs: {plan}"
        );
    }
}
