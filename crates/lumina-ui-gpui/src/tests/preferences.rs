//! Settings: a full-window page with a category sidebar, grouped rows per
//! category, and it fits a small window.

use super::ui_automation::{
    app_with_library, click, click_point, focus_app, keystrokes, leak, render_ui, seed_library,
    type_text,
};
use crate::app::preferences::{thumbnail_cache_dir, CacheUsage, PreferencesTab};

#[core::prelude::v1::test]
fn settings_shell_has_sidebar_categories_and_keyboard_nav() {
    let (temp_dir, engine) = seed_library("settings_shell", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    vcx.simulate_resize(gpui::size(gpui::px(1024.0), gpui::px(560.0)));
    render_ui(vcx);
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-,");
    render_ui(vcx);

    let page = vcx.debug_bounds("settings-page").expect("settings renders");
    assert_eq!(
        (f32::from(page.size.width), f32::from(page.size.height)),
        (1024.0, 560.0),
        "settings fills the whole window"
    );
    let sidebar = vcx
        .debug_bounds("settings-sidebar")
        .expect("the settings sidebar renders");
    assert!(
        (f32::from(sidebar.size.width) - 252.0).abs() <= 1.0,
        "the settings sidebar is 252 px wide, got {:?}",
        sidebar.size.width
    );
    let tops: Vec<f32> = [
        "settings-category-general",
        "settings-category-library",
        "settings-category-downloads",
        "settings-category-storage",
        "settings-category-about",
    ]
    .into_iter()
    .map(|selector| {
        let bounds = vcx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("{selector} renders"));
        assert!(
            bounds.bottom() <= gpui::px(560.0),
            "{selector} stays visible in a 560 px window"
        );
        f32::from(bounds.top())
    })
    .collect();
    assert!(
        tops.windows(2).all(|pair| pair[0] < pair[1]),
        "categories run General, Library, Downloads, Storage, About: {tops:?}"
    );
    let close = vcx
        .debug_bounds("preferences-close")
        .expect("the close control renders");
    assert!(
        close.top() >= gpui::px(0.0) && close.bottom() <= gpui::px(560.0),
        "the TopBar close control stays visible in a small window"
    );

    let library = |vcx: &mut gpui::VisualTestContext| {
        view.update(vcx, |app, _| {
            let count = app
                .grid_state
                .as_ref()
                .map(|s| s.engine.total_count())
                .expect("a library is open");
            (app.selected_index, count)
        })
    };
    let before = library(vcx);
    keystrokes(vcx, "down backspace secondary-backspace");
    assert_eq!(
        library(vcx),
        before,
        "library keys must not move the selection or delete media under settings"
    );
    view.update(vcx, |app, _| {
        assert!(
            app.delete_confirm_state.is_none(),
            "⌫ under settings must not open the delete dialog"
        );
    });
    keystrokes(vcx, "up");

    let selected =
        |vcx: &mut gpui::VisualTestContext| view.update(vcx, |app, _| app.preferences_tab);
    assert_eq!(selected(vcx), PreferencesTab::General);
    keystrokes(vcx, "down");
    assert_eq!(selected(vcx), PreferencesTab::Library, "↓ moves to Library");
    keystrokes(vcx, "down down");
    assert_eq!(
        selected(vcx),
        PreferencesTab::Storage,
        "↓↓ moves to Storage"
    );
    keystrokes(vcx, "down down");
    assert_eq!(selected(vcx), PreferencesTab::About, "↓ stops at About");
    keystrokes(vcx, "up");
    assert_eq!(selected(vcx), PreferencesTab::Storage, "↑ moves back");
    view.update(vcx, |app, _| {
        assert!(app.preferences_open, "arrows keep settings open");
    });

    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| assert!(!app.preferences_open, "Esc closes"));
    keystrokes(vcx, "secondary-,");
    view.update(vcx, |app, _| assert!(app.preferences_open, "⌘, opens"));
    keystrokes(vcx, "secondary-,");
    view.update(vcx, |app, _| {
        assert!(!app.preferences_open, "⌘, closes settings again")
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn menu_overlays_do_not_open_under_settings() {
    let (temp_dir, engine) = seed_library("settings_menu_guard", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-,");

    vcx.dispatch_action(crate::actions::ShowShortcuts);
    vcx.dispatch_action(crate::actions::ImportNetworkVideo);
    vcx.dispatch_action(crate::actions::DeleteRejected);
    vcx.dispatch_action(crate::actions::NewSmartAlbum);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(
            !app.shortcuts_overlay_open,
            "the shortcut sheet must not open hidden under settings"
        );
        assert!(app.network_import.stage.is_none(), "no URL import sheet");
        assert!(app.delete_confirm_state.is_none(), "no delete dialog");
        assert!(!app.album_draft.modal_open, "no smart album sheet");
    });
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(!app.preferences_open, "Esc still closes settings");
        assert!(!app.shortcuts_overlay_open, "and nothing opens behind it");
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn command_palette_menu_item_does_not_open_under_settings() {
    let (temp_dir, engine) = seed_library("settings_palette_guard", 2);
    let item = engine.get_item_slice(0..1)[0].clone();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.selected_item = Some(item.clone());
        app.selected_index = Some(0);
        app.open_preferences(cx);
    });
    focus_app(vcx, &view);
    vcx.dispatch_action(crate::actions::ShowCommandPalette);
    keystrokes(vcx, "r a t e space 5 enter");
    view.update(vcx, |app, _| {
        assert!(
            !app.command_palette_open,
            "the palette must not open over settings"
        );
        assert_eq!(
            app.item_ratings.get(&item.id).copied().unwrap_or(0),
            0,
            "a command must never rate the selection hidden under settings"
        );
        assert!(app.preferences_open, "settings stays open");
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn downloads_list_does_not_open_under_settings() {
    use super::downloads::{install, queue_app};
    let (_dir, engine, fake, mut cx) = queue_app("settings_downloads_guard");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    super::downloads_panel::queue(&view, vcx, &["https://example.com/a.mp4"]);
    view.update(vcx, |app, cx| app.close_downloads_panel(cx));
    focus_app(vcx, &view);
    vcx.dispatch_action(crate::actions::ToggleDownloads);
    view.update(vcx, |app, cx| {
        assert!(app.downloads.open, "the menu item opens the list normally");
        app.close_downloads_panel(cx);
        app.open_preferences(cx);
    });
    vcx.dispatch_action(crate::actions::ToggleDownloads);
    view.update(vcx, |app, _| {
        assert!(
            !app.downloads.open,
            "the downloads list must not open hidden under settings"
        );
    });
}

#[core::prelude::v1::test]
fn error_banner_shows_inside_settings_once() {
    let (temp_dir, engine) = seed_library("settings_banner", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.open_preferences(cx);
        app.set_error("Could not clear thumbnail cache: disk full", cx);
    });
    render_ui(vcx);
    let page = vcx.debug_bounds("settings-page").expect("settings renders");
    let banner = vcx
        .debug_bounds("error-banner")
        .expect("the error banner shows while settings is open");
    assert!(
        banner.top() == page.top() && banner.right() == page.right(),
        "the error banner must be drawn on the settings page, where it can be seen"
    );
    assert_eq!(
        vcx.debug_bounds("settings-sidebar").map(|b| b.right()),
        Some(banner.left()),
        "the banner sits in the settings column, not in the hidden library"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn preferences_tabs_group_actions_and_fit_a_small_window() {
    let (temp_dir, engine) = seed_library("prefs_v3", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    vcx.simulate_resize(gpui::size(gpui::px(1024.0), gpui::px(560.0)));
    view.update(vcx, |app, cx| app.open_preferences(cx));
    render_ui(vcx);

    let header = vcx.debug_bounds("page-header").expect("TopBar renders");
    let body = vcx
        .debug_bounds("preferences-body")
        .expect("settings content renders");
    assert_eq!(
        (body.top(), body.bottom()),
        (header.bottom(), gpui::px(560.0)),
        "the content scrolls in the space under the TopBar, never past the window"
    );
    assert!(
        vcx.debug_bounds("preferences-choose-editor").is_some(),
        "General must offer preferences-choose-editor"
    );

    click(vcx, "settings-category-library");
    for selector in ["preferences-change-library", "preferences-rescan"] {
        assert!(
            vcx.debug_bounds(selector).is_some(),
            "Library must offer {selector}"
        );
    }
    view.update(vcx, |app, _| {
        if let Some(state) = app.grid_state.as_mut() {
            state.is_scanning = false;
        }
    });
    click(vcx, "preferences-rescan");
    view.update(vcx, |app, _| {
        assert!(
            app.grid_state.as_ref().is_some_and(|s| s.is_scanning),
            "Rescan Library in Preferences must start a scan"
        );
        assert!(
            app.preferences_open,
            "rescanning must keep Preferences open"
        );
    });

    click(vcx, "settings-category-storage");
    for selector in [
        "preferences-clear-cache",
        "preferences-reveal-config",
        "preferences-reveal-logs",
    ] {
        assert!(
            vcx.debug_bounds(selector).is_some(),
            "Storage must offer {selector}"
        );
    }

    click(vcx, "settings-category-about");
    assert!(
        vcx.debug_bounds("preferences-version").is_some(),
        "About must show the version"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn preferences_v3_counts_thumbnails_confirms_clear_and_opens_license() {
    let (temp_dir, engine) = seed_library("prefs_v3_details", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| app.open_preferences(cx));
    render_ui(vcx);
    click(vcx, "settings-category-library");
    assert!(vcx.debug_bounds("preferences-indexed").is_some());
    assert!(
        vcx.debug_bounds("preferences-reveal-library").is_none(),
        "v3 drops Reveal from the Library folder row"
    );

    // Storage: "size · N thumbnails", then Clear Cache confirms inline.
    let cache = thumbnail_cache_dir();
    std::fs::create_dir_all(cache.join("ab")).unwrap();
    std::fs::write(cache.join("ab/1.jpg"), b"thumb").unwrap();
    std::fs::write(cache.join("2.jpg"), b"thumb").unwrap();
    click(vcx, "settings-category-storage");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.preferences_cache,
            Some(CacheUsage {
                bytes: 10,
                files: 2
            }),
            "Size on disk counts thumbnails, nested ones too"
        );
    });
    assert!(vcx.debug_bounds("preferences-cache-size").is_some());
    click(vcx, "preferences-clear-cache");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.preferences_cache.map(|u| u.files), Some(0));
        assert!(app.preferences_cache_cleared, "Clear Cache confirms inline");
        assert!(app.error_message.is_none());
    });
    assert!(vcx.debug_bounds("preferences-cache-cleared").is_some());
    view.update(vcx, |app, cx| {
        app.close_preferences(cx);
        app.open_preferences(cx);
        assert!(
            !app.preferences_cache_cleared,
            "reopening hides the confirmation"
        );
    });

    // About: MIT License and Acknowledgements resolve to real files.
    view.update(vcx, |app, cx| app.open_about(cx));
    render_ui(vcx);
    click(vcx, "preferences-license");
    click(vcx, "preferences-acknowledgements");
    view.update(vcx, |app, _| {
        assert!(app.error_message.is_none(), "both documents are found");
    });
    for name in ["LICENSE", "THIRD_PARTY_NOTICES.md"] {
        assert!(
            crate::app::preferences::bundled_doc(name).is_some(),
            "{name} resolves"
        );
    }

    // Keyboard path: ⌘K runs both documents too.
    for query in ["l i c e n s e enter", "a c k n o w enter"] {
        super::ui_automation::focus_app(vcx, &view);
        super::ui_automation::keystrokes(vcx, &format!("secondary-k {query}"));
        view.update(vcx, |app, _| {
            assert!(!app.command_palette_open, "{query} ran a command");
            assert!(app.error_message.is_none(), "{query} found its document");
        });
    }

    let _ = std::fs::remove_dir_all(&cache);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn library_folder_reads_from_home() {
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| crate::LuminaApp::new(cx));
    let home = dirs::home_dir().expect("tests run with a home directory");
    view.update(vcx, |app, _| {
        app.grid_state = None;
        app.config.last_library_path = Some(home.join("Pictures").join("Lumina"));
        assert_eq!(
            app.library_path_label(),
            "~/Pictures/Lumina",
            "the Library folder row shortens the home directory to ~"
        );
    });
}

/// Height of a debug selector's bounds, in pixels.
fn height(vcx: &mut gpui::VisualTestContext, selector: &'static str) -> f32 {
    let bounds = vcx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} renders"));
    f32::from(bounds.size.height)
}

#[core::prelude::v1::test]
fn settings_rows_match_design_heights() {
    let (temp_dir, engine) = seed_library("settings_rows", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    vcx.simulate_resize(gpui::size(gpui::px(1024.0), gpui::px(560.0)));
    view.update(vcx, |app, cx| app.open_preferences(cx));
    render_ui(vcx);

    let described = height(vcx, "settings-row-editor");
    assert!(
        (described - 62.5).abs() <= 1.0,
        "a settings row with a description is 62.5 tall, got {described}"
    );
    assert_eq!(height(vcx, "preferences-choose-editor"), 34.0);
    click(vcx, "settings-category-library");
    let plain = height(vcx, "settings-row-library-size");
    assert!(
        (plain - 43.5).abs() <= 1.0,
        "a plain settings row is 43.5 tall (frame l85pQN), got {plain}"
    );
    let location = height(vcx, "settings-row-library");
    assert!(
        (location - 62.5).abs() <= 1.0,
        "Library location has a description: 62.5 tall, got {location}"
    );
    for button in ["preferences-change-library", "preferences-rescan"] {
        assert_eq!(
            height(vcx, button),
            34.0,
            "{button} is a 34 tall row button"
        );
    }
    assert_eq!(
        height(vcx, "settings-path-library"),
        34.0,
        "the library path pill is 34 tall"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn long_library_path_truncates_inside_its_row() {
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| crate::LuminaApp::new(cx));
    // The window's minimum size (main.rs).
    vcx.simulate_resize(gpui::size(gpui::px(960.0), gpui::px(600.0)));
    let deep: std::path::PathBuf = [
        "/Volumes",
        "Photo Archive External Drive",
        "Family and Friends",
        "Every Year Since The Very Beginning",
        "Raw Imports Awaiting Review",
        "Lumina Library",
    ]
    .iter()
    .collect();
    let tooltip = format!("tooltip-{}", deep.display());
    view.update(vcx, |app, cx| {
        app.grid_state = None;
        app.config.last_library_path = Some(deep);
        app.open_preferences_at(PreferencesTab::Library, cx);
    });
    render_ui(vcx);

    let row = vcx
        .debug_bounds("settings-row-library")
        .expect("the library row renders");
    let pill = vcx
        .debug_bounds("settings-path-library")
        .expect("the path pill renders");
    let button = vcx
        .debug_bounds("preferences-change-library")
        .expect("Open Another… renders");
    assert!(
        pill.left() > row.left() + gpui::px(100.0) && pill.right() <= button.left(),
        "a long path truncates: the pill leaves room for the label and ends before \
         the button, row {row:?}, pill {pill:?}, button {button:?}"
    );
    assert!(
        button.right() <= row.right(),
        "the button stays inside its row, row {row:?}, button {button:?}"
    );
    assert_eq!(
        f32::from(button.size.height),
        34.0,
        "Open Another… never wraps to a second line"
    );
    assert!(
        (f32::from(row.size.height) - 62.5).abs() <= 1.0,
        "a long path keeps the row at its described height, got {:?}",
        row.size.height
    );

    vcx.simulate_mouse_move(pill.center(), None, gpui::Modifiers::default());
    vcx.background_executor
        .advance_clock(std::time::Duration::from_millis(600));
    vcx.run_until_parked();
    render_ui(vcx);
    assert!(
        vcx.debug_bounds(leak(tooltip)).is_some(),
        "hovering the truncated pill shows the full path in a tooltip"
    );

    // At the minimum window every row keeps its controls inside it.
    let inside = |vcx: &mut gpui::VisualTestContext, row: &'static str, control: &'static str| {
        let row = vcx
            .debug_bounds(row)
            .unwrap_or_else(|| panic!("{row} renders"));
        let control = vcx
            .debug_bounds(control)
            .unwrap_or_else(|| panic!("{control} renders"));
        assert!(
            control.left() >= row.left() + gpui::px(240.0)
                && control.right() <= row.right()
                && control.top() >= row.top()
                && control.bottom() <= row.bottom(),
            "at 960×600 {control:?} stays inside its row {row:?}, right of the label"
        );
        assert_eq!(f32::from(control.size.height), 34.0, "it never wraps");
    };
    inside(vcx, "settings-row-library-size", "preferences-rescan");
    click(vcx, "settings-category-general");
    inside(vcx, "settings-row-editor", "preferences-choose-editor");
    click(vcx, "settings-category-storage");
    for (row, control) in [
        ("settings-row-cache", "settings-path-cache"),
        ("settings-row-cache", "preferences-reveal-cache"),
        ("settings-row-cache-size", "preferences-clear-cache"),
        ("settings-row-config", "settings-path-config"),
        ("settings-row-config", "preferences-reveal-config"),
        ("settings-row-logs", "settings-path-logs"),
        ("settings-row-logs", "preferences-reveal-logs"),
    ] {
        inside(vcx, row, control);
    }
}

#[core::prelude::v1::test]
fn general_shows_general_rows_and_launch_view() {
    use crate::config::LaunchView;
    let (temp_dir, engine) = seed_library("settings_general", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    vcx.simulate_resize(gpui::size(gpui::px(1280.0), gpui::px(800.0)));
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-,");
    render_ui(vcx);

    for selector in [
        "settings-launch-view",
        "settings-launch-view-library",
        "settings-launch-view-last",
        "preferences-choose-editor",
    ] {
        assert!(
            vcx.debug_bounds(selector).is_some(),
            "General must show {selector}"
        );
    }
    for moved in [
        "preferences-change-library",
        "preferences-rescan",
        "preferences-indexed",
    ] {
        assert!(
            vcx.debug_bounds(moved).is_none(),
            "{moved} moved to the Library category"
        );
    }
    // The rendered note itself: a missing, hard-coded or misspelt version fails.
    let note = leak(format!(
        "settings-section-note:Lumina {}",
        env!("CARGO_PKG_VERSION")
    ));
    assert!(
        vcx.debug_bounds(note).is_some(),
        "the UPDATES note names the real build version: {note}"
    );
    let launch =
        |vcx: &mut gpui::VisualTestContext| view.update(vcx, |app, _| app.config.launch_view);
    assert_eq!(
        launch(vcx),
        LaunchView::LastViewed,
        "Last viewed is the default: today's behaviour"
    );

    click(vcx, "settings-launch-view-library");
    assert_eq!(launch(vcx), LaunchView::Library, "clicking Library sets it");
    click(vcx, "settings-launch-view-last");
    assert_eq!(
        launch(vcx),
        LaunchView::LastViewed,
        "clicking Last viewed sets it"
    );

    // Keyboard path: ←/→ pick the segment while General is shown.
    keystrokes(vcx, "left");
    assert_eq!(launch(vcx), LaunchView::Library, "← picks Library");
    keystrokes(vcx, "right");
    assert_eq!(launch(vcx), LaunchView::LastViewed, "→ picks Last viewed");
    view.update(vcx, |app, _| {
        assert!(app.preferences_open, "←/→ keep settings open");
        assert_eq!(
            app.preferences_tab,
            PreferencesTab::General,
            "←/→ do not change category"
        );
    });
    keystrokes(vcx, "down left");
    assert_eq!(
        launch(vcx),
        LaunchView::LastViewed,
        "← does nothing on another category"
    );
    keystrokes(vcx, "up");

    // A failed save keeps the saved value and says so (AGENTS §2).
    crate::app::state::SAVE_FAILS.with(|f| f.set(true));
    keystrokes(vcx, "left");
    click(vcx, "settings-launch-view-library");
    crate::app::state::SAVE_FAILS.with(|f| f.set(false));
    view.update(vcx, |app, _| {
        assert_eq!(
            app.config.launch_view,
            LaunchView::LastViewed,
            "a failed save reverts the control"
        );
        assert!(
            app.error_message.is_some(),
            "the failure is in the error banner"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

/// UI preferences away from their defaults, plus library, session and
/// editor values Reset must keep.
fn changed_config() -> crate::config::AppConfig {
    use crate::config::{AlbumSort, AppConfig, BrowseLayout, GroupLevel, LaunchView};
    AppConfig {
        sidebar_width: 320.0,
        inspector_width: 360.0,
        sidebar_visible: false,
        inspector_visible: true,
        grid_columns: 6,
        card_fit_mode: false,
        sort_field: Some("name".into()),
        sort_direction: Some("asc".into()),
        browse_layout: BrowseLayout::Mosaic,
        group_level: GroupLevel::Months,
        album_sort: AlbumSort::Created,
        album_panel_hidden: true,
        launch_view: LaunchView::Library,
        last_library_path: Some("/nonexistent/lumina-reset-test/Library".into()),
        recent_libraries: vec![
            "/nonexistent/lumina-reset-test/Library".into(),
            "/nonexistent/lumina-reset-test/Other".into(),
        ],
        external_editor: Some("com.example.editor".into()),
        last_view: Some("favorites".into()),
        slideshow_interval_secs: 10,
        video_speed: Some(1.5),
        ..AppConfig::default()
    }
}

/// The UI preferences, live and in the config, as Reset sees them.
#[derive(Debug, PartialEq)]
struct UiPrefs {
    sidebar_width: f32,
    inspector_width: f32,
    sidebar_collapsed: bool,
    inspector_open: bool,
    grid_columns: usize,
    card_fit_mode: bool,
    sort: lumina_core::SortSpec,
    browse_layout: crate::config::BrowseLayout,
    group_level: crate::config::GroupLevel,
    album_sort: crate::config::AlbumSort,
    album_panel_hidden: bool,
    launch_view: crate::config::LaunchView,
}

fn ui_prefs(
    view: &gpui::Entity<crate::app::LuminaApp>,
    vcx: &mut gpui::VisualTestContext,
) -> UiPrefs {
    view.update(vcx, |app, _| UiPrefs {
        sidebar_width: app.sidebar_width,
        inspector_width: app.inspector_width,
        sidebar_collapsed: app.sidebar_collapsed,
        inspector_open: app.inspector_open,
        grid_columns: app.grid_columns,
        card_fit_mode: app.card_fit_mode,
        sort: app.sort,
        browse_layout: app.config.browse_layout,
        group_level: app.config.group_level,
        album_sort: app.config.album_sort,
        album_panel_hidden: app.config.album_panel_hidden,
        launch_view: app.config.launch_view,
    })
}

/// What `AppConfig::default()` means for the live UI.
fn default_prefs() -> UiPrefs {
    let d = crate::config::AppConfig::default();
    UiPrefs {
        sidebar_width: d.sidebar_width,
        inspector_width: d.inspector_width,
        sidebar_collapsed: !d.sidebar_visible,
        inspector_open: d.inspector_visible,
        grid_columns: d.grid_columns,
        card_fit_mode: d.card_fit_mode,
        sort: d.restored_sort(),
        browse_layout: d.browse_layout,
        group_level: d.group_level,
        album_sort: d.album_sort,
        album_panel_hidden: d.album_panel_hidden,
        launch_view: d.launch_view,
    }
}

/// The values Reset must never touch.
fn kept_values(
    view: &gpui::Entity<crate::app::LuminaApp>,
    vcx: &mut gpui::VisualTestContext,
) -> serde_json::Value {
    view.update(vcx, |app, _| {
        let c = &app.config;
        serde_json::json!({
            "last_library_path": c.last_library_path,
            "recent_libraries": c.recent_libraries,
            "external_editor": c.external_editor,
            "last_view": c.last_view,
            "slideshow_interval_secs": c.slideshow_interval_secs,
            "video_speed": c.video_speed,
            "resume_library": c.resume_library,
            "templates": c.templates.len(),
        })
    })
}

/// The View menu's action titles, as the menu bar shows them.
fn view_menu_items(vcx: &mut gpui::VisualTestContext) -> Vec<String> {
    vcx.update(|_, cx| {
        cx.get_menus()
            .unwrap_or_default()
            .into_iter()
            .filter(|m| m.name.as_ref() == "View")
            .flat_map(|m| m.items)
            .filter_map(|item| match item {
                gpui::OwnedMenuItem::Action { name, .. } => Some(name.to_string()),
                _ => None,
            })
            .collect()
    })
}

fn reset_dialog_shown(vcx: &mut gpui::VisualTestContext) -> bool {
    render_ui(vcx);
    vcx.debug_bounds("settings-reset-dialog").is_some()
}

#[core::prelude::v1::test]
fn reset_to_defaults_asks_then_resets_ui_preferences_only() {
    let (temp_dir, engine) = seed_library("settings_reset", 2);
    crate::app::state::TEST_CONFIG.with(|c| *c.borrow_mut() = Some(changed_config()));
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    vcx.simulate_resize(gpui::size(gpui::px(1280.0), gpui::px(800.0)));
    focus_app(vcx, &view);
    let before = ui_prefs(&view, vcx);
    let kept = kept_values(&view, vcx);
    assert_ne!(
        before,
        default_prefs(),
        "the test starts away from the defaults"
    );
    keystrokes(vcx, "secondary-,");
    render_ui(vcx);

    let reset = vcx
        .debug_bounds("settings-reset")
        .expect("General's TopBar shows Reset to defaults");
    let header = vcx.debug_bounds("page-header").expect("the TopBar renders");
    assert!(
        header.contains(&reset.center()),
        "settings-reset sits in the TopBar actions"
    );
    assert_eq!(f32::from(reset.size.height), 32.0, "the button is 32 tall");

    click(vcx, "settings-reset");
    assert!(reset_dialog_shown(vcx), "Reset asks first");
    assert_eq!(ui_prefs(&view, vcx), before, "asking changes nothing");
    click(vcx, "settings-reset-cancel");
    assert!(!reset_dialog_shown(vcx), "Cancel closes the confirmation");
    assert_eq!(ui_prefs(&view, vcx), before, "Cancel changes nothing");

    click(vcx, "settings-reset");
    assert!(reset_dialog_shown(vcx));
    keystrokes(vcx, "escape");
    assert!(!reset_dialog_shown(vcx), "Esc closes the confirmation");
    view.update(vcx, |app, _| {
        assert!(
            app.preferences_open,
            "Esc in the confirmation keeps settings open"
        )
    });
    assert_eq!(ui_prefs(&view, vcx), before, "Esc changes nothing");

    click(vcx, "settings-reset");
    assert!(reset_dialog_shown(vcx));
    let backdrop = vcx
        .debug_bounds("settings-reset-backdrop")
        .expect("the confirmation has a dismiss backdrop");
    let card = vcx
        .debug_bounds("settings-reset-dialog")
        .expect("the confirmation card renders");
    // A point on the backdrop, outside the card.
    click_point(
        vcx,
        f32::from(backdrop.left()) + 8.0,
        f32::from(card.top()) / 2.0,
    );
    assert!(
        !reset_dialog_shown(vcx),
        "a backdrop click closes the confirmation"
    );
    view.update(vcx, |app, _| {
        assert!(app.preferences_open, "a backdrop click keeps settings open")
    });
    assert_eq!(
        ui_prefs(&view, vcx),
        before,
        "a backdrop click changes nothing"
    );

    // Sort changes reorder the grid: a position-based selection must go.
    view.update(vcx, |app, cx| app.select_index(0, cx));
    assert!(
        view.update(vcx, |app, _| app.selected_index.is_some()),
        "the test selects an item first"
    );
    click(vcx, "settings-reset");
    click(vcx, "settings-reset-confirm");
    assert!(!reset_dialog_shown(vcx), "Reset closes the confirmation");
    view.update(vcx, |app, _| {
        assert!(
            app.selected_index.is_none() && app.selected_indices.is_empty(),
            "a reset that changes the sort clears the selection (AGENTS §5.3)"
        )
    });
    assert!(
        view_menu_items(vcx).iter().any(|n| n == "Hide Sidebar"),
        "the View menu follows the reset (sidebar shown): {:?}",
        view_menu_items(vcx)
    );
    assert_eq!(
        ui_prefs(&view, vcx),
        default_prefs(),
        "Reset puts the UI preferences, live and saved, back to the defaults"
    );
    view.update(vcx, |app, _| {
        let d = crate::config::AppConfig::default();
        assert_eq!(
            (app.config.sidebar_width, app.config.grid_columns),
            (d.sidebar_width, d.grid_columns),
            "the config the next save writes holds the defaults too"
        );
        assert!(app.error_message.is_none(), "a good save shows no error");
    });
    assert_eq!(
        kept_values(&view, vcx),
        kept,
        "library, recents, session, editor and playback settings are kept"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn reset_to_defaults_by_keyboard() {
    let (temp_dir, engine) = seed_library("settings_reset_keys", 2);
    crate::app::state::TEST_CONFIG.with(|c| *c.borrow_mut() = Some(changed_config()));
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    vcx.simulate_resize(gpui::size(gpui::px(1280.0), gpui::px(800.0)));
    focus_app(vcx, &view);
    let before = ui_prefs(&view, vcx);
    let palette_reset = |vcx: &mut gpui::VisualTestContext| {
        keystrokes(vcx, "secondary-k");
        type_text(vcx, "reset settings");
        keystrokes(vcx, "enter");
    };

    palette_reset(vcx);
    assert!(
        reset_dialog_shown(vcx),
        "⌘K Reset Settings to Defaults opens settings with the confirmation"
    );
    // The confirmation takes every key: no category or segment moves under it.
    keystrokes(vcx, "down left secondary-backspace");
    view.update(vcx, |app, _| {
        assert!(
            app.preferences_open,
            "settings are open behind the confirmation"
        );
        assert_eq!(
            app.preferences_tab,
            PreferencesTab::General,
            "↓ is absorbed"
        );
        assert!(app.delete_confirm_state.is_none(), "⌘⌫ is absorbed");
    });
    assert_eq!(ui_prefs(&view, vcx), before, "absorbed keys change nothing");
    keystrokes(vcx, "escape");
    assert!(!reset_dialog_shown(vcx), "Esc cancels");
    assert_eq!(ui_prefs(&view, vcx), before, "Esc changes nothing");

    keystrokes(vcx, "escape");
    palette_reset(vcx);
    assert!(reset_dialog_shown(vcx));
    keystrokes(vcx, "enter");
    assert!(!reset_dialog_shown(vcx), "↵ confirms");
    assert_eq!(ui_prefs(&view, vcx), default_prefs(), "↵ resets");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn reset_to_defaults_reverts_when_the_save_fails() {
    let (temp_dir, engine) = seed_library("settings_reset_fails", 2);
    crate::app::state::TEST_CONFIG.with(|c| *c.borrow_mut() = Some(changed_config()));
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    vcx.simulate_resize(gpui::size(gpui::px(1280.0), gpui::px(800.0)));
    focus_app(vcx, &view);
    let before = ui_prefs(&view, vcx);
    let kept = kept_values(&view, vcx);
    keystrokes(vcx, "secondary-,");
    render_ui(vcx);

    click(vcx, "settings-reset");
    crate::app::state::SAVE_FAILS.with(|f| f.set(true));
    click(vcx, "settings-reset-confirm");
    crate::app::state::SAVE_FAILS.with(|f| f.set(false));
    assert_eq!(
        ui_prefs(&view, vcx),
        before,
        "a failed save puts every preference back, live and in the config (AGENTS §2)"
    );
    assert_eq!(kept_values(&view, vcx), kept, "kept values stay kept");
    assert!(
        view_menu_items(vcx).iter().any(|n| n == "Show Sidebar"),
        "the View menu follows the revert (sidebar hidden): {:?}",
        view_menu_items(vcx)
    );
    view.update(vcx, |app, _| {
        assert!(
            app.error_message.is_some(),
            "the failure is in the error banner"
        );
    });
    assert!(
        !reset_dialog_shown(vcx),
        "the confirmation closes either way"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn settings_has_library_downloads_and_storage_categories() {
    use crate::app::library::FOLDER_PICKS;
    let (temp_dir, engine) = seed_library("settings_categories", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    vcx.simulate_resize(gpui::size(gpui::px(1280.0), gpui::px(800.0)));
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-,");
    render_ui(vcx);

    let sidebar = vcx
        .debug_bounds("settings-sidebar")
        .expect("the settings sidebar renders");
    let tops: Vec<f32> = [
        "settings-category-general",
        "settings-category-library",
        "settings-category-downloads",
        "settings-category-storage",
        "settings-category-about",
    ]
    .into_iter()
    .map(|selector| {
        let bounds = vcx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("{selector} renders"));
        assert!(
            sidebar.contains(&bounds.center()),
            "{selector} sits in the settings sidebar"
        );
        f32::from(bounds.top())
    })
    .collect();
    assert!(
        tops.windows(2).all(|pair| pair[0] < pair[1]),
        "the sidebar runs General, Library, Downloads, Storage, About: {tops:?}"
    );
    assert!(
        vcx.debug_bounds("page-meta-text:Launch and external editor")
            .is_some(),
        "General's meta no longer mentions the library"
    );

    let selected =
        |vcx: &mut gpui::VisualTestContext| view.update(vcx, |app, _| app.preferences_tab);
    keystrokes(vcx, "down");
    render_ui(vcx);
    assert_eq!(
        selected(vcx),
        PreferencesTab::Library,
        "↓ from General is Library"
    );
    for marker in [
        "page-title-text:Library",
        "page-meta-text:Where the library lives and how big it is",
        "preferences-change-library-text:Open Another…",
        "preferences-rescan-text:Rescan Library",
    ] {
        assert!(
            vcx.debug_bounds(marker).is_some(),
            "the Library page shows {marker}"
        );
    }
    let size = format!(
        "preferences-indexed-text:{} · 2 items",
        crate::app::types::format_bytes(storage_report(&view, vcx).library_bytes())
    );
    assert!(
        vcx.debug_bounds(leak(size.clone())).is_some(),
        "Library size reads {size}"
    );
    let row = vcx
        .debug_bounds("settings-row-library")
        .expect("the Library location row renders");
    assert!(
        row.contains(&vcx.debug_bounds("settings-path-library").unwrap().center()),
        "the path pill sits in the Library location row"
    );

    let picks = FOLDER_PICKS.with(|n| n.get());
    click(vcx, "preferences-change-library");
    assert_eq!(
        FOLDER_PICKS.with(|n| n.get()),
        picks + 1,
        "Open Another… asks for another library folder"
    );
    view.update(vcx, |app, _| {
        if let Some(state) = app.grid_state.as_mut() {
            state.is_scanning = false;
        }
    });
    click(vcx, "preferences-rescan");
    view.update(vcx, |app, _| {
        assert!(
            app.grid_state.as_ref().is_some_and(|s| s.is_scanning),
            "Rescan Library on the Library page starts a scan"
        );
    });

    click(vcx, "settings-category-downloads");
    assert_eq!(selected(vcx), PreferencesTab::Downloads);
    for marker in [
        "page-title-text:Downloads",
        "page-meta-text:How videos from links are downloaded and filed away",
        "settings-section-note:Applies to videos imported from a link",
        "settings-concurrency",
    ] {
        assert!(
            vcx.debug_bounds(marker).is_some(),
            "the Downloads page shows {marker}"
        );
    }

    click(vcx, "settings-category-storage");
    assert_eq!(selected(vcx), PreferencesTab::Storage);
    for selector in [
        "preferences-reveal-cache",
        "preferences-cache-size",
        "preferences-clear-cache",
        "preferences-reveal-config",
        "preferences-reveal-logs",
    ] {
        assert!(
            vcx.debug_bounds(selector).is_some(),
            "Storage keeps {selector}"
        );
    }

    // Keyboard path: ⌘K Settings: Downloads.
    keystrokes(vcx, "escape");
    keystrokes(vcx, "secondary-k");
    type_text(vcx, "settings downloads");
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert!(app.preferences_open, "Settings: Downloads opens settings");
        assert_eq!(
            app.preferences_tab,
            PreferencesTab::Downloads,
            "on the Downloads category"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn downloads_settings_buttons_open_downloads_category() {
    use super::downloads::{install, queue_app};
    use super::downloads_screen::set_rows;
    let (_dir, engine, fake, mut cx) = queue_app("settings_downloads_buttons");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    vcx.simulate_resize(gpui::size(gpui::px(1600.0), gpui::px(1000.0)));
    set_rows(&view, vcx, &[lumina_core::DownloadState::Done]);
    view.update(vcx, |app, cx| app.show_downloads_screen(cx));
    render_ui(vcx);

    click(vcx, "downloads-settings");
    view.update(vcx, |app, cx| {
        assert!(
            app.preferences_open,
            "the screen's settings button opens settings"
        );
        assert_eq!(
            app.preferences_tab,
            PreferencesTab::Downloads,
            "on the Downloads category"
        );
        app.close_preferences(cx);
        app.toggle_downloads_panel(cx);
        assert!(app.downloads.open, "the popover opens");
    });
    render_ui(vcx);

    click(vcx, "downloads-header-settings");
    view.update(vcx, |app, _| {
        assert!(
            !app.downloads.open,
            "the popover's settings button closes it"
        );
        assert!(app.preferences_open, "and opens settings");
        assert_eq!(
            app.preferences_tab,
            PreferencesTab::Downloads,
            "on the Downloads category"
        );
    });
}

#[core::prelude::v1::test]
fn concurrency_segmented_responds_to_keys() {
    let (temp_dir, engine) = seed_library("settings_concurrency_keys", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    vcx.simulate_resize(gpui::size(gpui::px(1280.0), gpui::px(800.0)));
    focus_app(vcx, &view);
    view.update(vcx, |app, cx| {
        app.open_preferences_at(PreferencesTab::Downloads, cx)
    });
    render_ui(vcx);
    let concurrency = |vcx: &mut gpui::VisualTestContext| {
        view.update(vcx, |app, _| app.config.download_concurrency)
    };
    assert_eq!(concurrency(vcx), 3, "3 at once is the default");
    for selector in [
        "settings-concurrency",
        "settings-concurrency-1",
        "settings-concurrency-2",
        "settings-concurrency-3",
        "settings-concurrency-5",
        "settings-row-concurrency",
    ] {
        assert!(
            vcx.debug_bounds(selector).is_some(),
            "the Downloads page shows {selector}"
        );
    }

    keystrokes(vcx, "right");
    assert_eq!(concurrency(vcx), 5, "→ moves 3 to 5");
    keystrokes(vcx, "right");
    assert_eq!(concurrency(vcx), 5, "→ stops at 5");
    keystrokes(vcx, "left left");
    assert_eq!(concurrency(vcx), 2, "← moves 5 to 3, then 2");
    click(vcx, "settings-concurrency-1");
    assert_eq!(concurrency(vcx), 1, "clicking 1 picks it");
    keystrokes(vcx, "left");
    assert_eq!(concurrency(vcx), 1, "← stops at 1");
    view.update(vcx, |app, _| {
        assert!(app.preferences_open, "←/→ keep settings open");
        assert_eq!(app.preferences_tab, PreferencesTab::Downloads);
    });

    // ←/→ on another category leave the setting alone.
    keystrokes(vcx, "up right");
    assert_eq!(concurrency(vcx), 1, "→ on Library changes nothing");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn concurrency_save_failure_reverts() {
    let (temp_dir, engine) = seed_library("settings_concurrency_fails", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    vcx.simulate_resize(gpui::size(gpui::px(1280.0), gpui::px(800.0)));
    view.update(vcx, |app, cx| {
        app.open_preferences_at(PreferencesTab::Downloads, cx)
    });
    render_ui(vcx);

    crate::app::state::SAVE_FAILS.with(|f| f.set(true));
    click(vcx, "settings-concurrency-5");
    crate::app::state::SAVE_FAILS.with(|f| f.set(false));
    view.update(vcx, |app, _| {
        assert_eq!(
            app.config.download_concurrency, 3,
            "a failed save puts the control back (AGENTS §2)"
        );
        assert!(
            app.error_message.is_some(),
            "the failure is in the error banner"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

/// A seeded item copied as a `kind` file of `bytes` at `name` in `root`.
fn sized_item(
    base: &lumina_core::MediaItem,
    root: &std::path::Path,
    name: &str,
    kind: lumina_core::MediaType,
    bytes: u64,
) -> lumina_core::MediaItem {
    let path = root.join(name);
    lumina_core::MediaItem {
        id: lumina_core::MediaId(0),
        parent_dir: path.parent().unwrap().to_string_lossy().to_string(),
        file_name: path.file_name().unwrap().to_string_lossy().to_string(),
        path,
        media_type: kind,
        file_size: bytes,
        ..base.clone()
    }
}

fn storage_report(
    view: &gpui::Entity<crate::app::LuminaApp>,
    vcx: &mut gpui::VisualTestContext,
) -> crate::app::library_storage::LibraryStorage {
    view.update(vcx, |app, _| app.preferences_storage.clone())
        .expect("the storage report lands once the Library category shows")
}

#[core::prelude::v1::test]
fn storage_card_shows_segments() {
    use crate::app::types::format_bytes;
    use lumina_core::MediaType;
    let (temp_dir, engine) = seed_library("settings_storage_card", 2);
    let base = engine.get_item_slice(0..1)[0].clone();
    let photo = base.file_size;
    let catalog = engine.catalog();
    catalog
        .insert_or_update_batch(&[sized_item(
            &base,
            &temp_dir,
            "clip.mp4",
            MediaType::Video,
            4096,
        )])
        .unwrap();
    catalog
        .insert_with_source(
            &sized_item(&base, &temp_dir, "Imports/dl.mp4", MediaType::Video, 8192),
            "https://example.com/watch?v=1",
            None,
        )
        .unwrap();
    let cache = thumbnail_cache_dir();
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::write(cache.join("thumb.jpg"), vec![0u8; 1024]).unwrap();

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    vcx.simulate_resize(gpui::size(gpui::px(1280.0), gpui::px(800.0)));
    view.update(vcx, |app, cx| {
        app.open_preferences_at(PreferencesTab::Library, cx)
    });
    render_ui(vcx);
    render_ui(vcx);

    let report = storage_report(&view, vcx);
    assert_eq!(
        report.summary,
        lumina_core::StorageSummary {
            photo_bytes: 2 * photo,
            video_bytes: 4096,
            download_bytes: 8192,
        },
        "the card sums the library by kind, the download once"
    );
    assert_eq!(report.cache.bytes, 1024, "and the thumbnail cache");

    let bar = vcx
        .debug_bounds("settings-storage-bar")
        .expect("the storage bar renders");
    assert_eq!(f32::from(bar.size.height), 10.0, "the bar is 10 tall");
    let parts = [
        ("photos", 2 * photo),
        ("videos", 4096),
        ("downloads", 8192),
        ("cache", 1024),
    ];
    let sum: u64 = parts.iter().map(|(_, b)| b).sum();
    for (name, bytes) in parts {
        let seg = vcx
            .debug_bounds(leak(format!("settings-storage-seg-{name}")))
            .unwrap_or_else(|| panic!("the {name} segment renders"));
        let want = f32::from(bar.size.width) * bytes as f32 / sum as f32;
        assert!(
            (f32::from(seg.size.width) - want).abs() <= 1.0,
            "the {name} segment is its share of the bar: {:?} vs {want}",
            seg.size.width
        );
    }
    for (label, bytes) in [
        ("Photos", 2 * photo),
        ("Videos", 4096),
        ("Downloads", 8192),
        ("Cache", 1024),
    ] {
        let marker = leak(format!(
            "settings-storage-legend-text:{label} {}",
            format_bytes(bytes)
        ));
        assert!(
            vcx.debug_bounds(marker).is_some(),
            "the legend shows {marker}"
        );
    }
    let used = match report.capacity {
        Some((total, available)) => format!(
            "{} of {} used",
            format_bytes(total - available),
            format_bytes(total)
        ),
        None => format_bytes(report.library_bytes()),
    };
    assert!(
        vcx.debug_bounds(leak(format!("settings-storage-used-text:{used}")))
            .is_some(),
        "the card's header reads {used}"
    );
    let size = leak(format!(
        "preferences-indexed-text:{} · 4 items",
        format_bytes(2 * photo + 4096 + 8192)
    ));
    assert!(
        vcx.debug_bounds(size).is_some(),
        "Library size reads {size}"
    );

    assert!(
        vcx.debug_bounds("settings-storage-clear-cache-text:Clear cache · 1.0 KB")
            .is_some(),
        "the Clear cache button names the cache size"
    );
    click(vcx, "settings-storage-clear-cache");
    render_ui(vcx);
    let left = std::fs::read_dir(&cache).map(|d| d.count()).unwrap_or(0);
    assert_eq!(left, 0, "Clear cache empties the thumbnail cache");
    assert_eq!(
        storage_report(&view, vcx).cache.bytes,
        0,
        "and the card refreshes"
    );
    assert!(
        vcx.debug_bounds("settings-storage-legend-text:Cache 0 B")
            .is_some(),
        "the legend shows the emptied cache"
    );

    let _ = std::fs::remove_dir_all(&cache);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn storage_card_handles_empty_library() {
    use crate::app::library_storage::CAPACITY_UNKNOWN;
    let (temp_dir, engine) = seed_library("settings_storage_empty", 0);
    let _ = std::fs::remove_dir_all(thumbnail_cache_dir());
    CAPACITY_UNKNOWN.with(|c| c.set(true));
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    vcx.simulate_resize(gpui::size(gpui::px(1280.0), gpui::px(800.0)));
    view.update(vcx, |app, cx| {
        app.open_preferences_at(PreferencesTab::Library, cx)
    });
    render_ui(vcx);
    render_ui(vcx);
    CAPACITY_UNKNOWN.with(|c| c.set(false));

    let report = storage_report(&view, vcx);
    assert_eq!(report.summary, lumina_core::StorageSummary::default());
    assert_eq!(report.capacity, None, "the seam hides the volume");
    assert!(
        vcx.debug_bounds("settings-storage-bar").is_some(),
        "an empty bar still renders"
    );
    for name in ["photos", "videos", "downloads", "cache"] {
        assert!(
            vcx.debug_bounds(leak(format!("settings-storage-seg-{name}")))
                .is_none(),
            "nothing to show: no {name} segment"
        );
    }
    for label in ["Photos", "Videos", "Downloads", "Cache"] {
        let marker = leak(format!("settings-storage-legend-text:{label} 0 B"));
        assert!(
            vcx.debug_bounds(marker).is_some(),
            "the legend lists {marker}"
        );
    }
    assert!(
        vcx.debug_bounds("settings-storage-used-text:0 B").is_some(),
        "an unknown volume shows the library total alone, with no \"of\""
    );
    assert!(
        vcx.debug_bounds("preferences-indexed-text:0 B · 0 items")
            .is_some(),
        "Library size reads 0 B · 0 items"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn storage_card_hides_without_a_reading_for_this_library() {
    let (temp_dir, engine) = seed_library("settings_storage_stale", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    vcx.simulate_resize(gpui::size(gpui::px(1280.0), gpui::px(800.0)));
    view.update(vcx, |app, cx| {
        app.open_preferences_at(PreferencesTab::Library, cx)
    });
    render_ui(vcx);
    render_ui(vcx);
    let reading = storage_report(&view, vcx);
    let sized = leak(format!(
        "preferences-indexed-text:{} · 2 items",
        crate::app::types::format_bytes(reading.library_bytes())
    ));
    assert!(vcx.debug_bounds("settings-storage-card").is_some());
    assert!(
        vcx.debug_bounds(sized).is_some(),
        "Library size reads {sized}"
    );

    let hidden = |vcx: &mut gpui::VisualTestContext, why: &str| {
        render_ui(vcx);
        assert!(
            vcx.debug_bounds("settings-storage-card").is_none(),
            "{why}: no storage card"
        );
        assert!(
            vcx.debug_bounds("preferences-indexed-text:2 items")
                .is_some(),
            "{why}: Library size shows the count alone"
        );
    };
    // No reading yet (settings just opened).
    view.update(vcx, |app, _| app.preferences_storage = None);
    hidden(vcx, "before the first reading lands");
    // A reading for another library.
    view.update(vcx, |app, _| {
        app.preferences_storage = Some(crate::app::library_storage::LibraryStorage {
            root: "/nonexistent/lumina-other-library".into(),
            ..reading.clone()
        })
    });
    hidden(vcx, "a reading for another library is not shown");
    // No library open.
    view.update(vcx, |app, _| {
        app.preferences_storage = Some(reading.clone());
        app.grid_state = None;
    });
    hidden(vcx, "with no library open");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn downloads_path_row_reveals_imports() {
    use crate::app::preferences::REVEALED;
    let (temp_dir, engine) = seed_library("settings_downloads_path", 1);
    let root = engine.library_root().to_path_buf();
    let imports = root.join(lumina_core::import::IMPORTS_DIR);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    vcx.simulate_resize(gpui::size(gpui::px(1280.0), gpui::px(800.0)));
    view.update(vcx, |app, cx| {
        app.open_preferences_at(PreferencesTab::Downloads, cx)
    });
    render_ui(vcx);

    let shown = crate::views::preferences::tilde(&imports);
    for selector in [
        "settings-row-downloads-path",
        "settings-downloads-path",
        "settings-downloads-reveal-text:Show in Finder",
    ] {
        assert!(
            vcx.debug_bounds(selector).is_some(),
            "the Downloads page shows {selector}"
        );
    }
    assert!(
        vcx.debug_bounds(leak(format!("settings-downloads-path-text:{shown}")))
            .is_some(),
        "the path pill reads {shown}, the library's Imports folder"
    );

    let revealed = || REVEALED.with(|r| r.borrow_mut().drain(..).collect::<Vec<_>>());
    revealed();
    // No download yet: Imports does not exist, so the library is revealed.
    assert!(!imports.exists(), "a fresh library has no Imports folder");
    click(vcx, "settings-downloads-reveal");
    assert_eq!(
        revealed(),
        std::slice::from_ref(&root),
        "with no Imports folder, Show in Finder reveals the library"
    );
    assert!(!imports.exists(), "revealing creates no folder");

    std::fs::create_dir_all(&imports).unwrap();
    click(vcx, "settings-downloads-reveal");
    assert_eq!(
        revealed(),
        std::slice::from_ref(&imports),
        "Show in Finder reveals the Imports folder"
    );

    // No library open: there is nowhere downloads go.
    view.update(vcx, |app, _| app.grid_state = None);
    render_ui(vcx);
    for gone in ["settings-downloads-path", "settings-downloads-reveal"] {
        assert!(
            vcx.debug_bounds(gone).is_none(),
            "{gone} hides with no library open"
        );
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}

/// The settings button the Tab walk is on, by its selector.
fn settings_focus(
    view: &gpui::Entity<crate::app::LuminaApp>,
    vcx: &mut gpui::VisualTestContext,
) -> Option<&'static str> {
    view.update(vcx, |app, _| app.settings_focus.map(|b| b.id()))
}

#[core::prelude::v1::test]
fn settings_tab_walks_buttons_and_return_activates() {
    use crate::app::preferences::REVEALED;
    let (temp_dir, engine) = seed_library("settings_tab_walk", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    vcx.simulate_resize(gpui::size(gpui::px(1280.0), gpui::px(800.0)));
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-, down down down");
    assert_eq!(
        view.update(vcx, |app, _| app.preferences_tab),
        PreferencesTab::Storage
    );
    assert_eq!(
        settings_focus(&view, vcx),
        None,
        "nothing is focused at first"
    );
    keystrokes(vcx, "secondary-tab alt-tab");
    assert_eq!(
        settings_focus(&view, vcx),
        None,
        "⌘Tab and ⌥Tab stay absorbed and move no focus"
    );

    let order = [
        "preferences-reveal-cache",
        "preferences-clear-cache",
        "preferences-reveal-config",
        "preferences-reveal-logs",
    ];
    for expected in order {
        keystrokes(vcx, "tab");
        assert_eq!(
            settings_focus(&view, vcx),
            Some(expected),
            "Tab walks Storage's buttons in order"
        );
        assert!(vcx.debug_bounds(expected).is_some(), "{expected} renders");
    }
    keystrokes(vcx, "tab");
    assert_eq!(
        settings_focus(&view, vcx),
        Some(order[0]),
        "Tab past the last button wraps to the first"
    );
    keystrokes(vcx, "shift-tab");
    assert_eq!(
        settings_focus(&view, vcx),
        Some(order[3]),
        "⇧Tab before the first button wraps to the last"
    );
    keystrokes(vcx, "shift-tab");
    assert_eq!(settings_focus(&view, vcx), Some(order[2]), "⇧Tab goes back");

    // ↵ on Clear Cache clears this test's cache folder.
    let cache = thumbnail_cache_dir();
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::write(cache.join("1.jpg"), b"thumb").unwrap();
    keystrokes(vcx, "shift-tab");
    assert_eq!(settings_focus(&view, vcx), Some(order[1]));
    keystrokes(vcx, "enter");
    assert!(
        !cache.join("1.jpg").exists(),
        "↵ on Clear Cache clears the cache, as a click does"
    );
    view.update(vcx, |app, _| {
        assert!(app.preferences_cache_cleared, "and confirms inline");
    });

    // Space on Reveal Cache reveals it, as a click does.
    REVEALED.with(|r| r.borrow_mut().clear());
    keystrokes(vcx, "shift-tab space");
    assert_eq!(settings_focus(&view, vcx), Some(order[0]));
    assert_eq!(
        REVEALED.with(|r| r.borrow_mut().drain(..).collect::<Vec<_>>()),
        vec![cache.clone()],
        "Space on Reveal Cache reveals the cache folder"
    );
    keystrokes(vcx, "enter");
    assert_eq!(
        REVEALED.with(|r| r.borrow_mut().drain(..).collect::<Vec<_>>()),
        vec![cache.clone()],
        "↵ on Reveal Cache reveals the cache folder"
    );
    assert!(
        view.update(vcx, |app, _| app.preferences_open),
        "↵ and Space do not close settings"
    );

    keystrokes(vcx, "escape");
    assert_eq!(settings_focus(&view, vcx), None, "closing clears the focus");

    let _ = std::fs::remove_dir_all(&cache);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn settings_tab_focus_resets_on_category_change() {
    use crate::app::preferences::REVEALED;
    let (temp_dir, engine) = seed_library("settings_tab_reset", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    vcx.simulate_resize(gpui::size(gpui::px(1280.0), gpui::px(800.0)));
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-, down down down tab");
    assert_eq!(settings_focus(&view, vcx), Some("preferences-reveal-cache"));
    keystrokes(vcx, "down");
    assert_eq!(
        view.update(vcx, |app, _| app.preferences_tab),
        PreferencesTab::About
    );
    assert_eq!(
        settings_focus(&view, vcx),
        None,
        "changing category clears the focus"
    );
    REVEALED.with(|r| r.borrow_mut().clear());
    keystrokes(vcx, "enter space");
    assert!(
        REVEALED.with(|r| r.borrow().is_empty()),
        "with nothing focused ↵ and Space do nothing"
    );
    view.update(vcx, |app, _| {
        assert!(app.preferences_open, "↵ does not close settings");
        assert!(app.error_message.is_none());
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn settings_tab_on_library_rescans_and_opens_another() {
    use crate::app::library::FOLDER_PICKS;
    let (temp_dir, engine) = seed_library("settings_tab_library", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    vcx.simulate_resize(gpui::size(gpui::px(1280.0), gpui::px(800.0)));
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-, down");
    render_ui(vcx);
    storage_report(&view, vcx);
    render_ui(vcx);
    assert!(vcx.debug_bounds("settings-storage-card").is_some());

    keystrokes(vcx, "tab");
    assert_eq!(
        settings_focus(&view, vcx),
        Some("settings-storage-clear-cache"),
        "the storage card's Clear cache comes first"
    );
    // The card goes: a stale focus must not press the button now first.
    let picks = FOLDER_PICKS.with(|n| n.get());
    view.update(vcx, |app, _| app.preferences_storage = None);
    render_ui(vcx);
    assert!(vcx.debug_bounds("settings-storage-card").is_none());
    keystrokes(vcx, "enter");
    assert_eq!(
        FOLDER_PICKS.with(|n| n.get()),
        picks,
        "↵ on a button that is gone presses nothing"
    );
    view.update(vcx, |app, _| assert!(!app.preferences_cache_cleared));

    keystrokes(vcx, "tab");
    assert_eq!(
        settings_focus(&view, vcx),
        Some("preferences-change-library")
    );
    keystrokes(vcx, "enter");
    assert_eq!(
        FOLDER_PICKS.with(|n| n.get()),
        picks + 1,
        "↵ on Open Another… asks for another library folder"
    );

    view.update(vcx, |app, _| {
        if let Some(state) = app.grid_state.as_mut() {
            state.is_scanning = false;
        }
    });
    keystrokes(vcx, "tab");
    assert_eq!(settings_focus(&view, vcx), Some("preferences-rescan"));
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert!(
            app.grid_state.as_ref().is_some_and(|s| s.is_scanning),
            "↵ on Rescan Library starts a scan"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn settings_tab_on_general_reaches_editor_and_reset() {
    let (temp_dir, engine) = seed_library("settings_tab_general", 1);
    crate::app::state::TEST_CONFIG.with(|c| *c.borrow_mut() = Some(changed_config()));
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    vcx.simulate_resize(gpui::size(gpui::px(1280.0), gpui::px(800.0)));
    focus_app(vcx, &view);
    let before = ui_prefs(&view, vcx);
    keystrokes(vcx, "secondary-,");

    keystrokes(vcx, "tab");
    assert_eq!(
        settings_focus(&view, vcx),
        Some("settings-reset"),
        "the TopBar's Reset to defaults comes first"
    );
    keystrokes(vcx, "tab");
    assert_eq!(
        settings_focus(&view, vcx),
        Some("preferences-choose-editor")
    );
    keystrokes(vcx, "shift-tab enter");
    assert!(reset_dialog_shown(vcx), "↵ on Reset asks first");

    // The confirmation takes the keys: Tab, Space and ↓ act on nothing.
    keystrokes(vcx, "tab space down");
    assert!(reset_dialog_shown(vcx), "the confirmation stays up");
    view.update(vcx, |app, _| {
        assert_eq!(app.preferences_tab, PreferencesTab::General);
        assert_eq!(app.settings_focus.map(|b| b.id()), Some("settings-reset"));
    });
    keystrokes(vcx, "escape");
    assert!(!reset_dialog_shown(vcx), "Esc cancels");
    assert_eq!(ui_prefs(&view, vcx), before, "nothing behind it fired");
    assert!(
        view.update(vcx, |app, _| app.preferences_open),
        "Esc closes only the confirmation"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}
