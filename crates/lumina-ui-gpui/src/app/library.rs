use gpui::*;
use lumina_core::{Album, EngineEvent, LuminaEngine, MediaId, MediaItem, SmartAlbum, SystemAlbum};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::Instant;

use crate::app::history::NavigationSnapshot;
use crate::app::import::ImportDialog;
use crate::app::types::UiMessage;
use crate::app::LuminaApp;
use crate::bridge::{frame_to_gpui_image, video_frame_to_gpui_image};
use crate::config::LastView;
use crate::player::VideoPlayerBackend;
use crate::views::grid::GridState;
use crate::views::sidebar::AlbumSelection;

/// Maps the persisted view onto the current library, falling back to All when
/// the saved smart album or album no longer exists.
fn resolve_restored_view(
    view: LastView,
    user_albums: &[SmartAlbum],
    albums: &[Album],
) -> AlbumSelection {
    match view {
        LastView::User(id) if !user_albums.iter().any(|album| album.id == id) => {
            AlbumSelection::AllLibrary
        }
        LastView::Album(id) if !albums.iter().any(|album| album.id == id) => {
            AlbumSelection::AllLibrary
        }
        view => view.to_selection(),
    }
}

/// How close the player position must be to the requested seek time before a
/// paused frame poll is trusted.
///
/// The native bridge copies the pixel buffer for the item's *current* time, so
/// polling before AVFoundation has applied the seek would hand back the
/// pre-seek frame. The position gate covers both forward and backward seeks;
/// the bounded refresh window still stops the polling.
///
/// Half a 30 fps frame, so a single frame step cannot pass the gate before
/// the seek lands.
const SEEK_FRAME_TOLERANCE_SECS: f64 = 0.016;

/// The action an error banner's Retry re-runs.
pub type ErrorRetry = fn(&mut LuminaApp, &mut Context<LuminaApp>);

impl LuminaApp {
    pub fn handle_ui_message(&mut self, msg: UiMessage, cx: &mut Context<Self>) {
        match msg {
            UiMessage::LibraryEvent { generation, event } => {
                if generation == self.library_generation {
                    self.handle_ui_message(UiMessage::Engine(event), cx);
                }
            }
            UiMessage::Engine(evt) => {
                let mut should_refresh_counts = false;
                let mut should_clear_selection = false;
                let mut should_restore_session = false;
                let scan_completed = matches!(evt, EngineEvent::ScanComplete { .. });
                let had_multi_selection = !self.selected_indices.is_empty();
                // Read before ScanComplete clears a selection made during the
                // scan: a card the user picked still counts as them acting.
                let user_moved = self.pending_restore.is_some()
                    && self.navigation_snapshot() != NavigationSnapshot::default();
                let selected_item_id = self.selected_item.as_ref().map(|item| item.id);
                let mut removed_textures = Vec::new();
                let mut error = None;
                let mut selected_refresh = None;
                let mut changed_files = Vec::new();

                if let Some(ref mut state) = self.grid_state {
                    match evt {
                        EngineEvent::BatchDiscovered { items } => {
                            if let Some(scan) = self.first_scan.as_mut() {
                                scan.add_items(&items);
                            }
                            // A re-upserted file may decode now.
                            for item in &items {
                                state.clear_failed(item.id);
                            }
                            // A reconnected file keeps its id at a new path.
                            // Re-read it: the batch carries the file's own
                            // date, not an edited one.
                            if let Some(id) = selected_item_id
                                .filter(|id| items.iter().any(|item| item.id == *id))
                            {
                                selected_refresh = state.engine.get_item_by_id(id);
                            }
                            let new_total = state.engine.total_count();
                            let structural_change = new_total != state.total_count;
                            state.total_count = new_total;
                            // Initial scan batches are followed by ScanComplete; only
                            // refresh sidebar counts for post-scan (watcher) changes.
                            if !state.is_scanning {
                                // Outside a scan a batch is the watcher's: files
                                // whose signature changed (an editor saved over
                                // one) or that moved. Drop what was drawn from
                                // an old file, except after a pure rename (same
                                // size and mtime), which changes no pixels. A
                                // cached row may already be refetched, so only a
                                // row that shows the move keeps its texture.
                                // A rescan must not flush the grid, so scan
                                // batches skip this.
                                //
                                // The grid re-requests from its cached rows
                                // until the refetch lands; they must carry the
                                // new path, size and mtime (the disk cache key),
                                // or the decode reads a gone path (and marks the
                                // item failed) or brings the old thumbnail back.
                                let renamed = self.active_view.refresh_row_files(&items);
                                let textures = state.texture_cache.lock();
                                let open = self
                                    .selected_item
                                    .as_ref()
                                    .filter(|_| self.is_fullscreen_viewer);
                                changed_files.extend(
                                    items
                                        .iter()
                                        .filter(|item| {
                                            let grid = textures.contains(&item.id)
                                                && !renamed.contains(&item.id);
                                            let viewer = open.is_some_and(|open| {
                                                open.id == item.id
                                                    && !(open.path != item.path
                                                        && crate::app::active_view::same_file_signature(
                                                            open, item,
                                                        ))
                                            });
                                            grid || viewer
                                        })
                                        .map(|item| item.id),
                                );
                                drop(textures);
                                should_refresh_counts = true;
                                // Added/removed items shift position-based selection.
                                if structural_change {
                                    should_clear_selection = true;
                                }
                            }
                            cx.notify();
                        }
                        EngineEvent::ScanProgress {
                            items_found,
                            is_complete,
                            total,
                            skipped,
                            current_file,
                        } => {
                            state.scan_progress = items_found;
                            if let Some(scan) = self.first_scan.as_mut() {
                                scan.total = total;
                                scan.skipped = skipped;
                                scan.processed = items_found + skipped;
                                if current_file.is_some() {
                                    scan.current_file = current_file;
                                }
                            }
                            if is_complete {
                                state.is_scanning = false;
                            }
                            cx.notify();
                        }
                        EngineEvent::ScanComplete { total_items } => {
                            // Done: the library opens on its own.
                            self.first_scan = None;
                            self.last_scan_at = Some(std::time::Instant::now());
                            state.total_count = total_items;
                            state.is_scanning = false;
                            should_refresh_counts = true;
                            // Indices may have shifted while scanning.
                            if had_multi_selection {
                                should_clear_selection = true;
                            }
                            // The startup session restore waits for this event so
                            // user albums, counts, and tags exist before it runs.
                            should_restore_session = self.pending_restore.is_some();
                            cx.notify();
                        }
                        EngineEvent::ItemRemoved(id) => {
                            state.total_count = state.engine.total_count();
                            if let Some(old) = state.texture_cache.lock().pop(&id) {
                                removed_textures.push(old);
                            }
                            state.in_flight.lock().remove(&id);
                            state.clear_failed(id);
                            // The media is gone; its resume point can never load.
                            state.video_resume.positions.remove(id);
                            should_refresh_counts = true;
                            // Removals shift every following index; clear rather than
                            // risk batch actions targeting different files.
                            if had_multi_selection || selected_item_id == Some(id) {
                                should_clear_selection = true;
                            }
                            cx.notify();
                        }
                        EngineEvent::ItemsMissing(_) => {
                            should_refresh_counts = true;
                            cx.notify();
                        }
                        EngineEvent::ImportProgress { done, total } => {
                            if self.import.progress.is_some() {
                                self.import.progress = Some((done, total));
                                cx.notify();
                            }
                        }
                        EngineEvent::ImportChecking { checked, total } => {
                            if let Some(ImportDialog::Planning { checked: shown, .. }) =
                                self.import.dialog.as_mut()
                            {
                                *shown = Some((checked, total));
                                cx.notify();
                            }
                        }
                        EngineEvent::Error(message) => error = Some(message),
                    }
                }

                if let Some(message) = error {
                    self.set_error(message, cx);
                }
                if let Some(fresh) = selected_refresh {
                    self.selected_item = Some(fresh);
                }
                for image in removed_textures {
                    self.retire_image(image);
                }
                if !changed_files.is_empty() {
                    // Also reloads the open viewer photo.
                    self.evict_rendered(&changed_files);
                }
                if should_clear_selection {
                    self.clear_selection(cx);
                }
                if should_refresh_counts {
                    self.refresh_album_counts();
                    cx.notify();
                }
                if should_restore_session {
                    self.apply_restored_session(user_moved, cx);
                }
                if scan_completed && std::mem::take(&mut self.import.pending_import_after_open) {
                    self.pick_import_folders(cx);
                }
            }
            UiMessage::ThumbnailLoaded { id, frame } => {
                let Some(state) = self.grid_state.as_ref() else {
                    return;
                };
                state.in_flight.lock().remove(&id);
                match frame_to_gpui_image(frame) {
                    Ok(gpui_img) => {
                        state.clear_failed(id);
                        // `push` hands back the LRU entry it evicts (or the entry
                        // it replaces); its texture must be freed too.
                        let evicted = state.texture_cache.lock().push(id, gpui_img);
                        state.mark_fresh_thumb(id);
                        if let Some((_, old)) = evicted {
                            self.retire_image(old);
                        }
                    }
                    Err(e) => {
                        log::warn!("thumbnail for {id:?} could not be converted: {e:#}");
                        state.mark_failed(id);
                    }
                }
                cx.notify();
            }
            // Already recorded in the failure set by the decode callback.
            UiMessage::ThumbnailFailed => cx.notify(),
            UiMessage::ViewerLoaded { id, frame } => {
                // A poster must not replace live frames of the video the
                // viewer is playing.
                let playing_here = !self.video.in_pip
                    && self.video.player.is_some()
                    && self.video.item.as_ref().is_some_and(|item| item.id == id);
                // Nor may a saved frame replace the edit preview.
                let editing_here = self.edits.session.as_ref().is_some_and(|s| s.item == id);
                let keep_shown = playing_here || editing_here;
                if let Some(current) = self.selected_item.as_ref().filter(|_| !keep_shown) {
                    if current.id == id {
                        match frame_to_gpui_image(frame) {
                            Ok(gpui_img) => {
                                self.viewer_failed = None;
                                self.set_viewer_image(Some(gpui_img));
                            }
                            Err(e) => {
                                log::warn!("viewer frame for {id:?} could not be converted: {e:#}");
                                self.viewer_failed = Some(id);
                            }
                        }
                        cx.notify();
                    }
                }
            }
            UiMessage::ViewerFailed { id } => {
                if self
                    .selected_item
                    .as_ref()
                    .is_some_and(|item| item.id == id)
                {
                    self.viewer_failed = Some(id);
                    cx.notify();
                }
            }
            UiMessage::FolderPicked(folder) => {
                self.open_library(folder, cx);
            }
            UiMessage::NewLibraryPicked(folder) => {
                let generation = self.library_generation;
                self.open_library(folder, cx);
                // Only a library that opened (not a failed open, not one
                // waiting on the downloads prompt) gets the import.
                self.import.pending_import_after_open = self.library_generation != generation;
            }
            UiMessage::ExportFolderPicked(folder) => {
                self.export_to_folder(folder, cx);
            }
            UiMessage::LocatePicked(id, path) => {
                self.apply_locate(id, path, cx);
            }
            UiMessage::ImportPicked(paths) => {
                self.begin_import(paths, cx);
            }
            UiMessage::VideoFrameTick => {
                self.video.frame_pending.store(false, Ordering::Release);
                let playing = self.video.is_playing;
                let mut frame = None;
                let mut playback_ended = false;
                let mut stop_ticker = false;

                if let Some(ref mut player) = self.video.player {
                    if playing {
                        self.video.playback_time = player.position();
                        if player.is_playing() {
                            frame = player.next_frame();
                        } else {
                            playback_ended = true;
                        }
                    } else if let Some(deadline) = self.video.seek_refresh_until {
                        // Bounded post-seek refresh while paused. `seek_video_to_time`
                        // and the coalesced drag seeks arm this deadline; polling
                        // stops as soon as the frame arrives or the window expires,
                        // so a missing frame can never keep the ticker alive.
                        if Instant::now() >= deadline {
                            stop_ticker = true;
                        } else {
                            // Only trust a frame once the player has actually
                            // moved to the requested time; before that the
                            // output would hand back the pre-seek frame.
                            let position = player.position();
                            let landed = match self.video.step_from {
                                Some(from) => position != from,
                                None => {
                                    (position - self.video.playback_time).abs()
                                        <= SEEK_FRAME_TOLERANCE_SECS
                                }
                            };
                            if landed {
                                self.video.playback_time = position;
                                frame = player.next_frame();
                            }
                        }
                    } else {
                        // No playback and no pending refresh: the ticker has no
                        // work. Stop it instead of polling a paused player.
                        stop_ticker = true;
                    }
                } else {
                    self.video.seek_refresh_until = None;
                    stop_ticker = true;
                }

                if playback_ended {
                    self.video.is_playing = false;
                    // Playback can end while a scrub drag is active. Do not
                    // leave the transport/playhead stuck in scrub mode.
                    self.video.is_scrubbing = false;
                    self.video.scrub_time = None;
                    self.video.last_scrub_seek = None;
                    self.video.seek_refresh_until = None;
                    self.stop_video_ticker();
                    cx.notify();
                } else if let Some(frame) = frame {
                    if let Ok(img) = video_frame_to_gpui_image(frame) {
                        self.show_video_frame(img);
                        if !playing {
                            // One frame is all the paused refresh needs.
                            self.video.seek_refresh_until = None;
                            self.stop_video_ticker();
                        }
                        cx.notify();
                    }
                } else if stop_ticker {
                    if !playing {
                        self.video.seek_refresh_until = None;
                    }
                    self.stop_video_ticker();
                }
                // Throttled by playback progress, not wall-clock time, so tests
                // stay deterministic and the map does not churn every frame.
                self.note_video_resume_progress();
            }
            UiMessage::FloatingInspectorClosed => {
                self.floating_inspector_closed(cx);
            }
            UiMessage::SelectedTagsLoaded {
                request,
                single_tags,
                counts,
            } => {
                if self.selected_tags_request == request {
                    self.selected_item_tags = single_tags;
                    self.selected_tag_counts = counts;
                    cx.notify();
                }
            }
            UiMessage::FloatingPipClosed(window) => {
                self.floating_pip_closed(window, cx);
            }
            UiMessage::DuplicatesLoaded {
                generation,
                groups,
                items_count,
            } => {
                if generation == self.library_generation {
                    self.duplicate_groups = groups;
                    self.duplicate_items_count = items_count;
                    cx.notify();
                }
            }
            UiMessage::OpenFloatingInspector | UiMessage::OpenFloatingPip => {}
        }
    }

    /// Shows the error banner for a recoverable failure. "Title: detail"
    /// splits into a title line and a description.
    pub fn set_error(&mut self, message: impl Into<String>, cx: &mut Context<Self>) {
        self.error_message = Some(message.into());
        self.error_retry = None;
        cx.notify();
    }

    /// Like [`Self::set_error`], with a Retry button (and ⌘K "Retry Failed
    /// Action") that runs `retry`.
    pub fn set_error_with_retry(
        &mut self,
        message: impl Into<String>,
        retry: ErrorRetry,
        cx: &mut Context<Self>,
    ) {
        self.set_error(message, cx);
        self.error_retry = Some(retry);
    }

    /// Clears the banner and runs its retry, if it has one.
    pub fn retry_error(&mut self, cx: &mut Context<Self>) {
        if let Some(retry) = self.error_retry.take() {
            self.dismiss_error(cx);
            retry(self, cx);
        }
    }

    pub fn dismiss_error(&mut self, cx: &mut Context<Self>) {
        if let Some(state) = &self.grid_state {
            state.engine.rearm_read_errors();
        }
        self.error_message = None;
        self.error_retry = None;
        cx.notify();
    }

    pub fn open_library(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.ask_before_leaving(
            crate::app::downloads::DownloadsConfirm::OpenLibrary(path.clone()),
            cx,
        ) {
            return;
        }
        // A folder chosen by the user is a fresh session: drop the startup
        // restore context so another library's view or selection is never applied.
        self.pending_restore = None;
        self.config.clear_session_state();
        self.open_library_inner(path, cx);
    }

    /// Opens a library without touching the pending startup restore.
    pub(crate) fn open_library_inner(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if let Some(ref state) = self.grid_state {
            state.engine.cancel_scan();
        }
        // PiP plays media from the library being closed.
        self.close_pip(cx);

        // Tests must never write the user's real config or catalogs: they keep
        // the catalog inside the (temporary) library folder instead.
        #[cfg(not(test))]
        let opened = LuminaEngine::open(path.clone());
        #[cfg(test)]
        let opened = LuminaEngine::open_with_paths(
            path.clone(),
            path.join("catalog.db"),
            Some(path.join("cache")),
        );

        let (engine, rx) = match opened {
            Ok(res) => res,
            Err(e) => {
                log::error!("failed to open library at {:?}: {e:?}", path);
                self.set_error(format!("Could not open library: {e}"), cx);
                return;
            }
        };
        // Only a library that opened becomes the startup library and a recent.
        self.config.last_library_path = Some(path.clone());
        crate::app::recents::push_recent(&mut self.config.recent_libraries, &path);
        self.welcome.missing = None;
        self.welcome.focused_recent = None;
        #[cfg(not(test))]
        if let Err(err) = self.config.save() {
            self.set_error(format!("Could not save settings: {err}"), cx);
        }

        self.last_scan_at = None;
        // Share the config's resume map so playback positions recorded in the
        // grid are what the next config save writes to disk.
        // An empty catalog gets the "Building your library" screen.
        self.first_scan =
            (engine.total_count() == 0).then(crate::app::first_scan::FirstScanState::new);
        // Downloads belong to the library they were started in.
        self.downloads
            .reset(Some(crate::app::downloads::real_job(engine.clone())));
        let grid_state =
            GridState::with_resume_positions(engine, self.config.resume_positions.clone());
        if let Some(old) = self.grid_state.replace(grid_state) {
            let mut cache = old.texture_cache.lock();
            while let Some((_, image)) = cache.pop_lru() {
                self.retire_image(image);
            }
        }
        self.load_downloads(cx);
        self.selected_item = None;
        self.selected_index = None;
        self.selected_indices.clear();
        self.selection_anchor = None;
        self.selected_item_tags.clear();
        self.selected_tag_counts.clear();
        self.batch_summary = Default::default();
        self.set_viewer_image(None);
        self.selection = AlbumSelection::AllLibrary;
        // Album and media ids restart with each library: a dialog left open
        // would act on the new library's rows with the old ids (a Delete
        // could trash unrelated files), so every id-holding dialog closes.
        self.album_draft = Default::default();
        self.albums = Default::default();
        self.delete_confirm_state = None;
        self.context_menu_state = None;
        self.close_tag_manager(cx);
        // Undo entries hold ids from the old library; replaying them here
        // would change the wrong rows.
        self.undo = Default::default();
        // An import belongs to the library it was started in.
        self.abandon_import(cx);
        self.search_query.clear();
        self.search_focused = false;
        self.active_year = None;
        self.is_fullscreen_viewer = false;
        self.discard_edit_draft();
        self.edits.reset_for_library();
        // A new library is a jump, not user navigation: start a fresh trail.
        self.reset_navigation_history();

        // Drop and join the previous event-receiver thread to prevent orphaned threads.
        if let Some(handle) = self.event_receiver_thread.take() {
            let _ = handle.join();
        }

        // The old library's cancelled scan still queues events (a final
        // ScanComplete); the generation keeps them off the new library.
        self.library_generation += 1;
        let generation = self.library_generation;
        let tx = self.ui_tx.clone();
        self.event_receiver_thread = std::thread::Builder::new()
            .name("lumina-event-receiver".to_string())
            .spawn(move || {
                for event in rx {
                    let _ = tx.send_blocking(UiMessage::LibraryEvent { generation, event });
                }
            })
            .ok();

        self.refresh_album_counts();
        self.reload_edited_ids(cx);
        cx.notify();
    }

    pub fn pick_folder_dialog(&mut self, cx: &mut Context<Self>) {
        let current_dir = self
            .grid_state
            .as_ref()
            .map(|s| s.engine.library_root().to_path_buf())
            .or_else(|| self.config.last_library_path.clone());

        let tx = self.ui_tx.clone();
        // Tests count the request instead of opening a real dialog.
        #[cfg(test)]
        {
            let _ = (current_dir, tx, cx);
            FOLDER_PICKS.with(|n| n.set(n.get() + 1));
        }
        #[cfg(not(test))]
        cx.spawn(async move |_this, _cx| {
            let mut dialog = rfd::AsyncFileDialog::new().set_title("Choose Photos Library Folder");
            if let Some(ref dir) = current_dir {
                dialog = dialog.set_directory(dir);
            }
            if let Some(handle) = dialog.pick_folder().await {
                let _ = tx.send_blocking(UiMessage::FolderPicked(handle.path().to_path_buf()));
            }
        })
        .detach();
    }

    /// Opens a folder picker for exporting the current selection.
    pub fn export_pick_folder(&mut self, cx: &mut Context<Self>) {
        // Refuse before the picker, so the user does not choose a folder
        // only to be told an export is already running.
        if self.jobs.export.is_some() {
            self.show_toast(crate::i18n::t("jobs.export_running"), cx);
            return;
        }
        let selected = self.get_selected_ids();
        if selected.is_empty() {
            self.set_error("Select at least one item to export", cx);
            return;
        }
        let tx = self.ui_tx.clone();
        cx.spawn(async move |_this, _cx| {
            if let Some(handle) = rfd::AsyncFileDialog::new()
                .set_title(format!("Export {} item(s)", selected.len()))
                .pick_folder()
                .await
            {
                let _ =
                    tx.send_blocking(UiMessage::ExportFolderPicked(handle.path().to_path_buf()));
            }
        })
        .detach();
    }

    /// The view's count from the sidebar's cached counts, without querying.
    pub(crate) fn cached_active_count(&self) -> Option<usize> {
        if !self.search_query.is_empty() {
            return None;
        }
        if let Some(year) = self.active_year {
            return self
                .timeline_year_buckets
                .iter()
                .find(|(y, _)| *y == year)
                .map(|(_, count)| *count);
        }
        match &self.selection {
            AlbumSelection::AllLibrary => {
                (self.total_library_count > 0).then_some(self.total_library_count)
            }
            AlbumSelection::System(album) => self.system_album_counts.get(album).copied(),
            AlbumSelection::User(id) => self.user_album_counts.get(id).copied(),
            AlbumSelection::Album(id) => self.albums.counts.get(id).copied(),
            AlbumSelection::Trash => self
                .system_album_counts
                .get(&SystemAlbum::RecentlyDeleted)
                .copied(),
            AlbumSelection::Tag(tag) => self
                .library_tag_counts
                .iter()
                .find(|(t, _)| t == tag)
                .map(|(_, count)| *count),
            // Cached by refresh_album_counts.
            AlbumSelection::Untagged => Some(self.untagged_count),
            AlbumSelection::Duplicates => Some(self.duplicate_items_count),
        }
    }

    /// The view's count for an action: cached counts first, then the active
    /// view cache, and only then a query. Rendering uses
    /// [`Self::displayed_count`], which never queries.
    pub fn get_active_count(&self) -> usize {
        let Some(state) = self.grid_state.as_ref() else {
            return 0;
        };
        self.cached_active_count()
            .or_else(|| self.fresh_view_count())
            .unwrap_or_else(|| self.view_query().count(&state.engine))
    }

    /// The view's items at `range` for an action: from the active view cache
    /// when it is current, else a query. Rendering uses
    /// [`Self::displayed_rows`], which never queries.
    pub fn get_active_slice(&self, range: std::ops::Range<usize>) -> Vec<MediaItem> {
        let Some(state) = self.grid_state.as_ref() else {
            return Vec::new();
        };
        if range.is_empty() {
            return Vec::new();
        }
        self.fresh_view_rows(&range)
            .unwrap_or_else(|| self.view_query().slice(&state.engine, range))
    }

    pub fn select_album(&mut self, selection: AlbumSelection, cx: &mut Context<Self>) {
        let departing = self.navigation_snapshot();
        self.selection = selection;
        self.search_query.clear();
        self.search_focused = false;
        self.active_year = None;
        if self.selection == AlbumSelection::Duplicates {
            self.refresh_duplicates();
        }
        self.clear_selection(cx);
        self.record_navigation_change(departing);
    }

    pub fn toggle_shortcuts_overlay(&mut self, cx: &mut Context<Self>) {
        self.shortcuts_overlay_open = !self.shortcuts_overlay_open;
        self.shortcuts_scroll.set_offset(Point::default());
        cx.notify();
    }

    /// Scrolls the shortcut sheet by `dy` pixels (positive is down), clamped
    /// to its content.
    pub fn scroll_shortcuts(&mut self, dy: f32, cx: &mut Context<Self>) {
        let handle = &self.shortcuts_scroll;
        let max = handle.max_offset().y;
        let mut offset = handle.offset();
        offset.y = (offset.y - px(dy)).clamp(-max, px(0.0));
        handle.set_offset(offset);
        cx.notify();
    }

    pub fn close_shortcuts_overlay(&mut self, cx: &mut Context<Self>) {
        if self.shortcuts_overlay_open {
            self.shortcuts_overlay_open = false;
            cx.notify();
        }
    }

    /// Records a short user-facing confirmation for the status bar.
    pub fn set_last_action(&mut self, message: impl Into<String>) {
        self.last_action = Some(message.into());
    }

    /// Keeps XMP sidecars current after metadata edits, off the UI thread:
    /// a large batch writes thousands of files. Failures are logged but do
    /// not roll back the SQLite write. Sidecars another app wrote are left
    /// alone; the status bar says how many (the catalog keeps the edit).
    pub fn sync_metadata_sidecars(&self, ids: &[lumina_core::MediaId], cx: &mut Context<Self>) {
        if ids.is_empty() {
            return;
        }
        let Some(engine) = self.grid_state.as_ref().map(|state| state.engine.clone()) else {
            return;
        };
        let ids = ids.to_vec();
        let task = cx
            .background_executor()
            .spawn(async move { engine.write_metadata_sidecars(&ids) });
        cx.spawn(async move |this, cx| {
            let skipped = match task.await {
                Ok(report) => report.skipped_foreign.len(),
                Err(err) => {
                    log::warn!("could not update XMP sidecars: {err:?}");
                    return;
                }
            };
            if skipped == 0 {
                return;
            }
            // The app may be gone; then there is no status bar to update.
            let _ = this.update(cx, |app, cx| {
                app.set_last_action(if skipped == 1 {
                    "1 XMP sidecar was left unchanged: another app wrote it, or Lumina cannot read it".to_string()
                } else {
                    format!("{skipped} XMP sidecars were left unchanged: another app wrote them, or Lumina cannot read them")
                });
                cx.notify();
            });
        })
        .detach();
    }

    /// Drops keyboard focus from the toolbar search field.
    pub fn blur_search(&mut self, cx: &mut Context<Self>) {
        if self.search_focused {
            self.search_focused = false;
            // The dropdown is only open while the field is focused; clear the
            // dismissal flag so the next focus starts with suggestions again.
            self.search_suggestions.dismissed = false;
            self.search_suggestions.selected = None;
            cx.notify();
        }
    }

    /// Current suggestion rows for the toolbar search field.
    pub fn clear_search(&mut self, cx: &mut Context<Self>) {
        let departing = self.navigation_snapshot();
        self.search_query.clear();
        self.search_focused = false;
        self.active_year = None;
        self.clear_selection(cx);
        self.record_navigation_change(departing);
    }

    pub fn set_search_query(&mut self, query: &str, cx: &mut Context<Self>) {
        let departing = self.navigation_snapshot();
        self.search_query = query.to_string();
        // Before clearing the year: the helper snapshots the album as left.
        self.scope_search_to_library();
        self.active_year = None;
        self.clear_selection(cx);
        self.record_navigation_change(departing);
    }

    pub fn set_active_year(&mut self, year: Option<i32>, cx: &mut Context<Self>) {
        let departing = self.navigation_snapshot();
        self.active_year = year;
        // A year shows the whole library (it overrides the selection in
        // `ViewQuery`), so the sidebar must say Library, as a search does.
        // Back still returns to the album through `departing`.
        if year.is_some() {
            self.selection = AlbumSelection::AllLibrary;
        }
        self.search_query.clear();
        self.search_focused = false;
        self.clear_selection(cx);
        self.record_navigation_change(departing);
    }

    /// Applies the one-shot session captured at startup.
    ///
    /// Called exactly once, after the initial scan completes so the library's
    /// user albums and counts are known. The view/search/year are restored even
    /// when the saved media item has disappeared; in that case no item is selected.
    ///
    /// `user_moved`: the live state left the startup default (album, typed
    /// search, year, card, viewer) before the scan finished. Their choice
    /// then wins: the restore is dropped and their Back trail stays.
    fn apply_restored_session(&mut self, user_moved: bool, cx: &mut Context<Self>) {
        if self
            .grid_state
            .as_ref()
            .map(|state| state.is_scanning)
            .unwrap_or(true)
        {
            return;
        }
        let Some(restored) = self.pending_restore.take() else {
            return;
        };
        // One snapshot check covers every path, including a typed search
        // that never records a navigation change.
        if user_moved {
            self.save_config();
            return;
        }

        self.selection =
            resolve_restored_view(restored.view, &self.user_smart_albums, &self.albums.list);
        self.search_query = restored.search_query;
        // A session saved before D11 can hold an album with a query. The
        // trail is reset below, so no Back step is needed.
        if !self.search_query.trim().is_empty() {
            self.selection = AlbumSelection::AllLibrary;
        }
        self.search_focused = false;
        self.active_year = restored.active_year;
        // A session saved before years were scoped to the library.
        if self.active_year.is_some() {
            self.selection = AlbumSelection::AllLibrary;
        }

        // The startup open already cleared these, but restoring must never
        // inherit stale position-based state if a scan lands late.
        self.selected_item = None;
        self.selected_index = None;
        self.selected_indices.clear();
        self.selection_anchor = None;
        self.selected_item_tags.clear();
        self.selected_tag_counts.clear();
        self.batch_summary = Default::default();
        self.set_viewer_image(None);

        if let Some(id) = restored.selected_media_id {
            let media_id = MediaId(id);
            if let Some(index) = self.find_active_index(media_id) {
                if let Some(item) = self.get_active_slice(index..index + 1).into_iter().next() {
                    self.selected_indices.insert(index);
                    self.selection_anchor = Some(index);
                    self.selected_index = Some(index);
                    self.select_item_internal(item, cx);
                    self.refresh_selected_tags();
                    self.update_batch_summary();
                }
            }
        }
        self.restore_grid_scroll(restored.scroll_anchor_id.map(MediaId));
        // The startup restore is a jump, not user navigation: seed the trail
        // with the restored destination so Back cannot return to pre-restore state.
        self.reset_navigation_history();
        // Persist the applied session once so a startup window-bounds save cannot
        // leave an empty session on disk before the initial scan finishes.
        self.save_config();
        cx.notify();
    }

    /// Scrolls the restored grid so the saved anchor item's row is on top.
    ///
    /// GPUI keeps one deferred scroll, so revealing the selection afterwards
    /// would replace the anchor scroll: the selection is revealed only when
    /// the anchor is gone or the selected row falls outside the anchored
    /// viewport.
    fn restore_grid_scroll(&self, anchor: Option<MediaId>) {
        let Some(state) = self.grid_state.as_ref() else {
            return;
        };
        let cols = self.grid_columns.clamp(2, 8);
        let Some(anchor_row) = anchor
            .and_then(|id| self.find_active_index(id))
            .map(|index| index / cols)
        else {
            self.reveal_grid_selection();
            return;
        };
        state
            .scroll_handle
            .scroll_to_item_strict(anchor_row, gpui::ScrollStrategy::Top);
        let viewport = f32::from(
            state
                .scroll_handle
                .0
                .borrow()
                .base_handle
                .bounds()
                .size
                .height,
        );
        // Zero before the first layout: the anchor wins then.
        let visible_rows = (viewport / self.row_height()) as usize;
        let selected_row = self.selected_index.map(|index| index / cols);
        if selected_row.is_some_and(|row| {
            visible_rows > 0 && !(anchor_row..anchor_row + visible_rows).contains(&row)
        }) {
            self.reveal_grid_selection();
        }
    }

    /// Finds a media item's position in the active result set without per-frame SQL.
    ///
    /// The core engine has no index-of-id query for filtered views, so this walks
    /// the active slices once in bounded chunks during restore only.
    pub(crate) fn find_active_index(&self, id: MediaId) -> Option<usize> {
        const CHUNK: usize = 512;
        let total = self.get_active_count();
        let mut offset = 0;
        while offset < total {
            let end = (offset + CHUNK).min(total);
            let slice = self.get_active_slice(offset..end);
            if slice.is_empty() {
                break;
            }
            if let Some(position) = slice.iter().position(|item| item.id == id) {
                return Some(offset + position);
            }
            offset = end;
        }
        None
    }

    pub fn refresh_album_counts(&mut self) {
        if let Some(ref state) = self.grid_state {
            let engine = &state.engine;
            self.total_library_count = engine.total_count();
            let mut sys = HashMap::new();
            for album in lumina_core::SystemAlbum::all() {
                sys.insert(*album, engine.query_system_album_count(*album));
            }
            self.system_album_counts = sys;

            let albums = engine.list_smart_albums();
            let mut usr = HashMap::new();
            for a in &albums {
                usr.insert(a.id, engine.query_smart_album_count(a));
            }
            self.user_album_counts = usr;
            self.user_smart_albums = albums;
            let engine = engine.clone();
            self.refresh_albums(&engine);
            self.library_tag_counts = engine.query_tag_counts();
            self.untagged_count = engine.query_untagged_count();
            self.duplicate_items_count = engine.query_duplicate_count();
            if self.selection == AlbumSelection::Duplicates {
                self.duplicate_groups = engine.query_duplicate_groups();
            }
            self.item_ratings = engine.get_ratings_map();
            self.item_color_labels = engine.get_color_labels_map();
            self.item_flags = engine.get_flags_map();
            self.item_captions = engine.get_captions_map();
            self.missing_items = engine.get_missing_ids();
            self.month_header.counts = engine
                .get_timeline_buckets("month")
                .into_iter()
                .filter_map(|b| {
                    let (year, month) = b.bucket_key.split_once('-')?;
                    Some(((year.parse().ok()?, month.parse().ok()?), b.item_count))
                })
                .collect();
            let buckets = engine.get_timeline_buckets("year");
            self.timeline_year_buckets = buckets
                .into_iter()
                .filter_map(|b| {
                    let yr = b.bucket_key.parse::<i32>().ok()?;
                    Some((yr, b.item_count))
                })
                .collect();
        }
        // Aliases change with tag renames, so they reload with the counts.
        self.refresh_keyword_aliases();

        // Tag mutations, scan removals, and undo/redo all refresh counts here.
        // If the active tag view lost its last item, drop the stale
        // position-based selection instead of leaving it pointing nowhere.
        if matches!(
            self.selection,
            AlbumSelection::Tag(_) | AlbumSelection::Untagged
        ) && self.get_active_count() == 0
            && (self.selected_item.is_some() || !self.selected_indices.is_empty())
        {
            self.clear_selection_fields();
        }
    }

    pub fn refresh_duplicates(&mut self) {
        let Some(ref state) = self.grid_state else {
            return;
        };

        // Instant, non-blocking read of already-computed duplicate groups from SQLite cache.
        self.duplicate_groups = state.engine.get_cached_duplicate_groups();
        self.duplicate_items_count = state.engine.query_duplicate_count();

        #[cfg(test)]
        {
            // In unit/automation tests, run scan_duplicates synchronously so tests don't need async waits.
            self.duplicate_groups = state.engine.query_duplicate_groups();
            self.duplicate_items_count = state.engine.query_duplicate_count();
        }

        #[cfg(not(test))]
        {
            let engine = state.engine.clone();
            let tx = self.ui_tx.clone();
            let generation = self.library_generation;
            if let Err(err) = std::thread::Builder::new()
                .name("lumina-duplicates-scan".into())
                .spawn(move || {
                    let groups = engine.query_duplicate_groups();
                    let items_count = engine.query_duplicate_count();
                    let _ = tx.send_blocking(UiMessage::DuplicatesLoaded {
                        generation,
                        groups,
                        items_count,
                    });
                })
            {
                log::warn!("failed to spawn lumina-duplicates-scan thread: {err}");
            }
        }
    }

    pub fn merge_duplicate_group(
        &mut self,
        keep_id: MediaId,
        trash_ids: &[MediaId],
        cx: &mut Context<Self>,
    ) {
        let Some(ref state) = self.grid_state else {
            return;
        };
        if trash_ids.is_empty() {
            return;
        }
        let count = trash_ids.len();
        match state.engine.merge_duplicate_group(keep_id, trash_ids) {
            Ok(()) => {
                let changes: Vec<(MediaId, bool, bool)> =
                    trash_ids.iter().map(|id| (*id, false, true)).collect();
                self.undo
                    .undo_stack
                    .push(crate::app::undo::UndoAction::Trashed { changes });
                self.undo.redo_stack.clear();

                // If selected item was trashed, point selection to keeper
                if let Some(ref sel) = self.selected_item {
                    if trash_ids.contains(&sel.id) {
                        self.selected_item = self
                            .duplicate_groups
                            .iter()
                            .flat_map(|g| &g.items)
                            .find(|i| i.id == keep_id)
                            .cloned();
                    }
                }

                self.refresh_album_counts();
                self.refresh_duplicates();
                self.show_toast(
                    format!(
                        "Merged {count} duplicate{}",
                        if count == 1 { "" } else { "s" }
                    ),
                    cx,
                );
                cx.notify();
            }
            Err(err) => {
                self.show_toast(format!("Could not merge duplicates: {err:#}"), cx);
            }
        }
    }

    pub fn merge_all_duplicates(&mut self, cx: &mut Context<Self>) {
        let Some(ref state) = self.grid_state else {
            return;
        };
        match state.engine.merge_all_duplicates() {
            Ok(trashed_ids) => {
                let total_merged = trashed_ids.len();
                if !trashed_ids.is_empty() {
                    let changes: Vec<(MediaId, bool, bool)> =
                        trashed_ids.iter().map(|id| (*id, false, true)).collect();
                    self.undo
                        .undo_stack
                        .push(crate::app::undo::UndoAction::Trashed { changes });
                    self.undo.redo_stack.clear();

                    if let Some(ref sel) = self.selected_item {
                        if trashed_ids.contains(&sel.id) {
                            self.clear_selection_fields();
                        }
                    }
                }

                self.refresh_album_counts();
                self.refresh_duplicates();
                self.show_toast(
                    format!(
                        "Merged {total_merged} duplicate{}",
                        if total_merged == 1 { "" } else { "s" }
                    ),
                    cx,
                );
                cx.notify();
            }
            Err(err) => {
                self.show_toast(format!("Could not merge all duplicates: {err:#}"), cx);
            }
        }
    }
}

#[cfg(test)]
mod restore_tests {
    use super::*;
    use lumina_core::MatchMode;

    fn user_album(id: i64) -> SmartAlbum {
        let mut album = SmartAlbum::new(
            "Trips".to_string(),
            "folder".to_string(),
            MatchMode::All,
            Vec::new(),
        );
        album.id = id;
        album
    }

    #[core::prelude::v1::test]
    fn removed_user_album_falls_back_to_all() {
        assert_eq!(
            resolve_restored_view(LastView::User(7), &[], &[]),
            AlbumSelection::AllLibrary,
            "a removed smart album must not restore a phantom view"
        );
        assert_eq!(
            resolve_restored_view(LastView::User(7), &[user_album(7)], &[]),
            AlbumSelection::User(7),
            "an existing smart album restores its view"
        );
    }

    #[core::prelude::v1::test]
    fn tag_view_restores_without_a_user_album() {
        assert_eq!(
            resolve_restored_view(LastView::Tag("travel".to_string()), &[], &[]),
            AlbumSelection::Tag("travel".to_string()),
            "tag views do not depend on smart album ids"
        );
    }

    fn scanned_engine(prefix: &str) -> (std::path::PathBuf, std::sync::Arc<LuminaEngine>) {
        let temp_dir = std::env::temp_dir().join(format!(
            "lumina_restore_{}_{}",
            prefix,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();
        for index in 0..3 {
            std::fs::write(
                temp_dir.join(format!("restore_{index:02}.jpg")),
                b"test-jpeg-content",
            )
            .unwrap();
        }
        let (engine, rx) = LuminaEngine::open_with_paths(
            temp_dir.clone(),
            temp_dir.join("catalog.db"),
            Some(temp_dir.join("cache")),
        )
        .unwrap();
        while let Ok(event) = rx.recv_timeout(std::time::Duration::from_secs(10)) {
            if let EngineEvent::ScanComplete { .. } = event {
                break;
            }
        }
        (temp_dir, engine)
    }

    #[core::prelude::v1::test]
    fn restores_selection_after_scan_complete_and_survives_a_missing_item() {
        use crate::config::RestoredSession;

        let (temp_dir, engine) = scanned_engine("selection");
        let mut cx = crate::tests::TestApp::single();
        let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));

        view.update(cx, |app, cx| {
            app.grid_state = Some(GridState::new(engine.clone()));
            app.refresh_album_counts();
            let items = app.get_active_slice(0..3);
            assert_eq!(items.len(), 3, "the scan must produce three items");
            let target_index = 1;
            let target = items[target_index].clone();
            app.pending_restore = Some(RestoredSession {
                view: LastView::All,
                search_query: String::new(),
                active_year: None,
                selected_media_id: Some(target.id.0),
                scroll_anchor_id: None,
            });

            let total = app
                .grid_state
                .as_ref()
                .map(|state| state.total_count)
                .unwrap_or(0);
            app.handle_ui_message(
                UiMessage::Engine(EngineEvent::ScanComplete { total_items: total }),
                cx,
            );

            assert!(app.pending_restore.is_none(), "restore must run once");
            assert_eq!(
                app.selected_item.as_ref().map(|item| item.id),
                Some(target.id),
                "the saved media item must be selected after the scan"
            );
            assert_eq!(app.selected_index, Some(target_index));
            assert_eq!(app.selection_anchor, Some(target_index));
            assert!(app.selected_indices.contains(&target_index));
            assert_eq!(app.batch_summary.count, 1);
            let expected_row = target_index / app.grid_columns.clamp(2, 8);
            assert_eq!(
                app.grid_state
                    .as_ref()
                    .map(|state| state.scroll_handle.logical_scroll_top_index()),
                Some(expected_row),
                "the restored row must be revealed"
            );

            // A saved item that no longer exists must keep the view but select nothing.
            // Back to the untouched startup state first: a selection made
            // before the restore would (rightly) cancel it.
            app.clear_selection(cx);
            app.pending_restore = Some(RestoredSession {
                view: LastView::Favorites,
                search_query: String::new(),
                active_year: None,
                selected_media_id: Some(u64::MAX),
                scroll_anchor_id: None,
            });
            app.handle_ui_message(
                UiMessage::Engine(EngineEvent::ScanComplete { total_items: total }),
                cx,
            );
            assert!(app.pending_restore.is_none());
            assert_eq!(
                app.selection,
                AlbumSelection::System(SystemAlbum::Favorites),
                "the saved view must survive a missing media item"
            );
            assert!(
                app.selected_item.is_none(),
                "no stale item may stay selected"
            );
            assert!(app.selected_indices.is_empty());
        });

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

#[cfg(test)]
thread_local! {
    /// Folder pickers the app asked to open on this test thread.
    pub(crate) static FOLDER_PICKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
