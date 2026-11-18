//! Key handling for inline fields: tags, search, edit mode and the
//! inspector's text fields.

use gpui::*;

use crate::app::LuminaApp;

impl LuminaApp {
    /// The inspector's inline tag field.
    pub(super) fn tag_input_key(&mut self, key: &str, cx: &mut Context<Self>) {
        if key == "escape" {
            self.tag_input_active = false;
            self.tag_input_buffer.clear();
            cx.notify();
            return;
        }
        if key == "enter" || key == "return" {
            self.commit_tag_input(cx);
            return;
        }
        // ↑/↓ walk the suggestions; Enter then adds the highlighted one.
        if key == "up" || key == "down" {
            self.step_tag_suggestion(if key == "up" { -1 } else { 1 }, cx);
            return;
        }
        if key == "backspace" {
            // Token-field behaviour: ⌫ in an empty field (the input
            // passes it on, see app/text_input.rs) removes the single
            // item's last tag, the keyboard path for ×.
            if self.tag_input_buffer.is_empty() && self.selected_indices.len() <= 1 {
                if let Some(last) = self.selected_item_tags.last().cloned() {
                    self.batch_remove_tag(last, cx);
                }
            }
            cx.notify();
        }
    }

    /// The focused search field: chips, suggestions, Esc layers, ↵ commits.
    pub(super) fn search_field_key(&mut self, key: &str, cx: &mut Context<Self>) {
        // ←/→/⌫ on the query bar's chips (passed on by the input).
        if self.search_bar_key(key, cx) || self.top_match_key(key, cx) {
            return;
        }
        if key == "escape" {
            // Escape layers: first hide the suggestion dropdown, then blur.
            if !self.search_suggestions.dismissed && !self.search_suggestion_items().is_empty() {
                self.dismiss_search_suggestions();
                cx.notify();
            } else {
                self.search_focused = false;
                self.search_suggestions.dismissed = false;
                self.search_suggestions.selected = None;
                cx.notify();
            }
            return;
        }
        if key == "enter" || key == "return" {
            if !self.search_suggestions.dismissed {
                if let Some(index) = self.search_suggestions.selected {
                    let suggestion_count = self.search_suggestion_items().len();
                    if index < suggestion_count {
                        self.apply_search_suggestion_at(index, cx);
                        self.record_current_search();
                        cx.notify();
                        return;
                    }
                }
            }
            // No highlighted suggestion: keep the existing commit behavior.
            self.commit_search(cx);
            return;
        }
        if key == "up" || key == "down" {
            // Moving the highlight must not edit the query.
            self.move_search_suggestion(if key == "up" { -1 } else { 1 }, cx);
        }
    }

    /// Edit mode in the viewer: crop, rotate, exposure, auto, ↵ done.
    pub(super) fn edit_mode_key(
        &mut self,
        key: &str,
        cmd_or_ctrl: bool,
        keystroke: &Keystroke,
        cx: &mut Context<Self>,
    ) {
        let m = &keystroke.modifiers;
        let plain = !cmd_or_ctrl && !m.alt;
        let cropping = self
            .edits
            .session
            .as_ref()
            .is_some_and(|s| s.crop.is_some());
        let step = if m.shift { 0.1 } else { 0.01 };
        match key {
            // After a failed Done, Esc first dismisses the banner and
            // keeps the draft for another try.
            "escape" if self.error_message.is_some() => self.dismiss_error(cx),
            // Esc in crop mode leaves crop only, dropping that crop.
            "escape" if cropping => self.leave_crop(false, cx),
            "escape" => self.cancel_edit(cx),
            // C: crop, then the next aspect. ⇧C: crop on / off, keeping
            // the crop and edit mode (apply and keep editing).
            k if plain && k.eq_ignore_ascii_case("c") => {
                if m.shift {
                    self.toggle_crop(cx)
                } else {
                    self.crop_key(cx)
                }
            }
            // Arrows move the crop (⇧ 10 %), ⌥ resizes it. Outside crop
            // they fall through to the absorbing arm below.
            "left" | "right" | "up" | "down" if cropping && !cmd_or_ctrl => {
                let delta = match key {
                    "left" => (-step, 0.0),
                    "right" => (step, 0.0),
                    "up" => (0.0, -step),
                    _ => (0.0, step),
                };
                self.crop_arrow(delta, m.alt, cx);
            }
            "enter" | "return" => self.finish_edit(cx),
            k if plain && k.eq_ignore_ascii_case("r") => self.rotate_edit_draft(
                if m.shift {
                    crate::views::viewer_state::ViewerRotation::Left
                } else {
                    crate::views::viewer_state::ViewerRotation::Right
                },
                cx,
            ),
            "-" | "_" if plain => self.step_edit_exposure(-1, cx),
            "=" | "+" if plain => self.step_edit_exposure(1, cx),
            k if plain && !m.shift && k.eq_ignore_ascii_case("a") => self.toggle_edit_auto(cx),
            _ => {}
        }
    }

    /// The inspector's inline title, date, location or caption field, in
    /// that order when more than one is active.
    pub(super) fn inspector_field_key(&mut self, key: &str, cx: &mut Context<Self>) {
        let enter = key == "enter" || key == "return";
        if key != "escape" && !enter {
            return;
        }
        if self.caption.title_input_active {
            if enter {
                self.commit_title_input(cx)
            } else {
                self.cancel_title_input(cx)
            }
        } else if self.caption.date_input_active {
            if enter {
                self.commit_date_input(cx)
            } else {
                self.cancel_date_input(cx)
            }
        } else if self.caption.location_input_active {
            if enter {
                self.commit_location_input(cx)
            } else {
                self.cancel_location_input(cx)
            }
        } else if enter {
            self.commit_caption_input(cx)
        } else {
            self.cancel_caption_input(cx)
        }
    }
}
