use crate::tests::ui_automation::{app_with_library, click, render_ui, seed_library};
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
        // The transport maps the pointer continuously; 0.0 is the mute step.
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
fn test_volume_slider_pointer_mapping_and_drag() {
    let (temp_dir, engine) = seed_library("volume_drag", 3);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    let video = get_sample_items()
        .into_iter()
        .find(|item| item.media_type == lumina_core::MediaType::Video)
        .expect("the sample set must contain a video");
    view.update(vcx, |app, cx| {
        app.selected_item = Some(video);
        app.selected_index = Some(0);
        app.selected_indices = [0].into_iter().collect();
        app.is_fullscreen_viewer = true;
        app.video.duration = 60.0;
        app.video.volume = 0.5;
        cx.notify();
    });
    render_ui(vcx);

    let bounds = vcx
        .debug_bounds("video-volume-slider")
        .expect("the transport must paint the volume slider");
    let left: f32 = bounds.origin.x.into();
    let width: f32 = bounds.size.width.into();
    let y: f32 = bounds.center().y.into();
    assert!(
        width > 40.0,
        "the volume hit area must be usable: {width}px"
    );
    let point_at = |fraction: f32| gpui::point(gpui::px(left + width * fraction), gpui::px(y));

    vcx.simulate_click(point_at(0.75), gpui::Modifiers::default());
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(
            (app.video.volume - 0.75).abs() < 0.03,
            "a click at 75% must set ~0.75 volume, got {}",
            app.video.volume
        );
    });

    vcx.simulate_mouse_down(
        point_at(0.75),
        gpui::MouseButton::Left,
        gpui::Modifiers::default(),
    );
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.video.adjusting_volume, "press must arm the volume drag");
    });
    vcx.simulate_mouse_move(
        point_at(0.20),
        Some(gpui::MouseButton::Left),
        gpui::Modifiers::default(),
    );
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(
            (app.video.volume - 0.20).abs() < 0.03,
            "dragging to 20% must follow the pointer, got {}",
            app.video.volume
        );
    });
    vcx.simulate_mouse_up(
        point_at(0.20),
        gpui::MouseButton::Left,
        gpui::Modifiers::default(),
    );
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(!app.video.adjusting_volume, "release must end the drag");
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
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
        app.video.player = Some(crate::player::NativeVideoPlayer::new());
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

            // 3000px long edge on a 2x screen over a 900pt fit: 100% = 1.667x.
            app.viewer_state.canvas = crate::views::viewer_state::ViewerCanvas::new(
                (1200.0, 600.0),
                (600.0, 300.0),
                1.5,
                3000.0,
                2.0,
            );
            let actual = 1500.0 / 900.0;
            press_viewer_key(app, "1", window, cx);
            assert!(
                (app.viewer_state.zoom_scale - actual).abs() < 1e-4,
                "1 must select actual pixels"
            );
            assert_eq!(app.viewer_state.zoom_label(), "100%");
            press_viewer_key(app, "2", window, cx);
            assert!(
                (app.viewer_state.zoom_scale - 2.0 * actual).abs() < 1e-4,
                "2 must select 200% of actual pixels"
            );
            assert_eq!(app.viewer_state.zoom_label(), "200%");
            press_viewer_key(app, "4", window, cx);
            assert_eq!(app.viewer_state.zoom_label(), "400%", "4 must select 400%");
            press_viewer_key(app, "0", window, cx);
            assert_eq!(app.viewer_state.zoom_scale, 1.0, "0 must select Fit");

            press_viewer_key(app, "=", window, cx);
            assert_eq!(app.viewer_state.zoom_scale, 1.25, "+ must still step in");
            press_viewer_key(app, "0", window, cx);
            assert_eq!(app.viewer_state.zoom_scale, 1.0, "0 must still reset");
            press_viewer_key(app, "z", window, cx);
            assert!(
                (app.viewer_state.zoom_scale - actual).abs() < 1e-4,
                "Z must toggle Fit to actual pixels"
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
fn test_viewer_zoom_hud_floats_below_the_top_bar() {
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
        "viewer-back",
    ] {
        assert!(
            vcx.debug_bounds(selector).is_some(),
            "{selector} must be painted in the viewer at 1024x768"
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
    // v3: the zoom pill floats over the stage, below the top bar, so it can
    // never crowd the filename.
    assert!(
        hud.top() >= top_bar.bottom(),
        "zoom HUD must float below the top bar at 1024x768: hud={hud:?} top_bar={top_bar:?}"
    );
    assert!(
        hud.left() >= top_bar.left() && hud.right() <= top_bar.right(),
        "zoom HUD must stay inside the window width at 1024x768"
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
        hud_wide.top() >= vcx.debug_bounds("viewer-top-bar").unwrap().bottom(),
        "zoom HUD must stay below the top bar at 1400px: hud={hud_wide:?}"
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

/// Builds a temporary library with a real catalog so rotation persistence and
/// thumbnail invalidation are exercised end to end.
fn rotation_test_library(
    prefix: &str,
) -> (
    std::path::PathBuf,
    std::sync::Arc<lumina_core::LuminaEngine>,
) {
    let temp_dir = std::env::temp_dir().join(format!(
        "lumina_{prefix}_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock must be after the Unix epoch")
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).expect("temp library dir must be creatable");
    for index in 0..2 {
        std::fs::write(
            temp_dir.join(format!("photo{index}.jpg")),
            b"test-jpeg-content",
        )
        .expect("temp photo must be writable");
    }
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
    (temp_dir, engine)
}

#[core::prelude::v1::test]
fn test_viewer_rotate_controls_persist_and_invalidate_thumbnail() {
    use crate::views::grid::GridState;

    let (temp_dir, engine) = rotation_test_library("viewer_rotate_controls");
    let item = engine
        .get_item_slice(0..1)
        .into_iter()
        .next()
        .expect("scanned photo must be cataloged");
    let item_id = item.id;

    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));

    let texture = std::sync::Arc::new(gpui::RenderImage::new(vec![image::Frame::new(
        image::RgbaImage::new(4, 4),
    )]));
    view.update(vcx, |app, cx| {
        app.grid_state = Some(GridState::new(engine.clone()));
        app.select_item_at(item.clone(), 0, cx);
        app.open_viewer(item.clone(), cx);
        app.window_bounds = Some((0.0, 0.0, 1440.0, 900.0));
        let state = app
            .grid_state
            .as_ref()
            .expect("grid state must be installed");
        state.texture_cache.lock().put(item_id, texture.clone());
        app.viewer_image = Some(texture);
        cx.notify();
    });
    vcx.simulate_resize(gpui::size(gpui::px(1440.0), gpui::px(900.0)));
    render_viewer_ui(vcx);
    render_viewer_ui(vcx);

    assert_eq!(
        engine
            .get_rotation(item_id)
            .expect("rotation read must work"),
        0,
        "a scanned photo must start unrotated"
    );

    let left = vcx
        .debug_bounds("quick-edit-rotate-left")
        .expect("the viewer top bar must paint the Rotate Left control");
    let right = vcx
        .debug_bounds("quick-edit-rotate-right")
        .expect("the viewer top bar must paint the Rotate Right control");

    vcx.simulate_click(right.center(), gpui::Modifiers::default());
    render_viewer_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            engine
                .get_rotation(item_id)
                .expect("rotation read must work"),
            1,
            "Rotate Right must persist one clockwise quarter turn"
        );
        let state = app
            .grid_state
            .as_ref()
            .expect("grid state must be installed");
        assert!(
            !state.texture_cache.lock().contains(&item_id),
            "the cached thumbnail must be invalidated so the grid re-decodes"
        );
        assert!(
            app.viewer_image.is_none(),
            "the viewer image must be cleared for the rotated re-request"
        );
        assert_eq!(app.last_action.as_deref(), Some("Rotated right"));
    });

    vcx.simulate_click(right.center(), gpui::Modifiers::default());
    render_viewer_ui(vcx);
    vcx.simulate_click(left.center(), gpui::Modifiers::default());
    render_viewer_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            engine
                .get_rotation(item_id)
                .expect("rotation read must work"),
            1,
            "right, right, left must land back on one quarter turn"
        );
        assert!(
            app.error_message.is_none(),
            "rotation must not raise a banner"
        );
    });

    // The context menu exposes the same quick edits. The viewer is closed so the
    // shared debug selector is unique to the menu (the top bar is not painted).
    view.update(vcx, |app, cx| {
        app.close_viewer(cx);
        app.open_card_context_menu(
            item.clone(),
            0,
            gpui::point(gpui::px(40.0), gpui::px(40.0)),
            cx,
        );
        cx.notify();
    });
    render_viewer_ui(vcx);
    let menu_right = vcx
        .debug_bounds("quick-edit-rotate-right")
        .expect("the context menu must offer Rotate Right");
    vcx.simulate_click(menu_right.center(), gpui::Modifiers::default());
    render_viewer_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            engine
                .get_rotation(item_id)
                .expect("rotation read must work"),
            2,
            "the context-menu rotate control must persist the same edit"
        );
        assert!(
            app.context_menu_state.is_none(),
            "the context menu must close after rotating"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn test_viewer_rotate_video_is_a_clear_no_op() {
    use crate::views::viewer_state::ViewerRotation;

    let mut cx = gpui::TestAppContext::single();
    let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    let video = get_sample_items()[1].clone();

    view.update(cx, |app, cx| {
        app.grid_state = None;
        app.select_item_at(video.clone(), 1, cx);
        app.open_viewer(video, cx);
        app.rotate_viewer_selected(ViewerRotation::Right, cx);
        assert_eq!(
            app.last_action.as_deref(),
            Some("Rotation is not supported for video"),
            "video rotation must report a clear no-op"
        );
        assert!(
            app.error_message.is_none(),
            "an unsupported video rotation is not an error"
        );
    });
    cx.run_until_parked();
}

/// Synthetic double-click at a window-space position. The click count is read
/// from the mouse-up event, matching the platform driver used by the app.
fn double_click_at(cx: &mut gpui::VisualTestContext, position: gpui::Point<gpui::Pixels>) {
    cx.simulate_event(gpui::MouseDownEvent {
        position,
        modifiers: gpui::Modifiers::default(),
        button: gpui::MouseButton::Left,
        click_count: 2,
        first_mouse: false,
    });
    cx.simulate_event(gpui::MouseUpEvent {
        position,
        modifiers: gpui::Modifiers::default(),
        button: gpui::MouseButton::Left,
        click_count: 2,
    });
    render_viewer_ui(cx);
}

/// Opens the overlay viewer with a known gradient texture so the painted media
/// element has deterministic bounds independent of real decoder output.
fn viewer_with_gradient<'a>(
    cx: &'a mut gpui::TestAppContext,
    prefix: &str,
    width: f32,
    height: f32,
) -> (
    std::path::PathBuf,
    gpui::Entity<LuminaApp>,
    &'a mut gpui::VisualTestContext,
) {
    let (temp_dir, engine) = seed_library(prefix, 4);
    let (view, vcx) = app_with_library(cx, engine);
    let photo = view.update(vcx, |app, _| {
        app.get_active_slice(0..1)
            .into_iter()
            .next()
            .expect("seeded library must have an item")
    });
    view.update(vcx, |app, app_cx| {
        app.open_viewer(photo, app_cx);
        // The seeded JPEG payload is not a real image, so this gradient stays
        // the displayed texture for the whole test.
        app.viewer_image = Some(generate_gradient_image(
            (40, 60, 90),
            (10, 15, 30),
            800,
            600,
        ));
        app.window_bounds = Some((0.0, 0.0, width, height));
        app_cx.notify();
    });
    vcx.simulate_resize(gpui::size(gpui::px(width), gpui::px(height)));
    render_viewer_ui(vcx);
    render_viewer_ui(vcx);
    (temp_dir, view, vcx)
}

#[core::prelude::v1::test]
fn test_viewer_zoom_changes_painted_media_bounds() {
    let mut cx = gpui::TestAppContext::single();
    let (temp_dir, view, vcx) = viewer_with_gradient(&mut cx, "viewer_zoom_paint", 1440.0, 900.0);

    let fit = vcx
        .debug_bounds("viewer-media")
        .expect("the Fit viewer must paint the media element");
    assert!(
        f32::from(fit.size.width) > 0.0 && f32::from(fit.size.height) > 0.0,
        "test premise: the Fit media must have painted bounds, got {fit:?}"
    );

    click(vcx, "viewer-zoom-2x");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.viewer_state.zoom_scale, 2.0,
            "clicking 2x must apply the 2x preset"
        );
    });

    let zoomed = vcx
        .debug_bounds("viewer-media")
        .expect("the 2x viewer must paint the media element");
    assert!(
        f32::from(zoomed.size.width) > f32::from(fit.size.width) * 1.5,
        "2x must enlarge the painted media width, not only zoom_scale: fit={fit:?} zoomed={zoomed:?}"
    );
    assert!(
        f32::from(zoomed.size.height) > f32::from(fit.size.height) * 1.5,
        "2x must enlarge the painted media height, not only zoom_scale: fit={fit:?} zoomed={zoomed:?}"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn test_viewer_double_click_zoom_is_media_bounded() {
    let mut cx = gpui::TestAppContext::single();
    let (temp_dir, view, vcx) = viewer_with_gradient(&mut cx, "viewer_double_click", 1440.0, 900.0);

    let viewport = vcx
        .debug_bounds("viewer-viewport")
        .expect("the viewer must paint its viewport");
    let media = vcx
        .debug_bounds("viewer-media")
        .expect("the viewer must paint the media element");

    // Left edge of the viewport, vertically centred: outside the 800x600 media
    // that sits in the middle of the 1440px canvas.
    let letterbox = gpui::point(viewport.left() + gpui::px(8.0), viewport.center().y);
    // The media element fills the canvas; the visible picture is the
    // contained rect from the captured canvas geometry.
    view.update(vcx, |app, _| {
        assert!(
            !app.viewer_state.hits_media(letterbox),
            "test premise: the letterbox point must be outside the visible media; point={letterbox:?} canvas={:?}",
            app.viewer_state.canvas
        );
    });

    double_click_at(vcx, letterbox);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.viewer_state.zoom_scale, 1.0,
            "a double-click in the letterbox around the media must not zoom"
        );
    });

    double_click_at(vcx, media.center());
    view.update(vcx, |app, _| {
        assert_eq!(
            app.viewer_state.zoom_scale, 2.0,
            "a double-click on the media itself must toggle the zoom"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn test_inspector_filename_wraps_inside_the_open_drawer() {
    let (temp_dir, engine) = seed_library("inspector_filename_wrap", 4);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let photo = view.update(vcx, |app, _| {
        app.get_active_slice(0..1)
            .into_iter()
            .next()
            .expect("seeded library must have an item")
    });
    let long_name =
        "Screenshot_2026-10-30_at_10.00.00_AM_with_a_very_long_descriptive_suffix_that_cannot_fit.png";
    view.update(vcx, |app, app_cx| {
        app.open_viewer(photo, app_cx);
        app.inspector_open = true;
        app.window_bounds = Some((0.0, 0.0, 1440.0, 900.0));
        if let Some(item) = app.selected_item.as_mut() {
            item.file_name = long_name.to_string();
        }
        app_cx.notify();
    });
    vcx.simulate_resize(gpui::size(gpui::px(1440.0), gpui::px(900.0)));
    render_viewer_ui(vcx);
    render_viewer_ui(vcx);

    let drawer = vcx
        .debug_bounds("viewer-info-drawer")
        .expect("an open inspector must paint the info drawer");
    let name = vcx
        .debug_bounds("inspector-filename")
        .expect("the inspector hero must paint the filename");
    assert!(
        name.left() >= drawer.left() && name.right() <= drawer.right(),
        "a long filename must stay inside the drawer instead of widening it: name={name:?} drawer={drawer:?}"
    );
    // text_xs is 12px with GPUI's 1.618 line height, so one line is ~19px;
    // more than 24px proves the name wrapped onto multiple lines.
    assert!(
        f32::from(name.size.height) > 24.0,
        "a long filename must wrap to more than one line, got {name:?}"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn replaced_viewer_frames_are_evicted_but_cached_thumbnails_are_kept() {
    use crate::player::VideoFrame;
    let frame = || VideoFrame {
        width: 2,
        height: 2,
        data: vec![0; 16],
    };
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));

    let cached = crate::bridge::video_frame_to_gpui_image(frame()).expect("2x2 BGRA frame");
    view.update(vcx, |app, _| {
        app.set_viewer_image(Some(crate::bridge::video_frame_to_gpui_image(frame()).unwrap()));
        app.set_viewer_image(Some(crate::bridge::video_frame_to_gpui_image(frame()).unwrap()));
        assert_eq!(
            app.retired_images.len(),
            1,
            "a replaced video frame must be queued for atlas eviction, or GPU memory grows every frame"
        );
        app.set_viewer_image(Some(cached.clone()));
        app.set_viewer_image(None);
        assert_eq!(
            app.retired_images.len(),
            2,
            "an image still held by the thumbnail cache must keep its texture"
        );
    });
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(
            app.retired_images.is_empty(),
            "render must drain the eviction queue"
        );
    });
}

#[core::prelude::v1::test]
fn scrub_and_volume_drags_keep_tracking_after_leaving_the_control() {
    let (temp_dir, engine) = crate::tests::ui_automation::seed_library("drag_off_control", 2);
    let mut cx = gpui::TestAppContext::single();
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

    // Volume: press at 25%, then drift 60px above the 24px slider to 90%.
    let slider = vcx
        .debug_bounds("video-volume-slider")
        .expect("volume slider must be painted in the viewer");
    let (left, width): (f32, f32) = (slider.left().into(), slider.size.width.into());
    let y: f32 = slider.center().y.into();
    vcx.simulate_mouse_down(
        gpui::point(gpui::px(left + width * 0.25), gpui::px(y)),
        gpui::MouseButton::Left,
        gpui::Modifiers::default(),
    );
    render_viewer_ui(vcx);
    let off_slider = gpui::point(gpui::px(left + width * 0.9), gpui::px(y - 60.0));
    vcx.simulate_mouse_move(off_slider, left_button, gpui::Modifiers::default());
    render_viewer_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(
            (app.video.volume - 0.9).abs() < 0.05,
            "a volume drag must follow the pointer off the slider, got {}",
            app.video.volume
        );
    });
    vcx.simulate_mouse_up(
        off_slider,
        gpui::MouseButton::Left,
        gpui::Modifiers::default(),
    );
    render_viewer_ui(vcx);
    view.update(vcx, |app, _| assert!(!app.video.adjusting_volume));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn test_viewer_fills_canvas_and_offers_actual_pixels() {
    let mut cx = gpui::TestAppContext::single();
    let (temp_dir, view, vcx) = viewer_with_gradient(&mut cx, "viewer_actual", 1440.0, 900.0);
    view.update(vcx, |app, cx| {
        // A 4000x3000 original shown through the 800x600 gradient decode.
        let item = app.selected_item.as_mut().expect("viewer has an item");
        item.width = 4000;
        item.height = 3000;
        cx.notify();
    });
    render_viewer_ui(vcx);

    view.update(vcx, |app, cx| {
        let canvas = app
            .viewer_state
            .canvas
            .expect("painting the viewer must capture canvas geometry");
        assert!(
            canvas.fitted.0 > 800.0 || canvas.fitted.1 > 600.0,
            "Fit must enlarge media to the canvas like Photos, got {:?}",
            canvas.fitted
        );
        assert!(
            (canvas.fitted.0 - canvas.size.0).abs() < 0.5
                || (canvas.fitted.1 - canvas.size.1).abs() < 0.5,
            "Fit must touch the canvas on one axis, got {canvas:?}"
        );

        app.apply_viewer_zoom_preset(crate::views::viewer_state::ViewerZoomPreset::Actual, cx);
        assert!(
            (app.viewer_state.zoom_scale - canvas.actual_scale).abs() < 1e-4,
            "100% must map one source pixel to one screen pixel"
        );
        assert_eq!(app.viewer_state.zoom_label(), "100%");

        app.viewer_state.pan_by(100_000.0, 100_000.0);
        let max_x = (canvas.fitted.0 * app.viewer_state.zoom_scale - canvas.size.0) * 0.5;
        assert!(
            (app.viewer_state.pan_offset.0 - max_x).abs() < 0.5,
            "pan must stop at the media edge, not run off-screen"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn pip_window_evicts_retired_frames_while_the_main_window_is_not_drawing() {
    use crate::player::VideoFrame;
    let frame = || {
        crate::bridge::video_frame_to_gpui_image(VideoFrame {
            width: 2,
            height: 2,
            data: vec![0; 16],
        })
        .expect("2x2 BGRA frame")
    };
    let mut cx = gpui::TestAppContext::single();
    // The main app has no window here, standing in for an occluded main
    // window whose display link has stopped.
    let app = cx.new(LuminaApp::new);
    let ui_tx = app.read_with(&cx, |app, _| app.ui_tx.clone());
    let (_pip, vcx) = cx.add_window_view(|window, cx| {
        crate::views::floating_pip::FloatingPipPlayerView::new(app.downgrade(), ui_tx, window, cx)
    });

    app.update(vcx, |app, _| {
        app.set_viewer_image(Some(frame()));
        app.set_viewer_image(Some(frame()));
        assert_eq!(app.retired_images.len(), 1);
    });
    render_ui(vcx);
    app.update(vcx, |app, _| {
        assert!(
            app.retired_images.is_empty(),
            "PiP playback must evict retired frames even when the main window is not drawing"
        );
    });
}

fn tiny_decoded_frame(id: lumina_core::MediaId) -> lumina_core::DecodedFrame {
    lumina_core::DecodedFrame {
        id,
        width: 2,
        height: 2,
        format: lumina_core::PixelFormat::Rgba8,
        data: vec![0; 16],
    }
}

/// Regression: playing with every window minimized retired a frame per tick,
/// but only a render drained the queue, so memory grew ~270 MB/s.
#[core::prelude::v1::test]
fn retired_frames_are_evicted_without_any_window_drawing() {
    let mut cx = gpui::TestAppContext::single();
    let app = cx.new(LuminaApp::new);
    let item = get_sample_items()[0].clone();
    let ui_tx = app.update(&mut cx, |app, _| {
        app.selected_item = Some(item.clone());
        app.ui_tx.clone()
    });
    for _ in 0..3 {
        ui_tx
            .send_blocking(UiMessage::ViewerLoaded {
                id: item.id,
                frame: tiny_decoded_frame(item.id),
            })
            .unwrap();
    }
    cx.run_until_parked();
    app.read_with(&cx, |app, _| {
        assert!(app.viewer_image.is_some(), "the latest frame must be shown");
        assert!(
            app.retired_images.is_empty(),
            "replaced frames must be evicted as messages arrive, not only when a window draws"
        );
    });
}

/// Regression: thumbnails evicted from the LRU (or removed with their item)
/// dropped their Arc without freeing the GPU texture, so every scroll pass
/// through the library grew GPU memory by ~240 MB.
#[core::prelude::v1::test]
fn evicted_and_removed_thumbnails_are_queued_for_gpu_eviction() {
    let (temp_dir, engine) = seed_library("thumb_evict", 2);
    let (other_dir, _other) = seed_library("thumb_evict_other", 1);
    // Removes both libraries even when an assertion below fails.
    struct Cleanup(Vec<std::path::PathBuf>);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            for dir in &self.0 {
                let _ = std::fs::remove_dir_all(dir);
            }
        }
    }
    let _cleanup = Cleanup(vec![temp_dir.clone(), other_dir.clone()]);
    let mut cx = gpui::TestAppContext::single();
    let app = cx.new(LuminaApp::new);
    app.update(&mut cx, |app, cx| {
        let state = crate::views::grid::GridState::new(engine.clone());
        state
            .texture_cache
            .lock()
            .resize(std::num::NonZeroUsize::new(2).unwrap());
        app.grid_state = Some(state);
        for id in 1..=3 {
            let id = lumina_core::MediaId(id);
            app.handle_ui_message(
                UiMessage::ThumbnailLoaded {
                    id,
                    frame: tiny_decoded_frame(id),
                },
                cx,
            );
        }
        assert_eq!(
            app.retired_images.len(),
            1,
            "the thumbnail pushed out of the LRU must be queued for GPU eviction"
        );
        app.handle_ui_message(
            UiMessage::Engine(lumina_core::EngineEvent::ItemRemoved(lumina_core::MediaId(
                3,
            ))),
            cx,
        );
        assert_eq!(
            app.retired_images.len(),
            2,
            "a removed item's thumbnail must be queued for GPU eviction"
        );

        // Rotating re-decodes the thumbnail, so the stale one must go too.
        let survivor = app
            .grid_state
            .as_ref()
            .and_then(|state| state.texture_cache.lock().peek_lru().map(|(id, _)| *id))
            .expect("one thumbnail is still cached");
        app.selected_item = engine.get_item_by_id(survivor);
        assert!(
            app.selected_item.is_some(),
            "the cached item is in the catalog"
        );
        app.rotate_viewer_selected(crate::views::viewer_state::ViewerRotation::Right, cx);
        assert_eq!(
            app.retired_images.len(),
            3,
            "rotation must queue the stale thumbnail for GPU eviction"
        );

        // Switching libraries drops the whole old texture cache.
        let id = lumina_core::MediaId(9);
        app.handle_ui_message(
            UiMessage::ThumbnailLoaded {
                id,
                frame: tiny_decoded_frame(id),
            },
            cx,
        );
        app.open_library_inner(other_dir.clone(), cx);
        assert!(
            other_dir.join("catalog.db").exists(),
            "tests must keep catalogs in the temp library, never the user's data dir"
        );
        assert_eq!(
            app.retired_images.len(),
            4,
            "opening another library must queue the old library's thumbnails for GPU eviction"
        );
    });
}
