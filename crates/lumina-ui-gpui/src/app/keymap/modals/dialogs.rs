//! Key handling for the modal dialogs: album builder, settings, delete
//! confirm, network import, template editor and the downloads panel.

use gpui::*;

use crate::app::downloads::DownloadSurface;
use crate::app::LuminaApp;
use lumina_core::DeletionTarget;

impl LuminaApp {
    /// Smart album builder keys. Popups take arrows/Enter/Space/Esc first;
    /// otherwise Tab moves focus, ⌘Enter is OK and Esc is Cancel. In a text
    /// field Enter is OK (the default button); elsewhere Enter/Space press
    /// the focused control.
    pub(super) fn handle_album_builder_key(
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
    pub(super) fn settings_key(
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
    pub(super) fn delete_confirm_key(&mut self, key: &str, cx: &mut Context<Self>) {
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
    pub(super) fn network_import_key(
        &mut self,
        key: &str,
        cmd_or_ctrl: bool,
        cx: &mut Context<Self>,
    ) {
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
    pub(super) fn template_editor_key(
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
    pub(super) fn downloads_panel_key(
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
}
