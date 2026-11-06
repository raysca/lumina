mod browser;
mod delete_dialog;
mod design_system;
mod downloads;
mod downloads_panel;
mod duplicates;
mod editor_handoff;
mod engine_release;
mod get_info;
mod grid_scroll;
mod grid_zoom;
mod history;
mod immersive_hud;
mod import;
mod inspector;
mod interaction;
mod legacy_catalog;
mod missing;
mod motion;
mod multi_select;
mod network_import;
mod overlay_isolation;
mod palette_and_empty;
mod pip;
mod preferences;
mod regressions;
mod session_restore;
mod smart_album_builder;
mod states;
mod tag_manager;
mod tags_and_albums;
mod text_input;
mod thumbnails;
mod toasts;
mod ui_automation;
mod ui_thread;
mod video_player;
mod video_scrub;
mod viewer_and_video;
mod viewer_stage;
mod welcome;
mod windows_and_layout;
mod zoom_presets;

/// A `TestAppContext` that shuts its app down when the test ends.
///
/// `add_window_view` leaks a `VisualTestContext` that holds the app, so a
/// dropped context never frees its windows, and each test's `LuminaApp` (with
/// its engine's decoder pool, watcher and catalog) lived until the process
/// exited. `quit` closes the windows, which drops them. `#[gpui::test]` does
/// the same; these tests are plain `#[test]` functions.
pub(crate) struct TestApp(gpui::TestAppContext);

impl TestApp {
    pub(crate) fn single() -> Self {
        Self(gpui::TestAppContext::single())
    }
}

impl std::ops::Deref for TestApp {
    type Target = gpui::TestAppContext;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for TestApp {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for TestApp {
    fn drop(&mut self) {
        self.0.quit();
    }
}
