//! Keyboard command palette (Cmd+K).

use gpui::*;
use lumina_core::{SortDirection, SortField};

use crate::icons::Icon;
use crate::views::inspector::InspectorTab;
use crate::views::sidebar::AlbumSelection;

use crate::app::LuminaApp;

/// Commands shown at once; typing reaches the rest.
pub const MAX_COMMANDS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandId {
    OpenLibrary,
    Rescan,
    Preferences,
    About,
    ShowLicense,
    ShowAcknowledgements,
    EmptyTrash,
    RestoreAll,
    RetryError,
    Shortcuts,
    ToggleSidebar,
    ToggleInspector,
    GetInfo,
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
    /// Filter-bar chip (dims non-matching items).
    Filter(crate::views::search_bar::GridFilter),
    /// Sets the selection's rating; 0 clears it.
    Rate(u8),
    ToggleFavorite,
    TogglePip,
    PipPlayPause,
    NewSmartAlbum,
    EditSmartAlbum,
    DeleteSmartAlbum,
    /// Get Info actions, so they have a keyboard path (§11).
    AdjustDate,
    EditLocation,
    RemoveLocation,
    OpenTagManager,
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

    pub fn icon(&self) -> Icon {
        use CommandId::*;
        match self.id {
            OpenLibrary => Icon::Folder,
            Rescan => Icon::RotateCw,
            Preferences | SortDate | SortName | SortSize | SortRating | SortAsc | SortDesc => {
                Icon::Sliders
            }
            About | ToggleInspector | GetInfo | ShowLicense | ShowAcknowledgements => Icon::Info,
            Shortcuts => Icon::Zap,
            ToggleSidebar => Icon::PanelLeft,
            ExportSelection => Icon::ExternalLink,
            ImportXmp => Icon::ArrowDownToLine,
            ToggleFit => Icon::Maximize,
            ZoomInGrid => Icon::Plus,
            ZoomOutGrid => Icon::Minus,
            Filter(_) => Icon::Search,
            Rate(0) => Icon::Star,
            Rate(_) => Icon::StarFilled,
            ToggleFavorite => Icon::Heart,
            TogglePip => Icon::Pip,
            PipPlayPause => Icon::Play,
            NewSmartAlbum => Icon::Sparkles,
            EditSmartAlbum | AdjustDate | EditLocation => Icon::Pencil,
            RemoveLocation | DeleteSmartAlbum => Icon::X,
            EmptyTrash => Icon::Trash,
            RestoreAll => Icon::Undo2,
            RetryError => Icon::RotateCw,
            OpenTagManager => Icon::Tag,
        }
    }
}

/// What a palette row does when run.
#[derive(Debug, Clone, PartialEq)]
pub enum PaletteTarget {
    Command(CommandId),
    Go(AlbumSelection),
    Year(i32),
}

/// One rendered palette row: a command, or a GO TO destination.
#[derive(Debug, Clone)]
pub struct PaletteRow {
    pub target: PaletteTarget,
    pub title: String,
    pub shortcut: &'static str,
    pub icon: Icon,
    /// Stable `debug_selector`.
    pub selector: String,
}

impl PaletteRow {
    pub fn is_go_to(&self) -> bool {
        !matches!(self.target, PaletteTarget::Command(_))
    }
}

/// Every typed word must appear in the title or keywords ("name sort" finds
/// Sort by Name).
fn matches(words: &[String], title: &str, keywords: &str) -> bool {
    let haystack = format!("{} {keywords}", title.to_lowercase());
    words.iter().all(|word| haystack.contains(word.as_str()))
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
            id: CommandId::EmptyTrash,
            key: "command.empty_trash",
            shortcut: "",
            keywords: "trash recently deleted delete permanently",
        },
        Command {
            id: CommandId::RestoreAll,
            key: "command.restore_all",
            shortcut: "",
            keywords: "trash recently deleted undelete recover",
        },
        Command {
            id: CommandId::RetryError,
            key: "command.retry_error",
            shortcut: "",
            keywords: "error again failed",
        },
        Command {
            id: CommandId::ShowLicense,
            key: "command.show_license",
            shortcut: "",
            keywords: "license mit legal about",
        },
        Command {
            id: CommandId::ShowAcknowledgements,
            key: "command.show_acknowledgements",
            shortcut: "",
            keywords: "acknowledgements credits third party notices licenses about",
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
            id: CommandId::GetInfo,
            key: "command.get_info",
            shortcut: "⌘I",
            keywords: "info window details metadata",
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
        Command {
            id: CommandId::Filter(crate::views::search_bar::GridFilter::All),
            key: "command.filter_all",
            shortcut: "",
            keywords: "filter show everything clear",
        },
        Command {
            id: CommandId::Filter(crate::views::search_bar::GridFilter::Favorites),
            key: "command.filter_favorites",
            shortcut: "",
            keywords: "filter hearts loved",
        },
        Command {
            id: CommandId::Filter(crate::views::search_bar::GridFilter::Videos),
            key: "command.filter_videos",
            shortcut: "",
            keywords: "filter movies clips",
        },
        Command {
            id: CommandId::Filter(crate::views::search_bar::GridFilter::ThreeStars),
            key: "command.filter_three_stars",
            shortcut: "",
            keywords: "filter rating 3 stars best",
        },
        Command {
            id: CommandId::Filter(crate::views::search_bar::GridFilter::Labeled),
            key: "command.filter_labeled",
            shortcut: "",
            keywords: "filter colour color label",
        },
        Command {
            id: CommandId::Rate(1),
            key: "command.rate_1",
            shortcut: "",
            keywords: "rating stars one",
        },
        Command {
            id: CommandId::Rate(2),
            key: "command.rate_2",
            shortcut: "",
            keywords: "rating stars two",
        },
        Command {
            id: CommandId::Rate(3),
            key: "command.rate_3",
            shortcut: "",
            keywords: "rating stars three",
        },
        Command {
            id: CommandId::Rate(4),
            key: "command.rate_4",
            shortcut: "",
            keywords: "rating stars four",
        },
        Command {
            id: CommandId::Rate(5),
            key: "command.rate_5",
            shortcut: "",
            keywords: "rating stars five",
        },
        Command {
            id: CommandId::Rate(0),
            key: "command.clear_rating",
            shortcut: "",
            keywords: "rating stars remove unrate",
        },
        Command {
            id: CommandId::ToggleFavorite,
            key: "command.toggle_favorite",
            shortcut: "F",
            keywords: "heart star like",
        },
        Command {
            id: CommandId::TogglePip,
            key: "command.toggle_pip",
            shortcut: "⌥P",
            keywords: "pip picture in picture video float detach",
        },
        Command {
            id: CommandId::PipPlayPause,
            key: "command.pip_play_pause",
            shortcut: "",
            keywords: "pip picture in picture video pause resume",
        },
        Command {
            id: CommandId::NewSmartAlbum,
            key: "command.new_smart_album",
            shortcut: "⌥⌘N",
            keywords: "smart album rules filter create",
        },
        Command {
            id: CommandId::EditSmartAlbum,
            key: "command.edit_smart_album",
            shortcut: "",
            keywords: "smart album rules filter change",
        },
        Command {
            id: CommandId::DeleteSmartAlbum,
            key: "command.delete_smart_album",
            shortcut: "",
            keywords: "smart album remove delete",
        },
        Command {
            id: CommandId::AdjustDate,
            key: "command.adjust_date",
            shortcut: "",
            keywords: "date taken time edit change get info",
        },
        Command {
            id: CommandId::EditLocation,
            key: "command.edit_location",
            shortcut: "",
            keywords: "location gps coordinates place edit get info",
        },
        Command {
            id: CommandId::RemoveLocation,
            key: "command.remove_location",
            shortcut: "",
            keywords: "location gps coordinates place clear delete",
        },
        Command {
            id: CommandId::OpenTagManager,
            key: "command.open_tag_manager",
            shortcut: "",
            keywords: "tags keywords manage rename",
        },
    ]
}

impl LuminaApp {
    pub fn open_command_palette(&mut self, cx: &mut Context<Self>) {
        self.command_palette_open = true;
        self.command_query.clear();
        self.command_selected = 0;
        self.command_scroll.scroll_to_item(1);
        cx.notify();
    }

    pub fn close_command_palette(&mut self, cx: &mut Context<Self>) {
        self.command_palette_open = false;
        self.command_query.clear();
        cx.notify();
    }

    /// Commands that would do nothing right now stay out of the list.
    fn command_applies(&self, id: CommandId) -> bool {
        match id {
            CommandId::RetryError => self.error_retry.is_some(),
            CommandId::EmptyTrash | CommandId::RestoreAll => self
                .system_album_counts
                .get(&lumina_core::SystemAlbum::RecentlyDeleted)
                .is_some_and(|&n| n > 0),
            _ => true,
        }
    }

    /// COMMANDS (at most eight) then GO TO, filtered by the query.
    pub fn palette_rows(&self) -> Vec<PaletteRow> {
        let words: Vec<String> = self
            .command_query
            .to_lowercase()
            .split_whitespace()
            .map(str::to_string)
            .collect();
        let mut rows: Vec<PaletteRow> = commands()
            .iter()
            .filter(|c| self.command_applies(c.id) && matches(&words, &c.title(), c.keywords))
            .take(MAX_COMMANDS)
            .map(|c| PaletteRow {
                target: PaletteTarget::Command(c.id),
                title: c.title(),
                shortcut: c.shortcut,
                icon: c.icon(),
                selector: format!("command-item-{}", c.key),
            })
            .collect();

        let go = |selection: AlbumSelection, title: String, icon: Icon, slug: String| PaletteRow {
            target: PaletteTarget::Go(selection),
            title,
            shortcut: "",
            icon,
            selector: format!("palette-go-{slug}"),
        };
        let mut go_to = vec![
            go(
                AlbumSelection::AllLibrary,
                "All Photos".into(),
                Icon::Image,
                "all".into(),
            ),
            go(
                AlbumSelection::System(lumina_core::SystemAlbum::Favorites),
                "Favorites".into(),
                Icon::Heart,
                "favorites".into(),
            ),
            go(
                AlbumSelection::System(lumina_core::SystemAlbum::Videos),
                "Videos".into(),
                Icon::Film,
                "videos".into(),
            ),
            go(
                AlbumSelection::Trash,
                "Recently Deleted".into(),
                Icon::Trash,
                "trash".into(),
            ),
            go(
                AlbumSelection::Untagged,
                "Untagged".into(),
                Icon::Tag,
                "untagged".into(),
            ),
            go(
                AlbumSelection::Duplicates,
                "Duplicates".into(),
                Icon::Repeat,
                "duplicates".into(),
            ),
        ];
        go_to.extend(
            self.timeline_year_buckets
                .iter()
                .map(|(year, _)| PaletteRow {
                    target: PaletteTarget::Year(*year),
                    title: year.to_string(),
                    shortcut: "",
                    icon: Icon::Image,
                    selector: format!("palette-go-year-{year}"),
                }),
        );
        go_to.extend(self.library_tag_counts.iter().map(|(tag, _)| {
            go(
                AlbumSelection::Tag(tag.clone()),
                format!("#{tag}"),
                Icon::Tag,
                format!("tag-{tag}"),
            )
        }));
        go_to.extend(self.user_smart_albums.iter().map(|album| {
            go(
                AlbumSelection::User(album.id),
                album.title.clone(),
                Icon::Sparkles,
                format!("album-{}", album.id),
            )
        }));
        rows.extend(
            go_to
                .into_iter()
                .filter(|row| matches(&words, &row.title, "go to album")),
        );
        rows
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
            PaletteTarget::Year(year) => {
                self.close_command_palette(cx);
                // One history entry: Back returns to where the jump started.
                let departing = self.navigation_snapshot();
                self.selection = AlbumSelection::AllLibrary;
                self.active_year = Some(year);
                self.search_query.clear();
                self.search_focused = false;
                self.clear_selection(cx);
                self.record_navigation_change(departing);
            }
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
            CommandId::ShowLicense => self.open_bundled_doc("LICENSE", cx),
            CommandId::EmptyTrash => self.prompt_empty_trash(cx),
            CommandId::RestoreAll => self.restore_all_trash(cx),
            CommandId::RetryError => self.retry_error(cx),
            CommandId::ShowAcknowledgements => self.open_bundled_doc("THIRD_PARTY_NOTICES.md", cx),
            CommandId::Shortcuts => self.toggle_shortcuts_overlay(cx),
            CommandId::ToggleSidebar => self.toggle_sidebar(cx),
            CommandId::ToggleInspector => self.toggle_inspector(cx),
            CommandId::GetInfo => self.get_info(cx),
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
            CommandId::Filter(filter) => {
                self.grid_filter = filter;
                cx.notify();
            }
            CommandId::Rate(rating) => self.set_rating_selected(rating, cx),
            CommandId::ToggleFavorite if self.selected_indices.len() > 1 => {
                self.batch_toggle_favorite(cx)
            }
            CommandId::ToggleFavorite => self.toggle_favorite_selected(cx),
            CommandId::TogglePip => self.toggle_pip(cx),
            CommandId::PipPlayPause => self.toggle_pip_playback(cx),
            CommandId::NewSmartAlbum => self.open_album_builder(cx),
            CommandId::EditSmartAlbum => self.edit_selected_smart_album(cx),
            CommandId::DeleteSmartAlbum => self.delete_selected_smart_album(cx),
            CommandId::AdjustDate => {
                self.open_get_info_at(InspectorTab::Info, cx);
                self.start_date_input(cx);
            }
            CommandId::EditLocation => {
                self.open_get_info_at(InspectorTab::Location, cx);
                self.start_location_input(cx);
            }
            CommandId::RemoveLocation if self.selected_indices.len() <= 1 => {
                self.remove_location_selected(cx)
            }
            CommandId::RemoveLocation => self.set_error(
                "Remove Location works on one item at a time.".to_string(),
                cx,
            ),
            CommandId::OpenTagManager => self.open_tag_manager(cx),
        }
    }
}
