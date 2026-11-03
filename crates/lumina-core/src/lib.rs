pub mod cache;
pub mod catalog;
pub mod decoder;
pub mod duplicates;
pub mod geocoder;
pub mod import;
pub mod net_import;
pub mod scanner;
pub mod search;
pub mod smart_albums;
pub mod sort;
pub mod text;
pub mod watcher;
pub mod xmp;

use anyhow::{Context, Result};
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
pub use duplicates::{compute_file_hash, DuplicateCandidate, DuplicateGroup, DuplicateReport};
pub use geocoder::{GeoLocation, OfflineGeocoder};
pub use scanner::{extract_video_duration, extract_video_size, Scanner};
pub use search::StructuredQuery;
pub use smart_albums::{
    parse_rule_date, split_date_range, MatchMode, RuleField, RuleOperator, RuleValue, SmartAlbum,
    SmartAlbumRule, SystemAlbum,
};
pub use sort::{SortDirection, SortField, SortSpec};
pub use watcher::LibraryWatcher;
pub use xmp::{parse_xmp, read_sidecar, render_xmp, sidecar_path, write_sidecar, SidecarMetadata};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MediaId(pub u64);

/// (latitude, longitude) for a location edit; `None` clears it.
pub type Coordinates = (Option<f64>, Option<f64>);

/// Viewer rotation is stored as clockwise quarter turns in 0..=3.
pub const QUARTER_TURNS: u8 = 4;

/// Normalizes any signed quarter-turn count into the stored 0..=3 range.
///
/// Rotate Left subtracts a turn from 0, which must wrap to 3 instead of
/// underflowing; a full four turns is the identity.
pub fn normalize_quarter_turns(turns: i32) -> u8 {
    turns.rem_euclid(QUARTER_TURNS as i32) as u8
}

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
                dt.format("%a %-d %b %Y · %H:%M")
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
        /// Media files under the root, counted before the walk (0 = unknown).
        total: usize,
        /// Media-named files that could not be read.
        skipped: usize,
        /// The file the scan has most recently reached.
        current_file: Option<PathBuf>,
    },
    ItemRemoved(MediaId),
    /// Files gone from disk. The rows and their metadata are kept, marked
    /// missing; the order of the library is unchanged.
    ItemsMissing(Vec<MediaId>),
    /// Files an import plan has read so far, of `total`; duplicates are
    /// checked once `checked == total`.
    ImportChecking {
        checked: usize,
        total: usize,
    },
    /// Files a running import has copied or failed so far, of `total`.
    ImportProgress {
        done: usize,
        total: usize,
    },
    ScanComplete {
        total_items: usize,
    },
    /// A catalog or scan failure for the error banner, as "Title: detail".
    Error(String),
}

/// What [`LuminaEngine::relink`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelinkOutcome {
    /// The missing item now points at the chosen file.
    Relinked,
    /// The chosen file was already catalogued as this item (now deleted);
    /// its metadata was merged into the missing item, which took its file.
    Folded(MediaId),
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
    read_error_reported: Arc<AtomicBool>,
    /// Serializes sidecar passes. Each pass reads the catalog inside it, so
    /// the last pass to run writes the newest values even when passes race.
    sidecar_writes: Arc<Mutex<()>>,
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
        // Staging left in Imports/ is from a crash or a quit mid-download:
        // nothing of this library's runs yet. Never clean it per job, since
        // several imports and downloads run at once.
        crate::import::remove_stale_temps(&library_root.join(crate::import::IMPORTS_DIR));
        let decoder = DecoderPipeline::new(0)?;

        let cancel_flag = Arc::new(AtomicBool::new(false));
        let active_scan_cancel = Arc::new(Mutex::new(cancel_flag.clone()));

        // Start the real-time filesystem watcher alongside the initial scan so
        // external add/modify/delete operations are reflected without a manual
        // rescan. It lives as long as the engine (or any surviving clone).
        // A watcher error means events were lost, so it rescans. The engine
        // owns the watcher, so the hook holds it weakly. Untested: notify
        // errors cannot be triggered on demand.
        // ponytail: back-to-back errors each restart the scan (rescan cancels
        // the running one); debounce here if an error storm is ever seen.
        let rescan_target: Arc<std::sync::OnceLock<std::sync::Weak<Self>>> = Arc::default();
        let on_error = {
            let rescan_target = rescan_target.clone();
            Box::new(move || {
                if let Some(engine) = rescan_target.get().and_then(std::sync::Weak::upgrade) {
                    engine.rescan();
                }
            })
        };
        let watcher = match LibraryWatcher::new(
            library_root.clone(),
            catalog.clone(),
            event_tx.clone(),
            on_error,
        ) {
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
            read_error_reported: Arc::new(AtomicBool::new(false)),
            sidecar_writes: Arc::new(Mutex::new(())),
            _watcher: watcher,
        });

        let _ = rescan_target.set(Arc::downgrade(&engine));

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

    /// Unwraps a catalog read for the display queries. A failure is logged
    /// and the first one per library raises [`EngineEvent::Error`], so a
    /// broken catalog shows a banner instead of an empty library.
    /// ponytail: one banner per engine; later failures only log, since
    /// re-raising on every frame would loop the renderer.
    fn read<T: Default>(&self, what: &str, result: Result<T>) -> T {
        result.unwrap_or_else(|err| {
            log::error!("catalog read failed ({what}): {err:#}");
            if !self.read_error_reported.swap(true, Ordering::Relaxed) {
                let _ = self.event_tx.send(EngineEvent::Error(format!(
                    "Could not read the catalog: {err:#}"
                )));
            }
            T::default()
        })
    }

    /// Lets the next failed catalog read raise a banner again; called when
    /// the user dismisses one, which cannot loop the renderer.
    pub fn rearm_read_errors(&self) {
        self.read_error_reported.store(false, Ordering::Relaxed);
    }

    /// Moves whenever the catalog changes; see [`Catalog::revision`].
    pub fn data_revision(&self) -> u64 {
        self.catalog.revision()
    }

    /// See [`Catalog::revision_written_here`].
    pub fn revision_written_here(&self) -> u64 {
        self.catalog.revision_written_here()
    }

    /// Runs `f` while the catalog lock is held; see [`Catalog::with_lock_held`].
    #[cfg(feature = "test-support")]
    pub fn with_catalog_locked(&self, f: impl FnOnce()) {
        self.catalog.with_lock_held(f);
    }

    pub fn total_count(&self) -> usize {
        self.read("total count", self.catalog.total_count())
    }

    pub fn get_item_slice(&self, range: Range<usize>) -> Vec<MediaItem> {
        let offset = range.start;
        let limit = range.end.saturating_sub(range.start);
        if limit == 0 {
            return Vec::new();
        }
        self.read("get item slice", self.catalog.get_item_slice(offset, limit))
    }

    pub fn get_item_slice_sorted(
        &self,
        offset: usize,
        limit: usize,
        sort: SortSpec,
    ) -> Vec<MediaItem> {
        self.read(
            "get item slice sorted",
            self.catalog.get_item_slice_sorted(offset, limit, sort),
        )
    }

    pub fn get_item_by_id(&self, id: MediaId) -> Option<MediaItem> {
        self.read("get item by id", self.catalog.get_item_by_id(id))
    }

    pub fn get_timeline_buckets(&self, bucket_type: &str) -> Vec<TimelineBucket> {
        self.read(
            "get timeline buckets",
            self.catalog.get_timeline_buckets(bucket_type),
        )
    }

    pub fn search_structured(
        &self,
        query: &StructuredQuery,
        limit: usize,
        offset: usize,
    ) -> Vec<MediaItem> {
        self.read(
            "search structured",
            self.catalog.search_structured(query, limit, offset),
        )
    }

    pub fn search_structured_count(&self, query: &StructuredQuery) -> usize {
        self.read(
            "search structured count",
            self.catalog.search_structured_count(query),
        )
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
        self.read(
            "search structured sorted",
            self.catalog
                .search_structured_sorted(&q, limit, offset, sort),
        )
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
        self.read(
            "query system album",
            self.catalog.query_system_album(album, limit, offset),
        )
    }

    pub fn query_system_album_sorted(
        &self,
        album: SystemAlbum,
        limit: usize,
        offset: usize,
        sort: SortSpec,
    ) -> Vec<MediaItem> {
        self.read(
            "query system album sorted",
            self.catalog
                .query_system_album_sorted(album, limit, offset, sort),
        )
    }

    pub fn query_system_album_count(&self, album: SystemAlbum) -> usize {
        self.read(
            "query system album count",
            self.catalog.query_system_album_count(album),
        )
    }

    pub fn list_smart_albums(&self) -> Vec<SmartAlbum> {
        self.catalog.list_smart_albums().unwrap_or_else(|err| {
            log::warn!("listing smart albums failed: {err:#}");
            Vec::new()
        })
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

    /// Restores a deleted smart album under its original id (undo).
    pub fn restore_smart_album(&self, album: &SmartAlbum) -> Result<()> {
        self.catalog.restore_smart_album(album)
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
            .unwrap_or_else(|err| {
                log::warn!("smart album {:?} cannot run: {err:#}", album.title);
                Vec::new()
            })
    }

    pub fn query_smart_album_sorted(
        &self,
        album: &SmartAlbum,
        limit: usize,
        offset: usize,
        sort: SortSpec,
    ) -> Vec<MediaItem> {
        self.read(
            "query smart album sorted",
            self.catalog
                .query_smart_album_sorted(album, limit, offset, sort),
        )
    }

    pub fn query_smart_album_count(&self, album: &SmartAlbum) -> usize {
        self.catalog
            .query_smart_album_count(album)
            .unwrap_or_else(|err| {
                log::warn!("smart album {:?} cannot be counted: {err:#}", album.title);
                0
            })
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

    /// Batch edits: each `set_*s` writes every change in one transaction.
    pub fn set_ratings(&self, changes: &[(MediaId, u8)]) -> Result<()> {
        self.catalog.set_ratings(changes)
    }

    pub fn get_ratings_map(&self) -> std::collections::HashMap<MediaId, u8> {
        self.read("get ratings map", self.catalog.get_ratings_map())
    }

    pub fn set_color_label(&self, id: MediaId, label: u8) -> Result<()> {
        self.catalog.set_color_label(id, label)
    }

    pub fn set_color_labels(&self, changes: &[(MediaId, u8)]) -> Result<()> {
        self.catalog.set_color_labels(changes)
    }

    /// Sets the persisted clockwise quarter-turn rotation for every id.
    ///
    /// Rotation is display-only: the original file is never rewritten. The
    /// catalog write is one transaction, and cached thumbnails for the affected
    /// items are dropped because the cache key has no rotation component, so an
    /// unchanged key would otherwise keep serving the previous orientation.
    pub fn set_rotation(&self, ids: &[MediaId], quarters: u8) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let mut affected = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(item) = self.catalog.get_item_by_id(*id)? {
                affected.push(item);
            }
        }
        self.catalog.set_rotation(ids, quarters)?;
        for item in &affected {
            self.cache
                .remove_cached_item(&item.path, item.file_size, item.mtime_epoch_ms);
        }
        Ok(())
    }

    /// Persisted clockwise quarter turns for an item (0 when unset).
    pub fn get_rotation(&self, id: MediaId) -> Result<u8> {
        self.catalog.get_rotation(id)
    }

    pub fn set_title(&self, id: MediaId, title: Option<&str>) -> Result<()> {
        self.catalog.set_title(id, title)
    }

    pub fn set_titles(&self, changes: &[(MediaId, Option<&str>)]) -> Result<()> {
        self.catalog.set_titles(changes)
    }

    pub fn set_timestamp(&self, id: MediaId, epoch_ms: i64) -> Result<()> {
        self.catalog.set_timestamp(id, epoch_ms)
    }

    pub fn set_timestamps(&self, changes: &[(MediaId, i64)]) -> Result<()> {
        self.catalog.set_timestamps(changes)
    }

    pub fn set_location(
        &self,
        id: MediaId,
        latitude: Option<f64>,
        longitude: Option<f64>,
    ) -> Result<()> {
        self.catalog.set_location(id, latitude, longitude)
    }

    pub fn set_locations(&self, changes: &[(MediaId, Coordinates)]) -> Result<()> {
        self.catalog.set_locations(changes)
    }

    pub fn get_color_labels_map(&self) -> std::collections::HashMap<MediaId, u8> {
        self.read("get color labels map", self.catalog.get_color_labels_map())
    }

    pub fn set_caption(&self, id: MediaId, caption: &str) -> Result<()> {
        self.catalog.set_caption(id, caption)
    }

    pub fn set_captions(&self, changes: &[(MediaId, &str)]) -> Result<()> {
        self.catalog.set_captions(changes)
    }

    /// Copies the given items into destination, writing an XMP sidecar for each
    /// item that has rating/tags/caption metadata. Existing files are never
    /// overwritten; collisions get a numbered suffix.
    pub fn export_items(&self, ids: &[MediaId], destination: &Path) -> Result<ExportReport> {
        fs::create_dir_all(destination)?;
        let captions = self
            .catalog
            .get_captions_map()
            .context("could not read captions")?;
        let ratings = self
            .catalog
            .get_ratings_map()
            .context("could not read ratings")?;
        let labels = self
            .catalog
            .get_color_labels_map()
            .context("could not read color labels")?;
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

            match self.sidecar_metadata_for(*id, &captions, &ratings, &labels) {
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

    /// Writes/updates XMP sidecars next to the original files (metadata-only
    /// export). Safe to call from any thread; passes run one at a time.
    pub fn write_metadata_sidecars(&self, ids: &[MediaId]) -> Result<usize> {
        let _pass = self.sidecar_writes.lock();
        let captions = self
            .catalog
            .get_captions_map()
            .context("could not read captions")?;
        let ratings = self
            .catalog
            .get_ratings_map()
            .context("could not read ratings")?;
        let labels = self
            .catalog
            .get_color_labels_map()
            .context("could not read color labels")?;
        let mut written = 0;
        for id in ids {
            let Some(item) = self.catalog.get_item_by_id(*id)? else {
                continue;
            };
            let metadata = self.sidecar_metadata_for(*id, &captions, &ratings, &labels)?;
            if xmp::write_library_sidecar(&item.path, &metadata)?.is_some() {
                written += 1;
            }
        }
        Ok(written)
    }

    /// Imports XMP sidecars for the given items. Only values present in the
    /// sidecar are applied (ratings always, title/caption/color label when set,
    /// tags merged),
    /// so importing never clears metadata that the sidecar does not mention.
    pub fn import_sidecars(&self, ids: &[MediaId]) -> Result<ImportReport> {
        let captions = self
            .catalog
            .get_captions_map()
            .context("could not read captions")?;
        let ratings = self
            .catalog
            .get_ratings_map()
            .context("could not read ratings")?;
        let labels = self
            .catalog
            .get_color_labels_map()
            .context("could not read color labels")?;
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

                    // Compare as stored, so a sidecar with untidy text is not
                    // rewritten (and counted as updated) on every import.
                    let title = metadata
                        .title
                        .as_deref()
                        .map(|title| text::clean_text(title, text::MAX_TEXT_CHARS))
                        .filter(|title| !title.is_empty());
                    if metadata.title.is_some() && title != item.title {
                        self.catalog.set_title(*id, metadata.title.as_deref())?;
                        changed = true;
                    }
                    let current_rating = ratings.get(id).copied().unwrap_or(0);
                    if metadata.rating != current_rating {
                        self.catalog.set_rating(*id, metadata.rating)?;
                        changed = true;
                    }
                    // Sidecars from before labels were written carry none; that
                    // must not clear a label set in Lumina.
                    let current_label = labels.get(id).copied().unwrap_or(0);
                    if metadata.color_label != 0 && metadata.color_label != current_label {
                        self.catalog.set_color_label(*id, metadata.color_label)?;
                        changed = true;
                    }
                    let caption = metadata
                        .caption
                        .as_deref()
                        .map(|caption| text::clean_caption(caption, text::MAX_TEXT_CHARS))
                        .filter(|caption| !caption.is_empty());
                    if metadata.caption.is_some() && caption != captions.get(id).cloned() {
                        self.catalog
                            .set_caption(*id, metadata.caption.as_deref().unwrap_or(""))?;
                        changed = true;
                    }
                    if !metadata.tags.is_empty() {
                        self.catalog.add_tags_batch(&[*id], &metadata.tags)?;
                        changed = true;
                    }
                    // Other tools write a file name to dc:source; only a
                    // web address is a network attribution.
                    if let Some(ref source_url) = metadata.source_url {
                        if source_url.starts_with("http://") || source_url.starts_with("https://") {
                            self.catalog.record_media_source(
                                *id,
                                source_url,
                                metadata.credit.as_deref(),
                            )?;
                            changed = true;
                        }
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
        labels: &HashMap<MediaId, u8>,
    ) -> Result<SidecarMetadata> {
        let tags = self.catalog.get_tags_for_item(id)?;
        let title = self.catalog.get_item_by_id(id)?.and_then(|item| item.title);
        let (source_url, credit) = match self.catalog.get_media_source(id)? {
            Some((url, uploader, _)) => (Some(url), uploader),
            None => (None, None),
        };
        Ok(SidecarMetadata {
            title,
            caption: captions.get(&id).cloned(),
            rating: ratings.get(&id).copied().unwrap_or(0),
            color_label: labels.get(&id).copied().unwrap_or(0),
            tags,
            source_url,
            credit,
        })
    }

    pub fn get_captions_map(&self) -> std::collections::HashMap<MediaId, String> {
        self.read("get captions map", self.catalog.get_captions_map())
    }

    pub fn record_media_source(
        &self,
        media_id: MediaId,
        source_url: &str,
        uploader: Option<&str>,
    ) -> Result<()> {
        self.catalog
            .record_media_source(media_id, source_url, uploader)
    }

    pub fn get_media_source(
        &self,
        media_id: MediaId,
    ) -> Result<Option<(String, Option<String>, i64)>> {
        self.catalog.get_media_source(media_id)
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
            .context("could not list the trash")?;

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

    /// The subset of `ids` that carry `tag`; see [`Catalog::ids_with_tag`].
    pub fn ids_with_tag(&self, ids: &[MediaId], tag: &str) -> Result<Vec<MediaId>> {
        self.catalog.ids_with_tag(ids, tag)
    }

    pub fn get_tags_for_item(&self, id: MediaId) -> Vec<String> {
        self.read("get tags for item", self.catalog.get_tags_for_item(id))
    }

    /// Per-tag count across `ids`; see [`Catalog::tag_counts_for_items`].
    pub fn tag_counts_for_items(&self, ids: &[MediaId]) -> Vec<(String, usize)> {
        self.catalog
            .tag_counts_for_items(ids)
            .unwrap_or_else(|err| {
                log::warn!("tag counts for selection failed: {err:#}");
                Vec::new()
            })
    }

    pub fn query_tag_counts(&self) -> Vec<(String, usize)> {
        self.read("query tag counts", self.catalog.query_tag_counts())
    }

    /// Counts active media items that carry no tags at all.
    pub fn query_untagged_count(&self) -> usize {
        self.read("query untagged count", self.catalog.query_untagged_count())
    }

    /// Lists a page of active untagged media using the requested ordering.
    pub fn query_untagged_sorted(
        &self,
        limit: usize,
        offset: usize,
        sort: SortSpec,
    ) -> Vec<MediaItem> {
        self.read(
            "query untagged sorted",
            self.catalog.query_untagged_sorted(limit, offset, sort),
        )
    }

    /// Scans candidate media items with duplicate file sizes, computes their
    /// content hashes in parallel, stores them in `media_hashes`, and returns
    /// a report of all duplicate groups.
    pub fn scan_duplicates(&self) -> Result<DuplicateReport> {
        let candidates = self.catalog.find_unhashed_duplicate_candidates()?;
        let total_scanned = candidates.len();

        if !candidates.is_empty() {
            use rayon::prelude::*;
            let computed: Vec<(MediaId, u64, String)> = candidates
                .into_par_iter()
                .filter_map(|(id, path, size)| match compute_file_hash(&path) {
                    Ok(hash) => Some((id, size, hash)),
                    Err(err) => {
                        log::warn!("failed to compute hash for {}: {err:#}", path.display());
                        None
                    }
                })
                .collect();

            self.catalog.batch_insert_hashes(&computed)?;
        }

        let groups = self.catalog.query_duplicate_groups()?;
        let total_duplicate_items: usize = groups.iter().map(|g| g.items.len()).sum();
        let potential_reclaimable_bytes: u64 =
            groups.iter().map(|g| g.potential_reclaimable_bytes).sum();

        Ok(DuplicateReport {
            total_scanned,
            groups,
            total_duplicate_items,
            potential_reclaimable_bytes,
        })
    }

    /// Queries all duplicate groups. Ensures unhashed candidate items with
    /// duplicate file sizes are hashed first.
    pub fn query_duplicate_groups(&self) -> Vec<DuplicateGroup> {
        match self.scan_duplicates() {
            Ok(report) => report.groups,
            Err(err) => {
                log::warn!("scan_duplicates encountered an error: {err:#}");
                self.read(
                    "query duplicate groups fallback",
                    self.catalog.query_duplicate_groups(),
                )
            }
        }
    }

    /// Fast, non-blocking read of already-computed duplicate groups from the catalog
    /// without scanning or hashing unhashed files.
    pub fn get_cached_duplicate_groups(&self) -> Vec<DuplicateGroup> {
        self.read(
            "get cached duplicate groups",
            self.catalog.query_duplicate_groups(),
        )
    }

    /// Returns the total count of duplicate items across all duplicate groups.
    pub fn query_duplicate_count(&self) -> usize {
        self.read(
            "query duplicate items count",
            self.catalog.query_duplicate_items_count(),
        )
    }

    /// Returns a slice of items in duplicate groups for pagination and grid viewing.
    pub fn query_duplicate_items_slice(&self, offset: usize, limit: usize) -> Vec<MediaItem> {
        self.read(
            "query duplicate items slice",
            self.catalog.query_duplicate_items_slice(offset, limit),
        )
    }

    /// Merges duplicates into `keep_id` and moves `trash_ids` to Recently Deleted.
    pub fn merge_duplicate_group(&self, keep_id: MediaId, trash_ids: &[MediaId]) -> Result<()> {
        self.catalog.merge_duplicate_group(keep_id, trash_ids)
    }

    /// Merges all duplicate groups in the catalog in a single atomic transaction.
    /// Returns the IDs of all newly trashed items.
    pub fn merge_all_duplicates(&self) -> Result<Vec<MediaId>> {
        self.catalog.merge_all_duplicates()
    }

    /// Points the missing item `id` at `chosen`, a file inside the library
    /// ("Locate…"). When `chosen` is already catalogued as another item, the
    /// two merge only if they can be the same file: equal size and
    /// dimensions, and equal content hash when one was stored. A different photo is never merged
    /// away. Blocking: reads and may hash the file.
    pub fn relink(&self, id: MediaId, chosen: &Path) -> Result<RelinkOutcome> {
        const OUTSIDE: &str = "Choose a file inside the library folder. To bring in a file from elsewhere, import it first.";
        let root = self.library_root.canonicalize().context("library folder")?;
        let chosen = chosen.canonicalize().context("chosen file")?;
        let Ok(relative) = chosen.strip_prefix(&root) else {
            anyhow::bail!(OUTSIDE);
        };
        // Catalog paths use the library root as it was opened.
        let path = self.library_root.join(relative);
        if !scanner::is_supported_media(&path) {
            anyhow::bail!("That file type is not supported.");
        }
        let missing = self
            .catalog
            .get_item_by_id(id)?
            .context("the missing item no longer exists")?;
        let item = Scanner::new(self.library_root.clone(), self.event_tx.clone())
            .inspect_media_file(&path)
            .context("the chosen file cannot be read")?;

        let outcome = match self.catalog.id_at_path(&path)? {
            Some(twin) if twin != id => {
                // Fixed-size formats make size alone weak; dimensions too.
                let same_file = item.file_size == missing.file_size
                    && (item.width, item.height) == (missing.width, missing.height);
                let same_content = match self.catalog.stored_hash(id)? {
                    Some(hash) => compute_file_hash(&path)? == hash,
                    None => true,
                };
                if !(same_file && same_content) {
                    anyhow::bail!("That file is already in your library as another item. Delete one of them, or merge them from Duplicates.");
                }
                self.catalog.fold_into(id, twin)?;
                let _ = self.event_tx.send(EngineEvent::ItemRemoved(twin));
                RelinkOutcome::Folded(twin)
            }
            Some(_) => RelinkOutcome::Relinked,
            None => {
                self.catalog.relink_path(id, &path)?;
                RelinkOutcome::Relinked
            }
        };
        // Fresh size, dates and inode for the file now behind the row.
        self.catalog.insert_or_update_batch(&[item])?;
        Ok(outcome)
    }

    /// Ids of items whose file is missing from disk.
    pub fn get_missing_ids(&self) -> std::collections::HashSet<MediaId> {
        self.read("get missing ids", self.catalog.get_missing_ids())
    }

    /// Checks whether an external file is an exact duplicate of any item in the library.
    pub fn check_file_is_duplicate(&self, path: &Path) -> Result<Option<MediaItem>> {
        let meta = match fs::metadata(path) {
            Ok(m) => m,
            Err(_) => return Ok(None),
        };
        let file_size = meta.len();
        if file_size == 0 {
            return Ok(None);
        }
        let candidates = self.catalog.find_candidates_by_size(file_size)?;
        if candidates.is_empty() {
            return Ok(None);
        }

        let candidate_hash = compute_file_hash(path)?;

        let matched = self.catalog.find_items_by_hash(&candidate_hash)?;
        if let Some(item) = matched.into_iter().next() {
            return Ok(Some(item));
        }

        let mut new_hashes = Vec::new();
        let mut matched_item = None;
        for (cand_id, cand_path) in candidates {
            if let Ok(h) = compute_file_hash(&cand_path) {
                if h == candidate_hash && matched_item.is_none() {
                    matched_item = self.catalog.get_item_by_id(cand_id)?;
                }
                new_hashes.push((cand_id, file_size, h));
            }
        }
        if !new_hashes.is_empty() {
            if let Err(err) = self.catalog.batch_insert_hashes(&new_hashes) {
                log::warn!("failed to cache candidate hashes: {err:#}");
            }
        }

        Ok(matched_item)
    }

    /// Evaluates candidate files before importing them into the library.
    /// Once `cancel` is set, the remaining files are reported as not
    /// duplicates without being hashed; the caller discards the result.
    pub fn check_import_duplicates(
        &self,
        paths: &[PathBuf],
        cancel: &AtomicBool,
    ) -> Result<Vec<DuplicateCandidate>> {
        use rayon::prelude::*;
        let newly_computed: Arc<Mutex<Vec<(MediaId, u64, String)>>> =
            Arc::new(Mutex::new(Vec::new()));

        let results: Vec<DuplicateCandidate> = paths
            .par_iter()
            .map(|path| {
                let file_size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
                if file_size == 0 || cancel.load(Ordering::Relaxed) {
                    return DuplicateCandidate {
                        path: path.clone(),
                        file_size: 0,
                        content_hash: None,
                        matched_item_id: None,
                        matched_item_path: None,
                        is_duplicate: false,
                    };
                }

                let candidates = self
                    .catalog
                    .find_candidates_by_size(file_size)
                    .unwrap_or_default();
                if candidates.is_empty() {
                    return DuplicateCandidate {
                        path: path.clone(),
                        file_size,
                        content_hash: None,
                        matched_item_id: None,
                        matched_item_path: None,
                        is_duplicate: false,
                    };
                }

                let content_hash = match compute_file_hash(path) {
                    Ok(h) => h,
                    Err(_) => {
                        return DuplicateCandidate {
                            path: path.clone(),
                            file_size,
                            content_hash: None,
                            matched_item_id: None,
                            matched_item_path: None,
                            is_duplicate: false,
                        };
                    }
                };

                let matched = self
                    .catalog
                    .find_items_by_hash(&content_hash)
                    .unwrap_or_default();
                if let Some(existing) = matched.first() {
                    return DuplicateCandidate {
                        path: path.clone(),
                        file_size,
                        content_hash: Some(content_hash),
                        matched_item_id: Some(existing.id),
                        matched_item_path: Some(existing.path.clone()),
                        is_duplicate: true,
                    };
                }

                for (cand_id, cand_path) in candidates {
                    if let Ok(cand_hash) = compute_file_hash(&cand_path) {
                        newly_computed
                            .lock()
                            .push((cand_id, file_size, cand_hash.clone()));
                        if cand_hash == content_hash {
                            return DuplicateCandidate {
                                path: path.clone(),
                                file_size,
                                content_hash: Some(content_hash),
                                matched_item_id: Some(cand_id),
                                matched_item_path: Some(cand_path),
                                is_duplicate: true,
                            };
                        }
                    }
                }

                DuplicateCandidate {
                    path: path.clone(),
                    file_size,
                    content_hash: Some(content_hash),
                    matched_item_id: None,
                    matched_item_path: None,
                    is_duplicate: false,
                }
            })
            .collect();

        // Batch insert newly computed hashes in a single transaction
        let to_insert = newly_computed.lock().clone();
        if !to_insert.is_empty() {
            if let Err(err) = self.catalog.batch_insert_hashes(&to_insert) {
                log::warn!("failed to cache computed import hashes: {err:#}");
            }
        }

        Ok(results)
    }

    /// Newest active items carrying `tag`; see [`Catalog::get_items_by_tag`].
    pub fn items_with_tag(&self, tag: &str, limit: usize) -> Result<Vec<MediaItem>> {
        self.catalog.get_items_by_tag(tag, limit, 0)
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

        self.decoder.spawn(move || {
            let item = match catalog.get_item_by_id(id) {
                Ok(Some(it)) => it,
                _ => {
                    callback(None);
                    return;
                }
            };

            Self::process_thumbnail_request(catalog, cache, item, target_size, callback);
        });
    }

    /// Requests a thumbnail using an already-fetched `MediaItem`, bypassing the
    /// initial catalog query and eliminating SQLite connection lock contention.
    pub fn request_thumbnail_item<F>(&self, item: MediaItem, target_size: u32, callback: F)
    where
        F: FnOnce(Option<DecodedFrame>) + Send + 'static,
    {
        let catalog = self.catalog.clone();
        let cache = self.cache.clone();

        self.decoder.spawn(move || {
            Self::process_thumbnail_request(catalog, cache, item, target_size, callback);
        });
    }

    fn process_thumbnail_request<F>(
        catalog: Catalog,
        cache: DiskCache,
        item: MediaItem,
        target_size: u32,
        callback: F,
    ) where
        F: FnOnce(Option<DecodedFrame>),
    {
        let id = item.id;
        let rotation = match catalog.get_rotation(id) {
            Ok(quarters) => quarters,
            Err(err) => {
                log::warn!("failed to read rotation for {id:?}: {err:?}");
                0
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
            callback(Some(DecoderPipeline::rotate_frame(frame, rotation)));
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
                callback(Some(DecoderPipeline::rotate_frame(frame, rotation)));
            }
            Err(err) => {
                log::warn!("failed to decode thumbnail for {:?}: {err:?}", item.path);
                callback(None);
            }
        }
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

    fn run_scan(&self, cancel_flag: Arc<AtomicBool>) {
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn temp_library(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("lumina_{prefix}_{nanos}"));
        fs::create_dir_all(&dir).expect("temp library dir must be creatable");
        dir
    }

    fn request_thumbnail_frame(
        engine: &LuminaEngine,
        id: MediaId,
        target_size: u32,
    ) -> DecodedFrame {
        let (tx, rx) = mpsc::channel();
        engine.request_thumbnail(id, target_size, move |frame| {
            let _ = tx.send(frame);
        });
        rx.recv_timeout(Duration::from_secs(10))
            .expect("thumbnail callback must arrive")
            .expect("thumbnail must decode")
    }

    #[test]
    fn engine_rotation_persists_and_rotates_decodes_without_touching_originals() {
        let dir = temp_library("engine_rotation");
        let image_path = dir.join("landscape.png");
        let source = image::RgbaImage::from_fn(8, 4, |x, y| {
            image::Rgba([(x * 20) as u8, (y * 30) as u8, 40, 255])
        });
        source
            .save(&image_path)
            .expect("test image must be writable");
        let original_bytes = fs::read(&image_path).expect("test image must be readable");

        let (engine, events) = LuminaEngine::open_with_paths(
            dir.clone(),
            dir.join("catalog.db"),
            Some(dir.join("cache")),
        )
        .expect("engine must open");
        while let Ok(event) = events.recv_timeout(Duration::from_secs(10)) {
            if matches!(event, EngineEvent::ScanComplete { .. }) {
                break;
            }
        }

        let item = engine
            .get_item_slice(0..1)
            .into_iter()
            .next()
            .expect("scanned image must be cataloged");
        assert_eq!(
            engine
                .get_rotation(item.id)
                .expect("rotation read must work"),
            0,
            "a scanned item must start unrotated"
        );

        let unrotated = request_thumbnail_frame(&engine, item.id, 320);
        assert_eq!((unrotated.width, unrotated.height), (8, 4));

        engine
            .set_rotation(&[item.id], 1)
            .expect("rotation write must succeed");
        assert_eq!(engine.get_rotation(item.id).expect("rotation read"), 1);

        let rotated = request_thumbnail_frame(&engine, item.id, 320);
        assert_eq!(
            (rotated.width, rotated.height),
            (4, 8),
            "thumbnail decode must apply the stored quarter turn"
        );

        // A repeated request can be served from the disk cache; rotation is
        // applied after the cache lookup, so the cached bytes stay valid.
        let cached_rotated = request_thumbnail_frame(&engine, item.id, 320);
        assert_eq!(
            (cached_rotated.width, cached_rotated.height),
            (4, 8),
            "the cached frame must be rotated on every request"
        );

        assert_eq!(
            fs::read(&image_path).expect("original must stay readable"),
            original_bytes,
            "rotation must never rewrite the original file"
        );

        engine
            .set_rotation(&[], 3)
            .expect("an empty rotation batch must be a no-op");
        engine
            .set_rotation(&[item.id, MediaId(9_999)], 3)
            .expect("unknown ids must not fail the batch");
        assert_eq!(engine.get_rotation(item.id).expect("rotation read"), 3);

        drop(engine);
        let _ = fs::remove_dir_all(&dir);
    }
}
