use gpui::*;
use lumina_core::text::{clean_caption, clean_text, MAX_TEXT_CHARS};
use lumina_core::MediaItem;

use crate::app::types::UiMessage;
use crate::app::LuminaApp;
use crate::views::context_menu::ContextMenuState;
use crate::views::grid::{GridZoomAnchor, GridZoomGeometry};

/// Wheel delta below which a Cmd/Ctrl + wheel event is treated as noise.
const GRID_ZOOM_WHEEL_EPSILON: f32 = 0.01;
/// Pixel (trackpad) ⌘-scroll per grid density level.
const GRID_WHEEL_STEP_PX: f32 = 40.0;
/// Pinch magnification per grid density level (0.12 = 12%).
const GRID_PINCH_STEP: f32 = 0.12;

/// Adds `delta` to `accum` and spends whole `step`s of it: the signed number
/// of ladder levels to move, leaving the remainder for the next event.
fn take_steps(accum: &mut f32, delta: f32, step: f32) -> isize {
    *accum += delta;
    let steps = (*accum / step).trunc();
    *accum -= steps * step;
    steps as isize
}

impl LuminaApp {
    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        // A failed save is in the banner; put the sidebar back so the
        // screen matches the saved setting.
        if !self.save_config() {
            self.sidebar_collapsed = !self.sidebar_collapsed;
        }
        // The View menu names what the item will do next.
        cx.set_menus(crate::menus::build_menus(self.sidebar_collapsed));
        cx.notify();
    }

    pub fn toggle_inspector(&mut self, cx: &mut Context<Self>) {
        // The Albums overview and the Tags page hide the panel: a toggle
        // there would flip and save a setting the user cannot see. So does
        // Review Duplicates.
        if self.non_media_page_shown() || self.duplicates_review_shown() {
            return;
        }
        if self.floating_inspector_window.is_some() {
            // If already floating, focus the floating window
            self.inspector_open = false;
            self.save_config();
            cx.notify();
            return;
        }
        self.inspector_open = !self.inspector_open;
        self.tag_input_active = false;
        self.tag_input_buffer.clear();
        self.save_config();
        cx.notify();
    }

    pub fn undock_inspector(&mut self, cx: &mut Context<Self>) {
        self.inspector_open = false;
        self.tag_input_active = false;
        self.tag_input_buffer.clear();
        self.save_config();

        if self.floating_inspector_window.is_some() {
            cx.notify();
            return;
        }

        // Defer window creation to next tick of GPUI event loop
        let _ = self.ui_tx.send_blocking(UiMessage::OpenFloatingInspector);
        cx.notify();
    }

    /// ⌘I: the Get Info window on its Info tab. Like Finder, nothing
    /// happens with nothing selected.
    pub fn get_info(&mut self, cx: &mut Context<Self>) {
        if self.selected_item.is_some() {
            self.open_get_info_at(crate::views::inspector::InspectorTab::Info, cx);
        }
    }

    /// Opens the Get Info window on `tab` (⌘I, the context menu and the
    /// palette's Get Info actions).
    pub fn open_get_info_at(
        &mut self,
        tab: crate::views::inspector::InspectorTab,
        cx: &mut Context<Self>,
    ) {
        self.undock_inspector(cx);
        self.set_inspector_tab(tab, cx);
    }

    pub fn redock_inspector(&mut self, cx: &mut Context<Self>) {
        self.floating_inspector_window = None;
        self.inspector_open = true;
        self.tag_input_active = false;
        self.tag_input_buffer.clear();
        self.save_config();
        cx.notify();
    }

    pub fn floating_inspector_closed(&mut self, cx: &mut Context<Self>) {
        self.floating_inspector_window = None;
        self.tag_input_active = false;
        self.tag_input_buffer.clear();
        self.save_config();
        cx.notify();
    }

    /// Drops whichever inspector edit is open, unsaved.
    fn cancel_inspector_edits(&mut self, cx: &mut Context<Self>) {
        self.tag_input_active = false;
        self.tag_input_buffer.clear();
        if self.caption.title_input_active {
            self.cancel_title_input(cx);
        }
        if self.caption.date_input_active {
            self.cancel_date_input(cx);
        }
        if self.caption.location_input_active {
            self.cancel_location_input(cx);
        }
        if self.caption.input_active {
            self.cancel_caption_input(cx);
        }
    }

    pub fn set_inspector_tab(
        &mut self,
        tab: crate::views::inspector::InspectorTab,
        cx: &mut Context<Self>,
    ) {
        // The open edit's field may not be on `tab`: end it as Esc would
        // instead of leaving its input taking keys out of sight.
        if tab != self.inspector_tab {
            self.cancel_inspector_edits(cx);
        }
        self.inspector_tab = tab;
        cx.notify();
    }

    pub fn open_card_context_menu(
        &mut self,
        item: MediaItem,
        index: usize,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if !self.selected_indices.contains(&index) {
            self.selected_indices.clear();
            self.selected_indices.insert(index);
            self.selection_anchor = Some(index);
            self.selected_index = Some(index);
            self.select_item_internal(item.clone(), cx);
        }
        self.context_menu_state = Some(ContextMenuState {
            position,
            item,
            hovered: None,
        });
        // Right-clicking a card can change the selected item; persist it on the
        // same debounced path as keyboard and left-click selection.
        self.schedule_session_save(cx);
        cx.notify();
    }

    pub fn close_context_menu(&mut self, cx: &mut Context<Self>) {
        if self.context_menu_state.is_some() {
            self.context_menu_state = None;
            cx.notify();
        }
    }

    /// Larger thumbnails: one level up the size ladder (`SIZE_COLUMNS`).
    pub fn zoom_in_grid(&mut self, cx: &mut Context<Self>) {
        self.step_grid_size(1, cx);
    }

    /// Smaller thumbnails: one level down the size ladder.
    pub fn zoom_out_grid(&mut self, cx: &mut Context<Self>) {
        self.step_grid_size(-1, cx);
    }

    /// Whether one `step` along the ladder (positive: larger) would change
    /// the size; false at the ladder's ends, where the pill's button disables.
    pub(crate) fn grid_zoom_enabled(&self, step: isize) -> bool {
        use crate::views::search_bar::SIZE_COLUMNS;
        let level = ladder_step(self.grid_cols(), step);
        (0..SIZE_COLUMNS.len() as isize).contains(&level)
    }

    /// ⌘= / ⌘- and the palette: anchored on the selected item, which stays on
    /// screen, or on the item at the viewport's top-left.
    fn step_grid_size(&mut self, step: isize, cx: &mut Context<Self>) {
        if self.zoom_grid_at(step, None, self.grid_scroll_y(), cx) {
            self.save_config();
        }
    }

    fn grid_scroll_y(&self) -> Pixels {
        self.grid_state.as_ref().map_or(px(0.0), |state| {
            state.scroll_handle.0.borrow().base_handle.offset().y
        })
    }

    /// Moves `step` levels along `SIZE_COLUMNS` (positive: fewer, larger
    /// cards) and scrolls so the anchor stays put. The anchor is the item
    /// under `anchor_pos` (wheel, pinch); with no position it is the selected
    /// item, held inside the viewport, or else the top-left item (keyboard).
    /// Returns `true` when the density changed.
    ///
    /// `scroll_y_before` is the offset the anchor is measured against. It is
    /// written back even when the level does not change, which undoes a scroll
    /// the list applied for a ⌘-scroll event. The offset goes to the public
    /// handle with `set_offset` (not a deferred `scroll_to_item`, of which
    /// GPUI keeps only one) and the next prepaint clamps it against the new
    /// content height.
    ///
    /// Limitation: a column change moves row boundaries, so the anchor item
    /// can move horizontally. This preserves its new row and the anchor's
    /// fractional position within that row; it is not a pixel-exact zoom.
    fn zoom_grid_at(
        &mut self,
        step: isize,
        anchor_pos: Option<Point<Pixels>>,
        scroll_y_before: Pixels,
        cx: &mut Context<Self>,
    ) -> bool {
        use crate::views::search_bar::SIZE_COLUMNS;
        // Mosaic anchors on its own layout and never touches the uniform
        // list's handle (a queued offset there would jump the Grid later).
        if self.mosaic_shown() {
            return self.zoom_mosaic_at(step, anchor_pos, cx);
        }
        let columns = self.grid_cols();
        let level = ladder_step(columns, step).clamp(0, SIZE_COLUMNS.len() as isize - 1);
        // No step yet (a trackpad burst below the threshold): keep the
        // count, even one that is off the ladder.
        let new_columns = if step == 0 {
            columns
        } else {
            SIZE_COLUMNS[level as usize]
        };
        let item_count = self.get_active_count();
        let handle = self
            .grid_state
            .as_ref()
            .map(|state| state.scroll_handle.clone());
        let set_offset = |y: Pixels| {
            if let Some(handle) = &handle {
                let state = handle.0.borrow();
                let base = &state.base_handle;
                base.set_offset(point(base.offset().x, y));
            }
        };
        if new_columns == columns {
            set_offset(scroll_y_before);
            return false;
        }

        let viewport = handle
            .as_ref()
            .map(|handle| handle.0.borrow().base_handle.bounds())
            .unwrap_or_default();
        let geometry = GridZoomGeometry {
            left: f32::from(viewport.origin.x),
            top: f32::from(viewport.origin.y),
            width: f32::from(viewport.size.width),
            height: f32::from(viewport.size.height),
            scroll_y: f32::from(scroll_y_before),
            row_height: self.row_height(),
            columns,
            column_gap: self.grid_metrics().column_gap,
            item_count,
        };
        let selected = self.selected_index.filter(|&index| index < item_count);
        let anchor = match (anchor_pos, selected) {
            (Some(pos), _) => {
                let (x, y) = (f32::from(pos.x), f32::from(pos.y));
                GridZoomAnchor::at_pointer(x, y, geometry).map(|anchor| (anchor, y))
            }
            (None, Some(index)) => GridZoomAnchor::at_item(index, geometry),
            (None, None) => GridZoomAnchor::at_pointer(geometry.left, geometry.top, geometry)
                .map(|anchor| (anchor, geometry.top)),
        };

        // An open album zooms its own grid; Library's zoom stays put.
        if self.album_tiles() {
            self.albums.page.columns = Some(new_columns);
        } else {
            self.grid_columns = new_columns;
        }
        match anchor {
            Some((anchor, y)) => {
                let target =
                    anchor.target_scroll_offset(y, geometry, new_columns, self.row_height());
                set_offset(px(-target));
            }
            // No usable geometry (before the first prepaint): change density
            // and let the deferred reveal find the selection, if any.
            None => {
                set_offset(scroll_y_before);
                if anchor_pos.is_none() {
                    self.reveal_grid_selection();
                }
            }
        }
        cx.notify();
        true
    }

    /// Cmd/Ctrl + wheel over the media grid walks the size ladder around the
    /// pointer: one level per line (mouse-wheel notch), one per
    /// `GRID_WHEEL_STEP_PX` of pixel (trackpad) scroll.
    ///
    /// Returns `true` when the event belongs to the zoom gesture, so the caller
    /// can stop propagation; plain wheel scrolling must stay scrolling.
    ///
    /// The uniform list's own scroll listener runs before this bubble handler
    /// (child listeners bubble first), so the event delta is removed from the
    /// live offset to recover the pre-gesture scroll position.
    pub fn zoom_grid_with_wheel(
        &mut self,
        event: &ScrollEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !event.modifiers.secondary(cx) {
            return false;
        }
        if matches!(event.touch_phase, TouchPhase::Started | TouchPhase::Ended) {
            self.grid_zoom.wheel_accum_px = 0.0;
        }

        let delta_y = f32::from(event.delta.pixel_delta(window.line_height()).y);
        if !delta_y.is_finite() || delta_y.abs() < GRID_ZOOM_WHEEL_EPSILON {
            return false;
        }
        if self.get_active_count() == 0 {
            return false;
        }
        let Some(scroll_handle) = self
            .grid_state
            .as_ref()
            .map(|state| state.scroll_handle.clone())
        else {
            return false;
        };

        let (viewport, offset_after, max_offset) = {
            let state = scroll_handle.0.borrow();
            let base = &state.base_handle;
            (base.bounds(), base.offset(), base.max_offset())
        };
        // When the pointer is over the list, its own scroll listener has already
        // applied this event's scroll delta; undo it so a zoom step never also
        // scrolls. Outside the list (for example the surface's top padding) no
        // scroll was applied. `max_offset` is from the layout the event was
        // dispatched against.
        let scroll_y_before = if viewport.contains(&event.position) {
            (offset_after.y - px(delta_y)).clamp(-max_offset.y, px(0.0))
        } else {
            offset_after.y
        };

        // Wheel up (positive delta) tightens the grid (fewer, larger cards),
        // matching ⌘= / ⌘-.
        let step = match event.delta {
            ScrollDelta::Lines(_) => delta_y.signum() as isize,
            ScrollDelta::Pixels(_) => take_steps(
                &mut self.grid_zoom.wheel_accum_px,
                delta_y,
                GRID_WHEEL_STEP_PX,
            ),
        };
        // Wheel bursts are high frequency; debounce the config write through the
        // existing session-save path instead of writing on every tick.
        if self.zoom_grid_at(step, Some(event.position), scroll_y_before, cx) {
            self.schedule_session_save(cx);
        }
        true
    }

    /// Trackpad pinch over the grid: one ladder level per `GRID_PINCH_STEP`
    /// of magnification, anchored at the pinch centre. Never scrolls.
    pub fn zoom_grid_with_pinch(&mut self, event: &PinchEvent, cx: &mut Context<Self>) {
        // macOS never sends `Cancelled`; a cancelled pinch ends as `Moved`.
        if matches!(event.phase, TouchPhase::Started | TouchPhase::Ended) {
            self.grid_zoom.pinch_accum = 0.0;
        }
        if !event.delta.is_finite() {
            return;
        }
        let step = take_steps(
            &mut self.grid_zoom.pinch_accum,
            event.delta,
            GRID_PINCH_STEP,
        );
        if step != 0 && self.zoom_grid_at(step, Some(event.position), self.grid_scroll_y(), cx) {
            self.schedule_session_save(cx);
        }
    }

    pub fn start_caption_input(&mut self, cx: &mut Context<Self>) {
        if self.selected_indices.len() > 1 {
            let existing = self.batch_draft(&self.batch_metadata.caption);
            self.caption.batch = crate::app::state::BatchEditState {
                field: Some(crate::app::state::BatchEditField::Caption),
                initial_buffer: existing.clone(),
            };
            self.caption.input_active = true;
            self.caption.input_buffer = existing;
            cx.notify();
            return;
        }
        let existing = self
            .selected_item
            .as_ref()
            .and_then(|it| self.item_captions.get(&it.id))
            .cloned()
            .unwrap_or_default();
        self.caption.input_active = true;
        self.caption.input_buffer = existing;
        cx.notify();
    }

    pub fn commit_caption_input(&mut self, cx: &mut Context<Self>) {
        if self.selected_indices.len() > 1 {
            self.commit_batch_caption_input(cx);
            return;
        }
        let caption = std::mem::take(&mut self.caption.input_buffer);
        self.caption.input_active = false;
        if let Some(item) = self.selected_item.clone() {
            let trimmed = clean_caption(&caption, MAX_TEXT_CHARS);
            // Cleaned like the stored value, so undo shows what it restores.
            let before = self
                .item_captions
                .get(&item.id)
                .map(|caption| clean_caption(caption, MAX_TEXT_CHARS))
                .filter(|caption| !caption.is_empty());
            let after = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.clone())
            };
            if before == after {
                cx.notify();
                return;
            }
            if let Some(ref value) = after {
                self.item_captions.insert(item.id, value.clone());
            } else {
                self.item_captions.remove(&item.id);
            }
            let result = if let Some(ref state) = self.grid_state {
                state.engine.set_caption(item.id, &trimmed)
            } else {
                Ok(())
            };
            if let Err(err) = result {
                if let Some(previous) = before {
                    self.item_captions.insert(item.id, previous);
                } else {
                    self.item_captions.remove(&item.id);
                }
                self.set_error(format!("Could not save caption: {err:#}"), cx);
                return;
            }
            self.push_undo(crate::app::undo::UndoAction::Caption {
                id: item.id,
                before,
                after,
            });
            self.sync_metadata_sidecars(&[item.id], cx);
            self.refresh_counts_if_rules_use(lumina_core::RuleField::Query);
            let message = if trimmed.is_empty() {
                "Caption removed"
            } else {
                "Caption saved"
            };
            self.set_last_action(message);
            self.show_undo_toast(message, cx);
        }
        cx.notify();
    }

    pub fn cancel_caption_input(&mut self, cx: &mut Context<Self>) {
        self.caption.input_active = false;
        self.caption.input_buffer.clear();
        let was_batch = self.caption.batch.field.take().is_some();
        self.caption.batch.initial_buffer.clear();
        if was_batch {
            // Restore the "Mixed" / shared value the batch row shows when idle.
            self.update_batch_summary();
        }
        cx.notify();
    }

    /// Draft a batch field opens with: the shared value, or empty when the
    /// selection disagrees so an unchanged Enter is a no-op.
    fn batch_draft(&self, field: &crate::app::state::BatchFieldSummary) -> String {
        if field.mixed {
            String::new()
        } else {
            field.common.clone().unwrap_or_default()
        }
    }

    fn commit_batch_caption_input(&mut self, cx: &mut Context<Self>) {
        let buffer = std::mem::take(&mut self.caption.input_buffer);
        let initial = std::mem::take(&mut self.caption.batch.initial_buffer);
        let field = self.caption.batch.field.take();
        self.caption.input_active = false;
        if field != Some(crate::app::state::BatchEditField::Caption) {
            self.update_batch_summary();
            cx.notify();
            return;
        }
        if buffer == initial {
            // Untouched draft: never overwrite N items silently.
            self.update_batch_summary();
            cx.notify();
            return;
        }
        let trimmed = clean_caption(&buffer, MAX_TEXT_CHARS);
        let after = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        };
        self.apply_batch_caption(after, cx);
    }

    fn commit_batch_title_input(&mut self, cx: &mut Context<Self>) {
        let buffer = std::mem::take(&mut self.caption.title_input_buffer);
        let initial = std::mem::take(&mut self.caption.batch.initial_buffer);
        let field = self.caption.batch.field.take();
        self.caption.title_input_active = false;
        if field != Some(crate::app::state::BatchEditField::Title) {
            self.update_batch_summary();
            cx.notify();
            return;
        }
        if buffer == initial {
            self.update_batch_summary();
            cx.notify();
            return;
        }
        let trimmed = clean_text(&buffer, MAX_TEXT_CHARS);
        let after = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        };
        self.apply_batch_title(after, cx);
    }

    pub fn start_title_input(&mut self, cx: &mut Context<Self>) {
        if self.selected_indices.len() > 1 {
            let existing = self.batch_draft(&self.batch_metadata.title);
            self.caption.batch = crate::app::state::BatchEditState {
                field: Some(crate::app::state::BatchEditField::Title),
                initial_buffer: existing.clone(),
            };
            self.caption.title_input_active = true;
            self.caption.title_input_buffer = existing;
            cx.notify();
            return;
        }
        let existing = self
            .selected_item
            .as_ref()
            .and_then(|item| item.title.clone())
            .unwrap_or_default();
        self.caption.title_input_active = true;
        self.caption.title_input_buffer = existing;
        cx.notify();
    }

    pub fn commit_title_input(&mut self, cx: &mut Context<Self>) {
        if self.selected_indices.len() > 1 {
            self.commit_batch_title_input(cx);
            return;
        }
        let value = std::mem::take(&mut self.caption.title_input_buffer);
        self.caption.title_input_active = false;
        let Some(item) = self.selected_item.clone() else {
            cx.notify();
            return;
        };
        let trimmed = clean_text(&value, MAX_TEXT_CHARS);
        let after = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        };
        // Cleaned like the stored value, so undo shows what it restores.
        let before = item
            .title
            .as_deref()
            .map(|title| clean_text(title, MAX_TEXT_CHARS))
            .filter(|title| !title.is_empty());
        if before == after {
            cx.notify();
            return;
        }
        let result = if let Some(ref state) = self.grid_state {
            state.engine.set_title(item.id, after.as_deref())
        } else {
            Ok(())
        };
        if let Err(err) = result {
            self.set_error(format!("Could not save title: {err:#}"), cx);
            return;
        }
        if let Some(ref mut current) = self.selected_item {
            current.title = after.clone();
        }
        let removed = after.is_none();
        self.push_undo(crate::app::undo::UndoAction::Title {
            id: item.id,
            before,
            after,
        });
        self.sync_metadata_sidecars(&[item.id], cx);
        let message = if removed {
            "Title removed"
        } else {
            "Title saved"
        };
        self.set_last_action(message);
        self.show_undo_toast(message, cx);
        cx.notify();
    }

    pub fn cancel_title_input(&mut self, cx: &mut Context<Self>) {
        self.caption.title_input_active = false;
        self.caption.title_input_buffer.clear();
        let was_batch = self.caption.batch.field.take().is_some();
        self.caption.batch.initial_buffer.clear();
        if was_batch {
            self.update_batch_summary();
        }
        cx.notify();
    }

    pub fn start_date_input(&mut self, cx: &mut Context<Self>) {
        if self.selected_indices.len() > 1 {
            let existing = self.batch_draft(&self.batch_metadata.date);
            self.caption.batch = crate::app::state::BatchEditState {
                field: Some(crate::app::state::BatchEditField::Date),
                initial_buffer: existing.clone(),
            };
            self.caption.date_input_active = true;
            self.caption.date_input_buffer = existing;
            cx.notify();
            return;
        }
        let current = self
            .selected_item
            .as_ref()
            .map(|item| item.timestamp_epoch_ms)
            .unwrap_or(0);
        let buffer = chrono::DateTime::from_timestamp_millis(current)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_default();
        self.caption.date_input_active = true;
        self.caption.date_input_buffer = buffer;
        cx.notify();
    }

    pub fn commit_date_input(&mut self, cx: &mut Context<Self>) {
        if self.selected_indices.len() > 1 {
            self.commit_batch_date_input(cx);
            return;
        }
        let value = std::mem::take(&mut self.caption.date_input_buffer);
        self.caption.date_input_active = false;
        let Some(item) = self.selected_item.clone() else {
            cx.notify();
            return;
        };
        let Some(epoch_ms) = parse_edited_timestamp(&value, item.timestamp_epoch_ms) else {
            self.set_error(DATE_INPUT_ERROR, cx);
            return;
        };
        if epoch_ms == item.timestamp_epoch_ms {
            cx.notify();
            return;
        }
        let result = if let Some(ref state) = self.grid_state {
            state.engine.set_timestamp(item.id, epoch_ms)
        } else {
            Ok(())
        };
        if let Err(err) = result {
            self.set_error(format!("Could not save date: {err:#}"), cx);
            return;
        }
        self.push_undo(crate::app::undo::UndoAction::Date {
            id: item.id,
            before: item.timestamp_epoch_ms,
            after: epoch_ms,
        });
        self.sync_metadata_sidecars(&[item.id], cx);
        if let Some(updated) = self
            .grid_state
            .as_ref()
            .and_then(|state| state.engine.get_item_by_id(item.id))
        {
            self.set_selected_from_catalog(updated);
        }
        self.refresh_album_counts();
        self.set_last_action("Date updated");
        self.show_undo_toast("Date updated", cx);
        cx.notify();
    }

    pub fn cancel_date_input(&mut self, cx: &mut Context<Self>) {
        self.caption.date_input_active = false;
        self.caption.date_input_buffer.clear();
        if self.caption.batch.field == Some(crate::app::state::BatchEditField::Date) {
            self.caption.batch.field = None;
            self.caption.batch.initial_buffer.clear();
            self.update_batch_summary();
        }
        cx.notify();
    }

    pub fn start_location_input(&mut self, cx: &mut Context<Self>) {
        if self.selected_indices.len() > 1 {
            let existing = self.batch_draft(&self.batch_metadata.location);
            self.caption.batch = crate::app::state::BatchEditState {
                field: Some(crate::app::state::BatchEditField::Location),
                initial_buffer: existing.clone(),
            };
            self.caption.location_input_active = true;
            self.caption.location_input_buffer = existing;
            cx.notify();
            return;
        }
        let buffer = self
            .selected_item
            .as_ref()
            .and_then(|item| match (item.latitude, item.longitude) {
                (Some(lat), Some(lon)) => Some(format!("{lat:.6}, {lon:.6}")),
                _ => None,
            })
            .unwrap_or_default();
        self.caption.location_input_active = true;
        self.caption.location_input_buffer = buffer;
        cx.notify();
    }

    pub fn commit_location_input(&mut self, cx: &mut Context<Self>) {
        if self.selected_indices.len() > 1 {
            self.commit_batch_location_input(cx);
            return;
        }
        let value = std::mem::take(&mut self.caption.location_input_buffer);
        self.caption.location_input_active = false;
        let Some(item) = self.selected_item.clone() else {
            cx.notify();
            return;
        };
        let (lat, lon) = match parse_location_coordinates(&value) {
            Ok(coords) => coords,
            Err(message) => {
                self.set_error(message, cx);
                return;
            }
        };
        if (lat, lon) == (item.latitude, item.longitude) {
            cx.notify();
            return;
        }
        let result = if let Some(ref state) = self.grid_state {
            state.engine.set_location(item.id, lat, lon)
        } else {
            Ok(())
        };
        if let Err(err) = result {
            self.set_error(format!("Could not save location: {err:#}"), cx);
            return;
        }
        self.push_undo(crate::app::undo::UndoAction::Location {
            id: item.id,
            before: (item.latitude, item.longitude),
            after: (lat, lon),
        });
        self.sync_metadata_sidecars(&[item.id], cx);
        self.refresh_counts_if_rules_use(lumina_core::RuleField::Query);
        if let Some(updated) = self
            .grid_state
            .as_ref()
            .and_then(|state| state.engine.get_item_by_id(item.id))
        {
            self.set_selected_from_catalog(updated);
        }
        let message = if lat.is_some() {
            "Location updated"
        } else {
            "Location removed"
        };
        self.set_last_action(message);
        self.show_undo_toast(message, cx);
        cx.notify();
    }

    /// Get Info "Remove location": the same path as submitting an empty
    /// location, so it is saved, undoable and reports errors.
    pub fn remove_location_selected(&mut self, cx: &mut Context<Self>) {
        self.caption.location_input_buffer.clear();
        self.caption.location_input_active = true;
        self.commit_location_input(cx);
    }

    pub fn cancel_location_input(&mut self, cx: &mut Context<Self>) {
        self.caption.location_input_active = false;
        self.caption.location_input_buffer.clear();
        if self.caption.batch.field == Some(crate::app::state::BatchEditField::Location) {
            self.caption.batch.field = None;
            self.caption.batch.initial_buffer.clear();
            self.update_batch_summary();
        }
        cx.notify();
    }

    fn commit_batch_date_input(&mut self, cx: &mut Context<Self>) {
        let buffer = std::mem::take(&mut self.caption.date_input_buffer);
        let initial = std::mem::take(&mut self.caption.batch.initial_buffer);
        let field = self.caption.batch.field.take();
        self.caption.date_input_active = false;
        if field != Some(crate::app::state::BatchEditField::Date) {
            self.update_batch_summary();
            cx.notify();
            return;
        }
        if buffer == initial {
            // Untouched draft: never rewrite the whole selection.
            self.update_batch_summary();
            cx.notify();
            return;
        }
        let trimmed = buffer.trim().to_string();
        if trimmed.is_empty() {
            self.set_error(DATE_INPUT_ERROR, cx);
            self.update_batch_summary();
            cx.notify();
            return;
        }
        self.apply_batch_date(&trimmed, cx);
    }

    fn commit_batch_location_input(&mut self, cx: &mut Context<Self>) {
        let buffer = std::mem::take(&mut self.caption.location_input_buffer);
        let initial = std::mem::take(&mut self.caption.batch.initial_buffer);
        let field = self.caption.batch.field.take();
        self.caption.location_input_active = false;
        if field != Some(crate::app::state::BatchEditField::Location) {
            self.update_batch_summary();
            cx.notify();
            return;
        }
        if buffer == initial {
            // Untouched draft: never rewrite the whole selection.
            self.update_batch_summary();
            cx.notify();
            return;
        }
        let trimmed = buffer.trim().to_string();
        self.apply_batch_location(&trimmed, cx);
    }
}

/// Parses the inspector's location draft: decimal degrees as
/// `latitude, longitude`, accepting comma or whitespace separators. An empty
/// draft clears the location. Shared by the single-item and batch editors so
/// both accept exactly the same syntax.
pub(crate) fn parse_location_coordinates(
    input: &str,
) -> Result<(Option<f64>, Option<f64>), &'static str> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok((None, None));
    }
    let parts: Vec<&str> = trimmed
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|part| !part.is_empty())
        .collect();
    if parts.len() != 2 {
        return Err("Enter coordinates as latitude, longitude");
    }
    let (Ok(lat), Ok(lon)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>()) else {
        return Err("Coordinates must be decimal numbers");
    };
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return Err("Latitude must be -90..90 and longitude -180..180");
    }
    Ok((Some(lat), Some(lon)))
}

/// Shown when the inspector's date draft is neither a date nor a shift.
pub(crate) const DATE_INPUT_ERROR: &str = "Enter a date as YYYY-MM-DD, or a shift like +2h or -1d";

/// Parses a signed date shift into milliseconds: `+2h`, `-1d`, `+1d 3h 15m`
/// (units d/h/m/s) or `-0:30` (`H:MM`). The sign is required, so a bare
/// `2h` is not a shift.
pub(crate) fn parse_date_shift(input: &str) -> Option<i64> {
    fn number(digits: &str) -> Option<i64> {
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        digits.parse().ok()
    }
    let trimmed = input.trim();
    let sign = match trimmed.chars().next()? {
        '+' => 1,
        '-' => -1,
        _ => return None,
    };
    let body = trimmed[1..].trim();
    let seconds = if let Some((hours, minutes)) = body.split_once(':') {
        let minutes = number(minutes).filter(|&m| m < 60 && minutes.len() == 2)?;
        number(hours)?
            .checked_mul(3600)?
            .checked_add(minutes * 60)?
    } else {
        let mut total: i64 = 0;
        let mut rest = body;
        if rest.is_empty() {
            return None;
        }
        while !rest.is_empty() {
            let end = rest
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(rest.len());
            let value = number(&rest[..end])?;
            let mut tail = rest[end..].chars();
            let unit = match tail.next()? {
                'd' => 86_400,
                'h' => 3_600,
                'm' => 60,
                's' => 1,
                _ => return None,
            };
            total = total.checked_add(value.checked_mul(unit)?)?;
            rest = tail.as_str().trim_start();
        }
        total
    };
    seconds.checked_mul(1000).map(|ms| sign * ms)
}

/// Parses the inspector's date draft against an item's current timestamp: a
/// shift (`+2h`, see [`parse_date_shift`]) moves it, `YYYY-MM-DD HH:MM:SS`
/// replaces it, and `YYYY-MM-DD` changes the day and keeps the time of day.
pub(crate) fn parse_edited_timestamp(input: &str, current_epoch_ms: i64) -> Option<i64> {
    let trimmed = input.trim();
    if let Some(delta) = parse_date_shift(trimmed) {
        // Out of chrono's range is refused here, not by the catalog write.
        return current_epoch_ms
            .checked_add(delta)
            .filter(|&ms| chrono::DateTime::from_timestamp_millis(ms).is_some());
    }
    if let Ok(datetime) = chrono::NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S") {
        return Some(datetime.and_utc().timestamp_millis());
    }
    if let Ok(date) = chrono::NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
        let time = chrono::DateTime::from_timestamp_millis(current_epoch_ms)
            .map(|dt| dt.time())
            .unwrap_or_default();
        return Some(date.and_time(time).and_utc().timestamp_millis());
    }
    None
}

/// What the sidebar toggle does next: "Hide Sidebar" or "Show Sidebar".
pub(crate) fn sidebar_toggle_title(collapsed: bool) -> String {
    crate::i18n::t(if collapsed {
        "sidebar.show"
    } else {
        "sidebar.hide"
    })
}

/// The toolbar and palette icon for the sidebar toggle.
pub(crate) fn sidebar_toggle_icon(collapsed: bool) -> crate::icons::Icon {
    if collapsed {
        crate::icons::Icon::PanelLeftOpen
    } else {
        crate::icons::Icon::PanelLeftClose
    }
}

/// Show/Hide Sidebar, the macOS standard in Finder, Mail and Notes.
pub(crate) const SIDEBAR_SHORTCUT: &str = "⌃⌘S";

/// The ladder level `step` levels (positive: larger) from `columns`. An
/// off-ladder count (an album's 5) sits between two levels, so its first
/// step either way lands on the neighbour: 5 zooms in to 4 and out to 6.
fn ladder_step(columns: usize, step: isize) -> isize {
    use crate::views::search_bar::{size_level, SIZE_COLUMNS};
    let level = size_level(columns);
    let off_ladder = SIZE_COLUMNS[level] != columns.clamp(2, 8);
    level as isize + step - isize::from(off_ladder && step > 0)
}

#[cfg(test)]
mod tests {
    use super::{parse_date_shift, parse_edited_timestamp};

    const MIN: i64 = 60_000;

    #[core::prelude::v1::test]
    fn parse_date_shift_units_and_signs() {
        assert_eq!(parse_date_shift("+2h"), Some(120 * MIN), "hours");
        assert_eq!(
            parse_date_shift("-1d"),
            Some(-1440 * MIN),
            "a minus shifts back"
        );
        assert_eq!(
            parse_date_shift("+1d 3h 15m"),
            Some((1440 + 180 + 15) * MIN),
            "units add up"
        );
        assert_eq!(parse_date_shift(" +45s "), Some(45_000), "seconds, trimmed");
        assert_eq!(parse_date_shift("-0:30"), Some(-30 * MIN), "H:MM form");
        assert_eq!(parse_date_shift("+1:05"), Some(65 * MIN), "H:MM form");
    }

    #[core::prelude::v1::test]
    fn parse_date_shift_rejects_garbage() {
        for input in [
            "+",
            "2h",
            "+2x",
            "",
            "-",
            "+h",
            "+2",
            "+2h x",
            "+1:5",
            "+1:60",
            "+:30",
            "+-2h",
            "2024-05-01",
        ] {
            assert_eq!(parse_date_shift(input), None, "{input:?} is not a shift");
        }
    }

    #[core::prelude::v1::test]
    fn date_shift_out_of_range_is_refused() {
        assert_eq!(
            parse_edited_timestamp("+999999999d", 0),
            None,
            "a shift past chrono's range is an input error, not a failed write"
        );
        assert_eq!(parse_edited_timestamp("+1d", 0), Some(86_400_000));
    }
}
