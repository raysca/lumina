//! Crop mode with the mouse and the window: the overlay follows a resize in
//! the same frame, it occludes the viewer under it, a handle drag keeps
//! going outside the canvas, the edit bar fits the smallest window, and
//! Done from crop mode never blanks the photo.

use crate::tests::helpers::{app_with_library, click, keystrokes, render_ui};
use crate::tests::quick_edit::edits_of;
use crate::tests::quick_edit_crop::{crop_rect, edit_with_source, near, photo_library};
use crate::*;

/// The frame's contain-fit in the viewer canvas, for a 60×40 photo.
fn fitted(vcx: &mut gpui::VisualTestContext) -> gpui::Bounds<gpui::Pixels> {
    let canvas = vcx
        .debug_bounds("viewer-media")
        .expect("the preview must be rendered");
    let (cw, ch) = (f32::from(canvas.size.width), f32::from(canvas.size.height));
    let aspect = 1.5;
    let (w, h) = if cw / ch > aspect {
        (ch * aspect, ch)
    } else {
        (cw, cw / aspect)
    };
    gpui::Bounds::new(
        gpui::point(
            canvas.origin.x + gpui::px((cw - w) / 2.0),
            canvas.origin.y + gpui::px((ch - h) / 2.0),
        ),
        gpui::size(gpui::px(w), gpui::px(h)),
    )
}

fn close_bounds(a: gpui::Bounds<gpui::Pixels>, b: gpui::Bounds<gpui::Pixels>) -> bool {
    let v = |b: gpui::Bounds<gpui::Pixels>| {
        [
            f32::from(b.origin.x),
            f32::from(b.origin.y),
            f32::from(b.size.width),
            f32::from(b.size.height),
        ]
    };
    v(a).iter().zip(v(b)).all(|(x, y)| (x - y).abs() < 1.0)
}

#[core::prelude::v1::test]
fn crop_overlay_follows_a_window_resize_in_one_frame() {
    let (_dir, engine, ids) = photo_library("crop_resize");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    edit_with_source(&view, vcx, &engine, ids[0]);
    keystrokes(vcx, "c");
    let before = vcx.debug_bounds("crop-overlay").expect("overlay");
    assert!(close_bounds(before, fitted(vcx)), "overlay on the frame");

    vcx.simulate_resize(gpui::size(gpui::px(1000.0), gpui::px(700.0)));
    render_ui(vcx);
    let after = vcx.debug_bounds("crop-overlay").expect("overlay");
    let want = fitted(vcx);
    assert!(
        close_bounds(after, want),
        "after one render the overlay must match the fitted frame: {after:?} vs {want:?}"
    );
    let handle = vcx.debug_bounds("crop-handle-br").expect("handle");
    assert!(
        near(
            f32::from(handle.origin.x + handle.size.width),
            f32::from(want.origin.x + want.size.width),
            1.0
        ),
        "the bottom-right handle sits at the frame's corner"
    );
    // Geometry from an earlier frame must not place the overlay: the paint
    // callback that records it does not ask for another frame.
    view.update(vcx, |app, _| {
        if let Some(canvas) = app.viewer_state.canvas.as_mut() {
            canvas.size = (300.0, 200.0);
            canvas.fitted = (300.0, 200.0);
        }
    });
    render_ui(vcx);
    let stale = vcx.debug_bounds("crop-overlay").expect("overlay");
    assert!(
        close_bounds(stale, fitted(vcx)),
        "the overlay is laid out in the frame it is drawn, got {stale:?}"
    );
}

#[core::prelude::v1::test]
fn crop_overlay_occludes_the_viewer() {
    let (_dir, engine, ids) = photo_library("crop_occlude");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    edit_with_source(&view, vcx, &engine, ids[0]);
    // Square leaves shade at the left and right of the rect.
    keystrokes(vcx, "c c");
    let rect = crop_rect(&view, vcx);
    let frame = vcx.debug_bounds("crop-overlay").expect("overlay");
    let shade = gpui::point(
        frame.origin.x + frame.size.width * (rect.x / 2.0),
        frame.origin.y + frame.size.height / 2.0,
    );
    let none = gpui::Modifiers::none();
    vcx.simulate_mouse_down(shade, gpui::MouseButton::Left, none);
    vcx.simulate_mouse_move(
        gpui::point(shade.x + gpui::px(40.0), shade.y + gpui::px(30.0)),
        Some(gpui::MouseButton::Left),
        none,
    );
    vcx.simulate_mouse_up(shade, gpui::MouseButton::Left, none);
    render_ui(vcx);
    vcx.simulate_event(gpui::MouseDownEvent {
        position: shade,
        modifiers: none,
        button: gpui::MouseButton::Left,
        click_count: 2,
        first_mouse: false,
    });
    vcx.simulate_event(gpui::MouseUpEvent {
        position: shade,
        modifiers: none,
        button: gpui::MouseButton::Left,
        click_count: 2,
    });
    vcx.simulate_mouse_down(shade, gpui::MouseButton::Right, none);
    vcx.simulate_mouse_up(shade, gpui::MouseButton::Right, none);
    crate::tests::helpers::wheel(
        vcx,
        shade,
        gpui::ScrollDelta::Pixels(gpui::point(gpui::px(0.0), gpui::px(-200.0))),
        crate::tests::helpers::primary_modifier(),
    );
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.viewer_state.zoom_scale, 1.0,
            "no zoom under the overlay"
        );
        assert_eq!(app.viewer_state.pan_offset, (0.0, 0.0), "no pan under it");
        assert!(app.context_menu_state.is_none(), "no context menu under it");
        assert!(app.edits.session.is_some(), "still editing");
    });
    assert_eq!(
        rect,
        crop_rect(&view, vcx),
        "the shade does not move the crop"
    );
}

#[core::prelude::v1::test]
fn crop_drag_continues_outside_the_canvas() {
    let (_dir, engine, ids) = photo_library("crop_capture");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    edit_with_source(&view, vcx, &engine, ids[0]);
    keystrokes(vcx, "c");
    click(vcx, "edit-aspect-free");
    keystrokes(
        vcx,
        "alt-shift-left alt-shift-left alt-shift-up alt-shift-up",
    );
    let start = vcx.debug_bounds("crop-handle-br").expect("handle").center();
    let window = vcx.debug_bounds("viewer-viewport").expect("viewport");
    // The viewport's bottom-right padding is outside the media canvas.
    let outside = gpui::point(
        window.origin.x + window.size.width - gpui::px(4.0),
        window.origin.y + window.size.height - gpui::px(4.0),
    );
    let none = gpui::Modifiers::none();
    vcx.simulate_mouse_down(start, gpui::MouseButton::Left, none);
    render_ui(vcx);
    vcx.simulate_mouse_move(outside, Some(gpui::MouseButton::Left), none);
    render_ui(vcx);
    let rect = crop_rect(&view, vcx);
    assert!(
        near(rect.w, 1.0, 1e-3) && near(rect.h, 1.0, 1e-3),
        "a drag past the canvas still moves the corner, to the frame edge: {rect:?}"
    );
    vcx.simulate_mouse_up(outside, gpui::MouseButton::Left, none);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        let crop = app.edits.session.as_ref().unwrap().crop.unwrap();
        assert!(crop.dragging.is_none(), "mouse up outside ends the drag");
    });
}

#[core::prelude::v1::test]
fn edit_bar_fits_the_smallest_window_while_cropping() {
    let (_dir, engine, ids) = photo_library("crop_narrow");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    view.update(vcx, |app, _| app.inspector_open = true);
    vcx.simulate_resize(gpui::size(gpui::px(960.0), gpui::px(600.0)));
    edit_with_source(&view, vcx, &engine, ids[0]);
    keystrokes(vcx, "c");
    render_ui(vcx);
    let mut selectors: Vec<String> = ["edit-cancel", "edit-done", "edit-revert", "edit-crop"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    selectors.extend(
        crate::app::crop::CropAspect::ALL
            .iter()
            .map(|a| format!("edit-aspect-{}", a.name())),
    );
    let bounds: Vec<_> = selectors
        .iter()
        .map(|selector| {
            let b = vcx
                .debug_bounds(crate::tests::helpers::leak(selector.clone()))
                .unwrap_or_else(|| panic!("{selector} must be rendered"));
            let (left, right) = (f32::from(b.origin.x), f32::from(b.origin.x + b.size.width));
            assert!(
                left >= 0.0 && right <= 960.0,
                "{selector} must be inside a 960 pt window, got {left}..{right}"
            );
            b
        })
        .collect();
    for (i, a) in bounds.iter().enumerate() {
        for b in &bounds[i + 1..] {
            assert!(!a.intersects(b), "edit bar buttons overlap: {a:?} {b:?}");
        }
    }
}

#[core::prelude::v1::test]
fn done_from_crop_keeps_a_frame_on_screen() {
    let (_dir, engine, ids) = photo_library("crop_done_frame");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    edit_with_source(&view, vcx, &engine, ids[0]);
    keystrokes(vcx, "c c");
    view.update(vcx, |app, cx| {
        app.finish_edit(cx);
        assert!(app.edits.session.is_none(), "Done saved");
        assert!(
            app.viewer_image.is_some(),
            "Done from crop mode must not blank the photo to the placeholder"
        );
    });
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        let size = app.viewer_image.as_ref().expect("a frame").size(0);
        assert_eq!(
            size.width.0, size.height.0,
            "the uncropped preview is replaced by the Square crop"
        );
    });
    assert!(
        edits_of(&engine, ids[0]).crop.is_some(),
        "the crop is saved"
    );
}
