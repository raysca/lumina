//! App-level actions used by the macOS menu bar and keyboard shortcuts.

use gpui::actions;

actions!(
    lumina,
    [
        OpenPreferences,
        ShowCommandPalette,
        OpenLibrary,
        RescanLibrary,
        ShowShortcuts,
        ShowAbout,
        QuitApp,
        ToggleSidebar,
        ToggleInspector,
        ToggleAlbumPanel,
        ImportXmp,
        ManageTemplates,
        ImportMedia,
        ImportNetworkVideo,
        ToggleDownloads,
        NewSmartAlbum,
        ImportSmartAlbumRules,
        NewAlbum,
        AddToAlbum,
        DeleteRejected,
        ExportOriginals,
        ExportAsJpeg,
        ShareSelection,
        Compare,
        Slideshow,
        ShowMap,
        ShowAlbums,
        GoBack,
        GoForward,
        RotateLeft,
        RotateRight,
        EditPhoto,
        RevertToOriginal,
        EditInExternalEditor,
        EditCopyInExternalEditor
    ]
);
