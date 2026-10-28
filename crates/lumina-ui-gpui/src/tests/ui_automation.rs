//! Black-box-style UI automation against the real GPUI view tree.
//!
//! These tests drive the app with synthetic platform input (mouse clicks at
//! rendered element bounds, key events, window resizes) instead of calling app
//! methods directly, so layout, hit-testing, focus, overlays and event wiring are
//! all exercised.
#![allow(dead_code)]

use crate::*;
use lumina_core::{EngineEvent, LuminaEngine};

fn seed_library(prefix: &str, count: usize) -> (std::path::PathBuf, std::sync::Arc<LuminaEngine>) {
    let temp_dir = std::env::temp_dir().join(format!(
        "lumina_uitest_{}_{}",
        prefix,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    for i in 0..count {
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

fn leak(selector: String) -> &'static str {
    Box::leak(selector.into_boxed_str())
}

/// Draws the registered root view so layout, hit-testing and debug_bounds are
/// populated for the current app state.
fn render_ui(cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear();
    });
    cx.run_until_parked();
}

fn focus_app(cx: &mut VisualTestContext, view: &Entity<LuminaApp>) {
    cx.update(|window, cx| {
        let handle = view.read(cx).focus_handle.clone();
        window.focus(&handle);
    });
}

fn bounds_center(cx: &mut VisualTestContext, selector: &'static str) -> gpui::Point<gpui::Pixels> {
    cx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("no rendered bounds for selector {selector:?}"))
        .center()
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let position = bounds_center(cx, selector);
    cx.simulate_click(position, gpui::Modifiers::default());
    render_ui(cx);
}

fn click_point(cx: &mut VisualTestContext, x: f32, y: f32) {
    cx.simulate_click(
        gpui::point(gpui::px(x), gpui::px(y)),
        gpui::Modifiers::default(),
    );
    render_ui(cx);
}

fn keystrokes(cx: &mut VisualTestContext, keys: &str) {
    cx.simulate_keystrokes(keys);
    render_ui(cx);
}

fn type_text(cx: &mut VisualTestContext, text: &str) {
    cx.simulate_input(text);
    render_ui(cx);
}

fn double_click(cx: &mut VisualTestContext, selector: &'static str) {
    let position = bounds_center(cx, selector);
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
    render_ui(cx);
}

fn right_click(cx: &mut VisualTestContext, selector: &'static str) {
    let position = bounds_center(cx, selector);
    cx.simulate_mouse_down(
        position,
        gpui::MouseButton::Right,
        gpui::Modifiers::default(),
    );
    render_ui(cx);
}

fn app_with_library(
    cx: &mut TestAppContext,
    engine: std::sync::Arc<LuminaEngine>,
) -> (Entity<LuminaApp>, &mut VisualTestContext) {
    let (view, vcx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    view.update(vcx, |app, cx| {
        app.grid_state = Some(crate::views::grid::GridState::new(engine));
        app.refresh_album_counts();
        cx.notify();
    });
    render_ui(vcx);
    focus_app(vcx, &view);
    (view, vcx)
}

#[core::prelude::v1::test]
fn ui_automation_renders_all_core_controls() {
    let (temp_dir, engine) = seed_library("render", 8);
    let mut cx = gpui::TestAppContext::single();
    let (_view, vcx) = app_with_library(&mut cx, engine);

    assert!(vcx.debug_bounds("app-root").is_some());
    let selectors = [
        "sidebar-toggle",
        "mode-browser",
        "mode-split",
        "mode-viewer",
        "search-input",
        "switch-library",
        "inspector-toggle",
        "breadcrumb-library",
        "fit-mode-fit",
        "grid-zoom-in",
        "open-tag-manager",
        "new-smart-album",
    ];
    let missing: Vec<_> = selectors
        .iter()
        .filter(|selector| vcx.debug_bounds(selector).is_none())
        .collect();
    assert!(missing.is_empty(), "controls not painted: {missing:?}");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn ui_automation_toolbar_buttons_by_mouse() {
    let (temp_dir, engine) = seed_library("toolbar", 8);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    click(vcx, "sidebar-toggle");
    view.update(vcx, |app, _| assert!(app.sidebar_collapsed));
    assert!(vcx.debug_bounds("expand-sidebar").is_some());
    click(vcx, "expand-sidebar");
    view.update(vcx, |app, _| assert!(!app.sidebar_collapsed));

    click(vcx, "mode-split");
    view.update(vcx, |app, _| assert_eq!(app.layout_mode, LayoutMode::Split));
    click(vcx, "mode-browser");
    view.update(vcx, |app, _| {
        assert_eq!(app.layout_mode, LayoutMode::Browser)
    });

    click(vcx, "fit-mode-square");
    view.update(vcx, |app, _| assert!(!app.card_fit_mode));
    click(vcx, "fit-mode-fit");
    view.update(vcx, |app, _| assert!(app.card_fit_mode));

    let before = view.update(vcx, |app, _| app.grid_columns);
    click(vcx, "grid-zoom-in");
    let after = view.update(vcx, |app, _| app.grid_columns);
    assert_ne!(before, after, "grid zoom button must change column count");

    click(vcx, "inspector-toggle");
    view.update(vcx, |app, _| assert!(app.inspector_open));
    click(vcx, "inspector-toggle");
    view.update(vcx, |app, _| assert!(!app.inspector_open));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn ui_automation_search_click_type_clear() {
    let (temp_dir, engine) = seed_library("search", 8);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    click(vcx, "search-input");
    view.update(vcx, |app, _| assert!(app.search_focused));

    type_text(vcx, "is:video");
    view.update(vcx, |app, _| {
        assert_eq!(app.search_query, "is:video");
        assert!(app.search_focused, "typing keeps search focused");
    });

    // Clicking elsewhere must blur search so shortcuts work again.
    click(vcx, "mode-browser");
    view.update(vcx, |app, _| assert!(!app.search_focused));
    assert!(vcx.debug_bounds("clear-search").is_some());

    click(vcx, "search-input");
    click(vcx, "clear-search");
    view.update(vcx, |app, _| {
        assert!(app.search_query.is_empty());
        assert!(!app.search_focused);
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn ui_automation_grid_and_viewer_journey() {
    let (temp_dir, engine) = seed_library("journey", 8);
    let first_id = engine.get_item_slice(0..1)[0].id.0;
    let card = leak(format!("media-card-{}", first_id));

    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    click(vcx, card);
    view.update(vcx, |app, _| {
        assert!(app.selected_item.is_some(), "click must select a card");
        assert_eq!(app.selected_index, Some(0));
    });

    double_click(vcx, card);
    view.update(vcx, |app, _| assert!(app.is_fullscreen_viewer));
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| assert!(!app.is_fullscreen_viewer));

    right_click(vcx, card);
    view.update(vcx, |app, _| assert!(app.context_menu_state.is_some()));
    assert!(vcx.debug_bounds("context-menu").is_some());
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| assert!(app.context_menu_state.is_none()));

    keystrokes(vcx, "end");
    view.update(vcx, |app, _| assert_eq!(app.selected_index, Some(7)));
    keystrokes(vcx, "home");
    view.update(vcx, |app, _| assert_eq!(app.selected_index, Some(0)));
    keystrokes(vcx, "pagedown");
    view.update(vcx, |app, _| assert_eq!(app.selected_index, Some(7)));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn ui_automation_modals_dismiss_on_backdrop() {
    let (temp_dir, engine) = seed_library("modals", 8);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    click(vcx, "open-tag-manager");
    view.update(vcx, |app, _| assert!(app.tag_manager.open));
    assert!(vcx.debug_bounds("tag-manager-dialog").is_some());
    click_point(vcx, 4.0, 4.0);
    view.update(vcx, |app, _| assert!(!app.tag_manager.open));

    click(vcx, "new-smart-album");
    view.update(vcx, |app, _| assert!(app.album_draft.modal_open));
    assert!(vcx.debug_bounds("album-builder-card").is_some());
    click_point(vcx, 4.0, 4.0);
    view.update(vcx, |app, _| assert!(!app.album_draft.modal_open));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn ui_automation_shortcut_cheat_sheet() {
    let (temp_dir, engine) = seed_library("shortcuts", 8);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    keystrokes(vcx, "cmd-/");
    view.update(vcx, |app, _| assert!(app.shortcuts_overlay_open));
    assert!(vcx.debug_bounds("shortcuts-card").is_some());

    // Keystrokes are captured by the sheet while open.
    keystrokes(vcx, "i");
    view.update(vcx, |app, _| assert!(!app.inspector_open));

    click(vcx, "shortcuts-close");
    view.update(vcx, |app, _| assert!(!app.shortcuts_overlay_open));

    keystrokes(vcx, "cmd-/");
    click_point(vcx, 4.0, 4.0);
    view.update(vcx, |app, _| assert!(!app.shortcuts_overlay_open));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn ui_automation_export_writes_originals_and_sidecars() {
    let (temp_dir, engine) = seed_library("export", 3);
    let item = engine.get_item_slice(0..1)[0].clone();
    engine.catalog().set_rating(item.id, 5).unwrap();
    engine
        .catalog()
        .set_caption(item.id, "Export caption")
        .unwrap();
    engine
        .catalog()
        .add_tags_batch(&[item.id], &["exported".to_string()])
        .unwrap();

    let destination = temp_dir.join("exported");
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.selected_item = Some(item.clone());
        app.export_to_folder(destination.clone(), cx);
    });

    let copied = destination.join(&item.file_name);
    assert!(copied.exists(), "export must copy the original file");
    let sidecar = lumina_core::read_sidecar(&copied)
        .unwrap()
        .expect("export should write a sidecar for annotated items");
    assert_eq!(sidecar.rating, 5);
    assert_eq!(sidecar.caption.as_deref(), Some("Export caption"));
    assert!(sidecar.tags.contains(&"exported".to_string()));
    view.update(vcx, |app, _| {
        assert!(
            app.last_action
                .as_deref()
                .unwrap_or("")
                .contains("Exported"),
            "got {:?}",
            app.last_action
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn ui_automation_undo_redo_rating_and_sidecar() {
    let (temp_dir, engine) = seed_library("undo", 3);
    let item = engine.get_item_slice(0..1)[0].clone();
    let sidecar = lumina_core::sidecar_path(&item.path);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, cx| {
        app.selected_item = Some(item.clone());
        app.set_item_rating(item.id, 4, cx);
    });
    assert!(sidecar.exists(), "rating should write an XMP sidecar");

    focus_app(vcx, &view);
    keystrokes(vcx, "cmd-z");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.item_ratings.get(&item.id).copied().unwrap_or(0),
            0,
            "undo should clear the rating"
        );
    });
    assert!(
        !sidecar.exists(),
        "undoing the only metadata should remove the sidecar"
    );

    focus_app(vcx, &view);
    keystrokes(vcx, "cmd-shift-z");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.item_ratings.get(&item.id).copied().unwrap_or(0),
            4,
            "redo should restore the rating"
        );
    });
    assert!(sidecar.exists(), "redo should rewrite the sidecar");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn ui_automation_undo_tag_and_caption() {
    let (temp_dir, engine) = seed_library("undo_meta", 3);
    let item = engine.get_item_slice(0..1)[0].clone();
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.selected_item = Some(item.clone());
        app.selected_indices.insert(0);
        app.batch_add_tag("travel".to_string(), cx);
    });
    view.update(vcx, |app, _| {
        assert!(app.selected_item_tags.contains(&"travel".to_string()));
    });

    focus_app(vcx, &view);
    keystrokes(vcx, "cmd-z");
    view.update(vcx, |app, _| {
        assert!(
            !app.selected_item_tags.contains(&"travel".to_string()),
            "undo should remove the tag"
        );
    });
    keystrokes(vcx, "cmd-shift-z");
    view.update(vcx, |app, _| {
        assert!(app.selected_item_tags.contains(&"travel".to_string()));
    });

    view.update(vcx, |app, cx| {
        app.start_caption_input(cx);
        app.caption.input_buffer = "A short caption".to_string();
        app.commit_caption_input(cx);
    });
    view.update(vcx, |app, _| {
        assert_eq!(
            app.item_captions.get(&item.id).map(String::as_str),
            Some("A short caption")
        );
    });
    focus_app(vcx, &view);
    keystrokes(vcx, "cmd-z");
    view.update(vcx, |app, _| {
        assert!(!app.item_captions.contains_key(&item.id));
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn ui_automation_title_edit_undo_and_xmp_import() {
    let (temp_dir, engine) = seed_library("title_import", 3);
    let item = engine.get_item_slice(0..1)[0].clone();
    let sidecar = lumina_core::sidecar_path(&item.path);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, cx| {
        app.selected_item = Some(item.clone());
        app.start_title_input(cx);
        app.caption.title_input_buffer = "Kyoto Trip".to_string();
        app.commit_title_input(cx);
    });
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_item
                .as_ref()
                .and_then(|it| it.title.as_deref()),
            Some("Kyoto Trip")
        );
    });
    let written = lumina_core::read_sidecar(&item.path).unwrap().unwrap();
    assert_eq!(written.title.as_deref(), Some("Kyoto Trip"));

    focus_app(vcx, &view);
    keystrokes(vcx, "cmd-z");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_item
                .as_ref()
                .and_then(|it| it.title.as_deref()),
            None,
            "undo should clear the title"
        );
    });
    assert!(!sidecar.exists(), "empty title should remove the sidecar");

    lumina_core::write_sidecar(
        &item.path,
        &lumina_core::SidecarMetadata {
            title: Some("Imported title".to_string()),
            caption: Some("Imported caption".to_string()),
            rating: 5,
            tags: vec!["imported".to_string()],
        },
    )
    .unwrap();
    view.update(vcx, |app, cx| {
        app.import_sidecars_selected(cx);
    });
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_item
                .as_ref()
                .and_then(|it| it.title.as_deref()),
            Some("Imported title")
        );
        assert_eq!(app.item_ratings.get(&item.id).copied(), Some(5));
        assert_eq!(
            app.item_captions.get(&item.id).map(String::as_str),
            Some("Imported caption")
        );
        assert!(app.selected_item_tags.contains(&"imported".to_string()));
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn ui_automation_resize_updates_persisted_bounds() {
    let (temp_dir, engine) = seed_library("resize", 8);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    vcx.simulate_resize(gpui::size(gpui::px(1100.0), gpui::px(720.0)));
    render_ui(vcx);
    view.update(vcx, |app, _| {
        let (_, _, width, height) = app.window_bounds.expect("bounds observer should fire");
        assert!((width - 1100.0).abs() < 2.0, "width was {width}");
        assert!((height - 720.0).abs() < 2.0, "height was {height}");
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}
