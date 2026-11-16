use gpui::*;
use lumina_core::text::{clean_text, MAX_NAME_CHARS};
use lumina_core::{MatchMode, RuleField, RuleOperator, RuleValue, SmartAlbum, SmartAlbumRule};

use crate::app::smart_draft::{rules_from_rows, DraftFocus, DraftRow};
use crate::app::undo::UndoAction;
use crate::app::LuminaApp;
use crate::views::sidebar::AlbumSelection;

/// Thumbnails in the builder's preview strip.
const PREVIEW_LIMIT: usize = 8;

impl LuminaApp {
    /// New Smart Album… (⌥⌘N): an empty sheet with one "Favorite is Yes" row.
    pub fn open_album_builder(&mut self, cx: &mut Context<Self>) {
        self.open_draft(None, String::new(), MatchMode::All, &[], cx);
    }

    /// Edit Smart Album…: the sheet prefilled with a saved album.
    pub fn open_album_builder_for_edit(&mut self, album: SmartAlbum, cx: &mut Context<Self>) {
        let pinned = album.pinned;
        self.open_draft(
            Some(album.id),
            album.title,
            album.match_mode,
            &album.rules,
            cx,
        );
        self.album_draft.pin = pinned;
        self.album_draft.pinned_before = pinned;
    }

    /// Edits the smart album shown in the grid, or the focused card on the
    /// Albums overview; a toast elsewhere.
    pub fn edit_selected_smart_album(&mut self, cx: &mut Context<Self>) {
        let album = self
            .palette_smart_album()
            .and_then(|id| self.smart_album(id).cloned());
        match album {
            Some(album) => self.open_album_builder_for_edit(album, cx),
            None => self.show_toast("Open a smart album to edit it", cx),
        }
    }

    /// The keyboard path to the sidebar ×: asks before deleting the open
    /// smart album, or the focused card on the Albums overview.
    pub fn delete_selected_smart_album(&mut self, cx: &mut Context<Self>) {
        match self.palette_smart_album() {
            Some(id) => self.prompt_delete_smart_album(id, cx),
            None => self.show_toast("Open a smart album to delete it", cx),
        }
    }

    /// Save Search as Smart Album…: the builder prefilled with one rule that
    /// holds the search text as typed, titled with the query. The sheet's OK
    /// saves it; compile errors show in its footer.
    pub fn save_current_search_as_album(&mut self, cx: &mut Context<Self>) {
        let query = self.search_query.trim().to_string();
        if query.is_empty() {
            self.show_toast("Type a search to save it", cx);
            return;
        }
        let rule = SmartAlbumRule {
            field: RuleField::Query,
            operator: RuleOperator::Equals,
            value: RuleValue::Text(query.clone()),
        };
        let title = saved_search_title(&query);
        self.open_draft(None, title, MatchMode::All, &[rule], cx);
    }

    /// Save as Smart Album (Tags page ORGANISE row, row `…` menu, ⌘K "Save
    /// Tag as Smart Album"): the builder prefilled with one Tag-is-`<path>`
    /// rule (which covers the keywords below it), titled with the last
    /// segment. The sheet takes the keys, so the page's prompts and menus
    /// close first.
    pub fn save_command_keyword_as_album(&mut self, cx: &mut Context<Self>) {
        let Some(tag) = self.command_keyword() else {
            return;
        };
        self.cancel_tag_manager_prompts();
        self.tag_manager.close_popovers();
        let title = tag.rsplit('/').next().unwrap_or(&tag).to_string();
        let rule = SmartAlbumRule {
            field: RuleField::Tag,
            operator: RuleOperator::Equals,
            value: RuleValue::Text(tag),
        };
        self.open_draft(None, title, MatchMode::All, &[rule], cx);
    }

    fn open_draft(
        &mut self,
        editing: Option<i64>,
        title: String,
        match_mode: MatchMode,
        rules: &[SmartAlbumRule],
        cx: &mut Context<Self>,
    ) {
        // Settings covers the window: the sheet would open unseen under it.
        if self.preferences_open {
            return;
        }
        let rows: Vec<DraftRow> = rules.iter().map(DraftRow::from_rule).collect();
        let cameras = self.grid_state.as_ref().map_or_else(Vec::new, |state| {
            state.engine.distinct_cameras().unwrap_or_else(|err| {
                log::warn!("the library's cameras could not be listed: {err:#}");
                Vec::new()
            })
        });
        let draft = &mut self.album_draft;
        draft.cameras = cameras;
        draft.modal_open = true;
        draft.editing_album_id = editing;
        draft.album_title = title;
        draft.match_mode = match_mode;
        // Only a new album starts with a row; a saved zero-rule album must
        // reopen empty, or OK would narrow "every item" to favourites.
        draft.rows = if rows.is_empty() && editing.is_none() {
            vec![DraftRow::new(RuleField::IsFavorite)]
        } else {
            rows
        };
        draft.focus_on(DraftFocus::Title);
        draft.popup = None;
        draft.pin = false;
        draft.pinned_before = false;
        // A reopened builder starts at the first condition.
        draft.scroll = gpui::ScrollHandle::new();
        self.refresh_draft_preview();
        cx.notify();
    }

    pub fn close_album_builder(&mut self, cx: &mut Context<Self>) {
        self.album_draft.modal_open = false;
        self.album_draft.editing_album_id = None;
        self.album_draft.popup = None;
        self.album_draft.preview.clear();
        cx.notify();
    }

    /// Asks before deleting a smart album; see [`Self::confirm_delete_smart_album`].
    pub fn prompt_delete_smart_album(&mut self, album_id: i64, cx: &mut Context<Self>) {
        self.album_draft.confirm_delete = Some(album_id);
        cx.notify();
    }

    pub fn cancel_delete_smart_album(&mut self, cx: &mut Context<Self>) {
        self.album_draft.confirm_delete = None;
        cx.notify();
    }

    /// Deletes the album the open confirmation asks about.
    pub fn confirm_delete_smart_album(&mut self, cx: &mut Context<Self>) {
        if let Some(album_id) = self.album_draft.confirm_delete.take() {
            self.delete_smart_album(album_id, cx);
        }
    }

    /// Deletes a user smart album (metadata only; no media is touched). The
    /// toast offers Undo (⌘Z restores it).
    pub fn delete_smart_album(&mut self, album_id: i64, cx: &mut Context<Self>) {
        let deleted = self
            .user_smart_albums
            .iter()
            .find(|a| a.id == album_id)
            .cloned();
        let result = if let Some(ref state) = self.grid_state {
            state.engine.delete_smart_album(album_id)
        } else {
            Ok(false)
        };
        match result {
            Ok(true) => {
                if self.selection == AlbumSelection::User(album_id) {
                    self.selection = AlbumSelection::AllLibrary;
                    self.clear_selection(cx);
                }
                self.refresh_album_counts();
                self.set_last_action("Deleted smart album");
                if let Some(album) = deleted {
                    self.push_undo(UndoAction::SmartAlbumDeleted { album });
                    self.show_undo_toast("Deleted smart album", cx);
                } else {
                    self.show_toast("Deleted smart album", cx);
                }
                cx.notify();
            }
            Ok(false) => self.set_error("Smart album not found", cx),
            Err(err) => self.set_error(format!("Could not delete smart album: {err:#}"), cx),
        }
    }

    /// Every row as a core rule, or `None` while any row is invalid.
    pub fn draft_rules(&self) -> Option<Vec<SmartAlbumRule>> {
        rules_from_rows(&self.album_draft.rows)
    }

    /// Re-runs the count and preview queries. Called after a rule changes,
    /// never from render.
    // ponytail: on the UI thread. Count plus the first 8 cost at most ~5.6 ms
    // per edit on a 25k-item catalog (release build, measured for plan 16
    // Decision 8); move it to a background task if catalogs grow much larger.
    pub fn refresh_draft_preview(&mut self) {
        let draft = &mut self.album_draft;
        draft.match_count = None;
        draft.preview.clear();
        draft.problem = None;
        let (Some(rules), Some(state)) = (rules_from_rows(&draft.rows), self.grid_state.as_ref())
        else {
            return;
        };
        let album = SmartAlbum::new(String::new(), String::new(), draft.match_mode, rules);
        // The engine's query helpers read errors as "no matches"; compile
        // first so a rule the engine rejects is reported, not shown as 0.
        if let Err(err) = album.build_sql() {
            draft.problem = Some(err.to_string());
            return;
        }
        draft.match_count = Some(state.engine.query_smart_album_count(&album));
        draft.preview = state.engine.query_smart_album(&album, PREVIEW_LIMIT, 0);
    }

    /// OK: saves when every row is valid, then shows the album.
    pub fn save_draft_smart_album(&mut self, cx: &mut Context<Self>) {
        let Some(rules) = self.draft_rules() else {
            return;
        };
        if self.album_draft.problem.is_some() {
            return;
        }
        let title = match clean_text(&self.album_draft.album_title, MAX_NAME_CHARS).as_str() {
            "" => "Untitled Smart Album".to_string(),
            title => title.to_string(),
        };
        let editing_id = self.album_draft.editing_album_id;
        let mode = self.album_draft.match_mode;
        let Some(state) = self.grid_state.as_ref() else {
            return;
        };
        let result = match editing_id {
            Some(id) => state
                .engine
                .update_smart_album(id, &title, "folder", mode, &rules)
                .map(|_| id),
            None => state
                .engine
                .create_smart_album(&title, "folder", mode, &rules)
                .map(|album| album.id),
        };

        match result {
            Ok(album_id) => {
                let pin = self.album_draft.pin;
                let pin_changed = pin != self.album_draft.pinned_before;
                self.close_album_builder(cx);
                // Pinned after the save: a failed pin is in the error banner
                // and the album stays.
                if pin_changed {
                    let action = if pin {
                        "album.pin_failed"
                    } else {
                        "album.unpin_failed"
                    };
                    self.album_write(action, |e| e.set_smart_album_pinned(album_id, pin), cx);
                }
                self.refresh_album_counts();
                self.select_album(AlbumSelection::User(album_id), cx);
                let message = if editing_id.is_some() {
                    format!("Updated smart album \"{}\"", title)
                } else {
                    format!("Created smart album \"{}\"", title)
                };
                self.set_last_action(message.clone());
                self.show_toast(message, cx);
            }
            Err(err) => self.set_error(format!("Could not save smart album: {err:#}"), cx),
        }
    }
}

/// Longest default title for a saved search, in characters.
const SAVED_SEARCH_TITLE_CHARS: usize = 60;

/// The default title for a saved search: the query, trimmed and cut to at
/// most 60 characters (never inside a character).
pub(crate) fn saved_search_title(query: &str) -> String {
    query
        .trim()
        .chars()
        .take(SAVED_SEARCH_TITLE_CHARS)
        .collect()
}
