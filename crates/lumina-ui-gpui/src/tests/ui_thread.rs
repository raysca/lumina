//! The window never waits on the catalog lock: a scan or import holding it
//! must not freeze rendering or viewer navigation (AGENTS §1, "the UI never
//! blocks on IO").

use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::VisualTestContext;
use lumina_core::LuminaEngine;

use crate::tests::ui_automation::{app_with_library, render_ui, seed_library};

/// How long the "scan" keeps the catalog locked at most.
const HOLD: Duration = Duration::from_millis(500);
/// Generous debug budget for work that must not touch SQLite.
const BUDGET: Duration = Duration::from_millis(150);

/// Times `f` while another thread holds the catalog lock. The lock is
/// released as soon as `f` returns, or after [`HOLD`] if `f` is stuck on it.
fn timed_while_catalog_locked(engine: &Arc<LuminaEngine>, f: impl FnOnce()) -> Duration {
    let (held_tx, held_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel::<()>();
    let holder = engine.clone();
    let thread = std::thread::spawn(move || {
        holder.with_catalog_locked(|| {
            held_tx.send(()).expect("test thread waits for the lock");
            let _ = done_rx.recv_timeout(HOLD);
        })
    });
    held_rx.recv().expect("the holder takes the lock");
    let start = Instant::now();
    f();
    let took = start.elapsed();
    let _ = done_tx.send(());
    thread.join().expect("the holder thread finishes");
    took
}

/// One frame, without running background work.
fn draw(vcx: &mut VisualTestContext) {
    vcx.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear();
    });
}

#[core::prelude::v1::test]
fn a_frame_never_waits_on_the_catalog_lock() {
    let (dir, engine) = seed_library("ui_thread_frame", 24);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    view.update(vcx, |app, cx| {
        app.search_query = "photo".to_string();
        cx.notify();
    });
    render_ui(vcx);
    render_ui(vcx);

    let steady = timed_while_catalog_locked(&engine, || draw(vcx));
    assert!(
        steady < BUDGET,
        "redrawing an unchanged search must not query the catalog: took {steady:?}"
    );

    view.update(vcx, |app, cx| {
        app.search_query = "photo_0".to_string();
        cx.notify();
    });
    let switched = timed_while_catalog_locked(&engine, || draw(vcx));
    assert!(
        switched < BUDGET,
        "the first frame of a new search must not wait for its results: took {switched:?}"
    );
    render_ui(vcx);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(
            app.get_active_count() > 0,
            "the new search's results arrive once the lock is free"
        )
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn viewer_arrows_never_wait_on_the_catalog_lock() {
    let (dir, engine) = seed_library("ui_thread_viewer", 24);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    view.update(vcx, |app, cx| {
        app.select_index(0, cx);
        let item = app.selected_item.clone().expect("first item selected");
        app.open_viewer(item, cx);
    });
    render_ui(vcx);
    render_ui(vcx);

    let took = timed_while_catalog_locked(&engine, || {
        view.update(vcx, |app, cx| app.navigate_selection(1, cx));
        draw(vcx);
    });
    assert!(
        took < BUDGET,
        "stepping to the next photo must not wait on the catalog: took {took:?}"
    );
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.selected_index, Some(1), "the step still lands");
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn tags_follow_the_latest_selection_even_when_loads_land_late() {
    let (dir, engine) = seed_library("ui_thread_tags", 4);
    let items = engine.get_item_slice(0..2);
    engine
        .add_tags_batch(&[items[0].id], &["first".to_string()])
        .unwrap();
    engine
        .add_tags_batch(&[items[1].id], &["second".to_string()])
        .unwrap();
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    let first_request = view.update(vcx, |app, cx| {
        app.select_item(items[0].clone(), cx);
        app.selected_tags_request
    });
    view.update(vcx, |app, cx| app.select_item(items[1].clone(), cx));
    render_ui(vcx);
    view.update(vcx, |app, cx| {
        assert_eq!(app.selected_item_tags, vec!["second".to_string()]);
        // The first selection's load arriving after the second's.
        app.apply_selected_item_tags(first_request, vec!["first".to_string()], cx);
        assert_eq!(
            app.selected_item_tags,
            vec!["second".to_string()],
            "a late load for an earlier selection must not overwrite the current tags"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
/// The redraw a scan or watcher event triggers then shows the change; the
/// test draws those frames itself.
fn after_a_catalog_change_the_next_frames_show_it() {
    let (dir, engine) = seed_library("ui_thread_revision", 4);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    view.update(vcx, |app, cx| {
        app.search_query = "tag:fresh".to_string();
        cx.notify();
    });
    render_ui(vcx);
    view.update(vcx, |app, _| assert_eq!(app.displayed_count(), 0));

    // A write from elsewhere (a scan, the watcher, an import).
    let id = engine.get_item_slice(0..1)[0].id;
    engine
        .add_tags_batch(&[id], &["fresh".to_string()])
        .unwrap();
    render_ui(vcx);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.displayed_count(),
            1,
            "the cache refetches once the catalog revision moves"
        );
        assert_eq!(
            app.displayed_rows(0..1).first().map(|item| item.id),
            Some(id)
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn opening_another_library_never_shows_the_last_ones_rows() {
    let (dir_a, engine_a) = seed_library("ui_thread_lib_a", 3);
    let (dir_b, _engine_b) = seed_library("ui_thread_lib_b", 5);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine_a);
    render_ui(vcx);
    render_ui(vcx);

    view.update(vcx, |app, cx| {
        app.open_library_inner(dir_b.clone(), cx);
        // Before the new library's first fetch lands. Ids restart per
        // catalog, so an old row would act on the wrong photo.
        let shown = app.displayed_rows(0..5);
        assert!(
            shown.iter().all(|item| item.path.starts_with(&dir_b)),
            "the grid must not show the previous library's items: {:?}",
            shown.iter().map(|item| &item.path).collect::<Vec<_>>()
        );
        if let Some(item) = app.navigation_item(0) {
            assert!(
                item.path.starts_with(&dir_b),
                "arrows pick from the new library"
            );
        }
    });
    render_ui(vcx);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.displayed_count(), 5, "the new library's rows arrive");
    });
    let _ = std::fs::remove_dir_all(&dir_a);
    let _ = std::fs::remove_dir_all(&dir_b);
}

#[core::prelude::v1::test]
fn viewer_arrows_during_a_scan_never_wait_on_the_catalog_lock() {
    let (dir, engine) = seed_library("ui_thread_scan", 24);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    view.update(vcx, |app, cx| {
        app.search_query = "photo".to_string();
        app.select_index(0, cx);
        let item = app.selected_item.clone().expect("first item selected");
        app.open_viewer(item, cx);
    });
    render_ui(vcx);
    render_ui(vcx);

    // A scan batch lands from the scan's thread (the revision moves), then
    // the scan holds the lock.
    let last = engine.get_item_slice(23..24)[0].id;
    let scanner = engine.clone();
    std::thread::spawn(move || {
        scanner
            .add_tags_batch(&[last], &["scanned".to_string()])
            .unwrap()
    })
    .join()
    .expect("the scan batch lands");
    let took = timed_while_catalog_locked(&engine, || {
        view.update(vcx, |app, cx| app.navigate_selection(1, cx));
    });
    assert!(
        took < BUDGET,
        "an arrow press mid-scan must use the rows on screen, not wait: took {took:?}"
    );
    render_ui(vcx);
    view.update(vcx, |app, _| assert_eq!(app.selected_index, Some(1)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn trashing_in_the_viewer_moves_on_to_the_next_photo() {
    let (dir, engine) = seed_library("ui_thread_trash", 6);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let next = view.update(vcx, |app, cx| {
        app.select_index(0, cx);
        let item = app.selected_item.clone().expect("first item selected");
        app.open_viewer(item, cx);
        app.get_active_slice(1..2)[0].id
    });
    render_ui(vcx);
    render_ui(vcx);

    view.update(vcx, |app, cx| {
        let trashed = app.selected_item.clone().expect("a photo is shown").id;
        app.trash_or_restore_selected(cx);
        let shown = app.selected_item.clone().expect("a photo is still shown");
        assert_ne!(
            shown.id, trashed,
            "after its own write the app must remap the selection, not reuse cached rows"
        );
        assert_eq!(shown.id, next, "the viewer moves on to the next photo");
        assert!(!shown.is_trashed, "the photo now shown is not in the Trash");
    });
    let _ = std::fs::remove_dir_all(&dir);
}
