//! Video player controls: transport, resume progress, speed and mute.

use crate::tests::helpers::*;
use crate::*;

#[core::prelude::v1::test]
fn test_time_fractional_and_bytes_formatting_safety() {
    use crate::app::types::{format_bytes, format_time_fractional};

    // Edge case guards: negative, NaN, infinity
    assert_eq!(format_time_fractional(0.0), "0:00.00");
    assert_eq!(format_time_fractional(-10.5), "0:00.00");
    assert_eq!(format_time_fractional(f64::NAN), "0:00.00");
    assert_eq!(format_time_fractional(f64::INFINITY), "0:00.00");

    // Standard sub-minute durations
    assert_eq!(format_time_fractional(16.04), "0:16.04");
    assert_eq!(format_time_fractional(42.0), "0:42.00");
    assert_eq!(format_time_fractional(59.99), "0:59.99");

    // Over one minute
    assert_eq!(format_time_fractional(65.12), "1:05.12");

    // Over one hour
    assert_eq!(format_time_fractional(3665.50), "1:01:05.50");

    // format_bytes tests
    assert_eq!(format_bytes(0), "0 B");
    assert_eq!(format_bytes(512), "512 B");
    assert_eq!(format_bytes(2048), "2.0 KB");
    assert_eq!(format_bytes(10 * 1024 * 1024), "10.0 MB");
    assert_eq!(format_bytes(4 * 1024 * 1024 * 1024), "4.0 GB");
}

#[core::prelude::v1::test]
fn test_viewer_player_ux_and_navigation_controls() {
    use crate::views::viewer::MediaViewer;

    let mut cx = crate::tests::TestApp::single();
    let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));

    let (_temp_dir, engine) = seed_library("viewer_nav", 2);
    assert!(engine.total_count() >= 2);

    let sample = get_sample_items();
    let photo_item = sample[0].clone();
    let video_item = sample[1].clone();

    view.update(cx, |app, cx| {
        app.grid_state = Some(crate::views::grid::GridState::new(engine.clone()));
        // 1. Open photo item in viewer
        app.select_item_at(photo_item.clone(), 0, cx);
        app.open_viewer(photo_item.clone(), cx);
        assert!(app.is_fullscreen_viewer);

        // Double click / zoom toggle
        assert_eq!(app.viewer_state.zoom_scale, 1.0);
        app.toggle_viewer_zoom(cx);
        assert_eq!(app.viewer_state.zoom_scale, 2.0);
        app.toggle_viewer_zoom(cx);
        assert_eq!(app.viewer_state.zoom_scale, 1.0);

        // Bracket navigation: ']' advances selection
        app.navigate_selection(1, cx);
        assert_eq!(app.selected_index, Some(1));
        assert!(app.selected_item.is_some());

        // 2. Video Player Controls
        app.video.duration = 42.0;
        app.video.playback_time = 16.04;

        // Hover scrub test: set hover time and pct
        assert!(app.video.hover_pct.is_none());
        app.set_video_hover_scrub(Some(0.575), cx);
        assert_eq!(app.video.hover_pct, Some(0.575));
        assert!((app.video.hover_time.unwrap() - 24.15).abs() < 0.01);

        // Clear hover scrub
        app.set_video_hover_scrub(None, cx);
        assert!(app.video.hover_pct.is_none());
        assert!(app.video.hover_time.is_none());

        // Speed popover menu toggle and selection
        assert!(!app.video.speed_menu_open);
        app.toggle_video_speed_menu(cx);
        assert!(app.video.speed_menu_open);
        app.set_video_speed(1.5, cx);
        assert_eq!(app.video.playback_speed, 1.5);
        assert!(!app.video.speed_menu_open);

        // Render viewer with all elements active to verify GPUI tree construction
        let _el = MediaViewer::render(app, cx);

        // 3. Closing the viewer resets viewer state and returns to the grid.
        app.close_viewer(cx);
        assert!(!app.is_fullscreen_viewer);
        assert_eq!(app.viewer_state.zoom_scale, 1.0);
    });
    cx.run_until_parked();

    // 4. Test Keymap handling via KeyDownEvent in viewer mode
    cx.update(|window, app_cx| {
        view.update(app_cx, |app, cx| {
            // Open photo in viewer
            app.select_item_at(photo_item.clone(), 0, cx);
            app.open_viewer(photo_item.clone(), cx);
            assert!(app.is_fullscreen_viewer);

            // Space on a still must not close the viewer; Escape and the
            // viewer Back button are the exits.
            app.handle_key_down(
                &KeyDownEvent {
                    keystroke: Keystroke::parse("space").unwrap(),
                    is_held: false,
                    prefer_character_input: false,
                },
                window,
                cx,
            );
            assert!(
                app.is_fullscreen_viewer,
                "Space on a photo must not close the viewer"
            );

            // Open video in viewer
            app.select_item_at(video_item.clone(), 1, cx);
            app.open_viewer(video_item.clone(), cx);
            assert!(app.is_fullscreen_viewer);
            // The sample path doesn't exist, so the load failed; this part
            // is about a clip that loaded.
            app.video.load_error = None;

            // Space on video toggles play/pause
            assert!(!app.video.is_playing);
            app.handle_key_down(
                &KeyDownEvent {
                    keystroke: Keystroke::parse("space").unwrap(),
                    is_held: false,
                    prefer_character_input: false,
                },
                window,
                cx,
            );
            assert!(app.video.is_playing, "Space on video must toggle playback");

            // Speed menu open + Escape closes speed menu without closing viewer
            app.toggle_video_speed_menu(cx);
            assert!(app.video.speed_menu_open);

            app.handle_key_down(
                &KeyDownEvent {
                    keystroke: Keystroke::parse("escape").unwrap(),
                    is_held: false,
                    prefer_character_input: false,
                },
                window,
                cx,
            );
            assert!(
                !app.video.speed_menu_open,
                "Escape must close speed menu first"
            );
            assert!(
                app.is_fullscreen_viewer,
                "Viewer should remain open when dismissing speed menu"
            );

            // Next Escape closes viewer
            app.handle_key_down(
                &KeyDownEvent {
                    keystroke: Keystroke::parse("escape").unwrap(),
                    is_held: false,
                    prefer_character_input: false,
                },
                window,
                cx,
            );
            assert!(!app.is_fullscreen_viewer, "Second Escape must close viewer");
        });
    });
    cx.run_until_parked();
}

#[core::prelude::v1::test]
fn test_video_resume_progress_throttle_and_close() {
    let mut cx = crate::tests::TestApp::single();
    let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    let video = get_sample_items()[1].clone();
    let video_id = video.id;

    view.update(cx, |app, cx| {
        // Without a catalog, the selected video is the playback identity (the
        // production path always has grid state and tracks the loaded media id).
        app.grid_state = None;
        app.select_item_at(video.clone(), 1, cx);
        app.open_viewer(video.clone(), cx);
        app.video.duration = 30.0;

        // The first background update seeds the position.
        app.video.playback_time = 4.0;
        app.note_video_resume_progress();
        assert_eq!(app.config.resume_positions.get(video_id), Some(4.0));

        // A small advance is throttled; a large one commits.
        app.video.playback_time = 7.0;
        app.note_video_resume_progress();
        assert_eq!(
            app.config.resume_positions.get(video_id),
            Some(4.0),
            "progress under five seconds must not rewrite the resume point"
        );
        app.video.playback_time = 12.0;
        app.note_video_resume_progress();
        assert_eq!(app.config.resume_positions.get(video_id), Some(12.0));

        // Reaching the end margin clears the stale point once.
        app.video.playback_time = 29.0;
        app.note_video_resume_progress();
        assert_eq!(
            app.config.resume_positions.get(video_id),
            None,
            "a position near the end must be dropped"
        );

        // Pause commits the live position through the normal save path.
        app.video.playback_time = 20.0;
        app.video.player = Some(crate::player::NativeVideoPlayer::new());
        app.video.is_playing = true;
        app.toggle_video_play(cx);
        assert!(!app.video.is_playing);
        assert_eq!(
            app.config.resume_positions.get(video_id),
            Some(20.0),
            "pausing must persist the current position"
        );

        // Viewer close commits the final position too, even though the sample
        // path has no decodable movie (position falls back to playback_time).
        app.video.playback_time = 23.0;
        app.close_viewer(cx);
        assert!(!app.is_fullscreen_viewer);
        assert_eq!(
            app.config.resume_positions.get(video_id),
            Some(23.0),
            "closing the viewer must save the final position"
        );
    });
    cx.run_until_parked();
}

/// Speed and mute are restored from the config into the viewer state and the
/// player each video opens with, and changing them updates the config.
#[cfg(target_os = "macos")]
#[core::prelude::v1::test]
fn video_speed_and_mute_come_from_config() {
    use crate::player::VideoPlayerBackend;
    crate::app::state::TEST_CONFIG.with(|c| {
        *c.borrow_mut() = Some(crate::config::AppConfig {
            video_speed: Some(1.3),
            video_muted: true,
            ..Default::default()
        })
    });
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    view.update(vcx, |app, cx| {
        app.grid_state = None;
        assert_eq!(
            app.video.playback_speed, 1.5,
            "a saved 1.3x must snap to the menu's 1.5x"
        );
        assert!(app.video.is_muted, "a saved mute must be restored");

        let mut video = get_sample_items()
            .into_iter()
            .find(|item| item.media_type == lumina_core::MediaType::Video)
            .expect("the sample set must contain a video");
        video.path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/clip.mp4");
        app.select_item_at(video.clone(), 1, cx);
        app.open_viewer(video.clone(), cx);
        let player = app
            .video
            .player
            .as_ref()
            .expect("test premise: the real player loads the fixture");
        assert_eq!(
            player.rate(),
            1.5,
            "the new player must get the saved speed"
        );
        assert!(player.is_muted(), "the new player must start muted");

        app.set_video_speed(2.0, cx);
        app.toggle_video_mute(cx);
        assert_eq!(app.config.video_speed, Some(2.0), "speed changes are saved");
        assert!(!app.config.video_muted, "mute changes are saved");

        // A media change creates a new player, which gets the current values.
        let mut other = video.clone();
        other.id = lumina_core::MediaId(other.id.0 + 1000);
        app.load_video_player(&other, cx);
        let player = app.video.player.as_ref().expect("the second video loads");
        assert_eq!(player.rate(), 2.0, "the next player keeps the chosen speed");
        assert!(!player.is_muted(), "the next player keeps the chosen mute");

        // A failed write is reported, not swallowed.
        crate::app::state::SAVE_FAILS.with(|f| f.set(true));
        app.toggle_video_mute(cx);
        crate::app::state::SAVE_FAILS.with(|f| f.set(false));
        assert!(
            app.error_message.is_some(),
            "a failed mute save must reach the error banner"
        );
        app.close_viewer(cx);
    });
}
