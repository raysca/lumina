use gpui::*;

use crate::app::LuminaApp;
use lumina_core::DeletionTarget;

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
        if let Some(ref stage) = self.network_import.stage {
            match key {
                "escape" => self.close_network_import_dialog(cx),
                "enter" | "return" => match stage {
                    crate::app::network_import::NetworkImportStage::Input { url, .. } => {
                        let url = url.clone();
                        self.submit_network_url(url, cx);
                    }
                    crate::app::network_import::NetworkImportStage::AlreadyDownloaded {
                        ..
                    } if cmd_or_ctrl => self.download_again_from_dialog(cx),
                    crate::app::network_import::NetworkImportStage::AlreadyDownloaded {
                        prior,
                        ..
                    } => {
                        let media_id = prior.media_id;
                        self.show_download_in_library(media_id, cx);
                    }
                    crate::app::network_import::NetworkImportStage::Checking { .. } => {}
                },
                _ => {}
            }
            return true;
        }

        // 1d2. Metadata template editor: Tab/⇧Tab move between fields, ↑/↓
        // between templates, ↵ saves, ⇧⌘⌫ deletes (⌘⌫ edits the field).
        if self.templates.editor.is_some() {
            let shift = keystroke.modifiers.shift;
            match key {
                "escape" => self.close_template_editor(cx),
                "enter" | "return" => self.save_template_editor(cx),
                "tab" => self.cycle_template_field(if shift { -1 } else { 1 }, cx),
                "up" if !cmd_or_ctrl => self.step_template_in_editor(-1, cx),
                "down" if !cmd_or_ctrl => self.step_template_in_editor(1, cx),
                "backspace" | "delete" if cmd_or_ctrl && shift => {
                    self.delete_template_in_editor(cx)
                }
                _ => {}
            }
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

        // 1e. Downloads dropdown: every key stays in the list, so ⌫ removes
        // a download and never trashes the grid selection behind it.
        if self.downloads.open {
            let focused = self.downloads.focused;
            match key {
                "escape" => self.close_downloads_panel(cx),
                "up" => self.move_download_focus(-1, cx),
                "down" => self.move_download_focus(1, cx),
                // ⌘↵: Download Again on an "already in your library" row.
                "enter" | "return" if cmd_or_ctrl => {
                    if let Some(id) = focused {
                        self.download_again(id, cx);
                    }
                }
                "enter" | "return" => {
                    if let Some(id) = focused {
                        self.download_main_action(id, cx);
                    }
                }
                "backspace" | "delete" if !cmd_or_ctrl => {
                    if let Some(id) = focused {
                        self.cancel_or_remove_download(id, cx);
                    }
                }
                "l" if cmd_or_ctrl && keystroke.modifiers.alt => self.close_downloads_panel(cx),
                _ => {}
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
            // ↑/↓ walk the suggestions; Enter then adds the highlighted one.
            if key == "up" || key == "down" {
                self.step_tag_suggestion(if key == "up" { -1 } else { 1 }, cx);
                return true;
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
                return true;
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

            // ⌘Z / ⇧⌘Z: rename, merge and delete happen here, so their undo
            // must too. The history step reloads the counts and detail panel.
            if cmd_or_ctrl && key.eq_ignore_ascii_case("z") {
                if keystroke.modifiers.shift {
                    self.redo_last(cx);
                } else {
                    self.undo_last(cx);
                }
                return true;
            }

            // ⇧⌘A alias and ⇧⌘M Move to… for the focused keyword; before
            // ⌘A / ⌘M, which ignore ⇧.
            if cmd_or_ctrl && keystroke.modifiers.shift && key == "a" {
                self.start_alias_input(cx);
                return true;
            }
            if cmd_or_ctrl && keystroke.modifiers.shift && key == "m" {
                self.start_keyword_move(cx);
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
                // Not a filter character: keep it from the input handler.
                cx.stop_propagation();
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

            // Typing filters tags through the field input (app/text_input.rs).
            return true;
        }

        // 6. Sidebar search field input focus
        if self.search_focused {
            // ←/→/⌫ on the query bar's chips (passed on by the input).
            if self.search_bar_key(key, cx) || self.top_match_key(key, cx) {
                return true;
            }
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
            return true;
        }

        // 6b. Edit mode in the viewer. After every sheet and field above
        // (one opened from the menu takes its keys first), before the
        // global Enter (closes the viewer) and the viewer's -/= zoom. Every
        // other key is absorbed: ⌘⌫, arrows and ⌘Z must not act on the
        // photo, or another one, under the draft.
        if self.edits.session.is_some() {
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
            return true;
        }

        // 6c. Compare: after every sheet and field above, before the toast's
        // Esc and every global shortcut, which it absorbs (app/present.rs).
        if self.handle_compare_key(keystroke, cx) {
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
            }
        } else if enter || key == "space" {
            self.activate_draft_focus(cx);
        } else if key == "down" && !self.draft_popup_options(self.album_draft.focus).is_empty() {
            self.toggle_draft_popup(self.album_draft.focus, cx);
        }
    }
}
