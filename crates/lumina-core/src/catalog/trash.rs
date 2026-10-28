use anyhow::Result;
use rusqlite::{params, OptionalExtension};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::{Catalog, DeletionTarget};
use crate::MediaId;

impl Catalog {
    pub fn set_trashed(&self, id: MediaId, is_trashed: bool) -> Result<()> {
        let mut conn = self.conn.lock();
        conn.execute(
            "UPDATE media_items SET is_trashed = ?1 WHERE id = ?2;",
            params![is_trashed as i32, id.0 as i64],
        )?;
        self.compute_timeline_buckets_in_conn(&mut conn)?;
        Ok(())
    }

    pub fn batch_set_trashed(&self, ids: &[MediaId], is_trashed: bool) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        {
            let mut stmt =
                tx.prepare_cached("UPDATE media_items SET is_trashed = ?1 WHERE id = ?2;")?;
            let trash_val = is_trashed as i32;
            for id in ids {
                stmt.execute(params![trash_val, id.0 as i64])?;
            }
        }
        tx.commit()?;
        self.compute_timeline_buckets_in_conn(&mut conn)?;
        Ok(())
    }

    /// Restores specific trashed items back to the active library.
    pub fn restore_trashed_items(&self, ids: &[MediaId]) -> Result<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let mut count = 0;
        {
            let mut stmt = tx.prepare_cached(
                "UPDATE media_items SET is_trashed = 0 WHERE id = ?1 AND is_trashed = 1;",
            )?;
            for id in ids {
                count += stmt.execute(params![id.0 as i64])?;
            }
        }
        tx.commit()?;
        self.compute_timeline_buckets_in_conn(&mut conn)?;
        Ok(count)
    }

    /// Restores all items in the trash back to the active library.
    pub fn restore_all_trash(&self) -> Result<usize> {
        let mut conn = self.conn.lock();
        let count = conn.execute(
            "UPDATE media_items SET is_trashed = 0 WHERE is_trashed = 1;",
            [],
        )?;
        self.compute_timeline_buckets_in_conn(&mut conn)?;
        Ok(count)
    }

    /// Permanently deletes specific items from the catalog and optionally from the filesystem.
    pub fn permanently_delete_items(
        &self,
        ids: &[MediaId],
        target: DeletionTarget,
    ) -> Result<usize> {
        if ids.is_empty() {
            return Ok(0);
        }

        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let mut deleted_count = 0;

        for id in ids {
            let file_path: Option<String> = tx
                .query_row(
                    "SELECT file_path FROM media_items WHERE id = ?1;",
                    params![id.0 as i64],
                    |row| row.get(0),
                )
                .optional()?;

            if let Some(path_str) = file_path {
                let path = PathBuf::from(&path_str);
                match target {
                    DeletionTarget::CatalogOnly => {}
                    DeletionTarget::SystemTrash => {
                        if path.exists() {
                            let _ = trash::delete(&path);
                        }
                    }
                    DeletionTarget::PermanentDisk => {
                        if path.exists() {
                            let _ = std::fs::remove_file(&path);
                        }
                    }
                }
                deleted_count += tx.execute(
                    "DELETE FROM media_items WHERE id = ?1;",
                    params![id.0 as i64],
                )?;
            }
        }

        tx.commit()?;
        self.compute_timeline_buckets_in_conn(&mut conn)?;
        Ok(deleted_count)
    }

    /// Empties all trashed items (`is_trashed = 1`) from the catalog and optionally deletes them from disk.
    pub fn empty_trash(&self, target: DeletionTarget) -> Result<Vec<MediaId>> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;

        let trashed_items: Vec<(i64, String)> = {
            let mut stmt =
                tx.prepare("SELECT id, file_path FROM media_items WHERE is_trashed = 1;")?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?;
            let mut list = Vec::new();
            for r in rows {
                list.push(r?);
            }
            list
        };

        if trashed_items.is_empty() {
            return Ok(Vec::new());
        }

        for (_, path_str) in &trashed_items {
            let path = PathBuf::from(path_str);
            match target {
                DeletionTarget::CatalogOnly => {}
                DeletionTarget::SystemTrash => {
                    if path.exists() {
                        let _ = trash::delete(&path);
                    }
                }
                DeletionTarget::PermanentDisk => {
                    if path.exists() {
                        let _ = std::fs::remove_file(&path);
                    }
                }
            }
        }

        tx.execute("DELETE FROM media_items WHERE is_trashed = 1;", [])?;
        tx.commit()?;

        Ok(trashed_items
            .into_iter()
            .map(|(id, _)| MediaId(id as u64))
            .collect())
    }

    pub fn remove_item_by_path(&self, path: &Path) -> Result<Option<MediaId>> {
        let path_str = path.to_string_lossy().to_string();
        let conn = self.conn.lock();

        let id_opt: Option<i64> = conn
            .query_row(
                "SELECT id FROM media_items WHERE file_path = ?1;",
                params![path_str],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(id) = id_opt {
            conn.execute("DELETE FROM media_items WHERE id = ?1;", params![id])?;
            Ok(Some(MediaId(id as u64)))
        } else {
            Ok(None)
        }
    }

    /// Reconciles the database against a set of existing file paths on disk, removing stale entries.
    pub fn remove_items_not_in(&self, existing_paths: &HashSet<PathBuf>) -> Result<Vec<MediaId>> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;

        let all_rows: Vec<(i64, String)> = {
            let mut stmt = tx.prepare("SELECT id, file_path FROM media_items;")?;
            let mapped = stmt.query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?;
            let mut v = Vec::new();
            for r in mapped {
                v.push(r?);
            }
            v
        };

        let mut removed = Vec::new();
        {
            let mut del_stmt = tx.prepare_cached("DELETE FROM media_items WHERE id = ?1;")?;
            for (id, path_str) in all_rows {
                let p = PathBuf::from(&path_str);
                if !existing_paths.contains(&p) {
                    del_stmt.execute(params![id])?;
                    removed.push(MediaId(id as u64));
                }
            }
        }

        tx.commit()?;
        Ok(removed)
    }

    pub fn clear_all(&self) -> Result<()> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        // Deleting media_items fires the FTS delete triggers. Contentless FTS5
        // tables reject a plain DELETE, so any remaining rows are cleared with the
        // dedicated 'delete-all' command.
        tx.execute_batch(
            r#"
            DELETE FROM media_tags;
            DELETE FROM media_items;
            DELETE FROM timeline_buckets;
            "#,
        )?;
        tx.execute(
            "INSERT INTO media_search_fts(media_search_fts) VALUES('delete-all');",
            [],
        )?;
        tx.commit()?;
        Ok(())
    }
}
