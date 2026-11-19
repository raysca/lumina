//! Entrance motion: the touched mark pops, fresh thumbnails fade, and
//! fading overlays stay fully usable while they animate.

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
