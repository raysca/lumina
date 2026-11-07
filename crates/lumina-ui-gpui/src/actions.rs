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
        ImportXmp,
        ManageTemplates,
        ImportMedia,
        ImportNetworkVideo,
        ToggleDownloads,
        NewSmartAlbum,
        NewAlbum,
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
