//! Files arriving in the library (an import batch, a download, a file
//! dropped into the folder) shift positions but must not take the selection,
//! the open viewer or an unsaved edit away.

use crate::app::types::UiMessage;
use crate::tests::helpers::{app_with_library, keystrokes, render_ui};
use crate::tests::quick_edit::{edit_library, editing, open_viewer_on};
use lumina_core::{EngineEvent, LuminaEngine, MediaItem};

/// Catalogues a copy of `like` as a new file dated after every other item,
/// so it sorts first and moves every row down by one.
fn add_newest(engine: &LuminaEngine, like: &MediaItem) -> Vec<MediaItem> {
    let path = like.path.with_file_name("arrived.jpg");
    std::fs::copy(&like.path, &path).expect("copy the photo");
    let mut item = like.clone();
    item.id = lumina_core::MediaId(0);
    item.file_name = "arrived.jpg".into();
    item.path = path;
    item.timestamp_epoch_ms += 86_400_000;
    engine
        .catalog()
        .insert_or_update_batch(&[item])
        .expect("catalogue the new file")
}

#[core::prelude::v1::test]
fn a_new_file_keeps_the_viewer_and_the_edit_draft_on_the_same_photo() {
    let (_dir, engine, _photos, _) = edit_library("additions_keep_viewer");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let first = view.update(vcx, |app, _| app.get_active_slice(0..1).remove(0));
    open_viewer_on(&view, vcx, &engine, first.id);
    keystrokes(vcx, "e =");
    assert!(editing(&view, vcx), "the draft is open");

    let added = add_newest(&engine, &first);
    view.update(vcx, |app, cx| {
        // The scan is over: batches from here on are the watcher's or an
        // import's.
        app.grid_state.as_mut().expect("library").is_scanning = false;
        app.handle_ui_message(
            UiMessage::Engine(EngineEvent::BatchDiscovered { items: added }),
            cx,
        );
    });
    render_ui(vcx);

    assert!(
        editing(&view, vcx),
        "an unrelated file arriving must not discard the edit draft"
    );
    view.update(vcx, |app, _| {
        assert!(app.viewer_shown(), "the viewer stays open");
        assert_eq!(
            app.selected_item.as_ref().map(|item| item.id),
            Some(first.id),
            "the viewer still shows the same photo"
        );
        assert_eq!(
            app.selected_index,
            Some(1),
            "the position follows the photo, which moved down one row"
        );
        assert_eq!(
            app.get_active_slice(1..2).first().map(|item| item.id),
            Some(first.id),
            "so the next arrow key starts from the right row"
        );
    });
}

#[core::prelude::v1::test]
fn a_new_file_still_clears_a_multi_selection() {
    let (_dir, engine, _photos, _) = edit_library("additions_multi");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let first = view.update(vcx, |app, cx| {
        let rows = app.get_active_slice(0..2);
        app.select_item_at(rows[0].clone(), 0, cx);
        app.selected_indices = [0, 1].into_iter().collect();
        rows[0].clone()
    });
    let added = add_newest(&engine, &first);
    view.update(vcx, |app, cx| {
        // The scan is over: batches from here on are the watcher's or an
        // import's.
        app.grid_state.as_mut().expect("library").is_scanning = false;
        app.handle_ui_message(
            UiMessage::Engine(EngineEvent::BatchDiscovered { items: added }),
            cx,
        );
        assert!(
            app.selected_indices.is_empty() && app.selected_item.is_none(),
            "positions of a multi-selection cannot be trusted after rows shift, \
             so it is cleared rather than left on other items"
        );
    });
}

#[core::prelude::v1::test]
fn arrows_after_a_file_arrives_step_to_the_true_neighbour() {
    let (_dir, engine, _photos, _) = edit_library("additions_arrows");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let rows = view.update(vcx, |app, _| app.get_active_slice(0..3));
    open_viewer_on(&view, vcx, &engine, rows[0].id);

    // The watcher and imports write on their own threads, so the UI
    // thread's cache is not invalidated by its own write.
    let added = {
        let (engine, first) = (engine.clone(), rows[0].clone());
        std::thread::spawn(move || add_newest(&engine, &first))
            .join()
            .expect("the writer thread finishes")
    };
    view.update(vcx, |app, cx| {
        app.grid_state.as_mut().expect("library").is_scanning = false;
        // The refetch is slow (an import holds the lock): the cached rows
        // keep the old order.
        app.active_view.hold_fetch();
        app.handle_ui_message(
            UiMessage::Engine(EngineEvent::BatchDiscovered { items: added }),
            cx,
        );
    });
    keystrokes(vcx, "right");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_item.as_ref().map(|item| item.id),
            Some(rows[1].id),
            "→ steps to the photo after the shown one, not one row further"
        );
    });
}
