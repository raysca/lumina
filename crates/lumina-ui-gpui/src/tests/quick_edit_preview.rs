//! Edit mode's live preview: rendered from the unedited frame off the UI
//! thread, newest result only, coalesced while a key is held, and a note
//! when no preview can be made.

use crate::tests::helpers::{app_with_library, keystrokes};
use crate::tests::quick_edit::{edit_library, open_viewer_on, scanned, temp_library};
use crate::*;
use lumina_core::{LuminaEngine, MediaId};
use std::sync::Arc;

#[core::prelude::v1::test]
fn stale_preview_is_dropped() {
    let (_dir, engine, photos, _) = edit_library("stale_preview");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_viewer_on(&view, vcx, &engine, photos[0]);
    keystrokes(vcx, "e");

    let image = || {
        Arc::new(gpui::RenderImage::new(vec![image::Frame::new(
            image::RgbaImage::new(2, 2),
        )]))
    };
    view.update(vcx, |app, cx| {
        let generation = {
            let session = app.edits.session.as_mut().expect("editing");
            session.generation = 5;
            session.generation
        };
        let stale = image();
        app.land_edit_preview(photos[0], generation - 1, Some(stale.clone()), cx);
        assert!(
            !app.viewer_image
                .as_ref()
                .is_some_and(|shown| shown.id == stale.id),
            "an older preview must not replace a newer one"
        );
        let fresh = image();
        app.land_edit_preview(photos[0], generation, Some(fresh.clone()), cx);
        assert!(
            app.viewer_image
                .as_ref()
                .is_some_and(|shown| shown.id == fresh.id),
            "the newest preview is shown"
        );
        // A saved-frame decode still in flight must not replace it.
        app.handle_ui_message(
            crate::app::UiMessage::ViewerLoaded {
                id: photos[0],
                frame: lumina_core::DecodedFrame {
                    id: photos[0],
                    width: 1,
                    height: 1,
                    format: lumina_core::PixelFormat::Rgba8,
                    data: vec![0; 4],
                },
            },
            cx,
        );
        assert!(
            app.viewer_image
                .as_ref()
                .is_some_and(|shown| shown.id == fresh.id),
            "a late viewer frame must not replace the edit preview"
        );
    });
}

#[core::prelude::v1::test]
fn preview_renders_the_draft_from_the_unedited_frame() {
    // A real 8×4 photo, saved already turned once: the preview must start
    // from the unturned frame, not the viewer's.
    let dir = temp_library("preview");
    image::RgbImage::from_fn(8, 4, |x, _| image::Rgb([x as u8 * 30, 90, 160]))
        .save(dir.join("wide.jpg"))
        .expect("fixture photo must be written");
    let engine = scanned(&dir);
    let id = engine.get_item_slice(0..1)[0].id;
    engine.set_rotations(&[(id, 1)]).expect("seed rotation");

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_viewer_on(&view, vcx, &engine, id);
    keystrokes(vcx, "e r");

    // The same unedited decode edit mode asks for, delivered by hand so
    // the test does not wait on the decoder pool.
    let (tx, rx) = std::sync::mpsc::channel();
    engine.request_unedited_frame(id, 64, move |frame| {
        tx.send(frame).expect("test receiver is alive")
    });
    let source = rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("decode must answer")
        .expect("the photo must decode");
    assert_eq!(
        (source.width, source.height),
        (8, 4),
        "the source is unturned"
    );
    view.update(vcx, |app, cx| app.land_edit_source(id, Some(source), cx));
    vcx.run_until_parked();

    view.update(vcx, |app, _| {
        let shown = app.viewer_image.as_ref().expect("the preview is shown");
        let size = shown.size(0);
        assert_eq!(
            (size.width.0, size.height.0),
            (8, 4),
            "draft rotation 2 (saved 1 + R) shows the frame upside down, wide"
        );
    });
    keystrokes(vcx, "r");
    view.update(vcx, |app, _| {
        let size = app.viewer_image.as_ref().expect("preview").size(0);
        assert_eq!(
            (size.width.0, size.height.0),
            (4, 8),
            "each draft change renders a new preview"
        );
    });
}

/// One real 8×4 photo; returns the library, its engine and the photo id.
fn wide_library(
    prefix: &str,
) -> (
    crate::tests::helpers::TempLibrary,
    Arc<LuminaEngine>,
    MediaId,
) {
    let dir = temp_library(prefix);
    image::RgbImage::from_fn(8, 4, |x, _| image::Rgb([x as u8 * 30, 90, 160]))
        .save(dir.join("wide.jpg"))
        .expect("fixture photo must be written");
    let engine = scanned(&dir);
    let id = engine.get_item_slice(0..1)[0].id;
    (dir, engine, id)
}

#[core::prelude::v1::test]
fn held_exposure_key_renders_at_most_twice() {
    let (_dir, engine, id) = wide_library("coalesce");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_viewer_on(&view, vcx, &engine, id);
    keystrokes(vcx, "e");
    let (tx, rx) = std::sync::mpsc::channel();
    engine.request_unedited_frame(id, 64, move |frame| {
        tx.send(frame).expect("test receiver is alive")
    });
    let source = rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("decode must answer");
    view.update(vcx, |app, cx| app.land_edit_source(id, source, cx));
    vcx.run_until_parked();

    let before = view.update(vcx, |app, cx| {
        let before = app.edits.session.as_ref().expect("editing").renders;
        // Ten key repeats arrive before any render can land.
        for _ in 0..10 {
            app.step_edit_exposure(1, cx);
        }
        before
    });
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        let session = app.edits.session.as_ref().expect("editing");
        assert!(
            session.renders - before <= 2,
            "a held key must coalesce renders, ran {}",
            session.renders - before
        );
        assert!(session.rendering.is_none(), "the last render must land");
        assert_eq!(
            session.shown,
            Some(session.draft),
            "the preview on screen shows the final draft"
        );
    });
    view.update(vcx, |app, cx| {
        // Before the saved frame can land.
        app.finish_edit(cx);
        assert!(app.edits.session.is_none(), "Done saved");
        assert!(
            app.viewer_image.is_some(),
            "Done keeps the preview on screen until the saved frame arrives"
        );
    });
}

#[core::prelude::v1::test]
fn missing_preview_says_done_still_saves() {
    let (_dir, engine, photos, _) =
        crate::tests::quick_edit::edit_library_undecodable("no_preview");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_viewer_on(&view, vcx, &engine, photos[0]);
    keystrokes(vcx, "e");
    // The placeholder file cannot decode: the source request answers None.
    view.update(vcx, |app, cx| app.land_edit_source(photos[0], None, cx));
    view.update(vcx, |app, _| {
        assert!(
            app.toast
                .current()
                .is_some_and(|t| t.message.contains("Preview unavailable")),
            "a failed preview decode must say so, not show nothing silently"
        );
        assert!(app.edits.session.is_some(), "edit mode stays usable");
    });
}
