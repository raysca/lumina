//! Metadata edits across the selection: labels, favorites and the batch
//! inspector's title, caption, date and location.

use gpui::*;
use lumina_core::{MediaId, MediaItem};

use crate::app::batch_summary::{summarize, BATCH_SYNC_LIMIT};
use crate::app::state::{BatchFieldSummary, BatchMetadataSummary};
use crate::app::undo::UndoAction;
use crate::app::LuminaApp;

mod apply;

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
            Err(err) => self.set_error(format!("Could not update favorite: {err:#}"), cx),
        }
    }

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
        self.sync_batch_inputs();
    }

    /// Shows the batch summary in the inspector fields not being edited.
    pub(crate) fn sync_batch_inputs(&mut self) {
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

    /// Summarises the selection for the batch inspector: on the UI thread
    /// up to [`BATCH_SYNC_LIMIT`] rows, in the background above it.
    pub fn update_batch_summary(&mut self) {
        // A summary still loading belongs to the selection before this one.
        self.batch_load.invalidate();
        if self.selected_indices.len() > BATCH_SYNC_LIMIT {
            self.load_batch_summary();
            return;
        }
        if self.selected_indices.is_empty() {
            let items: Vec<MediaItem> = self.selected_item.iter().cloned().collect();
            self.batch_summary = summarize(&items);
            self.refresh_batch_metadata(&[]);
            return;
        }
        let items = self.get_selected_items();
        self.batch_summary = summarize(&items);
        self.refresh_batch_metadata(&items);
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
            Err(err) => self.set_error(format!("Could not update favorites: {err:#}"), cx),
        }
    }
}

/// Summarizes one text field across the batch selection.
///
/// `mixed` is true when the items disagree; `common` then carries `None`
/// because there is no single value to show. An empty iterator (no selection)
/// yields the default "not set" summary.
pub(crate) fn summarize_batch_text<I>(values: I) -> BatchFieldSummary
where
    I: IntoIterator<Item = Option<String>>,
{
    summarize_by(values, |value| value)
}

/// The shared first/mixed reduction: one value feeds `render`, any disagreement
/// is `mixed` with no common value. An empty iterator is the "not set" summary.
fn summarize_by<I, T, F>(values: I, render: F) -> BatchFieldSummary
where
    I: IntoIterator<Item = T>,
    T: PartialEq,
    F: FnOnce(T) -> Option<String>,
{
    let mut values = values.into_iter();
    let Some(first) = values.next() else {
        return BatchFieldSummary::default();
    };
    if values.any(|value| value != first) {
        return BatchFieldSummary {
            mixed: true,
            common: None,
        };
    }
    BatchFieldSummary {
        mixed: false,
        common: render(first),
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
pub(crate) fn summarize_batch_timestamps<I>(values: I) -> BatchFieldSummary
where
    I: IntoIterator<Item = i64>,
{
    summarize_by(values, |first| Some(format_batch_timestamp(first)))
}

/// Summarizes coordinates across the batch selection using the single-item
/// editor's `latitude, longitude` form. Items with no coordinates are "not set".
pub(crate) fn summarize_batch_locations<I>(values: I) -> BatchFieldSummary
where
    I: IntoIterator<Item = (Option<f64>, Option<f64>)>,
{
    summarize_by(values, |first| match first {
        (Some(lat), Some(lon)) => Some(format!("{lat:.6}, {lon:.6}")),
        _ => None,
    })
}

fn format_batch_timestamp(epoch_ms: i64) -> String {
    crate::app::types::format_edit_timestamp(epoch_ms)
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
