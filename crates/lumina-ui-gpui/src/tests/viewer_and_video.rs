use crate::*;

#[core::prelude::v1::test]
fn test_viewer_shortcuts_and_delete_acknowledgement() {
    let mut cx = gpui::TestAppContext::single();
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
fn test_immersive_fullscreen_mode_lifecycle() {
    let mut cx = gpui::TestAppContext::single();
    let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));

    view.update(cx, |app, cx| {
        app.grid_state = None;

        // 1. Initial State
        assert!(!app.is_immersive_fullscreen);
        assert!(app.immersive_hud_visible);

        // 2. Enter Immersive Fullscreen
        app.enter_immersive_fullscreen(None, cx);
        assert!(app.is_immersive_fullscreen);
        assert!(app.immersive_hud_visible);

        // 3. Toggle HUD visibility
        app.toggle_immersive_hud(cx);
        assert!(!app.immersive_hud_visible);
        app.toggle_immersive_hud(cx);
        assert!(app.immersive_hud_visible);

        // 4. Toggle Immersive Fullscreen (exits)
        app.toggle_immersive_fullscreen(None, cx);
        assert!(!app.is_immersive_fullscreen);
        assert!(app.immersive_hud_visible);

        // 5. Toggle Immersive Fullscreen back on
        app.toggle_immersive_fullscreen(None, cx);
        assert!(app.is_immersive_fullscreen);

        // 6. Close viewer resets immersive fullscreen
        app.close_viewer(cx);
        assert!(!app.is_immersive_fullscreen);
        assert!(app.immersive_hud_visible);
    });
    cx.run_until_parked();
}

#[core::prelude::v1::test]
fn test_e2e_fullscreen_viewer_journey() {
    use crate::views::viewer::MediaViewer;

    let mut cx = gpui::TestAppContext::single();
    let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));

    let sample = get_sample_items();

    // 1. Initial State: Viewer closed
    view.update(cx, |app, cx| {
        app.grid_state = None;
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

    // 2. Press "F" to enter immersive fullscreen
    cx.update(|window, app_cx| {
        view.update(app_cx, |app, cx| {
            app.handle_key_down(
                &KeyDownEvent {
                    keystroke: Keystroke::parse("f").unwrap(),
                    is_held: false,
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
        app.video.player = Some(crate::player::macos::AvFoundationPlayer::new());
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

        // Test volume setting
        app.set_video_volume(0.85, cx);
        assert_eq!(app.video.volume, 0.85);

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

#[core::prelude::v1::test]
fn test_volume_slider_range_and_mute() {
    let mut cx = gpui::TestAppContext::single();
    let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));

    view.update(cx, |app, cx| {
        // Test 6 discrete volume steps: 0.0, 0.2, 0.4, 0.6, 0.8, 1.0
        let steps: Vec<f32> = (0..=5).map(|i| i as f32 / 5.0).collect();
        assert_eq!(steps, vec![0.0, 0.2, 0.4, 0.6, 0.8, 1.0]);

        // Mute step (0.0)
        app.set_video_volume(0.0, cx);
        assert_eq!(app.video.volume, 0.0);

        // Half volume
        app.set_video_volume(0.5, cx);
        assert_eq!(app.video.volume, 0.5);

        // Full volume (1.0)
        app.set_video_volume(1.0, cx);
        assert_eq!(app.video.volume, 1.0);
    });
}

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
fn test_viewer_bottom_filmstrip_selection_transition() {
    let mut cx = gpui::TestAppContext::single();
    let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));

    let sample = get_sample_items();
    let photo_item = sample[0].clone();
    let video_item = sample[1].clone();
    assert_eq!(photo_item.media_type, lumina_core::MediaType::Image);
    assert_eq!(video_item.media_type, lumina_core::MediaType::Video);

    // 1. Open viewer on video item with active player
    view.update(cx, |app, cx| {
        app.select_item_at(video_item.clone(), 1, cx);
        app.open_viewer(video_item.clone(), cx);
        assert!(app.is_fullscreen_viewer);
        assert_eq!(app.selected_item.as_ref().unwrap().id, video_item.id);
        // Set active player backend
        app.video.player = Some(crate::player::macos::AvFoundationPlayer::new());
        app.video.duration = 30.0;
        assert!(app.video.player.is_some());
    });

    // 2. Select photo item via filmstrip transition
    view.update(cx, |app, cx| {
        app.select_item_at(photo_item.clone(), 0, cx);
        app.open_viewer(photo_item.clone(), cx);
        assert!(app.is_fullscreen_viewer);
        assert_eq!(app.selected_item.as_ref().unwrap().id, photo_item.id);
        // Switching to photo properly tears down video player
        assert!(app.video.player.is_none());
    });

    // 3. Switch back to video item
    view.update(cx, |app, cx| {
        app.select_item_at(video_item.clone(), 1, cx);
        app.open_viewer(video_item.clone(), cx);
        assert!(app.is_fullscreen_viewer);
        assert_eq!(app.selected_item.as_ref().unwrap().id, video_item.id);
    });
}

#[core::prelude::v1::test]
fn test_viewer_player_ux_and_navigation_controls() {
    use crate::views::viewer::MediaViewer;

    let mut cx = gpui::TestAppContext::single();
    let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));

    // Deterministic library: navigation must not depend on the developer's real config.
    let temp_dir = std::env::temp_dir().join(format!(
        "lumina_viewer_nav_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    std::fs::write(temp_dir.join("photo1.jpg"), b"test-jpeg-content").unwrap();
    std::fs::write(temp_dir.join("photo2.jpg"), b"test-jpeg-content").unwrap();
    let (engine, rx) = lumina_core::LuminaEngine::open_with_paths(
        temp_dir.clone(),
        temp_dir.join("catalog.db"),
        Some(temp_dir.join("cache")),
    )
    .unwrap();
    while let Ok(event) = rx.recv() {
        if let lumina_core::EngineEvent::ScanComplete { .. } = event {
            break;
        }
    }
    assert!(engine.total_count() >= 2);

    let sample = get_sample_items();
    let photo_item = sample[0].clone();
    let video_item = sample[1].clone();

    view.update(cx, |app, cx| {
        app.grid_state = Some(crate::views::grid::GridState::new(engine.clone()));
        // 1. Open photo item in viewer
        app.select_item_at(photo_item.clone(), 0, cx);
        app.open_viewer(photo_item.clone(), cx);
        app.layout_mode = LayoutMode::Viewer;
        assert!(app.is_fullscreen_viewer);
        assert_eq!(app.layout_mode, LayoutMode::Viewer);

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

        // 3. Layout Mode Switcher in Viewer Top Bar:
        // Switching to Browser via close_viewer resets viewer state and restores Browser mode
        app.close_viewer(cx);
        assert!(!app.is_fullscreen_viewer);
        assert_eq!(app.layout_mode, LayoutMode::Browser);
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

            // Space on photo closes viewer
            app.handle_key_down(
                &KeyDownEvent {
                    keystroke: Keystroke::parse("space").unwrap(),
                    is_held: false,
                },
                window,
                cx,
            );
            assert!(
                !app.is_fullscreen_viewer,
                "Space on photo must close viewer"
            );

            // Open video in viewer
            app.select_item_at(video_item.clone(), 1, cx);
            app.open_viewer(video_item.clone(), cx);
            assert!(app.is_fullscreen_viewer);

            // Space on video toggles play/pause
            assert!(!app.video.is_playing);
            app.handle_key_down(
                &KeyDownEvent {
                    keystroke: Keystroke::parse("space").unwrap(),
                    is_held: false,
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
                },
                window,
                cx,
            );
            assert!(!app.is_fullscreen_viewer, "Second Escape must close viewer");
        });
    });
    cx.run_until_parked();
}
