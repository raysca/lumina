//! The Albums overview page in the main column, reached from the
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
    let album = engine.create_album_with("Lisbon", &[]).unwrap();
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

pub(super) fn height(vcx: &mut gpui::VisualTestContext, selector: &'static str) -> f32 {
    let bounds = vcx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} is painted"));
    f32::from(bounds.size.height)
}

#[core::prelude::v1::test]
fn albums_overview_counts_albums_and_its_tabs_take_clicks_keys_and_palette() {
    let (dir, engine) = seed_library("albums_overview_header", 2);
    engine.create_album_with("Lisbon", &[]).unwrap();
    engine.create_album_with("Family", &[]).unwrap();
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

    assert!(
        vcx.debug_bounds("page-title-text:Albums").is_some(),
        "the header paints the title Albums"
    );
    assert!(
        vcx.debug_bounds("page-meta-text:2 albums · 1 smart album")
            .is_some(),
        "the header paints the catalog counts"
    );
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

/// The page shows no media: keys that act on a selection must find none,
/// even when an item was selected before the page opened.
#[core::prelude::v1::test]
fn albums_overview_absorbs_media_keys() {
    let (dir, engine) = seed_library("albums_overview_keys", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    keystrokes(vcx, "right");
    assert!(view.update(vcx, |app, _| app.selected_item.is_some()));
    let before = engine.get_item_slice(0..3);

    click(vcx, "sidebar-albums-header");
    focus_app(vcx, &view);
    for key in [
        "cmd-backspace",
        "backspace",
        "p",
        "x",
        "u",
        "0",
        "1",
        "2",
        "3",
        "4",
        "5",
        "left",
        "right",
        "up",
        "down",
        "cmd-a",
        "enter",
        "space",
    ] {
        keystrokes(vcx, key);
        view.update(vcx, |app, _| {
            assert!(app.albums_overview_shown(), "{key} keeps the page");
            assert!(
                app.selected_item.is_none() && app.selected_indices.is_empty(),
                "{key}: the page holds no selection"
            );
            assert!(
                app.delete_confirm_state.is_none() && !app.is_fullscreen_viewer,
                "{key} opens no delete dialog or viewer"
            );
        });
        assert_eq!(
            engine.get_item_slice(0..3),
            before,
            "{key} changes no catalog row on the overview"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn albums_overview_new_album_from_the_keyboard() {
    let (dir, engine) = seed_library("albums_overview_kbd_new", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    click(vcx, "sidebar-albums-header");
    focus_app(vcx, &view);

    keystrokes(vcx, "tab");
    keystrokes(vcx, "tab");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.albums.overview.focus,
            Some(OverviewFocus::NewAlbum),
            "the second Tab reaches New Album"
        );
    });
    keystrokes(vcx, "enter");
    assert_eq!(
        engine.list_albums_with_counts().len(),
        1,
        "↵ on New Album adds a row"
    );
    // ↵ saves the name the album started with.
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| assert_eq!(app.albums.renaming, None));

    focus_app(vcx, &view);
    keystrokes(vcx, "escape");
    keystrokes(vcx, "shift-tab");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.albums.overview.focus,
            Some(OverviewFocus::NewAlbum),
            "⇧Tab from no focus reaches New Album"
        );
    });
    keystrokes(vcx, "enter");
    assert_eq!(
        engine.list_albums_with_counts().len(),
        2,
        "⇧Tab then ↵ adds another"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn forward_returns_to_the_overview_without_its_old_focus() {
    let (dir, engine) = seed_library("albums_overview_forward", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    click(vcx, "sidebar-albums-header");
    focus_app(vcx, &view);
    keystrokes(vcx, "shift-tab");
    keystrokes(vcx, "cmd-[");
    assert!(view.update(vcx, |app, _| !app.albums_overview_shown()));

    keystrokes(vcx, "cmd-]");
    assert_on_overview(vcx, "⌘] Forward");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.albums.overview.focus, None,
            "the focus does not survive the visit"
        );
    });
    keystrokes(vcx, "enter");
    assert!(
        engine.list_albums_with_counts().is_empty(),
        "↵ after Forward runs no stale New Album"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn overview_hides_the_inspector_and_keeps_downloads() {
    use super::downloads::{install, queue_app, settle, submit};
    let (_dir, engine, fake, mut cx) = queue_app("albums_overview_header_actions");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    submit(&view, vcx, "https://example.com/a.mp4");
    settle(vcx, &view, "the row is stored", |app| {
        app.downloads.entries.len() == 1 && app.downloads.entries[0].row.id > 0
    });
    view.update(vcx, |app, cx| {
        app.inspector_open = true;
        app.close_network_import_dialog(cx);
    });
    render_ui(vcx);
    assert!(vcx.debug_bounds("inspector-panel").is_some());

    click(vcx, "sidebar-albums-header");
    assert!(
        vcx.debug_bounds("inspector-panel").is_none(),
        "the overview has nothing for the inspector to show"
    );
    assert!(
        vcx.debug_bounds("downloads-toolbar-btn").is_some(),
        "running downloads keep their header button on the overview"
    );
}
