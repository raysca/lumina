use gpui::*;
use lumina_core::{MediaId, MediaItem, MediaType};
use std::cmp;

use crate::app::types::{BatchSelectionSummary, UiMessage};
use crate::app::LuminaApp;
use crate::theme;

impl LuminaApp {
    pub fn set_rating_selected(&mut self, rating: u8, cx: &mut Context<Self>) {
        let ids = self.get_selected_ids();
        for id in ids {
            let current = self.item_ratings.get(&id).copied().unwrap_or(0);
            let new_r = if current == rating { 0 } else { rating };
            self.set_item_rating(id, new_r, cx);
        }
    }

    pub fn set_color_label_selected(&mut self, label: u8, cx: &mut Context<Self>) {
        let ids = self.get_selected_ids();
        for id in ids {
            let current = self.item_color_labels.get(&id).copied().unwrap_or(0);
            let new_l = if current == label { 0 } else { label };
            self.set_item_color_label(id, new_l, cx);
        }
    }

    pub fn toggle_favorite_selected(&mut self, cx: &mut Context<Self>) {
        if let Some(ref mut item) = self.selected_item {
            let new_fav = !item.is_favorite;
            item.is_favorite = new_fav;
            if let Some(ref state) = self.grid_state {
                let _ = state.engine.toggle_favorite(item.id, new_fav);
            }
            self.refresh_album_counts();
            cx.notify();
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

    pub fn select_item_at(&mut self, item: MediaItem, index: usize, cx: &mut Context<Self>) {
        self.selected_indices.clear();
        self.selected_indices.insert(index);
        self.selection_anchor = Some(index);
        self.selected_index = Some(index);
        self.select_item_internal(item, cx);
    }

    pub fn select_item(&mut self, item: MediaItem, cx: &mut Context<Self>) {
        self.select_item_internal(item, cx);
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

        if let Some(ref state) = self.grid_state {
            let _ = state.engine.batch_set_favorite(&ids, new_fav);
            if let Some(ref mut it) = self.selected_item {
                if ids.contains(&it.id) {
                    it.is_favorite = new_fav;
                }
            }
        }
        self.refresh_album_counts();
        cx.notify();
    }

    pub fn set_item_rating(&mut self, id: MediaId, rating: u8, cx: &mut Context<Self>) {
        let clamped = rating.min(5);
        if clamped == 0 {
            self.item_ratings.remove(&id);
        } else {
            self.item_ratings.insert(id, clamped);
        }
        if let Some(ref state) = self.grid_state {
            let _ = state.engine.set_rating(id, clamped);
        }
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
        let clamped = label.min(6);
        if clamped == 0 {
            self.item_color_labels.remove(&id);
        } else {
            self.item_color_labels.insert(id, clamped);
        }
        if let Some(ref state) = self.grid_state {
            let _ = state.engine.set_color_label(id, clamped);
        }
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
