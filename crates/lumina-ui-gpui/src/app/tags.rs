use gpui::*;

use crate::app::types::TagSortOrder;
use crate::app::LuminaApp;
use crate::views::sidebar::AlbumSelection;

impl LuminaApp {
    #[cfg(test)]
    pub fn refresh_tag_counts(&mut self) {
        if let Some(ref state) = self.grid_state {
            self.library_tag_counts = state.engine.query_tag_counts();
            let has_tag_smart_albums = self.user_smart_albums.iter().any(|a| {
                a.rules
                    .iter()
                    .any(|r| r.field == lumina_core::RuleField::Tag)
            });
            if has_tag_smart_albums {
                for a in &self.user_smart_albums {
                    if a.rules
                        .iter()
                        .any(|r| r.field == lumina_core::RuleField::Tag)
                    {
                        self.user_album_counts
                            .insert(a.id, state.engine.query_smart_album_count(a));
                    }
                }
            }
        }
    }

    pub fn batch_add_tag(&mut self, tag: String, cx: &mut Context<Self>) {
        let clean = tag.trim().to_lowercase();
        if clean.is_empty() {
            return;
        }
        let ids = self.get_selected_ids();
        if ids.is_empty() {
            return;
        }

        if let Some(ref state) = self.grid_state {
            let _ = state.engine.add_tags_batch(&ids, &[clean]);
        }
        self.refresh_selected_tags();
        self.refresh_album_counts();
        cx.notify();
    }

    pub fn batch_remove_tag(&mut self, tag: String, cx: &mut Context<Self>) {
        let clean = tag.trim().to_lowercase();
        if clean.is_empty() {
            return;
        }
        let ids = self.get_selected_ids();
        if ids.is_empty() {
            return;
        }

        if let Some(ref state) = self.grid_state {
            let _ = state.engine.remove_tags_batch(&ids, &[clean]);
        }
        self.refresh_selected_tags();
        self.refresh_album_counts();
        cx.notify();
    }

    pub fn start_tag_input(&mut self, cx: &mut Context<Self>) {
        self.tag_input_active = true;
        self.tag_input_buffer.clear();
        cx.notify();
    }

    pub fn commit_tag_input(&mut self, cx: &mut Context<Self>) {
        let tag = std::mem::take(&mut self.tag_input_buffer);
        self.tag_input_active = false;
        let clean = tag.trim().to_string();
        if !clean.is_empty() {
            self.batch_add_tag(clean, cx);
        }
        cx.notify();
    }

    pub fn cancel_tag_input(&mut self, cx: &mut Context<Self>) {
        self.tag_input_active = false;
        self.tag_input_buffer.clear();
        cx.notify();
    }

    #[cfg(test)]
    pub fn start_tag_rename(&mut self, tag: String, cx: &mut Context<Self>) {
        self.editing_tag = Some(tag.clone());
        self.editing_tag_buffer = tag;
        cx.notify();
    }

    pub fn commit_tag_rename(&mut self, cx: &mut Context<Self>) {
        if let Some(old_tag) = self.editing_tag.take() {
            let new_tag = std::mem::take(&mut self.editing_tag_buffer);
            let clean_new = new_tag.trim().to_lowercase();
            if !clean_new.is_empty() && clean_new != old_tag {
                if let Some(ref state) = self.grid_state {
                    let _ = state.engine.rename_tag(&old_tag, &clean_new);
                }
                if let AlbumSelection::Tag(ref curr) = self.selection {
                    if curr == &old_tag {
                        self.selection = AlbumSelection::Tag(clean_new.clone());
                    }
                }
                // If search query is currently filtering on old_tag, update it to clean_new
                let old_query = format!("tag:{}", old_tag);
                let old_query_quoted = format!("tag:\"{}\"", old_tag);
                if self.search_query == old_query || self.search_query == old_query_quoted {
                    self.search_query = if clean_new.contains(' ') {
                        format!("tag:\"{}\"", clean_new)
                    } else {
                        format!("tag:{}", clean_new)
                    };
                }
                self.refresh_selected_tags();
                self.refresh_album_counts();
            }
        }
        cx.notify();
    }

    pub fn cancel_tag_rename(&mut self, cx: &mut Context<Self>) {
        self.editing_tag = None;
        self.editing_tag_buffer.clear();
        cx.notify();
    }

    pub fn delete_tag_globally(&mut self, tag: String, cx: &mut Context<Self>) {
        let clean = tag.trim().to_lowercase();
        if clean.is_empty() {
            return;
        }
        if let Some(ref state) = self.grid_state {
            let _ = state.engine.delete_tag_globally(&clean);
        }
        if let AlbumSelection::Tag(ref curr) = self.selection {
            if curr == &clean {
                self.selection = AlbumSelection::AllLibrary;
            }
        }
        let tag_query = format!("tag:{}", clean);
        let tag_query_quoted = format!("tag:\"{}\"", clean);
        if self.search_query == tag_query || self.search_query == tag_query_quoted {
            self.clear_search(cx);
        }
        self.refresh_selected_tags();
        self.refresh_album_counts();
        cx.notify();
    }

    pub fn open_tag_manager(&mut self, cx: &mut Context<Self>) {
        self.tag_manager.open = true;
        self.tag_manager.search.clear();
        self.tag_manager.selected.clear();
        self.tag_manager.editing = None;
        self.tag_manager.merge_source = None;
        cx.notify();
    }

    pub fn close_tag_manager(&mut self, cx: &mut Context<Self>) {
        self.tag_manager.open = false;
        self.tag_manager.editing = None;
        self.tag_manager.merge_source = None;
        cx.notify();
    }

    pub fn set_tag_sort_order(&mut self, order: TagSortOrder, cx: &mut Context<Self>) {
        self.tag_manager.sort = order;
        cx.notify();
    }

    pub fn toggle_tag_manager_selection(&mut self, tag: String, cx: &mut Context<Self>) {
        if self.tag_manager.selected.contains(&tag) {
            self.tag_manager.selected.remove(&tag);
        } else {
            self.tag_manager.selected.insert(tag);
        }
        cx.notify();
    }

    pub fn clear_tag_manager_selection(&mut self, cx: &mut Context<Self>) {
        self.tag_manager.selected.clear();
        cx.notify();
    }

    pub fn start_modal_tag_rename(&mut self, tag: String, cx: &mut Context<Self>) {
        self.tag_manager.editing = Some(tag.clone());
        self.tag_manager.editing_buffer = tag;
        cx.notify();
    }

    pub fn commit_modal_tag_rename(&mut self, cx: &mut Context<Self>) {
        if let Some(old_tag) = self.tag_manager.editing.take() {
            let new_tag = self.tag_manager.editing_buffer.trim().to_lowercase();
            self.tag_manager.editing_buffer.clear();
            if !new_tag.is_empty() && new_tag != old_tag.to_lowercase() {
                if let Some(ref state) = self.grid_state {
                    let _ = state.engine.rename_tag(&old_tag, &new_tag);
                }
                let old_query = format!("tag:{}", old_tag);
                let old_query_quoted = format!("tag:\"{}\"", old_tag);
                if self.search_query == old_query || self.search_query == old_query_quoted {
                    self.search_query = format!("tag:{}", new_tag);
                }
                self.refresh_selected_tags();
                self.refresh_album_counts();
            }
            cx.notify();
        }
    }

    pub fn cancel_modal_tag_rename(&mut self, cx: &mut Context<Self>) {
        self.tag_manager.editing = None;
        self.tag_manager.editing_buffer.clear();
        cx.notify();
    }

    pub fn start_tag_merge(&mut self, source_tag: String, cx: &mut Context<Self>) {
        self.tag_manager.merge_source = Some(source_tag);
        cx.notify();
    }

    pub fn cancel_tag_merge(&mut self, cx: &mut Context<Self>) {
        self.tag_manager.merge_source = None;
        cx.notify();
    }

    pub fn commit_tag_merge(&mut self, target_tag: String, cx: &mut Context<Self>) {
        if let Some(source_tag) = self.tag_manager.merge_source.take() {
            if source_tag.to_lowercase() != target_tag.to_lowercase() {
                if let Some(ref state) = self.grid_state {
                    let _ = state.engine.merge_tags(&source_tag, &target_tag);
                }
                if let AlbumSelection::Tag(ref curr) = self.selection {
                    if curr.to_lowercase() == source_tag.to_lowercase() {
                        self.selection = AlbumSelection::Tag(target_tag.clone());
                    }
                }
                let old_query = format!("tag:{}", source_tag);
                let old_query_quoted = format!("tag:\"{}\"", source_tag);
                if self.search_query == old_query || self.search_query == old_query_quoted {
                    self.search_query = format!("tag:{}", target_tag);
                }
                self.refresh_selected_tags();
                self.refresh_album_counts();
            }
            cx.notify();
        }
    }

    pub fn bulk_delete_tags(&mut self, tags: Vec<String>, cx: &mut Context<Self>) {
        let engine_opt = self.grid_state.as_ref().map(|s| s.engine.clone());
        if let AlbumSelection::Tag(ref curr) = self.selection {
            if tags.iter().any(|t| t.to_lowercase() == curr.to_lowercase()) {
                self.selection = AlbumSelection::AllLibrary;
            }
        }
        if let Some(engine) = engine_opt {
            let mut should_clear = false;
            for tag in &tags {
                let clean = tag.trim().to_lowercase();
                if !clean.is_empty() {
                    let _ = engine.delete_tag_globally(&clean);
                    let tag_query = format!("tag:{}", clean);
                    let tag_query_quoted = format!("tag:\"{}\"", clean);
                    if self.search_query == tag_query || self.search_query == tag_query_quoted {
                        should_clear = true;
                    }
                }
            }
            if should_clear {
                self.clear_search(cx);
            }
        }
        self.tag_manager.selected.clear();
        self.refresh_selected_tags();
        self.refresh_album_counts();
        cx.notify();
    }
}
