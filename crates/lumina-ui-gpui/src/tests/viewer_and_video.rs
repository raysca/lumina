use crate::tests::ui_automation::{
    app_with_library, click, focus_app, keystrokes, render_ui, seed_library, type_text,
};
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
fn test_immersive_fullscreen_mode_lifecycle() {
    let mut cx = crate::tests::TestApp::single();
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

    // Keys reach the viewer only with a library open (the welcome screen
    // gates every key it does not handle).
    let (dir, engine) = seed_library("viewer_journey", 1);
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
    let _ = std::fs::remove_dir_all(&dir);
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
    let mut cx = crate::tests::TestApp::single();
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

    let mut cx = crate::tests::TestApp::single();
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
    std::fs::write(
        temp_dir.join("photo1.jpg"),
        crate::tests::ui_automation::tiny_jpeg(),
    )
    .unwrap();
    std::fs::write(
        temp_dir.join("photo2.jpg"),
        crate::tests::ui_automation::tiny_jpeg(),
    )
    .unwrap();
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

fn press_viewer_key(
    app: &mut LuminaApp,
    key: &'static str,
    window: &mut gpui::Window,
    cx: &mut gpui::Context<LuminaApp>,
) {
    let key = crate::tests::ui_automation::platform_test_keys(key);
    let key: &'static str = Box::leak(key.into_boxed_str());
    app.handle_key_down(
        &KeyDownEvent {
            keystroke: Keystroke::parse(key).expect("test keystroke must parse"),
            is_held: false,
            prefer_character_input: false,
        },
        window,
        cx,
    );
}

pub(crate) fn render_viewer_ui(cx: &mut gpui::VisualTestContext) {
    cx.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
    cx.run_until_parked();
}

#[core::prelude::v1::test]
fn test_viewer_zoom_presets_hud_label_and_shortcuts() {
    use crate::views::viewer_state::{ViewerZoomPreset, ZOOM_MAX_SCALE, ZOOM_MIN_SCALE};

    // Keys reach the viewer only with a library open (the welcome screen
    // gates every key it does not handle).
    let (dir, engine) = seed_library("viewer_zoom_keys", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, cx) = app_with_library(&mut cx, engine);
    let sample = get_sample_items();
    let photo = sample[0].clone();

    view.update(cx, |app, cx| {
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

    // Keyboard: ⌘1/⌘2/⌘4 presets plus preserved +, -, ⌘0 and Z paths.
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
            press_viewer_key(app, "cmd-1", window, cx);
            assert!(
                (app.viewer_state.zoom_scale - actual).abs() < 1e-4,
                "⌘1 must select actual pixels"
            );
            assert_eq!(app.viewer_state.zoom_label(), "100%");
            press_viewer_key(app, "cmd-2", window, cx);
            assert!(
                (app.viewer_state.zoom_scale - 2.0 * actual).abs() < 1e-4,
                "⌘2 must select 200% of actual pixels"
            );
            assert_eq!(app.viewer_state.zoom_label(), "200%");
            press_viewer_key(app, "cmd-4", window, cx);
            assert_eq!(app.viewer_state.zoom_label(), "400%", "⌘4 must select 400%");
            press_viewer_key(app, "cmd-0", window, cx);
            assert_eq!(app.viewer_state.zoom_scale, 1.0, "⌘0 must select Fit");

            press_viewer_key(app, "=", window, cx);
            assert_eq!(app.viewer_state.zoom_scale, 1.25, "+ must still step in");
            press_viewer_key(app, "cmd-0", window, cx);
            assert_eq!(app.viewer_state.zoom_scale, 1.0, "⌘0 must still reset");
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
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn test_viewer_fit_button_floats_below_the_top_bar() {
    let temp_dir = std::env::temp_dir().join(format!(
        "lumina_viewer_zoom_hud_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    std::fs::write(
        temp_dir.join("photo1.jpg"),
        crate::tests::ui_automation::tiny_jpeg(),
    )
    .unwrap();
    std::fs::write(
        temp_dir.join("photo2.jpg"),
        crate::tests::ui_automation::tiny_jpeg(),
    )
    .unwrap();
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

    let mut cx = crate::tests::TestApp::single();
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

    for selector in ["viewer-top-bar", "viewer-position", "viewer-back"] {
        assert!(
            vcx.debug_bounds(selector).is_some(),
            "{selector} must be painted in the viewer at 1024x768"
        );
    }
    assert!(
        vcx.debug_bounds("viewer-zoom-fit").is_none(),
        "at Fit there is no zoom chrome (v3)"
    );
    view.update(vcx, |app, cx| {
        app.apply_viewer_zoom_preset(crate::views::viewer_state::ViewerZoomPreset::TwoX, cx)
    });
    render_viewer_ui(vcx);

    let position = vcx.debug_bounds("viewer-position").unwrap();
    let hud = vcx
        .debug_bounds("viewer-zoom-fit")
        .expect("zoomed, the Fit to window button must show");
    let top_bar = vcx.debug_bounds("viewer-top-bar").unwrap();
    // The centre shows only "N / M" (the date moved to the
    // panel's When line); it must stay readable at 1024x768.
    assert!(
        position.size.width > gpui::px(20.0),
        "the position must retain readable width at 1024x768, got {:?}",
        position.size.width
    );
    assert_eq!(
        f32::from(hud.size.width),
        40.0,
        "Fit is a 40px round button"
    );
    assert!(
        hud.top() >= top_bar.bottom(),
        "the Fit button must float below the top bar: hud={hud:?} top_bar={top_bar:?}"
    );
    assert!(
        hud.left() >= top_bar.left() && hud.right() <= top_bar.right(),
        "the Fit button must stay inside the window width at 1024x768"
    );

    // At 1400px the top bar and title continue to render cleanly.
    view.update(vcx, |app, cx| {
        app.window_bounds = Some((0.0, 0.0, 1400.0, 768.0));
        cx.notify();
    });
    vcx.simulate_resize(gpui::size(gpui::px(1400.0), gpui::px(768.0)));
    render_viewer_ui(vcx);
    render_viewer_ui(vcx);
    let position_wide = vcx.debug_bounds("viewer-position").unwrap();
    assert_eq!(
        position_wide.size.width, position.size.width,
        "the position keeps its width at 1400px"
    );

    click(vcx, "viewer-zoom-fit");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.viewer_state.zoom_scale, 1.0,
            "Fit to window must return to Fit"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn test_viewer_cursor_anchored_zoom_action() {
    use crate::views::viewer_state::{ViewerZoomPreset, ZOOM_MAX_SCALE, ZOOM_MIN_SCALE, ZOOM_STEP};

    let mut cx = crate::tests::TestApp::single();
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
    let mut cx = crate::tests::TestApp::single();
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
    let mut cx = crate::tests::TestApp::single();
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
    let mut cx = crate::tests::TestApp::single();
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
    std::fs::write(
        temp_dir.join("photo1.jpg"),
        crate::tests::ui_automation::tiny_jpeg(),
    )
    .expect("temp photo must be writable");
    std::fs::write(
        temp_dir.join("photo2.jpg"),
        crate::tests::ui_automation::tiny_jpeg(),
    )
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

/// Builds a temporary library with a real catalog so rotation persistence and
/// thumbnail invalidation are exercised end to end.
fn rotation_test_library(
    prefix: &str,
    files: &[&str],
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
    // Not images: the rotation tests check that a thumbnail is dropped, so
    // no re-decode may land during the test (every decode of these fails).
    for file in files {
        std::fs::write(temp_dir.join(file), b"test-jpeg-content")
            .expect("temp media file must be writable");
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

    let (temp_dir, engine) =
        rotation_test_library("viewer_rotate_controls", &["photo0.jpg", "photo1.jpg"]);
    let item = engine
        .get_item_slice(0..1)
        .into_iter()
        .next()
        .expect("scanned photo must be cataloged");
    let item_id = item.id;

    let mut cx = crate::tests::TestApp::single();
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

    // Rotation lives in the viewer's right-click menu (v3 top bar is slim).
    let rotate = |vcx: &mut gpui::VisualTestContext, selector: &'static str| {
        let viewport = vcx.debug_bounds("viewer-viewport").expect("viewport");
        vcx.simulate_event(gpui::MouseDownEvent {
            position: viewport.center(),
            modifiers: gpui::Modifiers::default(),
            button: gpui::MouseButton::Right,
            click_count: 1,
            first_mouse: false,
        });
        render_viewer_ui(vcx);
        let control = vcx
            .debug_bounds(selector)
            .expect("the viewer right-click menu must offer rotation");
        vcx.simulate_click(control.center(), gpui::Modifiers::default());
        render_viewer_ui(vcx);
    };
    assert!(
        vcx.debug_bounds("quick-edit-rotate-left").is_none(),
        "the v3 viewer top bar has no rotate buttons"
    );

    rotate(vcx, "quick-edit-rotate-right");
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

    rotate(vcx, "quick-edit-rotate-right");
    rotate(vcx, "quick-edit-rotate-left");
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

    let mut cx = crate::tests::TestApp::single();
    let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    let video = get_sample_items()[1].clone();

    view.update(cx, |app, cx| {
        app.grid_state = None;
        app.select_item_at(video.clone(), 1, cx);
        app.open_viewer(video, cx);
        app.rotate_selection(ViewerRotation::Right, cx);
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

/// Three photos and a clip; returns the photo ids (in catalog order) and the
/// clip's id.
fn rotation_batch_library(
    prefix: &str,
) -> (
    std::path::PathBuf,
    std::sync::Arc<lumina_core::LuminaEngine>,
    Vec<lumina_core::MediaId>,
    lumina_core::MediaId,
) {
    let (dir, engine) = rotation_test_library(prefix, &["a.jpg", "b.jpg", "c.jpg", "clip.mp4"]);
    let items = engine.get_item_slice(0..4);
    assert_eq!(items.len(), 4, "three photos and a clip must be cataloged");
    let photos = items
        .iter()
        .filter(|item| item.media_type != lumina_core::MediaType::Video)
        .map(|item| item.id)
        .collect();
    let video = items
        .iter()
        .find(|item| item.media_type == lumina_core::MediaType::Video)
        .expect("the clip must be cataloged as video")
        .id;
    (dir, engine, photos, video)
}

fn rotations(engine: &lumina_core::LuminaEngine, ids: &[lumina_core::MediaId]) -> Vec<u8> {
    ids.iter()
        .map(|id| engine.get_rotation(*id).expect("rotation read must work"))
        .collect()
}

/// Opens the viewer on the first photo, with the app focused.
fn open_rotation_viewer(
    view: &gpui::Entity<LuminaApp>,
    vcx: &mut gpui::VisualTestContext,
    engine: &lumina_core::LuminaEngine,
    id: lumina_core::MediaId,
) {
    let item = engine.get_item_by_id(id).expect("photo must be cataloged");
    view.update(vcx, |app, cx| {
        app.select_item_at(item.clone(), 0, cx);
        app.open_viewer(item, cx);
    });
    render_viewer_ui(vcx);
    focus_app(vcx, view);
}

#[core::prelude::v1::test]
fn rotate_selection_by_shortcut_persists_all_and_undoes_once() {
    let (dir, engine, photos, video) = rotation_batch_library("rotate_batch_shortcut");
    // Different starting angles, so undo must restore each item's own value.
    let seeded = [0_u8, 1, 3];
    for (id, quarters) in photos.iter().zip(seeded) {
        engine
            .set_rotations(&[(*id, quarters)])
            .expect("seed rotation");
    }
    engine
        .set_rotations(&[(video, 2)])
        .expect("seed video rotation");

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    keystrokes(vcx, "cmd-a");
    keystrokes(vcx, "r");

    assert_eq!(
        rotations(&engine, &photos),
        vec![1, 2, 0],
        "R must turn every selected photo one quarter clockwise"
    );
    assert_eq!(
        engine.get_rotation(video).expect("rotation read"),
        2,
        "R must leave a selected video untouched"
    );
    view.update(vcx, |app, _| {
        assert!(app.error_message.is_none(), "{:?}", app.error_message);
        assert_eq!(
            app.undo.undo_stack.len(),
            1,
            "a batch rotation is one undo step"
        );
        // A thumbnail painted at the new angle, which undo must drop.
        let texture = std::sync::Arc::new(gpui::RenderImage::new(vec![image::Frame::new(
            image::RgbaImage::new(4, 4),
        )]));
        app.grid_state
            .as_ref()
            .expect("grid state must be installed")
            .texture_cache
            .lock()
            .put(photos[0], texture);
    });

    keystrokes(vcx, "cmd-z");
    view.update(vcx, |app, _| {
        let state = app.grid_state.as_ref().expect("grid state");
        assert!(
            !state.texture_cache.lock().contains(&photos[0]),
            "undo must drop the thumbnail painted at the undone angle"
        );
    });
    assert_eq!(
        rotations(&engine, &photos),
        seeded.to_vec(),
        "one ⌘Z must restore each photo's own previous rotation"
    );
    assert_eq!(engine.get_rotation(video).expect("rotation read"), 2);

    // Redo turns them again and must drop what was painted at the old angle.
    put_test_texture(&view, vcx, photos[0]);
    keystrokes(vcx, "cmd-shift-z");
    assert_eq!(
        rotations(&engine, &photos),
        vec![1, 2, 0],
        "⇧⌘Z must reapply each photo's own rotation"
    );
    view.update(vcx, |app, _| {
        let state = app.grid_state.as_ref().expect("grid state");
        assert!(
            !state.texture_cache.lock().contains(&photos[0]),
            "redo must drop the thumbnail painted at the redone angle"
        );
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn shift_r_rotates_left_in_viewer() {
    let (dir, engine, photos, _video) = rotation_batch_library("rotate_shift_r_viewer");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_rotation_viewer(&view, vcx, &engine, photos[0]);

    keystrokes(vcx, "shift-r");
    assert_eq!(
        rotations(&engine, &photos),
        vec![3, 0, 0],
        "⇧R in the viewer must turn only the open photo counter-clockwise"
    );
    view.update(vcx, |app, _| {
        assert!(
            app.is_fullscreen_viewer,
            "rotating must keep the viewer open"
        );
        assert_eq!(app.undo.undo_stack.len(), 1);
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn rotate_from_palette_in_viewer() {
    let (dir, engine, photos, _video) = rotation_batch_library("rotate_palette_viewer");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_rotation_viewer(&view, vcx, &engine, photos[0]);

    keystrokes(vcx, "cmd-k");
    type_text(vcx, "rotate right");
    keystrokes(vcx, "enter");
    assert_eq!(
        rotations(&engine, &photos),
        vec![1, 0, 0],
        "⌘K Rotate Right must turn the open photo clockwise"
    );
    view.update(vcx, |app, _| {
        assert!(
            !app.command_palette_open,
            "running a command closes the palette"
        );
        assert!(app.is_fullscreen_viewer);
    });

    let _ = std::fs::remove_dir_all(&dir);
}

/// Caches a placeholder thumbnail for `id`, as if painted at its angle.
fn put_test_texture(
    view: &gpui::Entity<LuminaApp>,
    vcx: &mut gpui::VisualTestContext,
    id: lumina_core::MediaId,
) {
    let texture = std::sync::Arc::new(gpui::RenderImage::new(vec![image::Frame::new(
        image::RgbaImage::new(4, 4),
    )]));
    view.update(vcx, |app, _| {
        app.grid_state
            .as_ref()
            .expect("grid state must be installed")
            .texture_cache
            .lock()
            .put(id, texture);
    });
}

/// Asserts nothing was rotated and no undo step was recorded.
fn assert_not_rotated(
    view: &gpui::Entity<LuminaApp>,
    vcx: &mut gpui::VisualTestContext,
    engine: &lumina_core::LuminaEngine,
    photos: &[lumina_core::MediaId],
    why: &str,
) {
    assert_eq!(rotations(engine, photos), vec![0, 0, 0], "{why}");
    view.update(vcx, |app, _| {
        assert!(app.undo.undo_stack.is_empty(), "{why}: no undo step");
    });
}

#[core::prelude::v1::test]
fn image_menu_rotate_is_refused_behind_sheets() {
    let (dir, engine, photos, _video) = rotation_batch_library("rotate_menu_shield");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    keystrokes(vcx, "cmd-a");

    view.update(vcx, |app, cx| app.open_preferences(cx));
    render_ui(vcx);
    vcx.dispatch_action(crate::actions::RotateRight);
    vcx.run_until_parked();
    assert_not_rotated(
        &view,
        vcx,
        &engine,
        &photos,
        "Image > Rotate must not turn the selection hidden behind Preferences",
    );
    view.update(vcx, |app, cx| {
        app.close_preferences(cx);
        app.open_tag_manager(cx);
    });
    render_ui(vcx);
    vcx.dispatch_action(crate::actions::RotateRight);
    vcx.run_until_parked();
    assert_not_rotated(
        &view,
        vcx,
        &engine,
        &photos,
        "Image > Rotate on the Tags page has no media selection to turn",
    );

    view.update(vcx, |app, cx| {
        // The Tags page dropped the selection; select everything again.
        app.select_album(crate::views::sidebar::AlbumSelection::AllLibrary, cx);
        app.select_all(cx);
        app.import.dialog = Some(crate::app::import::ImportDialog::Planning {
            count: 1,
            checked: None,
        });
    });
    render_ui(vcx);
    vcx.dispatch_action(crate::actions::RotateRight);
    vcx.run_until_parked();
    assert_not_rotated(
        &view,
        vcx,
        &engine,
        &photos,
        "Image > Rotate must not turn the selection hidden behind Import…",
    );

    view.update(vcx, |app, _| app.import.dialog = None);
    render_ui(vcx);
    focus_app(vcx, &view);
    keystrokes(vcx, "cmd-f");
    vcx.dispatch_action(crate::actions::RotateRight);
    vcx.run_until_parked();
    assert_not_rotated(
        &view,
        vcx,
        &engine,
        &photos,
        "Image > Rotate must not act while the search field has focus",
    );

    // With nothing open the same menu action rotates the selection.
    view.update(vcx, |app, cx| app.blur_search(cx));
    vcx.dispatch_action(crate::actions::RotateRight);
    vcx.run_until_parked();
    assert_eq!(
        rotations(&engine, &photos),
        vec![1, 1, 1],
        "Image > Rotate Right turns the selection when no sheet is open"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn rotate_is_refused_in_recently_deleted() {
    let (dir, engine, photos, _video) = rotation_batch_library("rotate_in_trash");
    engine
        .batch_set_trashed(&photos, true)
        .expect("photos must move to Recently Deleted");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    view.update(vcx, |app, cx| {
        app.select_album(crate::AlbumSelection::Trash, cx)
    });
    render_ui(vcx);
    keystrokes(vcx, "cmd-a");
    keystrokes(vcx, "r");
    view.update(vcx, |app, _| {
        assert!(
            !app.selected_indices.is_empty(),
            "⌘A selects the deleted photos"
        )
    });
    assert_not_rotated(
        &view,
        vcx,
        &engine,
        &photos,
        "R must not rotate in Recently Deleted, where the context menu hides Rotate",
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn option_r_does_not_rotate_in_grid() {
    let (dir, engine, photos, _video) = rotation_batch_library("rotate_option_r");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    keystrokes(vcx, "cmd-a");
    keystrokes(vcx, "alt-r");
    assert_not_rotated(&view, vcx, &engine, &photos, "only R and ⇧R rotate, not ⌥R");

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn r_in_text_field_does_not_rotate() {
    let (dir, engine, photos, _video) = rotation_batch_library("rotate_r_in_search");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    keystrokes(vcx, "cmd-a");
    keystrokes(vcx, "cmd-f");
    keystrokes(vcx, "r");
    keystrokes(vcx, "shift-r");

    view.update(vcx, |app, _| {
        assert!(app.search_focused, "⌘F focuses the search field");
        assert_eq!(
            app.search_query, "rR",
            "R and ⇧R type into the search field"
        );
        assert!(
            app.undo.undo_stack.is_empty(),
            "typing must not record a rotation"
        );
    });
    assert_eq!(
        rotations(&engine, &photos),
        vec![0, 0, 0],
        "typing R in a text field must not rotate the selection"
    );

    let _ = std::fs::remove_dir_all(&dir);
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
pub(crate) fn viewer_with_gradient<'a>(
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
    let mut cx = crate::tests::TestApp::single();
    let (temp_dir, view, vcx) = viewer_with_gradient(&mut cx, "viewer_zoom_paint", 1440.0, 900.0);

    let fit = vcx
        .debug_bounds("viewer-media")
        .expect("the Fit viewer must paint the media element");
    assert!(
        f32::from(fit.size.width) > 0.0 && f32::from(fit.size.height) > 0.0,
        "test premise: the Fit media must have painted bounds, got {fit:?}"
    );

    assert!(
        vcx.debug_bounds("viewer-navigator").is_none(),
        "the navigator only shows while zoomed"
    );
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-2");
    view.update(vcx, |app, _| {
        assert!(app.viewer_state.zoom_scale > 1.0, "⌘2 must zoom in");
    });

    // Navigator: 180px, top right of the stage, amber rect on the visible part.
    let nav = vcx
        .debug_bounds("viewer-navigator")
        .expect("zoomed, the navigator must show");
    let stage = vcx.debug_bounds("viewer-viewport").unwrap();
    assert_eq!(f32::from(nav.size.width), 180.0);
    assert!(
        nav.right() <= stage.right() && stage.right() - nav.right() <= gpui::px(20.0),
        "the navigator sits at the top right: nav={nav:?} stage={stage:?}"
    );
    let region = vcx.debug_bounds("viewer-navigator-region").unwrap();
    let (_, _, w, h) = view.update(vcx, |app, _| app.viewer_state.visible_region().unwrap());
    assert!(
        w < 1.0 && h <= 1.0,
        "zoomed, only part of the picture is visible"
    );
    assert!(
        // Inside the navigator's 1px border.
        (f32::from(region.size.width) - 178.0 * w).abs() < 1.0,
        "the amber rect spans the visible fraction: region={region:?} w={w}"
    );
    assert!(
        nav.contains(&region.origin) && region.right() <= nav.right() + gpui::px(0.5),
        "the region stays inside the navigator"
    );

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

    // With the docked info panel open the navigator keeps its 16px inset
    // from the stage's right edge, which is the panel's left edge.
    view.update(vcx, |app, cx| {
        app.inspector_open = true;
        cx.notify();
    });
    render_viewer_ui(vcx);
    let nav = vcx.debug_bounds("viewer-navigator").unwrap();
    let drawer = vcx
        .debug_bounds("viewer-info-drawer")
        .expect("the info drawer must paint");
    assert_eq!(
        drawer.left() - nav.right(),
        gpui::px(16.0),
        "the navigator sits 16px left of the docked panel: nav={nav:?} drawer={drawer:?}"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn test_viewer_double_click_zoom_is_media_bounded() {
    let mut cx = crate::tests::TestApp::single();
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
    let mut cx = crate::tests::TestApp::single();
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
    let mut cx = crate::tests::TestApp::single();
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
fn scrub_drag_keeps_tracking_after_leaving_the_control() {
    let (temp_dir, engine) = crate::tests::ui_automation::seed_library("drag_off_control", 2);
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

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn test_viewer_fills_canvas_and_offers_actual_pixels() {
    let mut cx = crate::tests::TestApp::single();
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
    let mut cx = crate::tests::TestApp::single();
    // The main app has no window here, standing in for an occluded main
    // window whose display link has stopped.
    let app = cx.new(LuminaApp::new);
    let ui_tx = app.read_with(&*cx, |app, _| app.ui_tx.clone());
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

pub(super) fn tiny_decoded_frame(id: lumina_core::MediaId) -> lumina_core::DecodedFrame {
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
    let mut cx = crate::tests::TestApp::single();
    let app = cx.new(LuminaApp::new);
    let item = get_sample_items()[0].clone();
    let ui_tx = app.update(&mut *cx, |app, _| {
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
    app.read_with(&*cx, |app, _| {
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
    let mut cx = crate::tests::TestApp::single();
    let app = cx.new(LuminaApp::new);
    app.update(&mut *cx, |app, cx| {
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
        app.rotate_selection(crate::views::viewer_state::ViewerRotation::Right, cx);
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

#[core::prelude::v1::test]
fn test_viewer_fits_a_photo_inside_the_stage_margins() {
    let mut cx = crate::tests::TestApp::single();
    let (temp_dir, view, vcx) = viewer_with_gradient(&mut cx, "viewer_fit_tall", 1440.0, 900.0);
    // V3Photo fits into at most 1200x760 of the 1440x848 stage, clear of the
    // prev/next buttons, instead of running edge to edge.
    view.update(vcx, |app, cx| {
        app.viewer_image = Some(generate_gradient_image(
            (40, 60, 90),
            (10, 15, 30),
            1600,
            1600,
        ));
        cx.notify();
    });
    render_viewer_ui(vcx);
    render_viewer_ui(vcx);

    let viewport = vcx.debug_bounds("viewer-viewport").unwrap();
    let media = vcx.debug_bounds("viewer-media").unwrap();
    assert!(
        media.top() - viewport.top() >= gpui::px(40.0)
            && viewport.bottom() - media.bottom() >= gpui::px(40.0)
            && media.left() - viewport.left() >= gpui::px(80.0),
        "Fit must leave the design's stage margins: viewport={viewport:?} media={media:?}"
    );
    let (fit_w, fit_h) = view.update(vcx, |app, _| {
        app.viewer_state
            .canvas
            .expect("canvas geometry is captured")
            .fitted
    });
    assert!(
        (fit_w - fit_h).abs() < 1.0 && fit_h <= 761.0,
        "a square photo fits as a square at most 760 tall: {fit_w}x{fit_h}"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn test_open_info_drawer_never_covers_the_photo_or_its_controls() {
    let mut cx = crate::tests::TestApp::single();
    let (temp_dir, view, vcx) = viewer_with_gradient(&mut cx, "viewer_drawer_clear", 1440.0, 900.0);
    view.update(vcx, |app, cx| {
        app.inspector_open = true;
        cx.notify();
    });
    render_viewer_ui(vcx);
    render_viewer_ui(vcx);

    let drawer = vcx.debug_bounds("viewer-info-drawer").unwrap();
    assert_eq!(
        (drawer.top(), drawer.right(), drawer.size.height),
        (gpui::px(0.0), gpui::px(1440.0), gpui::px(900.0)),
        "the info panel is docked full height at the right edge: {drawer:?}"
    );
    for selector in ["viewer-media", "viewer-next", "viewer-viewport"] {
        let b = vcx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("{selector} renders"));
        assert!(
            b.right() <= drawer.left(),
            "the info drawer must sit beside {selector}, not over it: {b:?} drawer={drawer:?}"
        );
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}

/// Sends a trackpad pinch at `position` after moving the pointer there, since
/// `on_pinch` only fires on the hovered element.
fn pinch(cx: &mut gpui::VisualTestContext, position: gpui::Point<gpui::Pixels>, delta: f32) {
    cx.simulate_mouse_move(position, None, gpui::Modifiers::default());
    cx.simulate_event(gpui::PinchEvent {
        position,
        delta,
        modifiers: gpui::Modifiers::default(),
        phase: gpui::TouchPhase::Moved,
    });
    render_viewer_ui(cx);
}

fn scroll_px(dx: f32, dy: f32) -> gpui::ScrollDelta {
    gpui::ScrollDelta::Pixels(gpui::point(gpui::px(dx), gpui::px(dy)))
}

/// The content point (in unzoomed canvas units) under window point `p`.
fn content_under(app: &LuminaApp, p: gpui::Point<gpui::Pixels>) -> (f32, f32) {
    let c = app.viewer_state.canvas.expect("painted canvas geometry");
    let s = app.viewer_state.zoom_scale;
    let (px, py) = app.viewer_state.pan_offset;
    (
        (f32::from(p.x) - c.center.0 - px) / s,
        (f32::from(p.y) - c.center.1 - py) / s,
    )
}

/// Zooms to 2x first, so neither axis is pinned by the pan clamp (at small
/// zoom the media is narrower than the canvas and x pan clamps to 0).
fn zoom_to_two_x(view: &gpui::Entity<LuminaApp>, vcx: &mut gpui::VisualTestContext) {
    view.update(vcx, |app, cx| {
        app.apply_viewer_zoom_preset(crate::views::viewer_state::ViewerZoomPreset::TwoX, cx)
    });
    render_viewer_ui(vcx);
}

/// An off-centre point inside the Fit media (about 1013x760 in a 1440x900
/// window) and the canvas, whatever the zoom.
fn off_centre_media_point(vcx: &mut gpui::VisualTestContext) -> gpui::Point<gpui::Pixels> {
    let c = vcx
        .debug_bounds("viewer-viewport")
        .expect("the viewer must paint its viewport")
        .center();
    gpui::point(c.x + gpui::px(200.0), c.y - gpui::px(120.0))
}

#[core::prelude::v1::test]
fn viewer_pinch_keeps_content_under_pinch_center() {
    let mut cx = crate::tests::TestApp::single();
    let (temp_dir, view, vcx) = viewer_with_gradient(&mut cx, "viewer_pinch_anchor", 1440.0, 900.0);
    zoom_to_two_x(&view, vcx);
    let at = off_centre_media_point(vcx);
    let (scale, before) = view.update(vcx, |app, _| {
        (app.viewer_state.zoom_scale, content_under(app, at))
    });

    pinch(vcx, at, 0.2);
    view.update(vcx, |app, _| {
        assert!(
            (app.viewer_state.zoom_scale - scale * 1.2).abs() < 1e-4,
            "a 0.2 pinch must zoom by 20%, got {}",
            app.viewer_state.zoom_scale
        );
        let after = content_under(app, at);
        assert!(
            (after.0 - before.0).abs() < 0.5 && (after.1 - before.1).abs() < 0.5,
            "the content under the pinch centre must stay put: {before:?} -> {after:?}"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn viewer_pinch_ignored_for_video() {
    let mut cx = crate::tests::TestApp::single();
    let (temp_dir, view, vcx) = viewer_with_gradient(&mut cx, "viewer_pinch_video", 1440.0, 900.0);
    view.update(vcx, |app, cx| {
        app.selected_item
            .as_mut()
            .expect("viewer has an item")
            .media_type = lumina_core::MediaType::Video;
        cx.notify();
    });
    render_viewer_ui(vcx);
    let at = off_centre_media_point(vcx);

    pinch(vcx, at, 0.3);
    crate::tests::ui_automation::wheel(vcx, at, scroll_px(40.0, 60.0), gpui::Modifiers::default());
    crate::tests::ui_automation::wheel(
        vcx,
        at,
        scroll_px(0.0, 60.0),
        crate::tests::ui_automation::primary_modifier(),
    );
    view.update(vcx, |app, _| {
        assert_eq!(
            app.viewer_state.zoom_scale,
            crate::views::viewer_state::ZOOM_MIN_SCALE,
            "pinch and scroll must never zoom a video"
        );
        assert_eq!(
            app.viewer_state.pan_offset,
            (0.0, 0.0),
            "a video never pans"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn pinch_out_to_fit_recenters() {
    let mut cx = crate::tests::TestApp::single();
    let (temp_dir, view, vcx) = viewer_with_gradient(&mut cx, "viewer_pinch_fit", 1440.0, 900.0);
    let at = off_centre_media_point(vcx);

    pinch(vcx, at, 0.5);
    pinch(vcx, at, 0.5);
    crate::tests::ui_automation::wheel(vcx, at, scroll_px(25.0, -15.0), gpui::Modifiers::default());
    view.update(vcx, |app, _| {
        assert!(
            app.viewer_state.zoom_scale > 2.0,
            "two pinches must zoom in"
        );
        assert_ne!(
            app.viewer_state.pan_offset,
            (0.0, 0.0),
            "test premise: panned"
        );
    });
    for _ in 0..6 {
        pinch(vcx, at, -0.5);
    }
    view.update(vcx, |app, _| {
        assert_eq!(
            app.viewer_state.zoom_scale,
            crate::views::viewer_state::ZOOM_MIN_SCALE,
            "pinching out past Fit must stop at Fit"
        );
        assert_eq!(app.viewer_state.pan_offset, (0.0, 0.0), "Fit must recenter");
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn viewer_scroll_pans_when_zoomed_and_clamps_like_drag() {
    let mut cx = crate::tests::TestApp::single();
    let (temp_dir, view, vcx) = viewer_with_gradient(&mut cx, "viewer_scroll_pan", 1440.0, 900.0);
    zoom_to_two_x(&view, vcx);
    let at = off_centre_media_point(vcx);
    let scale = view.update(vcx, |app, _| app.viewer_state.zoom_scale);

    // Trackpad: pixel deltas pan one to one.
    crate::tests::ui_automation::wheel(vcx, at, scroll_px(30.0, -20.0), gpui::Modifiers::default());
    view.update(vcx, |app, _| {
        assert_eq!(
            app.viewer_state.zoom_scale, scale,
            "a plain scroll must not zoom"
        );
        assert_eq!(
            app.viewer_state.pan_offset,
            (30.0, -20.0),
            "a pixel scroll must pan by its delta"
        );
    });

    // Mouse wheel: line deltas pan by the window line height per line.
    let line = vcx.update(|window, _| f32::from(window.line_height()));
    crate::tests::ui_automation::wheel(
        vcx,
        at,
        gpui::ScrollDelta::Lines(gpui::point(0.0, 1.0)),
        gpui::Modifiers::default(),
    );
    view.update(vcx, |app, _| {
        assert!(
            (app.viewer_state.pan_offset.1 - (-20.0 + line)).abs() < 1e-3,
            "one wheel line must pan by one line height, got {:?}",
            app.viewer_state.pan_offset
        );
    });

    // A huge scroll stops at the media edge, the same clamp as drag-pan.
    crate::tests::ui_automation::wheel(
        vcx,
        at,
        scroll_px(100_000.0, 100_000.0),
        gpui::Modifiers::default(),
    );
    view.update(vcx, |app, _| {
        let c = app.viewer_state.canvas.expect("painted canvas geometry");
        let max_x = ((c.fitted.0 * scale - c.size.0) * 0.5).max(0.0);
        let max_y = ((c.fitted.1 * scale - c.size.1) * 0.5).max(0.0);
        let (x, y) = app.viewer_state.pan_offset;
        assert!(
            (x - max_x).abs() < 0.5 && (y - max_y).abs() < 0.5,
            "scroll-pan must stop at the media edge ({max_x}, {max_y}), got ({x}, {y})"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn viewer_scroll_at_fit_does_nothing() {
    let mut cx = crate::tests::TestApp::single();
    let (temp_dir, view, vcx) = viewer_with_gradient(&mut cx, "viewer_scroll_fit", 1440.0, 900.0);
    let at = off_centre_media_point(vcx);
    crate::tests::ui_automation::wheel(vcx, at, scroll_px(30.0, 80.0), gpui::Modifiers::default());
    crate::tests::ui_automation::wheel(
        vcx,
        at,
        gpui::ScrollDelta::Lines(gpui::point(0.0, 3.0)),
        gpui::Modifiers::default(),
    );
    view.update(vcx, |app, _| {
        assert_eq!(
            app.viewer_state.zoom_scale,
            crate::views::viewer_state::ZOOM_MIN_SCALE,
            "a plain scroll at Fit must not zoom"
        );
        assert_eq!(app.viewer_state.pan_offset, (0.0, 0.0), "Fit must not pan");
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn viewer_cmd_scroll_zooms_anchored_at_pointer() {
    let mut cx = crate::tests::TestApp::single();
    let (temp_dir, view, vcx) = viewer_with_gradient(&mut cx, "viewer_cmd_scroll", 1440.0, 900.0);
    zoom_to_two_x(&view, vcx);
    let at = off_centre_media_point(vcx);
    let (scale, before) = view.update(vcx, |app, _| {
        (app.viewer_state.zoom_scale, content_under(app, at))
    });

    // A mouse-wheel notch with ⌘ zooms (one line is ~13%).
    crate::tests::ui_automation::wheel(
        vcx,
        at,
        gpui::ScrollDelta::Lines(gpui::point(0.0, 1.0)),
        crate::tests::ui_automation::primary_modifier(),
    );
    view.update(vcx, |app, _| {
        assert!(
            app.viewer_state.zoom_scale > scale * 1.1,
            "⌘ + wheel must zoom in, got {}",
            app.viewer_state.zoom_scale
        );
        let after = content_under(app, at);
        assert!(
            (after.0 - before.0).abs() < 0.5 && (after.1 - before.1).abs() < 0.5,
            "⌘-scroll zoom must keep the content under the pointer: {before:?} -> {after:?}"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
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
