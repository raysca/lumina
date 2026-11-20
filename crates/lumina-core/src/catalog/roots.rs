//! The library root each catalog was built from.
//!
//! Media paths are stored twice: the absolute `file_path` the rest of the
//! catalog and the UI read, and a root-relative `rel_path` that identifies the
//! file independently of where its volume is mounted. A root row records the
//! volume and mount point, so a drive that comes back elsewhere can be
//! remapped instead of rescanned into a second catalog.

use anyhow::{Context, Result};
use rusqlite::{params, OptionalExtension};
use std::path::{Path, PathBuf};

use super::paths;
use super::Catalog;
use crate::volumes::{self, VolumeIdentity};

/// A row of `library_roots`, as the app and tests see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryRoot {
    pub id: i64,
    /// `uuid:<fs-uuid>`, `dev:`, `mount:` or `root:` (see [`crate::volumes`]).
    pub volume_id: String,
    pub mount_point: PathBuf,
    /// The root's path under its mount point; empty when the root is the
    /// mount, or when the platform reports no usable mount boundary (APFS
    /// firmlinks), in which case the id is non-relocatable.
    pub rel_from_volume: PathBuf,
    /// Where the root was last resolved.
    pub stored_root: PathBuf,
}

/// What an insert needs to place a file under its root, captured once per
/// batch so a scan does not re-read it per item.
#[derive(Debug, Clone)]
pub(crate) struct RootContext {
    pub(crate) id: i64,
    pub(crate) path: PathBuf,
    pub(crate) volume_id: String,
}

/// The catalog's form of a media path: `%` escaped, invalid UTF-8 bytes as
/// `%XX`, and `/`-separated on Windows so prefix queries behave there.
/// Unix keeps its own spelling, where `\` is a legal filename character.
pub(crate) fn path_string(path: &Path) -> String {
    #[cfg(windows)]
    {
        paths::encode(path).replace('\\', "/")
    }
    #[cfg(not(windows))]
    {
        paths::encode(path)
    }
}

/// `path` under `root` as a relative path in the catalog's spelling (`/` on
/// Windows, the OS's own on Unix); `None` when it is not under the root.
pub(crate) fn relative_to(root: &Path, path: &Path) -> Option<String> {
    path.strip_prefix(root)
        .ok()
        .map(path_string)
        .filter(|rel| !rel.is_empty() && !rel.starts_with("../"))
}

/// The root columns for an item at `path`: the root id only when a relative
/// path exists (a row with a root but no relative path can never be remapped
/// and would defeat the backfill's `root_id IS NULL` sweep).
pub(crate) fn placement(
    root: Option<&RootContext>,
    path: &Path,
) -> (Option<i64>, Option<String>, Option<String>) {
    let Some(root) = root else {
        return (None, None, None);
    };
    let Some(rel_path) = relative_to(&root.path, path) else {
        return (None, None, None);
    };
    let rel_parent = path
        .parent()
        .and_then(|parent| {
            parent
                .strip_prefix(&root.path)
                .ok()
                .map(|parent| parent.to_string_lossy().into_owned())
        })
        .unwrap_or_default();
    (Some(root.id), Some(rel_path), Some(rel_parent))
}

impl Catalog {
    /// Records (or refreshes) the library root and backfills rows that were
    /// written before roots existed. Called when a library is opened.
    pub fn ensure_library_root(&self, root: &Path) -> Result<LibraryRoot> {
        // The scanner walks the path the app opened, which may not be the
        // canonical one (a `/var` vs `/private/var` temp path, a symlinked
        // folder). Paths must strip against that spelling, so keep it.
        let stored = root.to_path_buf();
        let probe_path = root.canonicalize().unwrap_or_else(|_| stored.clone());
        let identity = volumes::probe(&probe_path);
        // A mount point is only a useful anchor when it is a real path prefix.
        // APFS reports its data volume as `/System/Volumes/Data` for a path
        // under `/private/var`, which is a firmlink, not a mount boundary;
        // such a root keeps a non-relocatable id instead of an empty relative
        // path that would remap to the wrong folder.
        let (volume_id, mount_point, rel_from_volume) = match identity {
            Some(identity) if probe_path.starts_with(&identity.mount_point) => {
                let rel = identity.rel_from(&probe_path);
                (identity.id, identity.mount_point, rel)
            }
            Some(identity) => (
                volumes::fallback_id(&probe_path),
                identity.mount_point,
                PathBuf::new(),
            ),
            None => (
                volumes::fallback_id(&probe_path),
                probe_path.clone(),
                PathBuf::new(),
            ),
        };
        let prefix = path_string(&stored);

        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let existing: Option<i64> = tx
            .query_row(
                "SELECT id FROM library_roots ORDER BY id LIMIT 1;",
                [],
                |row| row.get(0),
            )
            .optional()?;
        let id = match existing {
            Some(id) => {
                tx.execute(
                    "UPDATE library_roots SET volume_id = ?1, mount_point = ?2, \
                     rel_from_volume = ?3, stored_root = ?4, updated_epoch_ms = ?5 \
                     WHERE id = ?6;",
                    params![
                        volume_id,
                        path_string(&mount_point),
                        path_string(&rel_from_volume),
                        prefix,
                        crate::now_ms(),
                        id
                    ],
                )?;
                id
            }
            None => {
                tx.execute(
                    "INSERT INTO library_roots \
                     (volume_id, mount_point, rel_from_volume, stored_root, updated_epoch_ms) \
                     VALUES (?1, ?2, ?3, ?4, ?5);",
                    params![
                        volume_id,
                        path_string(&mount_point),
                        path_string(&rel_from_volume),
                        prefix,
                        crate::now_ms()
                    ],
                )?;
                tx.last_insert_rowid()
            }
        };
        Self::backfill_root_in(&tx, id, &stored)?;
        Self::set_root_volume_in(&tx, id, &volume_id)?;
        tx.commit()?;
        drop(conn);

        *self.root.lock() = Some(RootContext {
            id,
            path: stored.clone(),
            volume_id: volume_id.clone(),
        });
        Ok(LibraryRoot {
            id,
            volume_id,
            mount_point,
            rel_from_volume,
            stored_root: stored,
        })
    }

    /// The root this catalog was opened for, if it has one.
    pub fn library_root(&self) -> Result<Option<LibraryRoot>> {
        let conn = self.conn.lock();
        let root = conn
            .query_row(
                "SELECT id, volume_id, mount_point, rel_from_volume, stored_root \
                 FROM library_roots ORDER BY id LIMIT 1;",
                [],
                |row| {
                    Ok(LibraryRoot {
                        id: row.get(0)?,
                        volume_id: row.get(1)?,
                        mount_point: paths::decode(&row.get::<_, String>(2)?),
                        rel_from_volume: paths::decode(&row.get::<_, String>(3)?),
                        stored_root: paths::decode(&row.get::<_, String>(4)?),
                    })
                },
            )
            .optional()?;
        Ok(root)
    }

    pub(crate) fn root_context(&self) -> Option<RootContext> {
        self.root.lock().clone()
    }

    /// Moves the catalog to `new_root`, rewriting every absolute path in one
    /// transaction. Refuses when the new root is not the recorded volume, so a
    /// different drive with a same-named folder cannot adopt the catalog.
    pub fn remap_library_root(&self, new_root: &Path) -> Result<bool> {
        let Some(stored) = self.library_root()? else {
            return Ok(false);
        };
        let stored_new = new_root.to_path_buf();
        let probe_path = new_root
            .canonicalize()
            .unwrap_or_else(|_| stored_new.clone());
        let identity: VolumeIdentity = match volumes::probe(&probe_path) {
            Some(identity) => identity,
            None => return Ok(false),
        };
        // Enforce the volume only when the catalog recorded a real filesystem
        // UUID or device; `mount:`/`root:` are fallbacks for platforms without
        // one, and a caller that resolved the new root through them has
        // already made the same check.
        let enforce = stored.volume_id.starts_with("uuid:") || stored.volume_id.starts_with("dev:");
        if enforce && stored.volume_id != identity.id {
            return Ok(false);
        }
        let prefix = path_string(&stored_new);
        // `parent_dir` is display/search text, not a path column: it must keep
        // the readable spelling, never the `%XX` escapes.
        let display_prefix = stored_new.to_string_lossy().into_owned();
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE library_roots SET volume_id = ?1, mount_point = ?2, rel_from_volume = ?3, \
             stored_root = ?4, updated_epoch_ms = ?5 WHERE id = ?6;",
            params![
                identity.id,
                path_string(&identity.mount_point),
                path_string(&identity.rel_from(&probe_path)),
                prefix,
                crate::now_ms(),
                stored.id
            ],
        )?;
        tx.execute(
            "UPDATE media_items SET \
                 file_path = ?1 || '/' || rel_path, \
                 parent_dir = CASE WHEN rel_parent = '' THEN ?2 ELSE ?2 || '/' || rel_parent END \
             WHERE root_id = ?3 AND rel_path IS NOT NULL;",
            params![prefix, display_prefix, stored.id],
        )
        .context("rewriting media paths after a root remap")?;
        Self::set_root_volume_in(&tx, stored.id, &identity.id)?;
        tx.commit()?;
        drop(conn);

        *self.root.lock() = Some(RootContext {
            id: stored.id,
            path: stored_new,
            volume_id: identity.id,
        });
        Ok(true)
    }

    /// Gives rows written before roots existed their root and relative path.
    /// Runs once per catalog (rows already rooted are skipped), inside the
    /// caller's transaction.
    fn backfill_root_in(tx: &rusqlite::Transaction<'_>, root_id: i64, root: &Path) -> Result<()> {
        let pending: Vec<(i64, String, String)> = {
            let mut stmt = tx.prepare(
                "SELECT id, file_path, parent_dir FROM media_items WHERE root_id IS NULL;",
            )?;
            let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        let mut assign = tx.prepare(
            "UPDATE media_items SET root_id = ?1, rel_path = ?2, rel_parent = ?3 WHERE id = ?4;",
        )?;
        for (id, file_path, _parent_dir) in pending {
            let file = paths::decode(&file_path);
            let parent = file.parent().unwrap_or(Path::new(""));
            let Some(rel_path) = relative_to(root, &file) else {
                // A row outside the root (moved away, or written by an old
                // build) keeps no root: it cannot be remapped with this one.
                continue;
            };
            let rel_parent = parent
                .strip_prefix(root)
                .map(|parent| parent.to_string_lossy().into_owned())
                .unwrap_or_default();
            if let Err(err) = assign.execute(params![root_id, rel_path, rel_parent, id]) {
                // One odd row (a duplicate relative path from an old
                // separator spelling, say) must not make the whole library
                // unopenable; it stays unrooted and is skipped by a remap.
                log::warn!("catalog row {id} could not be rooted: {err}");
            }
        }
        Ok(())
    }

    /// Records `volume_id` on every item's file state, so rename folding can
    /// tell "same file moved" from "another drive's inode that happens to
    /// match". Older catalogs have NULL until their first open after this.
    fn set_root_volume_in(
        tx: &rusqlite::Transaction<'_>,
        root_id: i64,
        volume_id: &str,
    ) -> Result<()> {
        tx.prepare_cached(
            "UPDATE media_file_state SET volume_id = ?1 \
             WHERE volume_id IS NOT ?1 \
               AND media_id IN (SELECT id FROM media_items WHERE root_id = ?2);",
        )?
        .execute(params![volume_id, root_id])?;
        Ok(())
    }

    /// Up to `limit` root-relative paths, preferring rows whose file is still
    /// present, for checking that a candidate remap target holds this library.
    pub fn sample_rooted_rel_paths(&self, limit: usize) -> Result<Vec<PathBuf>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT m.rel_path FROM media_items m \
             LEFT JOIN media_file_state s ON s.media_id = m.id \
             WHERE m.root_id IS NOT NULL AND m.rel_path IS NOT NULL \
               AND s.missing_since_epoch_ms IS NULL \
             LIMIT ?1;",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(paths::decode(&row.get::<_, String>(0)?))
        })?;
        let present: Vec<PathBuf> = rows.collect::<rusqlite::Result<_>>()?;
        if !present.is_empty() {
            return Ok(present);
        }
        // Every sampled file is missing: still let a candidate be checked
        // against the stored relative paths.
        let mut stmt = conn.prepare_cached(
            "SELECT rel_path FROM media_items \
             WHERE root_id IS NOT NULL AND rel_path IS NOT NULL LIMIT ?1;",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(paths::decode(&row.get::<_, String>(0)?))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::tests_support::video_item;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "lumina_roots_{name}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn rows_written_before_roots_get_a_root_and_relative_path() {
        let base = temp_dir("backfill");
        let library = base.join("lib");
        std::fs::create_dir_all(library.join("trip")).unwrap();
        let file = library.join("trip").join("a.mp4");
        std::fs::write(&file, b"video").unwrap();
        let catalog = Catalog::open(base.join("catalog.db")).unwrap();

        // A pre-roots row: inserted before the root exists, so both columns
        // are NULL.
        let mut item = video_item(&file.to_string_lossy());
        item.file_size = 5;
        catalog.insert_or_update_batch(&[item]).unwrap();
        assert_eq!(
            catalog.root_context().map(|root| root.id),
            None,
            "setup: no root was registered yet"
        );

        catalog.ensure_library_root(&library).unwrap();
        let conn = catalog.conn.lock();
        let (root_id, rel_path, rel_parent): (i64, String, String) = conn
            .query_row(
                "SELECT root_id, rel_path, rel_parent FROM media_items;",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert!(root_id > 0, "the row is attached to the root");
        assert_eq!(rel_path, "trip/a.mp4");
        assert_eq!(rel_parent, "trip");
        drop(conn);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_row_outside_the_root_keeps_no_root() {
        let base = temp_dir("outside");
        let library = base.join("lib");
        std::fs::create_dir_all(&library).unwrap();
        let outside = base.join("elsewhere.mp4");
        std::fs::write(&outside, b"video").unwrap();
        let catalog = Catalog::open(base.join("catalog.db")).unwrap();
        catalog
            .insert_or_update_batch(&[video_item(&outside.to_string_lossy())])
            .unwrap();

        catalog.ensure_library_root(&library).unwrap();
        let conn = catalog.conn.lock();
        let root_id: Option<i64> = conn
            .query_row("SELECT root_id FROM media_items;", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            root_id, None,
            "a row outside the root cannot be remapped with it"
        );
        let _ = std::fs::remove_dir_all(&base);
    }
}
