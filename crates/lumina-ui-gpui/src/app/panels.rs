use gpui::*;
use lumina_core::MediaItem;

use crate::app::types::UiMessage;
use crate::app::LuminaApp;
use crate::views::context_menu::ContextMenuState;

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
        self.context_menu_state = Some(ContextMenuState { position, item });
        cx.notify();
    }

    pub fn close_context_menu(&mut self, cx: &mut Context<Self>) {
        if self.context_menu_state.is_some() {
            self.context_menu_state = None;
            cx.notify();
        }
    }

    pub fn zoom_in_grid(&mut self, cx: &mut Context<Self>) {
        if self.grid_columns > 2 {
            self.grid_columns -= 1;
            self.save_config();
            cx.notify();
        }
    }

    pub fn zoom_out_grid(&mut self, cx: &mut Context<Self>) {
        if self.grid_columns < 8 {
            self.grid_columns += 1;
            self.save_config();
            cx.notify();
        }
    }

    pub fn start_caption_input(&mut self, cx: &mut Context<Self>) {
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
        let caption = std::mem::take(&mut self.caption.input_buffer);
        self.caption.input_active = false;
        if let Some(item) = self.selected_item.clone() {
            let trimmed = caption.trim().to_string();
            let before = self.item_captions.get(&item.id).cloned();
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
            self.sync_metadata_sidecars(&[item.id]);
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
        cx.notify();
    }

    pub fn start_title_input(&mut self, cx: &mut Context<Self>) {
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
        let value = std::mem::take(&mut self.caption.title_input_buffer);
        self.caption.title_input_active = false;
        let Some(item) = self.selected_item.clone() else {
            cx.notify();
            return;
        };
        let trimmed = value.trim();
        let after = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        };
        let before = item.title.clone();
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
        self.push_undo(crate::app::undo::UndoAction::Title {
            id: item.id,
            before,
            after,
        });
        self.sync_metadata_sidecars(&[item.id]);
        let message = if trimmed.is_empty() {
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
        cx.notify();
    }

    pub fn start_date_input(&mut self, cx: &mut Context<Self>) {
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
            self.selected_item = Some(updated);
        }
        self.refresh_album_counts();
        self.set_last_action("Date updated");
        self.show_undo_toast("Date updated", cx);
        cx.notify();
    }

    pub fn cancel_date_input(&mut self, cx: &mut Context<Self>) {
        self.caption.date_input_active = false;
        self.caption.date_input_buffer.clear();
        cx.notify();
    }

    pub fn start_location_input(&mut self, cx: &mut Context<Self>) {
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
        let value = std::mem::take(&mut self.caption.location_input_buffer);
        self.caption.location_input_active = false;
        let Some(item) = self.selected_item.clone() else {
            cx.notify();
            return;
        };
        let trimmed = value.trim();
        let (lat, lon) = if trimmed.is_empty() {
            (None, None)
        } else {
            let parts: Vec<&str> = trimmed
                .split(|c: char| c == ',' || c.is_whitespace())
                .filter(|part| !part.is_empty())
                .collect();
            if parts.len() != 2 {
                self.set_error("Enter coordinates as latitude, longitude", cx);
                return;
            }
            let (Ok(lat), Ok(lon)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>()) else {
                self.set_error("Coordinates must be decimal numbers", cx);
                return;
            };
            if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
                self.set_error("Latitude must be -90..90 and longitude -180..180", cx);
                return;
            }
            (Some(lat), Some(lon))
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
            self.selected_item = Some(updated);
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

    pub fn cancel_location_input(&mut self, cx: &mut Context<Self>) {
        self.caption.location_input_active = false;
        self.caption.location_input_buffer.clear();
        cx.notify();
    }
}

fn parse_edited_timestamp(input: &str, current_epoch_ms: i64) -> Option<i64> {
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
