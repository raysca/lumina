//! Video transport regressions (BUGS.md 8).
//!
//! Phase 5 keeps the real post-seek frame refresh bounded and replaces the
//! fabricated gradient "preview" with honest time-only feedback. These tests
//! stay deterministic: they never wait on a decoded frame from AVFoundation.

use crate::app::types::{get_sample_items, UiMessage};
use crate::tests::ui_automation::{app_with_library, render_ui, seed_library};
use lumina_core::{MediaItem, MediaType};
use std::time::{Duration, Instant};

fn video_sample() -> MediaItem {
    get_sample_items()
        .into_iter()
        .find(|item| item.media_type == MediaType::Video)
        .expect("the deterministic sample set must contain a video")
}

/// A native player handle exists without a movie loaded; the bounded refresh
/// only needs the backend object, not a decodable file.
fn stub_player() -> crate::player::NativeVideoPlayer {
    crate::player::NativeVideoPlayer::new()
}

fn open_video_in_plain_app(
    view: &gpui::Entity<crate::LuminaApp>,
    vcx: &mut gpui::VisualTestContext,
    video: MediaItem,
    duration: f64,
) {
    view.update(vcx, |app, cx| {
        app.grid_state = None;
        app.select_item_at(video.clone(), 1, cx);
        app.open_viewer(video, cx);
        app.video.player = Some(stub_player());
        app.video.duration = duration;
        cx.notify();
    });
}

#[core::prelude::v1::test]
fn seek_arms_bounded_refresh_without_corrupting_the_scrub_drag() {
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| crate::LuminaApp::new(cx));
    open_video_in_plain_app(&view, vcx, video_sample(), 60.0);

    view.update(vcx, |app, cx| {
        // Pointer-down starts a drag and arms a refresh for the direct seek.
        app.begin_video_scrub(0.25, cx);
        assert!(
            app.video.is_scrubbing,
            "pointer down must arm the scrub drag"
        );
        assert_eq!(app.video.scrub_time, Some(15.0));
        assert!(
            app.video.seek_refresh_until.is_some(),
            "a paused seek must arm the bounded frame refresh"
        );

        // A second seek while the drag is still held must refresh the deadline
        // but must not cancel the drag or rewrite its destination.
        app.seek_video_percent(0.75, cx);
        assert!(
            app.video.is_scrubbing,
            "a seek performed during a drag must not clear the drag"
        );
        assert_eq!(
            app.video.scrub_time,
            Some(15.0),
            "only the pointer may move the drag destination"
        );
        assert!(
            app.video.seek_refresh_until.is_some(),
            "the seek must arm the refresh deadline"
        );

        // Releasing commits the destination and arms the refresh again.
        app.end_video_scrub(cx);
        assert!(!app.video.is_scrubbing, "release must end the drag");
        assert!((app.video.playback_time - 15.0).abs() < 0.001);
        assert!(
            app.video.seek_refresh_until.is_some(),
            "committing a scrub must arm the post-seek refresh"
        );

        // Tests must not leave a real ticker thread behind.
        app.stop_video_ticker();
        assert!(app.video.ticker_active.is_none());
    });
}

#[core::prelude::v1::test]
fn expired_seek_refresh_clears_the_deadline_and_stops_the_ticker() {
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| crate::LuminaApp::new(cx));
    open_video_in_plain_app(&view, vcx, video_sample(), 30.0);

    view.update(vcx, |app, cx| {
        app.video.is_playing = false;
        app.video.seek_refresh_until = Some(Instant::now() - Duration::from_millis(1));
        app.start_video_ticker();
        assert!(
            app.video.ticker_active.is_some(),
            "test premise: the ticker must be running before the tick"
        );

        app.handle_ui_message(UiMessage::VideoFrameTick, cx);

        assert!(
            app.video.seek_refresh_until.is_none(),
            "an expired refresh must clear the deadline"
        );
        assert!(
            app.video.ticker_active.is_none(),
            "an expired refresh must stop the ticker"
        );
        assert!(!app.video.is_playing, "the paused state must be preserved");
    });
}

#[core::prelude::v1::test]
fn overlay_scrub_drag_leaves_the_selected_media_unchanged() {
    let (temp_dir, engine) = seed_library("video_scrub_overlay", 8);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let video = video_sample();
    let expected = video.id.0;

    view.update(vcx, |app, cx| {
        // Fullscreen viewer over the grid: the layout where the transport sits
        // above the grid's lower rows.
        app.open_viewer(video, cx);
        app.video.duration = 60.0;
        cx.notify();
    });
    render_ui(vcx);

    let track = vcx
        .debug_bounds("video-scrubber")
        .expect("the video transport must paint its scrubber");
    let left: f32 = track.left().into();
    let width: f32 = track.size.width.into();
    let y: f32 = track.center().y.into();
    assert!(
        width > 100.0,
        "the scrubber must have a usable width, got {width}"
    );

    let press = gpui::point(gpui::px(left + width * 0.25), gpui::px(y));
    let drag = gpui::point(gpui::px(left + width * 0.75), gpui::px(y));

    vcx.simulate_mouse_down(press, gpui::MouseButton::Left, gpui::Modifiers::default());
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.video.is_scrubbing, "pointer down must arm the scrub");
        let destination = app.video.scrub_time.expect("scrub destination");
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
    render_ui(vcx);
    vcx.simulate_mouse_up(drag, gpui::MouseButton::Left, gpui::Modifiers::default());
    render_ui(vcx);

    view.update(vcx, |app, _| {
        assert!(!app.video.is_scrubbing, "release must end the scrub drag");
        assert!(
            (app.video.playback_time - 45.0).abs() < 1.0,
            "release must commit near 45s, got {}",
            app.video.playback_time
        );
        assert_eq!(
            app.selected_item.as_ref().map(|item| item.id.0),
            Some(expected),
            "dragging the scrubber must not select a grid card behind it"
        );
        assert!(
            !app.grid_drag.active,
            "the scrub press must not arm a grid drag"
        );
        app.stop_video_ticker();
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn scrubber_feedback_is_a_time_badge_not_a_synthetic_preview() {
    let (temp_dir, engine) = seed_library("video_scrub_badge", 4);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let video = video_sample();

    view.update(vcx, |app, cx| {
        app.open_viewer(video, cx);
        app.video.duration = 60.0;
        cx.notify();
    });
    render_ui(vcx);

    let track = vcx
        .debug_bounds("video-scrubber")
        .expect("the video transport must paint its scrubber");
    let hover = gpui::point(
        gpui::px(f32::from(track.left()) + f32::from(track.size.width) * 0.5),
        gpui::px(f32::from(track.center().y)),
    );
    vcx.simulate_mouse_move(hover, None, gpui::Modifiers::default());
    render_ui(vcx);
    let track_hovered = vcx
        .debug_bounds("video-scrubber")
        .expect("the video transport must paint its scrubber");
    assert_eq!(
        track_hovered.top(),
        track.top(),
        "mousing over the scrubber track must not cause a layout jump"
    );
    assert!(
        vcx.debug_bounds("video-time-badge").is_some(),
        "hovering the scrubber must show the honest time badge"
    );

    // Leaving the track clears the hover destination, so the honest badge is
    // no longer driven.
    vcx.simulate_mouse_move(
        gpui::point(gpui::px(4.0), gpui::px(4.0)),
        None,
        gpui::Modifiers::default(),
    );
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(
            app.video.hover_pct.is_none(),
            "leaving the track must clear the hover destination"
        );
        assert!(
            app.video.hover_time.is_none(),
            "leaving the track must clear the hover timestamp"
        );
    });
    assert!(
        vcx.debug_bounds("video-time-badge").is_none(),
        "leaving the track hides the time badge"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}
