//! Keyboard command palette (Cmd+K).

use gpui::*;
use lumina_core::{SortDirection, SortField};

use crate::app::LuminaApp;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandId {
    OpenLibrary,
    Rescan,
    Preferences,
    About,
    Shortcuts,
    ToggleSidebar,
    ToggleInspector,
    ExportSelection,
    ImportXmp,
    SortDate,
    SortName,
    SortSize,
    SortRating,
    SortAsc,
    SortDesc,
    ToggleFit,
    ZoomInGrid,
    ZoomOutGrid,
}

pub struct Command {
    pub id: CommandId,
    pub key: &'static str,
    pub shortcut: &'static str,
    pub keywords: &'static str,
}

impl Command {
    pub fn title(&self) -> String {
        crate::i18n::t(self.key)
    }
}

pub fn commands() -> &'static [Command] {
    &[
        Command {
            id: CommandId::OpenLibrary,
            key: "command.open_library",
            shortcut: "⌘O",
            keywords: "folder choose switch library",
        },
        Command {
            id: CommandId::Rescan,
            key: "command.rescan",
            shortcut: "⌘R",
            keywords: "refresh index scan",
        },
        Command {
            id: CommandId::Preferences,
            key: "command.preferences",
            shortcut: "⌘,",
            keywords: "settings config cache",
        },
        Command {
            id: CommandId::About,
            key: "command.about",
            shortcut: "",
            keywords: "version info",
        },
        Command {
            id: CommandId::Shortcuts,
            key: "command.shortcuts",
            shortcut: "⌘/",
            keywords: "help keys",
        },
        Command {
            id: CommandId::ToggleSidebar,
            key: "command.toggle_sidebar",
            shortcut: "",
            keywords: "panel hide show",
        },
        Command {
            id: CommandId::ToggleInspector,
            key: "command.toggle_inspector",
            shortcut: "I",
            keywords: "panel info hide show",
        },
        Command {
            id: CommandId::ExportSelection,
            key: "command.export",
            shortcut: "",
            keywords: "download copy files xmp",
        },
        Command {
            id: CommandId::ImportXmp,
            key: "command.import_xmp",
            shortcut: "",
            keywords: "sidecar metadata read",
        },
        Command {
            id: CommandId::SortDate,
            key: "command.sort_date",
            shortcut: "",
            keywords: "order newest oldest",
        },
        Command {
            id: CommandId::SortName,
            key: "command.sort_name",
            shortcut: "",
            keywords: "order alphabetical filename",
        },
        Command {
            id: CommandId::SortSize,
            key: "command.sort_size",
            shortcut: "",
            keywords: "order bytes disk",
        },
        Command {
            id: CommandId::SortRating,
            key: "command.sort_rating",
            shortcut: "",
            keywords: "order stars",
        },
        Command {
            id: CommandId::SortAsc,
            key: "command.sort_asc",
            shortcut: "",
            keywords: "direction increasing",
        },
        Command {
            id: CommandId::SortDesc,
            key: "command.sort_desc",
            shortcut: "",
            keywords: "direction decreasing",
        },
        Command {
            id: CommandId::ToggleFit,
            key: "command.toggle_fit",
            shortcut: "",
            keywords: "cards square aspect",
        },
        Command {
            id: CommandId::ZoomInGrid,
            key: "command.zoom_in",
            shortcut: "⌘+",
            keywords: "columns density bigger",
        },
        Command {
            id: CommandId::ZoomOutGrid,
            key: "command.zoom_out",
            shortcut: "⌘-",
            keywords: "columns density smaller",
        },
    ]
}

impl LuminaApp {
    pub fn open_command_palette(&mut self, cx: &mut Context<Self>) {
        self.command_palette_open = true;
        self.command_query.clear();
        self.command_selected = 0;
        cx.notify();
    }

    pub fn close_command_palette(&mut self, cx: &mut Context<Self>) {
        self.command_palette_open = false;
        self.command_query.clear();
        cx.notify();
    }

    pub fn filtered_commands(&self) -> Vec<&'static Command> {
        let query = self.command_query.trim().to_lowercase();
        commands()
            .iter()
            .filter(|command| {
                query.is_empty()
                    || command.title().to_lowercase().contains(&query)
                    || command.keywords.contains(&query)
            })
            .collect()
    }

    pub fn move_command_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        let count = self.filtered_commands().len();
        if count == 0 {
            return;
        }
        let current = self.command_selected.min(count - 1) as isize;
        let next = (current + delta).rem_euclid(count as isize) as usize;
        self.command_selected = next;
        cx.notify();
    }

    pub fn execute_selected_command(&mut self, cx: &mut Context<Self>) {
        let commands = self.filtered_commands();
        if let Some(command) = commands
            .get(self.command_selected.min(commands.len().saturating_sub(1)))
            .copied()
        {
            self.execute_command(command.id, cx);
        }
    }

    pub fn execute_command(&mut self, id: CommandId, cx: &mut Context<Self>) {
        self.command_palette_open = false;
        self.command_query.clear();
        match id {
            CommandId::OpenLibrary => self.pick_folder_dialog(cx),
            CommandId::Rescan => self.rescan_library(cx),
            CommandId::Preferences => self.open_preferences(cx),
            CommandId::About => self.open_about(cx),
            CommandId::Shortcuts => self.toggle_shortcuts_overlay(cx),
            CommandId::ToggleSidebar => self.toggle_sidebar(cx),
            CommandId::ToggleInspector => self.toggle_inspector(cx),
            CommandId::ExportSelection => self.export_pick_folder(cx),
            CommandId::ImportXmp => self.import_sidecars_selected(cx),
            CommandId::SortDate => self.set_sort_field(SortField::DateTaken, cx),
            CommandId::SortName => self.set_sort_field(SortField::FileName, cx),
            CommandId::SortSize => self.set_sort_field(SortField::FileSize, cx),
            CommandId::SortRating => self.set_sort_field(SortField::Rating, cx),
            CommandId::SortAsc => self.set_sort_direction(SortDirection::Ascending, cx),
            CommandId::SortDesc => self.set_sort_direction(SortDirection::Descending, cx),
            CommandId::ToggleFit => {
                self.card_fit_mode = !self.card_fit_mode;
                self.save_config();
                cx.notify();
            }
            CommandId::ZoomInGrid => self.zoom_in_grid(cx),
            CommandId::ZoomOutGrid => self.zoom_out_grid(cx),
        }
    }
}
