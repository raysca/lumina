use gpui::*;

use crate::app::LuminaApp;
use lumina_core::DeletionTarget;

impl LuminaApp {
    /// Handles keystrokes when any modal dialog or inline text field is active.
    /// Absorbs keystrokes so background shortcuts don't mutate or delete files.
    /// Returns `true` if a modal consumed the event, `false` otherwise.
    pub fn handle_modal_key_down(
        &mut self,
        key: &str,
        cmd_or_ctrl: bool,
        keystroke: &Keystroke,
        cx: &mut Context<Self>,
    ) -> bool {
        // 0. Command palette
        if self.command_palette_open {
            match key {
                "escape" => self.close_command_palette(cx),
                "enter" | "return" => self.execute_selected_command(cx),
                "up" => self.move_command_selection(-1, cx),
                "down" => self.move_command_selection(1, cx),
                "backspace" => {
                    self.command_query.pop();
                    self.command_selected = 0;
                    self.command_scroll.scroll_to_item(1);
                    cx.notify();
                }
                _ => {
                    let typed = if key == "space" { " " } else { key };
                    if !cmd_or_ctrl && typed.chars().count() == 1 {
                        self.command_query.push_str(typed);
                        self.command_selected = 0;
                        self.command_scroll.scroll_to_item(1);
                        cx.notify();
                    }
                }
            }
            return true;
        }

        // 0. Keyboard shortcut cheat sheet
        if self.shortcuts_overlay_open {
            if key == "escape" || key == "/" {
                self.close_shortcuts_overlay(cx);
            }
            return true;
        }

        // 0b. Preferences modal
        if self.preferences_open {
            if key == "escape" {
                self.close_preferences(cx);
            }
            return true;
        }

        // 1. Delete confirmation modal
        if self.delete_confirm_state.is_some() {
            if key == "escape" {
                self.close_delete_modal(cx);
            } else if key == "enter" || key == "return" {
                self.execute_confirmed_deletion(cx);
            } else if key == "up" {
                self.step_delete_target(-1, cx);
            } else if key == "down" {
                self.step_delete_target(1, cx);
            } else if key == "space"
                && self
                    .delete_confirm_state
                    .as_ref()
                    .is_some_and(|st| st.target == DeletionTarget::PermanentDisk)
            {
                self.toggle_delete_permanent_acknowledged(cx);
            }
            return true;
        }

        // 2. Smart album builder sheet
        if self.album_draft.modal_open {
            self.handle_album_builder_key(key, cmd_or_ctrl, keystroke, cx);
            return true;
        }

        // 3. Inline tag input in inspector
        if self.tag_input_active {
            if key == "escape" {
                self.tag_input_active = false;
                self.tag_input_buffer.clear();
                cx.notify();
                return true;
            }
            if key == "enter" || key == "return" {
                self.commit_tag_input(cx);
                return true;
            }
            if key == "backspace" {
                // Token-field behaviour: ⌫ in an empty field removes the
                // single item's last tag (the keyboard path for ×).
                if self.tag_input_buffer.is_empty() && self.selected_indices.len() <= 1 {
                    if let Some(last) = self.selected_item_tags.last().cloned() {
                        self.batch_remove_tag(last, cx);
                    }
                } else {
                    self.tag_input_buffer.pop();
                }
                cx.notify();
                return true;
            }
            if !cmd_or_ctrl {
                if key == "space" {
                    self.tag_input_buffer.push(' ');
                    cx.notify();
                    return true;
                } else if key.chars().count() == 1
                    && !key.chars().next().is_some_and(|c| c.is_control())
                {
                    self.tag_input_buffer.push_str(key);
                    cx.notify();
                    return true;
                }
            }
            return true;
        }

        // 3a. Inline title input in inspector
        if self.caption.title_input_active {
            if key == "escape" {
                self.cancel_title_input(cx);
                return true;
            }
            if key == "enter" || key == "return" {
                self.commit_title_input(cx);
                return true;
            }
            if key == "backspace" {
                self.caption.title_input_buffer.pop();
                cx.notify();
                return true;
            }
            if !cmd_or_ctrl {
                if key == "space" {
                    self.caption.title_input_buffer.push(' ');
                    cx.notify();
                    return true;
                } else if key.chars().count() == 1
                    && !key.chars().next().is_some_and(|c| c.is_control())
                {
                    self.caption.title_input_buffer.push_str(key);
                    cx.notify();
                    return true;
                }
            }
            return true;
        }

        // 3b. Inline date input in inspector
        if self.caption.date_input_active {
            if key == "escape" {
                self.cancel_date_input(cx);
                return true;
            }
            if key == "enter" || key == "return" {
                self.commit_date_input(cx);
                return true;
            }
            if key == "backspace" {
                self.caption.date_input_buffer.pop();
                cx.notify();
                return true;
            }
            if !cmd_or_ctrl && key.chars().count() == 1 {
                self.caption.date_input_buffer.push_str(key);
                cx.notify();
                return true;
            }
            return true;
        }

        // 3c. Inline location input in inspector
        if self.caption.location_input_active {
            if key == "escape" {
                self.cancel_location_input(cx);
                return true;
            }
            if key == "enter" || key == "return" {
                self.commit_location_input(cx);
                return true;
            }
            if key == "backspace" {
                self.caption.location_input_buffer.pop();
                cx.notify();
                return true;
            }
            if !cmd_or_ctrl && key.chars().count() == 1 {
                self.caption.location_input_buffer.push_str(key);
                cx.notify();
                return true;
            }
            return true;
        }

        // 3d. Inline caption input in inspector
        if self.caption.input_active {
            if key == "escape" {
                self.cancel_caption_input(cx);
                return true;
            }
            if key == "enter" || key == "return" {
                self.commit_caption_input(cx);
                return true;
            }
            if key == "backspace" {
                self.caption.input_buffer.pop();
                cx.notify();
                return true;
            }
            if !cmd_or_ctrl {
                if key == "space" {
                    self.caption.input_buffer.push(' ');
                    cx.notify();
                    return true;
                } else if key.chars().count() == 1
                    && !key.chars().next().is_some_and(|c| c.is_control())
                {
                    self.caption.input_buffer.push_str(key);
                    cx.notify();
                    return true;
                }
            }
            return true;
        }

        // 4. Editing / renaming tag in inspector
        if self.editing_tag.is_some() {
            if key == "escape" {
                self.cancel_tag_rename(cx);
                return true;
            }
            if key == "enter" || key == "return" {
                self.commit_tag_rename(cx);
                return true;
            }
            if key == "backspace" {
                self.editing_tag_buffer.pop();
                cx.notify();
                return true;
            }
            if !cmd_or_ctrl {
                if key == "space" {
                    self.editing_tag_buffer.push(' ');
                    cx.notify();
                    return true;
                } else if key.chars().count() == 1
                    && !key.chars().next().is_some_and(|c| c.is_control())
                {
                    self.editing_tag_buffer.push_str(key);
                    cx.notify();
                    return true;
                }
            }
            return true;
        }

        // 5. Tag manager modal
        if self.tag_manager.open {
            if self.tag_manager.editing.is_some() {
                if key == "escape" {
                    self.cancel_modal_tag_rename(cx);
                    return true;
                }
                if key == "enter" || key == "return" {
                    self.commit_modal_tag_rename(cx);
                    return true;
                }
                if key == "backspace" {
                    self.tag_manager.editing_buffer.pop();
                    cx.notify();
                    return true;
                }
                if !cmd_or_ctrl {
                    if key == "space" {
                        self.tag_manager.editing_buffer.push(' ');
                        cx.notify();
                        return true;
                    } else if key.chars().count() == 1
                        && !key.chars().next().is_some_and(|c| c.is_control())
                    {
                        self.tag_manager.editing_buffer.push_str(key);
                        cx.notify();
                        return true;
                    }
                }
                return true;
            }

            // Esc deselects first, then closes.
            if key == "escape" {
                if self.tag_manager.selected.is_empty() {
                    self.close_tag_manager(cx);
                } else {
                    self.clear_tag_manager_selection(cx);
                }
                return true;
            }
            // Keyboard path for the selection bar: ⌘A check all shown, ⌘M
            // merge, ⌘⌫ delete checked; Space checks the focused tag (both
            // only while the filter is empty).
            if cmd_or_ctrl && key == "a" {
                self.toggle_all_tag_manager_rows(cx);
                return true;
            }
            if cmd_or_ctrl && key == "m" {
                self.merge_checked_tags(cx);
                return true;
            }
            // Only with an empty filter (⌘⌫ edits text) and the bar showing.
            if cmd_or_ctrl
                && key == "backspace"
                && self.tag_manager.search.is_empty()
                && self.tag_manager.selected.len() >= 2
            {
                let tags = self.tag_manager.selected.iter().cloned().collect();
                self.bulk_delete_tags(tags, cx);
                return true;
            }
            if key == "space" && self.tag_manager.search.is_empty() {
                if let Some(tag) = self.tag_manager.focused.clone() {
                    self.toggle_tag_manager_selection(tag, cx);
                }
                return true;
            }

            // ↑/↓ walk the list into the detail panel; Return renames the
            // tag shown there.
            if key == "up" || key == "down" {
                self.step_tag_manager_focus(if key == "up" { -1 } else { 1 }, cx);
                return true;
            }
            if key == "enter" || key == "return" {
                if let Some(tag) = self.tag_manager.focused.clone() {
                    self.start_modal_tag_rename(tag, cx);
                }
                return true;
            }

            // When in Tag Manager without active rename, typing filters tags
            if key == "backspace" {
                self.tag_manager.search.pop();
                cx.notify();
                return true;
            }
            if !cmd_or_ctrl {
                if key == "space" {
                    self.tag_manager.search.push(' ');
                    cx.notify();
                    return true;
                } else if key.chars().count() == 1
                    && !key.chars().next().is_some_and(|c| c.is_control())
                {
                    self.tag_manager.search.push_str(key);
                    cx.notify();
                    return true;
                }
            }
            return true;
        }

        // 6. Toolbar search bar input focus
        if self.search_focused {
            if key == "escape" {
                // Escape layers: first hide the suggestion dropdown, then blur.
                if !self.search_suggestions.dismissed && !self.search_suggestion_items().is_empty()
                {
                    self.dismiss_search_suggestions();
                    cx.notify();
                } else {
                    self.search_focused = false;
                    self.search_suggestions.dismissed = false;
                    self.search_suggestions.selected = None;
                    cx.notify();
                }
                return true;
            }
            if key == "enter" || key == "return" {
                if !self.search_suggestions.dismissed {
                    if let Some(index) = self.search_suggestions.selected {
                        let suggestion_count = self.search_suggestion_items().len();
                        if index < suggestion_count {
                            self.apply_search_suggestion_at(index, cx);
                            self.record_current_search();
                            cx.notify();
                            return true;
                        }
                    }
                }
                // No highlighted suggestion: keep the existing commit behavior.
                self.commit_search(cx);
                return true;
            }
            if key == "up" || key == "down" {
                // Moving the highlight must not edit the query.
                self.move_search_suggestion(if key == "up" { -1 } else { 1 }, cx);
                return true;
            }
            if key == "backspace" {
                self.search_query.pop();
                self.reset_search_suggestions();
                self.clear_selection(cx);
                return true;
            }
            if !cmd_or_ctrl {
                if key == "space" {
                    self.search_query.push(' ');
                    self.reset_search_suggestions();
                    self.clear_selection(cx);
                    return true;
                } else if key.chars().count() == 1
                    && !key.chars().next().is_some_and(|c| c.is_control())
                {
                    self.search_query.push_str(key);
                    self.reset_search_suggestions();
                    self.clear_selection(cx);
                    return true;
                }
            }
            return true;
        }

        // 7. Dismiss a visible toast from the keyboard.
        //
        // This sits after every modal and inline-editor branch above, so those
        // keep Escape precedence: Escape cancels the editor or closes the
        // dialog, and a toast underneath stays until the next Escape or its
        // auto-dismiss timer. The fullscreen viewer and card context menu also
        // own Escape for navigation and are handled after this shield, so the
        // toast yields to them and remains dismissible with the pointer.
        if key == "escape"
            && !cmd_or_ctrl
            && self.toast.is_visible()
            && !self.is_fullscreen_viewer
            && self.context_menu_state.is_none()
        {
            self.dismiss_toast(cx);
            return true;
        }

        false
    }

    /// Smart album builder keys. Popups take arrows/Enter/Space/Esc first;
    /// otherwise Tab moves focus, ⌘Enter is OK and Esc is Cancel. In a text
    /// field Enter is OK (the default button); elsewhere Enter/Space press
    /// the focused control.
    fn handle_album_builder_key(
        &mut self,
        key: &str,
        cmd_or_ctrl: bool,
        keystroke: &Keystroke,
        cx: &mut Context<Self>,
    ) {
        let shift = keystroke.modifiers.shift;
        let enter = key == "enter" || key == "return";
        if let Some(popup) = self.album_draft.popup {
            match key {
                "escape" => {
                    self.album_draft.popup = None;
                    cx.notify();
                }
                "up" => self.move_draft_popup_highlight(-1, cx),
                "down" => self.move_draft_popup_highlight(1, cx),
                "space" => self.pick_draft_option(popup.highlight, cx),
                _ if enter => self.pick_draft_option(popup.highlight, cx),
                "tab" => self.move_draft_focus(if shift { -1 } else { 1 }, cx),
                _ => {}
            }
            return;
        }
        if enter && cmd_or_ctrl {
            self.save_draft_smart_album(cx);
            return;
        }
        match key {
            "escape" => return self.close_album_builder(cx),
            "tab" => return self.move_draft_focus(if shift { -1 } else { 1 }, cx),
            _ => {}
        }
        if self.draft_focus_is_typed() {
            if enter {
                self.save_draft_smart_album(cx);
            } else if key == "backspace" {
                self.edit_draft_text(None, cx);
            } else if !cmd_or_ctrl {
                // key_char carries Shift (capitals, punctuation); key does not.
                let typed = keystroke.key_char.as_deref().unwrap_or(match key {
                    "space" => " ",
                    k => k,
                });
                if typed.chars().count() == 1 && !typed.chars().any(char::is_control) {
                    self.edit_draft_text(Some(typed), cx);
                }
            }
        } else if enter || key == "space" {
            self.activate_draft_focus(cx);
        } else if key == "down" && !self.draft_popup_options(self.album_draft.focus).is_empty() {
            self.toggle_draft_popup(self.album_draft.focus, cx);
        }
    }
}
