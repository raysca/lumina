//! Video transport regressions (BUGS.md 8).
//!
//! The post-seek frame refresh stays bounded, and scrub previews come from a
//! strip of real decoded frames built once per open video. These tests stay
//! deterministic: strip results are injected through the test seam and never
//! decoded by AVFoundation here.

use crate::app::scrub_preview::{test_seam, ScrubStrip};
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
    let mut cx = crate::tests::TestApp::single();
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
    let mut cx = crate::tests::TestApp::single();
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
    let mut cx = crate::tests::TestApp::single();
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

/// The deterministic sample set's two videos.
fn two_videos() -> (MediaItem, MediaItem) {
    let mut videos = get_sample_items()
        .into_iter()
        .filter(|item| item.media_type == MediaType::Video);
    let a = videos.next().expect("the sample set has a first video");
    let b = videos.next().expect("the sample set has a second video");
    (a, b)
}

/// A strip of 2x2 frames at `times`, as the background decode would deliver.
fn strip_frames(times: &[f64]) -> Vec<(f64, std::sync::Arc<gpui::RenderImage>)> {
    times
        .iter()
        .map(|time| {
            let buffer = image::RgbaImage::from_raw(2, 2, vec![200; 16]).expect("2x2 buffer");
            let image = gpui::RenderImage::new(smallvec::smallvec![image::Frame::new(buffer)]);
            (*time, std::sync::Arc::new(image))
        })
        .collect()
}

/// Four frames spread over a 60 s video, centred in four equal slots.
const STRIP_TIMES: [f64; 4] = [7.5, 22.5, 37.5, 52.5];

fn preview_selector(time: f64) -> &'static str {
    crate::tests::ui_automation::leak(format!("video-scrub-preview-{}", (time * 1000.0).round()))
}

/// A video open in the viewer over a seeded library, with a player handle
/// and a 60 s duration, rendered once.
fn open_video_with_player(
    view: &gpui::Entity<crate::LuminaApp>,
    vcx: &mut gpui::VisualTestContext,
    video: MediaItem,
) -> gpui::Bounds<gpui::Pixels> {
    view.update(vcx, |app, cx| {
        app.open_viewer(video, cx);
        app.video.player = Some(stub_player());
        app.video.duration = 60.0;
        cx.notify();
    });
    render_ui(vcx);
    vcx.debug_bounds("video-scrubber")
        .expect("the video transport must paint its scrubber")
}

fn track_point(track: gpui::Bounds<gpui::Pixels>, pct: f32) -> gpui::Point<gpui::Pixels> {
    gpui::point(
        gpui::px(f32::from(track.left()) + f32::from(track.size.width) * pct),
        gpui::px(f32::from(track.center().y)),
    )
}

#[core::prelude::v1::test]
fn scrubber_without_strip_shows_time_badge_only() {
    let (temp_dir, engine) = seed_library("video_scrub_badge", 4);
    let mut cx = crate::tests::TestApp::single();
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
    vcx.simulate_mouse_move(track_point(track, 0.5), None, gpui::Modifiers::default());
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
        "hovering the scrubber must show the time badge"
    );
    assert!(
        vcx.debug_bounds("video-scrub-preview").is_none(),
        "without a strip there is no preview card"
    );
    assert_eq!(
        test_seam::requests(),
        0,
        "no strip is requested before the player has loaded the video"
    );

    // Leaving the track clears the hover destination and hides the badge.
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

#[core::prelude::v1::test]
fn hover_shows_nearest_strip_frame() {
    let (temp_dir, engine) = seed_library("video_scrub_hover_preview", 4);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let track = open_video_with_player(&view, vcx, video_sample());
    test_seam::inject(Ok(strip_frames(&STRIP_TIMES)));

    vcx.simulate_mouse_move(track_point(track, 0.6), None, gpui::Modifiers::default());
    render_ui(vcx);

    assert!(
        vcx.debug_bounds("video-scrub-preview").is_some(),
        "hovering with a strip must show the preview card"
    );
    assert!(
        vcx.debug_bounds(preview_selector(37.5)).is_some(),
        "hover at 60% of 60 s (36 s) must show the frame nearest it (37.5 s)"
    );
    assert!(
        vcx.debug_bounds("video-time-badge").is_some(),
        "the preview keeps the time badge"
    );
    assert_eq!(
        track,
        vcx.debug_bounds("video-scrubber").expect("scrubber"),
        "the preview card must not move the track"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn drag_scrub_shows_preview_for_destination() {
    let (temp_dir, engine) = seed_library("video_scrub_drag_preview", 4);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let track = open_video_with_player(&view, vcx, video_sample());
    test_seam::inject(Ok(strip_frames(&STRIP_TIMES)));

    let press = track_point(track, 0.1);
    let drag = track_point(track, 0.8);
    vcx.simulate_mouse_down(press, gpui::MouseButton::Left, gpui::Modifiers::default());
    render_ui(vcx);
    assert!(
        vcx.debug_bounds(preview_selector(7.5)).is_some(),
        "a press at 10% (6 s) must preview the frame nearest it (7.5 s)"
    );
    vcx.simulate_mouse_move(
        drag,
        Some(gpui::MouseButton::Left),
        gpui::Modifiers::default(),
    );
    render_ui(vcx);
    assert!(
        vcx.debug_bounds(preview_selector(52.5)).is_some(),
        "dragging to 80% (48 s) must preview the destination frame (52.5 s)"
    );
    assert!(
        vcx.debug_bounds(preview_selector(7.5)).is_none(),
        "the press frame must give way to the destination frame"
    );
    vcx.simulate_mouse_up(drag, gpui::MouseButton::Left, gpui::Modifiers::default());
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(!app.video.is_scrubbing, "release must end the scrub drag");
        app.stop_video_ticker();
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn stale_scrub_strip_is_discarded_after_media_change() {
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| crate::LuminaApp::new(cx));
    let (video_a, video_b) = two_videos();
    open_video_in_plain_app(&view, vcx, video_a.clone(), 60.0);

    view.update(vcx, |app, cx| {
        app.request_scrub_strip(cx);
        let cancel_a = app
            .video
            .strip_request
            .as_ref()
            .expect("hovering A starts a strip request")
            .cancel
            .clone();

        app.open_viewer(video_b.clone(), cx);
        app.video.player = Some(stub_player());
        app.video.duration = 60.0;
        app.request_scrub_strip(cx);

        app.finish_scrub_strip(video_a.id, &cancel_a, Ok(strip_frames(&STRIP_TIMES)), cx);
        assert!(
            app.video.preview_strip.is_none(),
            "A's late strip must never be shown for B"
        );
        assert_eq!(
            app.video.strip_request.as_ref().map(|r| r.item),
            Some(video_b.id),
            "B's own request must survive A's late result"
        );
        assert!(app.scrub_preview_at(10.0).is_none());
    });
}

#[core::prelude::v1::test]
fn media_change_cancels_inflight_strip() {
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| crate::LuminaApp::new(cx));
    let (video_a, video_b) = two_videos();
    open_video_in_plain_app(&view, vcx, video_a, 60.0);

    view.update(vcx, |app, cx| {
        app.request_scrub_strip(cx);
        let cancel_a = app
            .video
            .strip_request
            .as_ref()
            .expect("a strip request is in flight for A")
            .cancel
            .clone();
        assert_eq!(cancel_a.load(std::sync::atomic::Ordering::Acquire), 0);

        app.open_viewer(video_b, cx);
        assert_ne!(
            cancel_a.load(std::sync::atomic::Ordering::Acquire),
            0,
            "changing media must cancel A's strip decode"
        );
        assert!(
            app.video.strip_request.is_none(),
            "the new video starts without a request"
        );
    });
}

#[core::prelude::v1::test]
fn scrub_strip_requested_once_per_video_on_first_hover() {
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| crate::LuminaApp::new(cx));
    let video = video_sample();
    open_video_in_plain_app(&view, vcx, video.clone(), 60.0);

    view.update(vcx, |app, cx| {
        app.set_video_hover_scrub(Some(0.3), cx);
        app.set_video_hover_scrub(None, cx);
        app.set_video_hover_scrub(Some(0.6), cx);
        app.begin_video_scrub(0.5, cx);
        app.end_video_scrub(cx);
        assert_eq!(
            test_seam::requests(),
            1,
            "hover, leave, hover and press must share one strip request"
        );
        let cancel = app
            .video
            .strip_request
            .as_ref()
            .expect("request")
            .cancel
            .clone();
        assert_eq!(
            cancel.load(std::sync::atomic::Ordering::Acquire),
            0,
            "leaving the scrubber must not cancel the strip"
        );
        app.close_viewer(cx);
    });
    open_video_in_plain_app(&view, vcx, video, 60.0);
    view.update(vcx, |app, cx| {
        app.set_video_hover_scrub(Some(0.3), cx);
        assert_eq!(
            test_seam::requests(),
            2,
            "reopening the video gives it one fresh request"
        );
        app.stop_video_ticker();
    });
}

#[core::prelude::v1::test]
fn undecodable_video_stays_time_only_and_is_not_retried() {
    let (temp_dir, engine) = seed_library("video_scrub_undecodable", 4);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let track = open_video_with_player(&view, vcx, video_sample());
    test_seam::inject(Err(anyhow::anyhow!("no frames")));

    vcx.simulate_mouse_move(track_point(track, 0.4), None, gpui::Modifiers::default());
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("video-scrub-preview").is_none(),
        "a failed strip must leave the scrubber time-only"
    );
    assert!(
        vcx.debug_bounds("video-time-badge").is_some(),
        "the time badge stays after a failed strip"
    );
    view.update(vcx, |app, _| {
        assert!(
            app.error_message.is_none(),
            "a preview failure is not an error banner"
        );
    });

    vcx.simulate_mouse_move(
        gpui::point(gpui::px(4.0), gpui::px(4.0)),
        None,
        gpui::Modifiers::default(),
    );
    render_ui(vcx);
    vcx.simulate_mouse_move(track_point(track, 0.7), None, gpui::Modifiers::default());
    render_ui(vcx);
    assert_eq!(
        test_seam::requests(),
        1,
        "a failed strip must not be retried while the video stays open"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn closing_viewer_retires_strip_images() {
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| crate::LuminaApp::new(cx));
    open_video_in_plain_app(&view, vcx, video_sample(), 60.0);

    view.update(vcx, |app, cx| {
        test_seam::inject(Ok(strip_frames(&STRIP_TIMES)));
        app.set_video_hover_scrub(Some(0.5), cx);
        let ids: Vec<_> = app
            .video
            .preview_strip
            .as_ref()
            .map(|strip: &ScrubStrip| strip.frames.iter().map(|(_, image)| image.id).collect())
            .unwrap_or_default();
        assert_eq!(ids.len(), 4, "the injected strip must be stored");

        app.close_viewer(cx);
        assert!(app.video.preview_strip.is_none(), "closing drops the strip");
        for id in ids {
            assert!(
                app.retired_images.iter().any(|image| image.id == id),
                "every strip image must be retired so its atlas texture is freed"
            );
        }
    });
}

/// The committed H.264 fixture, opened in the real player (what tests get
/// on macOS).
#[cfg(target_os = "macos")]
fn fixture_video() -> MediaItem {
    let mut video = video_sample();
    video.path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/clip.mp4");
    video
}

#[cfg(target_os = "macos")]
#[core::prelude::v1::test]
fn scrub_pauses_playing_video_and_resumes_on_release() {
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| crate::LuminaApp::new(cx));
    view.update(vcx, |app, cx| {
        app.grid_state = None;
        let video = fixture_video();
        app.select_item_at(video.clone(), 1, cx);
        app.open_viewer(video, cx);
        assert!(
            app.video.player.is_some() && app.video.duration > 0.0,
            "test premise: the real player loads the fixture"
        );
        app.toggle_video_play(cx);
        assert!(app.video.is_playing, "test premise: playback started");

        app.begin_video_scrub(0.5, cx);
        assert!(!app.video.is_playing, "a scrub press must pause playback");
        app.update_video_scrub(0.7, cx);
        assert!(!app.video.is_playing, "dragging keeps playback paused");

        app.end_video_scrub(cx);
        assert!(app.video.is_playing, "release must resume playback");
        app.close_viewer(cx);
        assert!(!app.video.resume_after_scrub);
    });
}

#[core::prelude::v1::test]
fn scrub_of_paused_video_stays_paused() {
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| crate::LuminaApp::new(cx));
    open_video_in_plain_app(&view, vcx, video_sample(), 60.0);

    view.update(vcx, |app, cx| {
        assert!(!app.video.is_playing, "test premise: paused");
        app.begin_video_scrub(0.5, cx);
        app.end_video_scrub(cx);
        assert!(
            !app.video.is_playing,
            "scrubbing a paused video must leave it paused"
        );
        app.stop_video_ticker();
    });
}

#[core::prelude::v1::test]
fn pausing_during_a_scrub_is_not_undone_on_release() {
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| crate::LuminaApp::new(cx));
    view.update(vcx, |app, cx| {
        app.grid_state = None;
        let video = fixture_video();
        app.select_item_at(video.clone(), 1, cx);
        app.open_viewer(video, cx);
        app.toggle_video_play(cx);
        assert!(app.video.is_playing, "test premise: playback started");

        app.begin_video_scrub(0.5, cx);
        // Space twice mid-drag: the user plays, then pauses on purpose.
        app.toggle_video_play(cx);
        app.toggle_video_play(cx);
        assert!(!app.video.is_playing);
        app.end_video_scrub(cx);
        assert!(
            !app.video.is_playing,
            "release must not resume a video the user paused during the drag"
        );
        app.close_viewer(cx);
    });
}

#[core::prelude::v1::test]
fn scrubber_is_a_strip_with_playhead_pill() {
    let (temp_dir, engine) = seed_library("video_scrub_strip", 4);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let track = open_video_with_player(&view, vcx, video_sample());
    assert_eq!(
        f32::from(track.size.height),
        54.0,
        "the scrubber is a 54px thumbnail strip"
    );
    assert!(
        vcx.debug_bounds("video-strip-frame-0").is_none(),
        "before the strip arrives the scrubber is a plain bar"
    );
    let pill = vcx
        .debug_bounds("video-playhead-time")
        .expect("the playhead carries a time pill");
    assert!(
        pill.top() >= track.bottom(),
        "the time pill hangs under the strip: {pill:?} {track:?}"
    );

    test_seam::inject(Ok(strip_frames(&STRIP_TIMES)));
    vcx.simulate_mouse_move(track_point(track, 0.5), None, gpui::Modifiers::default());
    view.update(vcx, |app, cx| {
        app.video.playback_time = 34.0;
        cx.notify();
    });
    render_ui(vcx);
    let first = vcx
        .debug_bounds("video-strip-frame-0")
        .expect("with a strip the frames tile the track");
    let last = vcx
        .debug_bounds("video-strip-frame-3")
        .expect("every strip frame gets a slot");
    assert!(
        (f32::from(first.left()) - f32::from(track.left())).abs() < 1.0
            && (f32::from(last.right()) - f32::from(track.right())).abs() < 1.0,
        "the frames span the track: {first:?} {last:?} {track:?}"
    );
    assert_eq!(
        crate::views::viewer::transport::playhead_label(34.0),
        "0:34",
        "the pill shows the current time"
    );
    let _ = std::fs::remove_dir_all(&temp_dir);
}
