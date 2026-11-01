use gpui::*;
use lumina_core::text::{clean_text, MAX_NAME_CHARS};
use lumina_core::{MatchMode, RuleField, SmartAlbum, SmartAlbumRule};

use crate::app::smart_draft::field_label;
use crate::app::smart_draft::{
    focus_order, operator_label, operators, rules_from_rows, DraftFocus, DraftPopup, DraftRow,
    ValueKind, FIELDS, UNITS,
};
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
        self.open_draft(
            Some(album.id),
            album.title,
            album.match_mode,
            &album.rules,
            cx,
        );
    }

    /// Edits the smart album shown in the grid; a no-op elsewhere.
    pub fn edit_selected_smart_album(&mut self, cx: &mut Context<Self>) {
        let album = match self.selection {
            AlbumSelection::User(id) => self.user_smart_albums.iter().find(|a| a.id == id).cloned(),
            _ => None,
        };
        match album {
            Some(album) => self.open_album_builder_for_edit(album, cx),
            None => self.show_toast("Open a smart album to edit it", cx),
        }
    }

    fn open_draft(
        &mut self,
        editing: Option<i64>,
        title: String,
        match_mode: MatchMode,
        rules: &[SmartAlbumRule],
        cx: &mut Context<Self>,
    ) {
        let rows: Vec<DraftRow> = rules.iter().map(DraftRow::from_rule).collect();
        let draft = &mut self.album_draft;
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
        draft.focus = DraftFocus::Title;
        draft.popup = None;
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
            Err(err) => self.set_error(format!("Could not delete smart album: {err}"), cx),
        }
    }

    /// Every row as a core rule, or `None` while any row is invalid.
    pub fn draft_rules(&self) -> Option<Vec<SmartAlbumRule>> {
        rules_from_rows(&self.album_draft.rows)
    }

    /// Re-runs the count and preview queries. Called after a rule changes,
    /// never from render.
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

    pub fn set_draft_focus(&mut self, focus: DraftFocus, cx: &mut Context<Self>) {
        self.album_draft.focus = focus;
        self.album_draft.popup = None;
        cx.notify();
    }

    /// Labels for the popup that `target` opens; empty when it has none.
    pub fn draft_popup_options(&self, target: DraftFocus) -> Vec<String> {
        let rows = &self.album_draft.rows;
        let to_strings = |labels: &[&str]| labels.iter().map(|s| s.to_string()).collect();
        match target {
            DraftFocus::Match => to_strings(&["all", "any"]),
            DraftFocus::Field(_) => FIELDS.iter().map(|f| field_label(f).to_string()).collect(),
            DraftFocus::Operator(i) => rows.get(i).map_or_else(Vec::new, |row| {
                operators(&row.field)
                    .iter()
                    .map(|op| operator_label(&row.field, op).to_string())
                    .collect()
            }),
            DraftFocus::Value(i) => match rows.get(i).map(DraftRow::kind) {
                Some(ValueKind::Choice(options)) => {
                    options.iter().map(|(_, label)| label.to_string()).collect()
                }
                Some(ValueKind::Tags) => self
                    .library_tag_counts
                    .iter()
                    .map(|(tag, _)| tag.clone())
                    .collect(),
                _ => Vec::new(),
            },
            DraftFocus::Unit(_) => UNITS.iter().map(|(_, l)| l.to_string()).collect(),
            _ => Vec::new(),
        }
    }

    /// Index of the current value in `target`'s popup, for the highlight.
    fn draft_popup_current(&self, target: DraftFocus) -> usize {
        let rows = &self.album_draft.rows;
        let position = match target {
            DraftFocus::Match => Some((self.album_draft.match_mode == MatchMode::Any) as usize),
            DraftFocus::Field(i) => rows
                .get(i)
                .and_then(|row| FIELDS.iter().position(|f| *f == row.field)),
            DraftFocus::Operator(i) => rows.get(i).and_then(|row| {
                operators(&row.field)
                    .iter()
                    .position(|o| *o == row.operator)
            }),
            DraftFocus::Value(i) => rows.get(i).and_then(|row| match row.kind() {
                ValueKind::Choice(options) => options.iter().position(|(k, _)| *k == row.text),
                _ => self
                    .library_tag_counts
                    .iter()
                    .position(|(tag, _)| *tag == row.text),
            }),
            DraftFocus::Unit(i) => rows
                .get(i)
                .and_then(|row| UNITS.iter().position(|(u, _)| *u == row.unit)),
            _ => None,
        };
        position.unwrap_or(0)
    }

    /// Opens `target`'s popup, or closes it when it is already open.
    pub fn toggle_draft_popup(&mut self, target: DraftFocus, cx: &mut Context<Self>) {
        self.album_draft.focus = target;
        self.album_draft.popup = if self.album_draft.popup.map(|p| p.target) == Some(target) {
            None
        } else {
            Some(DraftPopup {
                target,
                highlight: self.draft_popup_current(target),
            })
        };
        cx.notify();
    }

    pub fn move_draft_popup_highlight(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some(popup) = self.album_draft.popup else {
            return;
        };
        let count = self.draft_popup_options(popup.target).len();
        if count > 0 {
            let next = (popup.highlight as isize + delta).rem_euclid(count as isize) as usize;
            self.album_draft.popup = Some(DraftPopup {
                highlight: next,
                ..popup
            });
            cx.notify();
        }
    }

    /// Applies option `index` of the open popup and closes it.
    pub fn pick_draft_option(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(popup) = self.album_draft.popup.take() else {
            return;
        };
        let tags = &self.library_tag_counts;
        let draft = &mut self.album_draft;
        match popup.target {
            DraftFocus::Match => {
                draft.match_mode = if index == 1 {
                    MatchMode::Any
                } else {
                    MatchMode::All
                };
            }
            DraftFocus::Field(i) => {
                if let (Some(row), Some(field)) = (draft.rows.get_mut(i), FIELDS.get(index)) {
                    if row.field != *field {
                        *row = DraftRow::new(field.clone());
                    }
                }
            }
            DraftFocus::Operator(i) => {
                if let Some(row) = draft.rows.get_mut(i) {
                    if let Some(op) = operators(&row.field).get(index) {
                        row.set_operator(op.clone());
                    }
                }
            }
            DraftFocus::Value(i) => {
                if let Some(row) = draft.rows.get_mut(i) {
                    let key = match row.kind() {
                        ValueKind::Choice(options) => options.get(index).map(|o| o.0.to_string()),
                        _ => tags.get(index).map(|(tag, _)| tag.clone()),
                    };
                    if let Some(key) = key {
                        row.text = key;
                    }
                }
            }
            DraftFocus::Unit(i) => {
                if let (Some(row), Some((unit, _))) = (draft.rows.get_mut(i), UNITS.get(index)) {
                    row.unit = *unit;
                }
            }
            _ => {}
        }
        self.refresh_draft_preview();
        cx.notify();
    }

    /// Tab / Shift-Tab.
    pub fn move_draft_focus(&mut self, delta: isize, cx: &mut Context<Self>) {
        let order = focus_order(&self.album_draft.rows);
        let current = order
            .iter()
            .position(|f| *f == self.album_draft.focus)
            .unwrap_or(0);
        let next = (current as isize + delta).rem_euclid(order.len() as isize) as usize;
        self.set_draft_focus(order[next], cx);
    }

    /// Whether the focused control takes typed text.
    pub fn draft_focus_is_typed(&self) -> bool {
        match self.album_draft.focus {
            DraftFocus::Title => true,
            DraftFocus::Value(i) => self
                .album_draft
                .rows
                .get(i)
                .is_some_and(|row| row.kind().is_typed()),
            _ => false,
        }
    }

    /// Enter / Space on a focused control that is not a text field.
    pub fn activate_draft_focus(&mut self, cx: &mut Context<Self>) {
        match self.album_draft.focus {
            DraftFocus::Remove(i) => self.remove_draft_row(i, cx),
            DraftFocus::Add(i) => self.add_draft_row_after(i, cx),
            DraftFocus::Cancel => self.close_album_builder(cx),
            DraftFocus::Save => self.save_draft_smart_album(cx),
            DraftFocus::Title => {}
            target => {
                if !self.draft_popup_options(target).is_empty() {
                    self.toggle_draft_popup(target, cx);
                }
            }
        }
    }

    /// Deletes the last character of the focused text field. Typing arrives
    /// through the input handler (app/text_input.rs).
    pub fn backspace_draft_text(&mut self, cx: &mut Context<Self>) {
        let focus = self.album_draft.focus;
        let target = match focus {
            DraftFocus::Title => &mut self.album_draft.album_title,
            DraftFocus::Value(i) => match self.album_draft.rows.get_mut(i) {
                Some(row) => &mut row.text,
                None => return,
            },
            _ => return,
        };
        target.pop();
        if focus != DraftFocus::Title {
            self.refresh_draft_preview();
        }
        cx.notify();
    }

    /// With no rows, `Add(0)` is the empty state's + and inserts at 0.
    pub fn add_draft_row_after(&mut self, index: usize, cx: &mut Context<Self>) {
        let at = (index + 1).min(self.album_draft.rows.len());
        self.album_draft
            .rows
            .insert(at, DraftRow::new(RuleField::IsFavorite));
        self.album_draft.focus = DraftFocus::Field(at);
        self.album_draft.popup = None;
        self.refresh_draft_preview();
        cx.notify();
    }

    /// Removes a row. With none left every item matches; focus moves to the
    /// empty state's Add button.
    pub fn remove_draft_row(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.album_draft.rows.len() {
            self.album_draft.rows.remove(index);
            self.album_draft.focus = match self.album_draft.rows.len() {
                0 => DraftFocus::Add(0),
                n => DraftFocus::Field(index.min(n - 1)),
            };
            self.album_draft.popup = None;
            self.refresh_draft_preview();
            cx.notify();
        }
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
                self.close_album_builder(cx);
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
            Err(err) => self.set_error(format!("Could not save smart album: {err}"), cx),
        }
    }
}
