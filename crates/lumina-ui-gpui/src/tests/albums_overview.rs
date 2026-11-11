//! Design 05: the Albums overview page in the main column, reached from the
//! sidebar ALBUMS label, the palette and the Go menu.

use super::duplicates::run_palette_command;
use super::ui_automation::{
    app_with_library, click, focus_app, keystrokes, render_ui, seed_library,
};
use crate::app::albums_overview::{AlbumsTab, OverviewFocus};
use crate::views::sidebar::AlbumSelection;
use lumina_core::{MatchMode, RuleField, RuleOperator, RuleValue, SmartAlbumRule};

fn assert_on_overview(vcx: &mut gpui::VisualTestContext, how: &str) {
    assert!(
        vcx.debug_bounds("albums-overview").is_some(),
        "{how} shows the albums overview"
    );
    for gone in [
        "media-grid-surface",
        "status-bar",
        "multi-select-action-dock",
    ] {
        assert!(
            vcx.debug_bounds(gone).is_none(),
            "{how}: the overview hides {gone}"
        );
    }
}

#[core::prelude::v1::test]
fn albums_overview_opens_from_sidebar_and_palette() {
    let (dir, engine) = seed_library("albums_overview_nav", 2);
    let album = engine.create_album("Lisbon").unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    assert!(vcx.debug_bounds("media-grid-surface").is_some());

    click(vcx, "sidebar-albums-header");
    assert_on_overview(vcx, "clicking the ALBUMS label");
    view.update(vcx, |app, _| {
        assert_eq!(app.selection, AlbumSelection::AlbumsOverview);
        for row in [
            AlbumSelection::AllLibrary,
            AlbumSelection::Album(album.id),
            AlbumSelection::Trash,
        ] {
            assert!(
                !app.sidebar_highlights(&row),
                "no sidebar row is active on the overview: {row:?}"
            );
        }
    });

    focus_app(vcx, &view);
    keystrokes(vcx, "cmd-[");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::AllLibrary,
            "Back returns to the view before the overview"
        );
    });
    render_ui(vcx);
    assert!(vcx.debug_bounds("media-grid-surface").is_some());
    assert!(vcx.debug_bounds("albums-overview").is_none());

    run_palette_command(vcx, &view, "Show Albums");
    render_ui(vcx);
    assert_on_overview(vcx, "the palette's Show Albums");

    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::AllLibrary, cx)
    });
    vcx.dispatch_action(crate::actions::ShowAlbums);
    render_ui(vcx);
    assert_on_overview(vcx, "Go > Albums");
    let _ = std::fs::remove_dir_all(&dir);
}

fn height(vcx: &mut gpui::VisualTestContext, selector: &'static str) -> f32 {
    let bounds = vcx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} is painted"));
    f32::from(bounds.size.height)
}

#[core::prelude::v1::test]
fn albums_header_and_tabs_match_design() {
    let (dir, engine) = seed_library("albums_overview_header", 2);
    engine.create_album("Lisbon").unwrap();
    engine.create_album("Family").unwrap();
    let rule = SmartAlbumRule {
        field: RuleField::IsFavorite,
        operator: RuleOperator::Equals,
        value: RuleValue::Bool(true),
    };
    engine
        .create_smart_album("Loved", "folder", MatchMode::All, &[rule])
        .unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    run_palette_command(vcx, &view, "Show Albums");
    render_ui(vcx);

    for (selector, want) in [
        ("page-header", 67.5),
        ("page-tabs", 51.5),
        ("page-tab-albums", 34.0),
        ("page-tab-smart-albums", 34.0),
    ] {
        let got = height(vcx, selector);
        assert!(
            (got - want).abs() <= 0.5,
            "{selector} is {want} high: {got}"
        );
    }
    assert!(vcx.debug_bounds("page-title").is_some());
    assert!(vcx.debug_bounds("new-album-page-btn").is_some());
    view.update(vcx, |app, _| {
        assert_eq!(app.view_label(), "Albums", "the page title is Albums");
        assert_eq!(
            app.page_meta().as_deref(),
            Some("2 albums · 1 smart album"),
            "the meta line counts albums and smart albums from the catalog"
        );
        assert_eq!(
            app.albums.overview.tab,
            AlbumsTab::Albums,
            "Albums is the first tab"
        );
    });

    click(vcx, "page-tab-smart-albums");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.albums.overview.tab,
            AlbumsTab::SmartAlbums,
            "a click picks a tab"
        );
    });

    // Keyboard: Tab focuses the tab bar, ←/→ move between tabs.
    focus_app(vcx, &view);
    keystrokes(vcx, "tab");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.albums.overview.focus,
            Some(OverviewFocus::Tabs),
            "Tab focuses the tabs"
        );
    });
    keystrokes(vcx, "left");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.albums.overview.tab,
            AlbumsTab::Albums,
            "← moves to the previous tab"
        );
    });
    keystrokes(vcx, "right");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.albums.overview.tab,
            AlbumsTab::SmartAlbums,
            "→ moves to the next tab"
        );
    });
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert_eq!(app.albums.overview.focus, None, "Esc clears the focus");
    });

    run_palette_command(vcx, &view, "Show Albums Tab");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.albums.overview.tab,
            AlbumsTab::Albums,
            "⌘K Show Albums Tab"
        );
    });
    run_palette_command(vcx, &view, "Show Smart Albums Tab");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.albums.overview.tab,
            AlbumsTab::SmartAlbums,
            "⌘K Show Smart Albums Tab"
        );
    });

    click(vcx, "new-album-page-btn");
    let albums = engine.list_albums_with_counts();
    assert_eq!(albums.len(), 3, "New Album creates an album in the catalog");
    let newest = albums.iter().map(|(album, _)| album.id).max();
    view.update(vcx, |app, _| {
        assert_eq!(
            app.albums.renaming, newest,
            "the new album starts in rename mode"
        );
        assert_eq!(
            app.page_meta().as_deref(),
            Some("3 albums · 1 smart album"),
            "the meta line follows the catalog"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}
