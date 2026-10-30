use gpui::*;
use lumina_core::{EngineEvent, LuminaEngine, MediaId, MediaItem, SmartAlbum, SystemAlbum};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::Instant;

use crate::app::types::UiMessage;
use crate::app::LuminaApp;
use crate::bridge::{frame_to_gpui_image, video_frame_to_gpui_image};
use crate::config::LastView;
use crate::player::VideoPlayerBackend;
use crate::views::grid::GridState;
use crate::views::sidebar::AlbumSelection;

/// Maps the persisted view onto the current library, falling back to All when
/// the saved user smart album no longer exists.
fn resolve_restored_view(view: LastView, user_albums: &[SmartAlbum]) -> AlbumSelection {
    match view {
        LastView::User(id) if !user_albums.iter().any(|album| album.id == id) => {
            AlbumSelection::AllLibrary
        }
        view => view.to_selection(),
    }
}

/// Maximum number of committed queries kept in recent-search history.
pub(crate) const MAX_RECENT_SEARCHES: usize = 10;

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

/// Maximum number of rows rendered in the search suggestion dropdown.
pub(crate) const MAX_SEARCH_SUGGESTIONS: usize = 8;

/// Fields and quick-filter tokens offered when a query is empty or matches one
/// of them. These mirror the query grammar.
const SEARCH_FIELD_SUGGESTIONS: &[&str] = &[
    "is:favorite",
    "is:video",
    "is:raw",
    "is:trashed",
    "has:location",
    "has:tag",
    "has:color",
    "rating:>=4",
    "tag:",
    "camera:",
    "lens:",
    "city:",
    "country:",
    "iso:",
    "year:",
    "date:",
];

/// The token currently being typed: the last whitespace-separated word.
///
/// A trailing space starts a new token and therefore matches every suggestion.
pub(crate) fn current_search_token(query: &str) -> &str {
    if query.chars().next_back().is_some_and(char::is_whitespace) {
        ""
    } else {
        query.split_whitespace().next_back().unwrap_or("")
    }
}

/// Case-insensitive substring match so tag:tr finds tag:travel.
pub(crate) fn search_suggestion_matches(suggestion: &str, token: &str) -> bool {
    let token = token.trim();
    if token.is_empty() {
        return true;
    }
    suggestion.to_lowercase().contains(&token.to_lowercase())
}

/// Formats a cached tag as a query token, quoting tags that contain spaces.
pub(crate) fn tag_search_suggestion(tag: &str) -> String {
    let tag = tag.trim();
    if tag.chars().any(char::is_whitespace) || tag.contains('"') {
        format!("tag:\"{}\"", tag.replace('"', ""))
    } else {
        format!("tag:{tag}")
    }
}

fn push_search_suggestion(items: &mut Vec<String>, seen: &mut HashSet<String>, suggestion: &str) {
    if items.len() >= MAX_SEARCH_SUGGESTIONS {
        return;
    }
    let suggestion = suggestion.trim();
    if suggestion.is_empty() {
        return;
    }
    if seen.insert(suggestion.to_lowercase()) {
        items.push(suggestion.to_string());
    }
}

/// Builds the filtered suggestion list from cached state only (no SQL).
///
/// Empty queries show recent searches followed by common operators; typed
/// queries match recent searches, cached tag counts, then operators.
pub(crate) fn search_suggestion_items(
    recent_searches: &[String],
    tag_counts: &[(String, usize)],
    query: &str,
) -> Vec<String> {
    let token = current_search_token(query);
    let mut items: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    if token.is_empty() {
        // Keep room for the common operators even with a long recent list.
        for recent in recent_searches.iter().take(5) {
            push_search_suggestion(&mut items, &mut seen, recent);
        }
        for suggestion in SEARCH_FIELD_SUGGESTIONS {
            push_search_suggestion(&mut items, &mut seen, suggestion);
        }
        return items;
    }

    for recent in recent_searches {
        if search_suggestion_matches(recent, token) {
            push_search_suggestion(&mut items, &mut seen, recent);
        }
    }
    for (tag, _) in tag_counts {
        if items.len() >= MAX_SEARCH_SUGGESTIONS {
            break;
        }
        let suggestion = tag_search_suggestion(tag);
        if search_suggestion_matches(&suggestion, token) {
            push_search_suggestion(&mut items, &mut seen, &suggestion);
        }
    }
    for suggestion in SEARCH_FIELD_SUGGESTIONS {
        if items.len() >= MAX_SEARCH_SUGGESTIONS {
            break;
        }
        if search_suggestion_matches(suggestion, token) {
            push_search_suggestion(&mut items, &mut seen, suggestion);
        }
    }
    items
}

/// Records query as the newest recent search.
///
/// Returns true when a non-empty query was recorded. Entries are trimmed,
/// deduplicated case-insensitively, and bounded to MAX_RECENT_SEARCHES.
pub(crate) fn record_recent_search_entry(entries: &mut Vec<String>, query: &str) -> bool {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return false;
    }
    let key = trimmed.to_lowercase();
    entries.retain(|entry| entry.trim().to_lowercase() != key);
    entries.insert(0, trimmed.to_string());
    entries.truncate(MAX_RECENT_SEARCHES);
    true
}

/// Replaces the token being typed with suggestion, preserving earlier tokens.
pub(crate) fn apply_search_suggestion_text(query: &str, suggestion: &str) -> String {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return suggestion.to_string();
    }
    if query.chars().next_back().is_some_and(char::is_whitespace) {
        return format!("{trimmed} {suggestion}");
    }
    let mut parts: Vec<&str> = trimmed.split_whitespace().collect();
    parts.pop();
    parts.push(suggestion);
    parts.join(" ")
}

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
                let had_multi_selection = !self.selected_indices.is_empty();
                let selected_item_id = self.selected_item.as_ref().map(|item| item.id);
                let mut removed_textures = Vec::new();

                if let Some(ref mut state) = self.grid_state {
                    match evt {
                        EngineEvent::BatchDiscovered { items } => {
                            if let Some(scan) = self.first_scan.as_mut() {
                                scan.add_items(&items);
                            }
                            let new_total = state.engine.total_count();
                            let structural_change = new_total != state.total_count;
                            state.total_count = new_total;
                            // Initial scan batches are followed by ScanComplete; only
                            // refresh sidebar counts for post-scan (watcher) changes.
                            if !state.is_scanning {
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
                    }
                }

                for image in removed_textures {
                    self.retire_image(image);
                }
                if should_clear_selection {
                    self.clear_selection(cx);
                }
                if should_refresh_counts {
                    self.refresh_album_counts();
                    cx.notify();
                }
                if should_restore_session {
                    self.apply_restored_session(cx);
                }
            }
            UiMessage::ThumbnailLoaded { id, frame } => {
                let Some(state) = self.grid_state.as_ref() else {
                    return;
                };
                state.in_flight.lock().remove(&id);
                if let Ok(gpui_img) = frame_to_gpui_image(frame) {
                    // `push` hands back the LRU entry it evicts (or the entry it
                    // replaces); its texture must be freed too.
                    let evicted = state.texture_cache.lock().push(id, gpui_img);
                    if let Some((_, old)) = evicted {
                        self.retire_image(old);
                    }
                    cx.notify();
                }
            }
            UiMessage::ViewerLoaded { id, frame } => {
                // A poster must not replace live frames of the video the
                // viewer is playing.
                let playing_here = !self.video.in_pip
                    && self.video.player.is_some()
                    && self.video.item.as_ref().is_some_and(|item| item.id == id);
                if let Some(current) = self.selected_item.as_ref().filter(|_| !playing_here) {
                    if current.id == id {
                        if let Ok(gpui_img) = frame_to_gpui_image(frame) {
                            self.set_viewer_image(Some(gpui_img));
                            cx.notify();
                        }
                    }
                }
            }
            UiMessage::FolderPicked(folder) => {
                self.open_library(folder, cx);
            }
            UiMessage::ExportFolderPicked(folder) => {
                self.export_to_folder(folder, cx);
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
            UiMessage::FloatingPipClosed(window) => {
                self.floating_pip_closed(window, cx);
            }
            UiMessage::OpenFloatingInspector | UiMessage::OpenFloatingPip => {}
        }
    }

    /// Shows a dismissible error banner; used for recoverable user-facing failures.
    pub fn set_error(&mut self, message: impl Into<String>, cx: &mut Context<Self>) {
        self.error_message = Some(message.into());
        cx.notify();
    }

    pub fn dismiss_error(&mut self, cx: &mut Context<Self>) {
        self.error_message = None;
        cx.notify();
    }

    pub fn open_library(&mut self, path: PathBuf, cx: &mut Context<Self>) {
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

        self.config.last_library_path = Some(path.clone());
        // Tests must never write the user's real config or catalogs: they keep
        // the catalog inside the (temporary) library folder instead.
        #[cfg(not(test))]
        if let Err(err) = self.config.save() {
            self.set_error(format!("Could not save settings: {err}"), cx);
        }
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

        // Share the config's resume map so playback positions recorded in the
        // grid are what the next config save writes to disk.
        // An empty catalog gets the "Building your library" screen.
        self.first_scan =
            (engine.total_count() == 0).then(crate::app::first_scan::FirstScanState::new);
        let grid_state =
            GridState::with_resume_positions(engine, self.config.resume_positions.clone());
        if let Some(old) = self.grid_state.replace(grid_state) {
            let mut cache = old.texture_cache.lock();
            while let Some((_, image)) = cache.pop_lru() {
                self.retire_image(image);
            }
        }
        self.selected_item = None;
        self.selected_index = None;
        self.selected_indices.clear();
        self.selection_anchor = None;
        self.selected_item_tags.clear();
        self.selected_tag_counts.clear();
        self.batch_summary = Default::default();
        self.set_viewer_image(None);
        self.selection = AlbumSelection::AllLibrary;
        self.search_query.clear();
        self.search_focused = false;
        self.active_year = None;
        self.is_fullscreen_viewer = false;
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
        cx.notify();
    }

    pub fn pick_folder_dialog(&mut self, cx: &mut Context<Self>) {
        let current_dir = self
            .grid_state
            .as_ref()
            .map(|s| s.engine.library_root().to_path_buf())
            .or_else(|| self.config.last_library_path.clone());

        let tx = self.ui_tx.clone();
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

    /// Copies the selection into `folder`, writing XMP sidecars for annotated items.
    pub fn export_to_folder(&mut self, folder: std::path::PathBuf, cx: &mut Context<Self>) {
        let ids = self.get_selected_ids();
        if ids.is_empty() {
            return;
        }
        let Some(engine) = self.grid_state.as_ref().map(|state| state.engine.clone()) else {
            self.set_error("Open a library before exporting", cx);
            return;
        };
        match engine.export_items(&ids, &folder) {
            Ok(report) => {
                let mut summary = format!(
                    "Exported {} item(s) to {}",
                    report.exported,
                    folder.display()
                );
                if report.sidecars > 0 {
                    summary.push_str(&format!(" ({} XMP sidecar(s))", report.sidecars));
                }
                self.set_last_action(summary);
                if report.failed.is_empty() {
                    self.show_toast(format!("Exported {} item(s)", report.exported), cx);
                } else {
                    let details = report
                        .failed
                        .iter()
                        .take(3)
                        .map(|(path, error)| format!("{}: {error}", path.display()))
                        .collect::<Vec<_>>()
                        .join("; ");
                    self.set_error(
                        format!("{} export(s) failed: {details}", report.failed.len()),
                        cx,
                    );
                }
            }
            Err(err) => self.set_error(format!("Export failed: {err}"), cx),
        }
    }

    /// Imports rating/title/caption/tags from adjacent XMP sidecars for the selection.
    pub fn import_sidecars_selected(&mut self, cx: &mut Context<Self>) {
        let ids = self.get_selected_ids();
        if ids.is_empty() {
            return;
        }
        let Some(engine) = self.grid_state.as_ref().map(|state| state.engine.clone()) else {
            self.set_error("Open a library before importing metadata", cx);
            return;
        };
        match engine.import_sidecars(&ids) {
            Ok(report) => {
                self.refresh_after_history(cx);
                let mut summary = format!(
                    "Imported XMP metadata: {} sidecar(s), {} updated",
                    report.imported, report.updated
                );
                if report.skipped > 0 {
                    summary.push_str(&format!(", {} without sidecar", report.skipped));
                }
                self.set_last_action(summary);
                if !report.failed.is_empty() {
                    let details = report
                        .failed
                        .iter()
                        .take(3)
                        .map(|(path, error)| format!("{}: {error}", path.display()))
                        .collect::<Vec<_>>()
                        .join("; ");
                    self.set_error(
                        format!(
                            "{} sidecar import(s) failed: {details}",
                            report.failed.len()
                        ),
                        cx,
                    );
                } else {
                    self.show_toast(
                        format!(
                            "Imported {} sidecar(s), {} updated",
                            report.imported, report.updated
                        ),
                        cx,
                    );
                }
            }
            Err(err) => self.set_error(format!("XMP import failed: {err}"), cx),
        }
    }

    pub fn get_active_count(&self) -> usize {
        let state = match self.grid_state.as_ref() {
            Some(s) => s,
            None => return 0,
        };

        if !self.search_query.is_empty() {
            return state.engine.search_query_count(&self.search_query);
        }

        if let Some(year) = self.active_year {
            if let Some((_, count)) = self.timeline_year_buckets.iter().find(|(y, _)| *y == year) {
                return *count;
            }
            return state.engine.search_query_count(&format!("year:{}", year));
        }

        match self.selection {
            AlbumSelection::AllLibrary => {
                if self.total_library_count > 0 {
                    self.total_library_count
                } else {
                    state.engine.total_count()
                }
            }
            AlbumSelection::System(album) => self
                .system_album_counts
                .get(&album)
                .copied()
                .unwrap_or_else(|| state.engine.query_system_album_count(album)),
            AlbumSelection::User(id) => {
                if let Some(&count) = self.user_album_counts.get(&id) {
                    count
                } else if let Some(album) = self.user_smart_albums.iter().find(|a| a.id == id) {
                    state.engine.query_smart_album_count(album)
                } else {
                    0
                }
            }
            AlbumSelection::Trash => self
                .system_album_counts
                .get(&SystemAlbum::RecentlyDeleted)
                .copied()
                .unwrap_or_else(|| {
                    state
                        .engine
                        .query_system_album_count(SystemAlbum::RecentlyDeleted)
                }),
            AlbumSelection::Tag(ref tag) => {
                if let Some((_, count)) = self.library_tag_counts.iter().find(|(t, _)| t == tag) {
                    return *count;
                }
                let query = if tag.contains(' ') {
                    format!("tag:\"{}\"", tag)
                } else {
                    format!("tag:{}", tag)
                };
                state.engine.search_query_count(&query)
            }
            // Cached by refresh_album_counts; querying per frame would run SQL
            // on every render even when the count is stable.
            AlbumSelection::Untagged => self.untagged_count,
        }
    }

    pub fn get_active_slice(&self, range: std::ops::Range<usize>) -> Vec<MediaItem> {
        let state = match self.grid_state.as_ref() {
            Some(s) => s,
            None => return Vec::new(),
        };

        let offset = range.start;
        let limit = range.end.saturating_sub(range.start);
        if limit == 0 {
            return Vec::new();
        }

        if !self.search_query.is_empty() {
            return state
                .engine
                .search_query_sorted(&self.search_query, limit, offset, self.sort);
        }

        if let Some(year) = self.active_year {
            return state.engine.search_query_sorted(
                &format!("year:{}", year),
                limit,
                offset,
                self.sort,
            );
        }

        match self.selection {
            AlbumSelection::AllLibrary => {
                state.engine.get_item_slice_sorted(offset, limit, self.sort)
            }
            AlbumSelection::System(album) => state
                .engine
                .query_system_album_sorted(album, limit, offset, self.sort),
            AlbumSelection::User(id) => {
                if let Some(album) = self.user_smart_albums.iter().find(|a| a.id == id) {
                    state
                        .engine
                        .query_smart_album_sorted(album, limit, offset, self.sort)
                } else {
                    Vec::new()
                }
            }
            AlbumSelection::Trash => state.engine.query_system_album_sorted(
                SystemAlbum::RecentlyDeleted,
                limit,
                offset,
                self.sort,
            ),
            AlbumSelection::Tag(ref tag) => {
                let query = if tag.contains(' ') {
                    format!("tag:\"{}\"", tag)
                } else {
                    format!("tag:{}", tag)
                };
                state
                    .engine
                    .search_query_sorted(&query, limit, offset, self.sort)
            }
            AlbumSelection::Untagged => {
                state.engine.query_untagged_sorted(limit, offset, self.sort)
            }
        }
    }

    pub fn select_album(&mut self, selection: AlbumSelection, cx: &mut Context<Self>) {
        let departing = self.navigation_snapshot();
        self.selection = selection;
        self.search_query.clear();
        self.search_focused = false;
        self.active_year = None;
        self.clear_selection(cx);
        self.record_navigation_change(departing);
    }

    pub fn toggle_shortcuts_overlay(&mut self, cx: &mut Context<Self>) {
        self.shortcuts_overlay_open = !self.shortcuts_overlay_open;
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

    /// Keeps XMP sidecars current after metadata edits. Failures are logged but
    /// do not roll back the SQLite write.
    pub fn sync_metadata_sidecars(&self, ids: &[lumina_core::MediaId]) {
        if ids.is_empty() {
            return;
        }
        if let Some(state) = self.grid_state.as_ref() {
            if let Err(err) = state.engine.write_metadata_sidecars(ids) {
                log::warn!("could not update XMP sidecars: {err:?}");
            }
        }
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
    ///
    /// Reads only cached recent searches and cached tag counts, never SQL.
    pub(crate) fn search_suggestion_items(&self) -> Vec<String> {
        search_suggestion_items(
            &self.config.recent_searches,
            &self.library_tag_counts,
            &self.search_query,
        )
    }

    /// Reopens the dropdown and clears the keyboard highlight.
    pub(crate) fn reset_search_suggestions(&mut self) {
        self.search_suggestions.dismissed = false;
        self.search_suggestions.selected = None;
    }

    /// Hides the dropdown without leaving the search field.
    pub(crate) fn dismiss_search_suggestions(&mut self) {
        self.search_suggestions.dismissed = true;
        self.search_suggestions.selected = None;
    }

    /// Moves the suggestion highlight without editing the query.
    pub(crate) fn move_search_suggestion(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.search_suggestions.dismissed {
            return;
        }
        let count = self.search_suggestion_items().len();
        if count == 0 {
            return;
        }
        let next = match self.search_suggestions.selected {
            None => {
                if delta > 0 {
                    0
                } else {
                    count - 1
                }
            }
            Some(index) => (index as isize + delta).clamp(0, count as isize - 1) as usize,
        };
        self.search_suggestions.selected = Some(next);
        cx.notify();
    }

    /// Applies the suggestion at `index`, keeping the field focused so the user
    /// can keep refining the query.
    pub(crate) fn apply_search_suggestion_at(&mut self, index: usize, cx: &mut Context<Self>) {
        let items = self.search_suggestion_items();
        let Some(suggestion) = items.get(index).cloned() else {
            return;
        };
        let departing = self.navigation_snapshot();
        self.search_query = apply_search_suggestion_text(&self.search_query, &suggestion);
        self.active_year = None;
        self.clear_selection(cx);
        self.record_navigation_change(departing);
        self.dismiss_search_suggestions();
        cx.notify();
    }

    /// Records the current query as a committed recent search when non-empty.
    pub(crate) fn record_current_search(&mut self) {
        let query = self.search_query.clone();
        if record_recent_search_entry(&mut self.config.recent_searches, &query) {
            self.save_config();
        }
    }

    /// Commits the current query: records history and unfocuses the field.
    pub(crate) fn commit_search(&mut self, cx: &mut Context<Self>) {
        self.record_current_search();
        self.search_focused = false;
        self.search_suggestions.dismissed = false;
        self.search_suggestions.selected = None;
        cx.notify();
    }

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
        self.active_year = None;
        self.clear_selection(cx);
        self.record_navigation_change(departing);
    }

    pub fn set_active_year(&mut self, year: Option<i32>, cx: &mut Context<Self>) {
        let departing = self.navigation_snapshot();
        self.active_year = year;
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
    fn apply_restored_session(&mut self, cx: &mut Context<Self>) {
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

        self.selection = resolve_restored_view(restored.view, &self.user_smart_albums);
        self.search_query = restored.search_query;
        self.search_focused = false;
        self.active_year = restored.active_year;

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
                    self.reveal_grid_selection();
                }
            }
        }
        // The startup restore is a jump, not user navigation: seed the trail
        // with the restored destination so Back cannot return to pre-restore state.
        self.reset_navigation_history();
        // Persist the applied session once so a startup window-bounds save cannot
        // leave an empty session on disk before the initial scan finishes.
        self.save_config();
        cx.notify();
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
            self.library_tag_counts = engine.query_tag_counts();
            self.untagged_count = engine.query_untagged_count();
            self.item_ratings = engine.get_ratings_map();
            self.item_color_labels = engine.get_color_labels_map();
            self.item_captions = engine.get_captions_map();
            let buckets = engine.get_timeline_buckets("year");
            self.timeline_year_buckets = buckets
                .into_iter()
                .filter_map(|b| {
                    let yr = b.bucket_key.parse::<i32>().ok()?;
                    Some((yr, b.item_count))
                })
                .collect();
        }

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
            resolve_restored_view(LastView::User(7), &[]),
            AlbumSelection::AllLibrary,
            "a removed smart album must not restore a phantom view"
        );
        assert_eq!(
            resolve_restored_view(LastView::User(7), &[user_album(7)]),
            AlbumSelection::User(7),
            "an existing smart album restores its view"
        );
    }

    #[core::prelude::v1::test]
    fn tag_view_restores_without_a_user_album() {
        assert_eq!(
            resolve_restored_view(LastView::Tag("travel".to_string()), &[]),
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
        let mut cx = gpui::TestAppContext::single();
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
            app.pending_restore = Some(RestoredSession {
                view: LastView::Favorites,
                search_query: String::new(),
                active_year: None,
                selected_media_id: Some(u64::MAX),
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
mod search_suggestion_tests {
    use super::*;

    #[core::prelude::v1::test]
    fn recent_searches_trim_dedupe_and_bound() {
        let mut entries = Vec::new();
        for index in 0..12 {
            assert!(record_recent_search_entry(
                &mut entries,
                &format!("query {index}")
            ));
        }
        assert_eq!(
            entries.len(),
            MAX_RECENT_SEARCHES,
            "recent searches must stay bounded"
        );
        assert_eq!(entries[0], "query 11", "the newest query must be first");

        assert!(record_recent_search_entry(&mut entries, "  QUERY 11  "));
        assert_eq!(
            entries.len(),
            MAX_RECENT_SEARCHES,
            "re-recording an existing query must not grow the history"
        );
        assert_eq!(
            entries[0], "QUERY 11",
            "the recorded query must be trimmed and keep its newest casing"
        );
        assert_eq!(
            entries
                .iter()
                .filter(|entry| entry.to_lowercase() == "query 11")
                .count(),
            1,
            "deduplication must be case-insensitive"
        );

        assert!(
            !record_recent_search_entry(&mut entries, "   "),
            "empty queries must not be recorded"
        );
    }

    #[core::prelude::v1::test]
    fn empty_query_shows_recent_searches_and_common_operators() {
        let recent = vec!["camera:sony".to_string()];
        let items = search_suggestion_items(&recent, &[], "");
        assert_eq!(
            items.first().map(String::as_str),
            Some("camera:sony"),
            "recent searches come first for an empty query"
        );
        assert!(
            items.iter().any(|item| item == "is:favorite"),
            "common operators must fill the rest of an empty-query list"
        );
        assert!(items.len() <= MAX_SEARCH_SUGGESTIONS);
    }

    #[core::prelude::v1::test]
    fn suggestions_include_matching_tags_and_filter_by_token() {
        let tags = vec![
            ("travel".to_string(), 4),
            ("trip 2024".to_string(), 2),
            ("family".to_string(), 1),
        ];
        let items = search_suggestion_items(&[], &tags, "tag:");
        assert!(items.contains(&"tag:travel".to_string()));
        assert!(items.contains(&"tag:\"trip 2024\"".to_string()));

        let filtered = search_suggestion_items(&[], &tags, "tag:fam");
        assert_eq!(
            filtered.first().map(String::as_str),
            Some("tag:family"),
            "matching tags must be filtered by the typed token"
        );

        let operators = search_suggestion_items(&[], &tags, "is:");
        assert!(
            operators.iter().all(|item| item.contains("is:")),
            "operator suggestions must respect the typed token: {operators:?}"
        );
        assert!(operators.contains(&"is:favorite".to_string()));
        assert!(!operators.contains(&"tag:travel".to_string()));
    }

    #[core::prelude::v1::test]
    fn applying_a_suggestion_replaces_only_the_current_token() {
        assert_eq!(apply_search_suggestion_text("", "is:video"), "is:video");
        assert_eq!(
            apply_search_suggestion_text("camera:sony", "is:video"),
            "is:video"
        );
        assert_eq!(
            apply_search_suggestion_text("camera:sony tag:tr", "tag:travel"),
            "camera:sony tag:travel"
        );
        assert_eq!(
            apply_search_suggestion_text("camera:sony ", "tag:travel"),
            "camera:sony tag:travel"
        );
    }

    #[core::prelude::v1::test]
    fn current_token_handles_trailing_space() {
        assert_eq!(current_search_token(""), "");
        assert_eq!(current_search_token("camera:sony tag:tr"), "tag:tr");
        assert_eq!(
            current_search_token("tag:tr "),
            "",
            "a trailing space starts a new token"
        );
    }
}
