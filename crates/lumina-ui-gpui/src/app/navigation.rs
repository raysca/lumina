use gpui::*;
use lumina_core::{MediaId, MediaItem, MediaType};
use std::cmp;
use std::collections::VecDeque;

use crate::app::types::{BatchSelectionSummary, UiMessage};
use crate::app::undo::UndoAction;
use crate::app::LuminaApp;
use crate::theme;
use crate::views::sidebar::AlbumSelection;

/// Longest browsing trail kept for Back/Forward. Older destinations fall off
/// the front so the stack stays bounded no matter how long a session runs.
pub(crate) const NAVIGATION_HISTORY_LIMIT: usize = 50;

/// A restorable browsing destination.
///
/// Only durable navigation state belongs here: transient overlays (menus,
/// toasts, inline editors) must not reset the user's place when they go back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NavigationSnapshot {
    pub selection: AlbumSelection,
    pub search_query: String,
    pub active_year: Option<i32>,
    pub selected_media_id: Option<MediaId>,
    pub selected_index: Option<usize>,
    pub selection_anchor: Option<usize>,
    pub viewer_open: bool,
}

impl Default for NavigationSnapshot {
    fn default() -> Self {
        Self {
            selection: AlbumSelection::AllLibrary,
            search_query: String::new(),
            active_year: None,
            selected_media_id: None,
            selected_index: None,
            selection_anchor: None,
            viewer_open: false,
        }
    }
}

impl NavigationSnapshot {
    /// Whether two snapshots point at the same browsing destination.
    ///
    /// Selection is deliberately excluded: arrow keys move the selection
    /// without navigating and must not flood the trail with entries.
    fn same_destination(&self, other: &Self) -> bool {
        self.selection == other.selection
            && self.search_query == other.search_query
            && self.active_year == other.active_year
            && self.viewer_open == other.viewer_open
    }

    fn remember_selection(&mut self, other: &Self) {
        self.selected_media_id = other.selected_media_id;
        self.selected_index = other.selected_index;
        self.selection_anchor = other.selection_anchor;
    }
}

/// Bounded Back/Forward trail of browsing destinations.
#[derive(Debug)]
pub(crate) struct NavigationHistory {
    entries: VecDeque<NavigationSnapshot>,
    cursor: usize,
    restoring: bool,
}

impl NavigationHistory {
    pub(crate) fn new(initial: NavigationSnapshot) -> Self {
        let mut entries = VecDeque::with_capacity(NAVIGATION_HISTORY_LIMIT);
        entries.push_back(initial);
        Self {
            entries,
            cursor: 0,
            restoring: false,
        }
    }

    pub(crate) fn can_go_back(&self) -> bool {
        !self.restoring && self.cursor > 0
    }

    pub(crate) fn can_go_forward(&self) -> bool {
        !self.restoring && self.cursor + 1 < self.entries.len()
    }

    pub(crate) fn is_restoring(&self) -> bool {
        self.restoring
    }

    pub(crate) fn begin_restore(&mut self) {
        self.restoring = true;
    }

    pub(crate) fn end_restore(&mut self) {
        self.restoring = false;
    }

    /// True when the live state left the recorded cursor without a snapshot,
    /// for example a search typed straight into the field or a viewer opened
    /// from the grid.
    pub(crate) fn has_pending_destination(&self, snapshot: &NavigationSnapshot) -> bool {
        match self.entries.get(self.cursor) {
            Some(current) => !current.same_destination(snapshot),
            None => true,
        }
    }

    /// Materializes the live destination if the app moved without an explicit
    /// record call. Called before Back so the place being left stays reachable.
    pub(crate) fn sync_pending(&mut self, snapshot: NavigationSnapshot) {
        if self.restoring {
            return;
        }
        if !self.has_pending_destination(&snapshot) {
            if let Some(current) = self.entries.get_mut(self.cursor) {
                current.remember_selection(&snapshot);
            }
            return;
        }
        if self.entries.is_empty() {
            self.entries.push_back(snapshot);
            self.cursor = 0;
            return;
        }
        // Opening the viewer happens over the grid, so the state underneath is
        // the same destination with the viewer closed.
        if snapshot.viewer_open && !self.entries[self.cursor].viewer_open {
            let mut grid = snapshot.clone();
            grid.viewer_open = false;
            self.entries[self.cursor] = grid;
        }
        self.entries.truncate(self.cursor + 1);
        self.entries.push_back(snapshot);
        self.trim_front();
        self.cursor = self.entries.len().saturating_sub(1);
    }

    /// Appends a destination change. `departing` is the state before the
    /// change, so going Back restores the selection the user actually left.
    pub(crate) fn record(&mut self, departing: NavigationSnapshot, arriving: NavigationSnapshot) {
        if self.restoring || departing.same_destination(&arriving) {
            return;
        }
        if self.entries.is_empty() {
            self.entries.push_back(departing);
            self.cursor = 0;
        } else if self.entries[self.cursor].same_destination(&departing) {
            self.entries[self.cursor] = departing;
        } else {
            // The live state drifted before this explicit change; keep it as a
            // destination rather than silently dropping a visited place.
            self.entries.truncate(self.cursor + 1);
            self.entries.push_back(departing);
            self.trim_front();
            self.cursor = self.entries.len().saturating_sub(1);
        }
        self.entries.truncate(self.cursor + 1);
        self.entries.push_back(arriving);
        self.trim_front();
        self.cursor = self.entries.len().saturating_sub(1);
    }

    pub(crate) fn go_back(&mut self) -> Option<NavigationSnapshot> {
        if self.restoring || self.cursor == 0 || self.entries.is_empty() {
            return None;
        }
        self.cursor -= 1;
        self.entries.get(self.cursor).cloned()
    }

    pub(crate) fn go_forward(&mut self) -> Option<NavigationSnapshot> {
        if self.restoring || self.cursor + 1 >= self.entries.len() {
            return None;
        }
        self.cursor += 1;
        self.entries.get(self.cursor).cloned()
    }

    /// Keeps the record in sync when a saved album/tag/media had to fall back
    /// to the nearest valid view.
    pub(crate) fn replace_current(&mut self, snapshot: NavigationSnapshot) {
        if let Some(current) = self.entries.get_mut(self.cursor) {
            *current = snapshot;
        }
    }

    fn trim_front(&mut self) {
        while self.entries.len() > NAVIGATION_HISTORY_LIMIT {
            self.entries.pop_front();
        }
    }
}

impl LuminaApp {
    /// Snapshot of the durable browsing destination used by Back/Forward.
    pub(crate) fn navigation_snapshot(&self) -> NavigationSnapshot {
        NavigationSnapshot {
            selection: self.selection.clone(),
            search_query: self.search_query.clone(),
            active_year: self.active_year,
            selected_media_id: self.selected_item.as_ref().map(|item| item.id),
            selected_index: self.selected_index,
            selection_anchor: self.selection_anchor,
            viewer_open: self.is_fullscreen_viewer,
        }
    }

    /// Records a destination change that the caller already applied.
    pub(crate) fn record_navigation_change(&mut self, departing: NavigationSnapshot) {
        if self.navigation_history.is_restoring() {
            return;
        }
        let arriving = self.navigation_snapshot();
        self.navigation_history.record(departing, arriving);
    }

    /// Re-seeds the trail after a library switch or startup restore. Those are
    /// jumps, not user navigation, so they never create a Back entry.
    pub(crate) fn reset_navigation_history(&mut self) {
        self.navigation_history = NavigationHistory::new(self.navigation_snapshot());
    }

    pub fn can_navigate_back(&self) -> bool {
        let current = self.navigation_snapshot();
        self.navigation_history.can_go_back()
            || self.navigation_history.has_pending_destination(&current)
    }

    pub fn can_navigate_forward(&self) -> bool {
        let current = self.navigation_snapshot();
        self.navigation_history.can_go_forward()
            && !self.navigation_history.has_pending_destination(&current)
    }

    pub fn navigate_back(&mut self, cx: &mut Context<Self>) {
        self.navigation_history
            .sync_pending(self.navigation_snapshot());
        let Some(snapshot) = self.navigation_history.go_back() else {
            cx.notify();
            return;
        };
        self.apply_navigation_snapshot(snapshot, cx);
    }

    pub fn navigate_forward(&mut self, cx: &mut Context<Self>) {
        let current = self.navigation_snapshot();
        if self.navigation_history.has_pending_destination(&current) {
            // An unrecorded destination cuts the forward branch; moving forward
            // now would silently discard the place the user is looking at.
            cx.notify();
            return;
        }
        let Some(snapshot) = self.navigation_history.go_forward() else {
            cx.notify();
            return;
        };
        self.apply_navigation_snapshot(snapshot, cx);
    }

    fn apply_navigation_snapshot(&mut self, snapshot: NavigationSnapshot, cx: &mut Context<Self>) {
        self.navigation_history.begin_restore();
        self.restore_navigation_snapshot(snapshot, cx);
        self.navigation_history.end_restore();
        // A fallback (removed album/tag/media) may have changed the applied
        // destination; keep the trail pointing at what the user actually sees.
        self.navigation_history
            .replace_current(self.navigation_snapshot());
        cx.notify();
    }

    fn restore_navigation_snapshot(
        &mut self,
        snapshot: NavigationSnapshot,
        cx: &mut Context<Self>,
    ) {
        let viewer_was_open = self.is_fullscreen_viewer;
        if viewer_was_open && !snapshot.viewer_open {
            self.close_viewer(cx);
        }

        self.selection = self.resolve_navigation_selection(snapshot.selection);
        self.search_query = snapshot.search_query;
        self.search_focused = false;
        self.active_year = snapshot.active_year.filter(|year| {
            self.timeline_year_buckets.is_empty()
                || self
                    .timeline_year_buckets
                    .iter()
                    .any(|(bucket, _)| bucket == year)
        });
        self.clear_selection(cx);

        if let Some(media_id) = snapshot.selected_media_id {
            if let Some(index) = self.navigation_index_for(media_id, snapshot.selected_index) {
                let total = self.get_active_count();
                if let Some(item) = self.get_active_slice(index..index + 1).into_iter().next() {
                    self.selected_indices.insert(index);
                    self.selected_index = Some(index);
                    self.selection_anchor = snapshot
                        .selection_anchor
                        .filter(|anchor| *anchor < total)
                        .or(Some(index));
                    self.select_item_internal(item, cx);
                    self.refresh_selected_tags();
                    self.update_batch_summary();
                    self.reveal_grid_selection();
                }
            }
        }

        if snapshot.viewer_open {
            if self.selected_item.is_some() {
                self.is_fullscreen_viewer = true;
                self.viewer_state.reset();
                // `select_item_internal` already started video only when the
                // viewer was open while selecting; cover the closed -> open jump.
                if !viewer_was_open {
                    if let Some(item) = self.selected_item.clone() {
                        if item.media_type == MediaType::Video {
                            self.load_video_player(&item.path.clone(), cx);
                        } else {
                            self.stop_video_player();
                        }
                    }
                }
            } else {
                // The saved media item is gone: never open a viewer with no item.
                self.is_fullscreen_viewer = false;
            }
        }

        self.schedule_session_save(cx);
        cx.notify();
    }

    /// Falls back to All when a saved user album or tag no longer exists.
    fn resolve_navigation_selection(&self, selection: AlbumSelection) -> AlbumSelection {
        match selection {
            AlbumSelection::User(id)
                if !self.user_smart_albums.iter().any(|album| album.id == id) =>
            {
                AlbumSelection::AllLibrary
            }
            AlbumSelection::Tag(tag)
                if !self.library_tag_counts.iter().any(|(name, _)| name == &tag) =>
            {
                AlbumSelection::AllLibrary
            }
            other => other,
        }
    }

    /// Resolves a saved media position without per-frame SQL, reusing the
    /// restore-time chunk walk when the saved index no longer matches.
    fn navigation_index_for(&self, id: MediaId, saved_index: Option<usize>) -> Option<usize> {
        let total = self.get_active_count();
        if total == 0 {
            return None;
        }
        if let Some(index) = saved_index.filter(|index| *index < total) {
            if self
                .get_active_slice(index..index + 1)
                .into_iter()
                .next()
                .map(|item| item.id)
                == Some(id)
            {
                return Some(index);
            }
        }
        self.find_active_index(id)
    }
}

impl LuminaApp {
    pub fn set_rating_selected(&mut self, rating: u8, cx: &mut Context<Self>) {
        let ids = self.get_selected_ids();
        if ids.is_empty() {
            return;
        }
        let mut changed = 0usize;
        for id in &ids {
            let current = self.item_ratings.get(id).copied().unwrap_or(0);
            let new_r = if current == rating { 0 } else { rating };
            if current == new_r {
                continue;
            }
            self.set_item_rating(*id, new_r, cx);
            // A failed write reverts the optimistic map entry, so only count
            // changes that actually stuck before confirming them.
            if self.item_ratings.get(id).copied().unwrap_or(0) == new_r {
                changed += 1;
            }
        }
        if changed == 0 {
            cx.notify();
            return;
        }
        let cleared = ids
            .iter()
            .all(|id| self.item_ratings.get(id).copied().unwrap_or(0) == 0);
        self.show_undo_toast(
            if cleared {
                "Rating cleared".to_string()
            } else {
                format!("Rated {} item(s)", changed)
            },
            cx,
        );
    }

    pub fn set_color_label_selected(&mut self, label: u8, cx: &mut Context<Self>) {
        let ids = self.get_selected_ids();
        if ids.is_empty() {
            return;
        }
        let mut changed = 0usize;
        for id in &ids {
            let current = self.item_color_labels.get(id).copied().unwrap_or(0);
            let new_l = if current == label { 0 } else { label };
            if current == new_l {
                continue;
            }
            self.set_item_color_label(*id, new_l, cx);
            if self.item_color_labels.get(id).copied().unwrap_or(0) == new_l {
                changed += 1;
            }
        }
        if changed == 0 {
            cx.notify();
            return;
        }
        let cleared = ids
            .iter()
            .all(|id| self.item_color_labels.get(id).copied().unwrap_or(0) == 0);
        self.show_undo_toast(
            if cleared {
                "Color label removed".to_string()
            } else {
                format!("Color label updated for {} item(s)", changed)
            },
            cx,
        );
    }

    pub fn toggle_favorite_selected(&mut self, cx: &mut Context<Self>) {
        let Some(item) = self.selected_item.clone() else {
            return;
        };
        let new_fav = !item.is_favorite;
        let result = if let Some(ref state) = self.grid_state {
            state.engine.toggle_favorite(item.id, new_fav)
        } else {
            Ok(())
        };
        match result {
            Ok(()) => {
                if let Some(ref mut current) = self.selected_item {
                    current.is_favorite = new_fav;
                }
                self.push_undo(UndoAction::Favorite {
                    changes: vec![(item.id, item.is_favorite, new_fav)],
                });
                self.refresh_album_counts();
                self.show_undo_toast(
                    if new_fav {
                        "Added to favorites"
                    } else {
                        "Removed from favorites"
                    },
                    cx,
                );
            }
            Err(err) => self.set_error(format!("Could not update favorite: {err}"), cx),
        }
    }

    pub fn select_index(&mut self, index: usize, cx: &mut Context<Self>) {
        let total_count = self.get_active_count();
        if total_count == 0 {
            self.clear_selection(cx);
            return;
        }

        let clamped = cmp::min(index, total_count.saturating_sub(1));
        self.selected_indices.clear();
        self.selected_indices.insert(clamped);
        self.selection_anchor = Some(clamped);
        self.selected_index = Some(clamped);

        let slice = self.get_active_slice(clamped..clamped + 1);
        if let Some(item) = slice.into_iter().next() {
            self.select_item_internal(item, cx);
        }
        self.reveal_grid_selection();
        // Persist the new selection on a short timer, not on every arrow key.
        self.schedule_session_save(cx);
    }

    /// Keeps the keyboard-selected row visible in the virtualized grid.
    pub(crate) fn reveal_grid_selection(&self) {
        let (Some(state), Some(index)) = (self.grid_state.as_ref(), self.selected_index) else {
            return;
        };
        let cols = self.grid_columns.clamp(2, 8);
        state
            .scroll_handle
            .scroll_to_item(index / cols, gpui::ScrollStrategy::Center);
    }

    pub fn navigate_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        let total_count = self.get_active_count();
        if total_count == 0 {
            return;
        }

        self.viewer_state.reset();
        let current = self.selected_index.unwrap_or(0);
        let new_idx = if delta < 0 {
            current.saturating_sub((-delta) as usize)
        } else {
            cmp::min(current + delta as usize, total_count.saturating_sub(1))
        };
        self.select_index(new_idx, cx);
    }

    /// Extends the current selection by a keyboard delta while keeping the
    /// original anchor fixed. Shift+arrow should grow a range, not move it.
    pub fn extend_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        let total_count = self.get_active_count();
        if total_count == 0 {
            return;
        }
        let current = self.selected_index.unwrap_or(0);
        let target = if delta < 0 {
            current.saturating_sub((-delta) as usize)
        } else {
            cmp::min(current + delta as usize, total_count.saturating_sub(1))
        };
        self.extend_selection_to(target, cx);
    }

    /// Selects the contiguous range from the anchor to `index`, inclusive.
    pub fn extend_selection_to(&mut self, index: usize, cx: &mut Context<Self>) {
        let total_count = self.get_active_count();
        if total_count == 0 {
            return;
        }

        let target = cmp::min(index, total_count.saturating_sub(1));
        let anchor = self
            .selection_anchor
            .unwrap_or(self.selected_index.unwrap_or(target));
        if self.selection_anchor.is_none() {
            self.selection_anchor = Some(anchor);
        }

        let start = cmp::min(anchor, target);
        let end = cmp::max(anchor, target);
        self.selected_indices.clear();
        self.selected_indices.extend(start..=end);
        self.selected_index = Some(target);

        if let Some(item) = self.get_active_slice(target..target + 1).into_iter().next() {
            self.select_item_internal(item, cx);
        }
        self.refresh_selected_tags();
        self.update_batch_summary();
        self.reveal_grid_selection();
        self.schedule_session_save(cx);
        cx.notify();
    }

    pub fn select_item_at(&mut self, item: MediaItem, index: usize, cx: &mut Context<Self>) {
        self.selected_indices.clear();
        self.selected_indices.insert(index);
        self.selection_anchor = Some(index);
        self.selected_index = Some(index);
        self.select_item_internal(item, cx);
        self.schedule_session_save(cx);
    }

    pub fn select_item(&mut self, item: MediaItem, cx: &mut Context<Self>) {
        self.select_item_internal(item, cx);
        self.schedule_session_save(cx);
    }

    pub(crate) fn select_item_internal(&mut self, item: MediaItem, cx: &mut Context<Self>) {
        self.selected_item = Some(item.clone());
        self.viewer_image = None;
        self.tag_input_active = false;
        self.tag_input_buffer.clear();
        self.caption.input_active = false;
        self.caption.input_buffer.clear();

        if let Some(ref state) = self.grid_state {
            self.selected_item_tags = state.engine.get_tags_for_item(item.id);
            if let Some(cached) = state.texture_cache.lock().get(&item.id).cloned() {
                self.viewer_image = Some(cached);
            }

            let tx = self.ui_tx.clone();
            state
                .engine
                .request_thumbnail(item.id, 1024, move |frame_opt| {
                    if let Some(frame) = frame_opt {
                        let _ = tx.send_blocking(UiMessage::ViewerLoaded { id: item.id, frame });
                    }
                });
        }

        if self.is_fullscreen_viewer {
            if item.media_type == MediaType::Video {
                self.load_video_player(&item.path.clone(), cx);
            } else {
                self.stop_video_player();
            }
        }

        self.update_batch_summary();
        cx.notify();
    }

    pub fn is_index_selected(&self, index: usize) -> bool {
        self.selected_indices.contains(&index) || self.selected_index == Some(index)
    }

    pub fn handle_card_click(
        &mut self,
        item: MediaItem,
        index: usize,
        is_shift: bool,
        is_cmd: bool,
        cx: &mut Context<Self>,
    ) {
        if is_shift {
            let anchor = self
                .selection_anchor
                .unwrap_or(self.selected_index.unwrap_or(index));
            let start = cmp::min(anchor, index);
            let end = cmp::max(anchor, index);
            self.selected_indices.clear();
            for i in start..=end {
                self.selected_indices.insert(i);
            }
            self.selected_index = Some(index);
            self.select_item_internal(item, cx);
            self.refresh_selected_tags();
        } else if is_cmd {
            if self.selected_indices.contains(&index) {
                self.selected_indices.remove(&index);
                if self.selected_index == Some(index) {
                    if let Some(&first_idx) = self.selected_indices.iter().next() {
                        self.selected_index = Some(first_idx);
                        let slice = self.get_active_slice(first_idx..first_idx + 1);
                        if let Some(it) = slice.into_iter().next() {
                            self.select_item_internal(it, cx);
                        }
                    } else {
                        self.selected_index = None;
                        self.selected_item = None;
                        self.viewer_image = None;
                        self.selected_item_tags.clear();
                    }
                }
            } else {
                self.selected_indices.insert(index);
                self.selection_anchor = Some(index);
                self.selected_index = Some(index);
                self.select_item_internal(item, cx);
            }
            self.update_batch_summary();
            self.refresh_selected_tags();
        } else {
            // Normal click: select item and update selection metadata
            self.selected_indices.clear();
            self.selected_indices.insert(index);
            self.selection_anchor = Some(index);
            self.selected_index = Some(index);
            self.select_item_internal(item, cx);
            self.refresh_selected_tags();
        }
        self.schedule_session_save(cx);
        cx.notify();
    }

    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        let total = self.get_active_count();
        if total == 0 {
            return;
        }
        self.selected_indices.clear();
        self.selected_indices.extend(0..total);
        if self.selected_index.is_none() {
            self.selected_index = Some(0);
            let slice = self.get_active_slice(0..1);
            if let Some(it) = slice.into_iter().next() {
                self.selected_item = Some(it);
            }
        }
        self.update_batch_summary();
        self.refresh_selected_tags();
        self.schedule_session_save(cx);
        cx.notify();
    }

    pub fn clear_selection(&mut self, cx: &mut Context<Self>) {
        self.selected_indices.clear();
        self.selection_anchor = None;
        self.selected_index = None;
        self.selected_item = None;
        self.viewer_image = None;
        self.selected_item_tags.clear();
        self.batch_summary = BatchSelectionSummary::default();
        self.tag_input_active = false;
        self.tag_input_buffer.clear();
        // Clearing selection must forget the persisted item too (view/search
        // changes reach here after updating their own fields).
        self.schedule_session_save(cx);
        cx.notify();
    }

    pub fn update_batch_summary(&mut self) {
        if self.selected_indices.is_empty() {
            if let Some(ref it) = self.selected_item {
                self.batch_summary = BatchSelectionSummary {
                    count: 1,
                    total_bytes: it.file_size,
                    photos_count: if it.media_type == MediaType::Image {
                        1
                    } else {
                        0
                    },
                    videos_count: if it.media_type == MediaType::Video {
                        1
                    } else {
                        0
                    },
                    raw_count: if it.media_type == MediaType::Raw {
                        1
                    } else {
                        0
                    },
                    fav_count: if it.is_favorite { 1 } else { 0 },
                };
            } else {
                self.batch_summary = BatchSelectionSummary::default();
            }
            return;
        }

        let items = self.get_selected_items();
        let count = items.len();
        let total_bytes: u64 = items.iter().map(|i| i.file_size).sum();
        let photos_count = items
            .iter()
            .filter(|i| i.media_type == MediaType::Image)
            .count();
        let videos_count = items
            .iter()
            .filter(|i| i.media_type == MediaType::Video)
            .count();
        let raw_count = items
            .iter()
            .filter(|i| i.media_type == MediaType::Raw)
            .count();
        let fav_count = items.iter().filter(|i| i.is_favorite).count();

        self.batch_summary = BatchSelectionSummary {
            count,
            total_bytes,
            photos_count,
            videos_count,
            raw_count,
            fav_count,
        };
    }

    pub fn get_selected_items(&self) -> Vec<MediaItem> {
        if self.selected_indices.is_empty() {
            if let Some(ref it) = self.selected_item {
                return vec![it.clone()];
            }
            return Vec::new();
        }

        let mut items = Vec::with_capacity(self.selected_indices.len());
        let mut ranges: Vec<std::ops::Range<usize>> = Vec::new();
        let mut current_range: Option<std::ops::Range<usize>> = None;

        for &idx in &self.selected_indices {
            match current_range.take() {
                Some(r) => {
                    if idx == r.end {
                        current_range = Some(r.start..idx + 1);
                    } else {
                        ranges.push(r);
                        current_range = Some(idx..idx + 1);
                    }
                }
                None => {
                    current_range = Some(idx..idx + 1);
                }
            }
        }
        if let Some(r) = current_range {
            ranges.push(r);
        }

        for range in ranges {
            let slice = self.get_active_slice(range);
            items.extend(slice);
        }
        if items.is_empty() {
            if let Some(ref it) = self.selected_item {
                return vec![it.clone()];
            }
        }
        items
    }

    pub fn get_selected_ids(&self) -> Vec<MediaId> {
        if !self.selected_indices.is_empty() {
            let items = self.get_selected_items();
            if !items.is_empty() {
                return items.into_iter().map(|it| it.id).collect();
            }
        }
        self.selected_item
            .as_ref()
            .map(|it| vec![it.id])
            .unwrap_or_default()
    }

    pub fn refresh_selected_tags(&mut self) {
        if let Some(ref state) = self.grid_state {
            let ids = self.get_selected_ids();
            if ids.is_empty() {
                self.selected_item_tags = Vec::new();
            } else if ids.len() == 1 {
                self.selected_item_tags = state.engine.get_tags_for_item(ids[0]);
            } else {
                self.selected_item_tags = state.engine.get_tags_for_items_batch(&ids);
            }
        } else {
            self.selected_item_tags = Vec::new();
        }
    }

    pub fn batch_toggle_favorite(&mut self, cx: &mut Context<Self>) {
        let items = self.get_selected_items();
        if items.is_empty() {
            return;
        }
        let all_fav = items.iter().all(|i| i.is_favorite);
        let new_fav = !all_fav;
        let ids: Vec<MediaId> = items.iter().map(|i| i.id).collect();
        let changes: Vec<(MediaId, bool, bool)> = items
            .iter()
            .map(|item| (item.id, item.is_favorite, new_fav))
            .collect();

        let result = if let Some(ref state) = self.grid_state {
            state.engine.batch_set_favorite(&ids, new_fav)
        } else {
            Ok(())
        };
        match result {
            Ok(()) => {
                if let Some(ref mut it) = self.selected_item {
                    if ids.contains(&it.id) {
                        it.is_favorite = new_fav;
                    }
                }
                self.push_undo(UndoAction::Favorite { changes });
                self.refresh_album_counts();
                self.show_undo_toast(
                    if new_fav {
                        format!("Added {} item(s) to favorites", ids.len())
                    } else {
                        format!("Removed {} item(s) from favorites", ids.len())
                    },
                    cx,
                );
            }
            Err(err) => self.set_error(format!("Could not update favorites: {err}"), cx),
        }
    }

    pub fn set_item_rating(&mut self, id: MediaId, rating: u8, cx: &mut Context<Self>) {
        let before = self.item_ratings.get(&id).copied().unwrap_or(0);
        let clamped = rating.min(5);
        if before == clamped {
            cx.notify();
            return;
        }
        if clamped == 0 {
            self.item_ratings.remove(&id);
        } else {
            self.item_ratings.insert(id, clamped);
        }
        let result = if let Some(ref state) = self.grid_state {
            state.engine.set_rating(id, clamped)
        } else {
            Ok(())
        };
        if let Err(err) = result {
            if before == 0 {
                self.item_ratings.remove(&id);
            } else {
                self.item_ratings.insert(id, before);
            }
            self.set_error(format!("Could not save rating: {err}"), cx);
            return;
        }
        self.push_undo(UndoAction::Rating {
            id,
            before,
            after: clamped,
        });
        self.sync_metadata_sidecars(&[id]);
        if self.user_smart_albums.iter().any(|a| {
            a.rules
                .iter()
                .any(|r| r.field == lumina_core::RuleField::Rating)
        }) {
            self.refresh_album_counts();
        }
        cx.notify();
    }

    pub fn set_item_color_label(&mut self, id: MediaId, label: u8, cx: &mut Context<Self>) {
        let before = self.item_color_labels.get(&id).copied().unwrap_or(0);
        let clamped = label.min(6);
        if before == clamped {
            cx.notify();
            return;
        }
        if clamped == 0 {
            self.item_color_labels.remove(&id);
        } else {
            self.item_color_labels.insert(id, clamped);
        }
        let result = if let Some(ref state) = self.grid_state {
            state.engine.set_color_label(id, clamped)
        } else {
            Ok(())
        };
        if let Err(err) = result {
            if before == 0 {
                self.item_color_labels.remove(&id);
            } else {
                self.item_color_labels.insert(id, before);
            }
            self.set_error(format!("Could not save color label: {err}"), cx);
            return;
        }
        self.push_undo(UndoAction::ColorLabel {
            id,
            before,
            after: clamped,
        });
        if self.user_smart_albums.iter().any(|a| {
            a.rules
                .iter()
                .any(|r| r.field == lumina_core::RuleField::ColorLabel)
        }) {
            self.refresh_album_counts();
        }
        cx.notify();
    }

    pub fn get_item_rating(&self, item: &MediaItem) -> u8 {
        self.item_ratings.get(&item.id).copied().unwrap_or(0)
    }

    pub fn get_item_color_label(&self, item: &MediaItem) -> Option<gpui::Rgba> {
        self.item_color_labels
            .get(&item.id)
            .and_then(|&label| match label {
                1 => Some(theme::label_red()),
                2 => Some(theme::label_orange()),
                3 => Some(theme::label_yellow()),
                4 => Some(theme::label_green()),
                5 => Some(theme::label_blue()),
                6 => Some(theme::label_purple()),
                _ => None,
            })
    }
}
