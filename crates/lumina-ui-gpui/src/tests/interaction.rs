use crate::*;
use lumina_core::{EngineEvent, LuminaEngine};

/// Creates an isolated 12-image library and returns the engine after its initial scan.
fn scanned_engine(prefix: &str) -> (std::path::PathBuf, std::sync::Arc<LuminaEngine>) {
    let temp_dir = std::env::temp_dir().join(format!(
        "lumina_interaction_{}_{}",
        prefix,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    for i in 0..12 {
        std::fs::write(
            temp_dir.join(format!("photo_{:02}.jpg", i)),
            b"test-jpeg-content",
        )
        .unwrap();
    }
    let (engine, rx) = LuminaEngine::open_with_paths(
        temp_dir.clone(),
        temp_dir.join("catalog.db"),
        Some(temp_dir.join("cache")),
    )
    .unwrap();
    while let Ok(event) = rx.recv_timeout(std::time::Duration::from_secs(10)) {
        if let EngineEvent::ScanComplete { .. } = event {
            break;
        }
    }
    (temp_dir, engine)
}

#[core::prelude::v1::test]
fn test_filter_changes_clear_multi_selection() {
    let mut cx = gpui::TestAppContext::single();
    let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    let sample = get_sample_items();

    view.update(cx, |app, cx| {
        app.selected_indices = [0usize, 1, 2].into_iter().collect();
        app.selected_item = Some(sample[0].clone());
        app.set_search_query("is:favorite", cx);
        assert!(
            app.selected_indices.is_empty(),
            "search filters must clear stale multi-selection"
        );

        app.selected_indices = [0usize, 1].into_iter().collect();
        app.set_search_query("is:video", cx);
        assert!(
            app.selected_indices.is_empty(),
            "changing the search query must clear stale multi-selection"
        );

        app.selected_indices = [0usize].into_iter().collect();
        app.clear_search(cx);
        assert!(
            app.selected_indices.is_empty(),
            "clearing search must clear stale multi-selection"
        );
    });
}

#[core::prelude::v1::test]
fn test_search_focus_blur() {
    let mut cx = gpui::TestAppContext::single();
    let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));

    view.update(cx, |app, cx| {
        app.search_focused = true;
        app.blur_search(cx);
        assert!(!app.search_focused);
    });
}

#[core::prelude::v1::test]
fn test_watcher_batch_clears_selection_and_refreshes_counts() {
    let (temp_dir, engine) = scanned_engine("counts");
    let mut cx = gpui::TestAppContext::single();
    let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));

    view.update(cx, |app, cx| {
        app.grid_state = Some(crate::views::grid::GridState::new(engine.clone()));
        app.refresh_album_counts();
        // Simulate the end of the initial scan (the app normally receives ScanComplete).
        if let Some(state) = app.grid_state.as_mut() {
            state.is_scanning = false;
        }
        assert_eq!(app.total_library_count, 12);

        app.selected_indices = [0usize, 1].into_iter().collect();
        app.selected_item = Some(get_sample_items()[0].clone());

        // Simulate a post-scan watcher addition: catalog changes, then the event arrives.
        let extra = get_sample_items()[0].clone();
        engine.catalog().insert_or_update_batch(&[extra]).unwrap();
        assert_eq!(engine.total_count(), 13);
        app.handle_ui_message(
            UiMessage::Engine(EngineEvent::BatchDiscovered { items: Vec::new() }),
            cx,
        );

        assert!(
            app.selected_indices.is_empty(),
            "watcher additions must not leave stale position-based selection"
        );
        assert!(app.selected_item.is_none());
        assert_eq!(
            app.total_library_count, 13,
            "sidebar counts must refresh after watcher additions"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn test_viewer_wheel_zoom_clamps_and_ignores_video() {
    let mut cx = gpui::TestAppContext::single();
    let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    let sample = get_sample_items();

    view.update(cx, |app, cx| {
        app.selected_item = Some(sample[0].clone());
        app.zoom_viewer_by(0.5, cx);
        assert!((app.viewer_state.zoom_scale - 1.5).abs() < 0.01);
        app.zoom_viewer_by(10.0, cx);
        assert_eq!(app.viewer_state.zoom_scale, 11.5);
        app.zoom_viewer_by(100.0, cx);
        assert_eq!(
            app.viewer_state.zoom_scale,
            crate::views::viewer_state::ZOOM_MAX_SCALE
        );
        app.zoom_viewer_by(-100.0, cx);
        assert_eq!(app.viewer_state.zoom_scale, 1.0);

        app.selected_item = Some(sample[1].clone());
        app.zoom_viewer_by(1.0, cx);
        assert_eq!(
            app.viewer_state.zoom_scale, 1.0,
            "video playback must not zoom"
        );
    });
}

#[core::prelude::v1::test]
fn test_shortcuts_overlay_toggles() {
    let mut cx = gpui::TestAppContext::single();
    let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));

    view.update(cx, |app, cx| {
        assert!(!app.shortcuts_overlay_open);
        app.toggle_shortcuts_overlay(cx);
        assert!(app.shortcuts_overlay_open);
        app.close_shortcuts_overlay(cx);
        assert!(!app.shortcuts_overlay_open);
    });
}

#[core::prelude::v1::test]
fn test_keyboard_selection_reveals_grid_row() {
    let (temp_dir, engine) = scanned_engine("reveal");
    let mut cx = gpui::TestAppContext::single();
    let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));

    view.update(cx, |app, cx| {
        app.grid_state = Some(crate::views::grid::GridState::new(engine.clone()));
        app.grid_columns = 4;
        app.select_index(11, cx);
        assert_eq!(app.selected_index, Some(11));

        let scroll = app.grid_state.as_ref().unwrap().scroll_handle.clone();
        let deferred = scroll.0.borrow().deferred_scroll_to_item;
        assert_eq!(
            deferred.map(|d| d.item_index),
            Some(2),
            "selecting item 11 in a 4-column grid must reveal row 2"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}
