mod album_cards;
mod album_cards_keys;
mod album_membership;
mod album_membership_guards;
mod album_page;
mod albums;
mod albums_delete;
mod albums_overview;
mod albums_pinned;
mod batch_edit;
mod browser;
mod compare;
mod compare_guards;
mod culling;
mod culling_delete;
mod culling_filtered;
mod delete_dialog;
mod design_system;
mod downloads;
mod downloads_panel;
mod duplicates;
mod editor_copy;
mod editor_handoff;
mod editor_round_trip;
mod engine_release;
mod export;
mod filter_bar;
mod get_info;
mod grid_scroll;
mod grid_zoom;
mod group_levels;
mod history;
mod immersive_hud;
mod import;
mod inspector;
mod interaction;
mod keyword_undo;
mod keywords;
mod large_selection;
mod legacy_catalog;
mod library_header;
mod map;
mod missing;
mod mosaic;
mod mosaic_geometry;
mod motion;
mod multi_select;
mod network_import;
mod overlay_isolation;
mod palette_and_empty;
mod pip;
mod preferences;
mod quick_edit;
mod quick_edit_crop;
mod quick_edit_crop_mouse;
mod quick_edit_guards;
mod quick_edit_preview;
mod regressions;
mod search_field;
mod search_header;
mod session_restore;
#[cfg(target_os = "macos")]
mod share;
mod sidebar;
mod slideshow;
mod smart_album_builder;
mod smart_cards;
mod smart_cards_keys;
mod stacks;
mod stacks_guards;
mod stacks_members;
mod states;
mod tag_manager;
mod tags_and_albums;
mod templates;
mod text_input;
mod thumbnails;
mod toasts;
mod ui_automation;
mod ui_thread;
mod video_filters;
mod video_player;
mod video_scrub;
mod video_tiles;
mod video_tracks;
mod viewer_and_video;
mod viewer_filmstrip;
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
    /// `TestAppContext::single()` with task order kept FIFO.
    ///
    /// gpui-pre-scheduler 0.3.11 locks `state` then `rng` when a task is
    /// scheduled with `randomize_order` on, but `block_until` with a timeout
    /// (the shutdown in `quit`) locks `rng` then `state`. A decode worker
    /// waking a task while a test quits then deadlocked both forever (seen
    /// as a UI run stuck for minutes in `compare_close_drops_textures`).
    /// With `randomize_order` off, scheduling never takes `rng`. The seed was
    /// fixed at 0 anyway, so the order was never varied between runs.
    pub(crate) fn single() -> Self {
        let scheduler = std::sync::Arc::new(scheduler::TestScheduler::new(
            scheduler::TestSchedulerConfig {
                randomize_order: false,
                // As `TestDispatcher::new` sets it.
                timeout_ticks: 0..=1000,
                ..Default::default()
            },
        ));
        Self(gpui::TestAppContext::build(
            gpui::TestDispatcher::from_scheduler(scheduler),
            None,
        ))
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
