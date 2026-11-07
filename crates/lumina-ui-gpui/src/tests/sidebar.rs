//! Sidebar navigation: the default collections render from one ordered list,
//! each row with its icon (sidebar revamp, Task 2).

use lumina_core::{MatchMode, RuleField, RuleOperator, RuleValue, SmartAlbumRule, SystemAlbum};

use crate::tests::ui_automation::{app_with_library, click, leak, render_ui, seed_library};
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
