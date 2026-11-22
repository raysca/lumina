//! Motion: the touched mark pops, fresh thumbnails fade, fading-in
//! overlays stay fully usable, and closed ones fade out without taking
//! input.

use crate::tests::helpers::{
    app_with_library, click, keystrokes, render_ui, seed_library, seed_library_undecodable,
};

#[core::prelude::v1::test]
fn clicking_an_inspector_star_pops_that_star_only() {
    crate::motion::REDUCE_MOTION.with(|on| on.set(false));
    let (_dir, engine) = seed_library("motion_star", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    keystrokes(vcx, "home");
    view.update(vcx, |app, cx| {
        app.inspector_open = true;
        cx.notify();
    });
    render_ui(vcx);
    click(vcx, "inspector-star-4");
    // The popping star still renders and lays out.
    render_ui(vcx);
    assert!(vcx.debug_bounds("inspector-star-4").is_some());
    view.update(vcx, |app, _| {
        let id = app.selected_item.as_ref().map(|item| item.id);
        let (star, _) = app.mark_pop.star(id).expect("the clicked star pops");
        assert_eq!(star, 4, "only the star that was clicked");
    });
}

#[core::prelude::v1::test]
fn only_a_just_decoded_thumbnail_fades_in() {
    let (_dir, engine) = seed_library_undecodable("motion_thumb", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let (fresh, cached) = (lumina_core::MediaId(1), lumina_core::MediaId(2));
    view.update(vcx, |app, cx| {
        app.handle_ui_message(
            crate::UiMessage::ThumbnailLoaded {
                id: fresh,
                frame: super::helpers::tiny_decoded_frame(fresh),
                requested_size: crate::views::grid::THUMBNAIL_SIZE_MAX,
            },
            cx,
        );
        let state = app.grid_state.as_ref().expect("a library is open");
        assert!(
            state.thumb_is_fresh(fresh),
            "a thumbnail that just arrived fades"
        );
        assert!(
            !state.thumb_is_fresh(cached),
            "a cached thumbnail scrolled back into view appears at once"
        );
        let decoded = crate::motion::THUMB_IN * 2;
        for at in state.fresh_thumbs.lock().values_mut() {
            *at -= decoded;
        }
        assert!(
            !state.thumb_is_fresh(fresh),
            "once its fade is over the thumbnail is no longer fresh"
        );
    });
}

#[core::prelude::v1::test]
fn a_fading_palette_and_toast_work_mid_animation() {
    crate::motion::REDUCE_MOTION.with(|on| on.set(false));
    let (_dir, engine) = seed_library("motion_overlays", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    keystrokes(vcx, "secondary-k");
    assert!(
        vcx.debug_bounds("command-palette").is_some(),
        "the palette lays out while it fades in"
    );
    keystrokes(vcx, "escape");
    view.update(vcx, |app, cx| app.show_toast("Saved", cx));
    render_ui(vcx);
    click(vcx, "toast-dismiss");
    view.update(vcx, |app, _| {
        assert!(app.toast.current().is_none(), "a rising toast takes clicks")
    });
}

#[core::prelude::v1::test]
fn closed_overlays_and_toasts_fade_out_and_take_no_clicks() {
    crate::motion::EXITS.with(|on| on.set(true));
    let (_dir, engine) = seed_library("motion_exits", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| app.show_undo_toast("Moved to Trash", cx));
    render_ui(vcx);
    click(vcx, "toast-dismiss");
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("toast").is_some(),
        "a dismissed toast fades out where it stood"
    );
    click(vcx, "toast-action");
    view.update(vcx, |app, _| {
        assert!(
            app.toast.current().is_none(),
            "the fading toast's Undo takes no click: {:?}",
            app.toast.current().map(|t| t.message.clone())
        )
    });

    // The palette closes last: while it fades, its layer swallows clicks.
    keystrokes(vcx, "secondary-k");
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| assert!(!app.command_palette_open));
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("command-palette").is_some(),
        "a closed palette stays drawn while it fades out"
    );

    std::thread::sleep(crate::motion::EXIT + std::time::Duration::from_millis(30));
    view.update(vcx, |_, cx| cx.notify());
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("command-palette").is_none(),
        "gone after the fade"
    );
    assert!(vcx.debug_bounds("toast").is_none(), "gone after the fade");
    crate::motion::EXITS.with(|on| on.set(false));
}

#[core::prelude::v1::test]
fn viewer_fades_in_under_reduce_motion_too() {
    for reduce in [true, false] {
        crate::motion::REDUCE_MOTION.with(|on| on.set(reduce));
        let (_dir, engine) = seed_library("motion_viewer", 2);
        let mut cx = crate::tests::TestApp::single();
        let (view, vcx) = app_with_library(&mut cx, engine);
        keystrokes(vcx, "home enter");
        assert!(
            vcx.debug_bounds("viewer-back").is_some(),
            "the viewer lays out while it fades in (reduce motion {reduce})"
        );
        click(vcx, "viewer-back");
        view.update(vcx, |app, _| {
            assert!(
                !app.is_fullscreen_viewer,
                "a fading viewer takes clicks (reduce motion {reduce})"
            )
        });
        keystrokes(vcx, "enter");
        assert!(
            vcx.debug_bounds("viewer-viewport").is_some(),
            "the viewer paints again on reopen (reduce motion {reduce})"
        );
    }
    crate::motion::REDUCE_MOTION.with(|on| on.set(true));
}

#[core::prelude::v1::test]
fn the_grid_resizes_with_the_sliding_sidebar() {
    crate::motion::REDUCE_MOTION.with(|on| on.set(false));
    let (_dir, engine) = seed_library("motion_panels", 4);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    render_ui(vcx);
    let open = view.update(vcx, |app, cx| {
        let open = app.grid_metrics().photo_side;
        app.toggle_sidebar(cx);
        let mid = app.grid_metrics().photo_side;
        assert!(
            (mid - open).abs() < 2.0,
            "the grid starts at the old width and slides: {open} -> {mid}"
        );
        assert!(app.panel_slide.active(), "the sidebar slides shut");
        open
    });
    // A frame mid-slide lays out the clipped panels.
    render_ui(vcx);
    std::thread::sleep(crate::motion::MOTION_SLOW + std::time::Duration::from_millis(20));
    view.update(vcx, |app, _| {
        let settled = app.grid_metrics().photo_side;
        assert!(
            settled > open + 10.0,
            "the grid lands wider once the sidebar has collapsed: {open} -> {settled}"
        );
        assert!(!app.panel_slide.active());
    });

    crate::motion::REDUCE_MOTION.with(|on| on.set(true));
    view.update(vcx, |app, cx| {
        let before = app.grid_metrics().photo_side;
        app.toggle_sidebar(cx);
        assert!(
            app.grid_metrics().photo_side < before - 10.0,
            "under Reduce Motion the panel snaps"
        );
    });
}
