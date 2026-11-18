//! Scrubbing a video with the pointer.

use crate::tests::helpers::*;
use crate::*;

#[core::prelude::v1::test]
fn test_video_scrub_pointer_drag_updates_destination_and_commits() {
    // The root render path shows onboarding when no library is open, so the
    // scrubber can only be verified through a real (temporary) library.
    let (_temp_dir, engine) = seed_library("video_scrub_drag", 2);
    assert!(engine.total_count() >= 2);

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    let video = get_sample_items()[1].clone();

    view.update(vcx, |app, cx| {
        app.grid_state = Some(crate::views::grid::GridState::new(engine.clone()));
        app.select_item_at(video.clone(), 1, cx);
        app.open_viewer(video.clone(), cx);
        app.video.duration = 60.0;
        // A native player handle exists on macOS even without a loaded movie;
        // the drag path reaches the real seek bridge without requiring decoded
        // frames (the bridge exposes no still for arbitrary scrub positions).
        app.video.player = Some(crate::player::NativeVideoPlayer::new());
        app.window_bounds = Some((0.0, 0.0, 1024.0, 768.0));
        cx.notify();
    });
    vcx.simulate_resize(gpui::size(gpui::px(1024.0), gpui::px(768.0)));
    render_viewer_ui(vcx);
    render_viewer_ui(vcx);

    let track = vcx
        .debug_bounds("video-scrubber")
        .expect("video scrubber track must be painted in the viewer");
    let left: f32 = track.left().into();
    let width: f32 = track.size.width.into();
    let y: f32 = track.center().y.into();
    assert!(
        width > 100.0,
        "scrubber must have a usable width, got {width}"
    );

    let press = gpui::point(gpui::px(left + width * 0.25), gpui::px(y));
    let drag = gpui::point(gpui::px(left + width * 0.75), gpui::px(y));

    vcx.simulate_mouse_down(press, gpui::MouseButton::Left, gpui::Modifiers::default());
    render_viewer_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.video.is_scrubbing, "pointer down must start scrubbing");
        let destination = app
            .video
            .scrub_time
            .expect("scrub destination must be tracked");
        assert!(
            (destination - 15.0).abs() < 1.0,
            "press at 25% must land near 15s, got {destination}"
        );
    });

    vcx.simulate_mouse_move(
        drag,
        Some(gpui::MouseButton::Left),
        gpui::Modifiers::default(),
    );
    render_viewer_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(
            app.video.is_scrubbing,
            "dragging with the button held must keep scrubbing"
        );
        let destination = app
            .video
            .scrub_time
            .expect("scrub destination must follow the pointer");
        assert!(
            (destination - 45.0).abs() < 1.0,
            "drag to 75% must track near 45s, got {destination}"
        );
    });

    vcx.simulate_mouse_up(drag, gpui::MouseButton::Left, gpui::Modifiers::default());
    render_viewer_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(!app.video.is_scrubbing, "pointer up must end scrubbing");
        assert!(app.video.scrub_time.is_none());
        assert!(
            (app.video.playback_time - 45.0).abs() < 1.0,
            "release must commit the final destination, got {}",
            app.video.playback_time
        );
    });

    // A press + release without movement remains click-to-seek.
    let click = gpui::point(gpui::px(left + width * 0.5), gpui::px(y));
    vcx.simulate_click(click, gpui::Modifiers::default());
    render_viewer_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(!app.video.is_scrubbing);
        assert!(
            (app.video.playback_time - 30.0).abs() < 1.0,
            "click at 50% must seek near 30s, got {}",
            app.video.playback_time
        );
    });
}

#[core::prelude::v1::test]
fn scrub_drag_keeps_tracking_after_leaving_the_control() {
    let (_temp_dir, engine) = seed_library("drag_off_control", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    let video = get_sample_items()[1].clone();
    view.update(vcx, |app, cx| {
        app.grid_state = Some(crate::views::grid::GridState::new(engine.clone()));
        app.select_item_at(video.clone(), 1, cx);
        app.open_viewer(video.clone(), cx);
        app.video.duration = 60.0;
        app.video.player = Some(crate::player::NativeVideoPlayer::new());
        cx.notify();
    });
    vcx.simulate_resize(gpui::size(gpui::px(1024.0), gpui::px(768.0)));
    render_viewer_ui(vcx);
    render_viewer_ui(vcx);

    // Scrubber: press at 25%, then drift 80px above the 34px track to 75%.
    let track = vcx
        .debug_bounds("video-scrubber")
        .expect("video scrubber track must be painted in the viewer");
    let (left, width): (f32, f32) = (track.left().into(), track.size.width.into());
    let y: f32 = track.center().y.into();
    let left_button = Some(gpui::MouseButton::Left);
    vcx.simulate_mouse_down(
        gpui::point(gpui::px(left + width * 0.25), gpui::px(y)),
        gpui::MouseButton::Left,
        gpui::Modifiers::default(),
    );
    render_viewer_ui(vcx);
    let off_track = gpui::point(gpui::px(left + width * 0.75), gpui::px(y - 80.0));
    vcx.simulate_mouse_move(off_track, left_button, gpui::Modifiers::default());
    render_viewer_ui(vcx);
    view.update(vcx, |app, _| {
        let destination = app.video.scrub_time.expect("scrub must still be live");
        assert!(
            (destination - 45.0).abs() < 1.0,
            "a scrub drag must follow the pointer off the track, got {destination}"
        );
    });
    vcx.simulate_mouse_up(
        off_track,
        gpui::MouseButton::Left,
        gpui::Modifiers::default(),
    );
    render_viewer_ui(vcx);
    view.update(vcx, |app, _| assert!(!app.video.is_scrubbing));
}
