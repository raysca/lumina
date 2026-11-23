use anyhow::Result;
use rusqlite::{params, OptionalExtension};
use std::path::{Path, PathBuf};

use super::{Catalog, DeletionTarget};
use crate::MediaId;

/// One item a permanent delete actually removed, with the identity the disk
/// cache is keyed on so its thumbnails and decodes can be evicted in the same
/// pass. The row is gone by the time this is returned, so the cache key can
/// no longer be looked up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletedItem {
    pub id: MediaId,
    pub path: PathBuf,
    pub file_size: u64,
    pub mtime_epoch_ms: i64,
}

/// What a permanent delete did: the rows that went, and the files the OS would
/// not delete. A failed file keeps its catalog row so no metadata is lost and
/// the user can retry.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DeleteOutcome {
    pub deleted: Vec<DeletedItem>,
    pub failed: Vec<PathBuf>,
}

impl DeleteOutcome {
    pub fn deleted_count(&self) -> usize {
        self.deleted.len()
    }
}

impl Catalog {
    pub fn set_trashed(&self, id: MediaId, is_trashed: bool) -> Result<()> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE media_items SET is_trashed = ?1 WHERE id = ?2;",
            params![is_trashed as i32, id.0 as i64],
        )?;
        // Same transaction: the mutation and the buckets it moves commit
        // together, so a failed rebuild can never leave stale buckets.
        Self::rebuild_timeline_buckets(&tx)?;
        tx.commit()?;
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
        Self::rebuild_timeline_buckets(&tx)?;
        tx.commit()?;
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
        Self::rebuild_timeline_buckets(&tx)?;
        tx.commit()?;
        Ok(count)
    }

    /// Restores all items in the trash back to the active library.
    pub fn restore_all_trash(&self) -> Result<usize> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let count = tx.execute(
            "UPDATE media_items SET is_trashed = 0 WHERE is_trashed = 1;",
            [],
        )?;
        Self::rebuild_timeline_buckets(&tx)?;
        tx.commit()?;
        Ok(count)
    }

    /// Permanently deletes specific items from the catalog and optionally from the filesystem.
    ///
    /// A row is only deleted once its file is gone (or `target` is
    /// `CatalogOnly`). When the OS refuses to delete a file, its row and all
    /// its metadata stay in the catalog and the path is reported in
    /// [`DeleteOutcome::failed`], so a locked or unreadable file can never
    /// silently lose the user's tags, ratings and edits.
    pub fn permanently_delete_items(
        &self,
        ids: &[MediaId],
        target: DeletionTarget,
    ) -> Result<DeleteOutcome> {
        if ids.is_empty() {
            return Ok(DeleteOutcome::default());
        }

        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let mut outcome = DeleteOutcome::default();

        for id in ids {
            let row: Option<(String, u64, i64)> = tx
                .query_row(
                    "SELECT file_path, file_size, mtime_epoch_ms FROM media_items WHERE id = ?1;",
                    params![id.0 as i64],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()?;

            let Some((path_str, file_size, mtime_epoch_ms)) = row else {
                continue;
            };
            let path = super::paths::decode(&path_str);
            match delete_from_disk(&path, target) {
                Ok(()) => {
                    tx.execute(
                        "DELETE FROM media_items WHERE id = ?1;",
                        params![id.0 as i64],
                    )?;
                    outcome.deleted.push(DeletedItem {
                        id: *id,
                        path,
                        file_size,
                        mtime_epoch_ms,
                    });
                }
                Err(err) => {
                    log::error!(
                        "kept {path:?} in the catalog: the file could not be deleted: {err:#}"
                    );
                    outcome.failed.push(path);
                }
            }
        }

        Self::rebuild_timeline_buckets(&tx)?;
        tx.commit()?;
        Ok(outcome)
    }

    /// Empties all trashed items (`is_trashed = 1`) from the catalog and optionally deletes them from disk.
    ///
    /// A trashed row whose file the OS refuses to delete stays trashed and is
    /// reported in [`DeleteOutcome::failed`], so it can be retried rather than
    /// silently losing the file while its metadata is purged.
    pub fn empty_trash(&self, target: DeletionTarget) -> Result<DeleteOutcome> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;

        let trashed_items: Vec<(i64, String, u64, i64)> = {
            let mut stmt = tx.prepare(
                "SELECT id, file_path, file_size, mtime_epoch_ms FROM media_items WHERE is_trashed = 1;",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };

        let mut outcome = DeleteOutcome::default();
        {
            let mut delete_stmt = tx.prepare_cached("DELETE FROM media_items WHERE id = ?1;")?;
            for (id, path_str, file_size, mtime_epoch_ms) in trashed_items {
                let path = super::paths::decode(&path_str);
                match delete_from_disk(&path, target) {
                    Ok(()) => {
                        delete_stmt.execute(params![id])?;
                        outcome.deleted.push(DeletedItem {
                            id: MediaId(id as u64),
                            path,
                            file_size,
                            mtime_epoch_ms,
                        });
                    }
                    Err(err) => {
                        // Still trashed: the row stays so the item and its
                        // metadata can be retried, never purged behind a file
                        // that is still on disk.
                        log::error!(
                            "kept trashed item {path:?}: the file could not be deleted: {err:#}"
                        );
                        outcome.failed.push(path);
                    }
                }
            }
        }
        // No bucket rebuild: trashed rows are already excluded from every
        // bucket, so removing them changes no bucket count.
        tx.commit()?;

        Ok(outcome)
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
///
/// Returns the media file's deletion error (not a sidecar's): a caller that
/// cannot remove the file must keep its catalog row and metadata.
fn delete_from_disk(path: &Path, target: DeletionTarget) -> anyhow::Result<()> {
    if target == DeletionTarget::CatalogOnly {
        return Ok(());
    }
    let delete = |file: &Path| -> anyhow::Result<()> {
        // Only a definite "not found" counts as gone: `exists()` is also
        // false on a permission error, which must not let the row or sidecar go.
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
    delete(path)?;
    let sidecar = crate::xmp::sidecar_path(path);
    if let Err(err) = delete(&sidecar) {
        // The media is gone; a sidecar failure must not keep the row.
        log::warn!("could not delete {sidecar:?}: {err:#}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MediaItem, MediaType};

    fn temp_dir(prefix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "lumina-trash-{prefix}-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn item(path: &Path) -> MediaItem {
        MediaItem {
            id: MediaId(0),
            path: path.to_path_buf(),
            parent_dir: path.parent().unwrap().to_string_lossy().into_owned(),
            file_name: path.file_name().unwrap().to_string_lossy().into_owned(),
            title: None,
            file_size: 2048,
            mtime_epoch_ms: 1_000,
            media_type: MediaType::Image,
            width: 1920,
            height: 1080,
            duration_ms: None,
            timestamp_epoch_ms: 1_000,
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
            is_hidden: false,
            is_trashed: false,
        }
    }

    #[test]
    fn a_failed_media_delete_keeps_the_sidecar() {
        let dir = temp_dir("keep-sidecar");
        // remove_file fails on a directory: stands in for a locked file.
        let media = dir.join("photo.jpg");
        std::fs::create_dir_all(&media).unwrap();
        let sidecar = crate::xmp::sidecar_path(&media);
        std::fs::write(&sidecar, b"<x/>").unwrap();

        delete_from_disk(&media, DeletionTarget::PermanentDisk)
            .expect_err("an undeletable media file must be reported, not swallowed");
        assert!(media.exists(), "the undeletable media is still there");
        assert!(
            sidecar.exists(),
            "a sidecar must survive when its media could not be deleted"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_percent_in_a_filename_is_deleted_from_disk_not_treated_as_gone() {
        let dir = temp_dir("percent");
        let path = dir.join("100%41.jpg");
        std::fs::write(&path, b"photo").unwrap();
        let catalog = Catalog::open_in_memory().unwrap();
        let stored = catalog
            .insert_or_update_batch(&[item(&path)])
            .unwrap()
            .remove(0);

        let outcome = catalog
            .permanently_delete_items(&[stored.id], crate::DeletionTarget::PermanentDisk)
            .unwrap();
        assert_eq!(outcome.deleted.len(), 1);
        assert!(
            !path.exists(),
            "the file whose name contains the percent is the one deleted"
        );
        assert_eq!(catalog.total_count().unwrap(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Regression: a failed OS delete used to remove the catalog row anyway,
    /// silently discarding every tag, rating and edit while the file stayed
    /// on disk (and was re-imported as a blank item by the next scan).
    #[test]
    fn a_failed_disk_delete_keeps_the_row_metadata_and_reports_it() {
        let dir = temp_dir("keep-row");
        let media = dir.join("locked.jpg");
        // A directory where a file is expected makes remove_file fail.
        std::fs::create_dir_all(&media).unwrap();

        let catalog = Catalog::open_in_memory().unwrap();
        let inserted = catalog.insert_or_update_batch(&[item(&media)]).unwrap();
        let id = inserted[0].id;
        catalog
            .add_tags_batch(&[id], &["keep".to_string()])
            .unwrap();

        let outcome = catalog
            .permanently_delete_items(&[id], DeletionTarget::PermanentDisk)
            .unwrap();

        assert_eq!(
            outcome.deleted_count(),
            0,
            "an undeletable file must not count as deleted"
        );
        assert_eq!(outcome.failed, vec![media]);
        assert!(
            catalog.get_item_by_id(id).unwrap().is_some(),
            "the catalog row and its metadata must survive a failed disk delete"
        );
        assert_eq!(
            catalog.get_tags_for_item(id).unwrap(),
            vec!["keep".to_string()],
            "the item's tags must not be lost when the file cannot be deleted"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Empty Trash must keep a trashed row whose file the OS refuses to
    /// delete, so it can be retried instead of purging its metadata.
    #[test]
    fn a_failed_disk_delete_keeps_the_trashed_row_and_reports_it() {
        let dir = temp_dir("empty-keep");
        let media = dir.join("locked.jpg");
        std::fs::create_dir_all(&media).unwrap();

        let catalog = Catalog::open_in_memory().unwrap();
        let inserted = catalog.insert_or_update_batch(&[item(&media)]).unwrap();
        let id = inserted[0].id;
        catalog.set_trashed(id, true).unwrap();

        let outcome = catalog.empty_trash(DeletionTarget::PermanentDisk).unwrap();

        assert_eq!(
            outcome.deleted_count(),
            0,
            "an undeletable file must not count as deleted"
        );
        assert_eq!(outcome.failed, vec![media]);
        let kept = catalog
            .get_item_by_id(id)
            .unwrap()
            .expect("the trashed row must survive a failed disk delete");
        assert!(kept.is_trashed, "the item stays in the Trash for a retry");
        assert_eq!(
            catalog
                .query_system_album_count(crate::SystemAlbum::RecentlyDeleted)
                .unwrap(),
            1,
            "the item is still in the Trash, not purged"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
