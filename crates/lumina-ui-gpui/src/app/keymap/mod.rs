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
        let cmd_or_ctrl = keystroke.modifiers.secondary() || keystroke.modifiers.control;

        // Shield modal dialogs: absorb keystrokes so background shortcuts don't mutate or delete files
        if self.handle_modal_key_down(key, cmd_or_ctrl, cx) {
            return;
        }

        // Global Selection and Search Shortcuts
        if cmd_or_ctrl && key.eq_ignore_ascii_case("a") {
            self.select_all(cx);
            return;
        }

        if cmd_or_ctrl && key.eq_ignore_ascii_case("f") {
            self.search_focused = true;
            self.search_query.clear();
            cx.notify();
            return;
        }

        // Cmd+R: rescan the active library (filesystem watcher is best-effort)
        if cmd_or_ctrl && key.eq_ignore_ascii_case("r") {
            if let Some(ref state) = self.grid_state {
                state.engine.rescan();
            }
            cx.notify();
            return;
        }

        // Inspector Toggle
        if key.eq_ignore_ascii_case("i") {
            self.toggle_inspector(cx);
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

        // Fullscreen Viewer Shortcuts
        if self.is_fullscreen_viewer {
            if self.handle_viewer_key_down(key, cmd_or_ctrl, keystroke, window, cx) {
                return;
            }
            return;
        }

        // Grid & Browser Shortcuts
        self.handle_grid_key_down(key, cmd_or_ctrl, cx);
    }
}
