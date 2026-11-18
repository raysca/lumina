//! Viewer zoom: wheel, double click, cursor anchoring and fit.

use crate::tests::helpers::*;
use crate::*;

#[core::prelude::v1::test]
fn ui_automation_viewer_wheel_zoom_is_smooth() {
    let (_temp_dir, engine) = seed_library("wheel_smooth", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let item = view.update(vcx, |app, _| {
        app.get_active_slice(0..1)
            .into_iter()
            .next()
            .expect("seeded library must have an item")
    });
    view.update(vcx, |app, cx| app.open_viewer(item, cx));
    render_ui(vcx);

    let center = vcx
        .debug_bounds("viewer-viewport")
        .expect("the viewer must paint its viewport")
        .center();
    let step = 40.0;
    wheel(
        vcx,
        center,
        gpui::ScrollDelta::Pixels(gpui::point(gpui::px(0.0), gpui::px(step))),
        primary_modifier(),
    );
    let one = view.update(vcx, |app, _| app.viewer_state.zoom_scale);
    assert!(
        (one - 0.1_f32.exp()).abs() < 0.02,
        "40px of ⌘-wheel must zoom smoothly by ~10%, got {one}"
    );
    wheel(
        vcx,
        center,
        gpui::ScrollDelta::Pixels(gpui::point(gpui::px(0.0), gpui::px(step))),
        primary_modifier(),
    );
    let two = view.update(vcx, |app, _| app.viewer_state.zoom_scale);
    assert!(
        (two - one * 0.1_f32.exp()).abs() < 0.02,
        "a second wheel event must zoom by the same ~10% ratio, got {two}"
    );
    assert!(
        (two - 1.25).abs() > 0.02,
        "wheel zoom must not snap to the old 0.25 keyboard step"
    );
}

#[core::prelude::v1::test]
fn test_viewer_cursor_anchored_zoom_action() {
    use crate::views::viewer_state::{ViewerZoomPreset, ZOOM_MAX_SCALE, ZOOM_MIN_SCALE, ZOOM_STEP};

    let mut cx = crate::tests::TestApp::single();
    let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    let sample = get_sample_items();
    let photo = sample[0].clone();
    let video = sample[1].clone();

    view.update(cx, |app, cx| {
        app.grid_state = None;
        app.select_item_at(photo.clone(), 0, cx);
        app.open_viewer(photo.clone(), cx);

        let viewport = (1024.0, 768.0);
        let center = (viewport.0 * 0.5, viewport.1 * 0.5);
        // Wheel cursor right of and above the canvas center.
        let cursor = (center.0 + 188.0, center.1 - 184.0);
        let anchor = (cursor.0 - center.0, cursor.1 - center.1);

        app.zoom_viewer_at(ZOOM_STEP, cursor, viewport, cx);
        assert_eq!(app.viewer_state.zoom_scale, 1.25);
        let (pan_x, pan_y) = app.viewer_state.pan_offset;
        assert!(
            pan_x < 0.0,
            "a cursor right of center must pan content left, got {pan_x}"
        );
        assert!(
            pan_y > 0.0,
            "a cursor above center must pan content down, got {pan_y}"
        );
        assert!(pan_x.is_finite() && pan_y.is_finite());

        // The content point under the cursor stays anchored while zooming in.
        let before = ((anchor.0 - pan_x) / 1.25, (anchor.1 - pan_y) / 1.25);
        app.zoom_viewer_at(ZOOM_STEP, cursor, viewport, cx);
        assert_eq!(app.viewer_state.zoom_scale, 1.5);
        let (pan_x, pan_y) = app.viewer_state.pan_offset;
        let after = ((anchor.0 - pan_x) / 1.5, (anchor.1 - pan_y) / 1.5);
        assert!(
            (after.0 - before.0).abs() < 0.01 && (after.1 - before.1).abs() < 0.01,
            "cursor anchor must survive a second zoom stop: before={before:?} after={after:?}"
        );

        // Zooming out at the same cursor restores the same anchor, then Fit
        // resets pan exactly like the HUD reset and double-click toggle.
        app.zoom_viewer_at(-ZOOM_STEP, cursor, viewport, cx);
        assert_eq!(app.viewer_state.zoom_scale, 1.25);
        let (out_x, out_y) = app.viewer_state.pan_offset;
        let after_out = ((anchor.0 - out_x) / 1.25, (anchor.1 - out_y) / 1.25);
        assert!(
            (after_out.0 - before.0).abs() < 0.01 && (after_out.1 - before.1).abs() < 0.01,
            "zooming out must keep the same anchor: before={before:?} after={after_out:?}"
        );
        app.zoom_viewer_at(-ZOOM_STEP, cursor, viewport, cx);
        assert_eq!(app.viewer_state.zoom_scale, ZOOM_MIN_SCALE);
        assert_eq!(
            app.viewer_state.pan_offset,
            (0.0, 0.0),
            "Fit must reset pan"
        );

        // HUD presets keep their existing center-based behavior.
        app.apply_viewer_zoom_preset(ViewerZoomPreset::FourX, cx);
        assert_eq!(app.viewer_state.zoom_scale, 4.0);
        assert_eq!(app.viewer_state.pan_offset, (0.0, 0.0));
        app.apply_viewer_zoom_preset(ViewerZoomPreset::Fit, cx);
        assert_eq!(app.viewer_state.zoom_scale, ZOOM_MIN_SCALE);

        // A far-off cursor is bounded by the existing pan clamp and stays finite.
        app.zoom_viewer_at(ZOOM_STEP, (9_000.0, -9_000.0), viewport, cx);
        let (fin_x, fin_y) = app.viewer_state.pan_offset;
        let max_pan = (app.viewer_state.zoom_scale - ZOOM_MIN_SCALE) * 800.0;
        assert!(fin_x.is_finite() && fin_y.is_finite());
        assert!(
            fin_x.abs() <= max_pan && fin_y.abs() <= max_pan,
            "wheel zoom must respect the drag pan clamp, got ({fin_x}, {fin_y})"
        );
        app.set_viewer_zoom(ZOOM_MAX_SCALE, cx);
        let pan_at_max = app.viewer_state.pan_offset;
        app.zoom_viewer_at(ZOOM_STEP, cursor, viewport, cx);
        assert_eq!(app.viewer_state.zoom_scale, ZOOM_MAX_SCALE);
        assert_eq!(app.viewer_state.pan_offset, pan_at_max);

        // Missing viewport geometry falls back to center-anchored zoom.
        app.reset_viewer_zoom(cx);
        app.zoom_viewer_at(ZOOM_STEP, cursor, (0.0, 0.0), cx);
        assert_eq!(app.viewer_state.zoom_scale, 1.25);
        assert_eq!(
            app.viewer_state.pan_offset,
            (0.0, 0.0),
            "the geometry fallback must not invent an anchor"
        );

        // Videos ignore wheel zoom so playback controls keep the wheel.
        app.select_item_at(video.clone(), 1, cx);
        app.open_viewer(video.clone(), cx);
        app.zoom_viewer_at(ZOOM_STEP, cursor, viewport, cx);
        assert_eq!(
            app.viewer_state.zoom_scale, ZOOM_MIN_SCALE,
            "videos must ignore cursor-anchored wheel zoom"
        );
    });
    cx.run_until_parked();
}

#[core::prelude::v1::test]
fn test_viewer_zoom_changes_painted_media_bounds() {
    let mut cx = crate::tests::TestApp::single();
    let (_temp_dir, view, vcx) = viewer_with_gradient(&mut cx, "viewer_zoom_paint", 1440.0, 900.0);

    let fit = vcx
        .debug_bounds("viewer-media")
        .expect("the Fit viewer must paint the media element");
    assert!(
        f32::from(fit.size.width) > 0.0 && f32::from(fit.size.height) > 0.0,
        "test premise: the Fit media must have painted bounds, got {fit:?}"
    );

    assert!(
        vcx.debug_bounds("viewer-navigator").is_none(),
        "the navigator only shows while zoomed"
    );
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-2");
    view.update(vcx, |app, _| {
        assert!(app.viewer_state.zoom_scale > 1.0, "⌘2 must zoom in");
    });

    // Navigator: 180px, top right of the stage, amber rect on the visible part.
    let nav = vcx
        .debug_bounds("viewer-navigator")
        .expect("zoomed, the navigator must show");
    let stage = vcx.debug_bounds("viewer-viewport").unwrap();
    assert_eq!(f32::from(nav.size.width), 180.0);
    assert!(
        nav.right() <= stage.right() && stage.right() - nav.right() <= gpui::px(20.0),
        "the navigator sits at the top right: nav={nav:?} stage={stage:?}"
    );
    let region = vcx.debug_bounds("viewer-navigator-region").unwrap();
    let (_, _, w, h) = view.update(vcx, |app, _| app.viewer_state.visible_region().unwrap());
    assert!(
        w < 1.0 && h <= 1.0,
        "zoomed, only part of the picture is visible"
    );
    assert!(
        // Inside the navigator's 1px border.
        (f32::from(region.size.width) - 178.0 * w).abs() < 1.0,
        "the amber rect spans the visible fraction: region={region:?} w={w}"
    );
    assert!(
        nav.contains(&region.origin) && region.right() <= nav.right() + gpui::px(0.5),
        "the region stays inside the navigator"
    );

    let zoomed = vcx
        .debug_bounds("viewer-media")
        .expect("the 2x viewer must paint the media element");
    assert!(
        f32::from(zoomed.size.width) > f32::from(fit.size.width) * 1.5,
        "2x must enlarge the painted media width, not only zoom_scale: fit={fit:?} zoomed={zoomed:?}"
    );
    assert!(
        f32::from(zoomed.size.height) > f32::from(fit.size.height) * 1.5,
        "2x must enlarge the painted media height, not only zoom_scale: fit={fit:?} zoomed={zoomed:?}"
    );

    // With the docked info panel open the navigator keeps its 16px inset
    // from the stage's right edge, which is the panel's left edge.
    view.update(vcx, |app, cx| {
        app.inspector_open = true;
        cx.notify();
    });
    render_viewer_ui(vcx);
    let nav = vcx.debug_bounds("viewer-navigator").unwrap();
    let drawer = vcx
        .debug_bounds("viewer-info-drawer")
        .expect("the info drawer must paint");
    assert_eq!(
        drawer.left() - nav.right(),
        gpui::px(16.0),
        "the navigator sits 16px left of the docked panel: nav={nav:?} drawer={drawer:?}"
    );
}

#[core::prelude::v1::test]
fn test_viewer_double_click_zoom_is_media_bounded() {
    let mut cx = crate::tests::TestApp::single();
    let (_temp_dir, view, vcx) =
        viewer_with_gradient(&mut cx, "viewer_double_click", 1440.0, 900.0);

    let viewport = vcx
        .debug_bounds("viewer-viewport")
        .expect("the viewer must paint its viewport");
    let media = vcx
        .debug_bounds("viewer-media")
        .expect("the viewer must paint the media element");

    // Left edge of the viewport, vertically centred: outside the 800x600 media
    // that sits in the middle of the 1440px canvas.
    let letterbox = gpui::point(viewport.left() + gpui::px(8.0), viewport.center().y);
    // The media element fills the canvas; the visible picture is the
    // contained rect from the captured canvas geometry.
    view.update(vcx, |app, _| {
        assert!(
            !app.viewer_state.hits_media(letterbox),
            "test premise: the letterbox point must be outside the visible media; point={letterbox:?} canvas={:?}",
            app.viewer_state.canvas
        );
    });

    double_click_at(vcx, letterbox);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.viewer_state.zoom_scale, 1.0,
            "a double-click in the letterbox around the media must not zoom"
        );
    });

    double_click_at(vcx, media.center());
    view.update(vcx, |app, _| {
        assert_eq!(
            app.viewer_state.zoom_scale, 2.0,
            "a double-click on the media itself must toggle the zoom"
        );
    });
}

#[core::prelude::v1::test]
fn test_viewer_fills_canvas_and_offers_actual_pixels() {
    let mut cx = crate::tests::TestApp::single();
    let (_temp_dir, view, vcx) = viewer_with_gradient(&mut cx, "viewer_actual", 1440.0, 900.0);
    view.update(vcx, |app, cx| {
        // A 4000x3000 original shown through the 800x600 gradient decode.
        let item = app.selected_item.as_mut().expect("viewer has an item");
        item.width = 4000;
        item.height = 3000;
        cx.notify();
    });
    render_viewer_ui(vcx);

    view.update(vcx, |app, cx| {
        let canvas = app
            .viewer_state
            .canvas
            .expect("painting the viewer must capture canvas geometry");
        assert!(
            canvas.fitted.0 > 800.0 || canvas.fitted.1 > 600.0,
            "Fit must enlarge media to the canvas like Photos, got {:?}",
            canvas.fitted
        );
        assert!(
            (canvas.fitted.0 - canvas.size.0).abs() < 0.5
                || (canvas.fitted.1 - canvas.size.1).abs() < 0.5,
            "Fit must touch the canvas on one axis, got {canvas:?}"
        );

        app.apply_viewer_zoom_preset(crate::views::viewer_state::ViewerZoomPreset::Actual, cx);
        assert!(
            (app.viewer_state.zoom_scale - canvas.actual_scale).abs() < 1e-4,
            "100% must map one source pixel to one screen pixel"
        );
        assert_eq!(app.viewer_state.zoom_label(), "100%");

        app.viewer_state.pan_by(100_000.0, 100_000.0);
        let max_x = (canvas.fitted.0 * app.viewer_state.zoom_scale - canvas.size.0) * 0.5;
        assert!(
            (app.viewer_state.pan_offset.0 - max_x).abs() < 0.5,
            "pan must stop at the media edge, not run off-screen"
        );
    });
}

#[core::prelude::v1::test]
fn test_viewer_fits_a_photo_inside_the_stage_margins() {
    let mut cx = crate::tests::TestApp::single();
    let (_temp_dir, view, vcx) = viewer_with_gradient(&mut cx, "viewer_fit_tall", 1440.0, 900.0);
    // V3Photo fits into at most 1200x760 of the 1440x848 stage, clear of the
    // prev/next buttons, instead of running edge to edge.
    view.update(vcx, |app, cx| {
        app.viewer_image = Some(generate_gradient_image(
            (40, 60, 90),
            (10, 15, 30),
            1600,
            1600,
        ));
        cx.notify();
    });
    render_viewer_ui(vcx);
    render_viewer_ui(vcx);

    let viewport = vcx.debug_bounds("viewer-viewport").unwrap();
    let media = vcx.debug_bounds("viewer-media").unwrap();
    assert!(
        media.top() - viewport.top() >= gpui::px(40.0)
            && viewport.bottom() - media.bottom() >= gpui::px(40.0)
            && media.left() - viewport.left() >= gpui::px(80.0),
        "Fit must leave the design's stage margins: viewport={viewport:?} media={media:?}"
    );
    let (fit_w, fit_h) = view.update(vcx, |app, _| {
        app.viewer_state
            .canvas
            .expect("canvas geometry is captured")
            .fitted
    });
    assert!(
        (fit_w - fit_h).abs() < 1.0 && fit_h <= 761.0,
        "a square photo fits as a square at most 760 tall: {fit_w}x{fit_h}"
    );
}
