use gpui::*;
use lumina_core::{MediaId, MediaItem, MediaType};
use std::cmp;
use std::collections::BTreeSet;

use crate::app::history::{NavigationHistory, NavigationSnapshot};
use crate::app::panels::{parse_edited_timestamp, parse_location_coordinates, DATE_INPUT_ERROR};
use crate::app::state::{BatchFieldSummary, BatchMetadataSummary, GRID_DRAG_THRESHOLD};
use crate::app::types::BatchSelectionSummary;
use crate::app::undo::{BatchField, BatchLocationChange, UndoAction};
use crate::app::LuminaApp;
use crate::theme;
use crate::views::sidebar::AlbumSelection;
use lumina_core::Coordinates;

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

    pub fn navigate_back(&mut self, cx: &mut Context<Self>) {
        self.navigation_history
            .sync_pending(self.navigation_snapshot());
        let Some(snapshot) = self.navigation_history.go_back() else {
            cx.notify();
            return;
        };
        self.apply_navigation_snapshot(snapshot, cx);
    }

    /// Redoes a Back. Syncing first means a destination reached since the
    /// Back (a typed search, say) drops the Forward entries, like a browser.
    pub fn navigate_forward(&mut self, cx: &mut Context<Self>) {
        self.navigation_history
            .sync_pending(self.navigation_snapshot());
        let Some(snapshot) = self.navigation_history.go_forward() else {
            cx.notify();
            return;
        };
        self.apply_navigation_snapshot(snapshot, cx);
    }

    /// Back would move: a recorded entry precedes the cursor, or the live
    /// state drifted (an unsynced typed search can always go Back).
    pub(crate) fn can_navigate_back(&self) -> bool {
        self.navigation_history.can_go_back()
            || self
                .navigation_history
                .has_pending_destination(&self.navigation_snapshot())
    }

    /// Forward would move: an entry is ahead and no unsynced destination
    /// would truncate it first.
    pub(crate) fn can_navigate_forward(&self) -> bool {
        self.navigation_history.can_go_forward()
            && !self
                .navigation_history
                .has_pending_destination(&self.navigation_snapshot())
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

        // Back/Forward lands on Exact copies with a fresh Similar shots read
        // to come: the old list may hold items trashed meanwhile.
        self.stacks.reset();
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
                    self.sync_viewer_player(cx);
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
            AlbumSelection::Album(id) if self.album_title(id).is_none() => {
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
    pub(crate) fn navigation_index_for(
        &self,
        id: MediaId,
        saved_index: Option<usize>,
    ) -> Option<usize> {
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
    /// Gives each selected item `label`, or clears it on items that already
    /// have it: one write, one undo entry and one sidecar pass per batch.
    pub fn set_color_label_selected(&mut self, label: u8, cx: &mut Context<Self>) {
        let ids = self.get_selected_ids();
        if ids.is_empty() {
            return;
        }
        let label = label.min(6);
        let changes: Vec<(MediaId, u8, u8)> = ids
            .iter()
            .map(|id| {
                let current = self.item_color_labels.get(id).copied().unwrap_or(0);
                (*id, current, if current == label { 0 } else { label })
            })
            .filter(|(_, before, after)| before != after)
            .collect();
        if changes.is_empty() {
            cx.notify();
            return;
        }
        let writes: Vec<(MediaId, u8)> = changes.iter().map(|&(id, _, a)| (id, a)).collect();
        let held = self.hold_selection(&ids, writes.len());
        if let Some(state) = self.grid_state.as_ref() {
            if let Err(err) = state.engine.set_color_labels(&writes) {
                self.set_error(format!("Could not save color label: {err:#}"), cx);
                return;
            }
        }
        set_or_clear(&mut self.item_color_labels, &writes);
        self.reselect_held(held, cx);
        let changed: Vec<MediaId> = writes.iter().map(|&(id, _)| id).collect();
        self.push_undo(UndoAction::ColorLabel { changes });
        self.sync_metadata_sidecars(&changed, cx);
        self.refresh_counts_if_rules_use(lumina_core::RuleField::ColorLabel);
        let cleared = ids
            .iter()
            .all(|id| self.item_color_labels.get(id).copied().unwrap_or(0) == 0);
        self.show_undo_toast(
            if cleared {
                "Color label removed".to_string()
            } else {
                format!("Color label updated for {} item(s)", changed.len())
            },
            cx,
        );
    }

    /// Smart albums filtering on `field` change membership when it changes.
    /// Saved searches (`Query` rules) always refresh; pass `RuleField::Query`
    /// for an edit no other rule field covers, such as a caption.
    pub(crate) fn refresh_counts_if_rules_use(&mut self, field: lumina_core::RuleField) {
        if self
            .user_smart_albums
            .iter()
            // A saved search can filter on any field.
            .any(|a| {
                a.rules
                    .iter()
                    .any(|r| r.field == field || r.field == lumina_core::RuleField::Query)
            })
        {
            self.refresh_album_counts();
        }
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
                self.mark_pop.favorited(Some(item.id));
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
        // Similar shots shows no selection, so it can hold none.
        if self.similar_shots_shown() {
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
    pub(crate) fn grid_top_row(&self) -> usize {
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
        let first = self.grid_top_row() * self.grid_columns.clamp(2, 8);
        self.displayed_rows(first..first + 1)
            .first()
            .map(|item| item.id)
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
        if self.similar_shots_shown() {
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
        self.set_viewer_image(None);
        self.tag_input_active = false;
        self.tag_input_buffer.clear();
        self.caption.input_active = false;
        self.caption.input_buffer.clear();
        self.cancel_batch_edit();

        let cached = self
            .grid_state
            .as_ref()
            .and_then(|state| state.texture_cache.lock().get(&item.id).cloned());
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
                        self.set_viewer_image(None);
                        self.selected_item_tags.clear();
                        self.selected_tag_counts.clear();
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

    // ---- Drag selection -------------------------------------------------

    /// Starts a pointer drag-selection gesture at `position`.
    ///
    /// `anchor` is the card the press landed on; a background press passes
    /// `None` and the first card the pointer sweeps becomes the anchor.
    /// `additive` keeps the current selection as the base, so a drag beginning
    /// on an already-selected card cannot collapse a multi-selection.
    pub fn begin_grid_drag(
        &mut self,
        position: Point<Pixels>,
        anchor: Option<usize>,
        additive: bool,
    ) {
        self.grid_drag = crate::app::state::GridDragState {
            start: Some(position),
            current: Some(position),
            anchor,
            base: if additive {
                self.selected_indices.clone()
            } else {
                BTreeSet::new()
            },
            hovered: anchor,
            active: true,
            moved: false,
            suppress_click: false,
        };
    }

    /// Updates the live drag from a raw pointer move.
    ///
    /// Returns true once the gesture has moved past the click threshold. A move
    /// with no button held abandons a stale gesture: a mouse-up delivered
    /// outside the window never reaches the grid.
    pub fn grid_pointer_moved(
        &mut self,
        position: Point<Pixels>,
        pressed: Option<MouseButton>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.grid_drag.active {
            return false;
        }
        if pressed != Some(MouseButton::Left) {
            self.grid_drag.clear();
            cx.notify();
            return false;
        }
        self.update_grid_drag_position(position, cx)
    }

    fn update_grid_drag_position(
        &mut self,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.grid_drag.active {
            return false;
        }
        self.grid_drag.current = Some(position);
        if let Some(start) = self.grid_drag.start {
            if (start.x - position.x).abs() > px(GRID_DRAG_THRESHOLD)
                || (start.y - position.y).abs() > px(GRID_DRAG_THRESHOLD)
            {
                self.grid_drag.moved = true;
            }
        }
        if self.grid_drag.moved {
            cx.notify();
        }
        self.grid_drag.moved
    }

    /// Extends the drag selection to the card under the pointer.
    pub fn drag_over_card(
        &mut self,
        index: usize,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if !self.grid_drag.active {
            return;
        }
        if !self.update_grid_drag_position(position, cx) {
            return;
        }
        self.grid_drag.hovered = Some(index);
        if self.grid_drag.anchor.is_none() {
            self.grid_drag.anchor = Some(index);
        }
        self.apply_grid_drag_range(index, cx);
    }

    /// Replaces the selection with the drag base plus the grid rectangle
    /// spanned by the anchor and target cards, matching the marquee. The base
    /// preserves a multi-selection the drag started inside.
    fn apply_grid_drag_range(&mut self, target: usize, cx: &mut Context<Self>) {
        let total = self.get_active_count();
        if total == 0 {
            return;
        }
        let Some(anchor) = self.grid_drag.anchor.map(|anchor| anchor.min(total - 1)) else {
            return;
        };
        let target = target.min(total - 1);
        // Must match the column count `render_browser_grid` lays out.
        let cols = self.grid_columns.clamp(2, 8);
        let (rows, columns) = (
            cmp::min(anchor / cols, target / cols)..=cmp::max(anchor / cols, target / cols),
            cmp::min(anchor % cols, target % cols)..=cmp::max(anchor % cols, target % cols),
        );
        let mut next = self.grid_drag.base.clone();
        next.extend(
            rows.flat_map(|row| columns.clone().map(move |col| row * cols + col))
                .filter(|&index| index < total),
        );
        if next == self.selected_indices && self.selected_index == Some(target) {
            return;
        }
        self.selected_indices = next;
        self.selected_index = Some(target);
        self.update_batch_summary();
        self.schedule_session_save(cx);
        cx.notify();
    }

    /// Finishes the gesture on mouse-up.
    ///
    /// A still press on empty space clears the selection; a moved gesture
    /// finalizes the focused item and suppresses the click that follows.
    pub fn finish_grid_drag(&mut self, cx: &mut Context<Self>) {
        if !self.grid_drag.active {
            return;
        }
        let moved = self.grid_drag.moved;
        let anchor = self.grid_drag.anchor;
        if !moved {
            if anchor.is_none() {
                // Plain click on the grid background keeps its old meaning.
                self.clear_selection(cx);
            } else {
                self.grid_drag.clear();
                cx.notify();
            }
            return;
        }
        // Re-apply over the release target so a drag that ended over a gap still
        // includes the card it started on.
        if let Some(target) = self.grid_drag.hovered.or(anchor) {
            self.apply_grid_drag_range(target, cx);
        } else if self.grid_drag.base.is_empty() {
            self.clear_selection(cx);
            return;
        }
        if let Some(index) = self.selected_index {
            if let Some(item) = self.get_active_slice(index..index + 1).into_iter().next() {
                self.select_item_internal(item, cx);
            }
        }
        self.refresh_selected_tags();
        self.update_batch_summary();
        self.schedule_session_save(cx);
        self.grid_drag.clear();
        self.grid_drag.suppress_click = true;
        cx.notify();
    }

    /// True when the click that follows a drag must be ignored. Consumes the
    /// suppression so a later, genuine click is not swallowed.
    pub fn consume_grid_drag_click(&mut self) -> bool {
        let suppress = self.grid_drag.moved || self.grid_drag.suppress_click;
        if suppress {
            self.grid_drag.suppress_click = false;
        }
        suppress
    }

    // ---- Batch metadata -------------------------------------------------

    /// Cancels a batch inline edit without applying it.
    ///
    /// Single-item editing is untouched: `caption.batch.field` is only set
    /// while the batch inspector owns the shared inline buffers.
    pub(crate) fn cancel_batch_edit(&mut self) {
        if self.caption.batch.field.take().is_some() {
            self.caption.title_input_active = false;
            self.caption.title_input_buffer.clear();
            self.caption.input_active = false;
            self.caption.input_buffer.clear();
            self.caption.date_input_active = false;
            self.caption.date_input_buffer.clear();
            self.caption.location_input_active = false;
            self.caption.location_input_buffer.clear();
        }
        self.caption.batch.initial_buffer.clear();
    }

    /// Recomputes the mixed-value summary for the current batch selection.
    ///
    /// Inactive inline buffers mirror the summary so the shared inspector
    /// render path can display "Mixed" or the shared value without a second
    /// data channel through `render_inspector_body`.
    fn refresh_batch_metadata(&mut self, items: &[MediaItem]) {
        if self.selected_indices.len() <= 1 {
            self.cancel_batch_edit();
            self.batch_metadata = BatchMetadataSummary::default();
            return;
        }
        let title = summarize_batch_text(items.iter().map(|item| item.title.clone()));
        let captions: Vec<Option<String>> = items
            .iter()
            .map(|item| {
                self.item_captions
                    .get(&item.id)
                    .cloned()
                    .filter(|caption| !caption.is_empty())
            })
            .collect();
        let caption = summarize_batch_text(captions);
        let date = summarize_batch_timestamps(items.iter().map(|item| item.timestamp_epoch_ms));
        let location =
            summarize_batch_locations(items.iter().map(|item| (item.latitude, item.longitude)));
        self.batch_metadata = BatchMetadataSummary {
            title,
            caption,
            date,
            location,
        };
        if !self.caption.title_input_active {
            self.caption.title_input_buffer = batch_field_display(&self.batch_metadata.title);
        }
        if !self.caption.input_active {
            self.caption.input_buffer = batch_field_display(&self.batch_metadata.caption);
        }
        if !self.caption.date_input_active {
            self.caption.date_input_buffer = batch_field_display(&self.batch_metadata.date);
        }
        if !self.caption.location_input_active {
            self.caption.location_input_buffer = batch_field_display(&self.batch_metadata.location);
        }
    }

    fn refresh_selected_item_from_engine(&mut self) {
        if let Some(id) = self.selected_item.as_ref().map(|item| item.id) {
            if let Some(updated) = self
                .grid_state
                .as_ref()
                .and_then(|state| state.engine.get_item_by_id(id))
            {
                self.set_selected_from_catalog(updated);
            }
        }
    }

    /// Applies one title to every selected item as a single undo step.
    ///
    /// All-or-nothing: the catalog writes every title in one transaction.
    pub fn apply_batch_title(&mut self, value: Option<String>, cx: &mut Context<Self>) {
        let items = self.get_selected_items();
        if items.is_empty() {
            return;
        }
        let changes: Vec<(MediaId, Option<String>, Option<String>)> = items
            .iter()
            .map(|item| (item.id, item.title.clone(), value.clone()))
            .filter(|(_, before, after)| before != after)
            .collect();
        if changes.is_empty() {
            self.update_batch_summary();
            cx.notify();
            return;
        }

        let writes: Vec<(MediaId, Option<&str>)> = changes
            .iter()
            .map(|(id, _, after)| (*id, after.as_deref()))
            .collect();
        if let Some(Err(err)) = self
            .grid_state
            .as_ref()
            .map(|state| state.engine.set_titles(&writes))
        {
            self.update_batch_summary();
            self.set_error(
                format!(
                    "Could not apply title to {} item(s): {err:#}",
                    changes.len()
                ),
                cx,
            );
            return;
        }

        let ids: Vec<MediaId> = changes.iter().map(|(id, _, _)| *id).collect();
        self.refresh_selected_item_from_engine();
        self.push_undo(UndoAction::BatchMetadata {
            field: BatchField::Title,
            changes,
        });
        self.sync_metadata_sidecars(&ids, cx);
        self.update_batch_summary();
        let message = if value.is_some() {
            format!("Title applied to {} item(s)", ids.len())
        } else {
            format!("Title removed from {} item(s)", ids.len())
        };
        self.set_last_action(message.clone());
        self.show_undo_toast(message, cx);
    }

    /// Applies one caption to every selected item as a single undo step.
    ///
    /// All-or-nothing: the catalog writes every caption in one transaction,
    /// and the caption map only changes once it has.
    pub fn apply_batch_caption(&mut self, value: Option<String>, cx: &mut Context<Self>) {
        let items = self.get_selected_items();
        if items.is_empty() {
            return;
        }
        let changes: Vec<(MediaId, Option<String>, Option<String>)> = items
            .iter()
            .map(|item| {
                (
                    item.id,
                    self.item_captions
                        .get(&item.id)
                        .cloned()
                        .filter(|caption| !caption.is_empty()),
                    value.clone(),
                )
            })
            .filter(|(_, before, after)| before != after)
            .collect();
        if changes.is_empty() {
            self.update_batch_summary();
            cx.notify();
            return;
        }

        let writes: Vec<(MediaId, &str)> = changes
            .iter()
            .map(|(id, _, after)| (*id, after.as_deref().unwrap_or("")))
            .collect();
        if let Some(Err(err)) = self
            .grid_state
            .as_ref()
            .map(|state| state.engine.set_captions(&writes))
        {
            self.update_batch_summary();
            self.set_error(
                format!(
                    "Could not apply caption to {} item(s): {err:#}",
                    changes.len()
                ),
                cx,
            );
            return;
        }
        for (id, _, after) in &changes {
            match after {
                Some(caption) => {
                    self.item_captions.insert(*id, caption.clone());
                }
                None => {
                    self.item_captions.remove(id);
                }
            }
        }

        let ids: Vec<MediaId> = changes.iter().map(|(id, _, _)| *id).collect();
        self.push_undo(UndoAction::BatchMetadata {
            field: BatchField::Caption,
            changes,
        });
        self.sync_metadata_sidecars(&ids, cx);
        self.refresh_counts_if_rules_use(lumina_core::RuleField::Query);
        self.update_batch_summary();
        let message = if value.is_some() {
            format!("Caption applied to {} item(s)", ids.len())
        } else {
            format!("Caption removed from {} item(s)", ids.len())
        };
        self.set_last_action(message.clone());
        self.show_undo_toast(message, cx);
    }

    /// Applies one date to every selected item as a single undo step.
    ///
    /// Accepts the same syntax as the single-item editor. A date without a time
    /// keeps each item's existing time of day, mirroring the single-item path.
    /// A signed shift (`+2h`, `-1d`) moves each item by the same amount, so
    /// their spacing is kept.
    /// Items already at the target timestamp are left out of the undo entry;
    /// the catalog writes the rest in one transaction.
    pub fn apply_batch_date(&mut self, value: &str, cx: &mut Context<Self>) {
        let items = self.get_selected_items();
        if items.is_empty() {
            return;
        }
        let Some(first) = items.first() else {
            return;
        };
        if parse_edited_timestamp(value, first.timestamp_epoch_ms).is_none() {
            self.set_error(DATE_INPUT_ERROR, cx);
            self.update_batch_summary();
            cx.notify();
            return;
        }

        let mut changes: Vec<(MediaId, i64, i64)> = Vec::with_capacity(items.len());
        for item in &items {
            let Some(after) = parse_edited_timestamp(value, item.timestamp_epoch_ms) else {
                self.set_error(DATE_INPUT_ERROR, cx);
                self.update_batch_summary();
                cx.notify();
                return;
            };
            if after != item.timestamp_epoch_ms {
                changes.push((item.id, item.timestamp_epoch_ms, after));
            }
        }
        if changes.is_empty() {
            self.update_batch_summary();
            cx.notify();
            return;
        }

        let writes: Vec<(MediaId, i64)> = changes.iter().map(|&(id, _, a)| (id, a)).collect();
        if let Some(Err(err)) = self
            .grid_state
            .as_ref()
            .map(|state| state.engine.set_timestamps(&writes))
        {
            self.update_batch_summary();
            self.set_error(
                format!("Could not apply date to {} item(s): {err:#}", changes.len()),
                cx,
            );
            return;
        }

        let ids: Vec<MediaId> = changes.iter().map(|(id, _, _)| *id).collect();
        self.push_undo(UndoAction::BatchDate { changes });
        self.sync_metadata_sidecars(&ids, cx);
        self.refresh_selected_item_from_engine();
        self.refresh_album_counts();
        self.update_batch_summary();
        let message = format!("Date applied to {} item(s)", ids.len());
        self.set_last_action(message.clone());
        self.show_undo_toast(message, cx);
    }

    /// Applies one location to every selected item as a single undo step.
    ///
    /// The draft uses the single-item coordinate syntax (`latitude, longitude`;
    /// an empty draft clears the location). The catalog writes every item in
    /// one transaction.
    pub fn apply_batch_location(&mut self, value: &str, cx: &mut Context<Self>) {
        let items = self.get_selected_items();
        if items.is_empty() {
            return;
        }
        let (lat, lon) = match parse_location_coordinates(value) {
            Ok(coords) => coords,
            Err(message) => {
                self.set_error(message, cx);
                self.update_batch_summary();
                cx.notify();
                return;
            }
        };

        let changes: Vec<BatchLocationChange> = items
            .iter()
            .filter_map(|item| {
                let before = (item.latitude, item.longitude);
                let after = (lat, lon);
                (before != after).then_some((item.id, before, after))
            })
            .collect();
        if changes.is_empty() {
            self.update_batch_summary();
            cx.notify();
            return;
        }

        let writes: Vec<(MediaId, Coordinates)> =
            changes.iter().map(|&(id, _, a)| (id, a)).collect();
        if let Some(Err(err)) = self
            .grid_state
            .as_ref()
            .map(|state| state.engine.set_locations(&writes))
        {
            self.update_batch_summary();
            self.set_error(
                format!(
                    "Could not apply location to {} item(s): {err:#}",
                    changes.len()
                ),
                cx,
            );
            return;
        }

        let ids: Vec<MediaId> = changes.iter().map(|(id, _, _)| *id).collect();
        self.push_undo(UndoAction::BatchLocation { changes });
        self.sync_metadata_sidecars(&ids, cx);
        self.refresh_counts_if_rules_use(lumina_core::RuleField::Query);
        self.refresh_selected_item_from_engine();
        self.update_batch_summary();
        let message = if lat.is_some() {
            format!("Location applied to {} item(s)", ids.len())
        } else {
            format!("Location removed from {} item(s)", ids.len())
        };
        self.set_last_action(message.clone());
        self.show_undo_toast(message, cx);
    }

    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        let total = self.get_active_count();
        if total == 0 || self.similar_shots_shown() {
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
        self.batch_summary = BatchSelectionSummary::default();
        self.batch_metadata = BatchMetadataSummary::default();
        self.grid_drag.clear();
        self.cancel_batch_edit();
        self.tag_input_active = false;
        self.tag_input_buffer.clear();
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
                    ids: vec![it.id],
                    oldest_ms: it.timestamp_epoch_ms,
                    newest_ms: it.timestamp_epoch_ms,
                };
            } else {
                self.batch_summary = BatchSelectionSummary::default();
            }
            self.refresh_batch_metadata(&[]);
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

        self.batch_summary = BatchSelectionSummary {
            count,
            total_bytes,
            photos_count,
            videos_count,
            raw_count,
            ids: items.iter().map(|i| i.id).collect(),
            oldest_ms: items
                .iter()
                .map(|i| i.timestamp_epoch_ms)
                .min()
                .unwrap_or(0),
            newest_ms: items
                .iter()
                .map(|i| i.timestamp_epoch_ms)
                .max()
                .unwrap_or(0),
        };
        self.refresh_batch_metadata(&items);
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
        self.selected_tags_request = self.selected_tags_request.wrapping_add(1);
        let request = self.selected_tags_request;
        let Some(ref state) = self.grid_state else {
            self.selected_item_tags = Vec::new();
            self.selected_tag_counts = Vec::new();
            return;
        };
        let ids = self.get_selected_ids();
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
                log::warn!("failed to spawn lumina-tag-refresh thread: {err}");
            }
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

/// Summarizes one text field across the batch selection.
///
/// `mixed` is true when the items disagree; `common` then carries `None`
/// because there is no single value to show. An empty iterator (no selection)
/// yields the default "not set" summary.
fn summarize_batch_text<I>(values: I) -> BatchFieldSummary
where
    I: IntoIterator<Item = Option<String>>,
{
    let mut values = values.into_iter();
    let Some(first) = values.next() else {
        return BatchFieldSummary::default();
    };
    let mixed = values.any(|value| value != first);
    BatchFieldSummary {
        mixed,
        common: if mixed { None } else { first },
    }
}

/// Renders a summary for the shared inline buffers: "Mixed" when the selection
/// disagrees, otherwise the shared value (empty means "not set").
fn batch_field_display(field: &BatchFieldSummary) -> String {
    if field.mixed {
        "Mixed".to_string()
    } else {
        field.common.clone().unwrap_or_default()
    }
}

/// Summarizes capture timestamps across the batch selection.
///
/// A shared timestamp is rendered in the editor's own syntax so the batch row
/// shows exactly the value typing the same text would restore.
fn summarize_batch_timestamps<I>(values: I) -> BatchFieldSummary
where
    I: IntoIterator<Item = i64>,
{
    let mut values = values.into_iter();
    let Some(first) = values.next() else {
        return BatchFieldSummary::default();
    };
    let mixed = values.any(|value| value != first);
    BatchFieldSummary {
        mixed,
        common: if mixed {
            None
        } else {
            Some(format_batch_timestamp(first))
        },
    }
}

/// Summarizes coordinates across the batch selection using the single-item
/// editor's `latitude, longitude` form. Items with no coordinates are "not set".
fn summarize_batch_locations<I>(values: I) -> BatchFieldSummary
where
    I: IntoIterator<Item = (Option<f64>, Option<f64>)>,
{
    let mut values = values.into_iter();
    let Some(first) = values.next() else {
        return BatchFieldSummary::default();
    };
    let mixed = values.any(|value| value != first);
    let common = match first {
        (Some(lat), Some(lon)) => Some(format!("{lat:.6}, {lon:.6}")),
        _ => None,
    };
    BatchFieldSummary {
        mixed,
        common: if mixed { None } else { common },
    }
}

fn format_batch_timestamp(epoch_ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(epoch_ms)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_default()
}

/// Applies (id, value) writes to a 0-means-unset map.
pub(crate) fn set_or_clear(
    map: &mut std::collections::HashMap<MediaId, u8>,
    writes: &[(MediaId, u8)],
) {
    for &(id, value) in writes {
        if value == 0 {
            map.remove(&id);
        } else {
            map.insert(id, value);
        }
    }
}
