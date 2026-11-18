//! Batch inspector writes: one title, caption, date or location applied to
//! every selected item as a single undo step.

use gpui::*;
use lumina_core::MediaId;

use crate::app::inline_edit::{
    parse_edited_timestamp, parse_location_coordinates, DATE_INPUT_ERROR,
};
use crate::app::undo::{BatchField, BatchLocationChange, UndoAction};
use crate::app::LuminaApp;
use lumina_core::Coordinates;

impl LuminaApp {
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
}
