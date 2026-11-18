pub mod cache;
pub mod catalog;
pub mod decoder;
mod dims;
pub mod duplicates;
pub mod edit_copy;
pub mod edits;
mod engine;
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
mod types;
pub mod update;
mod video_frame;
pub mod watcher;
pub mod xmp;

use anyhow::{Context, Result};
use crossbeam_channel::{unbounded, Receiver, Sender};
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub use cache::DiskCache;
pub use catalog::{
    Album, AlbumCard, AlbumStats, Catalog, CropRect, DateSpan, DayPlace, DeletionTarget,
    DownloadRow, DownloadState, DuplicateMerge, Edits, GeoPoint, KeeperSnapshot, LayoutCell,
    PlaceCount, PriorDownload, RejectedItems, Rights, StorageSummary, TagChange, TagReplay,
    TemplateChange, ViewScope, ViewTotals, SCHEMA_VERSION,
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
pub use types::*;
pub use video_frame::video_preview_strip;
pub use watcher::LibraryWatcher;
pub use xmp::{
    parse_xmp, read_sidecar, render_xmp, sidecar_path, write_sidecar, SidecarMetadata,
    SidecarOutcome,
};

/// Wall-clock milliseconds since the Unix epoch, as the catalog stores times.
pub(crate) fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
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

    /// Bytes by kind for Settings › Library (see [`StorageSummary`]).
    pub fn storage_summary(&self) -> StorageSummary {
        self.read("storage summary", self.catalog.storage_summary())
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

    /// Creates an album holding `members` (none for an empty album) in one
    /// transaction.
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

    /// One cover card per album, in sidebar order. Run it off the UI thread.
    pub fn album_cards(&self) -> Vec<AlbumCard> {
        self.read("album cards", self.catalog.album_cards())
    }

    /// An album's photo and video counts and capture range, for its page
    /// header. Run it off the UI thread.
    pub fn album_stats(&self, id: i64) -> AlbumStats {
        self.read("album stats", self.catalog.album_stats(id))
    }

    /// Sets an album's key photo (`None` clears it); the photo must be a
    /// member of the album.
    pub fn set_album_cover(&self, id: i64, cover: Option<MediaId>) -> Result<()> {
        self.catalog.set_album_cover(id, cover)
    }

    /// Whether `media` is a member of album `id`.
    pub fn album_contains(&self, id: i64, media: MediaId) -> Result<bool> {
        self.catalog.album_contains(id, media)
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

    /// Every stored `(make, model)` pair, for the smart album "Camera is" list.
    pub fn distinct_cameras(&self) -> Result<Vec<(String, String)>> {
        self.catalog.distinct_cameras()
    }

    pub fn toggle_favorite(&self, id: MediaId, is_favorite: bool) -> Result<()> {
        self.catalog.toggle_favorite(id, is_favorite)
    }

    pub fn batch_set_favorite(&self, ids: &[MediaId], is_favorite: bool) -> Result<()> {
        self.catalog.batch_set_favorite(ids, is_favorite)
    }

    /// Batch edits: each `set_*s` writes every change in one transaction.
    pub fn set_ratings(&self, changes: &[(MediaId, u8)]) -> Result<()> {
        self.catalog.set_ratings(changes)
    }

    pub fn get_ratings_map(&self) -> std::collections::HashMap<MediaId, u8> {
        self.read("get ratings map", self.catalog.get_ratings_map())
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

    /// The view's count, summed duration and size, and items per capture
    /// month. One grouped pass: call it off the UI thread, once per
    /// revision. A read error is logged and reads as `None`, so the header
    /// shows the plain count.
    pub fn view_totals(&self, scope: &ViewScope) -> Option<ViewTotals> {
        self.read("view totals", self.catalog.view_totals(scope).map(Some))
    }

    /// Every item of the view as a Mosaic layout cell, in the view's sort
    /// order. Reads every row: call it off the UI thread, once per revision.
    /// A read error is logged and reads as an empty layout.
    pub fn view_layout(&self, scope: &ViewScope, sort: SortSpec) -> Vec<LayoutCell> {
        self.read("view layout", self.catalog.view_layout(scope, sort))
    }

    /// The most common place of each dated day in the view, by `yyyymmdd`.
    /// One grouped pass: call it off the UI thread, once per revision. A
    /// read error is logged and reads as no places.
    pub fn day_places(&self, scope: &ViewScope) -> std::collections::HashMap<u32, DayPlace> {
        self.read("day places", self.catalog.day_places(scope))
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

    pub fn get_captions_map(&self) -> std::collections::HashMap<MediaId, String> {
        self.read("get captions map", self.catalog.get_captions_map())
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
}

#[cfg(test)]
pub(crate) mod tests {
    /// Generous debug budget for decode and scan callbacks: a 2k decode
    /// competes with the whole parallel suite. A passing test never waits it out.
    pub(crate) const TEST_WAIT: Duration = Duration::from_secs(60);

    use super::*;
    use std::fs;
    use std::sync::mpsc;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    pub(crate) fn temp_library(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("lumina_{prefix}_{nanos}"));
        fs::create_dir_all(&dir).expect("temp library dir must be creatable");
        dir
    }

    pub(crate) fn request_thumbnail_frame(
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
}
