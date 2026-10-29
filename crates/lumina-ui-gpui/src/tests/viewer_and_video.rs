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

fn press_viewer_key(
    app: &mut LuminaApp,
    key: &str,
    window: &mut gpui::Window,
    cx: &mut gpui::Context<LuminaApp>,
) {
    app.handle_key_down(
        &KeyDownEvent {
            keystroke: Keystroke::parse(key).expect("test keystroke must parse"),
            is_held: false,
        },
        window,
        cx,
    );
}

fn render_viewer_ui(cx: &mut gpui::VisualTestContext) {
    cx.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear();
    });
    cx.run_until_parked();
}

#[core::prelude::v1::test]
fn test_viewer_zoom_presets_hud_label_and_shortcuts() {
    use crate::views::viewer_state::{ViewerZoomPreset, ZOOM_MAX_SCALE, ZOOM_MIN_SCALE};

    let mut cx = gpui::TestAppContext::single();
    let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    let sample = get_sample_items();
    let photo = sample[0].clone();

    view.update(cx, |app, cx| {
        app.grid_state = None;
        app.select_item_at(photo.clone(), 0, cx);
        app.open_viewer(photo.clone(), cx);
        assert_eq!(app.viewer_state.zoom_label(), "Fit");

        // Preset actions drive both scale and HUD text.
        app.apply_viewer_zoom_preset(ViewerZoomPreset::TwoX, cx);
        assert_eq!(app.viewer_state.zoom_scale, 2.0);
        assert_eq!(app.viewer_state.zoom_label(), "200%");
        app.apply_viewer_zoom_preset(ViewerZoomPreset::FourX, cx);
        assert_eq!(app.viewer_state.zoom_scale, 4.0);
        assert_eq!(app.viewer_state.zoom_label(), "400%");
        app.apply_viewer_zoom_preset(ViewerZoomPreset::Fit, cx);
        assert_eq!(app.viewer_state.zoom_scale, ZOOM_MIN_SCALE);
        assert_eq!(app.viewer_state.zoom_label(), "Fit");

        // Reset restores fit and clears pan without leaving drag state behind.
        app.set_viewer_zoom(2.0, cx);
        app.start_viewer_drag(10.0, 10.0, cx);
        app.drag_viewer_to(40.0, 30.0, cx);
        assert_eq!(app.viewer_state.pan_offset, (30.0, 20.0));
        app.reset_viewer_zoom(cx);
        assert_eq!(app.viewer_state.zoom_scale, ZOOM_MIN_SCALE);
        assert_eq!(app.viewer_state.pan_offset, (0.0, 0.0));
        assert!(!app.viewer_state.is_dragging);

        // Existing stepping behavior is unchanged.
        app.zoom_in_viewer(cx);
        assert_eq!(app.viewer_state.zoom_scale, 1.25);
        app.zoom_out_viewer(cx);
        assert_eq!(app.viewer_state.zoom_scale, 1.0);
        app.set_viewer_zoom(99.0, cx);
        assert_eq!(app.viewer_state.zoom_scale, ZOOM_MAX_SCALE);
    });
    cx.run_until_parked();

    // Keyboard: added 1/2/4 presets plus preserved +, -, 0 and Z paths.
    cx.update(|window, app_cx| {
        view.update(app_cx, |app, cx| {
            app.select_item_at(photo.clone(), 0, cx);
            app.open_viewer(photo.clone(), cx);

            press_viewer_key(app, "2", window, cx);
            assert_eq!(app.viewer_state.zoom_scale, 2.0, "2 must select 2x");
            press_viewer_key(app, "4", window, cx);
            assert_eq!(app.viewer_state.zoom_scale, 4.0, "4 must select 4x");
            press_viewer_key(app, "1", window, cx);
            assert_eq!(app.viewer_state.zoom_scale, 1.0, "1 must select Fit");

            press_viewer_key(app, "=", window, cx);
            assert_eq!(app.viewer_state.zoom_scale, 1.25, "+ must still step in");
            press_viewer_key(app, "0", window, cx);
            assert_eq!(app.viewer_state.zoom_scale, 1.0, "0 must still reset");
            press_viewer_key(app, "z", window, cx);
            assert_eq!(
                app.viewer_state.zoom_scale, 2.0,
                "Z must still toggle to 2x"
            );
            press_viewer_key(app, "z", window, cx);
            assert_eq!(
                app.viewer_state.zoom_scale, 1.0,
                "Z must still toggle to Fit"
            );
            press_viewer_key(app, "-", window, cx);
            assert_eq!(
                app.viewer_state.zoom_scale, 1.0,
                "- must still step down and clamp at Fit"
            );
        });
    });
    cx.run_until_parked();
}

#[core::prelude::v1::test]
fn test_viewer_zoom_hud_does_not_overlap_filename_at_1024x768() {
    let temp_dir = std::env::temp_dir().join(format!(
        "lumina_viewer_zoom_hud_{}",
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
    while let Ok(event) = rx.recv_timeout(std::time::Duration::from_secs(10)) {
        if let lumina_core::EngineEvent::ScanComplete { .. } = event {
            break;
        }
    }
    assert!(engine.total_count() >= 2);

    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    let photo = get_sample_items()[0].clone();

    view.update(vcx, |app, cx| {
        app.grid_state = Some(crate::views::grid::GridState::new(engine.clone()));
        app.select_item_at(photo.clone(), 0, cx);
        app.open_viewer(photo.clone(), cx);
        app.layout_mode = LayoutMode::Viewer;
        app.window_bounds = Some((0.0, 0.0, 1024.0, 768.0));
        cx.notify();
    });
    vcx.simulate_resize(gpui::size(gpui::px(1024.0), gpui::px(768.0)));
    render_viewer_ui(vcx);
    // The bounds observer fires on the resize frame; draw again so the top bar
    // renders with the observed width.
    render_viewer_ui(vcx);

    for selector in [
        "viewer-top-bar",
        "viewer-filename",
        "viewer-zoom-hud",
        "viewer-zoom-label",
        "viewer-zoom-fit",
        "viewer-zoom-2x",
        "viewer-zoom-4x",
        "viewer-zoom-reset",
        "mode-browser",
        "mode-split",
        "mode-viewer",
    ] {
        assert!(
            vcx.debug_bounds(selector).is_some(),
            "{selector} must be painted in the viewer top bar at 1024x768"
        );
    }

    let filename = vcx.debug_bounds("viewer-filename").unwrap();
    let hud = vcx.debug_bounds("viewer-zoom-hud").unwrap();
    let top_bar = vcx.debug_bounds("viewer-top-bar").unwrap();
    assert!(
        filename.size.width > gpui::px(40.0),
        "filename must retain readable width at 1024x768, got {:?} (hud={hud:?}, top_bar={top_bar:?})",
        filename.size.width
    );
    assert!(
        hud.left() >= filename.right(),
        "zoom HUD must sit clear of the filename at 1024x768: filename={filename:?} hud={hud:?}"
    );
    assert!(
        hud.right() <= top_bar.right(),
        "zoom HUD must stay inside the top bar at 1024x768"
    );

    // HUD controls are wired to the same preset and reset actions as the keys.
    let zoom_2x = vcx.debug_bounds("viewer-zoom-2x").unwrap().center();
    vcx.simulate_click(zoom_2x, gpui::Modifiers::default());
    render_viewer_ui(vcx);
    view.update(vcx, |app, _cx| {
        assert_eq!(
            app.viewer_state.zoom_scale, 2.0,
            "clicking 2x must apply the preset"
        );
    });
    let zoom_reset = vcx.debug_bounds("viewer-zoom-reset").unwrap().center();
    vcx.simulate_click(zoom_reset, gpui::Modifiers::default());
    render_viewer_ui(vcx);
    view.update(vcx, |app, _cx| {
        assert_eq!(
            app.viewer_state.zoom_scale, 1.0,
            "clicking reset must return to Fit"
        );
    });

    // Threshold band: at 1400px the specs line and full action labels return.
    // The filename must keep its 70px floor and the HUD must stay clear rather
    // than collapsing the center section.
    view.update(vcx, |app, cx| {
        app.window_bounds = Some((0.0, 0.0, 1400.0, 768.0));
        cx.notify();
    });
    vcx.simulate_resize(gpui::size(gpui::px(1400.0), gpui::px(768.0)));
    render_viewer_ui(vcx);
    render_viewer_ui(vcx);
    assert!(
        vcx.debug_bounds("viewer-specs").is_some(),
        "specs line must return at 1400px"
    );
    let filename_wide = vcx.debug_bounds("viewer-filename").unwrap();
    let hud_wide = vcx.debug_bounds("viewer-zoom-hud").unwrap();
    assert!(
        filename_wide.size.width >= gpui::px(70.0),
        "filename must keep its width floor at 1400px, got {:?}",
        filename_wide.size.width
    );
    assert!(
        hud_wide.left() >= filename_wide.right(),
        "zoom HUD must stay clear of the filename at 1400px: filename={filename_wide:?} hud={hud_wide:?} specs={:?}",
        vcx.debug_bounds("viewer-specs")
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn test_viewer_cursor_anchored_zoom_action() {
    use crate::views::viewer_state::{ViewerZoomPreset, ZOOM_MAX_SCALE, ZOOM_MIN_SCALE, ZOOM_STEP};

    let mut cx = gpui::TestAppContext::single();
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
fn test_video_scrub_state_transitions_and_commit() {
    let mut cx = gpui::TestAppContext::single();
    let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    let video = get_sample_items()[1].clone();

    view.update(cx, |app, cx| {
        app.grid_state = None;
        app.select_item_at(video.clone(), 1, cx);
        app.open_viewer(video, cx);
        // No player backend is loaded here (the sample path does not exist), so
        // this exercises the state/seek-commit boundary directly. The drag
        // wiring test below runs the same path with a native player handle.
        app.video.duration = 42.0;

        assert!(!app.video.is_scrubbing, "viewer opens with no active scrub");
        assert!(app.video.scrub_time.is_none());

        // Pointer-down starts scrubbing and seeks to the pressed position.
        app.begin_video_scrub(0.25, cx);
        assert!(app.video.is_scrubbing);
        assert!((app.video.scrub_time.unwrap() - 10.5).abs() < 1e-6);
        assert!((app.video.playback_time - 10.5).abs() < 1e-6);

        // Pointer-move always advances the destination, even when the player
        // seek inside the throttle window is coalesced.
        app.update_video_scrub(0.75, cx);
        assert!(app.video.is_scrubbing);
        assert!((app.video.scrub_time.unwrap() - 31.5).abs() < 1e-6);
        assert!(
            (app.video.playback_time - 10.5).abs() < 1e-6,
            "a move inside one throttle window must not queue another seek"
        );

        // Pointer-up ends scrubbing and commits the final destination.
        app.end_video_scrub(cx);
        assert!(!app.video.is_scrubbing);
        assert!(app.video.scrub_time.is_none());
        assert!(app.video.last_scrub_seek.is_none());
        assert!((app.video.playback_time - 31.5).abs() < 1e-6);

        // Releasing again or moving without a drag is a no-op, not a stray seek.
        app.end_video_scrub(cx);
        app.update_video_scrub(0.1, cx);
        assert!(!app.video.is_scrubbing);
        assert!(app.video.scrub_time.is_none());
        assert!((app.video.playback_time - 31.5).abs() < 1e-6);
    });
    cx.run_until_parked();
}

#[core::prelude::v1::test]
fn test_video_scrub_clamps_to_duration_and_catalog_fallback() {
    let mut cx = gpui::TestAppContext::single();
    let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    let video = get_sample_items()[1].clone();

    view.update(cx, |app, cx| {
        app.grid_state = None;
        app.select_item_at(video.clone(), 1, cx);
        app.open_viewer(video, cx);

        // Player metadata is authoritative once known.
        app.video.duration = 42.0;
        app.video.playback_time = 0.0;
        app.begin_video_scrub(-0.25, cx);
        assert_eq!(app.video.scrub_time, Some(0.0));
        assert_eq!(app.video.playback_time, 0.0);
        app.update_video_scrub(1.5, cx);
        assert!((app.video.scrub_time.unwrap() - 42.0).abs() < 1e-6);
        app.end_video_scrub(cx);
        assert!((app.video.playback_time - 42.0).abs() < 1e-6);

        // Before player metadata arrives the catalog duration drives the track;
        // the sample video is 15 seconds.
        app.video.duration = 0.0;
        app.video.playback_time = 0.0;
        app.begin_video_scrub(2.0, cx);
        assert!((app.video.scrub_time.unwrap() - 15.0).abs() < 1e-6);
        app.end_video_scrub(cx);

        // A video with no duration at all clamps to zero instead of inventing one.
        if let Some(item) = app.selected_item.as_mut() {
            item.duration_ms = None;
        }
        app.video.duration = 0.0;
        app.video.playback_time = 3.0;
        app.begin_video_scrub(0.5, cx);
        assert_eq!(app.video.scrub_time, Some(0.0));
        assert_eq!(app.video.playback_time, 0.0);
    });
    cx.run_until_parked();
}

#[core::prelude::v1::test]
fn test_video_scrub_resets_on_viewer_close_and_media_change() {
    let mut cx = gpui::TestAppContext::single();
    let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    let sample = get_sample_items();
    let video = sample[1].clone();
    let photo = sample[0].clone();

    view.update(cx, |app, cx| {
        app.grid_state = None;
        app.select_item_at(video.clone(), 1, cx);
        app.open_viewer(video.clone(), cx);
        app.video.duration = 31.0;

        // Closing the viewer stops playback and clears the armed drag.
        app.begin_video_scrub(0.5, cx);
        assert!(app.video.is_scrubbing);
        app.close_viewer(cx);
        assert!(!app.video.is_scrubbing);
        assert!(app.video.scrub_time.is_none());
        assert!(app.video.last_scrub_seek.is_none());

        // Switching media tears the video player down; no stale scrub survives.
        app.open_viewer(video.clone(), cx);
        app.video.duration = 31.0;
        app.begin_video_scrub(0.25, cx);
        assert!(app.video.is_scrubbing);
        app.open_viewer(photo.clone(), cx);
        assert!(!app.video.is_scrubbing);
        assert!(app.video.scrub_time.is_none());

        // Stopping the player directly also clears scrub state.
        app.open_viewer(video, cx);
        app.video.duration = 31.0;
        app.begin_video_scrub(0.75, cx);
        assert!(app.video.is_scrubbing);
        app.stop_video_player();
        assert!(!app.video.is_scrubbing);
        assert!(app.video.scrub_time.is_none());
    });
    cx.run_until_parked();
}

#[core::prelude::v1::test]
fn test_video_scrub_pointer_drag_updates_destination_and_commits() {
    // The root render path shows onboarding when no library is open, so the
    // scrubber can only be verified through a real (temporary) library.
    let temp_dir = std::env::temp_dir().join(format!(
        "lumina_video_scrub_drag_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock must be after the Unix epoch")
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).expect("temp library dir must be creatable");
    std::fs::write(temp_dir.join("photo1.jpg"), b"test-jpeg-content")
        .expect("temp photo must be writable");
    std::fs::write(temp_dir.join("photo2.jpg"), b"test-jpeg-content")
        .expect("temp photo must be writable");
    let (engine, rx) = lumina_core::LuminaEngine::open_with_paths(
        temp_dir.clone(),
        temp_dir.join("catalog.db"),
        Some(temp_dir.join("cache")),
    )
    .expect("temp engine must open");
    while let Ok(event) = rx.recv_timeout(std::time::Duration::from_secs(10)) {
        if let lumina_core::EngineEvent::ScanComplete { .. } = event {
            break;
        }
    }
    assert!(engine.total_count() >= 2);

    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    let video = get_sample_items()[1].clone();

    view.update(vcx, |app, cx| {
        app.grid_state = Some(crate::views::grid::GridState::new(engine.clone()));
        app.select_item_at(video.clone(), 1, cx);
        app.open_viewer(video.clone(), cx);
        app.layout_mode = LayoutMode::Viewer;
        app.video.duration = 60.0;
        // A native player handle exists on macOS even without a loaded movie;
        // the drag path reaches the real seek bridge without requiring decoded
        // frames (the bridge exposes no still for arbitrary scrub positions).
        app.video.player = Some(crate::player::macos::AvFoundationPlayer::new());
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

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn test_video_resume_start_selection_is_bounded() {
    use crate::config::resume_start_time;

    assert_eq!(
        resume_start_time(Some(42.0), 120.0),
        42.0,
        "a saved mid-video position must be used"
    );
    assert_eq!(
        resume_start_time(Some(118.5), 120.0),
        0.0,
        "a position near the end must restart from the start"
    );
    assert_eq!(
        resume_start_time(Some(42.0), 30.0),
        0.0,
        "a position beyond the duration is invalid"
    );
    assert_eq!(resume_start_time(Some(0.0), 120.0), 0.0);
    assert_eq!(resume_start_time(Some(-5.0), 120.0), 0.0);
    assert_eq!(resume_start_time(Some(f64::NAN), 120.0), 0.0);
    assert_eq!(resume_start_time(None, 120.0), 0.0);
    assert_eq!(
        resume_start_time(Some(42.0), 0.0),
        0.0,
        "an unknown duration must not produce a seek target"
    );
}

#[core::prelude::v1::test]
fn test_video_resume_progress_throttle_and_close() {
    let mut cx = gpui::TestAppContext::single();
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
        app.video.player = Some(crate::player::macos::AvFoundationPlayer::new());
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
