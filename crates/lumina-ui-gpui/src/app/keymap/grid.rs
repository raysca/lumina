use gpui::*;

use crate::app::LuminaApp;
use crate::views::sidebar::AlbumSelection;

impl LuminaApp {
    /// Handles grid navigation, zoom, selection, and contextual shortcuts.
    pub fn handle_grid_key_down(&mut self, key: &str, cmd_or_ctrl: bool, cx: &mut Context<Self>) {
        if !cmd_or_ctrl && key.eq_ignore_ascii_case("f") {
            if self.selected_indices.len() > 1 {
                self.batch_toggle_favorite(cx);
            } else {
                self.toggle_favorite_selected(cx);
            }
            return;
        }

        if !cmd_or_ctrl && (key == "backspace" || key == "delete") {
            if self.selection == AlbumSelection::Trash {
                self.prompt_delete_selected(cx);
            } else if self.selected_indices.len() > 1 {
                self.batch_trash_or_restore(cx);
            } else {
                self.trash_or_restore_selected(cx);
            }
            return;
        }

        // Grid Zoom shortcuts: Cmd/Ctrl + and Cmd/Ctrl -
        if cmd_or_ctrl && (key == "=" || key == "+") {
            self.zoom_in_grid(cx);
            return;
        }

        if cmd_or_ctrl && (key == "-" || key == "_") {
            self.zoom_out_grid(cx);
            return;
        }

        if key == "home" {
            self.select_index(0, cx);
            return;
        }

        if key == "end" {
            let count = self.get_active_count();
            if count > 0 {
                self.select_index(count - 1, cx);
            }
            return;
        }

        if key == "pageup" {
            let step = (self.grid_columns.clamp(2, 8) * 4) as isize;
            self.navigate_selection(-step, cx);
            return;
        }

        if key == "pagedown" {
            let step = (self.grid_columns.clamp(2, 8) * 4) as isize;
            self.navigate_selection(step, cx);
            return;
        }

        if key == "left" {
            self.navigate_selection(-1, cx);
            return;
        }

        if key == "right" {
            self.navigate_selection(1, cx);
            return;
        }

        if key == "space" {
            if let Some(ref item) = self.selected_item {
                self.open_viewer(item.clone(), cx);
                return;
            } else if self.get_active_count() > 0 {
                self.select_index(0, cx);
                if let Some(ref item) = self.selected_item {
                    self.open_viewer(item.clone(), cx);
                }
                return;
            }
        }

        if key == "escape" {
            if self.context_menu_state.is_some() {
                self.close_context_menu(cx);
                return;
            }
            if self.selected_indices.len() > 1 {
                self.clear_selection(cx);
            } else if self.inspector_open {
                self.inspector_open = false;
                cx.notify();
            } else if !self.search_query.is_empty() {
                self.clear_search(cx);
            } else if self.selected_item.is_some() {
                self.clear_selection(cx);
            }
            return;
        }

        if key == "up" {
            self.navigate_selection(-(self.grid_columns as isize), cx);
            return;
        }

        if key == "down" {
            self.navigate_selection(self.grid_columns as isize, cx);
        }
    }
}
