use gpui::*;
use lumina_core::catalog::keywords::{normalize_path, subtree_suffix};
use lumina_core::text::{clean_text, MAX_NAME_CHARS};

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

    /// Points everything that names `old` or a keyword below it (sidebar
    /// selection, a `tag:` search, the tag manager's focus and checks) at
    /// the same place under `new` after a rename or merge, or drops it after
    /// a delete (`new` is `None`).
    pub(crate) fn retarget_tag_refs(
        &mut self,
        old: &str,
        new: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        let old = old.trim().to_lowercase();
        // None: untouched; Some(None): deleted; Some(Some(path)): moved.
        let map = |path: &str| {
            subtree_suffix(&path.to_lowercase(), &old)
                .map(|rest| new.map(|new| format!("{new}{rest}")))
        };
        if let AlbumSelection::Tag(ref curr) = self.selection {
            match map(curr) {
                Some(Some(path)) => self.selection = AlbumSelection::Tag(path),
                Some(None) => self.selection = AlbumSelection::AllLibrary,
                None => {}
            }
        }
        let searched = self
            .search_query
            .strip_prefix("tag:")
            .map(|value| map(value.trim_matches('"')));
        match searched {
            Some(Some(Some(path))) if path.contains(' ') => {
                self.search_query = format!("tag:\"{path}\"")
            }
            Some(Some(Some(path))) => self.search_query = format!("tag:{path}"),
            Some(Some(None)) => self.clear_search(cx),
            _ => {}
        }
        if let Some(moved) = self.tag_manager.focused.as_deref().and_then(map) {
            if moved.is_none() {
                self.tag_manager.editing = None;
            }
            self.tag_manager.focused = moved;
        }
        self.tag_manager.selected = std::mem::take(&mut self.tag_manager.selected)
            .into_iter()
            .filter_map(|tag| map(&tag).unwrap_or(Some(tag)))
            .collect();
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
        // The stored path (an alias resolved to its keyword), so undo and
        // the "already had it" check see the same row the catalog writes. An
        // empty or over-long path is refused with an error, never cut.
        let resolved = match self.grid_state.as_ref() {
            Some(state) => state.engine.resolve_alias(&tag),
            None => normalize_path(&tag),
        };
        let Some(clean) = self.apply_tag_mutation("Could not add tag", resolved, cx) else {
            return;
        };
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
        self.sync_metadata_sidecars(&ids, cx);
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
        self.sync_metadata_sidecars(&ids, cx);
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
        // The floating inspector draws one item's tag field on its Tags tab.
        if self.floating_inspector_window.is_some() && self.selected_indices.len() <= 1 {
            self.inspector_tab = crate::views::inspector::InspectorTab::Tags;
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

    /// The path a rename buffer stores, or `None` when there is nothing to
    /// rename: the buffer is untouched or blank, or [`normalize_path`]
    /// refused it (then the error banner says why).
    fn renamed_path(
        &mut self,
        old_tag: &str,
        buffer: &str,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        if buffer == old_tag || clean_text(buffer, MAX_NAME_CHARS).is_empty() {
            return None;
        }
        self.apply_tag_mutation("Could not rename tag", normalize_path(buffer), cx)
    }

    pub fn start_tag_rename(&mut self, tag: String, cx: &mut Context<Self>) {
        self.editing_tag = Some(tag.clone());
        self.editing_tag_buffer = tag;
        cx.notify();
    }

    pub fn commit_tag_rename(&mut self, cx: &mut Context<Self>) {
        if let Some(old_tag) = self.editing_tag.take() {
            let new_tag = std::mem::take(&mut self.editing_tag_buffer);
            // An untouched or emptied buffer never renames, even if cleaning
            // would change it; an over-long path is refused with an error.
            let Some(clean_new) = self.renamed_path(&old_tag, &new_tag, cx) else {
                cx.notify();
                return;
            };
            if clean_new != old_tag {
                let result = match self.grid_state.as_ref() {
                    Some(state) => state.engine.rename_tag(&old_tag, &clean_new),
                    None => Ok(Default::default()),
                };
                let Some(change) = self.apply_tag_mutation("Could not rename tag", result, cx)
                else {
                    return;
                };
                self.sync_metadata_sidecars(&change.affected_ids(), cx);
                self.retarget_tag_refs(&old_tag, Some(&clean_new), cx);
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
            Some(state) => state.engine.delete_tag_globally(&clean),
            None => Ok(Default::default()),
        };
        let Some(change) = self.apply_tag_mutation("Could not delete tag", result, cx) else {
            return;
        };
        self.sync_metadata_sidecars(&change.affected_ids(), cx);
        // The delete succeeded, so nothing may keep pointing at a keyword
        // (or a child of it) that no longer exists in the catalog.
        self.retarget_tag_refs(&clean, None, cx);
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
        self.tag_manager.focused = None;
        self.tag_manager.tagged_items.clear();
        self.tag_manager.list_scroll = gpui::ScrollHandle::new();
        cx.notify();
    }

    pub fn close_tag_manager(&mut self, cx: &mut Context<Self>) {
        self.tag_manager.open = false;
        self.tag_manager.search_focused = false;
        self.tag_manager.editing = None;
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

    /// Selects or clears every visible (filtered) tag for bulk actions;
    /// checked tags hidden by the filter are left alone.
    pub fn toggle_all_tag_manager_rows(&mut self, cx: &mut Context<Self>) {
        let visible = self.tag_manager_rows();
        if visible
            .iter()
            .all(|(tag, _)| self.tag_manager.selected.contains(tag))
        {
            for (tag, _) in &visible {
                self.tag_manager.selected.remove(tag);
            }
        } else {
            self.tag_manager
                .selected
                .extend(visible.into_iter().map(|(tag, _)| tag));
        }
        cx.notify();
    }

    /// Shows `tag` in the tag manager's detail panel.
    pub fn focus_tag_manager_tag(&mut self, tag: String, cx: &mut Context<Self>) {
        // A rename belongs to the tag it started on; moving away cancels it.
        if self.tag_manager.focused.as_ref() != Some(&tag) {
            self.tag_manager.editing = None;
            self.tag_manager.editing_buffer.clear();
        }
        self.tag_manager.focused = Some(tag);
        self.load_tag_manager_items();
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
        self.load_tag_manager_items();
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
            let buffer = std::mem::take(&mut self.tag_manager.editing_buffer);
            let Some(new_tag) = self.renamed_path(&old_tag, &buffer, cx) else {
                cx.notify();
                return;
            };
            if new_tag != old_tag.to_lowercase() {
                let result = match self.grid_state.as_ref() {
                    Some(state) => state.engine.rename_tag(&old_tag, &new_tag),
                    None => Ok(Default::default()),
                };
                let Some(change) = self.apply_tag_mutation("Could not rename tag", result, cx)
                else {
                    return;
                };
                self.sync_metadata_sidecars(&change.affected_ids(), cx);
                self.retarget_tag_refs(&old_tag, Some(&new_tag), cx);
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

    /// The most-used checked tag (ties: A–Z): Merge folds the others into it.
    pub(crate) fn tag_merge_target(&self) -> Option<String> {
        self.library_tag_counts
            .iter()
            .filter(|(tag, _)| self.tag_manager.selected.contains(tag))
            .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
            .map(|(tag, _)| tag.clone())
    }

    /// Merges every checked tag into [`Self::tag_merge_target`] (v3 Merge).
    pub fn merge_checked_tags(&mut self, cx: &mut Context<Self>) {
        if self.tag_manager.selected.len() < 2 {
            return;
        }
        let Some(target) = self.tag_merge_target() else {
            return;
        };
        let mut sources: Vec<String> = self
            .tag_manager
            .selected
            .iter()
            .filter(|tag| **tag != target)
            .cloned()
            .collect();
        sources.sort();
        let mut merged = 0;
        let mut affected = Vec::new();
        for source in &sources {
            let result = match self.grid_state.as_ref() {
                Some(state) => state.engine.merge_tags(source, &target),
                None => Ok(Default::default()),
            };
            // Stop at the first failure; the merges before it are committed
            // and the counts and sidecars below reflect them.
            let Some(change) = self.apply_tag_mutation("Could not merge tags", result, cx) else {
                break;
            };
            affected.extend(change.affected_ids());
            merged += 1;
            self.retarget_tag_refs(source, Some(&target), cx);
        }
        if merged == sources.len() {
            self.tag_manager.selected.clear();
        }
        // sync_metadata_sidecars rewrites each id from the catalog, so an id
        // listed twice costs one extra write, never a wrong sidecar.
        self.sync_metadata_sidecars(&affected, cx);
        self.refresh_selected_tags();
        self.refresh_album_counts();
        self.focus_tag_manager_tag(target.clone(), cx);
        if merged > 0 {
            self.show_toast(format!("Merged {merged} tag(s) into \"{target}\""), cx);
        }
        cx.notify();
    }

    /// Loads the focused tag's items for the detail grid (6 columns × 4 rows).
    pub(crate) fn load_tag_manager_items(&mut self) {
        self.tag_manager.tagged_items = match (self.tag_manager.focused.as_ref(), &self.grid_state)
        {
            (Some(tag), Some(state)) => match state.engine.items_with_tag(tag, 24) {
                Ok(items) => items.into_iter().map(|item| item.id).collect(),
                Err(err) => {
                    log::warn!("could not load items for tag {tag:?}: {err:#}");
                    Vec::new()
                }
            },
            _ => Vec::new(),
        };
    }

    /// Deletes every tag in `tags` (and their subtrees) in one transaction:
    /// all of them or, on an error, none.
    pub fn bulk_delete_tags(&mut self, tags: Vec<String>, cx: &mut Context<Self>) {
        let deleted: Vec<String> = tags
            .iter()
            .map(|tag| tag.trim().to_lowercase())
            .filter(|tag| !tag.is_empty())
            .collect();
        let result = match self.grid_state.as_ref() {
            Some(state) => state.engine.delete_tags_globally(&deleted),
            None => Ok(Default::default()),
        };
        // On failure nothing was deleted, so the selection stays for a retry.
        let Some(change) = self.apply_tag_mutation("Could not delete tags", result, cx) else {
            cx.notify();
            return;
        };
        self.sync_metadata_sidecars(&change.affected_ids(), cx);

        for tag in &deleted {
            self.retarget_tag_refs(tag, None, cx);
        }
        self.refresh_selected_tags();
        self.refresh_album_counts();
        cx.notify();
    }
}
