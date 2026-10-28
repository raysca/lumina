pub mod cache;
pub mod catalog;
pub mod decoder;
pub mod geocoder;
pub mod scanner;
pub mod search;
pub mod smart_albums;
pub mod sort;
pub mod watcher;
pub mod xmp;

use anyhow::Result;
use crossbeam_channel::{unbounded, Receiver, Sender};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub use cache::DiskCache;
pub use catalog::{Catalog, DeletionTarget, SCHEMA_VERSION};
pub use decoder::DecoderPipeline;
pub use geocoder::{GeoLocation, OfflineGeocoder};
pub use scanner::{extract_video_duration, Scanner};
pub use search::StructuredQuery;
pub use smart_albums::{
    MatchMode, RuleField, RuleOperator, RuleValue, SmartAlbum, SmartAlbumRule, SystemAlbum,
};
pub use sort::{SortDirection, SortField, SortSpec};
pub use watcher::LibraryWatcher;
pub use xmp::{parse_xmp, read_sidecar, render_xmp, sidecar_path, write_sidecar, SidecarMetadata};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MediaId(pub u64);

/// Never overwrite an existing export: photo.jpg -> photo (1).jpg, photo (2).jpg...
fn unique_destination(directory: &Path, file_name: &str) -> PathBuf {
    let candidate = directory.join(file_name);
    if !candidate.exists() {
        return candidate;
    }
    let source = Path::new(file_name);
    let stem = source
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "export".to_string());
    let extension = source.extension().map(|e| e.to_string_lossy().to_string());
    for index in 1..10_000 {
        let name = match extension.as_deref() {
            Some(ext) => format!("{stem} ({index}).{ext}"),
            None => format!("{stem} ({index})"),
        };
        let candidate = directory.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }
    directory.join(format!("{stem}-{}.export", std::process::id()))
}

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
    /// User-editable title; falls back to a derived display title when absent.
    pub title: Option<String>,
    pub file_size: u64,
    pub mtime_epoch_ms: i64,

    // Dimensions & Media Classification
    pub media_type: MediaType,
    pub width: u32,
    pub height: u32,
    pub duration_ms: Option<u64>,

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

impl MediaItem {
    /// Returns duration in seconds derived from `duration_ms`.
    pub fn duration_secs(&self) -> f64 {
        self.duration_ms.map(|ms| ms as f64 / 1000.0).unwrap_or(0.0)
    }

    /// Formats `timestamp_epoch_ms` as a human-readable date/time string.
    pub fn formatted_timestamp(&self) -> String {
        chrono::DateTime::from_timestamp_millis(self.timestamp_epoch_ms)
            .map(|dt| {
                dt.format("%e %b %Y, %H:%M:%S")
                    .to_string()
                    .trim()
                    .to_string()
            })
            .unwrap_or_else(|| format!("{:04}-{:02}-{:02}", self.year, self.month, self.day))
    }

    /// Formats `mtime_epoch_ms` as a human-readable date/time string.
    pub fn formatted_mtime(&self) -> String {
        chrono::DateTime::from_timestamp_millis(self.mtime_epoch_ms)
            .map(|dt| dt.format("%e %b %Y, %H:%M").to_string().trim().to_string())
            .unwrap_or_else(|| format!("{:04}-{:02}-{:02}", self.year, self.month, self.day))
    }
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
#[allow(clippy::large_enum_variant)]
pub enum EngineEvent {
    BatchDiscovered {
        items: Vec<MediaItem>,
    },
    ScanProgress {
        items_found: usize,
        is_complete: bool,
    },
    ItemRemoved(MediaId),
    ScanComplete {
        total_items: usize,
    },
}

/// Result of exporting media + metadata.
#[derive(Debug, Default, Clone)]
pub struct ExportReport {
    pub exported: usize,
    pub sidecars: usize,
    pub skipped: usize,
    pub failed: Vec<(PathBuf, String)>,
}

/// Result of importing XMP sidecars.
#[derive(Debug, Default, Clone)]
pub struct ImportReport {
    pub imported: usize,
    pub updated: usize,
    pub skipped: usize,
    pub failed: Vec<(PathBuf, String)>,
}

#[derive(Clone)]
pub struct LuminaEngine {
    library_root: PathBuf,
    catalog: Catalog,
    cache: DiskCache,
    decoder: DecoderPipeline,
    event_tx: Sender<EngineEvent>,
    active_scan_cancel: Arc<Mutex<Arc<AtomicBool>>>,
    _watcher: Option<LibraryWatcher>,
}

impl LuminaEngine {
    pub fn open(library_root: PathBuf) -> Result<(Arc<Self>, Receiver<EngineEvent>)> {
        let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let db_dir = base.join("lumina");
        let canonical = library_root
            .canonicalize()
            .unwrap_or_else(|_| library_root.clone());
        let hash = xxhash_rust::xxh3::xxh3_64(canonical.to_string_lossy().as_bytes());
        let db_path = db_dir.join(format!("catalog_{:016x}.db", hash));
        Self::open_with_paths(library_root, db_path, None)
    }

    pub fn open_with_paths(
        library_root: PathBuf,
        db_path: PathBuf,
        cache_dir: Option<PathBuf>,
    ) -> Result<(Arc<Self>, Receiver<EngineEvent>)> {
        let (event_tx, event_rx) = unbounded();

        let db_existed = db_path.exists();
        let catalog = Catalog::open(db_path)?;
        if db_existed {
            match catalog.quick_check() {
                Ok(()) => log::info!("catalog integrity check passed"),
                Err(err) => log::error!("catalog integrity check failed: {err:?}"),
            }
            match catalog.daily_backup() {
                Ok(Some(path)) => log::info!("catalog backup created at {}", path.display()),
                Ok(None) => {}
                Err(err) => log::warn!("catalog backup failed: {err:?}"),
            }
        }

        let cache = match cache_dir {
            Some(dir) => DiskCache::with_directory(dir)?,
            None => DiskCache::new()?,
        };
        {
            let cache = cache.clone();
            let _ = std::thread::Builder::new()
                .name("lumina-cache-cleanup".to_string())
                .spawn(move || {
                    match cache.cleanup(
                        crate::cache::DEFAULT_MAX_CACHE_BYTES,
                        crate::cache::DEFAULT_MAX_CACHE_AGE,
                    ) {
                        Ok(stats) if stats.removed_files > 0 => {
                            log::info!(
                                "thumbnail cache cleanup removed {} files ({} bytes), {} remaining",
                                stats.removed_files,
                                stats.removed_bytes,
                                stats.remaining_bytes
                            );
                        }
                        Ok(_) => {}
                        Err(err) => log::warn!("thumbnail cache cleanup failed: {err:?}"),
                    }
                });
        }
        let decoder = DecoderPipeline::new(0)?;

        let cancel_flag = Arc::new(AtomicBool::new(false));
        let active_scan_cancel = Arc::new(Mutex::new(cancel_flag.clone()));

        // Start the real-time filesystem watcher alongside the initial scan so
        // external add/modify/delete operations are reflected without a manual
        // rescan. It lives as long as the engine (or any surviving clone).
        let watcher =
            match LibraryWatcher::new(library_root.clone(), catalog.clone(), event_tx.clone()) {
                Ok(watcher) => Some(watcher),
                Err(err) => {
                    log::warn!("real-time filesystem watcher unavailable: {err:?}");
                    None
                }
            };

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
                engine_clone.run_scan(cancel_flag);
            })?;

        Ok((engine, event_rx))
    }

    pub fn library_root(&self) -> &Path {
        &self.library_root
    }

    pub fn catalog(&self) -> &Catalog {
        &self.catalog
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
        self.catalog
            .get_item_slice(offset, limit)
            .unwrap_or_default()
    }

    pub fn get_item_slice_sorted(
        &self,
        offset: usize,
        limit: usize,
        sort: SortSpec,
    ) -> Vec<MediaItem> {
        self.catalog
            .get_item_slice_sorted(offset, limit, sort)
            .unwrap_or_default()
    }

    pub fn get_item_by_id(&self, id: MediaId) -> Option<MediaItem> {
        self.catalog.get_item_by_id(id).unwrap_or(None)
    }

    pub fn get_timeline_buckets(&self, bucket_type: &str) -> Vec<TimelineBucket> {
        self.catalog
            .get_timeline_buckets(bucket_type)
            .unwrap_or_default()
    }

    pub fn search_structured(
        &self,
        query: &StructuredQuery,
        limit: usize,
        offset: usize,
    ) -> Vec<MediaItem> {
        self.catalog
            .search_structured(query, limit, offset)
            .unwrap_or_default()
    }

    pub fn search_structured_count(&self, query: &StructuredQuery) -> usize {
        self.catalog.search_structured_count(query).unwrap_or(0)
    }

    pub fn search_query(&self, input: &str, limit: usize, offset: usize) -> Vec<MediaItem> {
        self.search_query_sorted(input, limit, offset, SortSpec::default())
    }

    pub fn search_query_sorted(
        &self,
        input: &str,
        limit: usize,
        offset: usize,
        sort: SortSpec,
    ) -> Vec<MediaItem> {
        let trimmed = input.trim();
        let q = StructuredQuery::parse(trimmed);
        self.catalog
            .search_structured_sorted(&q, limit, offset, sort)
            .unwrap_or_default()
    }

    pub fn search_query_count(&self, input: &str) -> usize {
        let trimmed = input.trim();
        let q = StructuredQuery::parse(trimmed);
        self.search_structured_count(&q)
    }

    pub fn query_system_album(
        &self,
        album: SystemAlbum,
        limit: usize,
        offset: usize,
    ) -> Vec<MediaItem> {
        self.catalog
            .query_system_album(album, limit, offset)
            .unwrap_or_default()
    }

    pub fn query_system_album_sorted(
        &self,
        album: SystemAlbum,
        limit: usize,
        offset: usize,
        sort: SortSpec,
    ) -> Vec<MediaItem> {
        self.catalog
            .query_system_album_sorted(album, limit, offset, sort)
            .unwrap_or_default()
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
        self.catalog
            .create_smart_album(title, icon_name, match_mode, rules)
    }

    pub fn update_smart_album(
        &self,
        id: i64,
        title: &str,
        icon_name: &str,
        match_mode: MatchMode,
        rules: &[SmartAlbumRule],
    ) -> Result<bool> {
        self.catalog
            .update_smart_album(id, title, icon_name, match_mode, rules)
    }

    pub fn delete_smart_album(&self, id: i64) -> Result<bool> {
        self.catalog.delete_smart_album(id)
    }

    pub fn query_smart_album(
        &self,
        album: &SmartAlbum,
        limit: usize,
        offset: usize,
    ) -> Vec<MediaItem> {
        self.catalog
            .query_smart_album(album, limit, offset)
            .unwrap_or_default()
    }

    pub fn query_smart_album_sorted(
        &self,
        album: &SmartAlbum,
        limit: usize,
        offset: usize,
        sort: SortSpec,
    ) -> Vec<MediaItem> {
        self.catalog
            .query_smart_album_sorted(album, limit, offset, sort)
            .unwrap_or_default()
    }

    pub fn query_smart_album_count(&self, album: &SmartAlbum) -> usize {
        self.catalog.query_smart_album_count(album).unwrap_or(0)
    }

    pub fn toggle_favorite(&self, id: MediaId, is_favorite: bool) -> Result<()> {
        self.catalog.toggle_favorite(id, is_favorite)
    }

    pub fn batch_set_favorite(&self, ids: &[MediaId], is_favorite: bool) -> Result<()> {
        self.catalog.batch_set_favorite(ids, is_favorite)
    }

    pub fn set_rating(&self, id: MediaId, rating: u8) -> Result<()> {
        self.catalog.set_rating(id, rating)
    }

    pub fn get_ratings_map(&self) -> std::collections::HashMap<MediaId, u8> {
        self.catalog.get_ratings_map().unwrap_or_default()
    }

    pub fn set_color_label(&self, id: MediaId, label: u8) -> Result<()> {
        self.catalog.set_color_label(id, label)
    }

    pub fn set_title(&self, id: MediaId, title: Option<&str>) -> Result<()> {
        self.catalog.set_title(id, title)
    }

    pub fn set_timestamp(&self, id: MediaId, epoch_ms: i64) -> Result<()> {
        self.catalog.set_timestamp(id, epoch_ms)
    }

    pub fn set_location(
        &self,
        id: MediaId,
        latitude: Option<f64>,
        longitude: Option<f64>,
    ) -> Result<()> {
        self.catalog.set_location(id, latitude, longitude)
    }

    pub fn get_color_labels_map(&self) -> std::collections::HashMap<MediaId, u8> {
        self.catalog.get_color_labels_map().unwrap_or_default()
    }

    pub fn set_caption(&self, id: MediaId, caption: &str) -> Result<()> {
        self.catalog.set_caption(id, caption)
    }

    /// Copies the given items into destination, writing an XMP sidecar for each
    /// item that has rating/tags/caption metadata. Existing files are never
    /// overwritten; collisions get a numbered suffix.
    pub fn export_items(&self, ids: &[MediaId], destination: &Path) -> Result<ExportReport> {
        fs::create_dir_all(destination)?;
        let captions = self.catalog.get_captions_map().unwrap_or_default();
        let ratings = self.catalog.get_ratings_map().unwrap_or_default();
        let mut report = ExportReport::default();

        for id in ids {
            let Some(item) = self.catalog.get_item_by_id(*id)? else {
                report.skipped += 1;
                continue;
            };
            if !item.path.exists() {
                report
                    .failed
                    .push((item.path.clone(), "source file is missing".to_string()));
                continue;
            }
            let destination_path = unique_destination(destination, &item.file_name);
            if let Err(err) = fs::copy(&item.path, &destination_path) {
                report
                    .failed
                    .push((item.path.clone(), format!("could not copy file: {err}")));
                continue;
            }
            report.exported += 1;

            match self.sidecar_metadata_for(*id, &captions, &ratings) {
                Ok(metadata) => match xmp::write_sidecar(&destination_path, &metadata) {
                    Ok(Some(_)) => report.sidecars += 1,
                    Ok(None) => {}
                    Err(err) => report.failed.push((
                        item.path.clone(),
                        format!("could not write XMP sidecar: {err}"),
                    )),
                },
                Err(err) => report
                    .failed
                    .push((item.path.clone(), format!("could not read metadata: {err}"))),
            }
        }
        Ok(report)
    }

    /// Writes/updates XMP sidecars next to the original files (metadata-only export).
    pub fn write_metadata_sidecars(&self, ids: &[MediaId]) -> Result<usize> {
        let captions = self.catalog.get_captions_map().unwrap_or_default();
        let ratings = self.catalog.get_ratings_map().unwrap_or_default();
        let mut written = 0;
        for id in ids {
            let Some(item) = self.catalog.get_item_by_id(*id)? else {
                continue;
            };
            let metadata = self.sidecar_metadata_for(*id, &captions, &ratings)?;
            if xmp::write_sidecar(&item.path, &metadata)?.is_some() {
                written += 1;
            }
        }
        Ok(written)
    }

    /// Imports XMP sidecars for the given items. Only values present in the
    /// sidecar are applied (ratings always, title/caption when set, tags merged),
    /// so importing never clears metadata that the sidecar does not mention.
    pub fn import_sidecars(&self, ids: &[MediaId]) -> Result<ImportReport> {
        let captions = self.catalog.get_captions_map().unwrap_or_default();
        let ratings = self.catalog.get_ratings_map().unwrap_or_default();
        let mut report = ImportReport::default();

        for id in ids {
            let Some(item) = self.catalog.get_item_by_id(*id)? else {
                report.skipped += 1;
                continue;
            };
            match xmp::read_sidecar(&item.path) {
                Ok(Some(metadata)) => {
                    report.imported += 1;
                    let mut changed = false;

                    if metadata.title.is_some() && metadata.title != item.title {
                        self.catalog.set_title(*id, metadata.title.as_deref())?;
                        changed = true;
                    }
                    let current_rating = ratings.get(id).copied().unwrap_or(0);
                    if metadata.rating != current_rating {
                        self.catalog.set_rating(*id, metadata.rating)?;
                        changed = true;
                    }
                    if metadata.caption.is_some() && metadata.caption != captions.get(id).cloned() {
                        self.catalog
                            .set_caption(*id, metadata.caption.as_deref().unwrap_or(""))?;
                        changed = true;
                    }
                    if !metadata.tags.is_empty() {
                        self.catalog.add_tags_batch(&[*id], &metadata.tags)?;
                        changed = true;
                    }
                    if changed {
                        report.updated += 1;
                    }
                }
                Ok(None) => report.skipped += 1,
                Err(err) => report
                    .failed
                    .push((item.path.clone(), format!("could not read sidecar: {err}"))),
            }
        }
        Ok(report)
    }

    fn sidecar_metadata_for(
        &self,
        id: MediaId,
        captions: &HashMap<MediaId, String>,
        ratings: &HashMap<MediaId, u8>,
    ) -> Result<SidecarMetadata> {
        let tags = self.catalog.get_tags_for_item(id)?;
        let title = self.catalog.get_item_by_id(id)?.and_then(|item| item.title);
        Ok(SidecarMetadata {
            title,
            caption: captions.get(&id).cloned(),
            rating: ratings.get(&id).copied().unwrap_or(0),
            tags,
        })
    }

    pub fn get_captions_map(&self) -> std::collections::HashMap<MediaId, String> {
        self.catalog.get_captions_map().unwrap_or_default()
    }

    pub fn set_trashed(&self, id: MediaId, is_trashed: bool) -> Result<()> {
        self.catalog.set_trashed(id, is_trashed)
    }

    pub fn batch_set_trashed(&self, ids: &[MediaId], is_trashed: bool) -> Result<()> {
        self.catalog.batch_set_trashed(ids, is_trashed)
    }

    pub fn restore_trashed_items(&self, ids: &[MediaId]) -> Result<usize> {
        self.catalog.restore_trashed_items(ids)
    }

    pub fn restore_all_trash(&self) -> Result<usize> {
        self.catalog.restore_all_trash()
    }

    pub fn permanently_delete_items(
        &self,
        ids: &[MediaId],
        target: DeletionTarget,
    ) -> Result<usize> {
        let items: Vec<MediaItem> = ids
            .iter()
            .filter_map(|id| self.catalog.get_item_by_id(*id).ok().flatten())
            .collect();

        let count = self.catalog.permanently_delete_items(ids, target)?;

        for item in &items {
            self.cache
                .remove_cached_item(&item.path, item.file_size, item.mtime_epoch_ms);
        }
        for id in ids {
            let _ = self.event_tx.send(EngineEvent::ItemRemoved(*id));
        }
        Ok(count)
    }

    pub fn empty_trash(&self, target: DeletionTarget) -> Result<usize> {
        let trashed_items = self
            .catalog
            .query_system_album(SystemAlbum::RecentlyDeleted, 100_000, 0)
            .unwrap_or_default();

        let removed_ids = self.catalog.empty_trash(target)?;
        let count = removed_ids.len();

        for item in &trashed_items {
            self.cache
                .remove_cached_item(&item.path, item.file_size, item.mtime_epoch_ms);
        }
        for id in removed_ids {
            let _ = self.event_tx.send(EngineEvent::ItemRemoved(id));
        }
        Ok(count)
    }

    pub fn add_tags_batch(&self, ids: &[MediaId], tags: &[String]) -> Result<()> {
        self.catalog.add_tags_batch(ids, tags)
    }

    pub fn remove_tags_batch(&self, ids: &[MediaId], tags: &[String]) -> Result<()> {
        self.catalog.remove_tags_batch(ids, tags)
    }

    pub fn get_tags_for_item(&self, id: MediaId) -> Vec<String> {
        self.catalog.get_tags_for_item(id).unwrap_or_default()
    }

    pub fn get_tags_for_items_batch(&self, ids: &[MediaId]) -> Vec<String> {
        self.catalog
            .get_tags_for_items_batch(ids)
            .unwrap_or_default()
    }

    pub fn query_tag_counts(&self) -> Vec<(String, usize)> {
        self.catalog.query_tag_counts().unwrap_or_default()
    }

    pub fn rename_tag(&self, old_tag: &str, new_tag: &str) -> Result<usize> {
        self.catalog.rename_tag(old_tag, new_tag)
    }

    pub fn delete_tag_globally(&self, tag: &str) -> Result<usize> {
        self.catalog.delete_tag_globally(tag)
    }

    pub fn merge_tags(&self, source_tag: &str, target_tag: &str) -> Result<usize> {
        self.catalog.merge_tags(source_tag, target_tag)
    }

    pub fn request_thumbnail<F>(&self, id: MediaId, target_size: u32, callback: F)
    where
        F: FnOnce(Option<DecodedFrame>) + Send + 'static,
    {
        let catalog = self.catalog.clone();
        let cache = self.cache.clone();

        // Dispatch catalog lookup, disk cache check, and decoding onto background Rayon thread
        // so the UI thread never blocks on SQLite mutexes or disk I/O.
        self.decoder.spawn(move || {
            let item = match catalog.get_item_by_id(id) {
                Ok(Some(it)) => it,
                _ => {
                    callback(None);
                    return;
                }
            };

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
                    log::warn!("failed to decode thumbnail for {:?}: {err:?}", item.path);
                    callback(None);
                }
            }
        });
    }

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
        std::thread::Builder::new()
            .name("lumina-rescan".to_string())
            .spawn(move || {
                engine.run_scan(cancel_flag);
            })
            .ok();
    }

    fn run_scan(&self, cancel_flag: Arc<AtomicBool>) {
        let (batch_tx, batch_rx) = unbounded();
        let known_signatures = self.catalog.get_file_signatures().unwrap_or_default();
        let scanner =
            Scanner::new_with_cancel(self.library_root.clone(), batch_tx, cancel_flag.clone())
                .with_known_signatures(known_signatures);

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
                    EngineEvent::ScanProgress {
                        items_found,
                        is_complete,
                    } => {
                        let _ = event_tx.send(EngineEvent::ScanProgress {
                            items_found,
                            is_complete,
                        });
                    }
                    EngineEvent::ScanComplete { .. } => {
                        break;
                    }
                    _ => {}
                }
            }
        });

        let scan_res = scanner.scan_and_collect_paths();
        let _ = ingest_handle.join();

        if let Ok((total_items, all_scanned_paths)) = scan_res {
            if !cancel_flag.load(Ordering::Relaxed) {
                // Reconcile deleted files on disk: remove catalog entries that no longer exist
                if let Ok(removed_ids) = self.catalog.remove_items_not_in(&all_scanned_paths) {
                    for id in removed_ids {
                        let _ = self.event_tx.send(EngineEvent::ItemRemoved(id));
                    }
                }
            }
            // Automatically recompute timeline buckets upon scan completion
            let _ = self.catalog.compute_timeline_buckets();
            let _ = self
                .event_tx
                .send(EngineEvent::ScanComplete { total_items });
        }
    }
}
