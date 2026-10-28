//! End-to-end test for the real-time library watcher.
//!
//! The engine is opened on a temporary library, the initial scan is awaited,
//! and then files are created/removed behind the engine's back. The test
//! asserts that the watcher (not a manual rescan) drives the catalog and the
//! event stream. Recv-timeout loops are used throughout so coalesced or
//! duplicate filesystem events cannot make the test flaky.

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

fn write_jpeg(path: &Path) {
    let image = image::RgbImage::from_fn(16, 16, |x, y| {
        image::Rgb([(x * 12) as u8, (y * 12) as u8, 96])
    });
    image.save(path).expect("write jpeg image");
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

/// Awaits the ItemRemoved event for a specific media id.
fn wait_for_item_removed(rx: &Receiver<EngineEvent>, id: MediaId) {
    recv_until(
        rx,
        EVENT_TIMEOUT,
        "ItemRemoved event",
        |event| matches!(event, EngineEvent::ItemRemoved(removed) if *removed == id),
    );
}

#[test]
fn test_watcher_detects_external_add_and_delete() {
    let library = TestLibrary::new("add_delete");
    let media_dir = library.media_dir();

    // Seed the library with one real image before the engine opens.
    write_jpeg(&media_dir.join("seed.jpg"));

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
    write_jpeg(&added_path);

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

    // Delete the image externally and expect a matching ItemRemoved.
    std::fs::remove_file(&added_path).expect("remove externally added image");
    wait_for_item_removed(&rx, added_id);
    assert_eq!(
        engine.total_count(),
        1,
        "watcher should drop the externally deleted image from the catalog"
    );

    // Dropping the engine must stop the watcher thread cleanly (no hang).
    drop(engine);
}
