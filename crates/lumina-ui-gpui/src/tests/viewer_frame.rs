//! The viewer's full-size frame is decoded only for the open viewer, once
//! stepping pauses, and only the latest request's answer is shown.

use super::helpers::{app_with_library, render_ui, seed_library, tiny_decoded_frame};
use super::viewer_stage::open_at;
use crate::app::viewer_frame::VIEWER_FRAME_REQUESTS;
use crate::app::UiMessage;
use std::time::Duration;

fn requests() -> usize {
    VIEWER_FRAME_REQUESTS.with(|n| n.get())
}

#[core::prelude::v1::test]
fn browsing_the_grid_with_the_viewer_closed_decodes_no_viewer_frame() {
    let (_dir, engine) = seed_library("viewer_frame_closed", 6);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let before = requests();
    view.update(vcx, |app, cx| {
        let item = app.get_active_slice(0..1)[0].clone();
        app.select_item_at(item, 0, cx);
        for _ in 0..5 {
            app.navigate_selection(1, cx);
        }
    });
    vcx.executor().advance_clock(Duration::from_millis(500));
    vcx.run_until_parked();
    assert_eq!(
        requests(),
        before,
        "selecting in the grid with the viewer closed must not queue a full-size decode"
    );

    view.update(vcx, |app, cx| {
        let item = app.selected_item.clone().expect("an item is selected");
        app.open_viewer(item, cx);
    });
    assert_eq!(
        requests(),
        before + 1,
        "opening the viewer asks for the shown item's frame once"
    );
}

#[core::prelude::v1::test]
fn stepping_quickly_decodes_only_the_item_landed_on() {
    let (_dir, engine) = seed_library("viewer_frame_steps", 6);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let (before, landed) = view.update(vcx, |app, cx| {
        open_at(app, 0, cx);
        let before = requests();
        for _ in 0..4 {
            app.navigate_selection(1, cx);
        }
        (before, app.selected_item.as_ref().map(|item| item.id))
    });
    assert_eq!(
        requests(),
        before,
        "steps right after one another wait for stepping to pause"
    );
    vcx.executor().advance_clock(Duration::from_millis(150));
    vcx.run_until_parked();
    assert_eq!(
        requests(),
        before + 1,
        "once stepping pauses only the item landed on is decoded"
    );
    view.update(vcx, |app, _| {
        assert_eq!(app.selected_item.as_ref().map(|item| item.id), landed);
    });
}

#[core::prelude::v1::test]
fn an_answer_for_an_earlier_request_is_dropped() {
    let (_dir, engine) = seed_library("viewer_frame_stale", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        let id = open_at(app, 0, cx);
        let old = app.viewer_frame.generation;
        // A newer request for the same item (a rotation, a discarded edit).
        app.request_viewer_frame(id);
        app.set_viewer_image(None);
        app.handle_ui_message(
            UiMessage::ViewerLoaded {
                id,
                generation: old,
                frame: tiny_decoded_frame(id),
            },
            cx,
        );
        assert!(
            app.viewer_image.is_none(),
            "an answer from a superseded request must not be shown"
        );
        app.handle_ui_message(
            UiMessage::ViewerFailed {
                id,
                generation: old,
            },
            cx,
        );
        assert_ne!(
            app.viewer_failed,
            Some(id),
            "a superseded failure must not mark the stage failed"
        );
        let current = app.viewer_frame.generation;
        app.handle_ui_message(
            UiMessage::ViewerLoaded {
                id,
                generation: current,
                frame: tiny_decoded_frame(id),
            },
            cx,
        );
        assert!(
            app.viewer_image.is_some(),
            "the current request's answer is shown"
        );
    });
    render_ui(vcx);
}

#[core::prelude::v1::test]
fn the_viewer_asks_for_a_frame_when_the_grid_thumbnail_failed() {
    let (_dir, engine) = seed_library("viewer_frame_grid_failed", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        let item = app.get_active_slice(0..1)[0].clone();
        app.grid_state
            .as_ref()
            .expect("a library is open")
            .mark_failed(item.id);
        let before = requests();
        app.open_viewer(item, cx);
        assert_eq!(
            requests(),
            before + 1,
            "a failed grid thumbnail must not stop the viewer's own decode"
        );
    });
}
