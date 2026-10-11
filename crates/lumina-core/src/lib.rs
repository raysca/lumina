pub mod cache;
pub mod catalog;
pub mod decoder;
pub mod geocoder;
pub mod scanner;
pub mod search;
pub mod smart_albums;
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
pub use geocoder::{GeoLocation, OfflineGeocoder};
pub use scanner::Scanner;
pub use search::StructuredQuery;
pub use smart_albums::{
    MatchMode, RuleField, RuleOperator, RuleValue, SmartAlbum, SmartAlbumRule, SystemAlbum,
};
pub use watcher::LibraryWatcher;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MediaId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaType {
    Image,
    Video,
    Raw,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaItem {
    pub id: MediaId,
    pub path: PathBuf,
    pub parent_dir: String,
    pub file_name: String,
    pub file_size: u64,
    pub mtime_epoch_ms: i64,

    // Dimensions & Media Classification
    pub media_type: MediaType,
    pub width: u32,
    pub height: u32,
    pub duration_ms: Option<u64>,
    pub duration_secs: f64,

    // EXIF Temporal & Spatial Attributes
    pub timestamp_epoch_ms: i64,
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub altitude: Option<f64>,

    // Reverse Geocoded Metadata
    pub country_code: Option<String>,
    pub admin1_region: Option<String>,
    pub city: Option<String>,

    // Camera Hardware Metadata
    pub make: Option<String>,
    pub model: Option<String>,
    pub lens: Option<String>,
    pub focal_length: Option<f64>,
    pub f_number: Option<f64>,
    pub iso: Option<u32>,
    pub exposure_time: Option<String>,

    // State Flags
    pub is_favorite: bool,
    pub is_hidden: bool,
    pub is_trashed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineBucket {
    pub bucket_type: String, // 'year', 'month', 'day'
    pub bucket_key: String,  // '2026', '2026-10', '2026-10-10'
    pub start_timestamp: i64,
    pub end_timestamp: i64,
    pub item_count: usize,
    pub cover_media_id: Option<MediaId>,
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

    pub fn get_timeline_buckets(&self, bucket_type: &str) -> Vec<TimelineBucket> {
        self.catalog.get_timeline_buckets(bucket_type).unwrap_or_default()
    }

    pub fn search_fts(&self, query: &str, limit: usize) -> Vec<MediaItem> {
        self.catalog.search_fts(query, limit).unwrap_or_default()
    }

    pub fn search_structured(&self, query: &StructuredQuery, limit: usize, offset: usize) -> Vec<MediaItem> {
        self.catalog.search_structured(query, limit, offset).unwrap_or_default()
    }

    pub fn search_structured_count(&self, query: &StructuredQuery) -> usize {
        self.catalog.search_structured_count(query).unwrap_or(0)
    }

    pub fn search_query(&self, input: &str, limit: usize, offset: usize) -> Vec<MediaItem> {
        let q = StructuredQuery::parse(input);
        self.search_structured(&q, limit, offset)
    }

    pub fn search_query_count(&self, input: &str) -> usize {
        let q = StructuredQuery::parse(input);
        self.search_structured_count(&q)
    }

    pub fn query_system_album(&self, album: SystemAlbum, limit: usize, offset: usize) -> Vec<MediaItem> {
        self.catalog.query_system_album(album, limit, offset).unwrap_or_default()
    }

    pub fn query_system_album_count(&self, album: SystemAlbum) -> usize {
        self.catalog.query_system_album_count(album).unwrap_or(0)
    }

    pub fn list_smart_albums(&self) -> Vec<SmartAlbum> {
        self.catalog.list_smart_albums().unwrap_or_default()
    }

    pub fn create_smart_album(
        &self,
        title: &str,
        icon_name: &str,
        match_mode: MatchMode,
        rules: &[SmartAlbumRule],
    ) -> Result<SmartAlbum> {
        self.catalog.create_smart_album(title, icon_name, match_mode, rules)
    }

    pub fn delete_smart_album(&self, id: i64) -> Result<bool> {
        self.catalog.delete_smart_album(id)
    }

    pub fn query_smart_album(&self, album: &SmartAlbum, limit: usize, offset: usize) -> Vec<MediaItem> {
        self.catalog.query_smart_album(album, limit, offset).unwrap_or_default()
    }

    pub fn query_smart_album_count(&self, album: &SmartAlbum) -> usize {
        self.catalog.query_smart_album_count(album).unwrap_or(0)
    }

    pub fn toggle_favorite(&self, id: MediaId, is_favorite: bool) -> Result<()> {
        self.catalog.toggle_favorite(id, is_favorite)
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
                        // Automatically recompute timeline buckets upon scan completion
                        let _ = catalog.compute_timeline_buckets();
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
