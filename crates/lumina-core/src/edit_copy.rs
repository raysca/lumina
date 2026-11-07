//! Edit a Copy: duplicates a media file next to the original for an external
//! editor, so the editor's save never touches the original.

use anyhow::{bail, Context, Result};
use std::fs::{self, File, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::scanner::file_signature;
use crate::{LuminaEngine, MediaId, MediaItem};

impl LuminaEngine {
    /// Copies the file of `id` to `name (edited).ext` in the same folder and
    /// catalogues it with the original's user metadata (tags, caption,
    /// favorite, rating, label, title, rights). The copy has no Lumina edits
    /// and no rotation. Blocking file IO: call it off the UI thread.
    ///
    /// The copy is created with `create_new`, so a file that appears under
    /// the chosen name in the meantime is never overwritten, and a name that
    /// a catalog row still holds (a file deleted in Finder, a trashed item) is
    /// skipped, so the copy never takes over that row. If the catalog write
    /// fails, the copied file is removed again.
    pub fn duplicate_for_edit(&self, id: MediaId) -> Result<MediaItem> {
        let original = self
            .catalog
            .get_item_by_id(id)?
            .with_context(|| format!("item {} is not in the catalog", id.0))?;
        let destination = copy_beside(&original.path, |path| self.catalog.has_path(path))?;
        let result = self.catalog_copy(&original, &destination);
        if result.is_err() {
            if let Err(err) = fs::remove_file(&destination) {
                log::warn!("could not remove the copy {destination:?}: {err:?}");
            }
        }
        result
    }

    fn catalog_copy(&self, original: &MediaItem, destination: &Path) -> Result<MediaItem> {
        let (file_size, mtime_epoch_ms) =
            file_signature(destination).context("could not read the copied file")?;
        let copy = MediaItem {
            id: MediaId(0),
            path: destination.to_path_buf(),
            file_name: destination
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default(),
            file_size,
            mtime_epoch_ms,
            is_trashed: false,
            ..original.clone()
        };
        self.catalog
            .insert_edit_copy(&copy, original.id)
            .context("could not add the copy to the catalog")
    }
}

/// Copies `source` to the first `stem (edited).ext`, `stem (edited) (1).ext`…
/// in its folder that is neither a file nor `taken` (in the catalog). The
/// destination is opened with `create_new`; a name created in between is
/// skipped. A partial copy is removed.
fn copy_beside(source: &Path, taken: impl Fn(&Path) -> Result<bool>) -> Result<PathBuf> {
    let directory = source.parent().context("the original has no folder")?;
    let stem = source
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let ext = source
        .extension()
        .map(|ext| format!(".{}", ext.to_string_lossy()))
        .unwrap_or_default();
    let mut reader = File::open(source).context("could not read the original")?;
    for index in 0..10_000 {
        let name = match index {
            0 => format!("{stem} (edited){ext}"),
            n => format!("{stem} (edited) ({n}){ext}"),
        };
        let destination = directory.join(name);
        if destination.exists() || taken(&destination)? {
            continue;
        }
        let mut writer = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
        {
            Ok(file) => file,
            Err(err) if err.kind() == ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err).context("could not create the copy"),
        };
        let copied = std::io::copy(&mut reader, &mut writer).and_then(|_| writer.sync_all());
        if let Err(err) = copied {
            drop(writer);
            if let Err(remove) = fs::remove_file(&destination) {
                log::warn!("could not remove the partial copy {destination:?}: {remove:?}");
            }
            return Err(err).context("could not copy the file");
        }
        return Ok(destination);
    }
    bail!("could not find a free name for the copy")
}

#[cfg(test)]
mod tests {
    use crate::catalog::{CropRect, Edits, Rights};
    use crate::{EngineEvent, LuminaEngine, MediaId, MediaItem};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn library(prefix: &str) -> (PathBuf, Arc<LuminaEngine>, MediaId) {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("lumina_edit_copy_{prefix}_{nanos}"));
        fs::create_dir_all(&dir).expect("temp library dir must be creatable");
        fs::write(dir.join("photo.jpg"), b"test-jpeg-content").expect("fixture writes");
        let (engine, rx) = LuminaEngine::open_with_paths(
            dir.clone(),
            dir.join("catalog.db"),
            Some(dir.join("cache")),
        )
        .expect("engine opens");
        while let Ok(event) = rx.recv_timeout(Duration::from_secs(60)) {
            if let EngineEvent::ScanComplete { .. } = event {
                break;
            }
        }
        let id = engine.get_item_slice(0..1)[0].id;
        (dir, engine, id)
    }

    fn edits() -> Edits {
        Edits {
            rotation: 1,
            crop: Some(CropRect {
                x: 0.1,
                y: 0.1,
                w: 0.5,
                h: 0.5,
            }),
            exposure_ev: 0.5,
            auto_enhance: true,
        }
    }

    #[test]
    fn duplicate_for_edit_copies_file_and_metadata() {
        let (dir, engine, id) = library("copies");
        engine.add_tags_batch(&[id], &["beach".into()]).unwrap();
        engine.set_caption(id, "sunset").unwrap();
        engine.toggle_favorite(id, true).unwrap();
        engine.set_ratings(&[(id, 4)]).unwrap();
        let rights = Rights {
            creator: Some("Ada".into()),
            ..Default::default()
        };
        engine.catalog.set_rights(&[(id, rights.clone())]).unwrap();
        engine.set_edits(&[(id, edits())]).unwrap();
        let original = engine.get_item_by_id(id).unwrap();

        let copy = engine.duplicate_for_edit(id).expect("the copy succeeds");

        assert_ne!(copy.id, id, "the copy is a new catalog item");
        assert_eq!(copy.path, dir.join("photo (edited).jpg"));
        assert_eq!(
            fs::read(&copy.path).unwrap(),
            fs::read(&original.path).unwrap(),
            "the copy has the original's bytes"
        );
        assert_eq!(engine.get_tags_for_item(copy.id), vec!["beach".to_string()]);
        assert_eq!(
            engine.get_captions_map().get(&copy.id).map(String::as_str),
            Some("sunset")
        );
        assert_eq!(engine.get_ratings_map().get(&copy.id), Some(&4));
        assert!(engine.get_item_by_id(copy.id).unwrap().is_favorite);
        assert_eq!(
            engine.catalog.get_rights(&[copy.id]).unwrap().get(&copy.id),
            Some(&rights),
            "rights are copied"
        );
        let copy_edits = engine.get_edits(&[copy.id]).unwrap()[&copy.id];
        assert_eq!(
            copy_edits,
            Edits::default(),
            "the copy starts with no Lumina edits and rotation 0 (Decision 13)"
        );
        let edit_rows: i64 = engine
            .catalog
            .conn
            .lock()
            .query_row(
                "SELECT COUNT(*) FROM media_edits WHERE media_id = ?1",
                [copy.id.0 as i64],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(edit_rows, 0, "the copy has no media_edits row");
        assert_eq!(
            engine.get_edits(&[id]).unwrap()[&id],
            edits(),
            "the original keeps its edits"
        );
        assert_eq!(
            engine.get_item_by_id(id).unwrap().path,
            original.path,
            "the original row is untouched"
        );
        drop(engine);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn duplicate_for_edit_never_overwrites_existing_name() {
        let (dir, engine, id) = library("no_overwrite");
        let taken = dir.join("photo (edited).jpg");
        fs::write(&taken, b"someone else's file").unwrap();

        let copy = engine.duplicate_for_edit(id).expect("the copy succeeds");

        assert_eq!(copy.path, dir.join("photo (edited) (1).jpg"));
        assert_eq!(
            fs::read(&taken).unwrap(),
            b"someone else's file",
            "an existing file is never overwritten"
        );
        drop(engine);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn duplicate_for_edit_never_reuses_a_catalog_row() {
        let (dir, engine, id) = library("stale_rows");
        let original = engine.get_item_by_id(id).unwrap();
        // Rows whose files are gone (deleted in Finder; one also trashed)
        // still hold the names a copy would take.
        let stale = |name: &str| MediaItem {
            path: dir.join(name),
            file_name: name.to_string(),
            ..original.clone()
        };
        let rows = engine
            .catalog
            .insert_or_update_batch(&[stale("photo (edited).jpg"), stale("photo (edited) (1).jpg")])
            .unwrap();
        let (gone, trashed) = (rows[0].id, rows[1].id);
        engine.add_tags_batch(&[gone], &["old".into()]).unwrap();
        engine.set_ratings(&[(gone, 2)]).unwrap();
        engine
            .catalog
            .conn
            .lock()
            .execute(
                "UPDATE media_items SET is_trashed = 1 WHERE id = ?1",
                [trashed.0 as i64],
            )
            .unwrap();
        let before = (
            engine.get_item_by_id(gone).unwrap(),
            engine.get_item_by_id(trashed).unwrap(),
        );

        let copy = engine.duplicate_for_edit(id).expect("the copy succeeds");

        assert!(
            copy.id != gone && copy.id != trashed,
            "the copy is a new row, never an old one"
        );
        assert_eq!(copy.path, dir.join("photo (edited) (2).jpg"));
        let row = engine.get_item_by_id(copy.id).unwrap();
        assert!(!row.is_trashed, "the copy is not in Recently Deleted");
        assert!(
            engine.get_tags_for_item(copy.id).is_empty(),
            "only the original's metadata: no old tags"
        );
        assert_eq!(
            engine.get_ratings_map().get(&copy.id),
            None,
            "no old rating"
        );
        assert_eq!(
            (
                engine.get_item_by_id(gone).unwrap(),
                engine.get_item_by_id(trashed).unwrap()
            ),
            before,
            "the old rows are untouched"
        );
        assert_eq!(engine.get_tags_for_item(gone), vec!["old".to_string()]);
        assert_eq!(engine.get_ratings_map().get(&gone), Some(&2));
        // The same rule holds inside the catalog transaction, for a name
        // that a trashed row took after the name was chosen.
        assert!(
            engine
                .catalog
                .insert_edit_copy(&stale("photo (edited) (1).jpg"), id)
                .is_err(),
            "a trashed row is never taken over"
        );
        assert_eq!(engine.get_item_by_id(trashed).unwrap(), before.1);
        drop(engine);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn duplicate_for_edit_catalog_failure_removes_copy() {
        let (dir, engine, id) = library("catalog_failure");
        engine.add_tags_batch(&[id], &["beach".into()]).unwrap();
        // Fires inside the copy's transaction (its tags are copied with the
        // metadata), after the file and its row exist.
        engine
            .catalog
            .conn
            .lock()
            .execute_batch(
                "CREATE TRIGGER fail_copy BEFORE INSERT ON media_tags
                 BEGIN SELECT RAISE(ABORT, 'injected'); END;",
            )
            .unwrap();
        let before = engine.total_count();

        assert!(engine.duplicate_for_edit(id).is_err());

        assert!(
            !dir.join("photo (edited).jpg").exists(),
            "a failed catalog write removes the copied file"
        );
        assert_eq!(engine.total_count(), before, "no row is left behind");
        drop(engine);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn duplicate_for_edit_rejects_a_missing_original() {
        let (dir, engine, id) = library("missing");
        fs::remove_file(dir.join("photo.jpg")).unwrap();

        assert!(engine.duplicate_for_edit(id).is_err());
        assert!(!dir.join("photo (edited).jpg").exists());
        drop(engine);
        let _ = fs::remove_dir_all(&dir);
    }
}
