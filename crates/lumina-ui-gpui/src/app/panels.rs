use gpui::*;
use lumina_core::text::{clean_caption, clean_text, MAX_TEXT_CHARS};
use lumina_core::MediaItem;

use crate::app::types::UiMessage;
use crate::app::LuminaApp;
use crate::views::context_menu::ContextMenuState;
use crate::views::grid::{GridZoomAnchor, GridZoomGeometry};

/// Wheel delta below which a Cmd/Ctrl + wheel event is treated as noise.
const GRID_ZOOM_WHEEL_EPSILON: f32 = 0.01;

impl LuminaApp {
    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        self.save_config();
        cx.notify();
    }

    pub fn toggle_inspector(&mut self, cx: &mut Context<Self>) {
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

    pub fn set_inspector_tab(
        &mut self,
        tab: crate::views::inspector::InspectorTab,
        cx: &mut Context<Self>,
    ) {
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

    /// Larger thumbnails: one step up the size slider's 5 levels.
    pub fn zoom_in_grid(&mut self, cx: &mut Context<Self>) {
        self.step_grid_size(1, cx);
    }

    /// Smaller thumbnails: one step down the size slider's 5 levels.
    pub fn zoom_out_grid(&mut self, cx: &mut Context<Self>) {
        self.step_grid_size(-1, cx);
    }

    fn step_grid_size(&mut self, step: isize, cx: &mut Context<Self>) {
        use crate::views::search_bar::{size_level, SIZE_COLUMNS};
        let level = (size_level(self.grid_columns) as isize + step)
            .clamp(0, SIZE_COLUMNS.len() as isize - 1) as usize;
        if self.grid_columns != SIZE_COLUMNS[level] {
            self.grid_columns = SIZE_COLUMNS[level];
            self.save_config();
            cx.notify();
        }
    }

    /// Cmd/Ctrl + wheel over the media grid changes column density while
    /// keeping the item under the pointer anchored as closely as the new row
    /// layout allows.
    ///
    /// Returns `true` when the event belongs to the zoom gesture, so the caller
    /// can stop propagation; plain wheel scrolling must stay scrolling.
    ///
    /// The uniform list's own scroll listener runs before this bubble handler
    /// (child listeners bubble first), so the event delta is removed from the
    /// live offset to recover the pre-gesture scroll position. The new offset is
    /// applied to the public scroll handle and clamped against the new content
    /// height in the next prepaint.
    ///
    /// Limitation: a column change moves row boundaries, so the item under the
    /// pointer can change horizontally. This preserves the anchor item's new row
    /// and the pointer's fractional position within that row; it is not a
    /// pixel-exact zoom transform.
    pub fn zoom_grid_with_wheel(
        &mut self,
        event: &ScrollWheelEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !event.modifiers.secondary() {
            return false;
        }

        let delta_y = f32::from(event.delta.pixel_delta(window.line_height()).y);
        if !delta_y.is_finite() || delta_y.abs() < GRID_ZOOM_WHEEL_EPSILON {
            return false;
        }

        let item_count = self.get_active_count();
        if item_count == 0 {
            return false;
        }
        let Some(scroll_handle) = self
            .grid_state
            .as_ref()
            .map(|state| state.scroll_handle.clone())
        else {
            return false;
        };

        let columns = self.grid_columns.clamp(2, 8);
        // Wheel up (positive delta) tightens the grid (fewer, larger cards),
        // matching the Cmd+= / Cmd+- semantics.
        let step: isize = if delta_y > 0.0 { -1 } else { 1 };
        let new_columns = (columns as isize + step).clamp(2, 8) as usize;

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
            (offset_after.y - px(delta_y)).clamp(-max_offset.height, px(0.0))
        } else {
            offset_after.y
        };

        let geometry = GridZoomGeometry {
            left: f32::from(viewport.origin.x),
            top: f32::from(viewport.origin.y),
            width: f32::from(viewport.size.width),
            height: f32::from(viewport.size.height),
            scroll_y: f32::from(scroll_y_before),
            row_height: self.row_height(),
            columns,
            item_count,
        };
        let anchor = GridZoomAnchor::at_pointer(
            f32::from(event.position.x),
            f32::from(event.position.y),
            geometry,
        );

        self.grid_columns = new_columns;
        let new_row_height = self.row_height();
        let target_offset = match anchor {
            Some(anchor) => anchor.target_scroll_offset(
                f32::from(event.position.y),
                geometry,
                new_columns,
                new_row_height,
            ),
            // No usable geometry (before the first prepaint): change density
            // without a scroll adjustment.
            None => f32::from(-scroll_y_before),
        };

        // Apply after the column change; the next prepaint clamps this against
        // the new content height.
        {
            let state = scroll_handle.0.borrow();
            let base = &state.base_handle;
            base.set_offset(point(base.offset().x, px(-target_offset)));
        }

        // Wheel bursts are high frequency; debounce the config write through the
        // existing session-save path instead of writing on every tick.
        self.schedule_session_save(cx);
        cx.notify();
        true
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
                self.set_error(format!("Could not save caption: {err}"), cx);
                return;
            }
            self.push_undo(crate::app::undo::UndoAction::Caption {
                id: item.id,
                before,
                after,
            });
            self.sync_metadata_sidecars(&[item.id], cx);
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
            self.set_error(format!("Could not save title: {err}"), cx);
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
            self.set_error("Enter a date as YYYY-MM-DD or YYYY-MM-DD HH:MM:SS", cx);
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
            self.set_error(format!("Could not save date: {err}"), cx);
            return;
        }
        self.push_undo(crate::app::undo::UndoAction::Date {
            id: item.id,
            before: item.timestamp_epoch_ms,
            after: epoch_ms,
        });
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
            self.set_error(format!("Could not save location: {err}"), cx);
            return;
        }
        self.push_undo(crate::app::undo::UndoAction::Location {
            id: item.id,
            before: (item.latitude, item.longitude),
            after: (lat, lon),
        });
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
            self.set_error("Enter a date as YYYY-MM-DD or YYYY-MM-DD HH:MM:SS", cx);
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

pub(crate) fn parse_edited_timestamp(input: &str, current_epoch_ms: i64) -> Option<i64> {
    let trimmed = input.trim();
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
