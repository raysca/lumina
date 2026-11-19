//! Command ids, with their titles and icons.

use crate::icons::Icon;
use crate::views::viewer_state::ViewerZoomPreset;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandId {
    OpenLibrary,
    /// Removes the open library's catalog and thumbnails; its files stay.
    DeleteLibrary,
    Rescan,
    Preferences,
    /// Settings › Reset to defaults (asks first).
    ResetPreferences,
    /// Settings on its Downloads category.
    SettingsDownloads,
    About,
    ShowLicense,
    ShowAcknowledgements,
    /// Looks for a newer Lumina and downloads it (macOS).
    CheckForUpdates,
    /// Quits, installs the downloaded update and opens it.
    RestartToUpdate,
    /// Turn the daily update check on or off.
    AutoUpdatesOn,
    AutoUpdatesOff,
    EmptyTrash,
    RestoreAll,
    MergeAllDuplicates,
    /// Review Duplicates: the shown group, or every group.
    SkipDuplicateGroup,
    MergeDuplicateGroup,
    AutoSelectBest,
    SkipAllDuplicateGroups,
    /// The Duplicates destination on its Exact copies / Similar shots tab.
    ShowExactCopies,
    ShowSimilarShots,
    /// The Albums overview page, and the page on one of its tabs.
    ShowAlbums,
    ShowAlbumsTab,
    ShowSmartAlbumsTab,
    /// Places on the map / on the grid.
    ShowMap,
    ShowGrid,
    /// The justified Mosaic layout (the Grid / Mosaic choice is saved).
    ShowMosaic,
    /// Selects every item of a Mosaic day.
    SelectDay,
    /// The Mosaic's grouping levels (where the zoom pill shows them).
    ShowYears,
    ShowMonths,
    ShowDays,
    /// Places on the map, centred on the selected item's location.
    ShowOnMap,
    RetryError,
    Shortcuts,
    ToggleSidebar,
    ToggleInspector,
    /// The open album's settings panel (Hide / Show Album Settings).
    ToggleAlbumPanel,
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
    /// Filter: 4K Videos: the Videos view's 4K chip.
    Videos4k,
    /// Removes every filter token from the search, keeping its text.
    ClearFilters,
    /// Empties the search query; shown only while there is one.
    ClearSearch,
    /// Opens the Add filter menu under the header's Filter button.
    AddFilter,
    /// Sets the selection's rating; 0 clears it.
    Rate(u8),
    /// Opens the viewer's star popover (Rate…).
    RateMenu,
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
    /// The open video's volume and frame steps; shown with the loop commands.
    VolumeUp,
    VolumeDown,
    PreviousFrame,
    NextFrame,
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
    /// The one selected item becomes the open album's key photo; shown
    /// only on an album page with one item selected.
    SetAlbumCover,
    /// Goes to All Media to pick items for the open album; shown only
    /// while an empty album is shown.
    AddFromLibrary,
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
    ShowAllDownloads,
    /// Act on the download list; each shown only when it has rows to act on.
    PauseAllDownloads,
    ResumeAllDownloads,
    RetryFailedDownloads,
    ClearFinishedDownloads,
    CancelImport,
    /// Get Info actions, so they have a keyboard path (§11).
    AdjustDate,
    EditLocation,
    RemoveLocation,
    OpenTagManager,
    /// Opens the Tags page sort menu; shown only on the page.
    SortTags,
    /// Act on the tag manager's focused keyword (or the sidebar's tag view);
    /// shown only when there is one.
    AddKeywordAlias,
    MoveKeyword,
    MergeKeyword,
    SaveKeywordSmart,
    RenameKeyword,
    DeleteKeyword,
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
            DeleteLibrary => Icon::Trash,
            Rescan => Icon::RotateCw,
            ResetPreferences => Icon::RefreshCw,
            Preferences | SettingsDownloads | SortDate | SortName | SortSize | SortRating
            | SortAsc | SortDesc => Icon::Sliders,
            About | ToggleInspector | GetInfo | ShowLicense | ShowAcknowledgements => Icon::Info,
            Shortcuts => Icon::Zap,
            CheckForUpdates => Icon::ArrowDownToLine,
            RestartToUpdate => Icon::RotateCw,
            AutoUpdatesOn | AutoUpdatesOff => Icon::RefreshCw,
            ToggleSidebar => Icon::PanelLeft,
            ToggleAlbumPanel => Icon::PanelRight,
            ExportSelection | ExportJpeg | Share => Icon::ExternalLink,
            ImportXmp => Icon::ArrowDownToLine,
            ToggleFit => Icon::Maximize,
            ZoomInGrid => Icon::Plus,
            ZoomOutGrid => Icon::Minus,
            ViewerZoom(ViewerZoomPreset::Fit) => Icon::Minimize,
            ViewerZoom(_) => Icon::Maximize,
            QuickFilter(_) | Videos4k => Icon::Search,
            ClearFilters | ClearSearch => Icon::X,
            AddFilter => Icon::SlidersHorizontal,
            Rate(0) | RateMenu => Icon::Star,
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
            VolumeUp | VolumeDown => Icon::Volume,
            PreviousFrame => Icon::StepBack,
            NextFrame => Icon::StepForward,
            NewAlbum | NewAlbumWithSelection | AddToAlbum => Icon::Plus,
            NewSmartAlbum | SaveSearch | DuplicateSmartAlbum => Icon::Sparkles,
            PinAlbum | UnpinAlbum => Icon::Star,
            SetAlbumCover => Icon::ImagePlus,
            AddFromLibrary => Icon::Library,
            ExportSmartAlbumRules => Icon::ExternalLink,
            ImportSmartAlbumRules => Icon::ArrowDownToLine,
            EditSmartAlbum | RenameAlbum | AdjustDate | EditLocation => Icon::Pencil,
            RemoveLocation | DeleteSmartAlbum | DeleteAlbum | RemoveFromAlbum => Icon::X,
            EmptyTrash => Icon::Trash,
            RestoreAll => Icon::Undo2,
            MergeAllDuplicates | ShowExactCopies | ShowSimilarShots => Icon::Repeat,
            SkipDuplicateGroup | SkipAllDuplicateGroups => Icon::StepForward,
            MergeDuplicateGroup => Icon::Combine,
            AutoSelectBest => Icon::WandSparkles,
            ShowMap | ShowOnMap => Icon::MapPin,
            ShowAlbums | ShowAlbumsTab | ShowSmartAlbumsTab => Icon::Folder,
            ShowGrid => Icon::Images,
            ShowMosaic => Icon::LayoutDashboard,
            SelectDay | ShowYears | ShowMonths | ShowDays => Icon::Calendar,
            LocateMissing => Icon::TriangleAlert,
            ImportMedia | ImportNetworkVideo | ToggleDownloads | ShowAllDownloads => {
                Icon::ArrowDownToLine
            }
            PauseAllDownloads => Icon::Pause,
            ResumeAllDownloads => Icon::Play,
            RetryFailedDownloads => Icon::RotateCcw,
            ClearFinishedDownloads => Icon::Check,
            CancelImport => Icon::X,
            RetryError => Icon::RotateCw,
            OpenTagManager | AddKeywordAlias | ApplyTemplate => Icon::Tag,
            MoveKeyword | RenameKeyword | ManageTemplates => Icon::Pencil,
            MergeKeyword => Icon::Merge,
            SaveKeywordSmart => Icon::FolderPlus,
            DeleteKeyword => Icon::Trash,
            SortTags => Icon::ArrowUpNarrowWide,
            ClearRecents | ClearRecentSearches => Icon::X,
            Back => Icon::ChevronLeft,
            Forward => Icon::ChevronRight,
        }
    }
}
