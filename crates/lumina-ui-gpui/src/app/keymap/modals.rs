use gpui::*;

use crate::app::downloads::DownloadSurface;
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
        let step: i64 = match key {
            "left" => -1,
            "right" => 1,
            _ => 0,
        };
        if step != 0 && (self.step_draft_rating(step, cx) || self.step_draft_token(step, cx)) {
            return;
        }
        if key == "backspace" && self.pop_draft_token(cx) {
            return;
        }
        if self.draft_focus_is_typed() {
            if enter {
                self.save_draft_smart_album(cx);
            }
        } else if enter || key == "space" {
            self.activate_draft_focus(cx);
        } else if key == "down" && self.draft_focus_has_popup(self.album_draft.focus) {
            self.toggle_draft_popup(self.album_draft.focus, cx);
        }
    }

    /// Settings: every key stays here, so library shortcuts never act on the
    /// hidden library. ⌘, toggles it.
    fn settings_key(
        &mut self,
        key: &str,
        cmd_or_ctrl: bool,
        keystroke: &Keystroke,
        cx: &mut Context<Self>,
    ) {
        // Reset to defaults asks first; its card takes every key.
        if self.preferences_reset_confirm {
            match key {
                "escape" => self.cancel_reset_preferences(cx),
                "enter" | "return" => self.confirm_reset_preferences(cx),
                _ => {}
            }
            return;
        }
        match key {
            "escape" => self.close_preferences(cx),
            "," if cmd_or_ctrl => self.close_preferences(cx),
            "up" => self.step_preferences_tab(-1, cx),
            "down" => self.step_preferences_tab(1, cx),
            // The page's buttons: Tab/⇧Tab walk them, ↵ or Space press.
            // ⌘/⌃/⌥-Tab stay absorbed, as on the welcome screen.
            "tab" if !cmd_or_ctrl && !keystroke.modifiers.alt => {
                let delta = if keystroke.modifiers.shift { -1 } else { 1 };
                self.step_settings_focus(delta, cx);
            }
            "enter" | "return" | "space" => self.press_settings_focus(cx),
            // Each page's one segmented control: General's Default view
            // on launch, Downloads' Simultaneous downloads.
            "left" | "right"
                if self.preferences_tab == crate::app::preferences::PreferencesTab::General =>
            {
                let view = if key == "left" {
                    crate::config::LaunchView::Library
                } else {
                    crate::config::LaunchView::LastViewed
                };
                self.set_launch_view(view, cx);
            }
            "left" | "right"
                if self.preferences_tab == crate::app::preferences::PreferencesTab::Downloads =>
            {
                let choices = crate::config::DOWNLOAD_CONCURRENCY_CHOICES;
                let at = choices
                    .iter()
                    .position(|n| *n == self.config.download_concurrency)
                    .unwrap_or(0);
                let next = if key == "left" {
                    at.saturating_sub(1)
                } else {
                    (at + 1).min(choices.len() - 1)
                };
                self.set_download_concurrency(choices[next], cx);
            }
            _ => {}
        }
    }

    /// The delete confirmation: Esc, ↵, ↑/↓ between targets, Space to
    /// acknowledge a permanent delete.
    fn delete_confirm_key(&mut self, key: &str, cx: &mut Context<Self>) {
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
    }

    /// The Import Video from URL dialog: ↵ runs its current step.
    fn network_import_key(&mut self, key: &str, cmd_or_ctrl: bool, cx: &mut Context<Self>) {
        let Some(ref stage) = self.network_import.stage else {
            return;
        };
        match key {
            "escape" => self.close_network_import_dialog(cx),
            "enter" | "return" => match stage {
                crate::app::network_import::NetworkImportStage::Input { url, .. } => {
                    let url = url.clone();
                    self.submit_network_url(url, cx);
                }
                crate::app::network_import::NetworkImportStage::AlreadyDownloaded { .. }
                    if cmd_or_ctrl =>
                {
                    self.download_again_from_dialog(cx)
                }
                crate::app::network_import::NetworkImportStage::AlreadyDownloaded {
                    prior, ..
                } => {
                    let media_id = prior.media_id;
                    self.show_download_in_library(media_id, cx);
                }
                crate::app::network_import::NetworkImportStage::Ready { .. } => {
                    self.import_inspected_link(cx);
                }
                crate::app::network_import::NetworkImportStage::Checking { .. } => {}
            },
            _ => {}
        }
    }

    /// The metadata template editor.
    fn template_editor_key(
        &mut self,
        key: &str,
        cmd_or_ctrl: bool,
        keystroke: &Keystroke,
        cx: &mut Context<Self>,
    ) {
        let shift = keystroke.modifiers.shift;
        match key {
            "escape" => self.close_template_editor(cx),
            "enter" | "return" => self.save_template_editor(cx),
            "tab" => self.cycle_template_field(if shift { -1 } else { 1 }, cx),
            "up" if !cmd_or_ctrl => self.step_template_in_editor(-1, cx),
            "down" if !cmd_or_ctrl => self.step_template_in_editor(1, cx),
            "backspace" | "delete" if cmd_or_ctrl && shift => self.delete_template_in_editor(cx),
            _ => {}
        }
    }

    /// The Downloads dropdown list.
    fn downloads_panel_key(
        &mut self,
        key: &str,
        cmd_or_ctrl: bool,
        keystroke: &Keystroke,
        cx: &mut Context<Self>,
    ) {
        let focused = self.downloads.focused;
        match key {
            "escape" => self.close_downloads_panel(cx),
            "up" => self.move_download_focus(-1, DownloadSurface::Panel, cx),
            "down" => self.move_download_focus(1, DownloadSurface::Panel, cx),
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
    }

    /// The inspector's inline tag field.
    fn tag_input_key(&mut self, key: &str, cx: &mut Context<Self>) {
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
    fn search_field_key(&mut self, key: &str, cx: &mut Context<Self>) {
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
    fn edit_mode_key(
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
    fn inspector_field_key(&mut self, key: &str, cx: &mut Context<Self>) {
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
