//! Keyboard command palette (Cmd+K).

mod exec;
mod ids;
mod rows;
mod table;

pub use ids::{Command, CommandId};
pub use rows::{PaletteRow, PaletteTarget};
pub use table::commands;

use gpui::*;

use crate::app::LuminaApp;

/// Commands shown at once; typing reaches the rest.
pub const MAX_COMMANDS: usize = 8;

impl LuminaApp {
    pub fn open_command_palette(&mut self, cx: &mut Context<Self>) {
        // Commands act on the grid; over the template editor they would
        // change media behind the sheet (the menu item reaches here, not the
        // keymap). Nor over edit mode: they would act on the photo under the
        // draft. The Tags page holds no media selection, so ⌘K works there.
        // Nor over settings: they would act on the library hidden under it.
        if self.templates.editor.is_some() || self.edits.session.is_some() || self.preferences_open
        {
            return;
        }
        self.command_palette_open = true;
        self.templates.palette_pick = false;
        self.command_query.clear();
        self.command_selected = 0;
        self.command_scroll.scroll_to_item(1);
        cx.notify();
    }

    pub fn close_command_palette(&mut self, cx: &mut Context<Self>) {
        self.command_palette_open = false;
        self.templates.palette_pick = false;
        self.command_query.clear();
        cx.notify();
    }

    /// Arrow keys clamp at the ends rather than wrapping.
    pub fn move_command_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        let rows = self.palette_rows();
        if rows.is_empty() {
            return;
        }
        let current = self.command_selected.min(rows.len() - 1) as isize;
        let next = (current + delta).clamp(0, rows.len() as isize - 1) as usize;
        self.command_selected = next;
        // Group headings are list children too: one before the first row,
        // one more before GO TO when commands precede it.
        let headings = 1 + usize::from(rows[next].is_go_to() && !rows[0].is_go_to());
        self.command_scroll.scroll_to_item(next + headings);
        cx.notify();
    }

    pub fn execute_selected_command(&mut self, cx: &mut Context<Self>) {
        let rows = self.palette_rows();
        if let Some(row) = rows.get(self.command_selected.min(rows.len().saturating_sub(1))) {
            self.run_palette_target(row.target.clone(), cx);
        }
    }

    pub fn run_palette_target(&mut self, target: PaletteTarget, cx: &mut Context<Self>) {
        match target {
            PaletteTarget::Command(id) => self.execute_command(id, cx),
            PaletteTarget::Go(selection) => {
                self.close_command_palette(cx);
                self.select_album(selection, cx);
            }
            PaletteTarget::Library(path) => {
                self.close_command_palette(cx);
                self.open_recent_library(path, cx);
            }
            PaletteTarget::Template(index) => {
                self.close_command_palette(cx);
                self.apply_template(index, cx);
            }
            PaletteTarget::AudioTrack(index) => {
                self.close_command_palette(cx);
                self.select_audio_track(index, cx);
            }
            PaletteTarget::Year(year) => {
                self.close_command_palette(cx);
                // The year filter; the rail scrolls instead. One history
                // entry: Back returns to where the jump started.
                self.set_active_year(Some(year), cx);
            }
        }
    }
}
