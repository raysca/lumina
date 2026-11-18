//! Viewer pinch zoom and scroll panning.

use crate::tests::helpers::*;
use crate::*;

/// Sends a trackpad pinch at `position` after moving the pointer there, since
/// `on_pinch` only fires on the hovered element.
fn pinch(cx: &mut gpui::VisualTestContext, position: gpui::Point<gpui::Pixels>, delta: f32) {
    cx.simulate_mouse_move(position, None, gpui::Modifiers::default());
    cx.simulate_event(gpui::PinchEvent {
        position,
        delta,
        modifiers: gpui::Modifiers::default(),
        phase: gpui::TouchPhase::Moved,
    });
    render_viewer_ui(cx);
}

fn scroll_px(dx: f32, dy: f32) -> gpui::ScrollDelta {
    gpui::ScrollDelta::Pixels(gpui::point(gpui::px(dx), gpui::px(dy)))
}

/// The content point (in unzoomed canvas units) under window point `p`.
fn content_under(app: &LuminaApp, p: gpui::Point<gpui::Pixels>) -> (f32, f32) {
    let c = app.viewer_state.canvas.expect("painted canvas geometry");
    let s = app.viewer_state.zoom_scale;
    let (px, py) = app.viewer_state.pan_offset;
    (
        (f32::from(p.x) - c.center.0 - px) / s,
        (f32::from(p.y) - c.center.1 - py) / s,
    )
}

/// Zooms to 2x first, so neither axis is pinned by the pan clamp (at small
/// zoom the media is narrower than the canvas and x pan clamps to 0).
fn zoom_to_two_x(view: &gpui::Entity<LuminaApp>, vcx: &mut gpui::VisualTestContext) {
    view.update(vcx, |app, cx| {
        app.apply_viewer_zoom_preset(crate::views::viewer_state::ViewerZoomPreset::TwoX, cx)
    });
    render_viewer_ui(vcx);
}

/// An off-centre point inside the Fit media (about 1013x760 in a 1440x900
/// window) and the canvas, whatever the zoom.
fn off_centre_media_point(vcx: &mut gpui::VisualTestContext) -> gpui::Point<gpui::Pixels> {
    let c = vcx
        .debug_bounds("viewer-viewport")
        .expect("the viewer must paint its viewport")
        .center();
    gpui::point(c.x + gpui::px(200.0), c.y - gpui::px(120.0))
}

#[core::prelude::v1::test]
fn viewer_pinch_keeps_content_under_pinch_center() {
    let mut cx = crate::tests::TestApp::single();
    let (_temp_dir, view, vcx) =
        viewer_with_gradient(&mut cx, "viewer_pinch_anchor", 1440.0, 900.0);
    zoom_to_two_x(&view, vcx);
    let at = off_centre_media_point(vcx);
    let (scale, before) = view.update(vcx, |app, _| {
        (app.viewer_state.zoom_scale, content_under(app, at))
    });

    pinch(vcx, at, 0.2);
    view.update(vcx, |app, _| {
        assert!(
            (app.viewer_state.zoom_scale - scale * 1.2).abs() < 1e-4,
            "a 0.2 pinch must zoom by 20%, got {}",
            app.viewer_state.zoom_scale
        );
        let after = content_under(app, at);
        assert!(
            (after.0 - before.0).abs() < 0.5 && (after.1 - before.1).abs() < 0.5,
            "the content under the pinch centre must stay put: {before:?} -> {after:?}"
        );
    });
}

#[core::prelude::v1::test]
fn viewer_pinch_ignored_for_video() {
    let mut cx = crate::tests::TestApp::single();
    let (_temp_dir, view, vcx) = viewer_with_gradient(&mut cx, "viewer_pinch_video", 1440.0, 900.0);
    view.update(vcx, |app, cx| {
        app.selected_item
            .as_mut()
            .expect("viewer has an item")
            .media_type = lumina_core::MediaType::Video;
        cx.notify();
    });
    render_viewer_ui(vcx);
    let at = off_centre_media_point(vcx);

    pinch(vcx, at, 0.3);
    crate::tests::helpers::wheel(vcx, at, scroll_px(40.0, 60.0), gpui::Modifiers::default());
    crate::tests::helpers::wheel(
        vcx,
        at,
        scroll_px(0.0, 60.0),
        crate::tests::helpers::primary_modifier(),
    );
    view.update(vcx, |app, _| {
        assert_eq!(
            app.viewer_state.zoom_scale,
            crate::views::viewer_state::ZOOM_MIN_SCALE,
            "pinch and scroll must never zoom a video"
        );
        assert_eq!(
            app.viewer_state.pan_offset,
            (0.0, 0.0),
            "a video never pans"
        );
    });
}

#[core::prelude::v1::test]
fn pinch_out_to_fit_recenters() {
    let mut cx = crate::tests::TestApp::single();
    let (_temp_dir, view, vcx) = viewer_with_gradient(&mut cx, "viewer_pinch_fit", 1440.0, 900.0);
    let at = off_centre_media_point(vcx);

    pinch(vcx, at, 0.5);
    pinch(vcx, at, 0.5);
    crate::tests::helpers::wheel(vcx, at, scroll_px(25.0, -15.0), gpui::Modifiers::default());
    view.update(vcx, |app, _| {
        assert!(
            app.viewer_state.zoom_scale > 2.0,
            "two pinches must zoom in"
        );
        assert_ne!(
            app.viewer_state.pan_offset,
            (0.0, 0.0),
            "test premise: panned"
        );
    });
    for _ in 0..6 {
        pinch(vcx, at, -0.5);
    }
    view.update(vcx, |app, _| {
        assert_eq!(
            app.viewer_state.zoom_scale,
            crate::views::viewer_state::ZOOM_MIN_SCALE,
            "pinching out past Fit must stop at Fit"
        );
        assert_eq!(app.viewer_state.pan_offset, (0.0, 0.0), "Fit must recenter");
    });
}

#[core::prelude::v1::test]
fn viewer_scroll_pans_when_zoomed_and_clamps_like_drag() {
    let mut cx = crate::tests::TestApp::single();
    let (_temp_dir, view, vcx) = viewer_with_gradient(&mut cx, "viewer_scroll_pan", 1440.0, 900.0);
    zoom_to_two_x(&view, vcx);
    let at = off_centre_media_point(vcx);
    let scale = view.update(vcx, |app, _| app.viewer_state.zoom_scale);

    // Trackpad: pixel deltas pan one to one.
    crate::tests::helpers::wheel(vcx, at, scroll_px(30.0, -20.0), gpui::Modifiers::default());
    view.update(vcx, |app, _| {
        assert_eq!(
            app.viewer_state.zoom_scale, scale,
            "a plain scroll must not zoom"
        );
        assert_eq!(
            app.viewer_state.pan_offset,
            (30.0, -20.0),
            "a pixel scroll must pan by its delta"
        );
    });

    // Mouse wheel: line deltas pan by the window line height per line.
    let line = vcx.update(|window, _| f32::from(window.line_height()));
    crate::tests::helpers::wheel(
        vcx,
        at,
        gpui::ScrollDelta::Lines(gpui::point(0.0, 1.0)),
        gpui::Modifiers::default(),
    );
    view.update(vcx, |app, _| {
        assert!(
            (app.viewer_state.pan_offset.1 - (-20.0 + line)).abs() < 1e-3,
            "one wheel line must pan by one line height, got {:?}",
            app.viewer_state.pan_offset
        );
    });

    // A huge scroll stops at the media edge, the same clamp as drag-pan.
    crate::tests::helpers::wheel(
        vcx,
        at,
        scroll_px(100_000.0, 100_000.0),
        gpui::Modifiers::default(),
    );
    view.update(vcx, |app, _| {
        let c = app.viewer_state.canvas.expect("painted canvas geometry");
        let max_x = ((c.fitted.0 * scale - c.size.0) * 0.5).max(0.0);
        let max_y = ((c.fitted.1 * scale - c.size.1) * 0.5).max(0.0);
        let (x, y) = app.viewer_state.pan_offset;
        assert!(
            (x - max_x).abs() < 0.5 && (y - max_y).abs() < 0.5,
            "scroll-pan must stop at the media edge ({max_x}, {max_y}), got ({x}, {y})"
        );
    });
}

#[core::prelude::v1::test]
fn viewer_scroll_at_fit_does_nothing() {
    let mut cx = crate::tests::TestApp::single();
    let (_temp_dir, view, vcx) = viewer_with_gradient(&mut cx, "viewer_scroll_fit", 1440.0, 900.0);
    let at = off_centre_media_point(vcx);
    crate::tests::helpers::wheel(vcx, at, scroll_px(30.0, 80.0), gpui::Modifiers::default());
    crate::tests::helpers::wheel(
        vcx,
        at,
        gpui::ScrollDelta::Lines(gpui::point(0.0, 3.0)),
        gpui::Modifiers::default(),
    );
    view.update(vcx, |app, _| {
        assert_eq!(
            app.viewer_state.zoom_scale,
            crate::views::viewer_state::ZOOM_MIN_SCALE,
            "a plain scroll at Fit must not zoom"
        );
        assert_eq!(app.viewer_state.pan_offset, (0.0, 0.0), "Fit must not pan");
    });
}

#[core::prelude::v1::test]
fn viewer_cmd_scroll_zooms_anchored_at_pointer() {
    let mut cx = crate::tests::TestApp::single();
    let (_temp_dir, view, vcx) = viewer_with_gradient(&mut cx, "viewer_cmd_scroll", 1440.0, 900.0);
    zoom_to_two_x(&view, vcx);
    let at = off_centre_media_point(vcx);
    let (scale, before) = view.update(vcx, |app, _| {
        (app.viewer_state.zoom_scale, content_under(app, at))
    });

    // A mouse-wheel notch with ⌘ zooms (one line is ~13%).
    crate::tests::helpers::wheel(
        vcx,
        at,
        gpui::ScrollDelta::Lines(gpui::point(0.0, 1.0)),
        crate::tests::helpers::primary_modifier(),
    );
    view.update(vcx, |app, _| {
        assert!(
            app.viewer_state.zoom_scale > scale * 1.1,
            "⌘ + wheel must zoom in, got {}",
            app.viewer_state.zoom_scale
        );
        let after = content_under(app, at);
        assert!(
            (after.0 - before.0).abs() < 0.5 && (after.1 - before.1).abs() < 0.5,
            "⌘-scroll zoom must keep the content under the pointer: {before:?} -> {after:?}"
        );
    });
}
