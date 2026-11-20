//! The inspector's inline date and location edits, single and batch.

use gpui::*;

use super::{parse_edited_timestamp, parse_location_coordinates, DATE_INPUT_ERROR};
use crate::app::LuminaApp;

impl LuminaApp {
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
        let buffer = crate::app::types::format_edit_timestamp(current);
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

    pub(super) fn commit_batch_date_input(&mut self, cx: &mut Context<Self>) {
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

    pub(super) fn commit_batch_location_input(&mut self, cx: &mut Context<Self>) {
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
