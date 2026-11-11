//! Keyboard command palette (Cmd+K).

use gpui::*;
use lumina_core::{SortDirection, SortField};

use crate::icons::Icon;
use crate::views::inspector::InspectorTab;
use crate::views::sidebar::AlbumSelection;
use crate::views::viewer_state::{ViewerRotation, ViewerZoomPreset};

use crate::app::LuminaApp;
use crate::i18n::t;

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
    MergeAllDuplicates,
    /// The Duplicates destination on its Exact copies / Similar shots tab.
    ShowExactCopies,
    ShowSimilarShots,
    /// Places on the map / on the grid.
    ShowMap,
    ShowGrid,
    RetryError,
    Shortcuts,
    ToggleSidebar,
    ToggleInspector,
    GetInfo,
    ExportSelection,
    ExportJpeg,
    /// The selected originals to the system share picker (macOS only).
    Share,
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
    /// Viewer zoom preset; offered only while a still is open in the viewer.
    ViewerZoom(ViewerZoomPreset),
    /// Quick filter: toggles this query token in the search (D8).
    QuickFilter(&'static str),
    /// Removes every filter token from the search, keeping its text.
    ClearFilters,
    /// Sets the selection's rating; 0 clears it.
    Rate(u8),
    /// Picks, rejects or unflags the selection (P / X / U).
    Flag(lumina_core::Flag),
    /// The current view's rejects, to the Delete dialog.
    DeleteRejected,
    /// Compare / survey of the 2-9 selected items (C in the grid).
    Compare,
    /// The current view in the immersive viewer (⌥⌘S).
    Slideshow,
    /// Toggles whether the slideshow wraps at the end.
    SlideshowLoop,
    /// Seconds per slide (3, 5 or 10).
    SlideshowInterval(u8),
    ToggleFavorite,
    /// Quarter turn of the viewer item or the selected stills.
    RotateLeft,
    RotateRight,
    /// Edit mode on the viewer photo (E in the viewer).
    EditPhoto,
    /// Clears rotation and edits on the viewer photo or the selection.
    RevertToOriginal,
    /// The one selected item in the external editor, in place or a copy.
    EditInExternalEditor,
    EditCopyInExternalEditor,
    TogglePip,
    PipPlayPause,
    /// Loop range of the open video; shown only while one is in the viewer.
    SetLoopStart,
    SetLoopEnd,
    ClearLoopRange,
    /// New Album (⌘N), always empty; New Album with Selection holds the
    /// selected items (shown only with a selection).
    NewAlbum,
    NewAlbumWithSelection,
    /// Act on the open album; shown only in an album view.
    RenameAlbum,
    DeleteAlbum,
    /// The Add to Album picker (⇧⌘A); shown only with a selection.
    AddToAlbum,
    /// Takes the selection out of the open album (⌫ there).
    RemoveFromAlbum,
    /// Pin or unpin the open album or smart album; one shows at a time.
    PinAlbum,
    UnpinAlbum,
    /// Act on the open smart album; shown only in a smart album view.
    DuplicateSmartAlbum,
    ExportSmartAlbumRules,
    ImportSmartAlbumRules,
    NewSmartAlbum,
    /// Saves the search as a smart album; shown only while there is a query.
    SaveSearch,
    EditSmartAlbum,
    DeleteSmartAlbum,
    LocateMissing,
    ImportMedia,
    ImportNetworkVideo,
    ToggleDownloads,
    CancelImport,
    /// Get Info actions, so they have a keyboard path (§11).
    AdjustDate,
    EditLocation,
    RemoveLocation,
    OpenTagManager,
    /// Act on the tag manager's focused keyword (or the sidebar's tag view);
    /// shown only when there is one.
    AddKeywordAlias,
    MoveKeyword,
    /// Browsing history; shown only when it would move.
    Back,
    Forward,
    /// Forgets every recent library; shown only when there are some.
    ClearRecents,
    /// Forgets the recent-search history; shown only when there is some.
    ClearRecentSearches,
    /// Lists template names to apply to the selection; shown only when
    /// there are templates and a selection.
    ApplyTemplate,
    /// Opens the metadata template editor.
    ManageTemplates,
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
            ExportSelection | ExportJpeg | Share => Icon::ExternalLink,
            ImportXmp => Icon::ArrowDownToLine,
            ToggleFit => Icon::Maximize,
            ZoomInGrid => Icon::Plus,
            ZoomOutGrid => Icon::Minus,
            ViewerZoom(ViewerZoomPreset::Fit) => Icon::Minimize,
            ViewerZoom(_) => Icon::Maximize,
            QuickFilter(_) => Icon::Search,
            ClearFilters => Icon::X,
            Rate(0) => Icon::Star,
            Rate(_) => Icon::StarFilled,
            Flag(lumina_core::Flag::Pick) => Icon::Check,
            Flag(lumina_core::Flag::Reject) => Icon::X,
            Flag(lumina_core::Flag::Unflagged) => Icon::Minus,
            DeleteRejected => Icon::Trash,
            Compare => Icon::Images,
            Slideshow | SlideshowInterval(_) => Icon::Play,
            SlideshowLoop => Icon::Repeat,
            ToggleFavorite => Icon::Heart,
            RotateLeft => Icon::RotateCcw,
            RotateRight => Icon::RotateCw,
            EditPhoto => Icon::Sliders,
            RevertToOriginal => Icon::Undo2,
            EditInExternalEditor | EditCopyInExternalEditor => Icon::ExternalLink,
            TogglePip => Icon::Pip,
            PipPlayPause => Icon::Play,
            SetLoopStart | SetLoopEnd | ClearLoopRange => Icon::Repeat,
            NewAlbum | NewAlbumWithSelection | AddToAlbum => Icon::Plus,
            NewSmartAlbum | SaveSearch | DuplicateSmartAlbum => Icon::Sparkles,
            PinAlbum | UnpinAlbum => Icon::Star,
            ExportSmartAlbumRules => Icon::ExternalLink,
            ImportSmartAlbumRules => Icon::ArrowDownToLine,
            EditSmartAlbum | RenameAlbum | AdjustDate | EditLocation => Icon::Pencil,
            RemoveLocation | DeleteSmartAlbum | DeleteAlbum | RemoveFromAlbum => Icon::X,
            EmptyTrash => Icon::Trash,
            RestoreAll => Icon::Undo2,
            MergeAllDuplicates | ShowExactCopies | ShowSimilarShots => Icon::Repeat,
            ShowMap => Icon::MapPin,
            ShowGrid => Icon::Images,
            LocateMissing => Icon::TriangleAlert,
            ImportMedia | ImportNetworkVideo | ToggleDownloads => Icon::ArrowDownToLine,
            CancelImport => Icon::X,
            RetryError => Icon::RotateCw,
            OpenTagManager | AddKeywordAlias | ApplyTemplate => Icon::Tag,
            MoveKeyword | ManageTemplates => Icon::Pencil,
            ClearRecents | ClearRecentSearches => Icon::X,
            Back => Icon::ChevronLeft,
            Forward => Icon::ChevronRight,
        }
    }
}

/// What a palette row does when run.
#[derive(Debug, Clone, PartialEq)]
pub enum PaletteTarget {
    Command(CommandId),
    Go(AlbumSelection),
    Year(i32),
    /// Open Recent Library: switches to that library.
    Library(std::path::PathBuf),
    /// Apply Metadata Template…: applies template `index` to the selection.
    Template(usize),
    /// Audio Track: {name}: plays track `index` of the open video.
    AudioTrack(usize),
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
        matches!(
            self.target,
            PaletteTarget::Go(_) | PaletteTarget::Year(_) | PaletteTarget::Library(_)
        )
    }

    /// The heading the row is listed under.
    pub fn group(&self) -> &'static str {
        match self.target {
            PaletteTarget::Command(_) | PaletteTarget::AudioTrack(_) => "COMMANDS",
            PaletteTarget::Template(_) => "TEMPLATES",
            _ => "GO TO",
        }
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
            id: CommandId::ImportMedia,
            key: "command.import_media",
            shortcut: "⇧⌘I",
            keywords: "import copy add files folder card camera",
        },
        Command {
            id: CommandId::ImportNetworkVideo,
            key: "command.import_network_video",
            shortcut: "⌥⌘U",
            keywords: "import video url youtube hls download stream network link web",
        },
        Command {
            id: CommandId::ToggleDownloads,
            key: "command.toggle_downloads",
            shortcut: "⌥⌘L",
            keywords: "downloads queue progress url video",
        },
        Command {
            id: CommandId::CancelImport,
            key: "command.cancel_import",
            shortcut: "",
            keywords: "import stop abort copy",
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
            id: CommandId::MergeAllDuplicates,
            key: "command.merge_all_duplicates",
            shortcut: "",
            keywords: "duplicate duplicates merge clean dedup",
        },
        Command {
            id: CommandId::ShowExactCopies,
            key: "command.show_exact_copies",
            shortcut: "",
            keywords: "duplicates exact copies identical",
        },
        Command {
            id: CommandId::ShowSimilarShots,
            key: "command.show_similar_shots",
            shortcut: "",
            keywords: "duplicates similar shots stacks raw jpeg pairs bursts",
        },
        Command {
            id: CommandId::ShowMap,
            key: "command.show_map",
            shortcut: "",
            keywords: "map places location gps world where",
        },
        Command {
            id: CommandId::ShowGrid,
            key: "command.show_grid",
            shortcut: "",
            keywords: "grid places photos thumbnails map",
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
            // Titled by state in `command_title`: Hide or Show Sidebar.
            key: "command.toggle_sidebar",
            shortcut: crate::app::panels::SIDEBAR_SHORTCUT,
            keywords: "panel hide show toggle sidebar",
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
        // Before Export as JPEG…, whose keywords also say "share".
        Command {
            id: CommandId::Share,
            key: "command.share",
            shortcut: "",
            keywords: "send airdrop mail messages",
        },
        Command {
            id: CommandId::ExportSelection,
            key: "command.export",
            shortcut: "",
            keywords: "download copy files xmp originals",
        },
        Command {
            id: CommandId::ExportJpeg,
            key: "command.export_jpeg",
            shortcut: "",
            keywords: "download convert jpg smaller share",
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
            id: CommandId::ViewerZoom(ViewerZoomPreset::Fit),
            key: "command.zoom_fit",
            shortcut: "⌘0",
            keywords: "zoom viewer fit window whole reset",
        },
        Command {
            id: CommandId::ViewerZoom(ViewerZoomPreset::Actual),
            key: "command.zoom_actual",
            shortcut: "⌘1",
            keywords: "zoom viewer 100% 1:1 pixels actual",
        },
        Command {
            id: CommandId::ViewerZoom(ViewerZoomPreset::TwoX),
            key: "command.zoom_200",
            shortcut: "⌘2",
            keywords: "zoom viewer 200% 2x magnify",
        },
        Command {
            id: CommandId::ViewerZoom(ViewerZoomPreset::Fill),
            key: "command.zoom_fill",
            shortcut: "⌘3",
            keywords: "zoom viewer fill crop cover",
        },
        Command {
            id: CommandId::ViewerZoom(ViewerZoomPreset::FourX),
            key: "command.zoom_400",
            shortcut: "⌘4",
            keywords: "zoom viewer 400% 4x magnify",
        },
        Command {
            id: CommandId::Rate(1),
            key: "command.rate_1",
            shortcut: "1",
            keywords: "rating stars one",
        },
        Command {
            id: CommandId::Rate(2),
            key: "command.rate_2",
            shortcut: "2",
            keywords: "rating stars two",
        },
        Command {
            id: CommandId::Rate(3),
            key: "command.rate_3",
            shortcut: "3",
            keywords: "rating stars three",
        },
        Command {
            id: CommandId::Rate(4),
            key: "command.rate_4",
            shortcut: "4",
            keywords: "rating stars four",
        },
        Command {
            id: CommandId::Rate(5),
            key: "command.rate_5",
            shortcut: "5",
            keywords: "rating stars five",
        },
        Command {
            id: CommandId::Rate(0),
            key: "command.clear_rating",
            shortcut: "0",
            keywords: "rating stars remove unrate",
        },
        Command {
            id: CommandId::Flag(lumina_core::Flag::Pick),
            key: "command.flag_pick",
            shortcut: "P",
            keywords: "flag cull keep select",
        },
        Command {
            id: CommandId::Flag(lumina_core::Flag::Reject),
            key: "command.flag_reject",
            shortcut: "X",
            keywords: "flag cull discard",
        },
        Command {
            id: CommandId::Flag(lumina_core::Flag::Unflagged),
            key: "command.flag_clear",
            shortcut: "U",
            keywords: "flag cull unflag unpick unreject remove",
        },
        Command {
            id: CommandId::DeleteRejected,
            key: "command.delete_rejected",
            shortcut: "",
            keywords: "flag cull reject rejects rejected trash remove",
        },
        Command {
            id: CommandId::Compare,
            key: "command.compare",
            shortcut: "C",
            keywords: "compare survey side by side cull",
        },
        Command {
            id: CommandId::Slideshow,
            key: "command.slideshow",
            shortcut: "⌥⌘S",
            keywords: "slideshow slide show play present full screen",
        },
        Command {
            id: CommandId::SlideshowLoop,
            key: "command.slideshow_loop",
            shortcut: "",
            keywords: "slideshow loop repeat wrap",
        },
        Command {
            id: CommandId::SlideshowInterval(3),
            key: "command.slideshow_interval_3",
            shortcut: "",
            keywords: "slideshow interval seconds speed timing",
        },
        Command {
            id: CommandId::SlideshowInterval(5),
            key: "command.slideshow_interval_5",
            shortcut: "",
            keywords: "slideshow interval seconds speed timing",
        },
        Command {
            id: CommandId::SlideshowInterval(10),
            key: "command.slideshow_interval_10",
            shortcut: "",
            keywords: "slideshow interval seconds speed timing",
        },
        Command {
            id: CommandId::ToggleFavorite,
            key: "command.toggle_favorite",
            shortcut: "F",
            keywords: "heart star like",
        },
        Command {
            id: CommandId::RotateLeft,
            key: "command.rotate_left",
            shortcut: "⇧R",
            keywords: "turn counterclockwise anticlockwise orientation image photo",
        },
        Command {
            id: CommandId::RotateRight,
            key: "command.rotate_right",
            shortcut: "R",
            keywords: "turn clockwise orientation image photo",
        },
        Command {
            id: CommandId::EditPhoto,
            key: "command.edit_photo",
            shortcut: "E",
            keywords: "adjust exposure brightness auto enhance quick edit image",
        },
        Command {
            id: CommandId::RevertToOriginal,
            key: "command.revert_to_original",
            shortcut: "",
            keywords: "reset undo remove edits rotation exposure image photo",
        },
        Command {
            id: CommandId::EditInExternalEditor,
            key: "command.edit_external",
            shortcut: "⌘E",
            keywords: "open app application editor retouch image photo",
        },
        Command {
            id: CommandId::EditCopyInExternalEditor,
            key: "command.edit_copy_external",
            shortcut: "⌥⌘E",
            keywords: "duplicate open app application editor image photo",
        },
        // After the Rate commands: "rate 3" also matches "Show Rated ★ 3+".
        Command {
            id: CommandId::QuickFilter("is:favorite"),
            key: "command.filter_favorites",
            shortcut: "",
            keywords: "filter hearts loved favourites",
        },
        Command {
            id: CommandId::QuickFilter("is:pick"),
            key: "command.filter_picks",
            shortcut: "",
            keywords: "filter flag picked cull",
        },
        Command {
            id: CommandId::QuickFilter("is:reject"),
            key: "command.filter_rejected",
            shortcut: "",
            keywords: "filter flag rejects cull",
        },
        Command {
            id: CommandId::QuickFilter("is:video"),
            key: "command.filter_videos",
            shortcut: "",
            keywords: "filter movies clips",
        },
        Command {
            id: CommandId::QuickFilter("rating:>=1"),
            key: "command.filter_rating_1",
            shortcut: "",
            keywords: "filter rating stars 1",
        },
        Command {
            id: CommandId::QuickFilter("rating:>=2"),
            key: "command.filter_rating_2",
            shortcut: "",
            keywords: "filter rating stars 2",
        },
        Command {
            id: CommandId::QuickFilter("rating:>=3"),
            key: "command.filter_rating_3",
            shortcut: "",
            keywords: "filter rating stars 3",
        },
        Command {
            id: CommandId::QuickFilter("rating:>=4"),
            key: "command.filter_rating_4",
            shortcut: "",
            keywords: "filter rating stars 4",
        },
        Command {
            id: CommandId::QuickFilter("rating:>=5"),
            key: "command.filter_rating_5",
            shortcut: "",
            keywords: "filter rating stars 5",
        },
        Command {
            id: CommandId::QuickFilter("is:unrated"),
            key: "command.filter_unrated",
            shortcut: "",
            keywords: "filter rating stars none no",
        },
        Command {
            id: CommandId::QuickFilter("color:red"),
            key: "command.filter_color_red",
            shortcut: "",
            keywords: "filter colour color label",
        },
        Command {
            id: CommandId::QuickFilter("color:orange"),
            key: "command.filter_color_orange",
            shortcut: "",
            keywords: "filter colour color label",
        },
        Command {
            id: CommandId::QuickFilter("color:yellow"),
            key: "command.filter_color_yellow",
            shortcut: "",
            keywords: "filter colour color label",
        },
        Command {
            id: CommandId::QuickFilter("color:green"),
            key: "command.filter_color_green",
            shortcut: "",
            keywords: "filter colour color label",
        },
        Command {
            id: CommandId::QuickFilter("color:blue"),
            key: "command.filter_color_blue",
            shortcut: "",
            keywords: "filter colour color label",
        },
        Command {
            id: CommandId::QuickFilter("color:purple"),
            key: "command.filter_color_purple",
            shortcut: "",
            keywords: "filter colour color label",
        },
        Command {
            id: CommandId::QuickFilter("is:unlabeled"),
            key: "command.filter_unlabeled",
            shortcut: "",
            keywords: "filter colour color label none no",
        },
        Command {
            id: CommandId::ClearFilters,
            key: "command.filter_clear",
            shortcut: "",
            keywords: "filter remove reset show everything all",
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
            id: CommandId::SetLoopStart,
            key: "command.set_loop_start",
            shortcut: "⌥I",
            keywords: "video loop range in point start",
        },
        Command {
            id: CommandId::SetLoopEnd,
            key: "command.set_loop_end",
            shortcut: "⌥O",
            keywords: "video loop range out point end",
        },
        Command {
            id: CommandId::ClearLoopRange,
            key: "command.clear_loop_range",
            shortcut: "⌥X",
            keywords: "video loop range remove reset",
        },
        Command {
            id: CommandId::NewAlbum,
            key: "command.new_album",
            shortcut: "⌘N",
            keywords: "album create collection",
        },
        Command {
            id: CommandId::NewAlbumWithSelection,
            key: "command.new_album_with_selection",
            shortcut: "",
            keywords: "album create collection selected items",
        },
        Command {
            id: CommandId::RenameAlbum,
            key: "command.rename_album",
            shortcut: "",
            keywords: "album name title change",
        },
        Command {
            id: CommandId::DeleteAlbum,
            key: "command.delete_album",
            shortcut: "",
            keywords: "album remove delete",
        },
        Command {
            id: CommandId::AddToAlbum,
            key: "command.add_to_album",
            shortcut: "⇧⌘A",
            keywords: "album add put collection selected items",
        },
        Command {
            id: CommandId::RemoveFromAlbum,
            key: "command.remove_from_album",
            shortcut: "⌫",
            keywords: "album remove take out collection selected items",
        },
        Command {
            id: CommandId::PinAlbum,
            key: "command.pin_album",
            shortcut: "",
            keywords: "album smart pin pinned top favorite keep first",
        },
        Command {
            id: CommandId::UnpinAlbum,
            key: "command.unpin_album",
            shortcut: "",
            keywords: "album smart unpin pinned remove",
        },
        Command {
            id: CommandId::DuplicateSmartAlbum,
            key: "command.duplicate_smart_album",
            shortcut: "",
            keywords: "smart album copy duplicate clone rules",
        },
        Command {
            id: CommandId::ExportSmartAlbumRules,
            key: "command.export_smart_album_rules",
            shortcut: "",
            keywords: "smart album rules export save file json share",
        },
        Command {
            id: CommandId::ImportSmartAlbumRules,
            key: "command.import_smart_album_rules",
            shortcut: "",
            keywords: "smart album rules import open file json",
        },
        Command {
            id: CommandId::NewSmartAlbum,
            key: "command.new_smart_album",
            shortcut: "⌥⌘N",
            keywords: "smart album rules filter create",
        },
        Command {
            id: CommandId::SaveSearch,
            key: "command.save_search",
            shortcut: "",
            keywords: "save search query smart album saved",
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
            id: CommandId::LocateMissing,
            key: "command.locate_missing",
            shortcut: "",
            keywords: "missing file find relink reconnect moved",
        },
        Command {
            id: CommandId::AdjustDate,
            key: "command.adjust_date",
            shortcut: "",
            keywords: "date taken time edit change shift offset get info",
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
        Command {
            id: CommandId::AddKeywordAlias,
            key: "command.add_keyword_alias",
            shortcut: "",
            keywords: "tag keyword alias synonym",
        },
        Command {
            id: CommandId::MoveKeyword,
            key: "command.move_keyword",
            shortcut: "",
            keywords: "tag keyword move parent hierarchy reparent",
        },
        Command {
            id: CommandId::Back,
            key: "command.back",
            shortcut: "⌘[",
            keywords: "history previous go return",
        },
        Command {
            id: CommandId::Forward,
            key: "command.forward",
            shortcut: "⌘]",
            keywords: "history next go redo",
        },
        Command {
            id: CommandId::ApplyTemplate,
            key: "command.apply_template",
            shortcut: "",
            keywords: "metadata template preset rights creator copyright apply",
        },
        Command {
            id: CommandId::ManageTemplates,
            key: "command.manage_templates",
            shortcut: "",
            keywords: "metadata template preset rights creator copyright edit new delete",
        },
        Command {
            id: CommandId::ClearRecents,
            key: "command.clear_recents",
            shortcut: "",
            keywords: "forget remove history libraries",
        },
        Command {
            id: CommandId::ClearRecentSearches,
            key: "command.clear_recent_searches",
            shortcut: "",
            keywords: "forget remove history search queries",
        },
    ]
}

impl LuminaApp {
    pub fn open_command_palette(&mut self, cx: &mut Context<Self>) {
        // Commands act on the grid; over the tag manager or the template
        // editor they would change media behind the sheet (the menu item
        // reaches here, not the keymap).
        // Nor over edit mode: they would act on the photo under the draft.
        if self.tag_manager.open || self.templates.editor.is_some() || self.edits.session.is_some()
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

    /// A command's palette title; the sidebar toggle says what it will do.
    fn command_title(&self, command: &Command) -> String {
        if command.id == CommandId::ToggleSidebar {
            crate::app::panels::sidebar_toggle_title(self.sidebar_collapsed)
        } else {
            command.title()
        }
    }

    /// Commands that would do nothing right now stay out of the list.
    fn command_applies(&self, id: CommandId) -> bool {
        match id {
            CommandId::RetryError => self.error_retry.is_some(),
            CommandId::Back => self.can_navigate_back() && !self.compare_holds_view(),
            CommandId::Forward => self.can_navigate_forward() && !self.compare_holds_view(),
            CommandId::ClearRecents => !self.config.recent_libraries.is_empty(),
            CommandId::ClearRecentSearches => !self.config.recent_searches.is_empty(),
            CommandId::SaveSearch => !self.search_query.trim().is_empty(),
            CommandId::NewAlbumWithSelection => {
                self.selected_item.is_some() || !self.selected_indices.is_empty()
            }
            CommandId::AddToAlbum => {
                self.selection != AlbumSelection::Trash
                    && (self.selected_item.is_some() || !self.selected_indices.is_empty())
            }
            CommandId::RemoveFromAlbum => {
                matches!(self.selection, AlbumSelection::Album(_))
                    && (self.selected_item.is_some() || !self.selected_indices.is_empty())
            }
            CommandId::RenameAlbum | CommandId::DeleteAlbum => {
                matches!(self.selection, AlbumSelection::Album(_))
            }
            CommandId::PinAlbum => self.open_album_pinned() == Some(false),
            CommandId::UnpinAlbum => self.open_album_pinned() == Some(true),
            CommandId::DuplicateSmartAlbum | CommandId::ExportSmartAlbumRules => {
                matches!(self.selection, AlbumSelection::User(_))
            }
            CommandId::ImportSmartAlbumRules => self.grid_state.is_some(),
            CommandId::ClearFilters => crate::app::search::query_has_filters(&self.search_query),
            CommandId::EmptyTrash | CommandId::RestoreAll => self
                .system_album_counts
                .get(&lumina_core::SystemAlbum::RecentlyDeleted)
                .is_some_and(|&n| n > 0),
            CommandId::MergeAllDuplicates => self.duplicate_items_count > 0,
            CommandId::ShowExactCopies | CommandId::ShowSimilarShots => {
                self.grid_state.is_some() && !self.compare_holds_view()
            }
            CommandId::ShowMap => !self.map_blocked() && !self.map_shown(),
            CommandId::ShowGrid => self.map_shown(),
            CommandId::DeleteRejected => {
                self.grid_state.is_some()
                    && !self.compare_holds_view()
                    && self.view_query().reject_scope().is_some()
            }
            CommandId::Compare => self.grid_state.is_some() && !self.is_fullscreen_viewer,
            CommandId::Slideshow => self.grid_state.is_some() && !self.compare_holds_view(),
            CommandId::SlideshowInterval(secs) => self.config.slideshow_interval_secs != secs,
            CommandId::ToggleDownloads => !self.downloads.entries.is_empty(),
            CommandId::ExportSelection | CommandId::ExportJpeg => self.jobs.export.is_none(),
            // Similar shots holds no selection, so it offers no Share….
            CommandId::Share => {
                cfg!(target_os = "macos")
                    && (self.selected_item.is_some() || !self.selected_indices.is_empty())
            }
            CommandId::ImportXmp => self.jobs.xmp.is_none(),
            CommandId::AddKeywordAlias | CommandId::MoveKeyword => self.command_keyword().is_some(),
            CommandId::RotateLeft | CommandId::RotateRight | CommandId::RevertToOriginal => {
                self.selected_item.is_some() || !self.selected_indices.is_empty()
            }
            CommandId::EditInExternalEditor | CommandId::EditCopyInExternalEditor => {
                self.selected_item.is_some()
                    && (self.is_fullscreen_viewer || self.selected_indices.len() <= 1)
            }
            CommandId::EditPhoto => self
                .selected_item
                .as_ref()
                .is_some_and(|item| item.media_type != lumina_core::MediaType::Video),
            CommandId::ApplyTemplate => {
                !self.config.templates.is_empty()
                    && (self.selected_item.is_some() || !self.selected_indices.is_empty())
            }
            CommandId::SetLoopStart | CommandId::SetLoopEnd | CommandId::ClearLoopRange => {
                self.loop_range_applies()
            }
            CommandId::ViewerZoom(_) => {
                self.is_fullscreen_viewer
                    && self
                        .selected_item
                        .as_ref()
                        .is_some_and(|item| item.media_type != lumina_core::MediaType::Video)
            }
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
        if self.templates.palette_pick {
            return self
                .config
                .templates
                .iter()
                .enumerate()
                .filter(|(_, template)| matches(&words, &template.name, "template"))
                .map(|(index, template)| PaletteRow {
                    target: PaletteTarget::Template(index),
                    title: template.name.clone(),
                    shortcut: "",
                    icon: Icon::Tag,
                    selector: format!("palette-template-{}", template.name),
                })
                .collect();
        }
        let mut rows: Vec<PaletteRow> = commands()
            .iter()
            .filter(|c| {
                self.command_applies(c.id) && matches(&words, &self.command_title(c), c.keywords)
            })
            .take(MAX_COMMANDS)
            .map(|c| PaletteRow {
                target: PaletteTarget::Command(c.id),
                title: self.command_title(c),
                shortcut: c.shortcut,
                icon: if c.id == CommandId::ToggleSidebar {
                    crate::app::panels::sidebar_toggle_icon(self.sidebar_collapsed)
                } else {
                    c.icon()
                },
                selector: format!("command-item-{}", c.key),
            })
            .collect();
        // The open video's audio choice (§11: the Audio menu's keyboard path),
        // within the same row budget as the commands.
        if self.video.player.is_some() && self.video.tracks.has_audio_choice() {
            let playing = self.current_audio_track();
            let audio = (0..self.video.tracks.audio.len())
                .map(|index| PaletteRow {
                    target: PaletteTarget::AudioTrack(index),
                    title: t("command.audio_track")
                        .replace("{name}", &self.video.tracks.audio_name(index)),
                    shortcut: "",
                    icon: if playing == Some(index) {
                        Icon::Check
                    } else {
                        Icon::Volume
                    },
                    selector: format!("palette-audio-track-{index}"),
                })
                .filter(|row| matches(&words, &row.title, "video sound language"))
                .take(MAX_COMMANDS.saturating_sub(rows.len()))
                .collect::<Vec<_>>();
            rows.extend(audio);
        }

        let go = |selection: AlbumSelection, title: String, icon: Icon, slug: String| PaletteRow {
            target: PaletteTarget::Go(selection),
            title,
            shortcut: "",
            icon,
            selector: format!("palette-go-{slug}"),
        };
        // open_library_inner puts every opened library first, so while one
        // is open the first recent is it.
        let skip_current = usize::from(self.grid_state.is_some());
        let mut go_to: Vec<PaletteRow> = self
            .config
            .recent_libraries
            .iter()
            .enumerate()
            .skip(skip_current)
            .filter(|(_, path)| !self.recent_unavailable(path))
            .map(|(i, path)| PaletteRow {
                target: PaletteTarget::Library(path.clone()),
                title: t("command.open_recent")
                    .replace("{name}", &crate::app::recents::library_name(path)),
                shortcut: "",
                icon: Icon::Folder,
                selector: format!("palette-recent-{i}"),
            })
            .collect();
        go_to.extend([
            go(
                AlbumSelection::AllLibrary,
                "All Media".into(),
                Icon::Images,
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
                Icon::Video,
                "videos".into(),
            ),
            go(
                AlbumSelection::System(lumina_core::SystemAlbum::RecentlyAdded),
                "Recently Added".into(),
                Icon::Clock,
                "recently added new".into(),
            ),
            go(
                AlbumSelection::System(lumina_core::SystemAlbum::Downloaded),
                "Downloaded".into(),
                Icon::ArrowDownToLine,
                "downloaded downloads url".into(),
            ),
            go(
                AlbumSelection::System(lumina_core::SystemAlbum::Screenshots),
                "Screenshots".into(),
                Icon::Camera,
                "screenshots".into(),
            ),
            go(
                AlbumSelection::System(lumina_core::SystemAlbum::Places),
                "Places".into(),
                Icon::MapPin,
                "places".into(),
            ),
            // Listed even at 0, like Downloaded: the palette is a keyboard
            // path, so its rows do not come and go with the counts.
            go(
                AlbumSelection::System(lumina_core::SystemAlbum::Missing),
                "Missing".into(),
                Icon::TriangleAlert,
                "missing".into(),
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
                Icon::Copy,
                "duplicates".into(),
            ),
        ]);
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
        go_to.extend(self.albums.list.iter().map(|album| {
            go(
                AlbumSelection::Album(album.id),
                album.title.clone(),
                Icon::Image,
                format!("curated-{}", album.id),
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
                // One history entry: Back returns to where the jump started.
                let departing = self.navigation_snapshot();
                self.selection = AlbumSelection::AllLibrary;
                self.stacks.reset();
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
            CommandId::MergeAllDuplicates => self.merge_all_duplicates(cx),
            CommandId::ShowExactCopies => {
                self.show_duplicates_tab(crate::app::stacks::DuplicatesTab::Exact, cx)
            }
            CommandId::ShowSimilarShots => {
                self.show_duplicates_tab(crate::app::stacks::DuplicatesTab::Similar, cx)
            }
            CommandId::ShowMap => self.show_map(cx),
            CommandId::ShowGrid => self.show_grid(cx),
            CommandId::RetryError => self.retry_error(cx),
            CommandId::ShowAcknowledgements => self.open_bundled_doc("THIRD_PARTY_NOTICES.md", cx),
            CommandId::Shortcuts => self.toggle_shortcuts_overlay(cx),
            CommandId::ToggleSidebar => self.toggle_sidebar(cx),
            CommandId::ToggleInspector => self.toggle_inspector(cx),
            CommandId::GetInfo => self.get_info(cx),
            CommandId::ExportSelection => {
                self.export_pick_folder(lumina_core::ExportPreset::Originals, cx)
            }
            CommandId::ExportJpeg => {
                self.export_pick_folder(lumina_core::ExportPreset::Jpeg2048, cx)
            }
            // Anchored at the window centre: the palette has no item to point at.
            #[cfg(target_os = "macos")]
            CommandId::Share => self.share_selection(None, cx),
            #[cfg(not(target_os = "macos"))]
            CommandId::Share => {}
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
            // On the map they zoom the map, as ⌘= / ⌘- do.
            CommandId::ZoomInGrid if self.map_shown() => self.zoom_map(1, cx),
            CommandId::ZoomOutGrid if self.map_shown() => self.zoom_map(-1, cx),
            CommandId::ZoomInGrid => self.zoom_in_grid(cx),
            CommandId::ZoomOutGrid => self.zoom_out_grid(cx),
            CommandId::ViewerZoom(preset) => self.apply_viewer_zoom_preset(preset, cx),
            CommandId::QuickFilter(token) => self.toggle_quick_filter(token, cx),
            CommandId::ClearFilters => self.clear_quick_filters(cx),
            CommandId::Rate(rating) => {
                self.set_rating_selected(rating, cx);
            }
            CommandId::Flag(flag) => {
                self.set_flag_selected(flag, cx);
            }
            CommandId::DeleteRejected => self.delete_rejected(cx),
            CommandId::Compare => self.open_compare(cx),
            CommandId::Slideshow => self.start_slideshow_deferred(cx),
            CommandId::SlideshowLoop => self.toggle_slideshow_loop(cx),
            CommandId::SlideshowInterval(secs) => self.set_slideshow_interval(secs, cx),
            CommandId::ToggleFavorite if self.selected_indices.len() > 1 => {
                self.batch_toggle_favorite(cx)
            }
            CommandId::ToggleFavorite => self.toggle_favorite_selected(cx),
            CommandId::RotateLeft => self.rotate_selection(ViewerRotation::Left, cx),
            CommandId::RotateRight => self.rotate_selection(ViewerRotation::Right, cx),
            CommandId::EditPhoto => self.enter_edit_mode(cx),
            CommandId::RevertToOriginal => self.revert_selection_to_original(cx),
            CommandId::EditInExternalEditor => self.edit_selected_externally(false, cx),
            CommandId::EditCopyInExternalEditor => self.edit_selected_externally(true, cx),
            CommandId::TogglePip => self.toggle_pip(cx),
            CommandId::PipPlayPause => self.toggle_pip_playback(cx),
            CommandId::SetLoopStart => self.set_loop_in(cx),
            CommandId::SetLoopEnd => self.set_loop_out(cx),
            CommandId::ClearLoopRange => self.clear_loop_range(cx),
            CommandId::NewAlbum => self.new_album(cx),
            CommandId::NewAlbumWithSelection => self.new_album_with_selection(cx),
            CommandId::RenameAlbum => self.rename_selected_album(cx),
            CommandId::DeleteAlbum => self.delete_selected_album(cx),
            CommandId::AddToAlbum => self.open_album_picker(cx),
            CommandId::RemoveFromAlbum => self.remove_selection_from_album(cx),
            CommandId::PinAlbum => self.set_open_album_pinned(true, cx),
            CommandId::UnpinAlbum => self.set_open_album_pinned(false, cx),
            CommandId::DuplicateSmartAlbum => self.duplicate_selected_smart_album(cx),
            CommandId::ExportSmartAlbumRules => self.export_selected_smart_album_rules(cx),
            CommandId::ImportSmartAlbumRules => self.import_smart_album_rules(cx),
            CommandId::NewSmartAlbum => self.open_album_builder(cx),
            CommandId::SaveSearch => self.save_current_search_as_album(cx),
            CommandId::EditSmartAlbum => self.edit_selected_smart_album(cx),
            CommandId::DeleteSmartAlbum => self.delete_selected_smart_album(cx),
            CommandId::LocateMissing => self.locate_missing_selected(cx),
            CommandId::ImportMedia => self.pick_import_folders(cx),
            CommandId::ImportNetworkVideo => self.open_network_import_dialog(cx),
            CommandId::ToggleDownloads => self.toggle_downloads_panel(cx),
            CommandId::CancelImport => self.cancel_running_import(cx),
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
            CommandId::AddKeywordAlias => self.start_alias_input(cx),
            CommandId::MoveKeyword => self.start_keyword_move(cx),
            CommandId::Back => self.navigate_back(cx),
            CommandId::Forward => self.navigate_forward(cx),
            CommandId::ClearRecents => self.clear_recents(cx),
            CommandId::ClearRecentSearches => self.clear_recent_searches(cx),
            CommandId::ApplyTemplate => self.start_template_pick(cx),
            CommandId::ManageTemplates => self.open_template_editor(cx),
        }
    }
}
