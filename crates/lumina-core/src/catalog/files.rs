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
        let exact = path.to_string_lossy().to_string();
        // Rows under the folder sort between "p/" and "p0" ('0' follows '/'),
        // a range on the unique file_path index; LIKE would need escaping.
        let lower = format!("{exact}/");
        let upper = format!("{exact}0");
        let now = now_ms();

        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let gone: Vec<(i64, i64, Option<i64>)> = {
            let mut stmt = tx.prepare_cached(
                r#"
                SELECT m.id, m.file_size, s.file_inode
                FROM media_items m LEFT JOIN media_file_state s ON s.media_id = m.id
                WHERE (m.file_path = ?1 OR (m.file_path >= ?2 AND m.file_path < ?3))
                  AND s.missing_since_epoch_ms IS NULL;
                "#,
            )?;
            let rows = stmt.query_map(params![exact, lower, upper], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?;
            rows.collect::<rusqlite::Result<_>>()?
        };

        let mut change = MissingChange::default();
        for (id, file_size, inode) in gone {
            Self::set_missing_in_tx(&tx, id, now)?;
            let twin = match inode {
                Some(inode) => tx
                    .query_row(
                        r#"
                        SELECT m.id FROM media_items m
                        JOIN media_file_state s ON s.media_id = m.id
                        WHERE s.file_inode = ?1 AND m.file_size = ?2 AND m.id != ?3
                          AND s.missing_since_epoch_ms IS NULL
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
                            upper
                        ],
                        |row| row.get::<_, i64>(0),
                    )
                    .optional()?,
                None => None,
            };
            match twin {
                Some(twin) => {
                    Self::fold_in_tx(&tx, MediaId(id as u64), MediaId(twin as u64))?;
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
            match std::fs::metadata(&path) {
                Ok(_) if inode.is_none() => {
                    if let Some(inode) = file_inode(Path::new(&path)) {
                        inodes.push((id, inode as i64));
                    }
                }
                Ok(_) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => gone.push(id),
                Err(err) => log::warn!("cannot check {path}: {err}"),
            }
        }
        if gone.is_empty() && inodes.is_empty() {
            return Ok(Vec::new());
        }

        let now = now_ms();
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        for id in &gone {
            Self::set_missing_in_tx(&tx, *id, now)?;
        }
        {
            let mut stmt = tx.prepare_cached(
                r#"
                INSERT INTO media_file_state (media_id, file_inode) VALUES (?1, ?2)
                ON CONFLICT(media_id) DO UPDATE SET file_inode = excluded.file_inode;
                "#,
            )?;
            for (id, inode) in &inodes {
                stmt.execute(params![id, inode])?;
            }
        }
        tx.commit()?;
        Ok(gone.into_iter().map(|id| MediaId(id as u64)).collect())
    }

    /// Folds `other` into `keep` when both are the same file: `keep` takes
    /// `other`'s metadata where it has none, then its path, and is present
    /// again; `other` is deleted.
    pub fn fold_into(&self, keep: MediaId, other: MediaId) -> Result<()> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        Self::fold_in_tx(&tx, keep, other)?;
        Self::rebuild_timeline_buckets(&tx)?;
        tx.commit()?;
        Ok(())
    }

    fn fold_in_tx(tx: &rusqlite::Transaction, keep: MediaId, other: MediaId) -> Result<()> {
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
        tx.execute(
            "DELETE FROM media_items WHERE id = ?1;",
            params![other.0 as i64],
        )?;
        // After the merge, so the search row is rebuilt with merged tags.
        tx.execute(
            "UPDATE media_items SET file_path = ?1, parent_dir = ?2, file_name = ?3 WHERE id = ?4;",
            params![path, parent, name, keep.0 as i64],
        )?;
        tx.execute(
            r#"
            INSERT INTO media_file_state (media_id, file_inode, missing_since_epoch_ms)
            VALUES (?1, ?2, NULL)
            ON CONFLICT(media_id) DO UPDATE SET
                file_inode = excluded.file_inode, missing_since_epoch_ms = NULL;
            "#,
            params![keep.0 as i64, inode],
        )?;
        Ok(())
    }

    fn set_missing_in_tx(tx: &rusqlite::Transaction, id: i64, now: i64) -> Result<()> {
        tx.prepare_cached(
            r#"
            INSERT INTO media_file_state (media_id, missing_since_epoch_ms) VALUES (?1, ?2)
            ON CONFLICT(media_id) DO UPDATE SET missing_since_epoch_ms = excluded.missing_since_epoch_ms;
            "#,
        )?
        .execute(params![id, now])?;
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
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(
                rusqlite::params_from_iter(
                    chunk.iter().map(|path| path.to_string_lossy().into_owned()),
                ),
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
                found.insert(PathBuf::from(path), (file_size, mtime_epoch_ms));
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
                [path.to_string_lossy()],
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
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE media_items SET file_path = ?1, parent_dir = ?2, file_name = ?3 WHERE id = ?4;",
            params![path.to_string_lossy(), parent, name, id.0 as i64],
        )?;
        tx.execute(
            r#"
            INSERT INTO media_file_state (media_id, missing_since_epoch_ms) VALUES (?1, NULL)
            ON CONFLICT(media_id) DO UPDATE SET missing_since_epoch_ms = NULL;
            "#,
            params![id.0 as i64],
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
pub(crate) mod tests {
    use super::*;
    use crate::{MediaItem, MediaType};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// A temp library dir, removed on drop.
    pub(crate) struct TempDir(pub PathBuf);

    impl TempDir {
        pub(crate) fn new(prefix: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let dir = std::env::temp_dir().join(format!("lumina_{prefix}_{nanos}"));
            std::fs::create_dir_all(&dir).expect("temp dir must be creatable");
            // Canonical, so paths match what the OS reports (/private/var).
            Self(dir.canonicalize().expect("temp dir must canonicalize"))
        }

        /// Writes `bytes` at `rel`, creating parent folders.
        pub(crate) fn write(&self, rel: &str, bytes: &[u8]) -> PathBuf {
            let path = self.0.join(rel);
            std::fs::create_dir_all(path.parent().expect("file has a parent"))
                .expect("parent dir must be creatable");
            std::fs::write(&path, bytes).expect("file must be writable");
            path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A catalog item for an existing file at `path`.
    pub(crate) fn item_at(path: &Path) -> MediaItem {
        let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        MediaItem {
            id: MediaId(0),
            path: path.to_path_buf(),
            parent_dir: path
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default(),
            file_name: path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            title: None,
            file_size,
            mtime_epoch_ms: 1_642_200_000_000,
            media_type: MediaType::Image,
            width: 10,
            height: 10,
            duration_ms: None,
            timestamp_epoch_ms: 1_642_200_000_000,
            year: 2022,
            month: 1,
            day: 15,
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

    /// Marks `id` missing directly, for tests of what follows.
    pub(crate) fn set_missing(catalog: &Catalog, id: MediaId) {
        catalog
            .conn
            .lock()
            .execute(
                "INSERT INTO media_file_state (media_id, missing_since_epoch_ms) VALUES (?1, 1)
                 ON CONFLICT(media_id) DO UPDATE SET missing_since_epoch_ms = 1;",
                [id.0 as i64],
            )
            .expect("missing state must be writable");
    }

    fn stored_inode(catalog: &Catalog, id: MediaId) -> Option<i64> {
        catalog
            .conn
            .lock()
            .query_row(
                "SELECT file_inode FROM media_file_state WHERE media_id = ?1;",
                [id.0 as i64],
                |row| row.get(0),
            )
            .expect("state row must exist")
    }

    fn ingest(catalog: &Catalog, path: &Path) -> MediaItem {
        catalog
            .insert_or_update_batch(&[item_at(path)])
            .expect("upsert must succeed")
            .remove(0)
    }

    #[test]
    fn upsert_records_inode() {
        let dir = TempDir::new("inode");
        let path = dir.write("a.jpg", b"a");
        let catalog = Catalog::open_in_memory().unwrap();
        let item = ingest(&catalog, &path);
        assert_eq!(
            stored_inode(&catalog, item.id),
            file_inode(&path).map(|i| i as i64),
            "ingest must record the file's inode"
        );
        assert!(catalog.get_missing_ids().unwrap().is_empty());
    }

    #[test]
    fn upsert_reconnects_missing_row_by_inode() {
        let dir = TempDir::new("reconnect");
        let old = dir.write("a.jpg", b"same bytes");
        let catalog = Catalog::open_in_memory().unwrap();
        let item = ingest(&catalog, &old);
        catalog
            .add_tags_batch(&[item.id], &["kept".to_string()])
            .unwrap();
        set_missing(&catalog, item.id);

        let new = dir.0.join("sub/b.jpg");
        std::fs::create_dir_all(new.parent().unwrap()).unwrap();
        std::fs::rename(&old, &new).unwrap();
        let moved = ingest(&catalog, &new);

        assert_eq!(moved.id, item.id, "a moved file must keep its row");
        assert_eq!(catalog.total_count().unwrap(), 1, "no second row");
        let row = catalog.get_item_by_id(item.id).unwrap().unwrap();
        assert_eq!(row.path, new);
        assert_eq!(row.file_name, "b.jpg");
        assert_eq!(catalog.get_tags_for_item(item.id).unwrap(), vec!["kept"]);
        assert!(
            catalog.get_missing_ids().unwrap().is_empty(),
            "a reconnected row is present"
        );
    }

    #[test]
    fn upsert_ignores_inode_match_with_different_size() {
        let dir = TempDir::new("reconnect_size");
        let path = dir.write("a.jpg", b"short");
        let catalog = Catalog::open_in_memory().unwrap();
        let item = ingest(&catalog, &path);
        set_missing(&catalog, item.id);

        let new = dir.0.join("b.jpg");
        std::fs::rename(&path, &new).unwrap();
        std::fs::write(&new, b"a different length").unwrap();
        let other = ingest(&catalog, &new);

        assert_ne!(other.id, item.id, "same inode, new size is another file");
        assert!(catalog.get_missing_ids().unwrap().contains(&item.id));
    }

    #[test]
    fn signatures_exclude_missing_rows() {
        let dir = TempDir::new("signatures");
        let a = dir.write("a.jpg", b"a");
        let b = dir.write("b.jpg", b"b");
        let catalog = Catalog::open_in_memory().unwrap();
        let item = ingest(&catalog, &a);
        ingest(&catalog, &b);
        set_missing(&catalog, item.id);

        let signatures = catalog.get_file_signatures().unwrap();
        assert!(!signatures.contains_key(&a), "missing rows must be re-read");
        assert!(signatures.contains_key(&b));
    }

    #[test]
    fn mark_missing_under_folder_spares_siblings() {
        let dir = TempDir::new("mark_folder");
        let catalog = Catalog::open_in_memory().unwrap();
        let a = ingest(&catalog, &dir.write("dir/a.jpg", b"a")).id;
        let b = ingest(&catalog, &dir.write("dir/sub/b.jpg", b"bb")).id;
        let c = ingest(&catalog, &dir.write("dir 2/c.jpg", b"ccc")).id;
        let d = ingest(&catalog, &dir.write("dir.old/d.jpg", b"dddd")).id;
        let e = ingest(&catalog, &dir.write("dir0/e.jpg", b"eeeee")).id;

        let change = catalog.mark_missing_under(&dir.0.join("dir")).unwrap();

        let mut missing = change.missing.clone();
        missing.sort_by_key(|id| id.0);
        assert_eq!(missing, vec![a, b], "only rows under dir/ go missing");
        assert!(change.folded.is_empty());
        let all = catalog.get_missing_ids().unwrap();
        for sibling in [c, d, e] {
            assert!(!all.contains(&sibling), "sibling folders are untouched");
        }
    }

    #[test]
    fn mark_missing_keeps_row_and_metadata() {
        let dir = TempDir::new("mark_file");
        let path = dir.write("a.jpg", b"a");
        let catalog = Catalog::open_in_memory().unwrap();
        let id = ingest(&catalog, &path).id;
        catalog
            .add_tags_batch(&[id], &["kept".to_string()])
            .unwrap();
        catalog.set_rating(id, 4).unwrap();

        let change = catalog.mark_missing_under(&path).unwrap();

        assert_eq!(change.missing, vec![id]);
        assert_eq!(catalog.total_count().unwrap(), 1, "the row is kept");
        assert_eq!(catalog.get_tags_for_item(id).unwrap(), vec!["kept"]);
        assert_eq!(catalog.get_rating(id).unwrap(), 4);
        assert!(
            catalog
                .mark_missing_under(&path)
                .unwrap()
                .missing
                .is_empty(),
            "a missing row is not marked twice"
        );
    }

    #[test]
    fn split_rename_folds_new_row() {
        let dir = TempDir::new("fold");
        let old = dir.write("a.jpg", b"same");
        let catalog = Catalog::open_in_memory().unwrap();
        let a = ingest(&catalog, &old).id;
        catalog.add_tags_batch(&[a], &["kept".to_string()]).unwrap();
        // The new path is seen first: a second row with the same inode.
        let new = dir.0.join("b.jpg");
        std::fs::rename(&old, &new).unwrap();
        let b = ingest(&catalog, &new).id;
        catalog.set_rating(b, 3).unwrap();
        assert_ne!(a, b);

        let change = catalog.mark_missing_under(&old).unwrap();

        assert_eq!(
            change.folded,
            vec![(a, b)],
            "the new row folds into the old"
        );
        assert!(change.missing.is_empty());
        assert_eq!(catalog.total_count().unwrap(), 1);
        let row = catalog.get_item_by_id(a).unwrap().unwrap();
        assert_eq!(row.path, new, "the old row takes the new path");
        assert_eq!(catalog.get_tags_for_item(a).unwrap(), vec!["kept"]);
        assert_eq!(
            catalog.get_rating(a).unwrap(),
            3,
            "the new row's edits merge"
        );
        assert!(catalog.get_missing_ids().unwrap().is_empty());
    }

    #[test]
    fn fold_keeps_the_other_rows_edits() {
        let dir = TempDir::new("fold_edits");
        let catalog = Catalog::open_in_memory().unwrap();
        let keep = ingest(&catalog, &dir.write("a.jpg", b"a")).id;
        let other = ingest(&catalog, &dir.write("b.jpg", b"b")).id;
        catalog.set_rotations(&[(other, 1)]).unwrap();
        // 2001-09-09 UTC
        catalog.set_timestamp(other, 1_000_000_000_000).unwrap();
        catalog
            .set_location(other, Some(51.5), Some(-0.12))
            .unwrap();

        catalog.fold_into(keep, other).unwrap();

        assert_eq!(
            catalog.get_rotation(keep).unwrap(),
            1,
            "rotation carries over"
        );
        let row = catalog.get_item_by_id(keep).unwrap().unwrap();
        assert_eq!(
            row.timestamp_epoch_ms, 1_000_000_000_000,
            "an edited date carries over"
        );
        assert_eq!(
            (row.year, row.month, row.day),
            (2001, 9, 9),
            "the date's parts carry over with it"
        );
        assert_eq!(
            (row.latitude, row.longitude),
            (Some(51.5), Some(-0.12)),
            "an edited place carries over"
        );
        let years: Vec<String> = catalog
            .get_timeline_buckets("year")
            .unwrap()
            .into_iter()
            .map(|bucket| bucket.bucket_key)
            .collect();
        assert!(
            years.contains(&"2001".to_string()),
            "buckets follow the new date, got {years:?}"
        );
        assert_eq!(row.path, dir.0.join("b.jpg"));
        assert!(catalog.get_item_by_id(other).unwrap().is_none());

        let hidden = ingest(&catalog, &dir.write("c.jpg", b"c")).id;
        catalog
            .conn
            .lock()
            .execute(
                "UPDATE media_items SET is_hidden = 1 WHERE id = ?1;",
                [hidden.0 as i64],
            )
            .unwrap();
        catalog.fold_into(keep, hidden).unwrap();
        let row = catalog.get_item_by_id(keep).unwrap().unwrap();
        assert!(row.is_hidden, "hidden carries over");
    }

    #[test]
    fn fold_into_keeps_pick() {
        let dir = TempDir::new("fold_flags");
        let catalog = Catalog::open_in_memory().unwrap();
        let keep = ingest(&catalog, &dir.write("a.jpg", b"a")).id;
        let other = ingest(&catalog, &dir.write("b.jpg", b"b")).id;
        catalog
            .set_flags(&[(keep, crate::Flag::Reject), (other, crate::Flag::Pick)])
            .unwrap();

        catalog.fold_into(keep, other).unwrap();

        assert_eq!(
            catalog.get_flags_map().unwrap(),
            std::collections::HashMap::from([(keep, crate::Flag::Pick)]),
            "fold deletes `other`, so its pick must move to `keep` first"
        );
    }

    #[test]
    fn fold_into_carries_album_membership() {
        let dir = TempDir::new("fold_albums");
        let catalog = Catalog::open_in_memory().unwrap();
        let keep = ingest(&catalog, &dir.write("a.jpg", b"a")).id;
        let other = ingest(&catalog, &dir.write("b.jpg", b"b")).id;
        let album = catalog.create_album("Picks").unwrap();
        catalog.add_to_album(album.id, &[other]).unwrap();

        catalog.fold_into(keep, other).unwrap();

        assert!(catalog.get_item_by_id(other).unwrap().is_none());
        assert_eq!(
            crate::catalog::tests_support::album_members(&catalog, album.id),
            vec![keep],
            "fold deletes `other`, so its membership must move to `keep` first"
        );
    }

    #[test]
    fn deleted_folder_with_hard_links_marks_both_missing() {
        let dir = TempDir::new("hardlink_dir");
        let a_path = dir.write("sub/a.jpg", b"same");
        let b_path = dir.0.join("sub/b.jpg");
        std::fs::hard_link(&a_path, &b_path).unwrap();
        let catalog = Catalog::open_in_memory().unwrap();
        let a = ingest(&catalog, &a_path).id;
        let b = ingest(&catalog, &b_path).id;
        std::fs::remove_dir_all(dir.0.join("sub")).unwrap();

        let mut change = catalog.mark_missing_under(&dir.0.join("sub")).unwrap();
        change.missing.sort_by_key(|id| id.0);

        assert_eq!(
            change.missing,
            vec![a, b],
            "links gone with their folder are both missing, not folded"
        );
        assert!(change.folded.is_empty());
    }

    #[test]
    fn old_present_twin_is_not_folded() {
        let dir = TempDir::new("hardlink");
        let a_path = dir.write("a.jpg", b"same");
        let b_path = dir.0.join("b.jpg");
        std::fs::hard_link(&a_path, &b_path).unwrap();
        let catalog = Catalog::open_in_memory().unwrap();
        let a = ingest(&catalog, &a_path).id;
        let b = ingest(&catalog, &b_path).id;
        catalog
            .conn
            .lock()
            .execute(
                "UPDATE media_items SET added_epoch_ms = added_epoch_ms - 120000;",
                [],
            )
            .unwrap();
        std::fs::remove_file(&a_path).unwrap();

        let change = catalog.mark_missing_under(&a_path).unwrap();

        assert_eq!(
            change.missing,
            vec![a],
            "a long-standing hard link is not a rename"
        );
        assert!(change.folded.is_empty());
        assert!(catalog.get_item_by_id(b).unwrap().is_some());
    }

    #[test]
    fn reconcile_marks_gone_files_and_backfills_inodes() {
        let dir = TempDir::new("reconcile");
        let kept = dir.write("kept.jpg", b"k");
        let gone = dir.write("gone.jpg", b"g");
        let catalog = Catalog::open_in_memory().unwrap();
        let kept_id = ingest(&catalog, &kept).id;
        let gone_id = ingest(&catalog, &gone).id;
        // As a catalog from before v12: no file state at all.
        catalog
            .conn
            .lock()
            .execute("DELETE FROM media_file_state;", [])
            .unwrap();
        std::fs::remove_file(&gone).unwrap();

        let missing = catalog.reconcile_files().unwrap();

        assert_eq!(missing, vec![gone_id]);
        assert_eq!(
            stored_inode(&catalog, kept_id),
            file_inode(&kept).map(|i| i as i64),
            "a present file's inode is backfilled"
        );
        assert!(catalog.reconcile_files().unwrap().is_empty(), "idempotent");
    }

    #[test]
    fn missing_rows_are_not_duplicates() {
        let dir = TempDir::new("dup_missing");
        let catalog = Catalog::open_in_memory().unwrap();
        let a = ingest(&catalog, &dir.write("a.jpg", b"identical")).id;
        let b = ingest(&catalog, &dir.write("b.jpg", b"identical")).id;
        catalog
            .batch_insert_hashes(&[(a, 9, "h".into()), (b, 9, "h".into())])
            .unwrap();
        assert_eq!(catalog.query_duplicate_items_count().unwrap(), 2);

        set_missing(&catalog, b);

        assert_eq!(catalog.query_duplicate_items_count().unwrap(), 0);
        assert!(catalog.query_duplicate_groups().unwrap().is_empty());
        assert_eq!(
            catalog.find_items_by_hash("h").unwrap().len(),
            1,
            "an import never matches a missing row"
        );
        assert_eq!(catalog.find_candidates_by_size(9).unwrap().len(), 1);
    }

    #[test]
    fn v11_catalog_migrates_to_file_state() {
        let dir = TempDir::new("migration");
        let db = dir.0.join("catalog.db");
        let path = dir.write("a.jpg", b"a");
        {
            let catalog = Catalog::open(&db).unwrap();
            ingest(&catalog, &path);
            // Back to a v11 catalog: no file state table.
            let conn = catalog.conn.lock();
            conn.execute_batch("DROP TABLE media_file_state; PRAGMA user_version = 11;")
                .unwrap();
        }
        for _ in 0..2 {
            let catalog = Catalog::open(&db).unwrap();
            assert_eq!(catalog.schema_version().unwrap(), crate::SCHEMA_VERSION);
            assert_eq!(catalog.total_count().unwrap(), 1, "migration keeps rows");
            assert!(
                catalog.get_missing_ids().unwrap().is_empty(),
                "rows from before v12 count as present"
            );
        }
    }

    #[test]
    fn inode_lookups_use_the_index() {
        let catalog = Catalog::open_in_memory().unwrap();
        let plan = catalog
            .explain_query_plan(
                "SELECT m.id FROM media_items m JOIN media_file_state s ON s.media_id = m.id \
                 WHERE s.file_inode = 1 AND m.file_size = 1 AND m.id != 1 \
                 AND s.missing_since_epoch_ms IS NULL AND m.added_epoch_ms >= 0",
            )
            .unwrap()
            .join(" | ");
        assert!(
            plan.contains("idx_file_state_inode"),
            "marking a folder missing must not scan the table per file: {plan}"
        );
    }
}
