//! Sidebar navigation: the default collections render from one ordered list,
//! each row with its icon.

use lumina_core::{MatchMode, RuleField, RuleOperator, RuleValue, SmartAlbumRule, SystemAlbum};

use crate::i18n::nav_aria_label as aria_label;
use crate::tests::ui_automation::{
    app_with_library, bounds_center, click, leak, render_ui, seed_library,
};
use crate::views::grid::SIDEBAR_COLLAPSED_WIDTH;
use crate::views::sidebar::nav::nav_entries;
use crate::views::sidebar::AlbumSelection;

#[core::prelude::v1::test]
fn every_default_collection_row_has_an_icon() {
    let (dir, engine) = seed_library("nav_icons", 2);
    let album = engine
        .create_smart_album(
            "Best",
            "folder",
            MatchMode::Any,
            &[SmartAlbumRule {
                field: RuleField::Rating,
                operator: RuleOperator::AtLeast,
                value: RuleValue::Integer(4),
            }],
        )
        .unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    // Show the conditional rows too, so every default collection is checked.
    view.update(vcx, |app, cx| {
        app.system_album_counts.insert(SystemAlbum::Downloaded, 1);
        app.system_album_counts.insert(SystemAlbum::Missing, 1);
        cx.notify();
    });
    render_ui(vcx);

    for key in [
        "all",
        "favorites",
        "videos",
        "recently-added",
        "downloaded",
        "screenshots",
        "places",
        "duplicates",
        "missing",
        "recently-deleted",
        "untagged",
    ] {
        assert!(
            vcx.debug_bounds(leak(format!("sidebar-nav-{key}-icon")))
                .is_some(),
            "the {key} row must show its icon"
        );
    }
    assert!(
        vcx.debug_bounds(leak(format!("smart-album-row-{}-icon", album.id)))
            .is_some(),
        "a smart-album row shows the sparkles icon"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn nav_entries_keep_order_and_conditional_rows() {
    let (dir, engine) = seed_library("nav_order", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, _| {
        app.system_album_counts.insert(SystemAlbum::Downloaded, 0);
        app.system_album_counts.insert(SystemAlbum::Missing, 0);
        let keys: Vec<_> = nav_entries(app).iter().map(|e| e.key).collect();
        assert_eq!(
            keys,
            [
                "all",
                "favorites",
                "videos",
                "recently-added",
                "screenshots",
                "keywords",
                "places",
                "duplicates",
                "recently-deleted",
                "untagged",
            ],
            "Downloaded and Missing are hidden at a count of 0"
        );

        app.system_album_counts.insert(SystemAlbum::Downloaded, 2);
        app.system_album_counts.insert(SystemAlbum::Missing, 3);
        let entries = nav_entries(app);
        let keys: Vec<_> = entries.iter().map(|e| e.key).collect();
        assert_eq!(
            keys,
            [
                "all",
                "favorites",
                "videos",
                "recently-added",
                "downloaded",
                "screenshots",
                "keywords",
                "places",
                "duplicates",
                "missing",
                "recently-deleted",
                "untagged",
            ],
            "the S4 table order, with Downloaded and Missing shown above 0"
        );
        let missing = entries.iter().find(|e| e.key == "missing").unwrap();
        assert_eq!(missing.count, 3, "an entry carries its album count");
        assert_eq!(
            missing.selection,
            AlbumSelection::System(SystemAlbum::Missing)
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn nav_rows_keep_their_click_rules() {
    let (dir, engine) = seed_library("nav_clicks", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::System(SystemAlbum::Favorites), cx);
        // After select_album, which clears the year itself.
        app.active_year = Some(2020);
    });
    click(vcx, "sidebar-nav-all");
    view.update(vcx, |app, _| {
        assert_eq!(app.selection, AlbumSelection::AllLibrary);
        assert_eq!(
            app.active_year, None,
            "All Photos & Videos also clears the active year"
        );
    });

    click(vcx, "sidebar-nav-untagged");
    view.update(vcx, |app, _| {
        assert_eq!(app.selection, AlbumSelection::Untagged)
    });
    click(vcx, "sidebar-nav-untagged");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::AllLibrary,
            "clicking Untagged again goes back to the whole library"
        )
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn one_back_after_all_photos_returns_to_the_year_view() {
    let (dir, engine) = seed_library("nav_back", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    // Back keeps only a year the library has, so use one from its timeline.
    let year = view.update(vcx, |app, cx| {
        let year = app
            .timeline_year_buckets
            .first()
            .map(|(year, _)| *year)
            .expect("the seeded library has a timeline year");
        app.select_album(AlbumSelection::System(SystemAlbum::Favorites), cx);
        app.set_active_year(Some(year), cx);
        year
    });
    click(vcx, "sidebar-nav-all");
    view.update(vcx, |app, cx| {
        assert_eq!(app.selection, AlbumSelection::AllLibrary);
        app.navigate_back(cx);
        assert_eq!(
            (app.selection.clone(), app.active_year),
            (AlbumSelection::AllLibrary, Some(year)),
            "clicking All Photos & Videos is one Back step, not two \
             (the year view is the whole library's)"
        );
        app.navigate_back(cx);
        assert_eq!(
            (app.selection.clone(), app.active_year),
            (AlbumSelection::System(SystemAlbum::Favorites), None),
            "the step before the year is the album it was chosen from"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

/// The rail's entries: the Library collections, without Untagged.
const RAIL_KEYS: [&str; 9] = [
    "all",
    "favorites",
    "videos",
    "downloaded",
    "screenshots",
    "places",
    "duplicates",
    "missing",
    "recently-deleted",
];

#[core::prelude::v1::test]
fn collapsed_rail_shows_an_icon_per_default_collection() {
    let (dir, engine) = seed_library("rail_icons", 2);
    let album = engine
        .create_smart_album(
            "Best",
            "folder",
            MatchMode::Any,
            &[SmartAlbumRule {
                field: RuleField::Rating,
                operator: RuleOperator::AtLeast,
                value: RuleValue::Integer(4),
            }],
        )
        .unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    // The minimum window height, with every conditional entry shown.
    vcx.simulate_resize(gpui::size(gpui::px(960.0), gpui::px(600.0)));
    view.update(vcx, |app, cx| {
        app.system_album_counts.insert(SystemAlbum::Downloaded, 1);
        app.system_album_counts.insert(SystemAlbum::Missing, 1);
        cx.notify();
    });
    click(vcx, "sidebar-toggle");
    view.update(vcx, |app, _| assert!(app.sidebar_collapsed));
    render_ui(vcx);

    let rail = vcx.debug_bounds("sidebar-rail").expect("collapsed rail");
    assert_eq!(
        f32::from(rail.size.width),
        SIDEBAR_COLLAPSED_WIDTH,
        "the rail keeps the collapsed column width"
    );
    for key in RAIL_KEYS {
        let button = vcx
            .debug_bounds(leak(format!("sidebar-rail-{key}")))
            .unwrap_or_else(|| panic!("the rail must show the {key} collection"));
        assert_eq!(f32::from(button.size.width), 32.0, "{key}: 32 px button");
        assert_eq!(f32::from(button.size.height), 32.0, "{key}: 32 px button");
        assert!(
            button.left() >= rail.left() && button.right() <= rail.right(),
            "{key}: the button fits inside the 48 px column"
        );
        assert!(
            f32::from(button.bottom()) <= 600.0,
            "{key}: every button is visible at the 600 px minimum height"
        );
    }
    for absent in [
        "sidebar-rail-untagged",
        "expand-sidebar",
        "sidebar-tags-header",
    ] {
        assert!(
            vcx.debug_bounds(absent).is_none(),
            "{absent} must not be in the collapsed rail"
        );
    }
    assert!(
        vcx.debug_bounds(leak(format!("smart-album-row-{}", album.id)))
            .is_none(),
        "smart albums are not in the rail"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn rail_icon_selects_collection_and_marks_it_active() {
    let (dir, engine) = seed_library("rail_select", 4);
    let first = engine.get_item_slice(0..1)[0].id;
    engine.toggle_favorite(first, true).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, _| app.refresh_album_counts());
    click(vcx, "sidebar-toggle");
    render_ui(vcx);
    assert!(vcx.debug_bounds("sidebar-rail-active").is_some());
    assert_eq!(
        vcx.debug_bounds("sidebar-rail-active"),
        vcx.debug_bounds("sidebar-rail-all"),
        "the pill starts on All Photos & Videos"
    );

    click(vcx, "sidebar-rail-favorites");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::System(SystemAlbum::Favorites)
        );
        assert_eq!(
            app.displayed_count(),
            app.system_album_counts[&SystemAlbum::Favorites],
            "the grid shows the Favorites album"
        );
        assert_eq!(app.displayed_count(), 1);
    });
    let pill = vcx.debug_bounds("sidebar-rail-active");
    assert!(pill.is_some(), "the active entry carries the pill");
    assert_eq!(
        pill,
        vcx.debug_bounds("sidebar-rail-favorites"),
        "the pill moves to the clicked entry"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn rail_tooltip_and_label_carry_the_count() {
    let (dir, engine) = seed_library("rail_tooltip", 3);
    let first = engine.get_item_slice(0..1)[0].id;
    engine.toggle_favorite(first, true).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, _| app.refresh_album_counts());
    click(vcx, "sidebar-toggle");
    render_ui(vcx);

    let center = bounds_center(vcx, "sidebar-rail-favorites");
    vcx.simulate_mouse_move(center, None, gpui::Modifiers::default());
    // GPUI waits 500ms before building a tooltip.
    vcx.background_executor
        .advance_clock(std::time::Duration::from_millis(600));
    vcx.run_until_parked();
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("tooltip-Favorites · 1").is_some(),
        "the rail tooltip names the collection and its count"
    );

    // The rail and the rows share one label format.
    assert_eq!(aria_label("Favorites", 1), "Favorites, 1 item");
    assert_eq!(aria_label("Favorites", 22), "Favorites, 22 items");
    let _ = std::fs::remove_dir_all(&dir);
}

fn press_ctrl_cmd_s(vcx: &mut gpui::VisualTestContext, view: &gpui::Entity<crate::LuminaApp>) {
    crate::tests::ui_automation::focus_app(vcx, view);
    crate::tests::ui_automation::keystrokes(vcx, "ctrl-cmd-s");
}

#[core::prelude::v1::test]
fn ctrl_cmd_s_toggles_the_sidebar_and_persists() {
    let (dir, engine) = seed_library("sidebar_shortcut", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    press_ctrl_cmd_s(vcx, &view);
    view.update(vcx, |app, _| assert!(app.sidebar_collapsed, "⌃⌘S hides"));
    press_ctrl_cmd_s(vcx, &view);
    view.update(vcx, |app, _| assert!(!app.sidebar_collapsed, "⌃⌘S shows"));

    // ⌘S and ⌃S alone are not the sidebar shortcut.
    for key in ["cmd-s", "ctrl-s"] {
        crate::tests::ui_automation::keystrokes(vcx, key);
        view.update(vcx, |app, _| {
            assert!(!app.sidebar_collapsed, "{key} must not toggle the sidebar")
        });
    }

    // The shortcut saves the setting. A failed save says so and puts the
    // sidebar back, so what you see matches what is saved. Under test the
    // save writes nothing (AGENTS §2).
    crate::app::state::SAVE_FAILS.with(|f| f.set(true));
    press_ctrl_cmd_s(vcx, &view);
    crate::app::state::SAVE_FAILS.with(|f| f.set(false));
    view.update(vcx, |app, _| {
        assert!(!app.sidebar_collapsed, "a failed save reverts the toggle");
        assert!(
            app.error_message
                .as_deref()
                .is_some_and(|m| m.starts_with("Could not save settings")),
            "⌃⌘S writes sidebar_visible through save_config"
        );
    });
    let view_items = vcx.update(|_, cx| {
        cx.get_menus()
            .unwrap_or_default()
            .into_iter()
            .filter(|m| m.name.as_ref() == "View")
            .flat_map(|m| m.items)
            .filter_map(|item| match item {
                gpui::OwnedMenuItem::Action { name, .. } => Some(name),
                _ => None,
            })
            .collect::<Vec<_>>()
    });
    assert!(
        view_items.iter().any(|n| n == "Hide Sidebar"),
        "the View menu matches the reverted state: {view_items:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn sidebar_shortcut_is_absorbed_by_modals() {
    let (dir, engine) = seed_library("sidebar_shortcut_modal", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    for (open, what, close) in [
        ("secondary-k", "the command palette", "escape"),
        ("secondary-/", "the shortcut sheet", "escape"),
        // The first Esc hides the search suggestions, the second blurs.
        ("secondary-f", "the search field", "escape escape"),
    ] {
        crate::tests::ui_automation::focus_app(vcx, &view);
        crate::tests::ui_automation::keystrokes(vcx, open);
        crate::tests::ui_automation::keystrokes(vcx, "ctrl-cmd-s");
        view.update(vcx, |app, _| {
            assert!(
                !app.sidebar_collapsed,
                "⌃⌘S must not reach the window while {what} has the keys"
            )
        });
        crate::tests::ui_automation::keystrokes(vcx, close);
    }
    view.update(vcx, |app, _| {
        assert!(!app.command_palette_open && !app.shortcuts_overlay_open && !app.search_focused)
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn toggle_label_follows_state() {
    use crate::app::command_palette::{CommandId, PaletteTarget};
    use crate::icons::Icon;

    let (dir, engine) = seed_library("sidebar_toggle_label", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    for (collapsed, title, icon) in [
        (false, "Hide Sidebar", Icon::PanelLeftClose),
        (true, "Show Sidebar", Icon::PanelLeftOpen),
        (false, "Hide Sidebar", Icon::PanelLeftClose),
    ] {
        if view.update(vcx, |app, _| app.sidebar_collapsed) != collapsed {
            click(vcx, "sidebar-toggle");
            render_ui(vcx);
        }

        // Toolbar tooltip (the aria label uses the same text).
        let center = bounds_center(vcx, "sidebar-toggle");
        vcx.simulate_mouse_move(center, None, gpui::Modifiers::default());
        vcx.background_executor
            .advance_clock(std::time::Duration::from_millis(600));
        vcx.run_until_parked();
        render_ui(vcx);
        let tooltip = format!("tooltip-{title} (⌃⌘S)");
        assert!(
            vcx.debug_bounds(leak(tooltip.clone())).is_some(),
            "the toolbar toggle says {tooltip}"
        );
        vcx.simulate_mouse_move(
            gpui::point(gpui::px(600.0), gpui::px(400.0)),
            None,
            gpui::Modifiers::default(),
        );

        // Palette entry: title, icon and shortcut.
        view.update(vcx, |app, _| {
            // The palette lists eight commands at most; "sidebar" finds it.
            app.command_query = "sidebar".into();
            let rows = app.palette_rows();
            app.command_query.clear();
            let row = rows
                .iter()
                .find(|r| matches!(r.target, PaletteTarget::Command(CommandId::ToggleSidebar)))
                .expect("the palette lists the sidebar toggle");
            assert_eq!(row.title, title, "the palette entry follows the state");
            assert_eq!(row.icon, icon, "the palette icon follows the state");
            assert_eq!(row.shortcut, "⌃⌘S");
        });
    }

    // View menu: rebuilt on every toggle.
    let view_menu_names = |vcx: &mut gpui::VisualTestContext| {
        vcx.update(|_, cx| {
            cx.get_menus()
                .unwrap_or_default()
                .into_iter()
                .filter(|m| m.name.as_ref() == "View")
                .flat_map(|m| m.items)
                .filter_map(|item| match item {
                    gpui::OwnedMenuItem::Action { name, .. } => Some(name),
                    _ => None,
                })
                .collect::<Vec<_>>()
        })
    };
    click(vcx, "sidebar-toggle");
    assert!(
        view_menu_names(vcx).iter().any(|n| n == "Show Sidebar"),
        "View offers Show Sidebar while the sidebar is hidden"
    );
    click(vcx, "sidebar-toggle");
    assert!(
        view_menu_names(vcx).iter().any(|n| n == "Hide Sidebar"),
        "View offers Hide Sidebar while the sidebar is shown"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn new_album_is_last_album_row() {
    let (dir, engine) = seed_library("sb_new_album", 2);
    let album = engine.create_album_with("Trip", &[]).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    render_ui(vcx);
    let row = vcx
        .debug_bounds(leak(format!("album-row-{}", album.id)))
        .expect("album row");
    let new_album = vcx.debug_bounds("new-album").expect("New Album row");
    assert!(
        new_album.top() >= row.bottom(),
        "New Album is the last row of the Albums section, not a header button"
    );
    assert_eq!(
        f32::from(new_album.size.height),
        32.0,
        "New Album is a NavItem row"
    );
    let before = view.update(vcx, |app, _| app.albums.list.len());
    click(vcx, "new-album");
    assert_eq!(
        view.update(vcx, |app, _| app.albums.list.len()),
        before + 1,
        "clicking New Album still creates an album"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn places_row_is_under_tags() {
    let (dir, engine) = seed_library("sb_places", 2);
    let mut cx = crate::tests::TestApp::single();
    let (_view, vcx) = app_with_library(&mut cx, engine);
    render_ui(vcx);
    let places = vcx.debug_bounds("sidebar-nav-places").expect("Places row");
    let tags = vcx
        .debug_bounds("sidebar-tags-header")
        .expect("TAGS header");
    assert!(
        places.top() > tags.bottom(),
        "Places lists under TAGS, as in the design"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn recently_added_row_opens_its_album() {
    let (dir, engine) = seed_library("sb_recent", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    render_ui(vcx);
    let videos = vcx.debug_bounds("sidebar-nav-videos").expect("Videos row");
    let recent = vcx
        .debug_bounds("sidebar-nav-recently-added")
        .expect("Recently Added row");
    assert!(
        recent.top() >= videos.bottom(),
        "Recently Added follows Videos"
    );
    click(vcx, "sidebar-nav-recently-added");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::System(SystemAlbum::RecentlyAdded),
            "the row opens the Recently Added album"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn sidebar_is_full_height_252() {
    let (dir, engine) = seed_library("sb_full_height", 2);
    let mut cx = crate::tests::TestApp::single();
    let (_view, vcx) = app_with_library(&mut cx, engine);
    render_ui(vcx);
    let window = vcx.update(|window, _| window.viewport_size());
    let sidebar = vcx.debug_bounds("sidebar").expect("sidebar");
    assert_eq!(
        f32::from(sidebar.top()),
        0.0,
        "the sidebar starts at the window's top edge, not under a toolbar"
    );
    assert_eq!(
        sidebar.size.height, window.height,
        "the sidebar runs the full window height"
    );
    assert_eq!(
        f32::from(sidebar.size.width),
        252.0,
        "the design's sidebar width on a fresh config"
    );
    let area = vcx
        .debug_bounds("toolbar-drag-area")
        .expect("toolbar-drag-area");
    assert!(
        area.left() >= sidebar.right(),
        "the header sits in the main column, beside the sidebar"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn sidebar_brand_row_present() {
    let (dir, engine) = seed_library("sb_brand", 2);
    let mut cx = crate::tests::TestApp::single();
    let (_view, vcx) = app_with_library(&mut cx, engine);
    render_ui(vcx);
    let brand = vcx.debug_bounds("sidebar-brand").expect("brand row");
    let slot = vcx
        .debug_bounds("sidebar-search-slot")
        .expect("search slot");
    let all = vcx.debug_bounds("sidebar-nav-all").expect("All Media row");
    assert!(
        brand.bottom() <= slot.top() && slot.bottom() <= all.top(),
        "brand row, then the search slot, then the Library rows"
    );
    assert_eq!(
        f32::from(slot.size.height),
        34.0,
        "the search slot keeps the field's height"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The fullscreen viewer covers the sidebar, so the sidebar leaves the tree
/// (VoiceOver listed its buttons under the viewer). Its width stays, so the
/// grid under the viewer keeps its layout.
#[core::prelude::v1::test]
fn fullscreen_viewer_takes_the_sidebar_out_of_the_tree() {
    let (dir, engine) = seed_library("nav_viewer_hides", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    render_ui(vcx);
    let sidebar = vcx.debug_bounds("sidebar").expect("the sidebar shows");
    let status = vcx
        .debug_bounds("status-bar")
        .expect("the status bar shows");
    assert!(vcx.debug_bounds("switch-library").is_some());

    view.update(vcx, |app, cx| {
        let first = app.get_active_slice(0..1)[0].clone();
        app.open_viewer(first, cx);
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("sidebar").is_none() && vcx.debug_bounds("switch-library").is_none(),
        "the viewer covers the sidebar, so its controls must not be in the tree"
    );
    assert_eq!(
        vcx.debug_bounds("status-bar").map(|b| b.left()),
        Some(status.left()),
        "the column under the viewer keeps its place"
    );

    view.update(vcx, |app, cx| app.close_viewer(cx));
    render_ui(vcx);
    assert_eq!(
        vcx.debug_bounds("sidebar"),
        Some(sidebar),
        "closing the viewer brings the sidebar back where it was"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Dropping the viewed item's selection (an untag in a Tag view, a merge
/// that trashes it) leaves the viewer flag set but draws no viewer: the
/// sidebar must come back, not stay blank.
#[core::prelude::v1::test]
fn sidebar_returns_when_the_viewed_item_is_dropped() {
    let (dir, engine) = seed_library("nav_viewer_dropped", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        let first = app.get_active_slice(0..1)[0].clone();
        app.open_viewer(first, cx);
    });
    render_ui(vcx);
    assert!(vcx.debug_bounds("sidebar").is_none(), "precondition");

    view.update(vcx, |app, cx| {
        app.clear_selection_fields();
        cx.notify();
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("sidebar").is_some(),
        "with no viewer drawn the sidebar must show"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
