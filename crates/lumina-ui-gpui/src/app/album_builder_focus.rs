//! Smart album builder: keyboard focus, option popups, the rating stars and
//! adding or removing condition rows.

use gpui::*;
use lumina_core::{MatchMode, RuleField};

use crate::app::smart_draft::{
    field_label, focus_order, operator_label, operators, DraftFocus, DraftPopup, DraftRow,
    ValueKind, FIELDS, UNITS,
};
use crate::app::LuminaApp;

impl LuminaApp {
    pub fn set_draft_focus(&mut self, focus: DraftFocus, cx: &mut Context<Self>) {
        self.album_draft.focus_on(focus);
        self.album_draft.popup = None;
        self.reveal_draft_focus();
        cx.notify();
    }

    /// Scrolls the focused control's row into the conditions list's view.
    /// Rows are the list's children in order, so the row index is the child index.
    fn reveal_draft_focus(&self) {
        if let Some(row) = self.album_draft.focus.row() {
            self.album_draft.scroll.scroll_to_item(row);
        }
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
            // Rating has no popup: its stars and ←/→ set it.
            DraftFocus::Value(i) if rows.get(i).is_some_and(|r| r.field == RuleField::Rating) => {
                Vec::new()
            }
            DraftFocus::Value(i) => match rows.get(i).map(DraftRow::kind) {
                Some(ValueKind::Choice(options)) => {
                    options.iter().map(|(_, label)| label.to_string()).collect()
                }
                Some(ValueKind::Tags) => self
                    .library_tag_counts
                    .iter()
                    .map(|(tag, _)| tag.clone())
                    .collect(),
                // The tags not chosen yet; picking one adds a chip.
                Some(ValueKind::TagList) => self
                    .library_tag_counts
                    .iter()
                    .map(|(tag, _)| tag.clone())
                    .filter(|tag| !rows[i].list.contains(tag))
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
        self.album_draft.focus_on(target);
        self.album_draft.popup = if self.album_draft.popup.map(|p| p.target) == Some(target) {
            None
        } else {
            Some(DraftPopup {
                target,
                highlight: self.draft_popup_current(target),
            })
        };
        // A popup on the bottom row opens below the fold otherwise.
        self.reveal_draft_focus();
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
        let picked = self.draft_popup_options(popup.target).get(index).cloned();
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
                        ValueKind::TagList => {
                            row.list.extend(picked);
                            None
                        }
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

    /// The chip's ×: drops `tag` from row `i`'s "include any of" list.
    pub fn remove_draft_token(&mut self, i: usize, tag: &str, cx: &mut Context<Self>) {
        if let Some(row) = self.album_draft.rows.get_mut(i) {
            row.list.retain(|t| t != tag);
        }
        self.set_draft_focus(DraftFocus::Value(i), cx);
        self.refresh_draft_preview();
    }

    /// The focused keyword list's chips, when the focus is on one.
    fn focused_tokens(&mut self) -> Option<&mut Vec<String>> {
        let DraftFocus::Value(i) = self.album_draft.focus else {
            return None;
        };
        let row = self.album_draft.rows.get_mut(i)?;
        (row.kind() == ValueKind::TagList).then_some(&mut row.list)
    }

    /// ←/→ on a focused keyword list: moves the chip highlight (none, then
    /// the chips; → past the last clears it). False elsewhere.
    pub fn step_draft_token(&mut self, delta: i64, cx: &mut Context<Self>) -> bool {
        let token = self.album_draft.token;
        let Some(len) = self.focused_tokens().map(|list| list.len()) else {
            return false;
        };
        self.album_draft.token = match (token, delta < 0) {
            _ if len == 0 => None,
            (None, true) => Some(len - 1),
            (None, false) => None,
            (Some(0), true) => Some(0),
            (Some(t), true) => Some(t - 1),
            (Some(t), false) => (t + 1 < len).then_some(t + 1),
        };
        cx.notify();
        true
    }

    /// ⌫ on a focused keyword list drops the highlighted chip, else the last;
    /// false elsewhere.
    pub fn pop_draft_token(&mut self, cx: &mut Context<Self>) -> bool {
        let token = self.album_draft.token.take();
        let Some(list) = self.focused_tokens() else {
            return false;
        };
        match token.filter(|t| *t < list.len()) {
            Some(t) => {
                list.remove(t);
            }
            None if list.pop().is_none() => return false,
            None => {}
        }
        self.refresh_draft_preview();
        cx.notify();
        true
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
            DraftFocus::Pin => self.toggle_draft_pin(cx),
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

    /// "Pin to sidebar" on or off; applied after the album is saved.
    pub fn toggle_draft_pin(&mut self, cx: &mut Context<Self>) {
        self.album_draft.pin = !self.album_draft.pin;
        self.set_draft_focus(DraftFocus::Pin, cx);
    }

    /// Sets row `i`'s rating to `n` stars (a star click) and focuses it.
    pub fn set_draft_rating(&mut self, i: usize, n: i64, cx: &mut Context<Self>) {
        if let Some(row) = self.album_draft.rows.get_mut(i) {
            row.text = n.clamp(1, 5).to_string();
        }
        self.set_draft_focus(DraftFocus::Value(i), cx);
        self.refresh_draft_preview();
    }

    /// ←/→ on a focused Rating value; false when the focus is elsewhere.
    pub fn step_draft_rating(&mut self, delta: i64, cx: &mut Context<Self>) -> bool {
        let DraftFocus::Value(i) = self.album_draft.focus else {
            return false;
        };
        let Some(row) = self.album_draft.rows.get(i) else {
            return false;
        };
        if row.field != RuleField::Rating {
            return false;
        }
        // An imported rating outside 1..5 is drawn as-is and clamped here, on the first ←/→.
        let n = row.text.parse::<i64>().unwrap_or(1) + delta;
        self.set_draft_rating(i, n, cx);
        true
    }

    /// With no rows, `Add(0)` is the empty state's + and inserts at 0.
    pub fn add_draft_row_after(&mut self, index: usize, cx: &mut Context<Self>) {
        let at = (index + 1).min(self.album_draft.rows.len());
        self.album_draft
            .rows
            .insert(at, DraftRow::new(RuleField::IsFavorite));
        self.album_draft.focus_on(DraftFocus::Field(at));
        self.album_draft.popup = None;
        self.reveal_draft_focus();
        self.refresh_draft_preview();
        cx.notify();
    }

    /// Removes a row. With none left every item matches; focus moves to the
    /// empty state's Add button.
    pub fn remove_draft_row(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.album_draft.rows.len() {
            self.album_draft.rows.remove(index);
            let focus = match self.album_draft.rows.len() {
                0 => DraftFocus::Add(0),
                n => DraftFocus::Field(index.min(n - 1)),
            };
            self.album_draft.focus_on(focus);
            self.album_draft.popup = None;
            self.refresh_draft_preview();
            cx.notify();
        }
    }
}
