use gpui::*;
use lumina_core::{EngineEvent, LuminaEngine, MediaItem, SystemAlbum};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

use crate::app::types::UiMessage;
use crate::app::LuminaApp;
use crate::bridge::{frame_to_gpui_image, video_frame_to_gpui_image};
use crate::player::VideoPlayerBackend;
use crate::views::grid::GridState;
use crate::views::sidebar::AlbumSelection;

impl LuminaApp {
    pub fn handle_ui_message(&mut self, msg: UiMessage, cx: &mut Context<Self>) {
        match msg {
            UiMessage::Engine(evt) => {
                let mut should_refresh_counts = false;
                let mut should_clear_selection = false;
                let had_multi_selection = !self.selected_indices.is_empty();
                let selected_item_id = self.selected_item.as_ref().map(|item| item.id);

                if let Some(ref mut state) = self.grid_state {
                    match evt {
                        EngineEvent::BatchDiscovered { .. } => {
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
                        } => {
                            state.scan_progress = items_found;
                            if is_complete {
                                state.is_scanning = false;
                            }
                            cx.notify();
                        }
                        EngineEvent::ScanComplete { total_items } => {
                            state.total_count = total_items;
                            state.is_scanning = false;
                            should_refresh_counts = true;
                            // Indices may have shifted while scanning.
                            if had_multi_selection {
                                should_clear_selection = true;
                            }
                            cx.notify();
                        }
                        EngineEvent::ItemRemoved(id) => {
                            state.total_count = state.engine.total_count();
                            state.texture_cache.lock().pop(&id);
                            state.in_flight.lock().remove(&id);
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

                if should_clear_selection {
                    self.clear_selection(cx);
                }
                if should_refresh_counts {
                    self.refresh_album_counts();
                    cx.notify();
                }
            }
            UiMessage::ThumbnailLoaded { id, frame } => {
                if let Some(ref state) = self.grid_state {
                    state.in_flight.lock().remove(&id);
                    if let Ok(gpui_img) = frame_to_gpui_image(frame) {
                        state.texture_cache.lock().put(id, gpui_img);
                        cx.notify();
                    }
                }
            }
            UiMessage::ViewerLoaded { id, frame } => {
                if let Some(ref current) = self.selected_item {
                    if current.id == id {
                        if let Ok(gpui_img) = frame_to_gpui_image(frame) {
                            self.viewer_image = Some(gpui_img);
                            cx.notify();
                        }
                    }
                }
            }
            UiMessage::FolderPicked(folder) => {
                self.open_library(folder, cx);
            }
            UiMessage::VideoFrameTick => {
                self.video.frame_pending.store(false, Ordering::Release);
                if let Some(ref mut player) = self.video.player {
                    self.video.playback_time = player.position();
                    if !player.is_playing() {
                        self.video.is_playing = false;
                        self.stop_video_ticker();
                        cx.notify();
                    } else if let Some(frame) = player.next_frame() {
                        if let Ok(img) = video_frame_to_gpui_image(frame) {
                            self.viewer_image = Some(img);
                            cx.notify();
                        }
                    }
                }
            }
            UiMessage::FloatingInspectorClosed => {
                self.floating_inspector_closed(cx);
            }
            UiMessage::FloatingPipClosed => {
                self.floating_pip_closed(cx);
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
        if let Some(ref state) = self.grid_state {
            state.engine.cancel_scan();
        }

        self.config.last_library_path = Some(path.clone());
        let _ = self.config.save();

        let (engine, rx) = match LuminaEngine::open(path.clone()) {
            Ok(res) => res,
            Err(e) => {
                eprintln!("Failed to open library at {:?}: {:?}", path, e);
                self.set_error(format!("Could not open library: {e}"), cx);
                return;
            }
        };

        let grid_state = GridState::new(engine);
        self.grid_state = Some(grid_state);
        self.selected_item = None;
        self.selected_index = None;
        self.selected_indices.clear();
        self.selection_anchor = None;
        self.selected_item_tags.clear();
        self.batch_summary = Default::default();
        self.viewer_image = None;
        self.selection = AlbumSelection::AllLibrary;
        self.search_query.clear();
        self.search_focused = false;
        self.active_year = None;
        self.is_fullscreen_viewer = false;

        // Drop and join the previous event-receiver thread to prevent orphaned threads.
        if let Some(handle) = self.event_receiver_thread.take() {
            let _ = handle.join();
        }

        let tx = self.ui_tx.clone();
        self.event_receiver_thread = std::thread::Builder::new()
            .name("lumina-event-receiver".to_string())
            .spawn(move || {
                for event in rx {
                    let _ = tx.send_blocking(UiMessage::Engine(event));
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
            return state.engine.search_query(&self.search_query, limit, offset);
        }

        if let Some(year) = self.active_year {
            return state
                .engine
                .search_query(&format!("year:{}", year), limit, offset);
        }

        match self.selection {
            AlbumSelection::AllLibrary => state.engine.get_item_slice(range),
            AlbumSelection::System(album) => state.engine.query_system_album(album, limit, offset),
            AlbumSelection::User(id) => {
                if let Some(album) = self.user_smart_albums.iter().find(|a| a.id == id) {
                    state.engine.query_smart_album(album, limit, offset)
                } else {
                    Vec::new()
                }
            }
            AlbumSelection::Trash => {
                state
                    .engine
                    .query_system_album(SystemAlbum::RecentlyDeleted, limit, offset)
            }
            AlbumSelection::Tag(ref tag) => {
                let query = if tag.contains(' ') {
                    format!("tag:\"{}\"", tag)
                } else {
                    format!("tag:{}", tag)
                };
                state.engine.search_query(&query, limit, offset)
            }
        }
    }

    pub fn select_album(&mut self, selection: AlbumSelection, cx: &mut Context<Self>) {
        self.selection = selection;
        self.search_query.clear();
        self.search_focused = false;
        self.active_year = None;
        self.clear_selection(cx);
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

    /// Drops keyboard focus from the toolbar search field.
    pub fn blur_search(&mut self, cx: &mut Context<Self>) {
        if self.search_focused {
            self.search_focused = false;
            cx.notify();
        }
    }

    pub fn append_search_pill(&mut self, pill: &str, cx: &mut Context<Self>) {
        if self.search_query.is_empty() {
            self.search_query = pill.to_string();
        } else if !self.search_query.contains(pill) {
            self.search_query = format!("{} {}", self.search_query, pill);
        }
        self.active_year = None;
        self.clear_selection(cx);
    }

    pub fn clear_search(&mut self, cx: &mut Context<Self>) {
        self.search_query.clear();
        self.search_focused = false;
        self.active_year = None;
        self.clear_selection(cx);
    }

    pub fn set_search_query(&mut self, query: &str, cx: &mut Context<Self>) {
        self.search_query = query.to_string();
        self.active_year = None;
        self.clear_selection(cx);
    }

    pub fn set_active_year(&mut self, year: Option<i32>, cx: &mut Context<Self>) {
        self.active_year = year;
        self.search_query.clear();
        self.search_focused = false;
        self.clear_selection(cx);
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
    }
}
