//! Keys on the Tags page. Its rename and alias fields are text fields
//! (`keymap/modals.rs`); typing filters through the field input.

use gpui::*;

use crate::app::types::{TagRowAction, TagSortOrder};
use crate::app::LuminaApp;

impl LuminaApp {
    /// The Tags page's keys: ↑/↓ focus a keyword, ↵ renames it, Space checks
    /// it, ⌘A / ⌘M / ⌘⌫ act on the checked ones, ⇧⌘A / ⇧⌘M alias and move
    /// it, ⌘Z / ⇧⌘Z undo and redo, Esc clears the checks, then the filter
    /// (and otherwise does nothing, as on the Albums overview). ⇧F10 opens
    /// the focused row's `…` menu. While the sort or row menu is open it
    /// takes ↑/↓, ↵ / Space and Esc. Every other key is absorbed, so
    /// the grid's shortcuts (⌫, F, ratings, ⌘E…) never reach media behind
    /// the page; [`passes_through`] lists the global ones that still work.
    pub(crate) fn handle_tags_page_key(
        &mut self,
        keystroke: &Keystroke,
        cmd_or_ctrl: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.tags_page_shown() || self.is_fullscreen_viewer {
            return false;
        }
        let key = keystroke.key.as_str();
        let shift = keystroke.modifiers.shift;
        if let Some(at) = self.tag_manager.sort_menu {
            let last = TagSortOrder::ALL.len() - 1;
            match key {
                "up" => self.tag_manager.sort_menu = Some(at.saturating_sub(1)),
                "down" => self.tag_manager.sort_menu = Some((at + 1).min(last)),
                "enter" | "return" | "space" => self.set_tag_sort_order(TagSortOrder::ALL[at], cx),
                "escape" => self.tag_manager.sort_menu = None,
                _ => return !passes_through(keystroke, cmd_or_ctrl),
            }
            cx.notify();
            return true;
        }
        if let Some((tag, at)) = self.tag_manager.row_menu.clone() {
            let last = TagRowAction::ALL.len() - 1;
            match key {
                "up" => self.tag_manager.row_menu = Some((tag, at.saturating_sub(1))),
                "down" => self.tag_manager.row_menu = Some((tag, (at + 1).min(last))),
                "enter" | "return" | "space" => {
                    self.run_tag_row_action(tag, TagRowAction::ALL[at], cx)
                }
                "escape" => self.tag_manager.row_menu = None,
                _ => return !passes_through(keystroke, cmd_or_ctrl),
            }
            cx.notify();
            return true;
        }
        match key {
            // macOS keyboards have no context-menu key; ⇧F10 stands in.
            "f10" if shift => {
                if let Some(tag) = self.tag_manager.focused.clone() {
                    self.open_tag_row_menu(tag, cx);
                }
            }
            // Rename, merge and delete happen here, so their undo does too.
            // The history step reloads the counts and detail panel.
            "z" if cmd_or_ctrl && shift => self.redo_last(cx),
            "z" if cmd_or_ctrl => self.undo_last(cx),
            "a" if cmd_or_ctrl && shift => self.start_alias_input(cx),
            "m" if cmd_or_ctrl && shift => self.start_keyword_move(cx),
            "a" if cmd_or_ctrl => self.toggle_all_tag_manager_rows(cx),
            "m" if cmd_or_ctrl => self.merge_checked_tags(cx),
            // Only with an empty filter (⌘⌫ edits text) and the bar showing.
            "backspace"
                if cmd_or_ctrl
                    && self.tag_manager.search.is_empty()
                    && self.tag_manager.selected.len() >= 2 =>
            {
                let tags = self.tag_manager.selected.iter().cloned().collect();
                self.bulk_delete_tags(tags, cx);
            }
            "space" if self.tag_manager.search.is_empty() => {
                // Not a filter character: keep it from the input handler.
                cx.stop_propagation();
                if let Some(tag) = self.tag_manager.focused.clone() {
                    self.toggle_tag_manager_selection(tag, cx);
                }
            }
            "up" | "down" if !cmd_or_ctrl => {
                self.step_tag_manager_focus(if key == "up" { -1 } else { 1 }, cx);
            }
            "enter" | "return" => {
                if let Some(tag) = self.tag_manager.focused.clone() {
                    self.start_modal_tag_rename(tag, cx);
                }
            }
            "escape" if !self.tag_manager.selected.is_empty() => {
                self.clear_tag_manager_selection(cx);
            }
            "escape" if !self.tag_manager.search.is_empty() => {
                self.tag_manager.search.clear();
                cx.notify();
            }
            _ => return !passes_through(keystroke, cmd_or_ctrl),
        }
        true
    }
}

/// Global shortcuts that open a window, sheet or another place and never
/// act on the media selection: they keep working on the Tags page. ⌘[ / ⌘]
/// are handled before the page.
fn passes_through(keystroke: &Keystroke, cmd_or_ctrl: bool) -> bool {
    let m = &keystroke.modifiers;
    if !cmd_or_ctrl {
        return false;
    }
    match keystroke.key.to_ascii_lowercase().as_str() {
        // Shortcut sheet, palette, New Album / Smart Album, Open Library,
        // Preferences, Find.
        "/" | "k" | "n" | "o" | "," | "f" => true,
        // ⌘R rescans; ⇧⌘R reveals the selection.
        "r" => !m.shift,
        // ⇧⌘I imports; ⌘I is Get Info on the selection.
        "i" => m.shift,
        // ⌥⌘U import from a link, ⌥⌘L downloads.
        "u" | "l" => m.alt,
        // ⌃⌘S sidebar; ⌥⌘S is a slideshow of the view.
        "s" => m.control && m.platform,
        _ => false,
    }
}
