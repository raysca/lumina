use gpui::*;

use crate::app::LuminaApp;

mod dialogs;
mod fields;

/// Shortcut sheet scroll steps: one row, and most of the 560px card.
const SHEET_LINE: f32 = 32.0;
const SHEET_PAGE: f32 = 480.0;

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
        // "Downloads are still running": it can open over any other dialog
        // (Quit from the menu), so its keys come first.
        if self.downloads.confirm.is_some() {
            match key {
                "escape" => self.dismiss_downloads_prompt(cx),
                "enter" | "return" => self.confirm_downloads_prompt(cx),
                _ => {}
            }
            return true;
        }

        // 0. Command palette
        if self.command_palette_open {
            match key {
                "escape" => self.close_command_palette(cx),
                "enter" | "return" => self.execute_selected_command(cx),
                "up" => self.move_command_selection(-1, cx),
                "down" => self.move_command_selection(1, cx),
                _ => {}
            }
            return true;
        }

        // 0. Add to Album picker: typing filters (the field input), and every
        // other key stays here, so ⌫ never removes or trashes the selection.
        if self.albums.picker.is_some() {
            match key {
                "escape" => self.close_album_picker(cx),
                "enter" | "return" => self.activate_album_picker(cx),
                "up" => self.move_album_picker(-1, cx),
                "down" => self.move_album_picker(1, cx),
                _ => {}
            }
            return true;
        }

        // 0. Keyboard shortcut cheat sheet
        if self.shortcuts_overlay_open {
            match key {
                "escape" | "/" => self.close_shortcuts_overlay(cx),
                "up" => self.scroll_shortcuts(-SHEET_LINE, cx),
                "down" => self.scroll_shortcuts(SHEET_LINE, cx),
                "pageup" => self.scroll_shortcuts(-SHEET_PAGE, cx),
                "pagedown" | "space" => self.scroll_shortcuts(SHEET_PAGE, cx),
                "home" => self.scroll_shortcuts(f32::NEG_INFINITY, cx),
                "end" => self.scroll_shortcuts(f32::INFINITY, cx),
                _ => {}
            }
            return true;
        }

        // 0b. Settings: every other key stays here, so library shortcuts
        // never act on the hidden library. ⌘, toggles it.
        if self.preferences_open {
            self.settings_key(key, cmd_or_ctrl, keystroke, cx);
            return true;
        }

        // 1. Delete confirmation modal
        if self.delete_confirm_state.is_some() {
            self.delete_confirm_key(key, cx);
            return true;
        }

        // 1b. Smart album delete confirmation
        if self.album_draft.confirm_delete.is_some() {
            match key {
                "escape" => self.cancel_delete_smart_album(cx),
                "enter" | "return" => self.confirm_delete_smart_album(cx),
                _ => {}
            }
            return true;
        }

        // 1b2. Album delete confirmation
        if self.albums.confirm_delete.is_some() {
            match key {
                "escape" => self.cancel_delete_album(cx),
                "enter" | "return" => self.confirm_delete_album(cx),
                _ => {}
            }
            return true;
        }

        // 1c. Import dialog
        if self.import.dialog.is_some() {
            match key {
                "escape" => self.cancel_import_dialog(cx),
                "enter" | "return" => self.confirm_import(cx),
                _ => {}
            }
            return true;
        }

        // 1d. Network import dialog
        if self.network_import.stage.is_some() {
            self.network_import_key(key, cmd_or_ctrl, cx);
            return true;
        }

        // 1d2. Metadata template editor: Tab/⇧Tab move between fields, ↑/↓
        // between templates, ↵ saves, ⇧⌘⌫ deletes (⌘⌫ edits the field).
        if self.templates.editor.is_some() {
            self.template_editor_key(key, cmd_or_ctrl, keystroke, cx);
            return true;
        }

        // 1d3. Sort popover: Escape closes it; other keys stay here, so ⌫
        // never trashes the selection behind it.
        if self.sort_menu_open {
            if key == "escape" {
                self.close_sort_menu(cx);
            }
            return true;
        }

        // 1d3b. Viewer star popover: 1–5 rate, 0 clears, Escape closes it
        // before the viewer; other keys stay here, so F or ] never act on
        // the photo behind it.
        if self.rate_popover_shown() {
            match key {
                "escape" => self.close_rate_popover(cx),
                "0" | "1" | "2" | "3" | "4" | "5" if !cmd_or_ctrl => {
                    self.rate_from_popover(key.parse().unwrap_or(0), cx)
                }
                _ => {}
            }
            return true;
        }

        // 1d4. Add filter menu: ↑/↓ move, ↵ chooses, Escape closes; other
        // keys stay here, so ⌫ never trashes the selection behind it.
        if self.filter_menu.is_some() {
            match key {
                "escape" => self.close_filter_menu(cx),
                "up" => self.move_filter_menu_highlight(-1, cx),
                "down" => self.move_filter_menu_highlight(1, cx),
                "enter" | "return" => self.activate_filter_menu_highlight(cx),
                _ => {}
            }
            return true;
        }

        // 1d5. Album page More menu: ↑/↓ move, ↵ runs, Escape closes;
        // other keys stay here, so ⌫ never trashes the selection behind it.
        if self.albums.page.menu.is_some() {
            self.handle_album_menu_key(key, cx);
            return true;
        }

        // 1d6. Downloads screen row "…" menu: ↑/↓ move, ↵ runs, Escape
        // closes; other keys stay here, so ⌫ never removes a row behind it.
        if self.downloads.row_menu.is_some() {
            self.handle_download_row_menu_key(key, cx);
            return true;
        }

        // 1e. Downloads dropdown: every key stays in the list, so ⌫ removes
        // a download and never trashes the grid selection behind it.
        if self.downloads.open {
            self.downloads_panel_key(key, cmd_or_ctrl, keystroke, cx);
            return true;
        }

        // 2. Smart album builder sheet
        if self.album_draft.modal_open {
            self.handle_album_builder_key(key, cmd_or_ctrl, keystroke, cx);
            return true;
        }

        // 3. Inline tag input in inspector
        if self.tag_input_active {
            self.tag_input_key(key, cx);
            return true;
        }

        // 3a-3d. Inline title, date, location and caption inputs in the
        // inspector: Esc cancels, ↵ commits, every other key is absorbed.
        if self.caption.title_input_active
            || self.caption.date_input_active
            || self.caption.location_input_active
            || self.caption.input_active
        {
            self.inspector_field_key(key, cx);
            return true;
        }

        // 4b. Renaming an album in the sidebar
        if self.albums.renaming.is_some() {
            match key {
                "escape" => self.cancel_album_rename(cx),
                "enter" | "return" => self.commit_album_rename(cx),
                _ => {}
            }
            return true;
        }

        // 5. The Tags page's rename and alias fields (its other keys are
        // page keys, `keymap/tags_page.rs`). Every other key is absorbed.
        if self.tag_manager.merge.is_some() {
            match key {
                "escape" => self.cancel_tag_merge(cx),
                "enter" | "return" => self.commit_tag_merge(cx),
                "up" => self.step_tag_merge(-1, cx),
                "down" => self.step_tag_merge(1, cx),
                _ => {}
            }
            return true;
        }
        if self.tag_manager.editing.is_some() {
            match key {
                "escape" => self.cancel_modal_tag_rename(cx),
                "enter" | "return" => self.commit_modal_tag_rename(cx),
                _ => {}
            }
            return true;
        }
        if self.tag_manager.alias_editing {
            match key {
                "escape" => self.cancel_alias_input(cx),
                "enter" | "return" => self.commit_alias_input(cx),
                // Reaches here only from an empty field (text_input.rs).
                "backspace" => self.remove_last_alias(cx),
                _ => {}
            }
            return true;
        }

        // 6. Sidebar search field input focus
        if self.search_focused {
            self.search_field_key(key, cx);
            return true;
        }

        // 6b. Edit mode in the viewer. After every sheet and field above
        // (one opened from the menu takes its keys first), before the
        // global Enter (closes the viewer) and the viewer's -/= zoom. Every
        // other key is absorbed: ⌘⌫, arrows and ⌘Z must not act on the
        // photo, or another one, under the draft.
        if self.edits.session.is_some() {
            self.edit_mode_key(key, cmd_or_ctrl, keystroke, cx);
            return true;
        }

        // 6c. Compare: after every sheet and field above, before the toast's
        // Esc and every global shortcut, which it absorbs (app/present.rs).
        if self.handle_compare_key(keystroke, cx) {
            return true;
        }

        // 6d. Album cover pick mode: ↵ picks the selection, Esc cancels,
        // before the toast's Esc (Set as Cover leaves an undo toast up).
        if !cmd_or_ctrl && self.handle_cover_pick_key(key, cx) {
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
}
