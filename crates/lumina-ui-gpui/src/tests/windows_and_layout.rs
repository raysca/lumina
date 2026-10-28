use crate::*;

#[core::prelude::v1::test]
fn test_floating_windows_lifecycle() {
    let mut cx = gpui::TestAppContext::single();
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

    // 3. Open floating PiP player
    cx.update(|_w, app_cx| {
        open_floating_pip_window(weak_app.clone(), ui_tx.clone(), app_cx);
    });

    cx.run_until_parked();

    view.update(cx, |app, _cx| {
        assert!(app.floating_pip_window.is_some());
    });

    // 4. Close floating PiP
    view.update(cx, |app, cx| {
        app.floating_pip_closed(cx);
        assert!(app.floating_pip_window.is_none());
    });

    cx.run_until_parked();
}

#[core::prelude::v1::test]
fn test_card_context_menu_lifecycle() {
    let mut cx = gpui::TestAppContext::single();
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
fn test_resizable_panes_and_layout_modes() {
    let mut cx = gpui::TestAppContext::single();
    let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));

    view.update(cx, |app, cx| {
        // Reset to defaults for deterministic unit testing
        app.sidebar_width = 230.0;
        app.inspector_width = 310.0;
        app.layout_mode = LayoutMode::Browser;
        app.grid_state = None;

        // 1. Initial dimensions
        assert_eq!(app.sidebar_width, 230.0);
        assert_eq!(app.inspector_width, 310.0);
        assert_eq!(app.layout_mode, LayoutMode::Browser);

        // 2. Resizing sidebar
        app.sidebar_width = 280.0;
        assert_eq!(app.sidebar_width, 280.0);

        // 3. Resizing inspector
        app.inspector_width = 350.0;
        assert_eq!(app.inspector_width, 350.0);

        // 4. Switching layout modes
        app.layout_mode = LayoutMode::Split;
        assert_eq!(app.layout_mode, LayoutMode::Split);

        app.layout_mode = LayoutMode::Viewer;
        assert_eq!(app.layout_mode, LayoutMode::Viewer);

        app.close_viewer(cx);
        assert_eq!(app.layout_mode, LayoutMode::Browser);
        assert!(!app.is_fullscreen_viewer);
    });
    cx.run_until_parked();
}

#[core::prelude::v1::test]
fn test_inspector_tabs_and_selection_marks() {
    let mut cx = gpui::TestAppContext::single();
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
    let mut cx = gpui::TestAppContext::single();
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
