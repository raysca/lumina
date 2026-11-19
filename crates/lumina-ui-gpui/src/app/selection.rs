//! Grid selection: clicks, keyboard moves, ranges and the selected item's tags.

use gpui::*;
use lumina_core::{MediaId, MediaItem};
use std::cmp;

use crate::app::batch_summary::{index_ranges, BATCH_SYNC_LIMIT};
use crate::app::state::BatchMetadataSummary;
use crate::app::types::BatchSelectionSummary;
use crate::app::LuminaApp;
use crate::theme;

impl LuminaApp {
    pub fn select_index(&mut self, index: usize, cx: &mut Context<Self>) {
        // Similar shots and the map show no selection, so they hold none.
        if self.similar_shots_shown() || self.map_shown() {
            return;
        }
        let total_count = self.navigation_count();
        if total_count == 0 {
            self.clear_selection(cx);
            return;
        }

        let clamped = cmp::min(index, total_count.saturating_sub(1));
        self.selected_indices.clear();
        self.selected_indices.insert(clamped);
        self.selection_anchor = Some(clamped);
        self.selected_index = Some(clamped);

        if let Some(item) = self.navigation_item(clamped) {
            self.select_item_internal(item, cx);
        }
        // Fetch ahead so the next steps (in the viewer, where the grid does not
        // render) read their items from the cache.
        self.want_view_rows(clamped.saturating_sub(16)..clamped + 16, cx);
        self.reveal_grid_selection();
        // Persist the new selection on a short timer, not on every arrow key.
        self.schedule_session_save(cx);
    }

    /// Index of the top visible grid row, from the scroll offset (0 with no
    /// library open).
    /// In Mosaic, the list's top layout row (headers included).
    pub(crate) fn grid_top_row(&self) -> usize {
        if self.mosaic_shown() {
            return self.mosaic_top_row();
        }
        let Some(state) = self.grid_state.as_ref() else {
            return 0;
        };
        let scroll_y = f32::from(state.scroll_handle.0.borrow().base_handle.offset().y);
        // Half a pixel of slack: a row scrolled exactly to the top can land a
        // float rounding short of its boundary.
        ((0.5 - scroll_y) / self.row_height()).floor().max(0.0) as usize
    }

    /// First item of the top visible grid row: the scroll anchor saved for
    /// relaunch. Reads only the cached rows, never SQL, so `None` when they
    /// are stale or no library is open.
    pub(crate) fn grid_top_item_id(&self) -> Option<MediaId> {
        self.grid_state.as_ref()?;
        let first = if self.mosaic_shown() {
            let layout = self.mosaic_layout()?;
            layout.rows.get(self.mosaic_top_row())?.first
        } else {
            self.grid_top_row() * self.grid_cols()
        };
        self.displayed_rows(first..first + 1)
            .first()
            .map(|item| item.id)
    }

    /// Keeps the keyboard-selected row visible in the virtualized grid.
    pub(crate) fn reveal_grid_selection(&self) {
        let (Some(state), Some(index)) = (self.grid_state.as_ref(), self.selected_index) else {
            return;
        };
        // Never the uniform list's handle in Mosaic: a queued scroll there
        // would jump the Grid when it comes back.
        if self.mosaic_shown() {
            self.reveal_mosaic_item(index);
            return;
        }
        let cols = self.grid_cols();
        state
            .scroll_handle
            .scroll_to_item(index / cols, gpui::ScrollStrategy::Center);
    }

    pub fn navigate_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        let total_count = self.navigation_count();
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
        let total_count = self.navigation_count();
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
        let total_count = self.navigation_count();
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

        if let Some(item) = self.navigation_item(target) {
            self.select_item_internal(item, cx);
        }
        self.refresh_selected_tags();
        self.update_batch_summary();
        self.reveal_grid_selection();
        self.schedule_session_save(cx);
        cx.notify();
    }

    /// Selects `item` at grid position `index`, keeping the position-based
    /// selection (index, anchor, set) in step with it.
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
        if self.similar_shots_shown() || self.map_shown() {
            self.clear_selection_fields();
            return;
        }
        // A draft belongs to the photo it was opened on: trash, a watcher
        // removal or any other step to another item discards it, so Done
        // can never write to a photo other than the one shown.
        if self
            .edits
            .session
            .as_ref()
            .is_some_and(|s| s.item != item.id)
        {
            self.discard_edit_draft();
        }
        self.selected_item = Some(item.clone());
        self.sync_missing_card();
        self.watch_missing_drive(cx);
        self.set_viewer_image(None);
        self.tag_input_active = false;
        self.tag_input_buffer.clear();
        self.caption.input_active = false;
        self.caption.input_buffer.clear();
        self.cancel_batch_edit();

        let cached = self.grid_state.as_ref().and_then(|state| {
            state
                .texture_cache
                .lock()
                .get(&item.id)
                .map(|t| t.image.clone())
        });
        if cached.is_some() {
            self.set_viewer_image(cached);
        }
        self.load_selected_item_tags(item.id, cx);
        self.viewer_failed = None;
        if let Some(state) = self.grid_state.as_ref().filter(|s| !s.is_failed(item.id)) {
            self.request_viewer_frame(state, item.id);
        }

        if self.is_fullscreen_viewer {
            self.prefetch_viewer_neighbours();
            self.sync_viewer_player(cx);
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
        // Change in the album panel: this click picks the cover.
        if self.cover_picking() {
            self.pick_cover(item.id, cx);
            return;
        }
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
                        // Deselecting the last card clears the selection but,
                        // as before, keeps an open edit draft: only fields
                        // with no remaining target are reset. The request bump
                        // stops a late tag load for the deselected card.
                        self.selected_index = None;
                        self.selected_item = None;
                        self.set_viewer_image(None);
                        self.selected_item_tags.clear();
                        self.selected_tag_counts.clear();
                        self.selected_tags_request = self.selected_tags_request.wrapping_add(1);
                        self.selection_anchor = None;
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
        // The duplicates review shows no grid: its hidden items are not
        // what ⌫ should be offered.
        if total == 0
            || self.similar_shots_shown()
            || self.map_shown()
            || self.duplicates_review_shown()
        {
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

    /// Selects exactly `range` (a Mosaic day), its first item as the
    /// selected item and anchor, and refreshes the batch summary. The item
    /// goes through `select_item_internal`, so open tag, caption and batch
    /// inputs and a draft for another photo are dropped.
    pub(crate) fn select_range(&mut self, range: std::ops::Range<usize>, cx: &mut Context<Self>) {
        if range.is_empty() || self.similar_shots_shown() || self.map_shown() {
            return;
        }
        self.selected_indices.clear();
        self.selected_indices.extend(range.clone());
        self.selected_index = Some(range.start);
        self.selection_anchor = Some(range.start);
        match self.navigation_item(range.start) {
            // Also refreshes the batch summary.
            Some(item) => self.select_item_internal(item, cx),
            None => self.update_batch_summary(),
        }
        self.refresh_selected_tags();
        self.schedule_session_save(cx);
        cx.notify();
    }

    pub fn clear_selection(&mut self, cx: &mut Context<Self>) {
        self.clear_selection_fields();
        // Clearing selection must forget the persisted item too (view/search
        // changes reach here after updating their own fields).
        self.schedule_session_save(cx);
        cx.notify();
    }

    /// Clears every selection-derived field without saving or rendering.
    ///
    /// Count refreshes run inside scan, undo, and tag-mutation paths that
    /// already notify their own caller, so they use this version to drop a
    /// selection that no longer belongs to the active view.
    pub(crate) fn clear_selection_fields(&mut self) {
        // No photo is shown any more, so a draft would have nothing to edit.
        self.discard_edit_draft();
        self.selected_indices.clear();
        self.selection_anchor = None;
        self.selected_index = None;
        self.selected_item = None;
        self.set_viewer_image(None);
        self.selected_item_tags.clear();
        self.selected_tag_counts.clear();
        // Invalidate any tag load still in flight for the item just cleared:
        // without this, its late result repopulates the cleared tag list.
        self.selected_tags_request = self.selected_tags_request.wrapping_add(1);
        self.batch_summary = BatchSelectionSummary::default();
        self.batch_metadata = BatchMetadataSummary::default();
        self.batch_load.invalidate();
        self.grid_drag.clear();
        self.cancel_batch_edit();
        self.tag_input_active = false;
        self.tag_input_buffer.clear();
    }

    /// Anything is selected. Never reads rows.
    pub(crate) fn has_selection(&self) -> bool {
        self.selected_item.is_some() || !self.selected_indices.is_empty()
    }

    pub fn get_selected_items(&self) -> Vec<MediaItem> {
        // One selected index is the selected item: no lookup (and no wait on
        // the catalog for every arrow press).
        let single = self.selected_indices.len() == 1
            && self.selected_indices.iter().next() == self.selected_index.as_ref();
        if self.selected_indices.is_empty() || single {
            if let Some(ref it) = self.selected_item {
                return vec![it.clone()];
            }
            return Vec::new();
        }

        let mut items = Vec::with_capacity(self.selected_indices.len());
        for range in index_ranges(&self.selected_indices) {
            let slice = self.navigation_rows(range);
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

    /// Loads the selected item's tags off the UI thread, so stepping through
    /// photos never waits on the catalog. A newer load or a synchronous
    /// [`Self::refresh_selected_tags`] wins over a late result.
    fn load_selected_item_tags(&mut self, id: lumina_core::MediaId, cx: &mut Context<Self>) {
        self.selected_tags_request = self.selected_tags_request.wrapping_add(1);
        let request = self.selected_tags_request;
        self.selected_item_tags.clear();
        let Some(engine) = self.grid_state.as_ref().map(|s| s.engine.clone()) else {
            return;
        };
        let task = cx
            .background_executor()
            .spawn(async move { engine.get_tags_for_item(id) });
        cx.spawn(async move |this, cx| {
            let tags = task.await;
            let _ = this.update(cx, |app, cx| {
                app.apply_selected_item_tags(request, tags, cx)
            });
        })
        .detach();
    }

    /// Lands a tag load unless a newer load or refresh has started since.
    pub(crate) fn apply_selected_item_tags(
        &mut self,
        request: u64,
        tags: Vec<String>,
        cx: &mut Context<Self>,
    ) {
        if self.selected_tags_request == request {
            self.selected_item_tags = tags;
            cx.notify();
        }
    }

    pub fn refresh_selected_tags(&mut self) {
        // A large selection's ids come from its summary (empty while it
        // loads; landing refreshes the tags), not a second read of the rows.
        let ids = if self.selected_indices.len() > BATCH_SYNC_LIMIT {
            self.batch_summary.ids.clone()
        } else {
            self.get_selected_ids()
        };
        self.refresh_tags_for(ids);
    }

    /// [`Self::refresh_selected_tags`] for the selection's `ids`, already
    /// known (no row reads).
    pub(crate) fn refresh_tags_for(&mut self, ids: Vec<MediaId>) {
        self.selected_tags_request = self.selected_tags_request.wrapping_add(1);
        let request = self.selected_tags_request;
        let Some(ref state) = self.grid_state else {
            self.selected_item_tags = Vec::new();
            self.selected_tag_counts = Vec::new();
            return;
        };
        if ids.is_empty() {
            self.selected_item_tags = Vec::new();
            self.selected_tag_counts = Vec::new();
            return;
        }

        #[cfg(test)]
        {
            let _ = request;
            if ids.len() == 1 {
                self.selected_item_tags = state.engine.get_tags_for_item(ids[0]);
                self.selected_tag_counts = Vec::new();
            } else {
                self.selected_tag_counts = state.engine.tag_counts_for_items(&ids);
                self.selected_item_tags = self
                    .selected_tag_counts
                    .iter()
                    .map(|(tag, _)| tag.clone())
                    .collect();
            }
        }

        #[cfg(not(test))]
        {
            let engine = state.engine.clone();
            let tx = self.ui_tx.clone();
            if let Err(err) = std::thread::Builder::new()
                .name("lumina-tag-refresh".to_string())
                .spawn(move || {
                    let (single_tags, counts) = if ids.len() == 1 {
                        (engine.get_tags_for_item(ids[0]), Vec::new())
                    } else {
                        let counts = engine.tag_counts_for_items(&ids);
                        let tags = counts.iter().map(|(t, _)| t.clone()).collect();
                        (tags, counts)
                    };
                    let _ = tx.send_blocking(crate::app::types::UiMessage::SelectedTagsLoaded {
                        request,
                        single_tags,
                        counts,
                    });
                })
            {
                log::warn!("failed to spawn lumina-tag-refresh thread: {err:#}");
            }
        }
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
