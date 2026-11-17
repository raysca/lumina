//! The viewer's filmstrip.

use super::ui_automation::{
    click, focus_app, keystrokes, leak, render_ui, seed_library_undecodable,
};
use super::viewer_stage::{app_without_decodes, detach_decoder_answers, open_at};
use crate::app::state::THUMB_REQUEST_ITEMS;
use crate::views::viewer::filmstrip::{filmstrip_slots, filmstrip_window, FILMSTRIP_MAX};
use crate::LuminaApp;
use gpui::{Entity, VisualTestContext};
use lumina_core::MediaId;

fn ids(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> Vec<MediaId> {
    view.update(vcx, |app, _| {
        let n = app.displayed_count();
        app.get_active_slice(0..n).iter().map(|i| i.id).collect()
    })
}

fn thumb(id: MediaId) -> &'static str {
    leak(format!("viewer-filmstrip-thumb-{}", id.0))
}

/// Thumbs painted, the current one included.
fn painted(vcx: &mut VisualTestContext, ids: &[MediaId]) -> usize {
    let others = ids
        .iter()
        .filter(|id| vcx.debug_bounds(thumb(**id)).is_some())
        .count();
    others + usize::from(vcx.debug_bounds("viewer-filmstrip-current").is_some())
}

#[core::prelude::v1::test]
fn filmstrip_window_is_bounded_and_never_wraps() {
    assert_eq!(filmstrip_window(9, 30, 15), 2..17, "centred on #10");
    assert_eq!(
        filmstrip_window(0, 30, 15),
        0..15,
        "first: off-centre, no gap"
    );
    assert_eq!(
        filmstrip_window(29, 30, 15),
        15..30,
        "last: off-centre, no gap"
    );
    assert_eq!(filmstrip_window(3, 5, 15), 0..5, "a short view shows all");
    assert_eq!(
        filmstrip_window(50_000, 100_000, 15),
        49_993..50_008,
        "a huge view still renders 15"
    );
    assert_eq!(filmstrip_slots(None), FILMSTRIP_MAX);
    assert_eq!(filmstrip_slots(Some(1260.0)), FILMSTRIP_MAX);
    let narrow = filmstrip_slots(Some(620.0));
    assert!(
        narrow < FILMSTRIP_MAX && narrow % 2 == 1 && narrow >= 1,
        "a narrow stage shows fewer thumbs, still centred: {narrow}"
    );
}

#[core::prelude::v1::test]
fn viewer_filmstrip_centres_current_and_navigates() {
    let (dir, engine) = seed_library_undecodable("viewer_filmstrip", 30);
    let mut cx = crate::tests::TestApp::single();
    // #9 failed to decode; #11 has a cached grid thumbnail.
    let seeded: Vec<MediaId> = engine.get_item_slice(0..30).iter().map(|i| i.id).collect();
    let (view, vcx) = app_without_decodes(&mut cx, engine, &[seeded[8]]);
    vcx.simulate_resize(gpui::size(gpui::px(1600.0), gpui::px(1000.0)));
    let ids = ids(&view, vcx);
    assert_eq!(
        ids, seeded,
        "test premise: the view lists the seed in order"
    );
    view.update(vcx, |app, cx| {
        app.handle_ui_message(
            crate::UiMessage::ThumbnailLoaded {
                id: ids[10],
                frame: super::viewer_and_video::tiny_decoded_frame(ids[10]),
            },
            cx,
        );
        app.inspector_open = true;
        open_at(app, 9, cx);
        detach_decoder_answers(app);
    });
    render_ui(vcx);
    let leak_id = |kind: &str, id: MediaId| leak(format!("viewer-filmstrip-{kind}-{}", id.0));
    assert!(
        vcx.debug_bounds(leak_id("img", ids[10])).is_some(),
        "a cached grid thumbnail shows in the strip"
    );
    assert!(
        vcx.debug_bounds(leak_id("failed", ids[8])).is_some(),
        "an undecodable item gets the placeholder glyph"
    );
    assert!(
        vcx.debug_bounds(leak_id("img", ids[11])).is_none()
            && vcx.debug_bounds(leak_id("failed", ids[11])).is_none(),
        "a thumb still decoding shows the plain well"
    );

    let strip = vcx
        .debug_bounds("viewer-filmstrip")
        .expect("the filmstrip paints");
    let viewport = vcx.debug_bounds("viewer-viewport").expect("viewport");
    assert!(
        viewport.bottom() <= strip.top(),
        "the viewport ends above the strip: {viewport:?} {strip:?}"
    );
    let current = vcx
        .debug_bounds("viewer-filmstrip-current")
        .expect("current thumb");
    assert_eq!(current.size, gpui::size(gpui::px(74.0), gpui::px(74.0)));
    assert!(
        vcx.debug_bounds(thumb(ids[9])).is_none(),
        "the current item is drawn once, as the current thumb"
    );
    assert_eq!(painted(vcx, &ids), FILMSTRIP_MAX, "7 either side of #10");
    let next = vcx.debug_bounds(thumb(ids[10])).expect("#11 paints");
    assert_eq!(next.size, gpui::size(gpui::px(58.0), gpui::px(58.0)));
    assert!(
        next.left() > current.right(),
        "#11 sits right of the current one"
    );
    assert!(
        (current.center().x - strip.center().x).abs() <= gpui::px(1.0),
        "mid-view, the current thumb is centred: {current:?} {strip:?}"
    );

    click(vcx, thumb(ids[10]));
    view.update(vcx, |app, _| {
        assert_eq!(app.selected_index, Some(10), "the click opens #11");
        assert_eq!(app.selected_item.as_ref().map(|i| i.id), Some(ids[10]));
        assert!(app.is_fullscreen_viewer, "and stays in the viewer");
    });
    assert!(
        vcx.debug_bounds(thumb(ids[10])).is_none(),
        "#11 is now current"
    );
    assert!(
        vcx.debug_bounds(thumb(ids[9])).is_some(),
        "#10 is a plain thumb now"
    );

    // The first item: off-centre, no empty slots.
    view.update(vcx, |app, cx| app.navigate_selection(-10, cx));
    render_ui(vcx);
    let current = vcx.debug_bounds("viewer-filmstrip-current").unwrap();
    let after = vcx.debug_bounds(thumb(ids[1])).unwrap();
    assert!(
        current.center().x < strip.center().x && after.left() > current.right(),
        "at the start the current thumb leads: {current:?}"
    );
    assert_eq!(painted(vcx, &ids), FILMSTRIP_MAX);
    assert!(vcx.debug_bounds(thumb(ids[29])).is_none(), "no wrap-around");

    // Narrow window beside the docked panel: fewer thumbs.
    view.update(vcx, |app, cx| {
        app.window_bounds = Some((0.0, 0.0, 960.0, 600.0));
        cx.notify();
    });
    vcx.simulate_resize(gpui::size(gpui::px(960.0), gpui::px(600.0)));
    render_ui(vcx);
    let shown = painted(vcx, &ids);
    assert!(
        shown < FILMSTRIP_MAX,
        "a 620-wide stage shows fewer thumbs, got {shown}"
    );
    let strip = vcx.debug_bounds("viewer-filmstrip").unwrap();
    let panel = vcx.debug_bounds("viewer-info-drawer").unwrap();
    assert!(
        strip.right() <= panel.left(),
        "the strip stays in the stage"
    );
    let viewport = vcx.debug_bounds("viewer-viewport").unwrap();
    assert!(
        viewport.bottom() <= strip.top() && viewport.size.height >= gpui::px(300.0),
        "at 960x600 the photo stage keeps room above the strip: {viewport:?} {strip:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn viewer_filmstrip_hides_in_immersive_slideshow_and_edit_mode() {
    let (dir, engine) = seed_library_undecodable("viewer_filmstrip_modes", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_without_decodes(&mut cx, engine, &[]);
    view.update(vcx, |app, cx| {
        open_at(app, 1, cx);
        detach_decoder_answers(app);
    });
    render_ui(vcx);
    assert!(vcx.debug_bounds("viewer-filmstrip").is_some());

    view.update(vcx, |app, cx| {
        app.is_immersive_fullscreen = true;
        app.immersive_hud_visible = true;
        cx.notify();
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("viewer-filmstrip").is_none(),
        "immersive hides it"
    );

    view.update(vcx, |app, cx| {
        app.is_immersive_fullscreen = false;
        app.enter_edit_mode(cx);
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("viewer-filmstrip").is_none(),
        "edit mode hides it"
    );
    view.update(vcx, |app, cx| app.cancel_edit(cx));
    render_ui(vcx);

    view.update(vcx, |app, cx| app.start_slideshow_deferred(cx));
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.slideshow.is_some(), "test premise")
    });
    assert!(
        vcx.debug_bounds("viewer-filmstrip").is_none(),
        "a slideshow hides it"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn viewer_filmstrip_queues_each_thumb_once_while_stepping() {
    let (dir, engine) = seed_library_undecodable("viewer_filmstrip_queue", 30);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_without_decodes(&mut cx, engine, &[]);
    vcx.simulate_resize(gpui::size(gpui::px(1600.0), gpui::px(1000.0)));
    view.update(vcx, |app, cx| {
        // A fresh claim set: the strip's thumbs are now uncached and free.
        // The seeds never decode, so each id fails once and stays failed.
        let state = app.grid_state.as_mut().unwrap();
        state.in_flight = std::sync::Arc::new(parking_lot::Mutex::new(Default::default()));
        state.thumb_failures = std::sync::Arc::new(parking_lot::Mutex::new(Default::default()));
        THUMB_REQUEST_ITEMS.with(|items| items.borrow_mut().clear());
        open_at(app, 9, cx);
        detach_decoder_answers(app);
    });
    render_ui(vcx);
    let all = ids(&view, vcx);
    view.update(vcx, |app, _| {
        let state = app.grid_state.as_ref().unwrap();
        for (index, id) in all.iter().enumerate().take(17).skip(2) {
            assert!(
                state.in_flight.lock().contains(id) || state.is_failed(*id),
                "the strip queues every uncached thumb it shows: #{} was not",
                index + 1
            );
        }
    });
    let requested = || THUMB_REQUEST_ITEMS.with(|items| items.borrow().len());
    let opened = requested();

    focus_app(vcx, &view);
    for _ in 0..5 {
        keystrokes(vcx, "]");
        render_ui(vcx);
    }
    let stepped = requested() - opened;
    assert!(
        stepped <= 5,
        "each ] step queues only the thumb entering the strip, got {stepped} for 5 steps"
    );
    THUMB_REQUEST_ITEMS.with(|items| {
        let items = items.borrow();
        let unique: std::collections::HashSet<_> = items.iter().map(|(id, _)| *id).collect();
        assert_eq!(unique.len(), items.len(), "no thumb is queued twice");
    });
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_index,
            Some(14),
            "] steps selection and strip together"
        )
    });
    assert!(vcx.debug_bounds("viewer-filmstrip-current").is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn viewer_filmstrip_click_opens_the_clicked_item_while_rows_are_stale() {
    let (dir, engine) = seed_library_undecodable("viewer_filmstrip_stale", 20);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_without_decodes(&mut cx, engine.clone(), &[]);
    vcx.simulate_resize(gpui::size(gpui::px(1600.0), gpui::px(1000.0)));
    let ids = ids(&view, vcx);
    view.update(vcx, |app, cx| {
        open_at(app, 9, cx);
        detach_decoder_answers(app);
    });
    render_ui(vcx);

    // #6 leaves the view while the next fetch is slow: the strip still
    // draws the old rows, and the selection is already remapped (#10 is
    // now at position 8).
    engine.set_trashed(ids[5], true).expect("trash writes");
    view.update(vcx, |app, cx| {
        app.active_view.hold_fetch();
        app.selected_index = Some(8);
        app.selected_indices = std::iter::once(8).collect();
        app.selection_anchor = Some(8);
        cx.notify();
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds(thumb(ids[9])).is_none(),
        "the outline stays on the shown item (#10), matched by id"
    );
    assert!(
        vcx.debug_bounds(thumb(ids[8])).is_some(),
        "#9 is a plain thumb even though it sits at the remapped position"
    );

    click(vcx, thumb(ids[12]));
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_item.as_ref().map(|i| i.id),
            Some(ids[12]),
            "a click opens exactly the clicked thumb (#13), not a neighbour"
        );
        assert_eq!(app.selected_index, Some(11), "at its fresh position");
    });
    let _ = std::fs::remove_dir_all(&dir);
}
