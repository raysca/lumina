use gpui::*;
use lumina_core::catalog::keywords::{normalize_path, subtree_suffix};
use lumina_core::text::{clean_text, MAX_NAME_CHARS};

use crate::app::types::TagSortOrder;
use crate::app::LuminaApp;
use crate::i18n::t;
use crate::views::sidebar::AlbumSelection;

impl LuminaApp {
    #[cfg(test)]
    pub fn refresh_tag_counts(&mut self) {
        self.refresh_keyword_aliases();
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
                self.set_error(format!("{action}: {err:#}"), cx);
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
        self.keywords.suggestion_focus = None;
        cx.notify();
    }

    pub fn commit_tag_input(&mut self, cx: &mut Context<Self>) {
        // Enter on a highlighted suggestion adds its stored keyword.
        let suggested = self.focused_tag_suggestion();
        let typed = std::mem::take(&mut self.tag_input_buffer);
        let tag = suggested.unwrap_or(typed);
        self.tag_input_active = false;
        self.keywords.suggestion_focus = None;
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
        let view_before = self.tag_view_refs();
        // The delete succeeded, so nothing may keep pointing at a keyword
        // (or a child of it) that no longer exists in the catalog.
        self.retarget_tag_refs(&clean, None, cx);
        let recorded =
            self.push_tag_rows_undo("tag delete", change, view_before, false, Vec::new());
        self.refresh_selected_tags();
        self.refresh_album_counts();
        // A deleted child changes the focused keyword's count.
        self.load_tag_manager_items();
        if recorded {
            self.show_undo_toast(format!("Deleted tag \"{clean}\""), cx);
        }
        cx.notify();
    }

    /// The Tags page is what the main column shows: selected, with no
    /// search or year over it (those show their matches instead).
    pub(crate) fn tags_page_shown(&self) -> bool {
        self.selection == AlbumSelection::Tags
            && self.search_query.is_empty()
            && self.active_year.is_none()
    }

    /// A page with no media shows (the Albums overview, the Tags page or
    /// the Downloads screen):
    /// no grid, no inspector, and no selection for keys or commands to act
    /// on, so a viewer or a new copy is shown from All Media instead.
    pub(crate) fn non_media_page_shown(&self) -> bool {
        self.albums_overview_shown() || self.tags_page_shown() || self.downloads_screen_shown()
    }

    /// The toolbar Tags button, TAGS › Keywords, ⌘K Open Tags and the empty
    /// state: the Tags page, one Back entry, starting fresh (empty filter,
    /// nothing checked or focused). Not while compare holds the view.
    pub fn open_tag_manager(&mut self, cx: &mut Context<Self>) {
        if self.grid_state.is_none() || self.compare_holds_view() {
            return;
        }
        if self.is_fullscreen_viewer {
            self.close_viewer(cx);
        }
        if !self.tags_page_shown() {
            self.select_album(AlbumSelection::Tags, cx);
            self.tag_manager.search.clear();
            self.tag_manager.search_focused = true;
            self.tag_manager.selected.clear();
            self.tag_manager.close_popovers();
            self.tag_manager.focused = None;
            self.tag_manager.tagged_items.clear();
            self.tag_manager.list_scroll = gpui::UniformListScrollHandle::new();
        }
        cx.notify();
    }

    pub fn set_tag_sort_order(&mut self, order: TagSortOrder, cx: &mut Context<Self>) {
        self.tag_manager.sort = order;
        self.tag_manager.sort_menu = None;
        cx.notify();
    }

    /// Opens the sort menu on the current order, or closes it.
    pub(crate) fn toggle_tag_sort_menu(&mut self, cx: &mut Context<Self>) {
        let sort = self.tag_manager.sort;
        self.tag_manager.sort_menu = match self.tag_manager.sort_menu {
            Some(_) => None,
            None => TagSortOrder::ALL.iter().position(|s| *s == sort),
        };
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
            .all(|row| self.tag_manager.selected.contains(&row.path))
        {
            for row in &visible {
                self.tag_manager.selected.remove(&row.path);
            }
        } else {
            self.tag_manager
                .selected
                .extend(visible.into_iter().map(|row| row.path));
        }
        cx.notify();
    }

    /// Shows `tag` in the tag manager's detail panel.
    pub fn focus_tag_manager_tag(&mut self, tag: String, cx: &mut Context<Self>) {
        // A rename belongs to the tag it started on; moving away cancels it.
        if self.tag_manager.focused.as_ref() != Some(&tag) {
            self.cancel_tag_manager_prompts();
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
            .and_then(|focused| rows.iter().position(|row| &row.path == focused));
        let next = match current {
            Some(i) => (i as isize + delta).clamp(0, rows.len() as isize - 1) as usize,
            None if delta < 0 => rows.len() - 1,
            None => 0,
        };
        if self.tag_manager.focused.as_ref() != Some(&rows[next].path) {
            self.cancel_tag_manager_prompts();
        }
        self.tag_manager.focused = Some(rows[next].path.clone());
        self.load_tag_manager_items();
        self.tag_manager
            .list_scroll
            .scroll_to_item(next, gpui::ScrollStrategy::Nearest);
        cx.notify();
    }

    /// Drops a rename, move or alias in progress (focus moved away, or the
    /// page was left: a prompt never outlives the visit).
    pub(crate) fn cancel_tag_manager_prompts(&mut self) {
        self.tag_manager.editing = None;
        self.tag_manager.editing_buffer.clear();
        self.tag_manager.moving = false;
        self.tag_manager.alias_editing = false;
        self.tag_manager.alias_buffer.clear();
        self.tag_manager.row_menu = None;
        self.tag_manager.merge = None;
    }

    pub fn start_modal_tag_rename(&mut self, tag: String, cx: &mut Context<Self>) {
        self.tag_manager.merge = None;
        self.tag_manager.moving = false;
        self.tag_manager.alias_editing = false;
        self.tag_manager.editing = Some(tag.clone());
        self.tag_manager.editing_buffer = tag;
        cx.notify();
    }

    pub fn commit_modal_tag_rename(&mut self, cx: &mut Context<Self>) {
        if let Some(old_tag) = self.tag_manager.editing.take() {
            let buffer = std::mem::take(&mut self.tag_manager.editing_buffer);
            let target = self.modal_rename_target(&old_tag, &buffer);
            let moved = std::mem::take(&mut self.tag_manager.moving);
            let Some(new_tag) = self.renamed_path(&old_tag, &target, cx) else {
                cx.notify();
                return;
            };
            if new_tag != old_tag.to_lowercase() {
                let keeps_positions = self.keyword_is_unused(&new_tag);
                let result = match self.grid_state.as_ref() {
                    Some(state) => state.engine.rename_tag(&old_tag, &new_tag),
                    None => Ok(Default::default()),
                };
                let Some(change) = self.apply_tag_mutation("Could not rename tag", result, cx)
                else {
                    return;
                };
                self.sync_metadata_sidecars(&change.affected_ids(), cx);
                let view_before = self.tag_view_refs();
                self.retarget_tag_refs(&old_tag, Some(&new_tag), cx);
                let label = if moved { "keyword move" } else { "tag rename" };
                let recorded = self.push_tag_rows_undo(
                    label,
                    change,
                    view_before,
                    keeps_positions,
                    vec![(old_tag.to_lowercase(), new_tag.clone())],
                );
                self.refresh_selected_tags();
                self.refresh_album_counts();
                // A move onto an existing keyword merges, so recount.
                self.load_tag_manager_items();
                let message = if moved {
                    t("tags.moved_to")
                } else {
                    t("tags.renamed_to")
                };
                self.tag_rows_toast(recorded, message.replace("{path}", &new_tag), cx);
            }
            cx.notify();
        }
    }

    pub fn cancel_modal_tag_rename(&mut self, cx: &mut Context<Self>) {
        self.tag_manager.moving = false;
        self.tag_manager.editing = None;
        self.tag_manager.editing_buffer.clear();
        cx.notify();
    }

    /// Loads the focused tag's newest items for the detail PREVIEW (2 × 4).
    pub(crate) fn load_tag_manager_items(&mut self) {
        self.load_focused_keyword_count();
        self.tag_manager.tagged_items = match (self.tag_manager.focused.as_ref(), &self.grid_state)
        {
            (Some(tag), Some(state)) => match state
                .engine
                .items_with_tag(tag, crate::views::tag_manager::detail::PREVIEW_ITEMS)
            {
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
        let view_before = self.tag_view_refs();
        for tag in &deleted {
            self.retarget_tag_refs(tag, None, cx);
        }
        // One transaction, so one undo entry for all N tags.
        let recorded =
            self.push_tag_rows_undo("tag delete", change, view_before, false, Vec::new());
        self.refresh_selected_tags();
        self.refresh_album_counts();
        // A deleted child changes the focused keyword's count.
        self.load_tag_manager_items();
        if recorded {
            let message = match deleted.len() {
                1 => format!("Deleted tag \"{}\"", deleted[0]),
                n => format!("Deleted {n} tags"),
            };
            self.show_undo_toast(message, cx);
        }
        cx.notify();
    }
}
