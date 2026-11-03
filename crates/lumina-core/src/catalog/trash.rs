use anyhow::Result;
use rusqlite::{params, OptionalExtension};
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
                delete_from_disk(&path, target);
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
            delete_from_disk(&path, target);
        }

        tx.execute("DELETE FROM media_items WHERE is_trashed = 1;", [])?;
        tx.commit()?;

        Ok(trashed_items
            .into_iter()
            .map(|(id, _)| MediaId(id as u64))
            .collect())
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

/// Deletes `path` per `target`, and its XMP sidecar with it: sidecars are
/// hidden in Finder, so one left behind would be invisible clutter. The
/// sidecar goes only once the media is gone; if the media can't be deleted,
/// the sidecar may be the last copy of its metadata.
fn delete_from_disk(path: &Path, target: DeletionTarget) {
    if target == DeletionTarget::CatalogOnly {
        return;
    }
    let delete = |file: &Path| -> anyhow::Result<()> {
        // Only a definite "not found" counts as gone: `exists()` is also
        // false on a permission error, which must not let the sidecar go.
        match std::fs::symlink_metadata(file) {
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(err.into()),
            Ok(_) => {}
        }
        if target == DeletionTarget::SystemTrash {
            trash::delete(file)?;
        } else {
            std::fs::remove_file(file)?;
        }
        Ok(())
    };
    if let Err(err) = delete(path) {
        log::warn!("could not delete {path:?}: {err:#}");
        return;
    }
    let sidecar = crate::xmp::sidecar_path(path);
    if let Err(err) = delete(&sidecar) {
        log::warn!("could not delete {sidecar:?}: {err:#}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_media_delete_keeps_the_sidecar() {
        let dir = std::env::temp_dir().join(format!(
            "lumina-trash-keep-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
        ));
        // remove_file fails on a directory: stands in for a locked file.
        let media = dir.join("photo.jpg");
        std::fs::create_dir_all(&media).unwrap();
        let sidecar = crate::xmp::sidecar_path(&media);
        std::fs::write(&sidecar, b"<x/>").unwrap();

        delete_from_disk(&media, DeletionTarget::PermanentDisk);
        assert!(media.exists(), "the undeletable media is still there");
        assert!(
            sidecar.exists(),
            "a sidecar must survive when its media could not be deleted"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
