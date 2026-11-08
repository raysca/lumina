//! The window never waits on the catalog lock: a scan or import holding it
//! must not freeze rendering or viewer navigation (AGENTS §1, "the UI never
//! blocks on IO").

use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use gpui::VisualTestContext;
use lumina_core::LuminaEngine;

use crate::tests::ui_automation::{app_with_library, render_ui, seed_library};

/// How long the "scan" keeps the catalog locked at most. Far longer than any
/// frame or key press takes even on a loaded debug build, so outlasting it
/// means the UI was stuck on the lock.
const HOLD: Duration = Duration::from_secs(5);

/// Counts the catalog lock calls this thread (the UI thread) makes in `f`
/// while another thread holds the lock. Every such call would wait, so the UI
/// must make none. Counting rather than timing keeps the check exact on a
/// loaded machine, where a slow debug frame is not a wait. The lock is
/// released as soon as `f` returns, or after [`HOLD`] if `f` is stuck on it;
/// that second case also catches a wait the count cannot see, such as
/// joining a thread that queries the catalog.
fn catalog_locks_while_held(engine: &Arc<LuminaEngine>, f: impl FnOnce()) -> u64 {
    let (held_tx, held_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel::<()>();
    let holder = engine.clone();
    let thread = std::thread::spawn(move || {
        holder.with_catalog_locked(|| {
            held_tx.send(()).expect("test thread waits for the lock");
            done_rx.recv_timeout(HOLD).is_err()
        })
    });
    held_rx.recv().expect("the holder takes the lock");
    let before = LuminaEngine::catalog_locks_on_this_thread();
    f();
    let locks = LuminaEngine::catalog_locks_on_this_thread() - before;
    let _ = done_tx.send(());
    let outlasted = thread.join().expect("the holder thread finishes");
    assert!(
        !outlasted,
        "UI work was still running when the {HOLD:?} lock hold ended: it waited on the catalog"
    );
    locks
}

/// One frame, without running background work.
fn draw(vcx: &mut VisualTestContext) {
    vcx.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
}

#[core::prelude::v1::test]
fn a_frame_never_waits_on_the_catalog_lock() {
    let (dir, engine) = seed_library("ui_thread_frame", 24);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    view.update(vcx, |app, cx| {
        app.search_query = "photo".to_string();
        cx.notify();
    });
    render_ui(vcx);
    render_ui(vcx);

    let steady = catalog_locks_while_held(&engine, || draw(vcx));
    assert_eq!(
        steady, 0,
        "redrawing an unchanged search must not query the catalog"
    );

    view.update(vcx, |app, cx| {
        app.search_query = "photo_0".to_string();
        cx.notify();
    });
    let switched = catalog_locks_while_held(&engine, || draw(vcx));
    assert_eq!(
        switched, 0,
        "the first frame of a new search must not wait for its results"
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
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    view.update(vcx, |app, cx| {
        app.select_index(0, cx);
        let item = app.selected_item.clone().expect("first item selected");
        app.open_viewer(item, cx);
    });
    render_ui(vcx);
    render_ui(vcx);

    let locks = catalog_locks_while_held(&engine, || {
        view.update(vcx, |app, cx| app.navigate_selection(1, cx));
        draw(vcx);
    });
    assert_eq!(
        locks, 0,
        "stepping to the next photo must not wait on the catalog"
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
    let mut cx = crate::tests::TestApp::single();
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
    let mut cx = crate::tests::TestApp::single();
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
    let mut cx = crate::tests::TestApp::single();
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
    let mut cx = crate::tests::TestApp::single();
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
    let locks = catalog_locks_while_held(&engine, || {
        view.update(vcx, |app, cx| app.navigate_selection(1, cx));
    });
    assert_eq!(
        locks, 0,
        "an arrow press mid-scan must use the rows on screen, not wait"
    );
    render_ui(vcx);
    view.update(vcx, |app, _| assert_eq!(app.selected_index, Some(1)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn trashing_in_the_viewer_moves_on_to_the_next_photo() {
    let (dir, engine) = seed_library("ui_thread_trash", 6);
    let mut cx = crate::tests::TestApp::single();
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

/// Selects every item of the view, as ⌘A does.
fn select_all(view: &gpui::Entity<crate::app::LuminaApp>, vcx: &mut VisualTestContext) {
    view.update(vcx, |app, cx| app.select_all(cx));
}

#[core::prelude::v1::test]
fn export_runs_in_background_and_refuses_a_second() {
    let (dir, engine) = seed_library("ui_thread_export", 3);
    let destination = dir.join("exported");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    select_all(&view, vcx);

    view.update(vcx, |app, cx| {
        app.command_query = "export".to_string();
        assert!(
            app.palette_rows()
                .iter()
                .any(|row| row.selector == "command-item-command.export"),
            "control: the palette lists Export while none runs"
        );
        app.command_query.clear();
        app.export_to_folder(
            destination.clone(),
            lumina_core::ExportPreset::Originals,
            cx,
        );
        assert_eq!(
            app.jobs.export,
            Some(3),
            "the export is recorded as running before any file is copied"
        );
        assert!(
            app.status_right_text().contains("Exporting 3 items"),
            "the status bar names the running export, got {:?}",
            app.status_right_text()
        );
        assert!(
            !destination.exists(),
            "the copy runs in the background, not inside the call"
        );
        app.command_query = "export".to_string();
        assert!(
            !app.palette_rows()
                .iter()
                .any(|row| row.selector == "command-item-command.export"),
            "the palette hides Export while one runs"
        );
        app.command_query.clear();
        app.export_to_folder(dir.join("second"), lumina_core::ExportPreset::Originals, cx);
        app.toast.dismiss();
        app.export_pick_folder(lumina_core::ExportPreset::Originals, cx);
        assert_eq!(
            app.toast.current().map(|t| t.message.as_str()),
            Some("An export is already running"),
            "the picker path refuses before any folder dialog opens"
        );
        assert_eq!(
            app.toast.current().map(|t| t.message.as_str()),
            Some("An export is already running"),
            "a second export is refused, not queued"
        );
    });
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.jobs.export, None, "a finished export clears the job");
        assert!(
            app.toast
                .current()
                .is_some_and(|t| t.message.contains("Exported 3")),
            "the result toast shows, got {:?}",
            app.toast.current().map(|t| &t.message)
        );
    });
    assert_eq!(
        std::fs::read_dir(&destination)
            .expect("the export folder exists")
            .filter(|e| e
                .as_ref()
                .is_ok_and(|e| e.path().extension().is_some_and(|x| x == "jpg")))
            .count(),
        3,
        "every selected file is copied"
    );
    assert!(
        !dir.join("second").exists(),
        "the refused export wrote nothing"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn export_failure_still_reaches_the_banner() {
    let (dir, engine) = seed_library("ui_thread_export_fail", 2);
    let destination = dir.join("not_a_folder");
    std::fs::write(&destination, b"a file").unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    select_all(&view, vcx);

    view.update(vcx, |app, cx| {
        app.export_to_folder(
            destination.clone(),
            lumina_core::ExportPreset::Originals,
            cx,
        )
    });
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.jobs.export, None, "a failed export clears the job");
        assert!(
            app.error_message
                .as_deref()
                .is_some_and(|m| m.starts_with("Export failed")),
            "a failed export reaches the error banner, got {:?}",
            app.error_message
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn xmp_import_runs_in_background() {
    let (dir, engine) = seed_library("ui_thread_xmp", 2);
    let item = engine.get_item_slice(0..1)[0].clone();
    lumina_core::write_sidecar(
        &item.path,
        &lumina_core::SidecarMetadata {
            rating: 4,
            ..Default::default()
        },
    )
    .unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    view.update(vcx, |app, cx| {
        app.selected_item = Some(item.clone());
        app.import_sidecars_selected(cx);
        assert_eq!(
            app.jobs.xmp,
            Some(1),
            "the XMP import is recorded as running"
        );
        assert!(
            app.status_right_text().contains("Reading XMP for 1 item"),
            "the status bar names the running XMP import, got {:?}",
            app.status_right_text()
        );
        assert_eq!(
            engine.get_ratings_map().get(&item.id).copied().unwrap_or(0),
            0,
            "the sidecar is read in the background, not inside the call"
        );
        app.import_sidecars_selected(cx);
        assert_eq!(
            app.toast.current().map(|t| t.message.as_str()),
            Some("An XMP import is already running"),
            "a second XMP import is refused"
        );
    });
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.jobs.xmp, None, "a finished XMP import clears the job");
        assert_eq!(
            engine.get_ratings_map().get(&item.id).copied(),
            Some(4),
            "the sidecar rating lands in the catalog"
        );
        assert_eq!(
            app.item_ratings.get(&item.id).copied(),
            Some(4),
            "the same library refreshes after the import"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn xmp_result_after_library_switch_does_not_refresh_the_new_library() {
    let (dir_a, engine_a) = seed_library("ui_thread_xmp_a", 2);
    let (dir_b, _engine_b) = seed_library("ui_thread_xmp_b", 2);
    let item = engine_a.get_item_slice(0..1)[0].clone();
    lumina_core::write_sidecar(
        &item.path,
        &lumina_core::SidecarMetadata {
            rating: 5,
            ..Default::default()
        },
    )
    .unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine_a.clone());

    view.update(vcx, |app, cx| {
        app.selected_item = Some(item.clone());
        app.import_sidecars_selected(cx);
        assert_eq!(app.jobs.xmp, Some(1), "the XMP import is still running");
        app.open_library(dir_b.clone(), cx);
        // A marker a refresh would reset (nothing is selected in the new
        // library). The new library's own scan never touches it.
        app.batch_summary.count = 99;
    });
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.jobs.xmp, None, "the job clears after a library switch");
        assert!(
            app.toast
                .current()
                .is_some_and(|t| t.message.starts_with("Imported 1 sidecar")),
            "the result toast still shows, got {:?}",
            app.toast.current().map(|t| &t.message)
        );
        assert_eq!(
            app.batch_summary.count, 99,
            "the old library's result must not refresh the new library's view"
        );
    });
    assert_eq!(
        engine_a.get_ratings_map().get(&item.id).copied(),
        Some(5),
        "the import still lands in the library it was started in"
    );
    let _ = std::fs::remove_dir_all(&dir_a);
    let _ = std::fs::remove_dir_all(&dir_b);
}
