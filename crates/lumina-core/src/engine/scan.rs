//! Library scans and the photo size backfill.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::*;

impl LuminaEngine {
    pub fn cancel_scan(&self) {
        let lock = self.active_scan_cancel.lock();
        lock.store(true, Ordering::SeqCst);
    }

    pub fn rescan(&self) {
        let new_flag = Arc::new(AtomicBool::new(false));
        {
            let mut lock = self.active_scan_cancel.lock();
            lock.store(true, Ordering::SeqCst);
            *lock = new_flag.clone();
        }
        let engine = self.clone();
        let cancel_flag = new_flag;
        let spawned = std::thread::Builder::new()
            .name("lumina-rescan".to_string())
            .spawn(move || {
                engine.run_scan(cancel_flag);
            });
        if let Err(err) = spawned {
            log::error!("could not start the rescan thread: {err}");
            let _ = self.event_tx.send(EngineEvent::Error(format!(
                "Could not start the rescan: {err}"
            )));
        }
    }

    pub(crate) fn run_scan(&self, cancel_flag: Arc<AtomicBool>) {
        self.run_scan_with(cancel_flag, dims::read_displayed_size);
    }

    /// [`Self::run_scan`] with the photo-size reader for the one-time size
    /// backfill injected, so tests can hold it.
    fn run_scan_with(
        &self,
        cancel_flag: Arc<AtomicBool>,
        measure_photo: impl Fn(&Path) -> Option<(u32, u32)>,
    ) {
        // An unmounted or deleted library folder would make every file look
        // gone; report it instead of marking the library missing.
        if !self.library_root.is_dir() {
            let _ = self.event_tx.send(EngineEvent::Error(format!(
                "Library folder is not available: {}",
                self.library_root.display()
            )));
            let total_items = self.catalog.total_count().unwrap_or(0);
            let _ = self
                .event_tx
                .send(EngineEvent::ScanComplete { total_items });
            return;
        }
        // Before the walk, so a file moved while Lumina was closed is already
        // missing when the walk finds it at its new path, and reconnects.
        match self.catalog.reconcile_files() {
            Ok(missing) if !missing.is_empty() => {
                let _ = self.event_tx.send(EngineEvent::ItemsMissing(missing));
            }
            Ok(_) => {}
            Err(err) => log::warn!("checking catalogued files failed: {err:#}"),
        }
        let (batch_tx, batch_rx) = unbounded();
        // An empty map would re-insert every file and overwrite edited dates
        // and places, so a failed read stops the scan instead.
        let known_signatures = match self.catalog.get_file_signatures() {
            Ok(signatures) => signatures,
            Err(err) => {
                log::error!("scan aborted, catalog file list unreadable: {err:#}");
                let _ = self
                    .event_tx
                    .send(EngineEvent::Error(format!("Library scan failed: {err:#}")));
                // Not self.total_count(): its own banner would replace this one.
                let total_items = self.catalog.total_count().unwrap_or(0);
                let _ = self
                    .event_tx
                    .send(EngineEvent::ScanComplete { total_items });
                return;
            }
        };
        let scanner =
            Scanner::new_with_cancel(self.library_root.clone(), batch_tx, cancel_flag.clone())
                .with_known_signatures(known_signatures);

        let catalog = self.catalog.clone();
        let event_tx = self.event_tx.clone();

        // Ingestion worker thread to consume discovered items and commit into SQLite in batches
        let ingest_handle = std::thread::spawn(move || {
            let mut save_failure_reported = false;
            for event in batch_rx {
                match event {
                    EngineEvent::BatchDiscovered { items } => {
                        match catalog.insert_or_update_batch(&items) {
                            Ok(inserted) => {
                                let _ =
                                    event_tx.send(EngineEvent::BatchDiscovered { items: inserted });
                            }
                            Err(err) => {
                                log::error!("saving {} scanned items failed: {err:#}", items.len());
                                if !save_failure_reported {
                                    save_failure_reported = true;
                                    let _ = event_tx.send(EngineEvent::Error(format!(
                                        "Could not save scanned items: {err:#}"
                                    )));
                                }
                            }
                        }
                    }
                    progress @ EngineEvent::ScanProgress { .. } => {
                        let _ = event_tx.send(progress);
                    }
                    EngineEvent::ScanComplete { .. } => {
                        break;
                    }
                    _ => {}
                }
            }
        });

        let scan_res = scanner.scan_files();
        let _ = ingest_handle.join();

        if let Ok(total_items) = scan_res {
            if !cancel_flag.load(Ordering::Relaxed) {
                // Videos catalogued before the scanner read their size are
                // unchanged on disk, so the incremental scan skips them.
                if let Err(e) = self.catalog.fill_video_sizes(extract_video_size) {
                    log::warn!("video size backfill failed: {e:#}");
                }
            }
            // Automatically recompute timeline buckets upon scan completion
            if let Err(e) = self.catalog.compute_timeline_buckets() {
                log::warn!("timeline bucket refresh failed: {e:#}");
            }
            let _ = self
                .event_tx
                .send(EngineEvent::ScanComplete { total_items });
            self.backfill_photo_sizes(&cancel_flag, measure_photo);
        }
    }

    /// Once per catalog, after ScanComplete so the library opens without
    /// waiting: fixes photo sizes stored before the scanner applied EXIF
    /// orientation (re-reading every photo once, slow on a large RAW or
    /// external-disk library). When rows changed, a second ScanComplete
    /// refreshes counts, as a rescan does. A cancelled scan (rescan, or the
    /// library closing) sends nothing: the next scan runs the backfill again,
    /// and the UI drops a closed library's events by generation.
    fn backfill_photo_sizes(
        &self,
        cancel_flag: &AtomicBool,
        measure_photo: impl Fn(&Path) -> Option<(u32, u32)>,
    ) {
        if cancel_flag.load(Ordering::Relaxed) {
            return;
        }
        match self
            .catalog
            .backfill_oriented_dims(measure_photo, cancel_flag)
        {
            Ok(0) => {}
            Ok(_) if cancel_flag.load(Ordering::Relaxed) => {}
            Ok(_) => {
                let total_items = match self.catalog.total_count() {
                    Ok(total) => total,
                    Err(e) => {
                        log::warn!("counting items after the photo size backfill failed: {e:#}");
                        return;
                    }
                };
                let _ = self
                    .event_tx
                    .send(EngineEvent::ScanComplete { total_items });
            }
            Err(e) => log::warn!("photo size backfill failed: {e:#}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{temp_library, TEST_WAIT};
    use std::fs;
    use std::sync::mpsc;
    use std::time::Duration;

    /// Waits for the next ScanComplete, skipping other events.
    fn next_scan_complete(rx: &Receiver<EngineEvent>) {
        let deadline = std::time::Instant::now() + TEST_WAIT;
        loop {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            match rx.recv_timeout(left) {
                Ok(EngineEvent::ScanComplete { .. }) => return,
                Ok(_) => {}
                Err(err) => panic!("no ScanComplete: {err}"),
            }
        }
    }

    /// The size backfill must not hold up ScanComplete (session restore,
    /// counts and the timeline wait for it); a second ScanComplete after it
    /// refreshes counts once sizes changed.
    #[test]
    fn scan_complete_is_sent_before_the_photo_size_backfill() {
        let dir = temp_library("dims_after_scan");
        let media = dir.join("media");
        fs::create_dir_all(&media).expect("media dir");
        crate::decoder::tests::orientation_six_jpeg(&media.join("p.jpg"), 16, 8);
        let db = dir.join("catalog.db");
        let (engine, rx) =
            LuminaEngine::open_with_paths(media, db.clone(), Some(dir.join("cache")))
                .expect("engine opens");
        next_scan_complete(&rx);
        let id = engine.get_item_slice(0..1)[0].id;
        let size = || {
            let item = engine.catalog().get_item_by_id(id).unwrap().unwrap();
            (item.width, item.height)
        };
        assert_eq!(size(), (8, 16), "a new scan stores the displayed size");
        // The startup scan's own backfill must not see the seeded catalog.
        engine.cancel_scan();

        // What an older Lumina left: the unrotated size and no marker.
        rusqlite::Connection::open(&db)
            .and_then(|conn| {
                conn.busy_timeout(Duration::from_secs(5))?;
                conn.execute_batch(
                    "UPDATE media_items SET width = 16, height = 8;
                     DELETE FROM app_config WHERE key = 'dims_oriented_v1';",
                )
            })
            .expect("seed an old catalog");

        let (release, held) = mpsc::channel::<()>();
        let scan = {
            let engine = engine.clone();
            std::thread::spawn(move || {
                engine.run_scan_with(Arc::new(AtomicBool::new(false)), move |path| {
                    held.recv_timeout(TEST_WAIT)
                        .expect("test releases the backfill");
                    dims::read_displayed_size(path)
                });
            })
        };
        next_scan_complete(&rx);
        assert_eq!(
            size(),
            (16, 8),
            "ScanComplete arrives while the backfill is still measuring"
        );
        release.send(()).expect("backfill waits");
        next_scan_complete(&rx);
        assert_eq!(
            size(),
            (8, 16),
            "the second ScanComplete follows the fixed sizes, so counts refresh"
        );
        scan.join().expect("scan thread");
        drop(engine);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cancel_scan_after_rescan_sets_the_new_scans_flag() {
        let dir = temp_library("cancel_scan");
        let (engine, rx) = LuminaEngine::open_with_paths(dir.clone(), dir.join("catalog.db"), None)
            .expect("engine opens");
        loop {
            match rx.recv_timeout(TEST_WAIT) {
                Ok(EngineEvent::ScanComplete { .. }) => break,
                Ok(_) => {}
                Err(err) => panic!("the startup scan must complete: {err}"),
            }
        }
        let first = engine.active_scan_cancel.lock().clone();
        engine.rescan();
        let second = engine.active_scan_cancel.lock().clone();
        assert!(
            !Arc::ptr_eq(&first, &second),
            "rescan must give the new scan its own flag"
        );
        assert!(
            !second.load(Ordering::SeqCst),
            "the new scan must not start cancelled"
        );
        engine.cancel_scan();
        assert!(
            second.load(Ordering::SeqCst),
            "cancel_scan must set the flag the newest scan polls, or Retry cannot stop it"
        );
        drop(engine);
        let _ = fs::remove_dir_all(&dir);
    }
}
