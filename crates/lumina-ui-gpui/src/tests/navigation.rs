//! Back/forward navigation between grid, viewer, albums and search.

use crate::tests::helpers::*;
use crate::*;

#[core::prelude::v1::test]
fn ui_automation_grid_and_viewer_journey() {
    let (_temp_dir, engine) = seed_library("journey", 8);
    let first_id = engine.get_item_slice(0..1)[0].id.0;
    let card = leak(format!("media-card-{}", first_id));

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    click(vcx, card);
    view.update(vcx, |app, _| {
        assert!(app.selected_item.is_some(), "click must select a card");
        assert_eq!(app.selected_index, Some(0));
    });

    double_click(vcx, card);
    view.update(vcx, |app, _| assert!(app.is_fullscreen_viewer));
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| assert!(!app.is_fullscreen_viewer));

    right_click(vcx, card);
    view.update(vcx, |app, _| assert!(app.context_menu_state.is_some()));
    assert!(vcx.debug_bounds("context-menu").is_some());
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| assert!(app.context_menu_state.is_none()));

    keystrokes(vcx, "end");
    view.update(vcx, |app, _| assert_eq!(app.selected_index, Some(7)));
    keystrokes(vcx, "home");
    view.update(vcx, |app, _| assert_eq!(app.selected_index, Some(0)));
    keystrokes(vcx, "pagedown");
    view.update(vcx, |app, _| assert_eq!(app.selected_index, Some(7)));
}

#[core::prelude::v1::test]
fn ui_automation_viewer_back_button_returns_to_grid() {
    let (_temp_dir, engine) = seed_library("viewer_back", 8);
    let first_id = engine.get_item_slice(0..1)[0].id.0;
    let card = leak(format!("media-card-{}", first_id));

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    double_click(vcx, card);
    view.update(vcx, |app, _| {
        assert!(
            app.is_fullscreen_viewer,
            "double-click must open the viewer"
        );
    });

    let back = vcx
        .debug_bounds("viewer-back")
        .expect("the viewer must paint its Back button");
    let top_bar = vcx
        .debug_bounds("viewer-top-bar")
        .expect("the viewer must paint its top bar");
    assert!(
        back.left() >= top_bar.left() && back.right() <= top_bar.right(),
        "the Back button must live inside the viewer top bar: back={back:?} top_bar={top_bar:?}"
    );
    assert!(
        vcx.debug_bounds("nav-forward").is_none(),
        "the removed Forward control must not be painted"
    );

    click(vcx, "viewer-back");
    view.update(vcx, |app, _| {
        assert!(
            !app.is_fullscreen_viewer,
            "the viewer Back button must return to the grid"
        );
        assert_eq!(
            app.selected_item.as_ref().map(|item| item.id.0),
            Some(first_id),
            "the Back button must keep the grid selection"
        );
    });

    // Escape remains an equivalent keyboard exit.
    double_click(vcx, card);
    view.update(vcx, |app, _| assert!(app.is_fullscreen_viewer));
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| assert!(!app.is_fullscreen_viewer));
}

#[core::prelude::v1::test]
fn ui_automation_navigation_album_back() {
    let (_temp_dir, engine) = seed_library("nav_album", 8);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, cx| {
        app.select_album(
            AlbumSelection::System(lumina_core::SystemAlbum::Favorites),
            cx,
        );
    });
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::System(lumina_core::SystemAlbum::Favorites),
            "the album change must reach the browsing destination"
        );
    });

    keystrokes(vcx, "secondary-[");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::AllLibrary,
            "Back must return to the previous album"
        );
    });

    // Edge: nothing precedes the oldest entry, so Back is a disabled no-op.
    keystrokes(vcx, "secondary-[");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::AllLibrary,
            "Back at the oldest entry must stay put"
        );
    });
}

#[core::prelude::v1::test]
fn ui_automation_navigation_search_back() {
    let (_temp_dir, engine) = seed_library("nav_search", 8);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, cx| app.set_search_query("is:video", cx));
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.search_query, "is:video");
    });

    keystrokes(vcx, "secondary-[");
    view.update(vcx, |app, _| {
        assert!(
            app.search_query.is_empty(),
            "Back must restore the pre-search view"
        );
        assert_eq!(app.selection, AlbumSelection::AllLibrary);
    });
}

#[core::prelude::v1::test]
fn ui_automation_navigation_edge_no_ops() {
    let (_temp_dir, engine) = seed_library("nav_edges", 8);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    render_ui(vcx);
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-[");
    // Cmd+] with nothing ahead is a no-op.
    keystrokes(vcx, "secondary-]");
    keystrokes(vcx, "secondary-[");
    view.update(vcx, |app, _| {
        assert_eq!(app.selection, AlbumSelection::AllLibrary);
        assert!(app.search_query.is_empty());
        assert!(!app.is_fullscreen_viewer);
        assert!(app.selected_item.is_none());
    });

    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::Trash, cx);
    });
    render_ui(vcx);

    keystrokes(vcx, "secondary-[");
    view.update(vcx, |app, _| {
        assert_eq!(app.selection, AlbumSelection::AllLibrary);
    });
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-[");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::AllLibrary,
            "Cmd+[ at the oldest entry must stay put"
        );
    });
    // Cmd+] redoes the Back; at the newest entry it stays put.
    keystrokes(vcx, "secondary-]");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::Trash,
            "Cmd+] must go Forward to the album Back left"
        );
    });
    keystrokes(vcx, "secondary-]");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::Trash,
            "Cmd+] at the newest entry must stay put"
        );
    });
}
