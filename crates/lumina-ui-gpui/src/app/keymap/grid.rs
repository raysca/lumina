use gpui::*;

use crate::app::LuminaApp;
use crate::views::sidebar::AlbumSelection;
use crate::views::viewer_state::ViewerRotation;

impl LuminaApp {
    /// Handles grid navigation, zoom, selection, and contextual shortcuts.
    pub fn handle_grid_key_down(
        &mut self,
        key: &str,
        cmd_or_ctrl: bool,
        shift: bool,
        alt: bool,
        cx: &mut Context<Self>,
    ) {
        if !cmd_or_ctrl && key.eq_ignore_ascii_case("f") {
            if self.selected_indices.len() > 1 {
                self.batch_toggle_favorite(cx);
            } else {
                self.toggle_favorite_selected(cx);
            }
            return;
        }

        // T: tag the selection (the context menu and dock "Tag" shortcut)
        if !cmd_or_ctrl && key.eq_ignore_ascii_case("t") {
            if self.selected_item.is_some() {
                self.begin_tagging_selection(cx);
            }
            return;
        }

        // C: compare the 2-9 selected items.
        if !cmd_or_ctrl && !alt && !shift && key.eq_ignore_ascii_case("c") {
            self.open_compare(cx);
            return;
        }

        // R / ⇧R: rotate the selected stills right / left.
        if !cmd_or_ctrl && !alt && key.eq_ignore_ascii_case("r") {
            let direction = if shift {
                ViewerRotation::Left
            } else {
                ViewerRotation::Right
            };
            self.rotate_selection(direction, cx);
            return;
        }

        // ⌫ in an album takes the selection out of the album and never
        // trashes; ⌘⌫ (the menu's shortcut), and ⌫ anywhere else, open the
        // Delete dialog.
        if key == "backspace" || key == "delete" {
            if !cmd_or_ctrl && matches!(self.selection, AlbumSelection::Album(_)) {
                self.remove_selection_from_album(cx);
            } else {
                self.prompt_delete_selected(cx);
            }
            return;
        }

        // Shift+arrow extends the selection from the anchor without moving it.
        if shift && !cmd_or_ctrl {
            let handled = match key {
                "left" => {
                    self.extend_selection(-1, cx);
                    true
                }
                "right" => {
                    self.extend_selection(1, cx);
                    true
                }
                "up" => {
                    self.extend_selection(-(self.grid_columns as isize), cx);
                    true
                }
                "down" => {
                    self.extend_selection(self.grid_columns as isize, cx);
                    true
                }
                "home" => {
                    self.extend_selection_to(0, cx);
                    true
                }
                "end" => {
                    let count = self.get_active_count();
                    if count > 0 {
                        self.extend_selection_to(count - 1, cx);
                    }
                    true
                }
                _ => false,
            };
            if handled {
                return;
            }
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
