//! Where a library's catalog lives, moving a catalog whose volume came back
//! under a new mount point, and removing a library's own data.
//!
//! None of this touches the user's media: deleting a library removes the
//! catalog and its cached decodes, and the folder stays where it is.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::cache::DiskCache;
use crate::catalog::Catalog;
use crate::volumes;

/// The directory the app keeps one catalog file per library in.
pub fn default_catalog_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("lumina")
}

/// The catalog file for `library_root`, in `catalog_dir`.
///
/// The name is a hash of the canonical root, so opening the same folder twice
/// reuses its catalog and a different folder never mixes into it.
pub fn catalog_path_in(catalog_dir: &Path, library_root: &Path) -> PathBuf {
    let canonical = library_root
        .canonicalize()
        .unwrap_or_else(|_| library_root.to_path_buf());
    let hash = xxhash_rust::xxh3::xxh3_64(canonical.to_string_lossy().as_bytes());
    catalog_dir.join(format!("catalog_{hash:016x}.db"))
}

/// [`catalog_path_in`] with the app's own catalog directory.
pub fn catalog_path_for(library_root: &Path) -> PathBuf {
    catalog_path_in(&default_catalog_dir(), library_root)
}

/// What removing a library deleted. The library folder and its files are
/// never included.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryRemoval {
    pub removed_catalog: bool,
    pub thumbnails_removed: usize,
}

/// Removes a library's catalog and its cached thumbnails, leaving every photo
/// and video where it is. `library_root` is only used to find the catalog.
///
/// A corrupt or unreadable catalog must not block removing the library, which
/// is exactly when a user needs it: signatures are skipped with a log line.
pub fn remove_library(library_root: &Path) -> Result<LibraryRemoval> {
    remove_library_at(&catalog_path_for(library_root), &DiskCache::default_dir())
}

/// [`remove_library`] with explicit paths, for tests and for the app after it
/// has closed a library (where the open engine's own catalog path is used).
pub fn remove_library_at(catalog_db: &Path, cache_dir: &Path) -> Result<LibraryRemoval> {
    // Read the item signatures first: the cache keys them by path, size and
    // mtime, and the catalog is about to go away.
    let signatures = if catalog_db.exists() {
        match Catalog::open(catalog_db) {
            Ok(catalog) => match catalog.all_file_signatures() {
                Ok(signatures) => signatures,
                Err(err) => {
                    log::warn!(
                        "could not read {} for thumbnail eviction: {err:#}",
                        catalog_db.display()
                    );
                    Vec::new()
                }
            },
            Err(err) => {
                log::warn!(
                    "could not open {} for thumbnail eviction: {err:#}",
                    catalog_db.display()
                );
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };
    let thumbnails_removed = if cache_dir.is_dir() {
        match DiskCache::with_directory(cache_dir) {
            Ok(cache) => cache.remove_cached_items_counted(
                signatures
                    .iter()
                    .map(|(path, size, mtime)| (path.as_path(), *size, *mtime)),
            ),
            Err(err) => {
                log::warn!(
                    "could not read the thumbnail cache at {}: {err:#}",
                    cache_dir.display()
                );
                0
            }
        }
    } else {
        0
    };

    let mut removed_catalog = false;
    // WAL and shared-memory companions, and the daily backup.
    for (suffix, is_catalog) in [
        ("", true),
        ("-wal", false),
        ("-shm", false),
        (".bak", false),
    ] {
        let path = PathBuf::from(format!("{}{suffix}", catalog_db.display()));
        match std::fs::remove_file(&path) {
            Ok(()) => removed_catalog |= is_catalog,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) if is_catalog => {
                return Err(err).with_context(|| format!("removing {}", path.display()))
            }
            // The catalog is gone; a companion left behind is stale and
            // harmless, so it must not turn a successful removal into an error.
            Err(err) => log::warn!("could not remove {}: {err:#}", path.display()),
        }
    }
    Ok(LibraryRemoval {
        removed_catalog,
        thumbnails_removed,
    })
}

/// A library found on its volume: the catalog has been remapped in memory to
/// the new root, and [`Self::commit`] moves it to the new root's catalog path
/// so opening that root reuses it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryRelocation {
    pub library_root: PathBuf,
    pub source_db: PathBuf,
    pub target_db: PathBuf,
}

impl LibraryRelocation {
    /// Moves the catalog and its SQLite companions to the new root's path.
    /// Only the rename happens here, so a caller can drop a superseded result
    /// before it changes the filesystem layout.
    pub fn commit(&self) -> Result<()> {
        if self.target_db != self.source_db && !self.target_db.exists() {
            move_catalog_files(&self.source_db, &self.target_db)?;
        }
        Ok(())
    }
}

/// Where the same library lives now, when its folder is gone but its volume is
/// mounted somewhere else (an external drive that came back under a different
/// mount point, or a macOS volume mounted as `Photos 1`).
pub fn find_library_relocation(library_root: &Path) -> Result<Option<LibraryRelocation>> {
    find_library_relocation_at(&catalog_path_for(library_root), library_root, None)
}

/// [`find_library_relocation`] with explicit paths, for tests: `target_db` is
/// where the catalog will move when the result is committed.
pub fn find_library_relocation_at(
    catalog_db: &Path,
    library_root: &Path,
    target_db: Option<&Path>,
) -> Result<Option<LibraryRelocation>> {
    if library_root.is_dir() || !catalog_db.exists() {
        return Ok(None);
    }
    let catalog = Catalog::open(catalog_db)?;
    let Some(root) = catalog.library_root()? else {
        return Ok(None);
    };
    // A saved device number can be reused by another disk. Before rewriting
    // anything, require that a candidate actually holds the library's files.
    let sample = catalog.sample_rooted_rel_paths(64)?;
    for mount in volumes::find_mounts(&root.volume_id) {
        let candidate = mount.join(&root.rel_from_volume);
        if !candidate.is_dir() {
            continue;
        }
        if !sample.is_empty() && !sample.iter().any(|rel| candidate.join(rel).exists()) {
            log::warn!(
                "{} (volume {}) does not hold this library; not remapping",
                candidate.display(),
                root.volume_id
            );
            continue;
        }
        let target = target_db
            .map(Path::to_path_buf)
            .unwrap_or_else(|| catalog_path_for(&candidate));
        if target != catalog_db && target.exists() {
            // The new folder was opened once already and has its own catalog;
            // keep that one rather than rewriting the old tree over it.
            log::warn!(
                "a catalog already exists for {}; leaving the old one in place",
                candidate.display()
            );
            return Ok(None);
        }
        if !catalog.remap_library_root(&candidate)? {
            continue;
        }
        log::info!(
            "library at {} was found at {} (volume {})",
            library_root.display(),
            candidate.display(),
            root.volume_id
        );
        return Ok(Some(LibraryRelocation {
            library_root: candidate,
            source_db: catalog_db.to_path_buf(),
            target_db: target,
        }));
    }
    Ok(None)
}

/// Moves a catalog and its SQLite companions to another path, so the open at
/// the new root reuses it.
fn move_catalog_files(from: &Path, to: &Path) -> Result<()> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::rename(from, to)
        .with_context(|| format!("moving {} to {}", from.display(), to.display()))?;
    for suffix in ["-wal", "-shm", ".bak"] {
        let source = PathBuf::from(format!("{}{suffix}", from.display()));
        if source.exists() {
            let target = PathBuf::from(format!("{}{suffix}", to.display()));
            // The main catalog already moved; a companion that cannot follow
            // is stale and harmless, but say so.
            if let Err(err) = std::fs::rename(&source, &target) {
                log::warn!("could not move {}: {err:#}", source.display());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "lumina_library_{name}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn catalogued_photo(catalog: &Catalog, path: &Path) -> crate::MediaItem {
        std::fs::write(path, b"photo bytes").unwrap();
        let mut item = crate::catalog::tests_support::video_item(&path.to_string_lossy());
        item.file_size = 11;
        item.mtime_epoch_ms = 1_700_000_000_000;
        catalog.insert_or_update_batch(&[item]).unwrap().remove(0)
    }

    #[test]
    fn catalog_paths_are_stable_and_per_library() {
        let base = temp_dir("paths");
        let one = base.join("one");
        let two = base.join("two");
        std::fs::create_dir_all(&one).unwrap();
        std::fs::create_dir_all(&two).unwrap();
        let catalogs = base.join("catalogs");

        let a = catalog_path_in(&catalogs, &one);
        assert_eq!(a, catalog_path_in(&catalogs, &one), "stable for one root");
        assert_ne!(a, catalog_path_in(&catalogs, &two), "different per root");
        assert_eq!(
            a.parent(),
            Some(catalogs.as_path()),
            "the file lives in the given directory"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn removing_a_library_keeps_its_files_and_drops_its_catalog() {
        let base = temp_dir("remove");
        let library = base.join("library");
        let catalogs = base.join("catalogs");
        let cache = base.join("cache");
        std::fs::create_dir_all(&library).unwrap();
        std::fs::create_dir_all(&catalogs).unwrap();
        std::fs::create_dir_all(&cache).unwrap();

        let photo = library.join("photo.jpg");
        let catalog_db = catalog_path_in(&catalogs, &library);
        {
            let catalog = Catalog::open(&catalog_db).unwrap();
            catalogued_photo(&catalog, &photo);
            assert_eq!(catalog.total_count().unwrap(), 1);
        }

        let removal = remove_library_at(&catalog_db, &cache).unwrap();
        assert!(removal.removed_catalog, "the catalog file is removed");
        assert!(!catalog_db.exists());
        assert!(
            photo.exists(),
            "deleting a library never touches the user's photos"
        );
        assert!(library.is_dir());
        assert_eq!(removal.thumbnails_removed, 0, "no thumbnails were cached");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn removing_a_library_evicts_its_cached_thumbnails() {
        let base = temp_dir("evict");
        let library = base.join("library");
        let catalogs = base.join("catalogs");
        let cache = base.join("cache");
        std::fs::create_dir_all(&library).unwrap();
        std::fs::create_dir_all(&catalogs).unwrap();
        let catalog_db = catalog_path_in(&catalogs, &library);

        let photo = library.join("photo.jpg");
        {
            let catalog = Catalog::open(&catalog_db).unwrap();
            catalogued_photo(&catalog, &photo);
        }
        let thumbs = DiskCache::with_directory(&cache).unwrap();
        for size in [320, 512] {
            let key = DiskCache::compute_key(&photo, 11, 1_700_000_000_000, size);
            std::fs::write(thumbs.cache_file_path(&key), b"thumb").unwrap();
        }
        let removal = remove_library_at(&catalog_db, &cache).unwrap();
        assert_eq!(
            removal.thumbnails_removed, 2,
            "every decoded size of the library's items is evicted"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn removing_a_library_without_a_catalog_is_not_an_error() {
        let base = temp_dir("absent");
        let removal = remove_library_at(&base.join("missing.db"), &base.join("cache")).unwrap();
        assert!(!removal.removed_catalog);
        assert_eq!(removal.thumbnails_removed, 0);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_missing_library_is_found_again_on_its_volume() {
        let base = temp_dir("relocate");
        // The same volume mounted first at old_mount, then at new_mount.
        let old_mount = base.join("old_mount");
        let new_mount = base.join("new_mount");
        let old = old_mount.join("lib");
        let new = new_mount.join("lib");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::create_dir_all(&new).unwrap();
        let db = base.join("catalog.db");
        let moved_db = base.join("moved_catalog.db");

        let catalog = Catalog::open(&db).unwrap();
        let root = catalog.ensure_library_root(&old).unwrap();
        let item = catalogued_photo(&catalog, &old.join("a.jpg"));
        catalog.toggle_favorite(item.id, true).unwrap();
        // The library's files came with the volume.
        std::fs::copy(old.join("a.jpg"), new.join("a.jpg")).unwrap();
        // Point the catalog at the new mount: find_mount resolves a `mount:`
        // id directly, so the decision can be tested without a real remount.
        catalog
            .conn
            .lock()
            .execute(
                "UPDATE library_roots SET volume_id = ?1, rel_from_volume = 'lib' WHERE id = ?2;",
                rusqlite::params![format!("mount:{}", new_mount.display()), root.id],
            )
            .unwrap();

        // While the old folder exists, nothing is relocated.
        assert_eq!(
            find_library_relocation_at(&db, &old, Some(&moved_db)).unwrap(),
            None
        );
        assert!(!moved_db.exists());
        std::fs::remove_dir_all(&old).unwrap();

        let relocation = find_library_relocation_at(&db, &old, Some(&moved_db))
            .unwrap()
            .expect("the catalog follows its volume, not the old path");
        assert_eq!(relocation.library_root, new);
        relocation.commit().unwrap();
        assert!(
            moved_db.exists(),
            "the catalog moves to the new root's path, so opening it reuses this one"
        );
        // Opening the moved catalog is what the app does next: same row,
        // rating and relative identity, no rescan needed.
        let reopened = Catalog::open(&moved_db).unwrap();
        let items = reopened.get_item_slice(0, 1).unwrap();
        assert_eq!(items[0].path, new.join("a.jpg"));
        assert!(items[0].is_favorite, "metadata followed the catalog");
        assert_eq!(
            reopened.sample_rooted_rel_paths(8).unwrap(),
            vec![PathBuf::from("a.jpg")],
            "the relative identity still names the file"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_candidate_without_the_librarys_files_is_not_adopted() {
        let base = temp_dir("sanity");
        let old = base.join("old");
        let new = base.join("new");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::create_dir_all(&new).unwrap();
        let db = base.join("catalog.db");
        let catalog = Catalog::open(&db).unwrap();
        let root = catalog.ensure_library_root(&old).unwrap();
        catalogued_photo(&catalog, &old.join("a.jpg"));
        // A reused device number points at a mount that lacks the library.
        catalog
            .conn
            .lock()
            .execute(
                "UPDATE library_roots SET volume_id = ?1, rel_from_volume = 'new' WHERE id = ?2;",
                rusqlite::params![format!("mount:{}", base.display()), root.id],
            )
            .unwrap();
        std::fs::remove_dir_all(&old).unwrap();
        assert_eq!(
            find_library_relocation_at(&db, &old, None).unwrap(),
            None,
            "an unrelated folder with the same name must not adopt the catalog"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn an_unreadable_catalog_can_still_be_removed() {
        let base = temp_dir("corrupt");
        let catalog_db = base.join("catalog.db");
        std::fs::write(&catalog_db, b"not a sqlite database").unwrap();
        let removal = remove_library_at(&catalog_db, &base.join("cache")).unwrap();
        assert!(removal.removed_catalog, "the corrupt catalog is removed");
        assert!(!catalog_db.exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_root_remaps_only_when_its_recorded_volume_matches() {
        let base = temp_dir("remap");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let moved = base.join("moved");
        std::fs::create_dir_all(&moved).unwrap();
        let db = base.join("catalog.db");

        let catalog = Catalog::open(&db).unwrap();
        let root = catalog.ensure_library_root(&library).unwrap();
        catalogued_photo(&catalog, &library.join("a.jpg"));

        // A different filesystem id cannot adopt the catalog.
        catalog
            .conn
            .lock()
            .execute(
                "UPDATE library_roots SET volume_id = 'uuid:dead-beef' WHERE id = ?1;",
                [root.id],
            )
            .unwrap();
        assert!(!catalog.remap_library_root(&moved).unwrap());
        assert_eq!(
            catalog.all_file_signatures().unwrap()[0].0,
            library.join("a.jpg"),
            "a refused remap leaves every path alone"
        );

        // A fallback id (a platform with no filesystem id) is trusted to the
        // caller, which already resolved the mount point; the paths follow.
        catalog
            .conn
            .lock()
            .execute(
                "UPDATE library_roots SET volume_id = 'mount:/nowhere', \
                 rel_from_volume = 'library' WHERE id = ?1;",
                [root.id],
            )
            .unwrap();
        assert!(catalog.remap_library_root(&moved).unwrap());
        assert_eq!(
            catalog.all_file_signatures().unwrap()[0].0,
            moved.join("a.jpg"),
            "the root-relative path is the identity, the absolute one follows"
        );
        assert_eq!(
            catalog.library_root().unwrap().unwrap().stored_root,
            moved,
            "the stored root moves with the paths"
        );
        let _ = std::fs::remove_dir_all(&base);
    }
}
