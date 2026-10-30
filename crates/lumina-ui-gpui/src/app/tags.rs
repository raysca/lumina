use gpui::*;

use crate::app::types::TagSortOrder;
use crate::app::LuminaApp;
use crate::views::sidebar::AlbumSelection;

impl LuminaApp {
    #[cfg(test)]
    pub fn refresh_tag_counts(&mut self) {
        if let Some(ref state) = self.grid_state {
            self.library_tag_counts = state.engine.query_tag_counts();
            self.untagged_count = state.engine.query_untagged_count();
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

    /// Runs a tag mutation and reports failures through the error banner.
    ///
    /// Tag changes are destructive, so a failure must never be swallowed or let
    /// the UI claim a tag was renamed/deleted when the catalog kept it. Returns
    /// `None` after surfacing the error so callers apply only their success
    /// state changes (selection remap, refreshed lists, toast).
    pub(crate) fn apply_tag_mutation<T>(
        &mut self,
        action: &str,
        result: anyhow::Result<T>,
        cx: &mut Context<Self>,
    ) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(err) => {
                log::warn!("{action}: {err:?}");
                self.set_error(format!("{action}: {err}"), cx);
                None
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

        // Undo must only touch items this edit changes: items that already
        // had the tag keep it present after an undo.
        let result = if let Some(ref state) = self.grid_state {
            state.engine.ids_with_tag(&ids, &clean).and_then(|had| {
                let had: std::collections::HashSet<_> = had.into_iter().collect();
                let changed: Vec<_> = ids.iter().copied().filter(|id| !had.contains(id)).collect();
                state
                    .engine
                    .add_tags_batch(&changed, std::slice::from_ref(&clean))?;
                Ok(changed)
            })
        } else {
            Ok(Vec::new())
        };
        let Some(changed) = self.apply_tag_mutation("Could not add tag", result, cx) else {
            return;
        };
        if changed.is_empty() {
            // Nothing to write or undo; an undo toast here would undo an
            // older, unrelated action.
            cx.notify();
            return;
        }
        self.push_undo(crate::app::undo::UndoAction::Tags {
            ids: changed,
            tag: clean.clone(),
            added: true,
        });
        self.sync_metadata_sidecars(&ids);
        self.refresh_selected_tags();
        self.refresh_album_counts();
        self.show_undo_toast(format!("Added tag \"{}\"", clean), cx);
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

        // Undo must only touch items this edit changes: items that already
        // lacked the tag keep it absent after an undo.
        let result = if let Some(ref state) = self.grid_state {
            state.engine.ids_with_tag(&ids, &clean).and_then(|had| {
                let had: std::collections::HashSet<_> = had.into_iter().collect();
                let changed: Vec<_> = ids.iter().copied().filter(|id| had.contains(id)).collect();
                state
                    .engine
                    .remove_tags_batch(&changed, std::slice::from_ref(&clean))?;
                Ok(changed)
            })
        } else {
            Ok(Vec::new())
        };
        let Some(changed) = self.apply_tag_mutation("Could not remove tag", result, cx) else {
            return;
        };
        if changed.is_empty() {
            // Nothing to write or undo; an undo toast here would undo an
            // older, unrelated action.
            cx.notify();
            return;
        }
        self.push_undo(crate::app::undo::UndoAction::Tags {
            ids: changed,
            tag: clean.clone(),
            added: false,
        });
        self.sync_metadata_sidecars(&ids);
        let message = format!("Removed tag \"{}\"", clean);
        self.set_last_action(message.clone());
        self.refresh_selected_tags();
        self.refresh_album_counts();
        self.show_undo_toast(message, cx);
        cx.notify();
    }

    /// Opens the tag field for the selection, docking the inspector only when
    /// neither the docked nor the floating one is showing.
    pub fn begin_tagging_selection(&mut self, cx: &mut Context<Self>) {
        if !self.inspector_open && self.floating_inspector_window.is_none() {
            self.inspector_open = true;
            self.save_config();
        }
        self.start_tag_input(cx);
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
            self.set_last_action(format!("Added tag \"{}\"", clean));
            self.batch_add_tag(clean, cx);
        }
        cx.notify();
    }

    pub fn cancel_tag_input(&mut self, cx: &mut Context<Self>) {
        self.tag_input_active = false;
        self.tag_input_buffer.clear();
        cx.notify();
    }

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
                let result = if let Some(ref state) = self.grid_state {
                    state.engine.rename_tag(&old_tag, &clean_new).map(|_| ())
                } else {
                    Ok(())
                };
                if self
                    .apply_tag_mutation("Could not rename tag", result, cx)
                    .is_none()
                {
                    return;
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
                let message = format!("Renamed tag to \"{}\"", clean_new);
                self.set_last_action(message.clone());
                self.show_toast(message, cx);
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
        let result = match self.grid_state.as_ref() {
            Some(state) => state.engine.delete_tag_globally(&clean).map(|_| ()),
            None => Ok(()),
        };
        if self
            .apply_tag_mutation("Could not delete tag", result, cx)
            .is_none()
        {
            return;
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
        // The delete succeeded, so the manager must not keep selecting a tag
        // that no longer exists in the catalog.
        self.tag_manager
            .selected
            .retain(|selected| !selected.eq_ignore_ascii_case(&clean));
        if self
            .tag_manager
            .focused
            .as_ref()
            .is_some_and(|focused| focused.eq_ignore_ascii_case(&clean))
        {
            self.tag_manager.focused = None;
        }
        self.refresh_selected_tags();
        self.refresh_album_counts();
        cx.notify();
    }

    pub fn open_tag_manager(&mut self, cx: &mut Context<Self>) {
        self.tag_manager.open = true;
        self.tag_manager.search.clear();
        self.tag_manager.search_focused = true;
        self.tag_manager.selected.clear();
        self.tag_manager.editing = None;
        self.tag_manager.merge_source = None;
        self.tag_manager.focused = None;
        self.tag_manager.list_scroll = gpui::ScrollHandle::new();
        cx.notify();
    }

    pub fn close_tag_manager(&mut self, cx: &mut Context<Self>) {
        self.tag_manager.open = false;
        self.tag_manager.search_focused = false;
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

    /// Selects or clears every visible (filtered) tag for bulk actions.
    pub fn toggle_all_tag_manager_rows(&mut self, cx: &mut Context<Self>) {
        let visible = self.tag_manager_rows();
        if visible
            .iter()
            .all(|(tag, _)| self.tag_manager.selected.contains(tag))
        {
            self.tag_manager.selected.clear();
        } else {
            self.tag_manager
                .selected
                .extend(visible.into_iter().map(|(tag, _)| tag));
        }
        cx.notify();
    }

    /// Shows `tag` in the tag manager's detail panel.
    pub fn focus_tag_manager_tag(&mut self, tag: String, cx: &mut Context<Self>) {
        self.tag_manager.focused = Some(tag);
        cx.notify();
    }

    /// Moves the detail focus up or down the visible list (↑/↓).
    pub fn step_tag_manager_focus(&mut self, delta: isize, cx: &mut Context<Self>) {
        let rows = self.tag_manager_rows();
        if rows.is_empty() {
            return;
        }
        let current = self
            .tag_manager
            .focused
            .as_ref()
            .and_then(|focused| rows.iter().position(|(tag, _)| tag == focused));
        let next = match current {
            Some(i) => (i as isize + delta).clamp(0, rows.len() as isize - 1) as usize,
            None if delta < 0 => rows.len() - 1,
            None => 0,
        };
        self.tag_manager.focused = Some(rows[next].0.clone());
        // The list's children are exactly these rows when it is non-empty.
        self.tag_manager.list_scroll.scroll_to_item(next);
        cx.notify();
    }

    /// Library tags after the manager's filter and sort, as painted in its list.
    pub(crate) fn tag_manager_rows(&self) -> Vec<(String, usize)> {
        let query = self.tag_manager.search.to_lowercase();
        let mut rows: Vec<(String, usize)> = self
            .library_tag_counts
            .iter()
            .filter(|(tag, _)| query.is_empty() || tag.to_lowercase().contains(&query))
            .cloned()
            .collect();
        match self.tag_manager.sort {
            TagSortOrder::Alphabetical => {
                rows.sort_by_key(|row| row.0.to_lowercase());
            }
            TagSortOrder::CountDesc => {
                rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            }
        }
        rows
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
                let result = if let Some(ref state) = self.grid_state {
                    state.engine.rename_tag(&old_tag, &new_tag).map(|_| ())
                } else {
                    Ok(())
                };
                if self
                    .apply_tag_mutation("Could not rename tag", result, cx)
                    .is_none()
                {
                    return;
                }
                let old_query = format!("tag:{}", old_tag);
                let old_query_quoted = format!("tag:\"{}\"", old_tag);
                if self.search_query == old_query || self.search_query == old_query_quoted {
                    self.search_query = format!("tag:{}", new_tag);
                }
                if self.tag_manager.focused.as_deref() == Some(old_tag.as_str()) {
                    self.tag_manager.focused = Some(new_tag.clone());
                }
                self.refresh_selected_tags();
                self.refresh_album_counts();
                self.show_toast(format!("Renamed tag to \"{}\"", new_tag), cx);
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
                let result = match self.grid_state.as_ref() {
                    Some(state) => state
                        .engine
                        .merge_tags(&source_tag, &target_tag)
                        .map(|_| ()),
                    None => Ok(()),
                };
                if self
                    .apply_tag_mutation("Could not merge tags", result, cx)
                    .is_none()
                {
                    return;
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
                if self.tag_manager.focused.as_deref() == Some(source_tag.as_str()) {
                    self.tag_manager.focused = Some(target_tag.clone());
                }
                self.refresh_selected_tags();
                self.refresh_album_counts();
            }
            cx.notify();
        }
    }

    pub fn bulk_delete_tags(&mut self, tags: Vec<String>, cx: &mut Context<Self>) {
        let engine_opt = self.grid_state.as_ref().map(|s| s.engine.clone());
        let mut deleted: Vec<String> = Vec::new();
        let mut failures: Vec<(String, String)> = Vec::new();
        if let Some(engine) = engine_opt {
            for tag in &tags {
                let clean = tag.trim().to_lowercase();
                if clean.is_empty() {
                    continue;
                }
                match engine.delete_tag_globally(&clean) {
                    Ok(_) => deleted.push(clean),
                    Err(err) => {
                        log::warn!("could not delete tag {clean:?}: {err:?}");
                        failures.push((clean, err.to_string()));
                    }
                }
            }
        }

        if !deleted.is_empty() {
            if let AlbumSelection::Tag(ref curr) = self.selection {
                if deleted.iter().any(|tag| tag == &curr.to_lowercase()) {
                    self.selection = AlbumSelection::AllLibrary;
                }
            }
            let mut should_clear = false;
            for clean in &deleted {
                let tag_query = format!("tag:{}", clean);
                let tag_query_quoted = format!("tag:\"{}\"", clean);
                if self.search_query == tag_query || self.search_query == tag_query_quoted {
                    should_clear = true;
                }
            }
            if should_clear {
                self.clear_search(cx);
            }
        }

        // Keep tags that failed selected so the retry starts from the same set;
        // only tags the catalog actually deleted leave the manager selection.
        self.tag_manager
            .selected
            .retain(|selected| !deleted.iter().any(|tag| tag == &selected.to_lowercase()));
        if self
            .tag_manager
            .focused
            .as_ref()
            .is_some_and(|focused| deleted.iter().any(|tag| tag == &focused.to_lowercase()))
        {
            self.tag_manager.focused = None;
        }
        self.refresh_selected_tags();
        self.refresh_album_counts();
        if !failures.is_empty() {
            let details = failures
                .iter()
                .take(3)
                .map(|(tag, err)| format!("{tag}: {err}"))
                .collect::<Vec<_>>()
                .join("; ");
            let suffix = if failures.len() > 3 { "; ..." } else { "" };
            self.set_error(
                format!(
                    "Could not delete {} tag(s): {details}{suffix}",
                    failures.len()
                ),
                cx,
            );
        }
        cx.notify();
    }
}
