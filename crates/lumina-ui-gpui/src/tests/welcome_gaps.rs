//! Welcome screen gaps from the design index (plans 11 and 18): ten recent
//! libraries, and Reset to defaults with no library open.

use super::downloads::TempLibrary;
use super::welcome::temp_folder;
use crate::tests::ui_automation::{click, focus_app, keystrokes, leak, render_ui, wheel};
use crate::LuminaApp;
use gpui::{Entity, VisualTestContext};

/// The welcome screen with `config` as the startup config (never saved).
fn welcome(
    cx: &mut gpui::TestAppContext,
    config: crate::config::AppConfig,
) -> (Entity<LuminaApp>, &mut VisualTestContext) {
    crate::app::state::TEST_CONFIG.with(|c| *c.borrow_mut() = Some(config));
    let (view, vcx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    render_ui(vcx);
    focus_app(vcx, &view);
    (view, vcx)
}

/// Plan 11: ten recents in a 900×600 window. Each row is one 32 px line,
/// and the welcome body scrolls the tenth fully into view.
#[core::prelude::v1::test]
fn ten_recents_are_one_line_rows_and_the_tenth_scrolls_into_view() {
    let folders: Vec<TempLibrary> = (0..10).map(|i| temp_folder(&format!("ten_{i}"))).collect();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = welcome(
        &mut cx,
        crate::config::AppConfig {
            recent_libraries: folders.iter().map(|f| f.to_path_buf()).collect(),
            ..Default::default()
        },
    );
    vcx.simulate_resize(gpui::size(gpui::px(900.0), gpui::px(600.0)));
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.grid_state.is_none(), "precondition: no library is open")
    });

    for i in 0..10 {
        let row = vcx
            .debug_bounds(leak(format!("welcome-recent-{i}")))
            .unwrap_or_else(|| panic!("recent {i} is listed: all ten recents show"));
        let line = vcx
            .debug_bounds(leak(format!("welcome-recent-path-{i}")))
            .unwrap_or_else(|| panic!("recent {i} has a path line"));
        assert_eq!(
            f32::from(row.size.height),
            32.0,
            "recent {i} must be one 32 px row"
        );
        assert!(
            f32::from(line.size.height) < 20.0
                && line.left() >= row.left()
                && line.right() <= row.right(),
            "recent {i}'s path must be one line inside its row: {line:?} in {row:?}"
        );
    }

    let header = vcx.debug_bounds("page-header").expect("header renders");
    let tenth = vcx.debug_bounds("welcome-recent-9").unwrap();
    assert!(
        tenth.top() > gpui::px(600.0),
        "precondition: at 900×600 the tenth recent starts below the window: {tenth:?}"
    );
    let body = vcx.debug_bounds("welcome-body").expect("body renders");
    for _ in 0..8 {
        wheel(
            vcx,
            body.center(),
            gpui::ScrollDelta::Pixels(gpui::point(gpui::px(0.0), gpui::px(-400.0))),
            gpui::Modifiers::default(),
        );
    }
    let tenth = vcx.debug_bounds("welcome-recent-9").unwrap();
    assert!(
        tenth.top() >= header.bottom() && tenth.bottom() <= gpui::px(600.0),
        "scrolling must bring the tenth recent fully into the window: {tenth:?}"
    );
}

/// ↑/↓ keep the focused recent inside the scrolling welcome body (nearest
/// edge), in a short window with ten recents.
#[core::prelude::v1::test]
fn arrow_focus_scrolls_the_recent_into_view() {
    let folders: Vec<TempLibrary> = (0..10)
        .map(|i| temp_folder(&format!("arrow_{i}")))
        .collect();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = welcome(
        &mut cx,
        crate::config::AppConfig {
            recent_libraries: folders.iter().map(|f| f.to_path_buf()).collect(),
            ..Default::default()
        },
    );
    vcx.simulate_resize(gpui::size(gpui::px(960.0), gpui::px(600.0)));
    render_ui(vcx);
    let inside = |vcx: &mut VisualTestContext, i: usize| {
        let body = vcx.debug_bounds("welcome-body").expect("body renders");
        let row = vcx
            .debug_bounds(leak(format!("welcome-recent-{i}")))
            .unwrap_or_else(|| panic!("recent {i} renders"));
        (
            row.top() >= body.top() && row.bottom() <= body.bottom(),
            row,
            body,
        )
    };
    assert!(
        !inside(vcx, 9).0,
        "precondition: at 960×600 the tenth recent starts outside the body"
    );
    for i in 0..10 {
        keystrokes(vcx, "down");
        let (shown, row, body) = inside(vcx, i);
        assert!(
            shown,
            "↓ to recent {i} must scroll it into the body: {row:?} in {body:?}"
        );
    }
    view.update(vcx, |app, _| {
        assert_eq!(app.welcome.focused_recent, Some(9))
    });
    for i in (0..9).rev() {
        keystrokes(vcx, "up");
        let (shown, row, body) = inside(vcx, i);
        assert!(
            shown,
            "↑ to recent {i} must scroll it into the body: {row:?} in {body:?}"
        );
    }
}

/// Plan 18: Reset to defaults from the welcome screen asks first, then
/// resets the UI preferences only, like the library case
/// (`tests::preferences::reset_to_defaults_asks_then_resets_ui_preferences_only`).
#[core::prelude::v1::test]
fn reset_to_defaults_from_welcome_resets_ui_preferences_only() {
    use crate::config::{AlbumSort, AppConfig, BrowseLayout, GroupLevel, LaunchView};
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = welcome(
        &mut cx,
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
            recent_libraries: vec!["/nonexistent/lumina-welcome-reset/Library".into()],
            external_editor: Some("com.example.editor".into()),
            slideshow_interval_secs: 10,
            video_speed: Some(1.5),
            ..AppConfig::default()
        },
    );
    vcx.simulate_resize(gpui::size(gpui::px(1280.0), gpui::px(800.0)));
    // The config the next save writes from (the live layout is checked below).
    let snapshot = |view: &Entity<LuminaApp>, vcx: &mut VisualTestContext| {
        view.update(vcx, |app, _| app.config.clone())
    };
    let before = snapshot(&view, vcx);
    view.update(vcx, |app, _| {
        assert!(app.grid_state.is_none(), "precondition: no library is open")
    });
    keystrokes(vcx, "secondary-,");
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("settings-page").is_some(),
        "precondition: settings paint on the welcome screen"
    );

    click(vcx, "settings-reset");
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("settings-reset-dialog").is_some(),
        "Reset asks first on the welcome screen too"
    );
    click(vcx, "settings-reset-cancel");
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("settings-reset-dialog").is_none(),
        "Cancel closes the confirmation"
    );
    assert_eq!(
        serde_json::to_value(snapshot(&view, vcx)).unwrap(),
        serde_json::to_value(&before).unwrap(),
        "Cancel changes nothing"
    );

    let saves = crate::app::state::SAVE_COUNT.with(|c| c.get());
    click(vcx, "settings-reset");
    click(vcx, "settings-reset-confirm");
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("settings-reset-dialog").is_none(),
        "Reset closes the confirmation"
    );
    assert!(
        vcx.debug_bounds("settings-page").is_some(),
        "settings stay open after the reset"
    );
    assert!(
        crate::app::state::SAVE_COUNT.with(|c| c.get()) > saves,
        "the reset is saved (a test-only no-op save, AGENTS §2)"
    );

    let mut expected = before.clone();
    expected.reset_ui_preferences();
    assert_eq!(
        serde_json::to_value(snapshot(&view, vcx)).unwrap(),
        serde_json::to_value(&expected).unwrap(),
        "Reset puts the UI preferences back to the defaults and keeps every other value"
    );
    let d = AppConfig::default();
    view.update(vcx, |app, _| {
        assert_eq!(
            (
                app.sidebar_collapsed,
                app.inspector_open,
                app.sidebar_width,
                app.inspector_width,
                app.card_fit_mode,
            ),
            (
                !d.sidebar_visible,
                d.inspector_visible,
                d.sidebar_width,
                d.inspector_width,
                d.card_fit_mode,
            ),
            "the live layout follows the reset"
        );
        assert_eq!(
            app.sort,
            d.restored_sort(),
            "the live sort follows the reset"
        );
        assert!(app.error_message.is_none(), "a good save shows no error");
        assert_eq!(
            app.last_action.as_deref(),
            Some("Settings reset to defaults"),
            "the reset is reported as in the library case"
        );
    });
    let view_menu: Vec<String> = vcx.update(|_, cx| {
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
    });
    assert!(
        view_menu.iter().any(|n| n == "Hide Sidebar"),
        "the View menu follows the reset (sidebar shown): {view_menu:?}"
    );
}
