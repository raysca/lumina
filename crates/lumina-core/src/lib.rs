pub mod cache;
pub mod catalog;
pub mod decoder;
mod dims;
pub mod duplicates;
pub mod edit_copy;
pub mod edits;
mod export;
pub mod geo_cluster;
pub mod geocoder;
pub mod import;
pub mod net_import;
pub mod sample;
pub mod scanner;
pub mod search;
pub mod smart_albums;
pub mod sort;
pub mod stacks;
pub mod text;
mod video_frame;
pub mod watcher;
pub mod xmp;

use anyhow::{Context, Result};
use crossbeam_channel::{unbounded, Receiver, Sender};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub use cache::DiskCache;
pub use catalog::{
    Album, Catalog, CropRect, DateSpan, DeletionTarget, DownloadRow, DownloadState, Edits,
    GeoPoint, PlaceCount, PriorDownload, RejectedItems, Rights, TagChange, TagReplay,
    TemplateChange, ViewScope, SCHEMA_VERSION,
};
pub use decoder::DecoderPipeline;
pub use duplicates::{compute_file_hash, DuplicateCandidate, DuplicateGroup, DuplicateReport};
pub use export::{ExportPreset, ExportReport};
pub use geo_cluster::Cluster;
pub use geocoder::{GeoLocation, OfflineGeocoder};
pub use scanner::{extract_video_duration, extract_video_size, Scanner};
pub use search::StructuredQuery;
pub use smart_albums::{
    from_portable_json, parse_rule_date, split_date_range, to_portable_json, MatchMode,
    PortableSmartAlbum, RuleField, RuleOperator, RuleValue, SmartAlbum, SmartAlbumRule,
    SystemAlbum, MAX_PORTABLE_BYTES,
};
pub use sort::{SortDirection, SortField, SortSpec};
pub use stacks::{Stack, StackKind};
pub use video_frame::video_preview_strip;
pub use watcher::LibraryWatcher;
pub use xmp::{
    parse_xmp, read_sidecar, render_xmp, sidecar_path, write_sidecar, SidecarMetadata,
    SidecarOutcome,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MediaId(pub u64);

/// A culling flag, stored as `media_items.flag`. Ordered so that the
/// highest wins a merge: pick beats unflagged beats reject. Not `None`,
/// which would shadow `Option::None` in glob imports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Flag {
    Reject = -1,
    Unflagged = 0,
    Pick = 1,
}

impl Flag {
    /// The flag for a stored value; anything out of range reads by its sign.
    pub(crate) fn from_db(value: i64) -> Self {
        match value.signum() {
            1 => Flag::Pick,
            -1 => Flag::Reject,
            _ => Flag::Unflagged,
        }
    }
}

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

/// Result of [`LuminaEngine::write_metadata_sidecars`].
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SidecarWriteReport {
    pub written: usize,
    /// Sidecars another app wrote (or Lumina cannot read), left untouched.
    pub skipped_foreign: Vec<PathBuf>,
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
        catalog
            .backfill_downloaded_sources(&library_root)
            .context("marking which sources are downloads")?;
        if !db_existed {
            // A new catalog's sizes all come from the current scanner.
            catalog
                .mark_dims_oriented()
                .context("marking photo sizes as current")?;
        }
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
    pub fn with_catalog_locked<R>(&self, f: impl FnOnce() -> R) -> R {
        self.catalog.with_lock_held(f)
    }

    /// How many times the current thread has taken the catalog lock, from
    /// any engine. Tests compare it around UI work to prove the UI thread
    /// never waits on the lock, without depending on timing.
    #[cfg(feature = "test-support")]
    pub fn catalog_locks_on_this_thread() -> u64 {
        catalog::locks_on_this_thread()
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

    /// Pins or unpins a smart album; false when it no longer exists.
    pub fn set_smart_album_pinned(&self, id: i64, pinned: bool) -> Result<bool> {
        self.catalog.set_smart_album_pinned(id, pinned)
    }

    /// Creates an empty hand-picked album.
    pub fn create_album(&self, title: &str) -> Result<Album> {
        self.catalog.create_album(title)
    }

    /// Creates an album holding `members` in one transaction.
    pub fn create_album_with(&self, title: &str, members: &[MediaId]) -> Result<Album> {
        self.catalog.create_album_with(title, members)
    }

    /// Renames an album; false when it no longer exists.
    pub fn rename_album(&self, id: i64, title: &str) -> Result<bool> {
        self.catalog.rename_album(id, title)
    }

    /// Deletes an album, returning it and its members for undo.
    pub fn delete_album(&self, id: i64) -> Result<Option<(Album, Vec<MediaId>)>> {
        self.catalog.delete_album(id)
    }

    /// Undoes [`Self::delete_album`]; members purged since are skipped.
    pub fn restore_album(&self, album: &Album, members: &[MediaId]) -> Result<()> {
        self.catalog.restore_album(album, members)
    }

    /// Every album with its active member count, for the sidebar.
    pub fn list_albums_with_counts(&self) -> Vec<(Album, usize)> {
        self.read(
            "list albums with counts",
            self.catalog.list_albums_with_counts(),
        )
    }

    /// Adds media to an album; returns how many were new members.
    pub fn add_to_album(&self, id: i64, media: &[MediaId]) -> Result<usize> {
        self.catalog.add_to_album(id, media)
    }

    /// Adds media to an album; returns the ids that were not members yet.
    pub fn add_new_to_album(&self, id: i64, media: &[MediaId]) -> Result<Vec<MediaId>> {
        self.catalog.add_new_to_album(id, media)
    }

    /// Removes media from an album (never from the library).
    pub fn remove_from_album(&self, id: i64, media: &[MediaId]) -> Result<usize> {
        self.catalog.remove_from_album(id, media)
    }

    /// Removes media from an album; returns the ids that were members.
    pub fn remove_members_from_album(&self, id: i64, media: &[MediaId]) -> Result<Vec<MediaId>> {
        self.catalog.remove_members_from_album(id, media)
    }

    /// A page of an album's active members in `sort` order.
    pub fn query_album_sorted(
        &self,
        id: i64,
        limit: usize,
        offset: usize,
        sort: SortSpec,
    ) -> Vec<MediaItem> {
        self.read(
            "query album sorted",
            self.catalog.query_album_sorted(id, limit, offset, sort),
        )
    }

    /// How many active members an album has.
    pub fn query_album_count(&self, id: i64) -> usize {
        self.read("query album count", self.catalog.query_album_count(id))
    }

    /// Pins or unpins an album; false when it no longer exists.
    pub fn set_album_pinned(&self, id: i64, pinned: bool) -> Result<bool> {
        self.catalog.set_album_pinned(id, pinned)
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

    /// Sets each item's culling flag in one transaction.
    pub fn set_flags(&self, changes: &[(MediaId, Flag)]) -> Result<()> {
        self.catalog.set_flags(changes)
    }

    /// The rejected items of one view, for Delete Rejected…. A read error
    /// is returned, not hidden as "no rejects".
    pub fn rejected_in(&self, scope: &ViewScope) -> Result<RejectedItems> {
        self.catalog.rejected_in(scope)
    }

    /// The first and last capture month of the view's dated items, over
    /// the same rows as the view's count; `None` when none is dated. A read
    /// error is logged and reads as `None`, so the header shows only the
    /// count.
    pub fn view_date_span(&self, scope: &ViewScope) -> Option<DateSpan> {
        self.read("view date span", self.catalog.view_date_span(scope))
    }

    /// Every picked or rejected item; unflagged items are left out.
    pub fn get_flags_map(&self) -> std::collections::HashMap<MediaId, Flag> {
        self.read("get flags map", self.catalog.get_flags_map())
    }

    pub fn set_color_labels(&self, changes: &[(MediaId, u8)]) -> Result<()> {
        self.catalog.set_color_labels(changes)
    }

    /// Sets each item's own clockwise quarter turns in one transaction
    /// (batch rotate and its undo); ids not in the catalog are skipped.
    ///
    /// Rotation is display-only: the original file is never rewritten, and
    /// the disk cache keeps its entries because it stores the unrotated
    /// decode (the turn is applied after the cache lookup, see
    /// `process_thumbnail_request`).
    pub fn set_rotations(&self, changes: &[(MediaId, u8)]) -> Result<()> {
        self.catalog.set_rotations(changes)
    }

    /// Each id's persisted rotation; ids not in the catalog are left out.
    pub fn get_rotations(&self, ids: &[MediaId]) -> Result<HashMap<MediaId, u8>> {
        self.catalog.get_rotations(ids)
    }

    /// Each id's edits (rotation, crop, exposure, auto-enhance); ids not in
    /// the catalog are left out.
    pub fn get_edits(&self, ids: &[MediaId]) -> Result<HashMap<MediaId, Edits>> {
        self.catalog.get_edits(ids)
    }

    /// Replaces each item's edits in one transaction; ids not in the catalog
    /// are skipped and an invalid crop fails the write. Nothing is dropped
    /// from the disk cache: it holds the unedited decode and edits apply
    /// after the lookup, like rotation.
    pub fn set_edits(&self, changes: &[(MediaId, Edits)]) -> Result<()> {
        self.catalog.set_edits(changes)
    }

    /// Every item with an edit or a rotation.
    pub fn edited_ids(&self) -> Result<HashSet<MediaId>> {
        self.catalog.edited_ids()
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

    /// Writes/updates XMP sidecars next to the original files (metadata-only
    /// export). Safe to call from any thread; passes run one at a time.
    /// A sidecar another app wrote is never rewritten or deleted: it is
    /// listed in `skipped_foreign` and logged once per pass.
    pub fn write_metadata_sidecars(&self, ids: &[MediaId]) -> Result<SidecarWriteReport> {
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
        let rights = self
            .catalog
            .get_rights(ids)
            .context("could not read rights")?;
        let edited = self
            .catalog
            .edited_flags(ids)
            .context("could not read edited dates and places")?;
        let mut report = SidecarWriteReport::default();
        for id in ids {
            let Some(item) = self.catalog.get_item_by_id(*id)? else {
                continue;
            };
            let metadata =
                self.sidecar_metadata_for(*id, &captions, &ratings, &labels, &rights, &edited)?;
            match xmp::write_library_sidecar(&item.path, &metadata)? {
                SidecarOutcome::Written(_) => report.written += 1,
                SidecarOutcome::SkippedForeign => {
                    report.skipped_foreign.push(xmp::sidecar_path(&item.path))
                }
                SidecarOutcome::Removed | SidecarOutcome::Unchanged => {}
            }
        }
        if let Some(first) = report.skipped_foreign.first() {
            log::warn!(
                "left {} XMP sidecar(s) written by another app unchanged, first {first:?}",
                report.skipped_foreign.len()
            );
        }
        Ok(report)
    }

    /// Imports XMP sidecars for the given items. Only values present in the
    /// sidecar are applied (ratings always, title/caption/color label, each
    /// rights field, the date and the GPS position when set, tags merged),
    /// so importing never clears metadata that the sidecar does not mention.
    pub fn import_sidecars(&self, ids: &[MediaId]) -> Result<ImportReport> {
        let before = self.edits_snapshot(ids)?;
        self.import_sidecars_since(ids, &before)
    }

    /// The user-editable fields of `ids` as they are now, read before any
    /// sidecar so an edit made while the import runs can be told apart.
    fn edits_snapshot(&self, ids: &[MediaId]) -> Result<HashMap<MediaId, ItemEdits>> {
        let mut before = HashMap::with_capacity(ids.len());
        for &id in ids {
            if let Some(item) = self.catalog.get_item_by_id(id)? {
                before.insert(id, self.item_edits(&item)?);
            }
        }
        Ok(before)
    }

    fn item_edits(&self, item: &MediaItem) -> Result<ItemEdits> {
        Ok(ItemEdits {
            title: item.title.clone(),
            rating: self
                .catalog
                .get_rating(item.id)
                .context("could not read rating")?,
            label: self
                .catalog
                .get_color_label(item.id)
                .context("could not read color label")?,
            caption: self
                .catalog
                .get_caption(item.id)
                .context("could not read caption")?,
            rights: self
                .catalog
                .get_rights(&[item.id])
                .context("could not read rights")?
                .remove(&item.id)
                .unwrap_or_default(),
            timestamp_epoch_ms: item.timestamp_epoch_ms,
            location: (item.latitude, item.longitude),
        })
    }

    /// Applies each item's sidecar, except to a field the user changed since
    /// `before` was taken: the import runs in the background, and an edit made
    /// meanwhile must win over the (older) sidecar value.
    // ponytail: read-then-write per field, so an edit landing in the
    // microseconds between the fresh read and the write can still lose; a
    // conditional `UPDATE ... WHERE rating = ?before` closes that if needed.
    fn import_sidecars_since(
        &self,
        ids: &[MediaId],
        before: &HashMap<MediaId, ItemEdits>,
    ) -> Result<ImportReport> {
        let mut report = ImportReport::default();

        for id in ids {
            let (Some(item), Some(then)) = (self.catalog.get_item_by_id(*id)?, before.get(id))
            else {
                report.skipped += 1;
                continue;
            };
            match xmp::read_sidecar(&item.path) {
                Ok(Some(metadata)) => {
                    report.imported += 1;
                    let mut changed = false;
                    let now = self.item_edits(&item)?;

                    // Compare as stored, so a sidecar with untidy text is not
                    // rewritten (and counted as updated) on every import.
                    let title = metadata
                        .title
                        .as_deref()
                        .map(|title| text::clean_text(title, text::MAX_TEXT_CHARS))
                        .filter(|title| !title.is_empty());
                    if metadata.title.is_some() && title != now.title && now.title == then.title {
                        self.catalog.set_title(*id, metadata.title.as_deref())?;
                        changed = true;
                    }
                    if metadata.rating != now.rating && now.rating == then.rating {
                        self.catalog.set_rating(*id, metadata.rating)?;
                        changed = true;
                    }
                    // Sidecars from before labels were written carry none; that
                    // must not clear a label set in Lumina.
                    if metadata.color_label != 0
                        && metadata.color_label != now.label
                        && now.label == then.label
                    {
                        self.catalog.set_color_label(*id, metadata.color_label)?;
                        changed = true;
                    }
                    let caption = metadata
                        .caption
                        .as_deref()
                        .map(|caption| text::clean_caption(caption, text::MAX_TEXT_CHARS))
                        .filter(|caption| !caption.is_empty());
                    if metadata.caption.is_some()
                        && caption != now.caption
                        && now.caption == then.caption
                    {
                        self.catalog
                            .set_caption(*id, metadata.caption.as_deref().unwrap_or(""))?;
                        changed = true;
                    }
                    // Fields the sidecar sets replace the catalog's; the
                    // others stay (parse_xmp already cleaned and dropped blanks).
                    let rights = now.rights.merged_with(&metadata.rights);
                    if rights != now.rights && now.rights == then.rights {
                        self.catalog.set_rights(&[(*id, rights)])?;
                        changed = true;
                    }
                    // Dates compare by the second (sidecars carry no
                    // milliseconds), places within 1e-5°, so a re-import
                    // of Lumina's own sidecar changes nothing. Both writes
                    // mark the edited flags, so a rescan keeps them.
                    if let Some(mut date) = metadata.date_taken {
                        // A date alone changes the day and keeps the time of
                        // day, as the date field in Get Info does.
                        if metadata.date_only {
                            if let Some(now) =
                                chrono::DateTime::from_timestamp_millis(now.timestamp_epoch_ms)
                            {
                                date = date.date().and_time(now.time());
                            }
                        }
                        let epoch_ms = date.and_utc().timestamp_millis();
                        if epoch_ms.div_euclid(1000) != now.timestamp_epoch_ms.div_euclid(1000)
                            && now.timestamp_epoch_ms == then.timestamp_epoch_ms
                        {
                            self.catalog.set_timestamps(&[(*id, epoch_ms)])?;
                            changed = true;
                        }
                    }
                    if let Some((lat, lon)) = metadata.location {
                        let same = match now.location {
                            (Some(now_lat), Some(now_lon)) => {
                                (now_lat - lat).abs() <= 1e-5 && (now_lon - lon).abs() <= 1e-5
                            }
                            _ => false,
                        };
                        if !same && now.location == then.location {
                            self.catalog
                                .set_locations(&[(*id, (Some(lat), Some(lon)))])?;
                            changed = true;
                        }
                    }
                    // A keyword the catalog refuses (over the length cap, or
                    // at or below an alias) is reported; the item's other
                    // keywords still import. A blank one (no segment left
                    // after cleaning) is skipped.
                    let mut tags = Vec::new();
                    for tag in &metadata.tags {
                        // Only an invalid path is a per-keyword failure; a
                        // database error from the alias lookup aborts.
                        let path = match catalog::keywords::normalize_path(tag) {
                            Ok(path) => self.catalog.resolve_alias(&path)?,
                            Err(_) if catalog::keywords::is_blank_path(tag) => continue,
                            Err(err) => {
                                report.failed.push((
                                    item.path.clone(),
                                    format!("could not import keyword: {err}"),
                                ));
                                continue;
                            }
                        };
                        match self.catalog.keyword_refusal(&path)? {
                            Some(reason) => report.failed.push((
                                item.path.clone(),
                                format!("could not import keyword: {reason}"),
                            )),
                            None => tags.push(path),
                        }
                    }
                    if !tags.is_empty() {
                        self.catalog.add_tags_batch(&[*id], &tags)?;
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
        rights: &HashMap<MediaId, Rights>,
        edited: &HashMap<MediaId, (bool, bool)>,
    ) -> Result<SidecarMetadata> {
        let tags = self.catalog.get_tags_for_item(id)?;
        let item = self.catalog.get_item_by_id(id)?;
        // Decision: only an edited date or place is written; file EXIF stays
        // the source otherwise. The catalog keeps wall time as UTC.
        let (date_edited, place_edited) = edited.get(&id).copied().unwrap_or_default();
        let date_taken = item
            .as_ref()
            .filter(|_| date_edited)
            .and_then(|item| chrono::DateTime::from_timestamp_millis(item.timestamp_epoch_ms))
            .map(|date| date.naive_utc());
        let location = item
            .as_ref()
            .filter(|_| place_edited)
            .and_then(|item| item.latitude.zip(item.longitude));
        let title = item.and_then(|item| item.title);
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
            rights: rights.get(&id).cloned().unwrap_or_default(),
            date_taken,
            date_only: false,
            location,
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

    /// Applies a metadata template's rights and keywords to `ids` in one
    /// transaction; see [`Catalog::apply_template`].
    pub fn apply_template(
        &self,
        ids: &[MediaId],
        rights: &Rights,
        keywords: &[String],
    ) -> Result<TemplateChange> {
        self.catalog.apply_template(ids, rights, keywords)
    }

    /// Undoes (`before = true`) or redoes a template apply; see
    /// [`Catalog::replay_template_change`].
    pub fn replay_template_change(&self, change: &TemplateChange, before: bool) -> Result<()> {
        self.catalog.replay_template_change(change, before)
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

    /// Cities starting with `prefix`, with their active item counts, most
    /// items first (search top matches). Scans the catalog: call it off the
    /// UI thread.
    pub fn matching_places(&self, prefix: &str, limit: usize) -> Vec<PlaceCount> {
        self.read(
            "matching places",
            self.catalog.matching_places(prefix, limit),
        )
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
                // A quarter-turned photo that went missing before sizes were
                // stored oriented holds its unrotated size. Only then does a
                // transposed size match: two uncompressed TIFFs of 6000x4000
                // and 4000x6000 share a byte size.
                let (w, h) = (missing.width, missing.height);
                let turned = (5..=8).contains(&dims::read_exif_size(&path).orientation);
                let same_file = item.file_size == missing.file_size
                    && ((item.width, item.height) == (w, h)
                        || (turned && (item.width, item.height) == (h, w)));
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

    /// Renames a keyword and its subtree; see [`Catalog::rename_tag`].
    pub fn rename_tag(&self, old_tag: &str, new_tag: &str) -> Result<TagChange> {
        self.catalog.rename_tag(old_tag, new_tag)
    }

    /// Deletes a keyword and its subtree; see [`Catalog::delete_tag_globally`].
    pub fn delete_tag_globally(&self, tag: &str) -> Result<TagChange> {
        self.catalog.delete_tag_globally(tag)
    }

    /// Deletes several keywords in one transaction; see
    /// [`Catalog::delete_tags_globally`].
    pub fn delete_tags_globally(&self, tags: &[String]) -> Result<TagChange> {
        self.catalog.delete_tags_globally(tags)
    }

    /// Undoes (`before = true`) or redoes a global keyword change; returns
    /// what it left alone because it changed since. See
    /// [`Catalog::replace_tag_rows`].
    pub fn apply_tag_rows(&self, change: &TagChange, before: bool) -> Result<TagReplay> {
        self.catalog.replace_tag_rows(change, before)
    }

    /// Merges a keyword into another; see [`Catalog::merge_tags`].
    pub fn merge_tags(&self, source_tag: &str, target_tag: &str) -> Result<TagChange> {
        self.catalog.merge_tags(source_tag, target_tag)
    }

    /// See [`Catalog::set_alias`].
    pub fn set_alias(&self, alias: &str, tag: &str) -> Result<()> {
        self.catalog.set_alias(alias, tag)
    }

    /// See [`Catalog::remove_alias`].
    pub fn remove_alias(&self, alias: &str) -> Result<()> {
        self.catalog.remove_alias(alias)
    }

    /// See [`Catalog::aliases`].
    pub fn aliases(&self) -> Result<Vec<(String, String)>> {
        self.catalog.aliases()
    }

    /// See [`Catalog::resolve_alias`].
    pub fn resolve_alias(&self, raw: &str) -> Result<String> {
        self.catalog.resolve_alias(raw)
    }

    /// See [`Catalog::subtree_item_count`].
    pub fn subtree_item_count(&self, path: &str) -> Result<usize> {
        self.catalog.subtree_item_count(path)
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

    /// Decodes `id` at `target_size` with no edits and no rotation: the
    /// source frame for edit mode's live preview.
    pub fn request_unedited_frame<F>(&self, id: MediaId, target_size: u32, callback: F)
    where
        F: FnOnce(Option<DecodedFrame>) + Send + 'static,
    {
        let catalog = self.catalog.clone();
        let cache = self.cache.clone();
        self.decoder
            .spawn(move || match catalog.get_item_by_id(id) {
                Ok(Some(item)) => {
                    callback(Self::decode_through_cache(&cache, &item, target_size, true))
                }
                Ok(None) => callback(None),
                Err(err) => {
                    log::warn!("failed to read item {id:?} for an unedited frame: {err:?}");
                    callback(None);
                }
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
        // One read: get_edits joins rotation and the side row.
        let edits = match catalog.get_edits(&[id]) {
            Ok(mut edits) => edits.remove(&id).unwrap_or_default(),
            Err(err) => {
                log::warn!("failed to read edits for {id:?}: {err:?}");
                Edits::default()
            }
        };
        let edits = edits::effective_edits(item.media_type, edits);
        let decode_size =
            edits::edit_decode_size(target_size, edits.crop, (item.width, item.height));
        let cacheable = edits::edit_decode_is_cacheable(target_size, decode_size);
        let frame = Self::decode_through_cache(&cache, &item, decode_size, cacheable);
        callback(frame.map(|frame| edits::apply_edits(frame, &edits)));
    }

    /// The unedited decode of `item` at `size`: from the disk cache when
    /// present, else decoded and written back only when `cacheable`.
    fn decode_through_cache(
        cache: &DiskCache,
        item: &MediaItem,
        size: u32,
        cacheable: bool,
    ) -> Option<DecodedFrame> {
        if let Some(frame) = cache.get_cached(
            item.id,
            &item.path,
            item.file_size,
            item.mtime_epoch_ms,
            size,
        ) {
            return Some(frame);
        }
        match DecoderPipeline::decode_media(item.id, item.media_type, &item.path, size) {
            Ok(frame) => {
                if cacheable {
                    if let Err(err) = cache.put_cached(
                        &item.path,
                        item.file_size,
                        item.mtime_epoch_ms,
                        size,
                        &frame,
                    ) {
                        log::warn!("failed to cache the decode of {:?}: {err:#}", item.path);
                    }
                }
                Some(frame)
            }
            Err(err) => {
                log::warn!("failed to decode thumbnail for {:?}: {err:?}", item.path);
                None
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

/// The fields an XMP import may overwrite, as the catalog holds them.
#[derive(Debug, Clone, PartialEq)]
struct ItemEdits {
    title: Option<String>,
    rating: u8,
    label: u8,
    caption: Option<String>,
    rights: Rights,
    timestamp_epoch_ms: i64,
    location: Coordinates,
}

#[cfg(test)]
mod tests {
    /// Generous debug budget for decode and scan callbacks: a 2k decode
    /// competes with the whole parallel suite. A passing test never waits it out.
    const TEST_WAIT: Duration = Duration::from_secs(60);

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
        rx.recv_timeout(TEST_WAIT)
            .expect("thumbnail callback must arrive")
            .expect("thumbnail must decode")
    }

    #[test]
    fn xmp_import_keeps_a_field_the_user_edited_while_it_ran() {
        let dir = temp_library("xmp_edit_race");
        fs::write(dir.join("a.jpg"), b"test-jpeg-content").expect("fixture writes");
        let (engine, rx) = LuminaEngine::open_with_paths(dir.clone(), dir.join("catalog.db"), None)
            .expect("engine opens");
        while let Ok(event) = rx.recv_timeout(TEST_WAIT) {
            if let EngineEvent::ScanComplete { .. } = event {
                break;
            }
        }
        let item = engine.get_item_slice(0..1).remove(0);
        write_sidecar(
            &item.path,
            &SidecarMetadata {
                title: Some("Sidecar title".into()),
                caption: Some("Sidecar caption".into()),
                rating: 5,
                color_label: 4,
                rights: crate::catalog::Rights {
                    creator: Some("Sidecar creator".into()),
                    ..Default::default()
                },
                date_taken: chrono::DateTime::from_timestamp(1_500_000_000, 0)
                    .map(|date| date.naive_utc()),
                location: Some((48.8566, 2.3522)),
                ..Default::default()
            },
        )
        .expect("sidecar writes");

        let before = engine.edits_snapshot(&[item.id]).expect("snapshot");
        engine
            .catalog
            .set_timestamp(item.id, 1_600_000_000_000)
            .expect("date");
        engine
            .catalog
            .set_location(item.id, Some(-33.8688), Some(151.2093))
            .expect("place");
        let mine = crate::catalog::Rights {
            creator: Some("My name".into()),
            ..Default::default()
        };
        engine
            .catalog
            .set_rights(&[(item.id, mine.clone())])
            .expect("rights");
        // The user edits while the background import is still running.
        engine.catalog.set_rating(item.id, 2).expect("rating");
        engine
            .catalog
            .set_title(item.id, Some("Mine"))
            .expect("title");
        engine
            .import_sidecars_since(&[item.id], &before)
            .expect("import runs");

        assert_eq!(
            engine.catalog.get_rating(item.id).unwrap(),
            2,
            "a rating set during the import must not be overwritten by the sidecar"
        );
        let title = engine
            .catalog
            .get_item_by_id(item.id)
            .unwrap()
            .unwrap()
            .title;
        assert_eq!(title.as_deref(), Some("Mine"), "the user's title wins");
        assert_eq!(
            engine.catalog.get_rights(&[item.id]).unwrap().get(&item.id),
            Some(&mine),
            "rights set during the import must not be overwritten by the sidecar"
        );
        assert_eq!(
            engine.catalog.get_color_label(item.id).unwrap(),
            4,
            "an untouched field still imports"
        );
        assert_eq!(
            engine.catalog.get_caption(item.id).unwrap().as_deref(),
            Some("Sidecar caption"),
            "an untouched caption still imports"
        );
        let after = engine.catalog.get_item_by_id(item.id).unwrap().unwrap();
        assert_eq!(
            after.timestamp_epoch_ms, 1_600_000_000_000,
            "a date set during the import must not be overwritten by the sidecar"
        );
        assert_eq!(
            (after.latitude, after.longitude),
            (Some(-33.8688), Some(151.2093)),
            "a place set during the import must not be overwritten by the sidecar"
        );
        drop(engine);
        let _ = fs::remove_dir_all(&dir);
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
        while let Ok(event) = events.recv_timeout(TEST_WAIT) {
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
        let cache_file = engine.cache.cache_file_path(&DiskCache::compute_key(
            &item.path,
            item.file_size,
            item.mtime_epoch_ms,
            320,
        ));
        assert!(
            cache_file.exists(),
            "the first decode must fill the disk cache"
        );

        engine
            .set_rotations(&[(item.id, 1)])
            .expect("rotation write must succeed");
        assert_eq!(engine.get_rotation(item.id).expect("rotation read"), 1);
        assert!(
            cache_file.exists(),
            "rotating must keep the cached decode: it holds the unrotated frame"
        );

        // Served from the disk cache: rotation is applied after the lookup,
        // so the cached unrotated bytes stay valid.
        let rotated = request_thumbnail_frame(&engine, item.id, 320);
        assert_eq!(
            (rotated.width, rotated.height),
            (4, 8),
            "a cached frame decoded after rotation must come back rotated"
        );

        assert_eq!(
            fs::read(&image_path).expect("original must stay readable"),
            original_bytes,
            "rotation must never rewrite the original file"
        );

        engine
            .set_rotations(&[])
            .expect("an empty rotation batch must be a no-op");
        engine
            .set_rotations(&[(item.id, 3), (MediaId(9_999), 3)])
            .expect("unknown ids must not fail the batch");
        assert_eq!(engine.get_rotation(item.id).expect("rotation read"), 3);

        drop(engine);
        let _ = fs::remove_dir_all(&dir);
    }

    /// An engine over `dir` holding one generated image, scanned.
    fn engine_with_image(
        dir: &Path,
        name: &str,
        width: u32,
        height: u32,
    ) -> (Arc<LuminaEngine>, MediaItem) {
        let source = image::RgbImage::from_fn(width, height, |x, y| {
            image::Rgb([(x * 255 / width) as u8, (y * 255 / height) as u8, 90])
        });
        source
            .save(dir.join(name))
            .expect("test image must be writable");
        let (engine, events) = LuminaEngine::open_with_paths(
            dir.to_path_buf(),
            dir.join("catalog.db"),
            Some(dir.join("cache")),
        )
        .expect("engine must open");
        while let Ok(event) = events.recv_timeout(TEST_WAIT) {
            if matches!(event, EngineEvent::ScanComplete { .. }) {
                break;
            }
        }
        let item = engine.get_item_slice(0..1).remove(0);
        (engine, item)
    }

    fn crop_edits(x: f32, y: f32, w: f32, h: f32) -> Edits {
        Edits {
            crop: Some(CropRect { x, y, w, h }),
            ..Default::default()
        }
    }

    #[test]
    fn edits_never_touch_original_or_cache() {
        let dir = temp_library("edits_untouched");
        let (engine, item) = engine_with_image(&dir, "photo.jpg", 64, 32);
        let original_hash = xxhash_rust::xxh3::xxh3_64(&fs::read(&item.path).expect("read"));
        engine
            .catalog
            .set_edits(&[(item.id, crop_edits(0.0, 0.0, 0.5, 0.5))])
            .expect("edits write");

        let frame = request_thumbnail_frame(&engine, item.id, 32);
        assert_eq!(
            (frame.width, frame.height),
            (32, 16),
            "the thumbnail shows the top-left quarter of the 64x32 decode"
        );
        assert_eq!(
            xxhash_rust::xxh3::xxh3_64(&fs::read(&item.path).expect("read")),
            original_hash,
            "an edit must never rewrite the original file"
        );
        let cached = engine
            .cache
            .get_cached(item.id, &item.path, item.file_size, item.mtime_epoch_ms, 64)
            .expect("a 2x crop decode is cached at its decode size");
        assert_eq!(
            (cached.width, cached.height),
            (64, 32),
            "the cache holds the uncropped decode"
        );

        let unedited = {
            let (tx, rx) = mpsc::channel();
            engine.request_unedited_frame(item.id, 64, move |frame| {
                let _ = tx.send(frame);
            });
            rx.recv_timeout(TEST_WAIT)
                .expect("callback arrives")
                .expect("frame decodes")
        };
        assert_eq!(
            (unedited.width, unedited.height),
            (64, 32),
            "the unedited frame ignores the crop"
        );
        drop(engine);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn unedited_frame_ignores_rotation_and_exposure() {
        let dir = temp_library("unedited_frame");
        let (engine, item) = engine_with_image(&dir, "photo.png", 8, 4);
        let plain = request_thumbnail_frame(&engine, item.id, 16);
        engine
            .catalog
            .set_edits(&[(
                item.id,
                Edits {
                    rotation: 1,
                    exposure_ev: 1.0,
                    ..Default::default()
                },
            )])
            .expect("edits write");
        assert_eq!(
            (request_thumbnail_frame(&engine, item.id, 16).width),
            4,
            "the thumbnail is rotated"
        );
        let (tx, rx) = mpsc::channel();
        engine.request_unedited_frame(item.id, 16, move |frame| {
            let _ = tx.send(frame);
        });
        let unedited = rx
            .recv_timeout(TEST_WAIT)
            .expect("callback arrives")
            .expect("frame decodes");
        assert_eq!(
            ((unedited.width, unedited.height), unedited.data),
            ((plain.width, plain.height), plain.data),
            "the unedited frame matches the decode before any edit"
        );
        drop(engine);
        let _ = fs::remove_dir_all(&dir);
    }

    fn cache_file_count(dir: &Path) -> usize {
        fs::read_dir(dir.join("cache"))
            .map(|entries| entries.filter_map(|e| e.ok()).count())
            .unwrap_or(0)
    }

    #[test]
    fn oversize_edit_decode_is_not_cached() {
        let dir = temp_library("oversize_edit");
        let (engine, item) = engine_with_image(&dir, "photo.png", 64, 64);
        request_thumbnail_frame(&engine, item.id, 16);
        assert_eq!(
            cache_file_count(&dir),
            1,
            "control: an unedited decode is written to the cache"
        );
        engine
            .catalog
            .set_edits(&[(item.id, crop_edits(0.5, 0.5, 0.25, 0.25))])
            .expect("edits write");
        // Decode size is 16 / 0.25 = 64 > 2 x 16: decoded per request.
        let frame = request_thumbnail_frame(&engine, item.id, 16);
        assert_eq!((frame.width, frame.height), (16, 16), "the crop is applied");
        assert_eq!(
            cache_file_count(&dir),
            1,
            "a decode above twice the target must not be written to the disk cache"
        );
        drop(engine);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn uncacheable_edit_decode_still_reads_the_cache() {
        let dir = temp_library("uncacheable_read");
        let (engine, item) = engine_with_image(&dir, "photo.png", 64, 64);
        // The unedited frame at 64 is cached, the size a 0.25 crop at 16 needs.
        let (tx, rx) = mpsc::channel();
        engine.request_unedited_frame(item.id, 64, move |frame| {
            let _ = tx.send(frame);
        });
        rx.recv_timeout(TEST_WAIT)
            .expect("callback arrives")
            .expect("frame decodes");
        fs::remove_file(&item.path).expect("original removable");
        engine
            .catalog
            .set_edits(&[(item.id, crop_edits(0.5, 0.5, 0.25, 0.25))])
            .expect("edits write");
        let frame = request_thumbnail_frame(&engine, item.id, 16);
        assert_eq!(
            (frame.width, frame.height),
            (16, 16),
            "a decode too large to write is still read from the cache when present"
        );
        drop(engine);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cropped_viewer_decode_is_not_cached() {
        let dir = temp_library("viewer_crop");
        let (engine, item) = engine_with_image(&dir, "photo.png", 2100, 1050);
        // The viewer's request size (VIEWER_IMAGE_SIZE in the UI).
        let viewer = 2048;
        let plain = request_thumbnail_frame(&engine, item.id, viewer);
        assert_eq!((plain.width, plain.height), (2048, 1024));
        assert_eq!(
            cache_file_count(&dir),
            1,
            "control: the unedited viewer decode is cached"
        );
        engine
            .catalog
            .set_edits(&[(item.id, crop_edits(0.0, 0.0, 0.5, 0.5))])
            .expect("edits write");
        // Decoded at the original's 2100 px: over the 2048 cache bound.
        let frame = request_thumbnail_frame(&engine, item.id, viewer);
        assert_eq!(
            (frame.width, frame.height),
            (1050, 525),
            "the crop comes from a full-size decode"
        );
        assert_eq!(
            cache_file_count(&dir),
            1,
            "a cropped viewer decode must not be written to the disk cache"
        );
        drop(engine);
        let _ = fs::remove_dir_all(&dir);
    }

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
}
