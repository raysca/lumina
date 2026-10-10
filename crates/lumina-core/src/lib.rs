pub mod cache;
pub mod catalog;
pub mod decoder;
pub mod scanner;
pub mod watcher;

use anyhow::Result;
use crossbeam_channel::{unbounded, Receiver, Sender};
use serde::{Deserialize, Serialize};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub use cache::DiskCache;
pub use catalog::Catalog;
pub use decoder::DecoderPipeline;
pub use scanner::Scanner;
pub use watcher::LibraryWatcher;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MediaId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaType {
    Image,
    Video,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaItem {
    pub id: MediaId,
    pub path: PathBuf,
    pub media_type: MediaType,
    pub width: u32,
    pub height: u32,
    pub duration_secs: f64,
    pub file_size: u64,
    pub mtime_epoch_ms: i64,
    pub timestamp_epoch_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Rgba8,
    Bgra8,
}

pub struct DecodedFrame {
    pub id: MediaId,
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum EngineEvent {
    BatchDiscovered { items: Vec<MediaItem> },
    ScanProgress { items_found: usize, is_complete: bool },
    ItemAdded(MediaItem),
    ItemRemoved(MediaId),
    ScanComplete { total_items: usize },
}

pub struct LuminaEngine {
    library_root: PathBuf,
    catalog: Catalog,
    cache: DiskCache,
    decoder: DecoderPipeline,
    event_tx: Sender<EngineEvent>,
    active_scan_cancel: Arc<AtomicBool>,
    _watcher: Option<LibraryWatcher>,
}

impl LuminaEngine {
    pub fn open(library_root: PathBuf) -> Result<(Arc<Self>, Receiver<EngineEvent>)> {
        let (event_tx, event_rx) = unbounded();

        let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let db_dir = base.join("lumina");
        let db_path = db_dir.join("catalog.db");
        let catalog = Catalog::open(db_path)?;

        let cache = DiskCache::new()?;
        let decoder = DecoderPipeline::new(0)?;

        let watcher = LibraryWatcher::start(&library_root, event_tx.clone()).ok();
        let active_scan_cancel = Arc::new(AtomicBool::new(false));

        let engine = Arc::new(Self {
            library_root: library_root.clone(),
            catalog,
            cache,
            decoder,
            event_tx: event_tx.clone(),
            active_scan_cancel,
            _watcher: watcher,
        });

        // Trigger initial background scan
        let engine_clone = engine.clone();
        std::thread::Builder::new()
            .name("lumina-init-scan".to_string())
            .spawn(move || {
                engine_clone.run_scan();
            })?;

        Ok((engine, event_rx))
    }

    pub fn library_root(&self) -> &Path {
        &self.library_root
    }

    pub fn total_count(&self) -> usize {
        self.catalog.total_count().unwrap_or(0)
    }

    pub fn get_item_slice(&self, range: Range<usize>) -> Vec<MediaItem> {
        let offset = range.start;
        let limit = range.end.saturating_sub(range.start);
        if limit == 0 {
            return Vec::new();
        }
        self.catalog.get_item_slice(offset, limit).unwrap_or_default()
    }

    pub fn get_item_by_id(&self, id: MediaId) -> Option<MediaItem> {
        self.catalog.get_item_by_id(id).unwrap_or(None)
    }

    pub fn request_thumbnail<F>(&self, id: MediaId, target_size: u32, callback: F)
    where
        F: FnOnce(Option<DecodedFrame>) + Send + 'static,
    {
        let item = match self.catalog.get_item_by_id(id) {
            Ok(Some(it)) => it,
            _ => {
                callback(None);
                return;
            }
        };

        let cache = self.cache.clone();

        // Check disk cache first
        if let Some(frame) = cache.get_cached(
            id,
            &item.path,
            item.file_size,
            item.mtime_epoch_ms,
            target_size,
        ) {
            callback(Some(frame));
            return;
        }

        // Cache miss: dispatch decoding onto background Rayon thread
        self.decoder.spawn(move || {
            match DecoderPipeline::decode_media(id, item.media_type, &item.path, target_size) {
                Ok(frame) => {
                    let _ = cache.put_cached(
                        &item.path,
                        item.file_size,
                        item.mtime_epoch_ms,
                        target_size,
                        &frame,
                    );
                    callback(Some(frame));
                }
                Err(err) => {
                    eprintln!("Failed to decode thumbnail for {:?}: {:?}", item.path, err);
                    callback(None);
                }
            }
        });
    }

    pub fn rescan(&self) {
        self.active_scan_cancel.store(true, Ordering::Relaxed);
        let engine = self.clone_ref();
        std::thread::Builder::new()
            .name("lumina-rescan".to_string())
            .spawn(move || {
                engine.run_scan();
            })
            .ok();
    }

    fn clone_ref(&self) -> Arc<Self> {
        Arc::new(Self {
            library_root: self.library_root.clone(),
            catalog: self.catalog.clone(),
            cache: self.cache.clone(),
            decoder: DecoderPipeline::new(0).unwrap(),
            event_tx: self.event_tx.clone(),
            active_scan_cancel: Arc::new(AtomicBool::new(false)),
            _watcher: None,
        })
    }

    fn run_scan(&self) {
        let (batch_tx, batch_rx) = unbounded();
        let scanner = Scanner::new(self.library_root.clone(), batch_tx);

        let catalog = self.catalog.clone();
        let event_tx = self.event_tx.clone();

        // Ingestion worker thread to consume discovered items and commit into SQLite in batches
        let ingest_handle = std::thread::spawn(move || {
            for event in batch_rx {
                match event {
                    EngineEvent::BatchDiscovered { items } => {
                        if let Ok(inserted) = catalog.insert_or_update_batch(&items) {
                            let _ = event_tx.send(EngineEvent::BatchDiscovered { items: inserted });
                        }
                    }
                    EngineEvent::ScanProgress { items_found, is_complete } => {
                        let _ = event_tx.send(EngineEvent::ScanProgress {
                            items_found,
                            is_complete,
                        });
                    }
                    EngineEvent::ScanComplete { total_items } => {
                        let _ = event_tx.send(EngineEvent::ScanComplete { total_items });
                    }
                    _ => {}
                }
            }
        });

        let _ = scanner.scan();
        let _ = ingest_handle.join();
    }
}
