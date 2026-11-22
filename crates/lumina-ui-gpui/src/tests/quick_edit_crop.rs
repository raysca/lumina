//! Crop mode inside edit mode (C): aspect presets by keyboard, the stored
//! rect in unrotated coordinates, handle drags at rendered bounds, arrows
//! held inside the frame, and Esc leaving crop without leaving edit mode.

use crate::tests::helpers::{app_with_library, click, keystrokes, render_ui};
use crate::tests::quick_edit::{edits_of, open_viewer_on, scanned, temp_library};
use crate::*;
use lumina_core::{CropRect, LuminaEngine, MediaId};
use std::sync::Arc;

/// Two real 60×40 photos, the top half red and the bottom half blue;
/// returns the library, its engine and their ids.
pub(crate) fn photo_library(
    prefix: &str,
) -> (
    crate::tests::helpers::TempLibrary,
    Arc<LuminaEngine>,
    Vec<MediaId>,
) {
    let dir = temp_library(prefix);
    for name in ["a.jpg", "b.jpg"] {
        image::RgbImage::from_fn(60, 40, |_, y| {
            image::Rgb(if y < 20 { [255, 0, 0] } else { [0, 0, 255] })
        })
        .save(dir.join(name))
        .expect("fixture photo must be written");
    }
    let engine = scanned(&dir);
    let ids = engine.get_item_slice(0..2).iter().map(|i| i.id).collect();
    (dir, engine, ids)
}

/// Opens edit mode on `id` and hands it the unedited source the app asks
/// for, so the test does not wait on the decoder pool.
pub(crate) fn edit_with_source(
    view: &gpui::Entity<LuminaApp>,
    vcx: &mut gpui::VisualTestContext,
    engine: &LuminaEngine,
    id: MediaId,
) {
    open_viewer_on(view, vcx, engine, id);
    keystrokes(vcx, "e");
    let (tx, rx) = std::sync::mpsc::channel();
    engine.request_unedited_frame(id, 64, move |frame| {
        tx.send(frame).expect("test receiver is alive")
    });
    let source = rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("decode must answer");
    assert!(source.is_some(), "the fixture photo must decode");
    view.update(vcx, |app, cx| app.land_edit_source(id, source, cx));
    render_ui(vcx);
}

pub(crate) fn crop_rect(
    view: &gpui::Entity<LuminaApp>,
    vcx: &mut gpui::VisualTestContext,
) -> CropRect {
    view.update(vcx, |app, _| {
        app.edits
            .session
            .as_ref()
            .and_then(|s| s.crop)
            .expect("crop mode must be open")
            .rect
    })
}

fn saved_crop(engine: &LuminaEngine, id: MediaId) -> CropRect {
    edits_of(engine, id).crop.expect("a crop must be saved")
}

pub(crate) fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

#[core::prelude::v1::test]
fn keyboard_only_crop_persists() {
    let (_dir, engine, ids) = photo_library("crop_keys");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    edit_with_source(&view, vcx, &engine, ids[0]);

    // C enters crop at Original, the next C is Square; Enter is Done.
    keystrokes(vcx, "c c");
    assert!(
        vcx.debug_bounds("crop-overlay").is_some(),
        "crop mode draws the overlay"
    );
    keystrokes(vcx, "enter");

    let crop = saved_crop(&engine, ids[0]);
    assert!(
        near(crop.w * 60.0, crop.h * 40.0, 0.5),
        "Square is square in pixels, got {crop:?}"
    );
    assert!(near(crop.h, 1.0, 1e-4), "the largest square is full height");
    assert!(near(crop.x, 1.0 / 6.0, 1e-3), "the square is centred");
    view.update(vcx, |app, _| {
        assert!(app.edits.session.is_none(), "Enter is Done while cropping");
        assert!(app.is_fullscreen_viewer, "Done keeps the viewer open");
        assert_eq!(app.undo.undo_stack.len(), 1, "the crop is one undo step");
    });

    // The saved frame the viewer asks for is the cropped, square one.
    let (tx, rx) = std::sync::mpsc::channel();
    engine.request_thumbnail(ids[0], 64, move |frame| {
        tx.send(frame).expect("test receiver is alive")
    });
    let frame = rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("decode must answer")
        .expect("the photo must decode");
    let id = ids[0];
    view.update(vcx, |app, cx| {
        app.handle_ui_message(
            crate::app::UiMessage::ViewerLoaded {
                id,
                generation: app.viewer_frame.generation,
                frame,
            },
            cx,
        )
    });
    view.update(vcx, |app, _| {
        let size = app.viewer_image.as_ref().expect("viewer frame").size(0);
        assert_eq!(
            size.width.0, size.height.0,
            "the viewer frame of a Square crop is square"
        );
    });

    keystrokes(vcx, "cmd-z");
    assert_eq!(
        edits_of(&engine, ids[0]).crop,
        None,
        "undo removes the crop"
    );
}

#[core::prelude::v1::test]
fn crop_on_rotated_photo_stores_unrotated_coordinates() {
    let (_dir, engine, ids) = photo_library("crop_rotated");
    engine.set_rotations(&[(ids[0], 1)]).expect("seed rotation");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    edit_with_source(&view, vcx, &engine, ids[0]);

    // Crop (the whole frame), Free, then ⌥⇧← five times: the shown left
    // half.
    keystrokes(vcx, "c");
    click(vcx, "edit-aspect-free");
    keystrokes(
        vcx,
        "alt-shift-left alt-shift-left alt-shift-left alt-shift-left alt-shift-left",
    );
    let shown = crop_rect(&view, vcx);
    assert!(
        near(shown.x, 0.0, 1e-4) && near(shown.w, 0.5, 1e-4) && near(shown.h, 1.0, 1e-4),
        "the shown rect is the left half, got {shown:?}"
    );
    keystrokes(vcx, "enter");

    let saved = edits_of(&engine, ids[0]);
    assert_eq!(saved.rotation, 1, "the rotation is unchanged");
    let crop = saved.crop.expect("a crop must be saved");
    // A clockwise turn puts the unrotated bottom edge on the left.
    assert!(
        near(crop.x, 0.0, 1e-4)
            && near(crop.y, 0.5, 1e-4)
            && near(crop.w, 1.0, 1e-4)
            && near(crop.h, 0.5, 1e-4),
        "the shown left half of a clockwise turn is the unrotated bottom half, got {crop:?}"
    );
    // End to end in pixels: the saved frame is the blue (bottom) half only.
    let (tx, rx) = std::sync::mpsc::channel();
    engine.request_thumbnail(ids[0], 64, move |frame| {
        tx.send(frame).expect("test receiver is alive")
    });
    let frame = rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("decode must answer")
        .expect("the photo must decode");
    assert_eq!(
        (frame.width, frame.height),
        (20, 60),
        "half the frame, turned"
    );
    for (x, y) in [(2, 2), (10, 30), (17, 57)] {
        let at = ((y * frame.width + x) * 4) as usize;
        let px = &frame.data[at..at + 4];
        let (r, b) = match frame.format {
            lumina_core::PixelFormat::Bgra8 => (px[2], px[0]),
            _ => (px[0], px[2]),
        };
        assert!(
            b > 200 && r < 60,
            "pixel ({x}, {y}) of the saved crop must be blue, got r {r} b {b}"
        );
    }
}

#[core::prelude::v1::test]
fn crop_handle_drag_resizes() {
    let (_dir, engine, ids) = photo_library("crop_drag");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    edit_with_source(&view, vcx, &engine, ids[0]);
    keystrokes(vcx, "c");
    // C enters with the whole frame; Free lets the corner move on its own.
    click(vcx, "edit-aspect-free");

    let frame = vcx
        .debug_bounds("crop-overlay")
        .expect("the overlay must be rendered");
    let handle = vcx
        .debug_bounds("crop-handle-br")
        .expect("the bottom-right handle must be rendered");
    let start = handle.center();
    let end = gpui::point(
        start.x - frame.size.width / 2.0,
        start.y - frame.size.height / 2.0,
    );
    vcx.simulate_mouse_down(start, gpui::MouseButton::Left, gpui::Modifiers::none());
    render_ui(vcx);
    vcx.simulate_mouse_move(end, Some(gpui::MouseButton::Left), gpui::Modifiers::none());
    render_ui(vcx);
    vcx.simulate_mouse_up(end, gpui::MouseButton::Left, gpui::Modifiers::none());
    render_ui(vcx);

    let rect = crop_rect(&view, vcx);
    assert!(
        near(rect.x, 0.0, 1e-4) && near(rect.y, 0.0, 1e-4),
        "the opposite corner stays put, got {rect:?}"
    );
    assert!(
        near(rect.w, 0.5, 0.01) && near(rect.h, 0.5, 0.01),
        "dragging the corner by half the frame halves the rect, got {rect:?}"
    );
    view.update(vcx, |app, _| {
        assert!(
            app.edits
                .session
                .as_ref()
                .and_then(|s| s.crop)
                .unwrap()
                .dragging
                .is_none(),
            "mouse up ends the drag"
        );
        assert_eq!(
            app.viewer_state.zoom_scale, 1.0,
            "a crop drag never zooms or pans"
        );
    });
    keystrokes(vcx, "enter");
    let crop = saved_crop(&engine, ids[0]);
    assert!(
        near(crop.w, 0.5, 0.01) && near(crop.h, 0.5, 0.01),
        "{crop:?}"
    );
}

#[core::prelude::v1::test]
fn crop_arrow_keys_stay_inside_frame() {
    let (_dir, engine, ids) = photo_library("crop_arrows");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    edit_with_source(&view, vcx, &engine, ids[0]);
    keystrokes(vcx, "c c");

    keystrokes(
        vcx,
        "shift-right shift-right shift-right shift-right shift-right",
    );
    let moved = crop_rect(&view, vcx);
    assert!(
        near(moved.x + moved.w, 1.0, 1e-4) && moved.x + moved.w <= 1.0,
        "⇧→ stops at the right edge, got {moved:?}"
    );
    keystrokes(vcx, "left");
    let back = crop_rect(&view, vcx);
    assert!(near(moved.x - back.x, 0.01, 1e-4), "← moves 1 %");
    keystrokes(vcx, "up down up");
    assert_eq!(
        back.y,
        crop_rect(&view, vcx).y,
        "a full-height rect cannot move up or down"
    );
    // ⌥→ grows, held to Square and to the frame.
    keystrokes(vcx, "alt-shift-right alt-shift-right alt-shift-right");
    let grown = crop_rect(&view, vcx);
    assert!(
        grown.x + grown.w <= 1.0 && grown.y + grown.h <= 1.0,
        "{grown:?}"
    );
    assert!(near(grown.w * 60.0, grown.h * 40.0, 0.5), "still square");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_item.as_ref().map(|i| i.id),
            Some(ids[0]),
            "arrows in crop mode must not step to another photo"
        );
    });

    keystrokes(vcx, "enter");
    let crop = saved_crop(&engine, ids[0]);
    assert!(crop.validate().is_ok(), "the saved crop is valid: {crop:?}");
}

#[core::prelude::v1::test]
fn escape_leaves_crop_but_not_edit_mode() {
    let (_dir, engine, ids) = photo_library("crop_escape");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    edit_with_source(&view, vcx, &engine, ids[0]);
    keystrokes(vcx, "= c c");
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        let session = app
            .edits
            .session
            .as_ref()
            .expect("Esc in crop keeps edit mode");
        assert!(session.crop.is_none(), "Esc leaves crop mode");
        assert_eq!(session.draft.crop, None, "Esc drops the unapplied crop");
    });
    assert!(
        vcx.debug_bounds("crop-overlay").is_none(),
        "the overlay goes with crop mode"
    );
    keystrokes(vcx, "enter");
    let saved = edits_of(&engine, ids[0]);
    assert_eq!(saved.crop, None, "no crop is saved after Esc");
    assert!(
        near(saved.exposure_ev, 1.0 / 3.0, 1e-3),
        "other edits are kept"
    );
}

#[core::prelude::v1::test]
fn crop_mode_previews_the_whole_turned_frame() {
    let (_dir, engine, ids) = photo_library("crop_preview");
    let saved = lumina_core::Edits {
        rotation: 1,
        crop: Some(CropRect {
            x: 0.0,
            y: 0.0,
            w: 0.5,
            h: 0.5,
        }),
        exposure_ev: 0.0,
        auto_enhance: false,
    };
    engine.set_edits(&[(ids[0], saved)]).expect("seed edits");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    edit_with_source(&view, vcx, &engine, ids[0]);

    let shown = |view: &gpui::Entity<LuminaApp>, vcx: &mut gpui::VisualTestContext| {
        view.update(vcx, |app, _| {
            let size = app.viewer_image.as_ref().expect("preview").size(0);
            (size.width.0, size.height.0)
        })
    };
    keystrokes(vcx, "= c");
    vcx.run_until_parked();
    assert_eq!(
        shown(&view, vcx),
        (40, 60),
        "while cropping the preview is the whole frame, turned"
    );
    view.update(vcx, |app, _| {
        let session = app.edits.session.as_ref().expect("editing");
        let preview = session.shown.expect("a preview was rendered");
        assert_eq!(preview.crop, None, "the crop-mode preview is uncropped");
        assert!(preview.exposure_ev > 0.0, "and keeps the exposure change");
    });
    // The saved crop is drawn in shown coordinates: the unrotated top-left
    // quarter is the shown top-right quarter after a clockwise turn.
    let rect = crop_rect(&view, vcx);
    assert!(
        near(rect.x, 0.5, 1e-4) && near(rect.y, 0.0, 1e-4),
        "{rect:?}"
    );
    keystrokes(vcx, "escape");
    vcx.run_until_parked();
    assert_eq!(
        shown(&view, vcx),
        (20, 30),
        "leaving crop renders the cropped preview again"
    );
}

#[core::prelude::v1::test]
fn rotating_while_cropping_keeps_the_region() {
    let (_dir, engine, ids) = photo_library("crop_rotate");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    edit_with_source(&view, vcx, &engine, ids[0]);
    keystrokes(vcx, "c c shift-right shift-right");
    let before = crop_rect(&view, vcx);
    keystrokes(vcx, "r");
    let after = crop_rect(&view, vcx);
    // Shown 60×40 becomes 40×60: the square follows the picture.
    assert!(
        near(after.w * 40.0, after.h * 60.0, 0.5),
        "the rect stays square in pixels after a turn, got {after:?}"
    );
    assert!(
        near(after.y, before.x, 1e-4),
        "a clockwise turn moves the rect's left edge to its top"
    );
    keystrokes(vcx, "enter");
    let saved = edits_of(&engine, ids[0]);
    assert_eq!(saved.rotation, 1, "R in crop mode turns the draft");
    let crop = saved.crop.expect("a crop must be saved");
    assert!(
        near(crop.x, before.x, 1e-4) && near(crop.w, before.w, 1e-4),
        "the stored (unrotated) crop is the region chosen before the turn: {crop:?} vs {before:?}"
    );
}

#[core::prelude::v1::test]
fn shift_c_applies_the_crop_and_keeps_editing() {
    let (_dir, engine, ids) = photo_library("crop_shift_c");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    edit_with_source(&view, vcx, &engine, ids[0]);
    keystrokes(vcx, "shift-c");
    view.update(vcx, |app, _| {
        assert!(
            app.edits.session.as_ref().unwrap().crop.is_some(),
            "⇧C enters crop mode"
        );
    });
    keystrokes(vcx, "c shift-c");
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        let session = app.edits.session.as_ref().expect("⇧C keeps edit mode");
        assert!(session.crop.is_none(), "⇧C leaves crop mode");
        let crop = session.draft.crop.expect("⇧C keeps the crop in the draft");
        assert!(near(crop.w * 60.0, crop.h * 40.0, 0.5), "the Square crop");
        let size = app.viewer_image.as_ref().expect("preview").size(0);
        assert_eq!(size.width.0, size.height.0, "the preview is cropped again");
    });
    assert_eq!(edits_of(&engine, ids[0]).crop, None, "nothing is saved yet");
}
