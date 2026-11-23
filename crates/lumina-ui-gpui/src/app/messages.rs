//! Handling the messages background work sends to the UI thread.

use gpui::*;
use lumina_core::EngineEvent;
use std::sync::atomic::Ordering;
use std::time::Instant;

use crate::app::history::NavigationSnapshot;
use crate::app::import::ImportDialog;
use crate::app::types::UiMessage;
use crate::app::LuminaApp;
use crate::bridge::{frame_to_gpui_image, video_frame_to_gpui_image};
use crate::player::VideoPlayerBackend;

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
                        EngineEvent::ItemsRemoved(ids) => {
                            state.total_count = state.engine.total_count();
                            {
                                let mut textures = state.texture_cache.lock();
                                let mut in_flight = state.in_flight.lock();
                                for id in &ids {
                                    if let Some(old) = textures.pop(id) {
                                        removed_textures.push(old.image);
                                    }
                                    in_flight.remove(id);
                                }
                            }
                            for &id in &ids {
                                state.clear_failed(id);
                                // The media is gone; its resume point can never load.
                                state.video_resume.positions.remove(id);
                            }
                            should_refresh_counts = true;
                            // Removals shift every following index; clear rather than
                            // risk batch actions targeting different files.
                            if had_multi_selection
                                || selected_item_id.is_some_and(|selected| ids.contains(&selected))
                            {
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
                    self.refresh_album_counts_soon(cx);
                    // The item on screen may have just gone offline.
                    self.watch_missing_drive(cx);
                    cx.notify();
                }
                if should_restore_session {
                    self.apply_restored_session(user_moved, cx);
                }
                if scan_completed && std::mem::take(&mut self.import.pending_import_after_open) {
                    self.pick_import_folders(cx);
                }
                #[cfg(all(target_os = "linux", feature = "linux-ui-smoke"))]
                if scan_completed && std::env::var_os("LUMINA_UI_SMOKE").is_some() {
                    let count = self.get_active_count();
                    log::info!("LUMINA_UI_SMOKE: scan complete with {count} media items");
                    if let Some(item) = self.get_active_slice(0..1).into_iter().next() {
                        self.open_viewer(item, cx);
                    } else {
                        log::error!("LUMINA_UI_SMOKE: no media item was available to open");
                    }
                }
            }
            UiMessage::ThumbnailLoaded {
                id,
                frame,
                requested_size,
            } => {
                let Some(state) = self.grid_state.as_ref() else {
                    return;
                };
                state.in_flight.lock().remove(&id);
                match frame_to_gpui_image(frame) {
                    Ok(gpui_img) => {
                        state.clear_failed(id);
                        // `push` hands back the LRU entry it evicts (or the entry
                        // it replaces); its texture must be freed too.
                        let evicted = state.texture_cache.lock().push(
                            id,
                            crate::views::grid::CachedThumb::new(requested_size, gpui_img),
                        );
                        state.mark_fresh_thumb(id);
                        if let Some((_, old)) = evicted {
                            self.retire_image(old.image);
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
            // An answer for an earlier request is dropped: the user moved
            // on, or a newer decode of the same item is coming.
            UiMessage::ViewerLoaded { generation, .. }
            | UiMessage::ViewerFailed { generation, .. }
                if !self.viewer_frame_is_current(generation) => {}
            UiMessage::ViewerLoaded { id, frame, .. } => {
                #[cfg(test)]
                crate::app::state::VIEWER_ANSWERS.with(|n| n.set(n.get() + 1));
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
                                #[cfg(all(target_os = "linux", feature = "linux-ui-smoke"))]
                                if std::env::var_os("LUMINA_UI_SMOKE").is_some()
                                    && self.is_fullscreen_viewer
                                {
                                    log::info!("LUMINA_UI_SMOKE: viewer frame loaded for {id:?}");
                                }
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
            UiMessage::ViewerFailed { id, .. } => {
                #[cfg(test)]
                crate::app::state::VIEWER_ANSWERS.with(|n| n.set(n.get() + 1));
                if self
                    .selected_item
                    .as_ref()
                    .is_some_and(|item| item.id == id)
                {
                    self.viewer_failed = Some(id);
                    #[cfg(all(target_os = "linux", feature = "linux-ui-smoke"))]
                    if std::env::var_os("LUMINA_UI_SMOKE").is_some() && self.is_fullscreen_viewer {
                        log::error!("LUMINA_UI_SMOKE: viewer frame failed for {id:?}");
                    }
                    cx.notify();
                }
            }
            UiMessage::FolderPicked(folder) => {
                self.open_library(folder, cx);
            }
            UiMessage::NewLibraryPicked(folder) => {
                let ticket = self.welcome.retry_generation;
                self.open_library(folder, cx);
                // Only this open, once it succeeds, gets the import (not a
                // failed open, not one waiting on the downloads prompt).
                self.import.import_after_open = (self.welcome.retry_generation != ticket)
                    .then_some(self.welcome.retry_generation);
            }
            UiMessage::ExportFolderPicked(folder, preset) => {
                self.export_to_folder(folder, preset, cx);
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
                let mut surface = None;
                let mut playback_ended = false;
                let mut playback_error = None;
                let mut stop_ticker = false;

                if let Some(ref mut player) = self.video.player {
                    if playing {
                        self.video.playback_time = player.position();
                        if player.is_playing() {
                            frame = player.next_frame();
                            surface = player.take_surface();
                        } else if let Some(error) = player.take_playback_error() {
                            playback_error = Some(error);
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
                                surface = player.take_surface();
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

                // Decoding failed mid-clip: show it as failed, like a load.
                if let Some(error) = playback_error {
                    self.record_video_failure(error);
                    cx.notify();
                    return;
                }

                if playing && self.enforce_loop_range(playback_ended) {
                    playback_ended = false;
                    frame = None;
                    surface = None;
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
                } else if frame.is_some() || surface.is_some() {
                    if let Some(surface) = surface {
                        self.video.surface = Some(surface);
                    }
                    if let Some(img) = frame.and_then(|frame| video_frame_to_gpui_image(frame).ok())
                    {
                        self.show_video_frame(img);
                    }
                    if !playing {
                        // One frame is all the paused refresh needs.
                        self.video.seek_refresh_until = None;
                        self.stop_video_ticker();
                    }
                    self.repaint_for_video_frame(cx);
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
            UiMessage::BatchSummaryLoaded(loaded) => {
                if self.apply_batch_summary(*loaded) {
                    cx.notify();
                }
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
                    self.set_duplicate_groups(groups);
                    self.duplicate_items_count = items_count;
                    cx.notify();
                }
            }
            UiMessage::OpenFloatingInspector | UiMessage::OpenFloatingPip => {}
        }
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
