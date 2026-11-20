//! Where each row's file is: its inode, and whether it has gone missing.
//!
//! A file that vanishes keeps its row and metadata, marked missing, until the
//! user removes it. A file moved or renamed inside the library keeps its
//! inode, which [`Catalog::insert_or_update_batch`] uses to reconnect it.

use anyhow::Result;
use rusqlite::{params, OptionalExtension};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::Catalog;
use crate::now_ms;
use crate::MediaId;

/// SQL condition, for a `media_items` row aliased `m`, that its file is not
/// missing.
pub(crate) const PRESENT: &str = "NOT EXISTS (SELECT 1 FROM media_file_state s \
     WHERE s.media_id = m.id AND s.missing_since_epoch_ms IS NOT NULL)";

/// A row added this recently with the inode and size of a row that just went
/// missing is the same file, seen at its new path before the old path's
/// removal arrived (the watcher times each path separately). Older pairs are
/// hard links, both present, and must not fold.
const FOLD_WINDOW_MS: i64 = 60_000;

/// What [`Catalog::mark_missing_under`] changed.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MissingChange {
    /// Rows now marked missing.
    pub missing: Vec<MediaId>,
    /// `(kept, removed)`: a row that went missing took over the path of
    /// `removed`, a row just added for the same file, which was deleted.
    pub folded: Vec<(MediaId, MediaId)>,
}

/// The file's inode, if it exists and the platform has inodes.
pub(crate) fn file_inode(path: &Path) -> Option<u64> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata(path).ok().map(|metadata| metadata.ino())
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        None
    }
}

impl Catalog {
    /// Marks every present row at `path`, or under the folder `path`, as
    /// missing, keeping its metadata. A row that goes missing while a row
    /// with its inode and size was just added elsewhere is folded with it:
    /// the file was renamed.
    pub fn mark_missing_under(&self, path: &Path) -> Result<MissingChange> {
        let exact = super::paths::encode(path);
        // Rows under the folder sort between "p/" and "p0" ('0' follows '/'),
        // a range on the unique file_path index; LIKE would need escaping.
        let lower = format!("{exact}/");
        let upper = format!("{exact}0");
        let now = now_ms();

        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let gone: Vec<(i64, i64, Option<i64>, Option<String>)> = {
            let mut stmt = tx.prepare_cached(
                r#"
                SELECT m.id, m.file_size, s.file_inode, s.volume_id
                FROM media_items m LEFT JOIN media_file_state s ON s.media_id = m.id
                WHERE (m.file_path = ?1 OR (m.file_path >= ?2 AND m.file_path < ?3))
                  AND s.missing_since_epoch_ms IS NULL;
                "#,
            )?;
            let rows = stmt.query_map(params![exact, lower, upper], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?;
            rows.collect::<rusqlite::Result<_>>()?
        };

        let mut change = MissingChange::default();
        let root = self.root_context();
        for (id, file_size, inode, gone_volume) in gone {
            let volume = gone_volume
                .as_deref()
                .or_else(|| root.as_ref().map(|root| root.volume_id.as_str()));
            Self::set_missing_in_tx(&tx, id, now, volume)?;
            let twin = match inode {
                Some(inode) => tx
                    .query_row(
                        r#"
                        SELECT m.id FROM media_items m
                        JOIN media_file_state s ON s.media_id = m.id
                        WHERE s.file_inode = ?1 AND m.file_size = ?2 AND m.id != ?3
                          AND s.missing_since_epoch_ms IS NULL
                          AND s.volume_id IS ?8
                          AND m.added_epoch_ms >= ?4
                          -- A hard link gone with the same folder is no twin.
                          AND NOT (m.file_path = ?5 OR (m.file_path >= ?6 AND m.file_path < ?7))
                        ORDER BY m.added_epoch_ms DESC LIMIT 1;
                        "#,
                        params![
                            inode,
                            file_size,
                            id,
                            now - FOLD_WINDOW_MS,
                            exact,
                            lower,
                            upper,
                            volume
                        ],
                        |row| row.get::<_, i64>(0),
                    )
                    .optional()?,
                None => None,
            };
            match twin {
                Some(twin) => {
                    Self::fold_in_tx(&tx, MediaId(id as u64), MediaId(twin as u64), root.as_ref())?;
                    change
                        .folded
                        .push((MediaId(id as u64), MediaId(twin as u64)));
                }
                None => change.missing.push(MediaId(id as u64)),
            }
        }
        if !change.folded.is_empty() {
            Self::rebuild_timeline_buckets(&tx)?;
        }
        tx.commit()?;
        Ok(change)
    }

    /// Checks every present row's file before a rescan: a file that is gone
    /// is marked missing, and a known file's unrecorded inode is filled in.
    /// Files are stat'ed with the catalog lock released. Only "not found"
    /// counts as gone; a file that cannot be read for another reason is
    /// left alone. Returns the rows newly marked missing.
    pub fn reconcile_files(&self) -> Result<Vec<MediaId>> {
        let rows: Vec<(i64, String, Option<i64>)> = {
            let conn = self.conn.lock();
            let mut stmt = conn.prepare(
                r#"
                SELECT m.id, m.file_path, s.file_inode
                FROM media_items m LEFT JOIN media_file_state s ON s.media_id = m.id
                WHERE s.missing_since_epoch_ms IS NULL;
                "#,
            )?;
            let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
            rows.collect::<rusqlite::Result<_>>()?
        };

        let mut gone = Vec::new();
        let mut inodes = Vec::new();
        for (id, path, inode) in rows {
            let file = super::paths::decode(&path);
            match std::fs::metadata(&file) {
                Ok(_) if inode.is_none() => {
                    if let Some(inode) = file_inode(&file) {
                        inodes.push((id, inode as i64));
                    }
                }
                Ok(_) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => gone.push(id),
                Err(err) => log::warn!("cannot check {}: {err}", file.display()),
            }
        }
        if gone.is_empty() && inodes.is_empty() {
            return Ok(Vec::new());
        }

        let now = now_ms();
        let root = self.root_context();
        let volume = root.as_ref().map(|root| root.volume_id.as_str());
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        for id in &gone {
            Self::set_missing_in_tx(&tx, *id, now, volume)?;
        }
        {
            let mut stmt = tx.prepare_cached(
                r#"
                INSERT INTO media_file_state (media_id, file_inode, volume_id)
                VALUES (?1, ?2, ?3)
                ON CONFLICT(media_id) DO UPDATE SET
                    file_inode = excluded.file_inode,
                    volume_id = COALESCE(excluded.volume_id, volume_id);
                "#,
            )?;
            for (id, inode) in &inodes {
                stmt.execute(params![id, inode, volume])?;
            }
        }
        tx.commit()?;
        Ok(gone.into_iter().map(|id| MediaId(id as u64)).collect())
    }

    /// Folds `other` into `keep` when both are the same file: `keep` takes
    /// `other`'s metadata where it has none, then its path, and is present
    /// again; `other` is deleted.
    pub fn fold_into(&self, keep: MediaId, other: MediaId) -> Result<()> {
        let root = self.root_context();
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        Self::fold_in_tx(&tx, keep, other, root.as_ref())?;
        Self::rebuild_timeline_buckets(&tx)?;
        tx.commit()?;
        Ok(())
    }

    fn fold_in_tx(
        tx: &rusqlite::Transaction,
        keep: MediaId,
        other: MediaId,
        root: Option<&super::roots::RootContext>,
    ) -> Result<()> {
        Self::merge_metadata_in_tx(tx, keep, &[other])?;
        // Edits the metadata merge leaves out: rotation, and an edited date
        // or place, where `keep` has none; hidden if either is.
        tx.execute(
            r#"
            UPDATE media_items SET
                rotation_quarters = CASE WHEN media_items.rotation_quarters = 0
                    THEN o.rotation_quarters ELSE media_items.rotation_quarters END,
                is_hidden = MAX(media_items.is_hidden, o.is_hidden),
                timestamp_epoch_ms = CASE WHEN media_items.date_edited = 0 AND o.date_edited = 1
                    THEN o.timestamp_epoch_ms ELSE media_items.timestamp_epoch_ms END,
                year = CASE WHEN media_items.date_edited = 0 AND o.date_edited = 1
                    THEN o.year ELSE media_items.year END,
                month = CASE WHEN media_items.date_edited = 0 AND o.date_edited = 1
                    THEN o.month ELSE media_items.month END,
                day = CASE WHEN media_items.date_edited = 0 AND o.date_edited = 1
                    THEN o.day ELSE media_items.day END,
                date_edited = MAX(media_items.date_edited, o.date_edited),
                latitude = CASE WHEN media_items.place_edited = 0 AND o.place_edited = 1
                    THEN o.latitude ELSE media_items.latitude END,
                longitude = CASE WHEN media_items.place_edited = 0 AND o.place_edited = 1
                    THEN o.longitude ELSE media_items.longitude END,
                country_code = CASE WHEN media_items.place_edited = 0 AND o.place_edited = 1
                    THEN o.country_code ELSE media_items.country_code END,
                admin1_region = CASE WHEN media_items.place_edited = 0 AND o.place_edited = 1
                    THEN o.admin1_region ELSE media_items.admin1_region END,
                city = CASE WHEN media_items.place_edited = 0 AND o.place_edited = 1
                    THEN o.city ELSE media_items.city END,
                place_edited = MAX(media_items.place_edited, o.place_edited)
            FROM media_items o
            WHERE media_items.id = ?1 AND o.id = ?2;
            "#,
            params![keep.0 as i64, other.0 as i64],
        )?;
        let (path, parent, name, inode): (String, String, String, Option<i64>) = tx.query_row(
            r#"
            SELECT m.file_path, m.parent_dir, m.file_name, s.file_inode
            FROM media_items m LEFT JOIN media_file_state s ON s.media_id = m.id
            WHERE m.id = ?1;
            "#,
            params![other.0 as i64],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
        let (root_id, rel_path, rel_parent) =
            super::roots::placement(root, &super::paths::decode(&path));
        let volume = root.map(|root| root.volume_id.as_str());
        tx.execute(
            "DELETE FROM media_items WHERE id = ?1;",
            params![other.0 as i64],
        )?;
        // After the merge, so the search row is rebuilt with merged tags.
        tx.execute(
            "UPDATE media_items SET file_path = ?1, parent_dir = ?2, file_name = ?3, \
             root_id = COALESCE(?4, root_id), rel_path = COALESCE(?5, rel_path), \
             rel_parent = COALESCE(?6, rel_parent) WHERE id = ?7;",
            params![
                path,
                parent,
                name,
                root_id,
                rel_path,
                rel_parent,
                keep.0 as i64
            ],
        )?;
        tx.execute(
            r#"
            INSERT INTO media_file_state (media_id, file_inode, volume_id, missing_since_epoch_ms)
            VALUES (?1, ?2, ?3, NULL)
            ON CONFLICT(media_id) DO UPDATE SET
                file_inode = excluded.file_inode,
                volume_id = COALESCE(excluded.volume_id, volume_id),
                missing_since_epoch_ms = NULL;
            "#,
            params![keep.0 as i64, inode, volume],
        )?;
        Ok(())
    }

    fn set_missing_in_tx(
        tx: &rusqlite::Transaction,
        id: i64,
        now: i64,
        volume: Option<&str>,
    ) -> Result<()> {
        tx.prepare_cached(
            r#"
            INSERT INTO media_file_state (media_id, volume_id, missing_since_epoch_ms)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(media_id) DO UPDATE SET
                volume_id = COALESCE(media_file_state.volume_id, excluded.volume_id),
                missing_since_epoch_ms = excluded.missing_since_epoch_ms;
            "#,
        )?
        .execute(params![id, volume, now])?;
        Ok(())
    }

    /// `(file_size, mtime_epoch_ms)` of the present rows at `paths`.
    pub fn present_signatures(&self, paths: &[PathBuf]) -> Result<HashMap<PathBuf, (u64, i64)>> {
        let conn = self.conn.lock();
        let mut found = HashMap::with_capacity(paths.len());
        // One read per 500 paths instead of a round-trip per file.
        for chunk in paths.chunks(500) {
            let marks = vec!["?"; chunk.len()].join(", ");
            let sql = format!(
                "SELECT m.file_path, m.file_size, m.mtime_epoch_ms FROM media_items m \
                 WHERE m.file_path IN ({marks}) AND {PRESENT};"
            );
            let mut stmt = conn.prepare_cached(&sql)?;
            let rows = stmt.query_map(
                rusqlite::params_from_iter(chunk.iter().map(|path| super::paths::encode(path))),
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, u64>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )?;
            for row in rows {
                let (path, file_size, mtime_epoch_ms) = row?;
                found.insert(super::paths::decode(&path), (file_size, mtime_epoch_ms));
            }
        }
        Ok(found)
    }

    /// The row whose file is at `path`, if any.
    pub fn id_at_path(&self, path: &Path) -> Result<Option<MediaId>> {
        let conn = self.conn.lock();
        let id = conn
            .query_row(
                "SELECT id FROM media_items WHERE file_path = ?1;",
                [super::paths::encode(path)],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        Ok(id.map(|id| MediaId(id as u64)))
    }

    /// The content hash stored for `id` by duplicate detection, if any.
    pub fn stored_hash(&self, id: MediaId) -> Result<Option<String>> {
        let conn = self.conn.lock();
        Ok(conn
            .query_row(
                "SELECT content_hash FROM media_hashes WHERE media_id = ?1;",
                [id.0 as i64],
                |row| row.get(0),
            )
            .optional()?)
    }

    /// Points `id` at `path` (a file no other row has) and marks it present.
    pub fn relink_path(&self, id: MediaId, path: &Path) -> Result<()> {
        let parent = path
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let root = self.root_context();
        let volume = root.as_ref().map(|root| root.volume_id.as_str());
        let (root_id, rel_path, rel_parent) = super::roots::placement(root.as_ref(), path);
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE media_items SET file_path = ?1, parent_dir = ?2, file_name = ?3, \
             root_id = COALESCE(?4, root_id), rel_path = COALESCE(?5, rel_path), \
             rel_parent = COALESCE(?6, rel_parent) WHERE id = ?7;",
            params![
                super::roots::path_string(path),
                parent,
                name,
                root_id,
                rel_path,
                rel_parent,
                id.0 as i64
            ],
        )?;
        tx.execute(
            r#"
            INSERT INTO media_file_state (media_id, volume_id, missing_since_epoch_ms)
            VALUES (?1, ?2, NULL)
            ON CONFLICT(media_id) DO UPDATE SET
                volume_id = COALESCE(excluded.volume_id, volume_id),
                missing_since_epoch_ms = NULL;
            "#,
            params![id.0 as i64, volume],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Ids of rows whose file is missing.
    pub fn get_missing_ids(&self) -> Result<HashSet<MediaId>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT media_id FROM media_file_state WHERE missing_since_epoch_ms IS NOT NULL;",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, i64>(0))?;
        let mut ids = HashSet::new();
        for row in rows {
            ids.insert(MediaId(row? as u64));
        }
        Ok(ids)
    }
}

#[cfg(test)]
pub(crate) mod tests;
