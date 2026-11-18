//! The inspector's inline title, caption, date and location edits, single
//! and batch, and the date-shift and coordinate parsers they share.

use gpui::*;
use lumina_core::text::{clean_caption, clean_text, MAX_TEXT_CHARS};

use crate::app::LuminaApp;

mod date_location;
mod parse;

pub(crate) use parse::{parse_edited_timestamp, parse_location_coordinates, DATE_INPUT_ERROR};

impl LuminaApp {
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
}
