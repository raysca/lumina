//! Viewer journeys: shortcuts, delete acknowledgement, fullscreen.

use crate::tests::helpers::*;
use crate::*;

#[core::prelude::v1::test]
fn test_viewer_shortcuts_and_delete_acknowledgement() {
    let mut cx = crate::tests::TestApp::single();
    let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));

    view.update(cx, |app, cx| {
        app.grid_state = None;

        // 1. Viewer shortcuts popover
        assert!(!app.viewer_shortcuts_open);
        app.toggle_viewer_shortcuts(cx);
        assert!(app.viewer_shortcuts_open);
        app.close_viewer(cx);
        assert!(!app.viewer_shortcuts_open);

        // 2. Delete acknowledgement
        let sample = get_sample_items();
        app.selected_item = Some(sample[0].clone());
        app.selected_indices = [0].into_iter().collect();

        // Prompt delete
        app.prompt_delete_selected(cx);
        assert!(app.delete_confirm_state.is_some());
        let st = app.delete_confirm_state.as_ref().unwrap();
        assert_eq!(st.target, lumina_core::DeletionTarget::SystemTrash);
        assert!(!st.permanent_acknowledged);

        // Switch to permanent disk deletion
        app.set_delete_target(lumina_core::DeletionTarget::PermanentDisk, cx);
        let st = app.delete_confirm_state.as_ref().unwrap();
        assert_eq!(st.target, lumina_core::DeletionTarget::PermanentDisk);
        assert!(!st.permanent_acknowledged);

        // Toggle acknowledgement
        app.toggle_delete_permanent_acknowledged(cx);
        assert!(
            app.delete_confirm_state
                .as_ref()
                .unwrap()
                .permanent_acknowledged
        );

        // Close modal
        app.close_delete_modal(cx);
        assert!(app.delete_confirm_state.is_none());
    });
    cx.run_until_parked();
}

#[core::prelude::v1::test]
fn test_e2e_fullscreen_viewer_journey() {
    use crate::views::viewer::MediaViewer;

    // Keys reach the viewer only with a library open (the welcome screen
    // gates every key it does not handle).
    let (_dir, engine) = seed_library("viewer_journey", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, cx) = app_with_library(&mut cx, engine);

    let sample = get_sample_items();

    // 1. Initial State: Viewer closed
    view.update(cx, |app, cx| {
        assert!(!app.is_fullscreen_viewer);
        assert!(!app.is_immersive_fullscreen);

        // Open viewer on first item (photo)
        app.select_item_at(sample[0].clone(), 0, cx);
        app.open_viewer(sample[0].clone(), cx);
        assert!(app.is_fullscreen_viewer);
        assert!(!app.is_immersive_fullscreen);
        assert!(app.immersive_hud_visible);

        // Render viewer in standard mode
        let _el = MediaViewer::render(app, cx);
    });
    cx.run_until_parked();

    // 2. Press ⌃⌘F to enter immersive fullscreen
    cx.update(|window, app_cx| {
        view.update(app_cx, |app, cx| {
            app.handle_key_down(
                &KeyDownEvent {
                    keystroke: Keystroke::parse("ctrl-cmd-f").unwrap(),
                    is_held: false,
                    prefer_character_input: false,
                },
                window,
                cx,
            );

            assert!(app.is_immersive_fullscreen);
            assert!(app.immersive_hud_visible);

            // Render viewer in immersive mode with HUD visible
            let _el = MediaViewer::render(app, cx);
        });
    });
    cx.run_until_parked();

    // 3. Press "H" to hide HUD controls (distraction-free view)
    cx.update(|window, app_cx| {
        view.update(app_cx, |app, cx| {
            app.handle_key_down(
                &KeyDownEvent {
                    keystroke: Keystroke::parse("h").unwrap(),
                    is_held: false,
                    prefer_character_input: false,
                },
                window,
                cx,
            );

            assert!(app.is_immersive_fullscreen);
            assert!(!app.immersive_hud_visible);

            // Render viewer in immersive mode with HUD hidden
            let _el = MediaViewer::render(app, cx);
        });
    });
    cx.run_until_parked();

    // 4. Press "H" again to restore HUD controls
    cx.update(|window, app_cx| {
        view.update(app_cx, |app, cx| {
            app.handle_key_down(
                &KeyDownEvent {
                    keystroke: Keystroke::parse("h").unwrap(),
                    is_held: false,
                    prefer_character_input: false,
                },
                window,
                cx,
            );

            assert!(app.is_immersive_fullscreen);
            assert!(app.immersive_hud_visible);
        });
    });
    cx.run_until_parked();

    // 5. Navigate to next item while in fullscreen
    view.update(cx, |app, cx| {
        app.select_item_at(sample[1].clone(), 1, cx);
        assert_eq!(app.selected_index, Some(1));
        assert!(app.is_immersive_fullscreen);
    });
    cx.run_until_parked();

    // 6. Press Escape: exits fullscreen back to normal viewer (does NOT close viewer yet)
    cx.update(|window, app_cx| {
        view.update(app_cx, |app, cx| {
            app.handle_key_down(
                &KeyDownEvent {
                    keystroke: Keystroke::parse("escape").unwrap(),
                    is_held: false,
                    prefer_character_input: false,
                },
                window,
                cx,
            );

            assert!(!app.is_immersive_fullscreen);
            assert!(app.is_fullscreen_viewer);
            assert!(app.immersive_hud_visible);

            let _el = MediaViewer::render(app, cx);
        });
    });
    cx.run_until_parked();

    // 7. Press Escape again: closes viewer back to grid
    cx.update(|window, app_cx| {
        view.update(app_cx, |app, cx| {
            app.handle_key_down(
                &KeyDownEvent {
                    keystroke: Keystroke::parse("escape").unwrap(),
                    is_held: false,
                    prefer_character_input: false,
                },
                window,
                cx,
            );

            assert!(!app.is_fullscreen_viewer);
            assert!(!app.is_immersive_fullscreen);
        });
    });
    cx.run_until_parked();

    // 8. Video Item Fullscreen Journey:
    // Open viewer on video item (sample[1])
    view.update(cx, |app, cx| {
        let video_item = sample[1].clone();
        assert_eq!(video_item.media_type, lumina_core::MediaType::Video);

        app.select_item_at(video_item.clone(), 1, cx);
        app.open_viewer(video_item, cx);
        assert!(app.is_fullscreen_viewer);

        // Setup real AvFoundation player backend
        app.video.player = Some(crate::player::NativeVideoPlayer::new());
        app.video.duration = 30.0;

        // Enter fullscreen via toolbar/method
        app.enter_immersive_fullscreen(None, cx);
        assert!(app.is_immersive_fullscreen);

        // Toggle play/pause
        assert!(!app.video.is_playing);
        app.toggle_video_play(cx);
        assert!(app.video.is_playing);
        app.toggle_video_play(cx);
        assert!(!app.video.is_playing);

        // Test video playback speed cycling
        assert_eq!(app.video.playback_speed, 1.0);
        app.cycle_video_speed(cx);
        assert_eq!(app.video.playback_speed, 1.5);

        // Test mute and loop toggling
        assert!(!app.video.is_muted);
        app.toggle_video_mute(cx);
        assert!(app.video.is_muted);
        app.toggle_video_mute(cx);
        assert!(!app.video.is_muted);

        assert!(app.video.is_looping);
        app.toggle_video_loop(cx);
        assert!(!app.video.is_looping);
        app.toggle_video_loop(cx);
        assert!(app.video.is_looping);

        // Test speed popover menu & direct speed selection
        app.toggle_video_speed_menu(cx);
        assert!(app.video.speed_menu_open);
        app.set_video_speed(2.0, cx);
        assert_eq!(app.video.playback_speed, 2.0);
        assert!(!app.video.speed_menu_open);

        // Test hover scrub preview
        app.set_video_hover_scrub(Some(24.18 / 30.0), cx);
        assert!(app.video.hover_time.is_some());

        // Render video player in immersive mode with all studio components active
        let _el = MediaViewer::render(app, cx);

        // Close viewer cleans up all state
        app.close_viewer(cx);
        assert!(!app.is_fullscreen_viewer);
        assert!(!app.is_immersive_fullscreen);
        assert!(app.immersive_hud_visible);
    });
    cx.run_until_parked();
}
