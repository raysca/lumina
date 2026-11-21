//! Startup session restore: the late restore yields to the user, the grid
//! comes back at the saved scroll anchor, and Quit saves first.

use super::helpers::{
    app_with_library, click, double_click, focus_app, keystrokes, leak, render_ui, seed_library,
    type_text,
};
use crate::app::types::UiMessage;
use crate::config::{LastView, RestoredSession};
use crate::views::sidebar::AlbumSelection;
use crate::LuminaApp;
use lumina_core::{EngineEvent, MediaId, SystemAlbum};

const FAVORITES: AlbumSelection = AlbumSelection::System(SystemAlbum::Favorites);

fn session(view: LastView, selected: Option<u64>, anchor: Option<u64>) -> RestoredSession {
    RestoredSession {
        view,
        search_query: String::new(),
        active_year: None,
        selected_media_id: selected,
        scroll_anchor_id: anchor,
    }
}

/// Delivers the startup scan's `ScanComplete`, which runs the restore.
fn finish_scan(view: &gpui::Entity<crate::LuminaApp>, vcx: &mut gpui::VisualTestContext) {
    view.update(vcx, |app, cx| {
        let total = app.grid_state.as_ref().map_or(0, |s| s.total_count);
        app.handle_ui_message(
            UiMessage::Engine(EngineEvent::ScanComplete { total_items: total }),
            cx,
        );
    });
    render_ui(vcx);
}

fn start_with_pending(
    view: &gpui::Entity<crate::LuminaApp>,
    vcx: &mut gpui::VisualTestContext,
    restore: RestoredSession,
) {
    view.update(vcx, |app, _| {
        assert!(
            app.grid_state.as_ref().is_some_and(|s| s.is_scanning),
            "the startup scan is still running"
        );
        app.pending_restore = Some(restore);
    });
}

#[core::prelude::v1::test]
fn user_navigation_before_scan_complete_cancels_restore() {
    let (_dir, engine) = seed_library("restore_nav", 4);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    start_with_pending(&view, vcx, session(LastView::Favorites, None, None));

    let tag = AlbumSelection::Tag("travel".into());
    view.update(vcx, |app, cx| app.select_album(tag.clone(), cx));
    finish_scan(&view, vcx);
    view.update(vcx, |app, _| {
        assert!(app.pending_restore.is_none(), "the restore is dropped");
        assert_eq!(
            app.selection, tag,
            "a late restore must not move a user who already navigated"
        );
    });
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-[");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::AllLibrary,
            "the user's Back trail must survive the scan"
        );
    });
}

#[core::prelude::v1::test]
fn typing_a_search_before_scan_complete_cancels_restore() {
    let (_dir, engine) = seed_library("restore_type", 4);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    start_with_pending(&view, vcx, session(LastView::Favorites, None, None));

    click(vcx, "search-input");
    type_text(vcx, "cat");
    finish_scan(&view, vcx);
    view.update(vcx, |app, _| {
        assert!(app.pending_restore.is_none());
        assert_eq!(
            app.search_query, "cat",
            "a typed search must survive the late restore"
        );
        assert_eq!(app.selection, AlbumSelection::AllLibrary);
    });
}

#[core::prelude::v1::test]
fn selecting_a_card_before_scan_complete_cancels_restore() {
    let (_dir, engine) = seed_library("restore_card", 4);
    let items = engine.get_item_slice(0..2);
    let card = leak(format!("media-card-{}", items[1].id.0));
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    start_with_pending(
        &view,
        vcx,
        session(LastView::Favorites, Some(items[0].id.0), None),
    );

    click(vcx, card);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_item.as_ref().map(|item| item.id),
            Some(items[1].id),
            "the click selects the card"
        )
    });
    finish_scan(&view, vcx);
    view.update(vcx, |app, _| {
        assert!(app.pending_restore.is_none());
        assert_eq!(
            app.selection,
            AlbumSelection::AllLibrary,
            "a card picked during the scan cancels the restore"
        );
        // ScanComplete still clears a selection made while indices could
        // shift (AGENTS 5.3); the restored item must not replace it.
        assert_ne!(
            app.selected_item.as_ref().map(|item| item.id),
            Some(items[0].id),
            "the saved item must not be selected over the user's pick"
        );
    });
}

#[core::prelude::v1::test]
fn untouched_startup_still_restores() {
    let (_dir, engine) = seed_library("restore_idle", 4);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    start_with_pending(&view, vcx, session(LastView::Favorites, None, None));

    finish_scan(&view, vcx);
    view.update(vcx, |app, _| {
        assert!(app.pending_restore.is_none());
        assert_eq!(app.selection, FAVORITES, "no input: the session restores");
    });
}

/// Rows of the grid viewport: (top row, fully visible rows).
fn visible_rows(
    view: &gpui::Entity<crate::LuminaApp>,
    vcx: &mut gpui::VisualTestContext,
) -> (usize, usize) {
    view.update(vcx, |app, _| {
        let row_height = app.row_height();
        let handle = app.grid_state.as_ref().unwrap().scroll_handle.clone();
        let state = handle.0.borrow();
        let top = (-f32::from(state.base_handle.offset().y) / row_height).round() as usize;
        let rows = (f32::from(state.base_handle.bounds().size.height) / row_height) as usize;
        (top, rows)
    })
}

/// A 200-item library (so the grid scrolls) at 4 columns.
fn big_library<'a>(
    prefix: &str,
    cx: &'a mut gpui::TestAppContext,
) -> (
    crate::tests::helpers::TempLibrary,
    gpui::Entity<crate::LuminaApp>,
    &'a mut gpui::VisualTestContext,
    Vec<MediaId>,
) {
    let (dir, engine) = seed_library(prefix, 200);
    let (view, vcx) = app_with_library(cx, engine);
    view.update(vcx, |app, cx| {
        app.grid_columns = 4;
        cx.notify();
    });
    render_ui(vcx);
    let ids = view.update(vcx, |app, _| {
        app.get_active_slice(0..200)
            .into_iter()
            .map(|item| item.id)
            .collect::<Vec<_>>()
    });
    assert_eq!(ids.len(), 200, "the scan must produce 200 items");
    (dir, view, vcx, ids)
}

#[core::prelude::v1::test]
fn restore_scrolls_to_saved_anchor_item() {
    let mut cx = crate::tests::TestApp::single();
    let (_dir, view, vcx, ids) = big_library("restore_anchor", &mut cx);
    start_with_pending(
        &view,
        vcx,
        session(LastView::All, Some(ids[28].0), Some(ids[24].0)),
    );
    finish_scan(&view, vcx);
    view.update(vcx, |app, _| assert_eq!(app.selected_index, Some(28)));
    // Read from the scroll offset: `logical_scroll_top_index` reports the
    // base handle's tracked children, which a uniform list does not have.
    assert_eq!(
        visible_rows(&view, vcx).0,
        6,
        "the anchor row must be at the top; the reveal must not overwrite it"
    );
}

#[core::prelude::v1::test]
fn restore_reveals_selection_outside_the_anchored_view_or_without_an_anchor() {
    for (case, anchor) in [("restore_far", None), ("restore_noanchor", Some(u64::MAX))] {
        let mut cx = crate::tests::TestApp::single();
        let (_dir, view, vcx, ids) = big_library(case, &mut cx);
        let anchor = anchor.unwrap_or(ids[24].0);
        start_with_pending(
            &view,
            vcx,
            session(LastView::All, Some(ids[180].0), Some(anchor)),
        );
        finish_scan(&view, vcx);
        let (top, rows) = visible_rows(&view, vcx);
        assert!(
            (top..top + rows.max(1)).contains(&45),
            "{case}: the selected row (45) must be visible, viewport rows {top}..{}",
            top + rows
        );
    }
}

#[core::prelude::v1::test]
fn grid_top_item_id_reads_the_first_visible_row() {
    let mut cx = crate::tests::TestApp::single();
    let (_dir, view, vcx, ids) = big_library("restore_top", &mut cx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.grid_top_item_id(),
            Some(ids[0]),
            "unscrolled: first item"
        );
        app.grid_state
            .as_ref()
            .unwrap()
            .scroll_handle
            .scroll_to_item_strict(5, gpui::ScrollStrategy::Top);
    });
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.grid_top_item_id(),
            Some(ids[5 * 4]),
            "the anchor is the first item of the top visible row"
        );
    });
}

#[core::prelude::v1::test]
fn viewer_escape_keeps_grid_scroll() {
    let mut cx = crate::tests::TestApp::single();
    let (_dir, view, vcx, ids) = big_library("restore_viewer", &mut cx);
    view.update(vcx, |app, _| {
        app.grid_state
            .as_ref()
            .unwrap()
            .scroll_handle
            .scroll_to_item_strict(6, gpui::ScrollStrategy::Top);
    });
    render_ui(vcx);
    assert_eq!(visible_rows(&view, vcx).0, 6);

    let card = leak(format!("media-card-{}", ids[25].0));
    click(vcx, card);
    double_click(vcx, card);
    view.update(vcx, |app, _| assert!(app.is_fullscreen_viewer));
    focus_app(vcx, &view);
    keystrokes(vcx, "escape");
    render_ui(vcx);
    view.update(vcx, |app, _| assert!(!app.is_fullscreen_viewer));
    assert_eq!(
        visible_rows(&view, vcx).0,
        6,
        "closing the viewer must leave the grid where it was"
    );
}

#[core::prelude::v1::test]
fn request_quit_still_quits_after_saving() {
    let (_dir, engine) = seed_library("restore_quit", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| app.request_quit(cx));
    view.update(vcx, |app, _| {
        assert!(
            app.downloads.quit_requested,
            "saving first must not stop Quit"
        );
    });
}

#[core::prelude::v1::test]
fn closing_the_main_window_runs_the_quit_path() {
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = cx.add_window_view(LuminaApp::new_in_window);
    assert!(
        !vcx.simulate_close(),
        "the window stays open until the quit completes"
    );
    view.update(vcx, |app, _| {
        assert!(
            app.downloads.quit_requested,
            "closing the main window quits the app"
        );
    });
}

#[core::prelude::v1::test]
fn a_restored_album_with_a_query_comes_back_as_library() {
    let (_dir, engine) = seed_library("restore_scope", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let mut saved = session(LastView::Favorites, None, None);
    saved.search_query = "kyoto".into();
    start_with_pending(&view, vcx, saved);
    finish_scan(&view, vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.search_query, "kyoto");
        assert_eq!(
            app.selection,
            AlbumSelection::AllLibrary,
            "a restored query searches the whole library, so the sidebar says Library"
        );
    });
}

/// Starts the app from a config whose last view is Favorites.
fn launch_with(view_on_launch: crate::config::LaunchView, name: &str) -> AlbumSelection {
    let (_dir, engine) = seed_library(name, 4);
    crate::app::state::TEST_CONFIG.with(|c| {
        *c.borrow_mut() = Some(crate::config::AppConfig {
            last_view: Some(LastView::Favorites.token()),
            launch_view: view_on_launch,
            ..Default::default()
        })
    });
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    finish_scan(&view, vcx);
    let selection = view.update(vcx, |app, _| {
        assert!(app.pending_restore.is_none(), "the restore ran");
        app.selection.clone()
    });
    selection
}

#[core::prelude::v1::test]
fn launch_view_library_opens_all_media_instead_of_last_view() {
    assert_eq!(
        launch_with(crate::config::LaunchView::Library, "launch_library"),
        AlbumSelection::AllLibrary,
        "Default view on launch = Library opens All Media, not the last view"
    );
}

#[core::prelude::v1::test]
fn launch_view_last_viewed_still_restores_last_view() {
    assert_eq!(
        launch_with(crate::config::LaunchView::LastViewed, "launch_last"),
        FAVORITES,
        "Last viewed still reopens the last view"
    );
}
