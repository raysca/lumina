//! The "Original not available" card on stages other than a
//! photo in the plain viewer: a slideshow hides it, and a missing video's
//! smaller frame still holds the whole card.

use super::downloads::TempLibrary;
use super::ui_automation::{app_with_library, keystrokes, render_ui, seed_library};
use gpui::px;
use lumina_core::MediaType;

#[core::prelude::v1::test]
fn slideshow_hides_the_missing_card() {
    let (temp_dir, engine) = seed_library("missing_slideshow", 2);
    // Removed when the test ends, pass or fail.
    let temp_dir = TempLibrary(temp_dir);
    for file in ["photo_00.jpg", "photo_01.jpg"] {
        let gone = temp_dir.join(file);
        std::fs::remove_file(&gone).unwrap();
        engine.catalog().mark_missing_under(&gone).unwrap();
    }
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.select_index(0, cx);
        let item = app.selected_item.clone().unwrap();
        app.open_viewer(item, cx);
    });
    vcx.run_until_parked();
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("viewer-missing-card").is_some(),
        "the plain viewer shows the card over a missing item"
    );
    keystrokes(vcx, "escape");

    keystrokes(vcx, "alt-secondary-s");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.slideshow.is_some(), "⌥⌘S starts the slideshow");
        assert!(app.is_fullscreen_viewer, "it plays in the viewer");
        assert!(
            app.selected_item
                .as_ref()
                .is_some_and(|item| app.missing_items.contains(&item.id)),
            "the slide on screen is a missing item"
        );
    });
    assert!(
        vcx.debug_bounds("viewer-missing-card").is_none(),
        "a slideshow must not stop on the card: it has no one to press its buttons"
    );
}

#[core::prelude::v1::test]
fn missing_video_shows_the_whole_card_inside_its_frame() {
    let (temp_dir, engine) = seed_library("missing_video", 1);
    // Removed when the test ends, pass or fail.
    let temp_dir = TempLibrary(temp_dir);
    let mut clip = engine.get_item_slice(0..1).remove(0);
    clip.path = temp_dir.join("clip.mp4");
    clip.file_name = "clip.mp4".into();
    clip.media_type = MediaType::Video;
    (clip.width, clip.height) = (1920, 1080);
    clip.duration_ms = Some(10_000);
    let clip = engine
        .catalog()
        .insert_or_update_batch(&[clip])
        .unwrap()
        .remove(0);
    engine.catalog().mark_missing_under(&clip.path).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        assert!(
            app.missing_items.contains(&clip.id),
            "the fixture's video is missing"
        );
        app.open_viewer(clip.clone(), cx);
    });
    vcx.run_until_parked();
    render_ui(vcx);

    let stage = vcx
        .debug_bounds("viewer-viewport")
        .expect("the viewer stage is drawn");
    let window = vcx.debug_bounds("app-root").expect("the app root is drawn");
    assert!(
        stage.size.width < window.size.width && stage.size.height < window.size.height,
        "a video gets the smaller fitted frame, not the full stage: {stage:?} in {window:?}"
    );
    let card = vcx
        .debug_bounds("viewer-missing-card")
        .expect("a missing video shows the card");
    assert!(
        stage.contains(&card.origin) && stage.contains(&card.bottom_right()),
        "the card fits inside the video frame: card {card:?} frame {stage:?}"
    );
    let off_centre = (f32::from(card.center().x) - f32::from(stage.center().x)).abs();
    assert!(
        off_centre < 1.0,
        "the card is centred on the video frame, off by {off_centre}"
    );
    let bar = vcx
        .debug_bounds("viewer-missing-bar")
        .expect("a missing video shows the bar");
    assert!(
        (bar.left() - stage.left()).abs() < px(0.5)
            && (bar.right() - stage.right()).abs() < px(0.5),
        "the bar spans the video frame: bar {bar:?} frame {stage:?}"
    );
    assert!(
        vcx.debug_bounds("viewer-missing-locate").is_some()
            && vcx.debug_bounds("viewer-missing-keep").is_some(),
        "both card buttons are drawn"
    );
    view.update(vcx, |app, _| {
        assert!(
            !app.playback_failed_shown(),
            "a missing video shows the offline card, not the playback-failed card"
        )
    });
}
