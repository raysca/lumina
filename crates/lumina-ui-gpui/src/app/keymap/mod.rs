use gpui::*;

use crate::app::LuminaApp;

mod grid;
mod modals;
mod viewer;

impl LuminaApp {
    pub fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let keystroke = &event.keystroke;
        let key = keystroke.key.as_str();
        let cmd_or_ctrl = keystroke.modifiers.secondary(cx) || keystroke.modifiers.control;
        // ⌃⌘ chords (⌃⌘F full screen) must not fall into the ⌘ handlers.
        let ctrl_cmd = keystroke.modifiers.platform && keystroke.modifiers.control;

        // Typed text for the active field goes to its input (Shift, ⌥
        // characters, dead keys, IME); see app/text_input.rs.
        if self.leaves_key_for_text_input(keystroke) {
            return;
        }

        // Shield modal dialogs: absorb keystrokes so background shortcuts don't mutate or delete files
        if self.handle_modal_key_down(key, cmd_or_ctrl, keystroke, cx) {
            return;
        }

        // "Building your library": ↵/Esc open the library, ⌘. stops the
        // scan; nothing else reaches the grid hidden behind it.
        if self.first_scan.is_some() {
            if key == "enter" || key == "escape" {
                self.dismiss_first_scan(cx);
            } else if cmd_or_ctrl && key == "." {
                self.stop_first_scan(cx);
            }
            return;
        }
        // Welcome: ↑/↓ focus a recent library and ↵ opens it; with none
        // focused ↵ chooses a folder. ⇧↵ uses Pictures (when offered), ⌥↵
        // explores the sample library, ⌫
        // forgets the focused recent, Esc clears the focus (then the error
        // banner). ⌘O opens a library, ⇧⌘I imports into a new one (before
        // the global ⇧⌘I, which needs an open library) and ⌘, Preferences.
        // Every other key stops here: the palette, shortcut sheet and grid are not painted
        // on this screen, and an invisible overlay would swallow every key.
        if self.grid_state.is_none() {
            let plain = !cmd_or_ctrl && !keystroke.modifiers.alt && !keystroke.modifiers.shift;
            let focused = self.welcome.focused_recent;
            match key {
                "enter"
                    if keystroke.modifiers.shift && !cmd_or_ctrl && !keystroke.modifiers.alt =>
                {
                    if self.has_desktop_pics {
                        self.open_pictures_library(cx);
                    }
                }
                "enter"
                    if keystroke.modifiers.alt && !cmd_or_ctrl && !keystroke.modifiers.shift =>
                {
                    self.open_sample_library(cx)
                }
                "enter" if plain => match focused {
                    Some(index) => self.open_recent_at(index, cx),
                    None => self.pick_folder_dialog(cx),
                },
                "up" | "down" if plain => {
                    self.move_recent_focus(if key == "up" { -1 } else { 1 }, cx);
                }
                "backspace" | "delete" if plain => {
                    if let Some(index) = focused {
                        self.remove_recent_at(index, cx);
                    }
                }
                "escape" if focused.is_some() => {
                    self.welcome.focused_recent = None;
                    cx.notify();
                }
                "escape" if self.error_message.is_some() => self.dismiss_error(cx),
                "o" if cmd_or_ctrl && !keystroke.modifiers.shift => self.pick_folder_dialog(cx),
                k if k.eq_ignore_ascii_case("i")
                    && cmd_or_ctrl
                    && keystroke.modifiers.shift
                    && !keystroke.modifiers.alt =>
                {
                    self.pick_new_library_for_import(cx)
                }
                "," if cmd_or_ctrl && !keystroke.modifiers.shift && !keystroke.modifiers.alt => {
                    self.open_preferences(cx)
                }
                _ => {}
            }
            return;
        }

        // The viewer's right-click menu is modal: Escape closes it and every
        // other key (⌘A, ⌘Z, ]…) is absorbed so nothing changes what it acts on.
        if self.is_fullscreen_viewer && self.context_menu_state.is_some() {
            if key == "escape" {
                self.close_context_menu(cx);
            }
            return;
        }

        // Any key that reaches the viewer brings the immersive HUD back
        // (I, ⌘K, arrows…), except H, which toggles it.
        if self.is_fullscreen_viewer && !key.eq_ignore_ascii_case("h") {
            self.wake_immersive_hud(cx);
        }

        // Esc dismisses the error banner before it reaches the grid or viewer.
        if key == "escape" && self.error_message.is_some() {
            self.dismiss_error(cx);
            return;
        }

        // ⌘[ / ⌘]: browsing Back / Forward. Secondary modifier only, so the
        // viewer's plain [ / ] previous/next shortcuts keep working.
        if cmd_or_ctrl && key == "[" {
            self.navigate_back(cx);
            return;
        }
        if cmd_or_ctrl && key == "]" {
            self.navigate_forward(cx);
            return;
        }

        // ⌘A: Select All; ⇧⌘A: Add to Album…
        if cmd_or_ctrl && key.eq_ignore_ascii_case("a") {
            if keystroke.modifiers.shift {
                if !keystroke.modifiers.alt {
                    self.open_album_picker(cx);
                }
            } else {
                self.select_all(cx);
            }
            return;
        }

        // Cmd+/: keyboard shortcut cheat sheet
        if cmd_or_ctrl && key == "/" {
            self.toggle_shortcuts_overlay(cx);
            return;
        }

        // Cmd+K: command palette
        if cmd_or_ctrl && key.eq_ignore_ascii_case("k") {
            if self.command_palette_open {
                self.close_command_palette(cx);
            } else {
                self.open_command_palette(cx);
            }
            return;
        }

        // ⌃⌘S: Show/Hide Sidebar. After the modal shield above, so a sheet,
        // the palette or a focused field keeps it, like every other shortcut.
        if ctrl_cmd
            && !keystroke.modifiers.shift
            && !keystroke.modifiers.alt
            && key.eq_ignore_ascii_case("s")
        {
            self.toggle_sidebar(cx);
            return;
        }

        // ⌥⌘N: New Smart Album…
        if cmd_or_ctrl && keystroke.modifiers.alt && key.eq_ignore_ascii_case("n") {
            self.open_album_builder(cx);
            return;
        }

        // ⌘N: New Album (always empty, in rename mode)
        if cmd_or_ctrl
            && !ctrl_cmd
            && !keystroke.modifiers.alt
            && !keystroke.modifiers.shift
            && key.eq_ignore_ascii_case("n")
        {
            self.new_album(cx);
            return;
        }

        // ⌥⌘U: Import Video from URL…
        if cmd_or_ctrl && keystroke.modifiers.alt && key.eq_ignore_ascii_case("u") {
            self.open_network_import_dialog(cx);
            return;
        }

        // ⌥⌘L: show or hide the downloads list
        if cmd_or_ctrl && keystroke.modifiers.alt && key.eq_ignore_ascii_case("l") {
            self.toggle_downloads_panel(cx);
            return;
        }

        // ⌘O: Open Library… (also File menu and the sidebar LIBRARY header)
        if cmd_or_ctrl && !keystroke.modifiers.shift && key.eq_ignore_ascii_case("o") {
            self.pick_folder_dialog(cx);
            return;
        }

        // Cmd+,: preferences
        if cmd_or_ctrl && key == "," {
            self.open_preferences(cx);
            return;
        }

        // Cmd+Z / Cmd+Shift+Z: undo / redo reversible catalog edits
        if cmd_or_ctrl && key.eq_ignore_ascii_case("z") {
            if keystroke.modifiers.shift {
                self.redo_last(cx);
            } else {
                self.undo_last(cx);
            }
            return;
        }

        if cmd_or_ctrl && !ctrl_cmd && key.eq_ignore_ascii_case("f") {
            self.search_focused = true;
            self.search_query.clear();
            cx.notify();
            return;
        }

        // ⇧⌘R: Reveal in Finder (the context menu's shortcut)
        if cmd_or_ctrl && keystroke.modifiers.shift && key.eq_ignore_ascii_case("r") {
            if let Some(path) = self.selected_item.as_ref().map(|item| item.path.clone()) {
                self.reveal_item_in_finder(&path, cx);
            }
            return;
        }

        // ⌥⌘C: Copy File Path
        if cmd_or_ctrl && keystroke.modifiers.alt && key.eq_ignore_ascii_case("c") {
            if let Some(item) = self.selected_item.as_ref() {
                let path = item.path.to_string_lossy().to_string();
                self.copy_to_clipboard(&path, cx);
            }
            return;
        }

        // ⌘E: Edit in External Editor; ⌥⌘E: Edit a Copy. Text fields, sheets
        // and edit mode took the key in `handle_modal_key_down` above.
        if cmd_or_ctrl && !ctrl_cmd && !keystroke.modifiers.shift && key.eq_ignore_ascii_case("e") {
            self.edit_selected_externally(keystroke.modifiers.alt, cx);
            return;
        }

        // Cmd+R: rescan the active library (filesystem watcher is best-effort)
        if cmd_or_ctrl && key.eq_ignore_ascii_case("r") {
            self.rescan_library(cx);
            return;
        }

        // ⇧⌘I imports; ⌘I opens the Get Info window, as in Finder; I toggles
        // the inspector.
        if key.eq_ignore_ascii_case("i") {
            if cmd_or_ctrl && keystroke.modifiers.shift {
                self.pick_import_folders(cx);
            } else if cmd_or_ctrl {
                self.get_info(cx);
            } else {
                self.toggle_inspector(cx);
            }
            return;
        }

        // Enter/Return: Close viewer if open, or open selected item
        if key == "enter" || key == "return" {
            if self.is_fullscreen_viewer {
                self.close_viewer(cx);
            } else if let Some(ref item) = self.selected_item {
                self.open_viewer(item.clone(), cx);
            } else if self.get_active_count() > 0 {
                self.select_index(0, cx);
                if let Some(ref item) = self.selected_item {
                    self.open_viewer(item.clone(), cx);
                }
            }
            return;
        }

        // P / X / U flag and 0-5 rate, in the grid and the viewer (plain
        // keys only: ⌥P stays PiP).
        if self.handle_culling_key(keystroke, cx) {
            return;
        }

        // Fullscreen Viewer Shortcuts
        if self.is_fullscreen_viewer {
            if self.handle_viewer_key_down(key, cmd_or_ctrl, keystroke, window, cx) {
                return;
            }
            return;
        }

        if key.eq_ignore_ascii_case("p") && keystroke.modifiers.alt && !cmd_or_ctrl {
            self.toggle_pip(cx);
            return;
        }

        // Grid & Browser Shortcuts
        self.handle_grid_key_down(
            key,
            cmd_or_ctrl,
            keystroke.modifiers.shift,
            keystroke.modifiers.alt,
            cx,
        );
    }
}
