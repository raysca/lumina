//! End-to-end test for the real-time library watcher.
//!
//! The engine is opened on a temporary library, the initial scan is awaited,
//! and then files are created/removed behind the engine's back. The test
//! asserts that the watcher (not a manual rescan) drives the catalog and the
//! event stream. Recv-timeout loops are used throughout so coalesced or
//! duplicate filesystem events cannot make the test flaky.

mod common;

use common::write_jpeg;
use crossbeam_channel::{Receiver, RecvTimeoutError};
use lumina_core::{EngineEvent, LuminaEngine, MediaId};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const EVENT_TIMEOUT: Duration = Duration::from_secs(15);

/// Temporary library with its own media directory, database and cache.
struct TestLibrary {
    root: PathBuf,
}

impl TestLibrary {
    fn new(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "lumina_watcher_{name}_{}_{}",
            std::process::id(),
            nanos
        ));
        std::fs::create_dir_all(root.join("media")).expect("create temp media dir");
        Self { root }
    }

    fn media_dir(&self) -> PathBuf {
        self.root.join("media")
    }

    fn db_path(&self) -> PathBuf {
        self.root.join("catalog.db")
    }

    fn cache_dir(&self) -> PathBuf {
        self.root.join("cache")
    }
}

impl Drop for TestLibrary {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Pulls events until the predicate matches, panicking on timeout.
fn recv_until<F>(
    rx: &Receiver<EngineEvent>,
    timeout: Duration,
    label: &str,
    mut predicate: F,
) -> EngineEvent
where
    F: FnMut(&EngineEvent) -> bool,
{
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            panic!("timed out after {timeout:?} waiting for {label}");
        }
        match rx.recv_timeout(remaining) {
            Ok(event) => {
                if predicate(&event) {
                    return event;
                }
                // Any other event (ScanProgress, coalesced/duplicate batches,
                // unrelated removals) is tolerated and ignored.
            }
            Err(RecvTimeoutError::Timeout) => {
                panic!("timed out after {timeout:?} waiting for {label}")
            }
            Err(RecvTimeoutError::Disconnected) => {
                panic!("engine event channel disconnected while waiting for {label}")
            }
        }
    }
}

/// Awaits the engine's initial ScanComplete and returns the reported total.
fn wait_for_scan_complete(rx: &Receiver<EngineEvent>) -> usize {
    let event = recv_until(rx, EVENT_TIMEOUT, "initial ScanComplete", |event| {
        matches!(event, EngineEvent::ScanComplete { .. })
    });
    match event {
        EngineEvent::ScanComplete { total_items } => total_items,
        _ => unreachable!(),
    }
}

/// Awaits a BatchDiscovered batch that contains the given path.
fn wait_for_batch_with_path(
    rx: &Receiver<EngineEvent>,
    path: &Path,
) -> Vec<lumina_core::MediaItem> {
    let event = recv_until(
        rx,
        EVENT_TIMEOUT,
        &format!("BatchDiscovered for {}", path.display()),
        |event| match event {
            EngineEvent::BatchDiscovered { items } => items.iter().any(|item| item.path == path),
            _ => false,
        },
    );
    match event {
        EngineEvent::BatchDiscovered { items } => items,
        _ => unreachable!(),
    }
}

/// Awaits the ItemsMissing event naming a specific media id.
fn wait_for_item_missing(rx: &Receiver<EngineEvent>, id: MediaId) {
    recv_until(
        rx,
        EVENT_TIMEOUT,
        "ItemsMissing event",
        |event| matches!(event, EngineEvent::ItemsMissing(ids) if ids.contains(&id)),
    );
}

#[test]
fn test_watcher_detects_external_add_and_delete() {
    let library = TestLibrary::new("add_delete");
    let media_dir = library.media_dir();

    // Seed the library with one real image before the engine opens.
    write_jpeg(&media_dir.join("seed.jpg"), 96);

    let (engine, rx) = LuminaEngine::open_with_paths(
        media_dir.clone(),
        library.db_path(),
        Some(library.cache_dir()),
    )
    .expect("open engine with isolated paths");

    let scanned_total = wait_for_scan_complete(&rx);
    assert_eq!(scanned_total, 1, "initial scan should find the seed image");
    assert_eq!(engine.total_count(), 1);

    // Create a new image without asking the engine to rescan.
    let added_path = media_dir.join("added_externally.jpg");
    write_jpeg(&added_path, 96);

    let inserted = wait_for_batch_with_path(&rx, &added_path);
    let added_id = inserted
        .iter()
        .find(|item| item.path == added_path)
        .map(|item| item.id)
        .expect("new image should be present in the BatchDiscovered payload");
    assert_eq!(
        engine.total_count(),
        2,
        "watcher should add the externally created image to the catalog"
    );

    // Delete the image externally: the row stays, marked missing.
    std::fs::remove_file(&added_path).expect("remove externally added image");
    wait_for_item_missing(&rx, added_id);
    assert_eq!(
        engine.total_count(),
        2,
        "an externally deleted image keeps its row and metadata"
    );
    assert!(
        engine.get_missing_ids().contains(&added_id),
        "the deleted image is marked missing"
    );

    // Dropping the engine must stop the watcher thread cleanly (no hang).
    drop(engine);
}

/// Opens the engine on `library`, waits for its scan, and returns it.
fn open_scanned(library: &TestLibrary) -> (std::sync::Arc<LuminaEngine>, Receiver<EngineEvent>) {
    let (engine, rx) = LuminaEngine::open_with_paths(
        library.media_dir(),
        library.db_path(),
        Some(library.cache_dir()),
    )
    .expect("open engine with isolated paths");
    wait_for_scan_complete(&rx);
    (engine, rx)
}

fn id_at(engine: &LuminaEngine, path: &Path) -> MediaId {
    engine
        .get_item_slice(0..100)
        .into_iter()
        .find(|item| item.path == path)
        .map(|item| item.id)
        .unwrap_or_else(|| panic!("{} must be catalogued", path.display()))
}

#[test]
fn rescan_reconnects_moved_files_and_keeps_deleted_ones() {
    let library = TestLibrary::new("rescan_missing");
    let media = library.media_dir();
    let (moved, deleted) = (media.join("moved.jpg"), media.join("deleted.jpg"));
    write_jpeg(&moved, 96);
    write_jpeg(&deleted, 96);
    let (moved_id, deleted_id) = {
        let (engine, _rx) = open_scanned(&library);
        let ids = (id_at(&engine, &moved), id_at(&engine, &deleted));
        engine
            .add_tags_batch(&[ids.0], &["kept".to_string()])
            .unwrap();
        ids
    };

    // While Lumina is closed: one file moved into a folder, one deleted.
    let new_path = media.join("2024/moved.jpg");
    std::fs::create_dir_all(new_path.parent().unwrap()).unwrap();
    std::fs::rename(&moved, &new_path).unwrap();
    let deleted_bytes = std::fs::read(&deleted).unwrap();
    std::fs::remove_file(&deleted).unwrap();

    let (engine, _rx) = open_scanned(&library);
    let item = engine.get_item_by_id(moved_id).expect("moved row kept");
    assert_eq!(item.path, new_path, "a file moved while closed reconnects");
    assert_eq!(engine.get_tags_for_item(moved_id), vec!["kept"]);
    assert_eq!(engine.total_count(), 2, "nothing is added or dropped");
    assert_eq!(
        engine.get_missing_ids(),
        [deleted_id].into_iter().collect(),
        "a deleted file is kept, marked missing"
    );
    drop(engine);

    // The deleted file comes back at its old path.
    std::fs::write(&deleted, deleted_bytes).unwrap();
    let (engine, _rx) = open_scanned(&library);
    assert!(
        engine.get_missing_ids().is_empty(),
        "a file restored at its old path is present again"
    );
    assert_eq!(id_at(&engine, &deleted), deleted_id);
}

#[test]
fn scan_of_unavailable_library_marks_nothing() {
    let library = TestLibrary::new("rescan_root_gone");
    write_jpeg(&library.media_dir().join("a.jpg"), 96);
    drop(open_scanned(&library));

    let away = library.root.join("unmounted");
    std::fs::rename(library.media_dir(), &away).unwrap();
    let (engine, rx) = LuminaEngine::open_with_paths(
        library.media_dir(),
        library.db_path(),
        Some(library.cache_dir()),
    )
    .expect("an unavailable library still opens");
    recv_until(
        &rx,
        EVENT_TIMEOUT,
        "unavailable-library error",
        |event| matches!(event, EngineEvent::Error(message) if message.starts_with("Library folder is not available")),
    );
    wait_for_scan_complete(&rx);
    assert!(
        engine.get_missing_ids().is_empty(),
        "nothing is marked missing"
    );
    assert_eq!(engine.total_count(), 1);
}

/// A tagged photo copied to `copy.jpg` and deleted while Lumina was closed:
/// the original is missing and the copy is catalogued as a new item.
fn copied_back(name: &str) -> (TestLibrary, std::sync::Arc<LuminaEngine>, MediaId, MediaId) {
    let library = TestLibrary::new(name);
    let media = library.media_dir();
    write_jpeg(&media.join("a.jpg"), 96);
    let original = {
        let (engine, _rx) = open_scanned(&library);
        let id = id_at(&engine, &media.join("a.jpg"));
        engine.add_tags_batch(&[id], &["kept".to_string()]).unwrap();
        id
    };
    std::fs::copy(media.join("a.jpg"), media.join("copy.jpg")).unwrap();
    std::fs::remove_file(media.join("a.jpg")).unwrap();
    let (engine, _rx) = open_scanned(&library);
    let copy = id_at(&engine, &media.join("copy.jpg"));
    assert!(engine.get_missing_ids().contains(&original));
    (library, engine, original, copy)
}

#[test]
fn locate_folds_a_copy_of_the_same_file() {
    let (library, engine, original, copy) = copied_back("locate_fold");
    let chosen = library.media_dir().join("copy.jpg");
    assert_eq!(
        engine.relink(original, &chosen).unwrap(),
        lumina_core::RelinkOutcome::Folded(copy)
    );
    assert_eq!(engine.get_item_by_id(original).unwrap().path, chosen);
    assert!(
        engine.get_item_by_id(copy).is_none(),
        "the copy's row folds in"
    );
    assert_eq!(engine.get_tags_for_item(original), vec!["kept"]);
    assert!(engine.get_missing_ids().is_empty());
}

#[test]
fn locate_repoints_to_an_uncatalogued_file() {
    let (library, engine, original, copy) = copied_back("locate_repoint");
    engine
        .catalog()
        .permanently_delete_items(&[copy], lumina_core::DeletionTarget::CatalogOnly)
        .unwrap();
    let chosen = library.media_dir().join("copy.jpg");
    assert_eq!(
        engine.relink(original, &chosen).unwrap(),
        lumina_core::RelinkOutcome::Relinked
    );
    assert_eq!(engine.get_item_by_id(original).unwrap().path, chosen);
    assert!(engine.get_missing_ids().is_empty());
}

#[test]
fn locate_refuses_a_different_photo_and_outside_files() {
    let (library, engine, original, copy) = copied_back("locate_refuse");
    let other = library.media_dir().join("other.png");
    image::RgbImage::from_fn(40, 40, |x, _| image::Rgb([x as u8, 0, 0]))
        .save(&other)
        .unwrap();
    // Catalogue it now rather than waiting on the watcher.
    let (tx, _rx) = crossbeam_channel::unbounded();
    let item = lumina_core::scanner::Scanner::new(library.media_dir(), tx)
        .inspect_media_file(&other)
        .unwrap();
    engine.catalog().insert_or_update_batch(&[item]).unwrap();

    let err = engine.relink(original, &other).unwrap_err().to_string();
    assert!(
        err.starts_with("That file is already in your library"),
        "{err}"
    );
    assert!(
        engine.get_missing_ids().contains(&original),
        "nothing changed"
    );
    assert!(engine.get_item_by_id(copy).is_some());

    let outside = library.root.join("outside.jpg");
    write_jpeg(&outside, 96);
    let err = engine.relink(original, &outside).unwrap_err().to_string();
    assert!(
        err.starts_with("Choose a file inside the library folder"),
        "{err}"
    );
}

/// A 24x16 JPEG, tagged EXIF orientation 6 (displayed 16x24) when `turned`.
// ponytail: the lib's decoder test helper is crate-private, unreachable here.
fn write_landscape_jpeg(path: &Path, turned: bool) {
    let image = image::RgbImage::from_fn(24, 16, |x, y| {
        image::Rgb([(x * 10) as u8, (y * 10) as u8, 96])
    });
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 90)
        .encode_image(&image)
        .unwrap();
    if turned {
        // Big-endian TIFF, IFD0 with one entry: Orientation (0x0112) = 6,
        // spliced after SOI and the encoder's APP0.
        let mut app1 = b"Exif\0\0MM\0\x2a\0\0\0\x08\0\x01".to_vec();
        app1.extend_from_slice(&[0x01, 0x12, 0, 3, 0, 0, 0, 1, 0, 6, 0, 0, 0, 0, 0, 0]);
        let mut segment = vec![0xff, 0xe1];
        segment.extend_from_slice(&((app1.len() + 2) as u16).to_be_bytes());
        segment.extend_from_slice(&app1);
        let app0_end = 4 + u16::from_be_bytes([jpeg[4], jpeg[5]]) as usize;
        jpeg.splice(app0_end..app0_end, segment);
    }
    std::fs::write(path, jpeg).unwrap();
}

/// `a.jpg` catalogued, copied to `copy.jpg` and deleted while closed, with
/// the missing row's size read the other way round. Returns the relink
/// result for `copy.jpg` and the copy's id.
fn relink_with_transposed_row(
    name: &str,
    turned: bool,
) -> (anyhow::Result<lumina_core::RelinkOutcome>, MediaId) {
    let library = TestLibrary::new(name);
    let media = library.media_dir();
    write_landscape_jpeg(&media.join("a.jpg"), turned);
    let original = {
        let (engine, _rx) = open_scanned(&library);
        id_at(&engine, &media.join("a.jpg"))
    };
    std::fs::copy(media.join("a.jpg"), media.join("copy.jpg")).unwrap();
    std::fs::remove_file(media.join("a.jpg")).unwrap();
    rusqlite::Connection::open(library.db_path())
        .unwrap()
        .execute(
            "UPDATE media_items SET width = height, height = width WHERE id = ?1;",
            [original.0 as i64],
        )
        .unwrap();
    let (engine, _rx) = open_scanned(&library);
    let copy = id_at(&engine, &media.join("copy.jpg"));
    assert!(engine.get_missing_ids().contains(&original));
    (engine.relink(original, &media.join("copy.jpg")), copy)
}

/// Regression: a portrait (EXIF orientation 6) that went missing before
/// sizes were stored oriented keeps its unrotated 24x16, so Locate compared
/// it with the scanned 16x24 and refused to fold the same file.
#[test]
fn locate_folds_a_twin_whose_old_row_stored_the_turned_size() {
    let (outcome, copy) = relink_with_transposed_row("locate_turned_size", true);
    assert_eq!(
        outcome.unwrap(),
        lumina_core::RelinkOutcome::Folded(copy),
        "a quarter-turned photo's old unrotated size still matches its twin"
    );
}

/// Without a quarter-turn orientation, a transposed size is a different
/// photo (two uncompressed TIFFs, 6000x4000 and 4000x6000, share a byte
/// size), so it must never fold and move tags onto the wrong file.
#[test]
fn locate_refuses_a_transposed_size_without_a_quarter_turn() {
    let (outcome, _) = relink_with_transposed_row("locate_transposed_plain", false);
    let err = outcome.expect_err("a transposed plain photo is not a twin");
    assert!(
        err.to_string()
            .starts_with("That file is already in your library"),
        "{err}"
    );
}

/// Regression: the scanner already stores the displayed (oriented) size, so a
/// quarter-turned photo's row holds 16x24. Relinking must accept that stored
/// pair and not double-rotate it into 24x16, which refused to fold a normal
/// rotated twin.
#[test]
fn locate_folds_a_normally_scanned_turned_twin() {
    let library = TestLibrary::new("locate_turned_normal");
    let media = library.media_dir();
    write_landscape_jpeg(&media.join("a.jpg"), true);
    let original = {
        let (engine, _rx) = open_scanned(&library);
        let id = id_at(&engine, &media.join("a.jpg"));
        let scanned = engine.get_item_by_id(id).expect("original row");
        assert_eq!(
            (scanned.width, scanned.height),
            (16, 24),
            "the scanner stores the displayed (oriented) size; without this the \
             test would still pass if the scanner stopped orienting"
        );
        id
    };
    std::fs::copy(media.join("a.jpg"), media.join("copy.jpg")).unwrap();
    std::fs::remove_file(media.join("a.jpg")).unwrap();
    let (engine, _rx) = open_scanned(&library);
    let copy = id_at(&engine, &media.join("copy.jpg"));
    assert!(engine.get_missing_ids().contains(&original));
    assert_eq!(
        engine.relink(original, &media.join("copy.jpg")).unwrap(),
        lumina_core::RelinkOutcome::Folded(copy),
        "a normally scanned quarter-turned photo must still relink to its twin"
    );
}
