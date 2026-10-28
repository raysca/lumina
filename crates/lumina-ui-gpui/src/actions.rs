//! App-level actions used by the macOS menu bar and keyboard shortcuts.

use gpui::actions;

actions!(
    lumina,
    [
        OpenPreferences,
        OpenLibrary,
        RescanLibrary,
        ShowShortcuts,
        ShowAbout,
        QuitApp,
        ToggleSidebar,
        ToggleInspector,
        ImportXmp
    ]
);
