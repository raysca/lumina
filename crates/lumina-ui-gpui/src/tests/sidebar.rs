//! Sidebar navigation: the default collections render from one ordered list,
//! each row with its icon (sidebar revamp, Task 2).

use lumina_core::{MatchMode, RuleField, RuleOperator, RuleValue, SmartAlbumRule, SystemAlbum};

use crate::tests::ui_automation::{
    app_with_library, bounds_center, click, leak, render_ui, seed_library,
};
use crate::views::grid::SIDEBAR_COLLAPSED_WIDTH;
use crate::views::sidebar::nav::{aria_label, nav_entries};
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

    for label in [
        "All Photos & Videos",
        "Favorites",
        "Videos",
        "Downloaded",
        "Screenshots",
        "Places",
        "Duplicates",
        "Missing",
        "Recently Deleted",
        "Untagged",
    ] {
        assert!(
            vcx.debug_bounds(leak(format!("sidebar-nav-{label}-icon")))
                .is_some(),
            "the {label} row must show its icon"
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
                "screenshots",
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
                "downloaded",
                "screenshots",
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
    click(vcx, "sidebar-nav-All Photos & Videos");
    view.update(vcx, |app, _| {
        assert_eq!(app.selection, AlbumSelection::AllLibrary);
        assert_eq!(
            app.active_year, None,
            "All Photos & Videos also clears the active year"
        );
    });

    click(vcx, "sidebar-nav-Untagged");
    view.update(vcx, |app, _| {
        assert_eq!(app.selection, AlbumSelection::Untagged)
    });
    click(vcx, "sidebar-nav-Untagged");
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
    click(vcx, "sidebar-nav-All Photos & Videos");
    view.update(vcx, |app, cx| {
        assert_eq!(app.selection, AlbumSelection::AllLibrary);
        app.navigate_back(cx);
        assert_eq!(
            (app.selection.clone(), app.active_year),
            (AlbumSelection::System(SystemAlbum::Favorites), Some(year)),
            "clicking All Photos & Videos is one Back step, not two"
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
