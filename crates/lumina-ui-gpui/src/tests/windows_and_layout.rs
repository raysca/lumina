use crate::*;

#[core::prelude::v1::test]
fn test_floating_windows_lifecycle() {
    let mut cx = crate::tests::TestApp::single();
    let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));

    // 1. Open floating inspector via open_floating_inspector_window
    let weak_app = view.downgrade();
    let ui_tx = view.update(cx, |app, _| app.ui_tx.clone());
    cx.update(|_w, app_cx| {
        open_floating_inspector_window(weak_app.clone(), ui_tx.clone(), app_cx);
    });

    // Run event loop to ensure drawing and event dispatch succeed without panic
    cx.run_until_parked();

    view.update(cx, |app, _cx| {
        assert!(app.floating_inspector_window.is_some());
    });

    // 2. Redock inspector
    view.update(cx, |app, cx| {
        app.redock_inspector(cx);
        assert!(app.floating_inspector_window.is_none());
        assert!(app.inspector_open);
    });

    cx.run_until_parked();

    // 3. Open floating PiP player. The window only opens for a PiP session.
    cx.update(|_w, app_cx| {
        open_floating_pip_window(weak_app.clone(), ui_tx.clone(), app_cx);
    });
    cx.run_until_parked();
    view.update(cx, |app, _cx| {
        assert!(
            app.floating_pip_window.is_none(),
            "a PiP window must not open without a PiP session"
        );
        app.video.in_pip = true;
    });
    cx.update(|_w, app_cx| {
        open_floating_pip_window(weak_app.clone(), ui_tx.clone(), app_cx);
    });

    cx.run_until_parked();

    view.update(cx, |app, _cx| {
        assert!(app.floating_pip_window.is_some());
    });

    // 4. Close floating PiP
    view.update(cx, |app, cx| {
        let window = app.floating_pip_window.unwrap().window_id();
        app.floating_pip_closed(window, cx);
        assert!(app.floating_pip_window.is_none());
        assert!(!app.video.in_pip, "closing the window ends the PiP session");
    });

    cx.run_until_parked();
}

#[core::prelude::v1::test]
fn test_card_context_menu_lifecycle() {
    let mut cx = crate::tests::TestApp::single();
    let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));

    let temp_dir = std::env::temp_dir().join(format!(
        "lumina_ctx_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let photo_path = temp_dir.join("context_test.jpg");
    std::fs::write(&photo_path, b"test-ctx-content").unwrap();

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

    let items = engine.get_item_slice(0..10);
    assert_eq!(items.len(), 1);
    let test_item = items[0].clone();

    view.update(cx, |app, cx| {
        app.grid_state = Some(crate::views::grid::GridState::new(engine.clone()));
        app.refresh_album_counts();
        cx.notify();
    });
    cx.run_until_parked();

    // 1. Right click on card at (150, 200) -> opens context menu and selects item
    view.update(cx, |app, cx| {
        assert!(app.context_menu_state.is_none());
        app.open_card_context_menu(test_item.clone(), 0, point(px(150.0), px(200.0)), cx);
        assert!(app.context_menu_state.is_some());
        let st = app.context_menu_state.as_ref().unwrap();
        assert_eq!(st.position.x, px(150.0));
        assert_eq!(st.position.y, px(200.0));
        assert_eq!(st.item.id, test_item.id);
        assert!(app.is_index_selected(0));
    });
    cx.run_until_parked();

    // 2. Escape dismisses context menu
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
            assert!(app.context_menu_state.is_none());
        });
    });
    cx.run_until_parked();

    // 3. Re-open and toggle favorite via menu action
    view.update(cx, |app, cx| {
        app.open_card_context_menu(test_item.clone(), 0, point(px(50.0), px(60.0)), cx);
        assert!(app.context_menu_state.is_some());
        app.batch_toggle_favorite(cx);
        app.close_context_menu(cx);
        assert!(app.context_menu_state.is_none());
        assert!(app.selected_item.as_ref().unwrap().is_favorite);
    });
    cx.run_until_parked();

    // 4. Clipboard helper copy test
    view.update(cx, |app, cx| {
        let path_str = test_item.path.to_string_lossy().to_string();
        app.copy_to_clipboard(&path_str, cx);
    });
    cx.run_until_parked();

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[core::prelude::v1::test]
fn test_resizable_panes_and_viewer_open_close() {
    let mut cx = crate::tests::TestApp::single();
    let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));

    view.update(cx, |app, cx| {
        // Reset to defaults for deterministic unit testing
        app.sidebar_width = 230.0;
        app.inspector_width = 310.0;
        app.grid_state = None;

        // 1. Initial dimensions
        assert_eq!(app.sidebar_width, 230.0);
        assert_eq!(app.inspector_width, 310.0);
        assert!(!app.is_fullscreen_viewer);

        // 2. Resizing sidebar
        app.sidebar_width = 280.0;
        assert_eq!(app.sidebar_width, 280.0);

        // 3. Resizing inspector
        app.inspector_width = 350.0;
        assert_eq!(app.inspector_width, 350.0);

        // 4. Selecting does not open the viewer; opening is explicit.
        let sample = get_sample_items();
        let item = sample[2].clone();
        app.handle_card_click(item.clone(), 2, false, false, cx);
        assert!(
            !app.is_fullscreen_viewer,
            "a single card click must not open the fullscreen viewer"
        );
        assert_eq!(app.selected_index, Some(2));

        // 5. The viewer is the fullscreen overlay over the grid.
        app.open_viewer(item.clone(), cx);
        assert!(app.is_fullscreen_viewer);
        assert_eq!(app.selected_item.as_ref().map(|it| it.id), Some(item.id));

        // 6. Closing returns to the same grid selection.
        app.close_viewer(cx);
        assert!(!app.is_fullscreen_viewer);
        assert_eq!(app.selected_index, Some(2));
        assert_eq!(app.selected_item.as_ref().map(|it| it.id), Some(item.id));
    });
    cx.run_until_parked();
}

#[core::prelude::v1::test]
fn test_inspector_tabs_and_selection_marks() {
    let mut cx = crate::tests::TestApp::single();
    let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));

    view.update(cx, |app, cx| {
        app.grid_state = None;

        // 1. Tab switching
        assert_eq!(
            app.inspector_tab,
            crate::views::inspector::InspectorTab::Info
        );
        app.set_inspector_tab(crate::views::inspector::InspectorTab::Camera, cx);
        assert_eq!(
            app.inspector_tab,
            crate::views::inspector::InspectorTab::Camera
        );
        app.set_inspector_tab(crate::views::inspector::InspectorTab::Location, cx);
        assert_eq!(
            app.inspector_tab,
            crate::views::inspector::InspectorTab::Location
        );
        app.set_inspector_tab(crate::views::inspector::InspectorTab::Tags, cx);
        assert_eq!(
            app.inspector_tab,
            crate::views::inspector::InspectorTab::Tags
        );
        app.set_inspector_tab(crate::views::inspector::InspectorTab::Info, cx);

        // 2. Rating selection
        let sample = get_sample_items();
        app.selected_item = Some(sample[0].clone());
        app.selected_index = Some(0);
        app.selected_indices = [0].into_iter().collect();

        app.set_rating_selected(4, cx);
        assert_eq!(app.get_item_rating(&sample[0]), 4);
        // Clicking same rating clears it (toggle to 0)
        app.set_rating_selected(4, cx);
        assert_eq!(app.get_item_rating(&sample[0]), 0);

        // 3. Color label selection
        app.set_color_label_selected(2, cx);
        assert_eq!(app.item_color_labels.get(&sample[0].id).copied(), Some(2));
        // Clicking same label clears it (toggle to 0)
        app.set_color_label_selected(2, cx);
        assert_eq!(app.item_color_labels.get(&sample[0].id).copied(), None);

        // 4. Interactive caption input lifecycle
        assert!(!app.caption.input_active);
        app.start_caption_input(cx);
        assert!(app.caption.input_active);
        app.caption.input_buffer = "Golden hour in Kyoto".to_string();
        app.commit_caption_input(cx);
        assert!(!app.caption.input_active);
        assert_eq!(
            app.item_captions.get(&sample[0].id).map(|s| s.as_str()),
            Some("Golden hour in Kyoto")
        );
    });
    cx.run_until_parked();
}

#[core::prelude::v1::test]
fn test_card_click_selection_and_clean_startup() {
    let mut cx = crate::tests::TestApp::single();
    let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));

    view.update(cx, |app, cx| {
        // 1. Verify clean startup state
        assert!(app.context_menu_state.is_none());
        assert!(app.active_year.is_none());
        assert!(!app.is_fullscreen_viewer);
        assert!(app.selected_item.is_none());
        assert!(app.selected_indices.is_empty());

        // 2. Simulate single-click on card: selects item without hijacking to fullscreen viewer
        let sample = get_sample_items();
        let card_item = sample[2].clone();
        app.handle_card_click(card_item.clone(), 2, false, false, cx);

        assert_eq!(app.selected_index, Some(2));
        assert!(app.selected_indices.contains(&2));
        assert_eq!(
            app.selected_item.as_ref().map(|i| &i.file_name),
            Some(&card_item.file_name)
        );
        assert!(
            !app.is_fullscreen_viewer,
            "Single click must NOT open fullscreen viewer"
        );

        // 3. Opening viewer explicitly (double click / enter / space) activates fullscreen viewer
        app.open_viewer(card_item.clone(), cx);
        assert!(
            app.is_fullscreen_viewer,
            "Explicit open_viewer must activate fullscreen viewer"
        );

        // 4. Closing viewer returns cleanly to grid mode
        app.close_viewer(cx);
        assert!(!app.is_fullscreen_viewer);
        assert_eq!(app.selected_index, Some(2));
    });
}

#[core::prelude::v1::test]
fn test_grid_row_height_tracks_window_and_panel_chrome() {
    use crate::views::grid::grid_metrics;

    let mut cx = crate::tests::TestApp::single();
    let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));

    view.update(cx, |app, _cx| {
        app.grid_state = None;
        app.timeline_year_buckets.clear();
        app.selected_item = None;
        app.grid_columns = 4;
        app.sidebar_width = 230.0;
        app.sidebar_collapsed = false;
        app.inspector_open = false;
        app.is_fullscreen_viewer = false;
        app.window_bounds = Some((0.0, 0.0, 1280.0, 840.0));

        let base = grid_metrics(1280.0, 230.0, false, false, None, 4);
        assert_eq!(app.grid_metrics(), base);
        assert_eq!(app.row_height(), base.row_height);

        // The collapsed sidebar is a 48px rail with no resizer, so the grid
        // gets wider and the square well grows.
        app.sidebar_collapsed = true;
        let collapsed = grid_metrics(1280.0, 230.0, true, false, None, 4);
        assert_eq!(app.row_height(), collapsed.row_height);
        assert!(collapsed.row_height > base.row_height);

        // Density changes the card width, so it also changes the square side.
        app.grid_columns = 8;
        let dense = grid_metrics(1280.0, 230.0, true, false, None, 8);
        assert_eq!(app.row_height(), dense.row_height);
        assert!(dense.row_height < collapsed.row_height);

        // An open inspector with a selection narrows the grid; the fullscreen
        // viewer overlay hides the inspector again.
        app.selected_item = get_sample_items().into_iter().next();
        app.inspector_open = true;
        let with_inspector = grid_metrics(1280.0, 230.0, true, false, Some(app.inspector_width), 8);
        assert_eq!(app.row_height(), with_inspector.row_height);
        assert!(with_inspector.row_height < dense.row_height);

        app.is_fullscreen_viewer = true;
        assert_eq!(app.row_height(), dense.row_height);
        app.is_fullscreen_viewer = false;
        app.inspector_open = false;
        app.selected_item = None;

        // The year rail only takes space when the timeline has buckets.
        app.grid_columns = 2;
        let without_rail = grid_metrics(1280.0, 230.0, true, false, None, 2);
        assert_eq!(app.row_height(), without_rail.row_height);
        app.timeline_year_buckets = vec![(2024, 3)];
        let with_rail = grid_metrics(1280.0, 230.0, true, true, None, 2);
        assert_eq!(app.row_height(), with_rail.row_height);
        assert!(with_rail.row_height < without_rail.row_height);
    });

    cx.run_until_parked();
}

#[core::prelude::v1::test]
fn test_error_banner_lifecycle() {
    let mut cx = crate::tests::TestApp::single();
    let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));

    view.update(cx, |app, cx| {
        assert!(app.error_message.is_none());

        app.set_error("Could not open library: permission denied", cx);
        assert_eq!(
            app.error_message.as_deref(),
            Some("Could not open library: permission denied")
        );

        app.dismiss_error(cx);
        assert!(app.error_message.is_none());
    });

    cx.run_until_parked();
}

#[core::prelude::v1::test]
fn frameless_windows_get_their_own_window_controls() {
    use crate::app::render::FORCE_CLIENT_DECORATIONS;
    use crate::tests::ui_automation::{app_with_library, render_ui, seed_library};

    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            FORCE_CLIENT_DECORATIONS.with(|force| force.set(false));
        }
    }
    let _reset = Reset;

    let (temp_dir, engine) = seed_library("window_controls", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    assert!(
        vcx.debug_bounds("toolbar-drag-area").is_some(),
        "the toolbar keeps an empty area to drag the window by"
    );
    for control in ["window-minimize", "window-maximize", "window-close"] {
        assert!(
            vcx.debug_bounds(control).is_none(),
            "{control} must not draw over a window the platform decorates"
        );
    }

    FORCE_CLIENT_DECORATIONS.with(|force| force.set(true));
    view.update(vcx, |_, cx| cx.notify());
    render_ui(vcx);
    for control in ["window-minimize", "window-maximize", "window-close"] {
        assert!(
            vcx.debug_bounds(control).is_some(),
            "a client-decorated window must show {control}, or it cannot be managed"
        );
    }
    let _ = std::fs::remove_dir_all(&temp_dir);
}
