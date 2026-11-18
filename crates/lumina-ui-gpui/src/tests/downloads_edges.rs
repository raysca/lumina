//! Downloads screen edges: the restored screen while the stored queue is
//! still loading, a failed Clear completed, ↑/↓ under a filter chip, and
//! row thumbnails. Rows are stored or set directly; no job runs, so nothing
//! touches the network.

use super::downloads::{install, persisted, queue_app, settle, states};
use super::downloads_screen::{marker, set_rows};
use super::helpers::{app_with_library, click, focus_app, keystrokes, render_ui};
use crate::app::types::UiMessage;
use crate::config::{LastView, RestoredSession};
use crate::views::sidebar::AlbumSelection;
use lumina_core::{DownloadState, EngineEvent, MediaId};

#[core::prelude::v1::test]
fn restored_downloads_screen_waits_for_the_queue_to_load() {
    let (_dir, engine, _fake, mut cx) = queue_app("dle_restore_loading");
    let mut stored = engine
        .catalog()
        .add_download("https://example.com/stored.mp4")
        .unwrap();
    stored.state = DownloadState::Failed;
    engine.catalog().update_download(&stored).unwrap();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        assert!(
            !app.downloads.loaded && app.downloads.entries.is_empty(),
            "the stored queue has not loaded yet"
        );
        app.pending_restore = Some(RestoredSession {
            view: LastView::Downloads,
            search_query: String::new(),
            active_year: None,
            selected_media_id: None,
            scroll_anchor_id: None,
        });
        // The startup scan finishing runs the restore.
        let total = app.grid_state.as_ref().map_or(0, |s| s.total_count);
        app.handle_ui_message(
            UiMessage::Engine(EngineEvent::ScanComplete { total_items: total }),
            cx,
        );
    });
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::Downloads,
            "a queue still loading is not an empty one: the saved screen comes back"
        )
    });
    assert!(
        marker(vcx, "downloads-screen".into()),
        "the restored Downloads screen is drawn, not the grid"
    );

    view.update(vcx, |app, cx| app.load_downloads(cx));
    settle(vcx, &view, "the stored queue loads", |app| {
        app.downloads.loaded
    });
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::Downloads,
            "the queue landing must not move the user off the restored screen"
        );
        assert_eq!(
            states(app),
            [DownloadState::Failed],
            "the stored row loads into the list as it was stored"
        );
    });
    assert!(
        marker(vcx, format!("downloads-screen-row-{}", stored.id)),
        "the row that loaded after the restore shows on the restored screen"
    );
}

#[core::prelude::v1::test]
fn failed_clear_completed_puts_the_rows_back() {
    let (dir, engine, fake, mut cx) = queue_app("dle_clear_fail");
    let urls = [
        ("https://example.com/done.mp4", DownloadState::Done),
        ("https://example.com/failed.mp4", DownloadState::Failed),
        ("https://example.com/dup.mp4", DownloadState::Duplicate),
    ];
    for (url, state) in urls {
        let mut row = engine.catalog().add_download(url).unwrap();
        row.state = state;
        engine.catalog().update_download(&row).unwrap();
    }
    rusqlite::Connection::open(dir.join("catalog.db"))
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER fail_clear BEFORE DELETE ON download_queue
             BEGIN SELECT RAISE(ABORT, 'injected'); END;",
        )
        .unwrap();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    view.update(vcx, |app, cx| app.open_library_inner(dir.clone(), cx));
    vcx.run_until_parked();
    install(&view, vcx, &fake);
    settle(vcx, &view, "the stored rows load", |app| {
        app.downloads.entries.len() == 3
    });
    let before: Vec<(i64, DownloadState)> = view.update(vcx, |app, cx| {
        app.show_downloads_screen(cx);
        app.downloads
            .entries
            .iter()
            .map(|e| (e.row.id, e.row.state))
            .collect()
    });
    render_ui(vcx);

    click(vcx, "downloads-clear-completed");
    settle(vcx, &view, "the failed clear is reported", |app| {
        app.error_message.is_some()
    });
    render_ui(vcx);
    view.update(vcx, |app, _| {
        let after: Vec<(i64, DownloadState)> = app
            .downloads
            .entries
            .iter()
            .map(|e| (e.row.id, e.row.state))
            .collect();
        assert_eq!(
            after, before,
            "a clear that couldn't be stored puts every row back, in queue order"
        );
    });
    assert!(marker(vcx, "error-banner".into()), "the banner is drawn");
    for (id, _) in &before {
        assert!(
            marker(vcx, format!("downloads-screen-row-{id}")),
            "row {id} is listed again"
        );
    }
    assert_eq!(
        persisted(&engine)
            .into_iter()
            .map(|(_, state)| state)
            .collect::<Vec<_>>(),
        [
            DownloadState::Done,
            DownloadState::Failed,
            DownloadState::Duplicate
        ],
        "the stored list is untouched"
    );
}

#[core::prelude::v1::test]
fn arrow_keys_skip_rows_the_filter_hides() {
    let (_dir, engine, fake, mut cx) = queue_app("dle_filter_keys");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    vcx.simulate_resize(gpui::size(gpui::px(1600.0), gpui::px(1000.0)));
    // Ids 1-5: paused rows 1, 3 and 5 between a running and a failed one.
    set_rows(
        &view,
        vcx,
        &[
            DownloadState::Paused,
            DownloadState::Running,
            DownloadState::Paused,
            DownloadState::Failed,
            DownloadState::Paused,
        ],
    );
    view.update(vcx, |app, cx| {
        app.show_downloads_screen(cx);
        // Focus a row the chip is about to hide.
        app.downloads.screen_focused = Some(2);
    });
    render_ui(vcx);
    click(vcx, "downloads-filter-paused");
    focus_app(vcx, &view);
    let focused =
        |vcx: &mut gpui::VisualTestContext| view.update(vcx, |app, _| app.downloads.screen_focused);
    keystrokes(vcx, "down");
    assert_eq!(
        focused(vcx),
        Some(1),
        "a hidden focused row is dropped: ↓ starts at the first shown row"
    );
    keystrokes(vcx, "down");
    assert_eq!(focused(vcx), Some(3), "↓ skips the hidden running row");
    keystrokes(vcx, "down");
    assert_eq!(focused(vcx), Some(5), "↓ skips the hidden failed row");
    keystrokes(vcx, "down");
    assert_eq!(focused(vcx), Some(5), "↓ stops at the last shown row");
    keystrokes(vcx, "up");
    assert_eq!(focused(vcx), Some(3), "↑ skips the hidden failed row");
}

#[core::prelude::v1::test]
fn catalogued_active_row_shows_its_thumbnail() {
    let (_dir, engine, fake, mut cx) = queue_app("dle_thumb");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    vcx.simulate_resize(gpui::size(gpui::px(1600.0), gpui::px(1000.0)));
    set_rows(
        &view,
        vcx,
        &[DownloadState::Running, DownloadState::Running],
    );
    view.update(vcx, |app, cx| {
        // Row 1 is catalogued (the seeded item); row 2 is not.
        let entry = &mut app.downloads.entries[0];
        entry.row.media_id = Some(MediaId(1));
        entry.row.in_library = true;
        app.show_downloads_screen(cx);
    });
    // The row asks for the seeded item's thumbnail like a grid card does;
    // the real decode lands in the shared texture cache.
    settle(
        vcx,
        &view,
        "the seeded item's thumbnail is decoded",
        |app| {
            app.grid_state
                .as_ref()
                .is_some_and(|state| state.texture_cache.lock().get(&MediaId(1)).is_some())
        },
    );
    render_ui(vcx);
    assert!(
        marker(vcx, "downloads-thumb-image-1".into()),
        "a catalogued row with a thumbnail shows the thumbnail"
    );
    assert!(
        !marker(vcx, "downloads-thumb-film-1".into()),
        "the placeholder gives way to the thumbnail"
    );
    assert!(
        marker(vcx, "downloads-thumb-film-2".into()),
        "a row with no catalogued item keeps the placeholder"
    );
}
