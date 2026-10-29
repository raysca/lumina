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
fn ui_automation_selection_contract_and_empty_space_click() {
    let (temp_dir, engine) = seed_library("selection_contract", 8);
    let first_id = engine.get_item_slice(0..1)[0].id.0;
    let card = leak(format!("media-card-{}", first_id));

    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    click(vcx, card);
    view.update(vcx, |app, _| {
        assert_eq!(app.selected_index, Some(0), "card click selects");
    });

    let surface = vcx
        .debug_bounds("media-grid-surface")
        .expect("grid surface must be painted");
    let empty_point = gpui::point(
        surface.origin.x + gpui::px(24.0),
        surface.origin.y + surface.size.height - gpui::px(24.0),
    );
    vcx.simulate_click(empty_point, gpui::Modifiers::default());
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(
            app.selected_indices.is_empty(),
            "empty space clears selection"
        );
        assert!(
            app.selected_index.is_none(),
            "empty space clears the focus index"
        );
        assert!(
            app.selected_item.is_none(),
            "empty space clears selected metadata"
        );
    });

    click(vcx, card);
    keystrokes(vcx, "shift-right");
    view.update(vcx, |app, _| {
        assert_eq!(app.selected_index, Some(1), "Shift+Right moves the focus");
        assert!(app.selected_indices.contains(&0));
        assert!(app.selected_indices.contains(&1));
    });
    keystrokes(vcx, "shift-right");
    view.update(vcx, |app, _| {
        assert_eq!(app.selected_index, Some(2));
        assert_eq!(
            app.selected_indices.len(),
            3,
            "Shift+Right grows from the anchor"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn ui_automation_tag_double_click_starts_rename() {
    let (temp_dir, engine) = seed_library("tag_rename", 3);
    let item = engine.get_item_slice(0..1)[0].clone();
    engine
        .add_tags_batch(&[item.id], &["travel".to_string()])
        .unwrap();

    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.refresh_album_counts();
        cx.notify();
    });
    render_ui(vcx);

    assert!(
        vcx.debug_bounds("sidebar-tag-filter-travel").is_some(),
        "the tagged item must create a sidebar tag row"
    );
    double_click(vcx, "sidebar-tag-filter-travel");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.editing_tag.as_deref(),
            Some("travel"),
            "double-clicking a tag must begin its inline rename"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn ui_automation_card_skeleton_selection_and_open() {
    let (temp_dir, engine) = seed_library("card_states", 8);
    // Seed fixtures contain invalid JPEG bytes, which is exactly the
    // "texture not decoded yet" state the skeleton exists for.
    let first_id = engine.get_item_slice(0..1)[0].id.0;
    let card = leak(format!("media-card-{}", first_id));
    let photo = leak(format!("media-card-photo-{}", first_id));
    let skeleton = leak(format!("media-card-skeleton-{}", first_id));
    let selected = leak(format!("media-card-selected-{}", first_id));

    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    assert!(
        vcx.debug_bounds(skeleton).is_some(),
        "a card without a decoded texture must paint media-card-skeleton-<id>"
    );
    let photo_bounds = vcx
        .debug_bounds(photo)
        .expect("every card must paint its photo area");
    let skeleton_bounds = vcx
        .debug_bounds(skeleton)
        .expect("skeleton bounds disappeared between assertions");
    assert!(
        photo_bounds.contains(&skeleton_bounds.center()),
        "the skeleton must sit inside the card photo area"
    );
    assert_eq!(
        photo_bounds.size, skeleton_bounds.size,
        "the skeleton must fill the photo area so the layout cannot jump on reveal"
    );
    assert!(
        vcx.debug_bounds(selected).is_none(),
        "an unselected card must not paint the selected marker"
    );

    click(vcx, card);
    view.update(vcx, |app, _| {
        assert_eq!(app.selected_index, Some(0), "single-click still selects");
        assert!(app.selected_item.is_some());
    });
    assert!(
        vcx.debug_bounds(selected).is_some(),
        "selecting a card must paint media-card-selected-<id>"
    );

    double_click(vcx, card);
    view.update(vcx, |app, _| {
        assert!(
            app.is_fullscreen_viewer,
            "double-click must still open the viewer"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn ui_automation_card_texture_replaces_skeleton() {
    let (temp_dir, engine) = seed_library("card_reveal", 1);
    let item = engine.get_item_slice(0..1)[0].clone();
    let card_id = item.id.0;
    let photo = leak(format!("media-card-photo-{}", card_id));
    let skeleton = leak(format!("media-card-skeleton-{}", card_id));
    let thumb = leak(format!("media-card-thumb-{}", card_id));

    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    assert!(vcx.debug_bounds(skeleton).is_some());
    assert!(vcx.debug_bounds(thumb).is_none());

    let texture = std::sync::Arc::new(gpui::RenderImage::new(vec![image::Frame::new(
        image::RgbaImage::new(4, 4),
    )]));
    view.update(vcx, |app, cx| {
        let state = app
            .grid_state
            .as_ref()
            .expect("grid state is installed by the test harness");
        state.texture_cache.lock().put(item.id, texture);
        cx.notify();
    });
    render_ui(vcx);

    // debug_bounds is cumulative across frames, so the reveal is proven by the
    // thumbnail appearing rather than by the skeleton's stale bounds vanishing.
    assert!(
        vcx.debug_bounds(thumb).is_some(),
        "a cached texture must paint inside the photo area"
    );
    let photo_bounds = vcx
        .debug_bounds(photo)
        .expect("the photo area must stay painted across the reveal");
    let thumb_bounds = vcx
        .debug_bounds(thumb)
        .expect("thumbnail bounds disappeared between assertions");
    assert_eq!(
        photo_bounds.size, thumb_bounds.size,
        "the revealed thumbnail must keep the same layout as the skeleton"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn ui_automation_empty_search_state_offers_a_clear_action() {
    let (temp_dir, engine) = seed_library("empty_search", 8);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, cx| {
        app.set_search_query("no-such-photo", cx);
    });
    render_ui(vcx);
    assert!(vcx.debug_bounds("empty-state").is_some());
    assert!(vcx.debug_bounds("empty-primary").is_some());

    click(vcx, "empty-primary");
    view.update(vcx, |app, _| {
        assert!(
            app.search_query.is_empty(),
            "empty state must clear the search"
        );
        assert!(app.selected_item.is_none());
    });

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
fn ui_automation_date_and_location_edit_undo() {
    let (temp_dir, engine) = seed_library("edit_time_place", 3);
    let item = engine.get_item_slice(0..1)[0].clone();
    let original_timestamp = item.timestamp_epoch_ms;
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.selected_item = Some(item.clone());
        app.start_date_input(cx);
        app.caption.date_input_buffer = "2020-05-06".to_string();
        app.commit_date_input(cx);
    });
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_item.as_ref().map(|it| it.year),
            Some(2020),
            "date edit should update the selected item"
        );
        assert!(app
            .timeline_year_buckets
            .iter()
            .any(|(year, _)| *year == 2020));
    });

    focus_app(vcx, &view);
    keystrokes(vcx, "cmd-z");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_item.as_ref().map(|it| it.timestamp_epoch_ms),
            Some(original_timestamp),
            "undo should restore the original date"
        );
    });
    keystrokes(vcx, "cmd-shift-z");
    view.update(vcx, |app, _| {
        assert_eq!(app.selected_item.as_ref().map(|it| it.year), Some(2020));
    });

    view.update(vcx, |app, cx| {
        app.start_location_input(cx);
        app.caption.location_input_buffer = "48.8566, 2.3522".to_string();
        app.commit_location_input(cx);
    });
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_item
                .as_ref()
                .and_then(|it| it.latitude)
                .map(|lat| (lat * 10_000.0).round() / 10_000.0),
            Some(48.8566)
        );
        assert_eq!(
            app.selected_item
                .as_ref()
                .and_then(|it| it.longitude)
                .map(|lon| (lon * 10_000.0).round() / 10_000.0),
            Some(2.3522)
        );
    });
    focus_app(vcx, &view);
    keystrokes(vcx, "cmd-z");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_item.as_ref().and_then(|it| it.latitude),
            None,
            "undo should clear the location"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn ui_automation_command_palette() {
    let (temp_dir, engine) = seed_library("command_palette", 3);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    render_ui(vcx);
    focus_app(vcx, &view);
    keystrokes(vcx, "cmd-k");
    view.update(vcx, |app, _| assert!(app.command_palette_open));

    for key in ["p", "r", "e", "f"] {
        keystrokes(vcx, key);
    }
    view.update(vcx, |app, _| {
        assert_eq!(app.command_query, "pref");
        let titles: Vec<String> = app
            .filtered_commands()
            .into_iter()
            .map(|command| command.title())
            .collect();
        assert_eq!(titles.first().map(String::as_str), Some("Preferences…"));
    });
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert!(!app.command_palette_open);
        assert!(app.preferences_open);
    });
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| assert!(!app.preferences_open));

    focus_app(vcx, &view);
    keystrokes(vcx, "cmd-k");
    for key in ["a", "b", "o", "u", "t"] {
        keystrokes(vcx, key);
    }
    click(vcx, "command-item-command.about");
    view.update(vcx, |app, _| {
        assert!(app.preferences_open);
        assert_eq!(
            app.preferences_tab,
            crate::app::preferences::PreferencesTab::About
        );
    });
    keystrokes(vcx, "escape");

    focus_app(vcx, &view);
    keystrokes(vcx, "cmd-k");
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| assert!(!app.command_palette_open));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn ui_automation_preferences_modal() {
    let (temp_dir, engine) = seed_library("preferences", 3);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    render_ui(vcx);
    focus_app(vcx, &view);
    keystrokes(vcx, "cmd-,");
    view.update(vcx, |app, _| {
        assert!(app.preferences_open, "Cmd+, should open preferences");
        assert!(app.preferences_cache_bytes.is_some());
    });

    click(vcx, "preferences-tab-storage");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.preferences_tab,
            crate::app::preferences::PreferencesTab::Storage
        );
    });
    click(vcx, "preferences-tab-about");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.preferences_tab,
            crate::app::preferences::PreferencesTab::About
        );
    });
    click(vcx, "preferences-close");
    view.update(vcx, |app, _| assert!(!app.preferences_open));

    focus_app(vcx, &view);
    keystrokes(vcx, "cmd-,");
    view.update(vcx, |app, _| assert!(app.preferences_open));
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(!app.preferences_open, "Escape should close preferences");
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn ui_automation_sort_menu_orders_grid() {
    let (temp_dir, engine) = seed_library("sorting", 5);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    render_ui(vcx);
    click(vcx, "sort-menu-button");
    view.update(vcx, |app, _| assert!(app.sort_menu_open));

    click(vcx, "sort-field-name");
    view.update(vcx, |app, _| {
        assert_eq!(app.sort.field, lumina_core::SortField::FileName);
        assert!(!app.sort_menu_open);
    });

    click(vcx, "sort-menu-button");
    click(vcx, "sort-direction-asc");
    view.update(vcx, |app, _| {
        assert_eq!(app.sort.direction, lumina_core::SortDirection::Ascending);
        assert_eq!(app.sort_label(), "Name ↑");
        let names: Vec<String> = app
            .get_active_slice(0..5)
            .into_iter()
            .map(|item| item.file_name)
            .collect();
        let mut expected = names.clone();
        expected.sort_by_key(|name| name.to_lowercase());
        assert_eq!(names, expected, "grid must follow the selected sort");
        assert_eq!(names.first().map(String::as_str), Some("photo_00.jpg"));
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn ui_automation_smart_album_edit_and_delete() {
    let (temp_dir, engine) = seed_library("smart_album_mgmt", 3);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, cx| {
        app.open_album_builder(cx);
        app.album_draft.album_title = "Round Trip".to_string();
        app.album_draft.rules = vec![lumina_core::SmartAlbumRule {
            field: lumina_core::RuleField::IsFavorite,
            operator: lumina_core::RuleOperator::Equals,
            value: lumina_core::RuleValue::Bool(true),
        }];
        app.save_draft_smart_album(cx);
    });
    let album = view.update(vcx, |app, _| {
        let album = app
            .user_smart_albums
            .first()
            .cloned()
            .expect("album should be created");
        assert_eq!(album.title, "Round Trip");
        album
    });

    render_ui(vcx);
    click(vcx, leak(format!("smart-album-edit-{}", album.id)));
    view.update(vcx, |app, _| {
        assert!(
            app.album_draft.modal_open,
            "edit button should open builder"
        );
        assert_eq!(app.album_draft.editing_album_id, Some(album.id));
    });
    view.update(vcx, |app, cx| {
        app.album_draft.album_title = "Renamed Trip".to_string();
        app.save_draft_smart_album(cx);
    });
    view.update(vcx, |app, _| {
        let updated = app
            .user_smart_albums
            .iter()
            .find(|candidate| candidate.id == album.id)
            .expect("album should still exist");
        assert_eq!(updated.title, "Renamed Trip");
    });

    render_ui(vcx);
    click(vcx, leak(format!("smart-album-delete-{}", album.id)));
    view.update(vcx, |app, _| {
        assert!(
            app.user_smart_albums.is_empty(),
            "delete button should remove the album"
        );
        assert_eq!(
            app.selection,
            crate::views::sidebar::AlbumSelection::AllLibrary
        );
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

#[core::prelude::v1::test]
fn ui_automation_navigation_album_back_forward() {
    let (temp_dir, engine) = seed_library("nav_album", 8);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, cx| {
        app.select_album(
            AlbumSelection::System(lumina_core::SystemAlbum::Favorites),
            cx,
        );
    });
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::System(lumina_core::SystemAlbum::Favorites),
            "the album change must reach the browsing destination"
        );
    });

    click(vcx, "nav-back");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::AllLibrary,
            "Back must return to the previous album"
        );
    });
    click(vcx, "nav-forward");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::System(lumina_core::SystemAlbum::Favorites),
            "Forward must revisit the album"
        );
    });

    // Edge: nothing follows the newest entry, so Forward is a disabled no-op.
    click(vcx, "nav-forward");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::System(lumina_core::SystemAlbum::Favorites),
            "Forward at the end of the trail must be a no-op"
        );
    });
    click(vcx, "nav-back");
    click(vcx, "nav-back");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::AllLibrary,
            "Back at the oldest entry must stay put"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn ui_automation_navigation_search_back() {
    let (temp_dir, engine) = seed_library("nav_search", 8);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, cx| app.set_search_query("is:video", cx));
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.search_query, "is:video");
    });

    click(vcx, "nav-back");
    view.update(vcx, |app, _| {
        assert!(
            app.search_query.is_empty(),
            "Back must restore the pre-search view"
        );
        assert_eq!(app.selection, AlbumSelection::AllLibrary);
    });
    click(vcx, "nav-forward");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.search_query, "is:video",
            "Forward must restore the search destination"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn ui_automation_navigation_viewer_back_closes_and_forward_reopens() {
    let (temp_dir, engine) = seed_library("nav_viewer", 8);
    let item = engine.get_item_slice(0..1)[0].clone();
    let card = leak(format!("media-card-{}", item.id.0));

    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    click(vcx, card);
    double_click(vcx, card);
    view.update(vcx, |app, _| {
        assert!(
            app.is_fullscreen_viewer,
            "double-click must open the viewer"
        );
        assert_eq!(app.selected_item.as_ref().map(|it| it.id), Some(item.id));
    });

    focus_app(vcx, &view);
    keystrokes(vcx, "cmd-[");
    view.update(vcx, |app, _| {
        assert!(
            !app.is_fullscreen_viewer,
            "Back from the viewer must close it and return to the grid"
        );
        assert_eq!(
            app.selected_item.as_ref().map(|it| it.id),
            Some(item.id),
            "Back must keep the grid selection on the item that was open"
        );
    });

    focus_app(vcx, &view);
    keystrokes(vcx, "cmd-]");
    view.update(vcx, |app, _| {
        assert!(
            app.is_fullscreen_viewer,
            "Forward must reopen the viewer on the same destination"
        );
        assert_eq!(app.selected_item.as_ref().map(|it| it.id), Some(item.id));
    });

    // Close again, then prove the oldest edge cannot trap the user.
    focus_app(vcx, &view);
    keystrokes(vcx, "cmd-[");
    keystrokes(vcx, "cmd-[");
    view.update(vcx, |app, _| {
        assert!(!app.is_fullscreen_viewer);
        assert_eq!(app.selected_item.as_ref().map(|it| it.id), Some(item.id));
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn ui_automation_navigation_edge_no_ops() {
    let (temp_dir, engine) = seed_library("nav_edges", 8);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    render_ui(vcx);
    focus_app(vcx, &view);
    keystrokes(vcx, "cmd-[");
    keystrokes(vcx, "cmd-]");
    click(vcx, "nav-back");
    click(vcx, "nav-forward");
    view.update(vcx, |app, _| {
        assert_eq!(app.selection, AlbumSelection::AllLibrary);
        assert!(app.search_query.is_empty());
        assert!(!app.is_fullscreen_viewer);
        assert!(app.selected_item.is_none());
    });

    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::Trash, cx);
    });
    render_ui(vcx);
    click(vcx, "nav-forward");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::Trash,
            "Forward at the newest entry must not move"
        );
    });

    click(vcx, "nav-back");
    view.update(vcx, |app, _| {
        assert_eq!(app.selection, AlbumSelection::AllLibrary);
    });
    focus_app(vcx, &view);
    keystrokes(vcx, "cmd-[");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::AllLibrary,
            "Cmd+[ at the oldest entry must stay put"
        );
    });
    keystrokes(vcx, "cmd-]");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::Trash,
            "Cmd+] must still move Forward after a Back"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn ui_automation_search_suggestions_filter_and_keyboard_apply() {
    let (temp_dir, engine) = seed_library("search_suggestions", 3);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    click(vcx, "search-input");
    view.update(vcx, |app, _| {
        assert!(app.search_focused, "clicking the field focuses search");
        assert!(
            !app.search_suggestions.dismissed,
            "focusing the field opens suggestions"
        );
        assert!(
            !app.search_suggestion_items().is_empty(),
            "an empty query offers recent searches and operators"
        );
    });
    assert!(
        vcx.debug_bounds("search-suggestions").is_some(),
        "the dropdown must paint under the focused field"
    );
    assert!(
        vcx.debug_bounds("search-suggestion-0").is_some(),
        "the first suggestion row must be painted"
    );

    type_text(vcx, "is:f");
    view.update(vcx, |app, _| {
        assert_eq!(app.search_query, "is:f");
        let items = app.search_suggestion_items();
        assert!(
            items.iter().any(|item| item == "is:favorite"),
            "the matching operator must be suggested: {items:?}"
        );
        assert!(
            items
                .iter()
                .all(|item| item.to_lowercase().contains("is:f")),
            "suggestions must be filtered by the typed token: {items:?}"
        );
    });

    keystrokes(vcx, "down");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.search_suggestions.selected,
            Some(0),
            "ArrowDown highlights the first suggestion"
        );
        assert_eq!(
            app.search_query, "is:f",
            "moving the highlight must not edit the query"
        );
    });

    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.search_query, "is:favorite",
            "Enter applies the highlighted suggestion"
        );
        assert!(
            app.search_focused,
            "applying a suggestion keeps the field focused for refinement"
        );
        assert!(
            app.search_suggestions.dismissed,
            "applying a suggestion closes the dropdown"
        );
        assert_eq!(
            app.config.recent_searches.first().map(String::as_str),
            Some("is:favorite"),
            "an applied suggestion is a committed search"
        );
    });

    // Escape hides the open dropdown first and only the second Escape blurs.
    click(vcx, "search-input");
    view.update(vcx, |app, _| {
        assert!(!app.search_suggestions.dismissed);
    });
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(
            app.search_focused,
            "the first Escape only closes the dropdown"
        );
        assert!(app.search_suggestions.dismissed);
    });
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(
            !app.search_focused,
            "the second Escape unfocuses the search field"
        );
    });

    // Committing the same query again must not duplicate recent history.
    click(vcx, "search-input");
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert!(!app.search_focused, "Enter without a highlight unfocuses");
        assert_eq!(
            app.config
                .recent_searches
                .iter()
                .filter(|entry| entry.eq_ignore_ascii_case("is:favorite"))
                .count(),
            1,
            "recent searches dedupe case-insensitively"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn ui_automation_search_suggestion_click_applies_without_blurring() {
    let (temp_dir, engine) = seed_library("search_suggestion_click", 3);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, _| {
        app.config.recent_searches = vec!["camera:sony".to_string(), "is:video".to_string()];
    });

    click(vcx, "search-input");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.search_suggestion_items().first().map(String::as_str),
            Some("camera:sony"),
            "the newest recent search is the first empty-query suggestion"
        );
    });
    assert!(
        vcx.debug_bounds("search-suggestion-0").is_some(),
        "the recent-search row must be painted"
    );

    click(vcx, "search-suggestion-0");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.search_query, "camera:sony",
            "clicking a suggestion applies it"
        );
        assert!(
            app.search_focused,
            "clicking inside the dropdown must not blur the search field"
        );
        assert!(app.search_suggestions.dismissed);
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn ui_automation_grid_drag_selects_range_and_suppresses_click() {
    use std::collections::BTreeSet;

    let (temp_dir, engine) = seed_library("grid_drag_range", 8);
    let items = engine.get_item_slice(0..8);
    let cards: Vec<&'static str> = items
        .iter()
        .map(|item| leak(format!("media-card-{}", item.id.0)))
        .collect();

    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    let start = bounds_center(vcx, cards[0]);
    let end = bounds_center(vcx, cards[2]);
    vcx.simulate_mouse_down(start, gpui::MouseButton::Left, gpui::Modifiers::default());
    render_ui(vcx);
    vcx.simulate_mouse_move(
        end,
        Some(gpui::MouseButton::Left),
        gpui::Modifiers::default(),
    );
    render_ui(vcx);

    view.update(vcx, |app, _| {
        assert!(app.grid_drag.active, "a sweeping press must stay a drag");
        assert!(
            app.grid_drag.moved,
            "moving past the threshold marks the drag"
        );
        assert_eq!(
            app.grid_drag.anchor,
            Some(0),
            "the pressed card is the anchor"
        );
        assert_eq!(
            app.selected_indices,
            BTreeSet::from([0usize, 1, 2]),
            "dragging over cards extends a contiguous range"
        );
        assert_eq!(app.selected_index, Some(2));
    });
    let marquee = vcx
        .debug_bounds("grid-selection-rect")
        .expect("the marquee rectangle must paint while a moved drag is live");
    assert!(
        marquee.size.width > gpui::px(0.0) && marquee.size.height > gpui::px(0.0),
        "the marquee surface must cover the grid instead of collapsing to zero size"
    );

    vcx.simulate_mouse_up(end, gpui::MouseButton::Left, gpui::Modifiers::default());
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(!app.grid_drag.active, "mouse-up resets the drag");
        assert_eq!(
            app.selected_indices.len(),
            3,
            "the release must not collapse the drag selection into a single click"
        );
        assert_eq!(app.batch_summary.count, 3, "the inspector shows the range");
    });
    // Note: GPUI 0.2.2 keeps debug_bounds entries for selectors that stopped
    // painting, so disappearance is asserted through the drag state above
    // rather than through the bounds map.

    // Suppression must be consumed: the next genuine click is still a single select.
    click(vcx, cards[1]);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_indices,
            BTreeSet::from([1usize]),
            "a plain click after a drag selects exactly one card"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn ui_automation_grid_drag_from_selected_card_preserves_multi_selection() {
    use std::collections::BTreeSet;

    let (temp_dir, engine) = seed_library("grid_drag_preserve", 8);
    let items = engine.get_item_slice(0..8);
    let cards: Vec<&'static str> = items
        .iter()
        .map(|item| leak(format!("media-card-{}", item.id.0)))
        .collect();

    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    // Build {0,1,2} the keyboard-equivalent way: click then Shift+click.
    click(vcx, cards[0]);
    let shift_click = bounds_center(vcx, cards[2]);
    vcx.simulate_click(
        shift_click,
        gpui::Modifiers {
            shift: true,
            ..Default::default()
        },
    );
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.selected_indices, BTreeSet::from([0usize, 1, 2]));
    });

    // Drag from an already-selected card, staying inside it (moved but no new card).
    let start = bounds_center(vcx, cards[1]);
    let end = gpui::point(start.x + gpui::px(24.0), start.y);
    vcx.simulate_mouse_down(start, gpui::MouseButton::Left, gpui::Modifiers::default());
    render_ui(vcx);
    vcx.simulate_mouse_move(
        end,
        Some(gpui::MouseButton::Left),
        gpui::Modifiers::default(),
    );
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.grid_drag.moved);
        assert_eq!(
            app.grid_drag.anchor,
            Some(1),
            "the dragged card is the anchor"
        );
        assert_eq!(
            app.selected_indices,
            BTreeSet::from([0usize, 1, 2]),
            "dragging from a selected card must not collapse the multi-selection"
        );
    });

    // The release lands on the same card, so a click event fires: it must be
    // suppressed rather than reducing the selection back to {1}.
    vcx.simulate_mouse_up(end, gpui::MouseButton::Left, gpui::Modifiers::default());
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_indices,
            BTreeSet::from([0usize, 1, 2]),
            "the drag-end click must not clear the multi-selection"
        );
        assert!(!app.grid_drag.active);
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn ui_automation_grid_drag_with_cmd_is_additive() {
    use std::collections::BTreeSet;

    let (temp_dir, engine) = seed_library("grid_drag_cmd", 8);
    let items = engine.get_item_slice(0..8);
    let cards: Vec<&'static str> = items
        .iter()
        .map(|item| leak(format!("media-card-{}", item.id.0)))
        .collect();

    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    click(vcx, cards[0]);
    view.update(vcx, |app, _| {
        assert_eq!(app.selected_indices, BTreeSet::from([0usize]));
    });

    let cmd = gpui::Modifiers::secondary_key();
    let start = bounds_center(vcx, cards[3]);
    let end = bounds_center(vcx, cards[5]);
    vcx.simulate_mouse_down(start, gpui::MouseButton::Left, cmd);
    render_ui(vcx);
    vcx.simulate_mouse_move(end, Some(gpui::MouseButton::Left), cmd);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_indices,
            BTreeSet::from([0usize, 3, 4, 5]),
            "Cmd-drag adds the swept range to the existing selection"
        );
        assert_eq!(app.grid_drag.anchor, Some(3));
    });

    vcx.simulate_mouse_up(end, gpui::MouseButton::Left, cmd);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_indices,
            BTreeSet::from([0usize, 3, 4, 5]),
            "the additive selection survives the release"
        );
        assert!(!app.grid_drag.active);
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn ui_automation_grid_drag_from_empty_background_sweeps_and_escape_resets() {
    use std::collections::BTreeSet;

    let (temp_dir, engine) = seed_library("grid_drag_bg", 8);
    let items = engine.get_item_slice(0..8);
    let cards: Vec<&'static str> = items
        .iter()
        .map(|item| leak(format!("media-card-{}", item.id.0)))
        .collect();

    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    let surface = vcx
        .debug_bounds("media-grid-surface")
        .expect("grid surface must be painted");
    let empty = gpui::point(
        surface.origin.x + gpui::px(24.0),
        surface.origin.y + surface.size.height - gpui::px(24.0),
    );

    vcx.simulate_mouse_down(empty, gpui::MouseButton::Left, gpui::Modifiers::default());
    render_ui(vcx);
    let first_card = bounds_center(vcx, cards[0]);
    vcx.simulate_mouse_move(
        first_card,
        Some(gpui::MouseButton::Left),
        gpui::Modifiers::default(),
    );
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.grid_drag.anchor,
            Some(0),
            "the first swept card becomes the anchor"
        );
        assert_eq!(app.selected_indices, BTreeSet::from([0usize]));
    });

    let last_card = bounds_center(vcx, cards[2]);
    vcx.simulate_mouse_move(
        last_card,
        Some(gpui::MouseButton::Left),
        gpui::Modifiers::default(),
    );
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_indices,
            BTreeSet::from([0usize, 1, 2]),
            "a background marquee extends from the first swept card"
        );
    });

    // Escape must abandon the live gesture (the grid keymap clears the batch).
    focus_app(vcx, &view);
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(!app.grid_drag.active, "Escape resets the drag gesture");
        assert!(
            app.selected_indices.is_empty(),
            "Escape clears the multi-selection it was building"
        );
    });

    // Release after the cancel must not resurrect the drag.
    vcx.simulate_mouse_up(
        last_card,
        gpui::MouseButton::Left,
        gpui::Modifiers::default(),
    );
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(!app.grid_drag.active);
        assert!(app.selected_indices.is_empty());
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

/// Sends a wheel event at `position` after moving the pointer there, because
/// GPUI hit-tests scroll events against the last mouse position.
fn wheel(
    cx: &mut VisualTestContext,
    position: gpui::Point<gpui::Pixels>,
    delta: gpui::ScrollDelta,
    modifiers: gpui::Modifiers,
) {
    cx.simulate_mouse_move(position, None, gpui::Modifiers::default());
    cx.simulate_event(gpui::ScrollWheelEvent {
        position,
        delta,
        modifiers,
        touch_phase: gpui::TouchPhase::Moved,
    });
    render_ui(cx);
}

#[core::prelude::v1::test]
fn ui_automation_grid_zoom_anchor_preserves_pointer_row_and_fraction() {
    use crate::views::grid::{GridZoomAnchor, GridZoomGeometry};

    // 920px wide list with the grid's 16px side padding and 10px gaps leaves
    // 214.5px cards at four columns, so x=600 is inside the third column.
    let geometry = GridZoomGeometry {
        left: 100.0,
        top: 100.0,
        width: 920.0,
        height: 600.0,
        scroll_y: -100.0,
        row_height: 200.0,
        columns: 4,
        item_count: 40,
    };
    // Window y=300 maps to content y=300: row 1, half way down the row.
    let anchor = GridZoomAnchor::at_pointer(600.0, 300.0, geometry)
        .expect("a pointer inside the viewport must anchor an item");
    assert_eq!(anchor.row, 1, "content y=300 sits in the second 200px row");
    assert_eq!(
        anchor.item,
        4 + 2,
        "row 1 x 4 columns + column index 2 must anchor item 6"
    );
    assert!(
        (anchor.fraction_within_row - 0.5).abs() < 0.001,
        "the pointer is half way down the row, got {}",
        anchor.fraction_within_row
    );

    let target = anchor.target_scroll_offset(300.0, geometry, 3, 260.0);
    // Item 6 becomes row 2 at three columns; half of a 260px row puts the
    // content top at 2*260 + 130 = 650, so the viewport needs 450px of scroll.
    assert!(
        (target - 450.0).abs() < 0.001,
        "item 6 must land in row 2 with the pointer half a row down, got {target}"
    );

    // The pointer now sits inside row 2, which is the anchor item's new row.
    let new_content_y = 300.0 - geometry.top - (-target);
    assert_eq!(
        (new_content_y / 260.0).floor() as usize,
        anchor.item / 3,
        "the anchor item's row must remain under the pointer after zoom"
    );
}

#[core::prelude::v1::test]
fn ui_automation_grid_zoom_anchor_clamps_partial_rows_and_invalid_geometry() {
    use crate::views::grid::{GridZoomAnchor, GridZoomGeometry};

    // Five items at four columns: the last row is partial. A pointer in the
    // empty part of that row still anchors the final real item.
    let geometry = GridZoomGeometry {
        left: 0.0,
        top: 0.0,
        width: 920.0,
        height: 600.0,
        scroll_y: 0.0,
        row_height: 200.0,
        columns: 4,
        item_count: 5,
    };
    let anchor = GridZoomAnchor::at_pointer(880.0, 350.0, geometry)
        .expect("a pointer over the last partial row must anchor an item");
    assert_eq!(
        anchor.item, 4,
        "the anchor must clamp to the last real item, not invent a sixth"
    );

    // Two columns at 380px rows: item 4 becomes row 2, but the desired 695px
    // offset exceeds the 540px scroll range, so it clamps to the end.
    let target = anchor.target_scroll_offset(350.0, geometry, 2, 380.0);
    assert!(
        (target - 540.0).abs() < 0.001,
        "the target offset must clamp to the new content height, got {target}"
    );

    assert!(
        GridZoomAnchor::at_pointer(
            0.0,
            0.0,
            GridZoomGeometry {
                height: 0.0,
                ..geometry
            }
        )
        .is_none(),
        "an unmeasured viewport cannot anchor a pointer"
    );
    assert!(
        GridZoomAnchor::at_pointer(
            0.0,
            50.0,
            GridZoomGeometry {
                top: 100.0,
                ..geometry
            }
        )
        .is_none(),
        "a pointer above the content cannot anchor an item"
    );
}

#[core::prelude::v1::test]
fn ui_automation_plain_wheel_scrolls_and_cmd_wheel_zooms_around_pointer() {
    use crate::views::grid::{GridZoomAnchor, GridZoomGeometry};

    let (temp_dir, engine) = seed_library("grid_wheel_zoom", 40);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, cx| {
        app.grid_columns = 4;
        cx.notify();
    });
    render_ui(vcx);

    let surface = vcx
        .debug_bounds("media-grid-surface")
        .expect("grid surface must be painted");
    let pointer = surface.center();

    // A plain wheel must keep scrolling: density and selection stay untouched.
    wheel(
        vcx,
        pointer,
        gpui::ScrollDelta::Pixels(gpui::point(gpui::px(0.0), gpui::px(-600.0))),
        gpui::Modifiers::default(),
    );
    let (offset_after_plain, columns_after_plain) = view.update(vcx, |app, _| {
        let scroll_handle = app
            .grid_state
            .as_ref()
            .expect("the test harness installs grid state")
            .scroll_handle
            .clone();
        let state = scroll_handle.0.borrow();
        (state.base_handle.offset().y, app.grid_columns)
    });
    assert_eq!(
        columns_after_plain, 4,
        "a plain wheel must not change grid density"
    );
    assert!(
        offset_after_plain < gpui::px(0.0),
        "a plain wheel must scroll the grid, got offset {offset_after_plain:?}"
    );

    // Capture the item under the pointer with the same public geometry the
    // production handler reads from the scroll handle.
    let (geometry, anchor) = view.update(vcx, |app, _| {
        let scroll_handle = app
            .grid_state
            .as_ref()
            .expect("the test harness installs grid state")
            .scroll_handle
            .clone();
        let state = scroll_handle.0.borrow();
        let bounds = state.base_handle.bounds();
        let geometry = GridZoomGeometry {
            left: f32::from(bounds.origin.x),
            top: f32::from(bounds.origin.y),
            width: f32::from(bounds.size.width),
            height: f32::from(bounds.size.height),
            scroll_y: f32::from(state.base_handle.offset().y),
            row_height: app.row_height(),
            columns: app.grid_columns,
            item_count: app.get_active_count(),
        };
        drop(state);
        let anchor =
            GridZoomAnchor::at_pointer(f32::from(pointer.x), f32::from(pointer.y), geometry)
                .expect("the pointer must be inside the measured grid viewport");
        (geometry, anchor)
    });
    assert!(
        geometry.height > 0.0 && geometry.width > 0.0,
        "the scroll handle must expose real viewport bounds, got {geometry:?}"
    );

    // Cmd+wheel up tightens the grid without starting a drag or a click.
    wheel(
        vcx,
        pointer,
        gpui::ScrollDelta::Pixels(gpui::point(gpui::px(0.0), gpui::px(60.0))),
        gpui::Modifiers::secondary_key(),
    );

    let (columns_after_zoom, selection_len, dragging) = view.update(vcx, |app, _| {
        (
            app.grid_columns,
            app.selected_indices.len(),
            app.grid_drag.active,
        )
    });
    assert_eq!(
        columns_after_zoom, 3,
        "Cmd+wheel up must make the grid denser (one fewer column)"
    );
    assert_eq!(
        selection_len, 0,
        "a zoom gesture must not select an item as if it were a click"
    );
    assert!(!dragging, "a Cmd+wheel event must not start a drag");

    // The anchor item's new row must still sit under the pointer.
    let (row_under_pointer, row_height, new_columns) = view.update(vcx, |app, _| {
        let scroll_handle = app
            .grid_state
            .as_ref()
            .expect("the test harness installs grid state")
            .scroll_handle
            .clone();
        let state = scroll_handle.0.borrow();
        let bounds = state.base_handle.bounds();
        let scroll_y = f32::from(state.base_handle.offset().y);
        let row_height = app.row_height();
        let content_y = f32::from(pointer.y) - f32::from(bounds.origin.y) - scroll_y;
        (
            (content_y / row_height).floor() as usize,
            row_height,
            app.grid_columns,
        )
    });
    assert_eq!(
        row_under_pointer,
        anchor.item / new_columns,
        "the anchored item's new row must stay under the pointer (row {row_under_pointer}, row height {row_height}, {new_columns} columns)"
    );

    // At the loosest setting Cmd+wheel cannot zoom further; it must also not
    // silently scroll the grid. Two columns leave the 40 items scrollable, so
    // the offset assertion exercises the event-delta undo.
    view.update(vcx, |app, cx| {
        app.grid_columns = 2;
        cx.notify();
    });
    render_ui(vcx);
    let offset_before_limit = view.update(vcx, |app, _| {
        app.grid_state
            .as_ref()
            .expect("the test harness installs grid state")
            .scroll_handle
            .0
            .borrow()
            .base_handle
            .offset()
            .y
    });
    assert!(
        offset_before_limit < gpui::px(0.0),
        "the limit check must run on a scrollable grid, got {offset_before_limit:?}"
    );
    wheel(
        vcx,
        pointer,
        gpui::ScrollDelta::Pixels(gpui::point(gpui::px(0.0), gpui::px(60.0))),
        gpui::Modifiers::secondary_key(),
    );
    let (columns_at_limit, offset_at_limit) = view.update(vcx, |app, _| {
        (
            app.grid_columns,
            app.grid_state
                .as_ref()
                .expect("the test harness installs grid state")
                .scroll_handle
                .0
                .borrow()
                .base_handle
                .offset()
                .y,
        )
    });
    assert_eq!(
        columns_at_limit, 2,
        "density must stay clamped at 2 columns"
    );
    assert!(
        (f32::from(offset_at_limit) - f32::from(offset_before_limit)).abs() < 0.5,
        "a Cmd+wheel that cannot zoom must leave the scroll offset alone: before={offset_before_limit:?} after={offset_at_limit:?}"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn ui_automation_batch_title_and_caption_mixed_apply_and_grouped_undo() {
    let (temp_dir, engine) = seed_library("batch_meta", 4);
    let items = engine.get_item_slice(0..4);
    let first = items[0].clone();
    let ids: Vec<_> = items[0..3].iter().map(|item| item.id).collect();
    engine.set_title(first.id, Some("First")).unwrap();
    engine.set_caption(first.id, "First caption").unwrap();

    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    view.update(vcx, |app, cx| {
        // Test harnesses wire grid_state directly, so load the metadata caches
        // the real open path fills.
        app.refresh_after_history(cx);
        app.selected_indices = [0usize, 1, 2].into_iter().collect();
        app.selected_index = Some(0);
        app.selected_item = Some(first.clone());
        app.update_batch_summary();
        cx.notify();
    });
    render_ui(vcx);

    view.update(vcx, |app, cx| {
        assert!(
            app.batch_metadata.title.mixed,
            "differing titles must be flagged mixed"
        );
        assert_eq!(app.batch_metadata.title.common, None);
        assert!(
            app.batch_metadata.caption.mixed,
            "differing captions must be flagged mixed"
        );
        assert_eq!(
            app.caption.title_input_buffer, "Mixed",
            "the batch inspector shows Mixed for differing titles"
        );
        assert_eq!(app.caption.input_buffer, "Mixed");

        app.start_title_input(cx);
        assert_eq!(
            app.caption.title_input_buffer, "",
            "a mixed draft starts empty so an unchanged Enter cannot overwrite N items"
        );
        app.caption.title_input_buffer = "Batch Title".to_string();
        app.commit_title_input(cx);
    });

    let titles: Vec<_> = ids
        .iter()
        .map(|id| engine.get_item_by_id(*id).unwrap().title)
        .collect();
    assert_eq!(
        titles,
        vec![Some("Batch Title".to_string()); 3],
        "the batch title must apply to every selected item"
    );
    view.update(vcx, |app, _| {
        assert_eq!(
            app.undo.undo_stack.len(),
            1,
            "applying to N items must create exactly one undo entry"
        );
        assert!(
            matches!(
                &app.undo.undo_stack[0],
                crate::app::undo::UndoAction::BatchMetadata {
                    field: crate::app::undo::BatchField::Title,
                    ..
                }
            ),
            "the entry must be the grouped batch variant"
        );
        assert!(!app.batch_metadata.title.mixed);
        assert_eq!(
            app.batch_metadata.title.common.as_deref(),
            Some("Batch Title"),
            "the summary refreshes after the apply"
        );
    });

    focus_app(vcx, &view);
    keystrokes(vcx, "cmd-z");
    let restored: Vec<_> = ids
        .iter()
        .map(|id| engine.get_item_by_id(*id).unwrap().title)
        .collect();
    assert_eq!(
        restored,
        vec![Some("First".to_string()), None, None],
        "one Cmd+Z must restore every item touched by the batch"
    );
    view.update(vcx, |app, _| {
        assert!(app.undo.undo_stack.is_empty());
        assert!(
            app.batch_metadata.title.mixed,
            "undo refreshes the mixed-value summary"
        );
    });

    // Same contract for captions, with the optimistic map reverted on undo.
    view.update(vcx, |app, cx| {
        app.start_caption_input(cx);
        assert_eq!(app.caption.input_buffer, "", "mixed captions start empty");
        app.caption.input_buffer = "Shared caption".to_string();
        app.commit_caption_input(cx);
    });
    view.update(vcx, |app, _| {
        for id in &ids {
            assert_eq!(
                app.item_captions.get(id).map(String::as_str),
                Some("Shared caption")
            );
        }
        assert_eq!(app.undo.undo_stack.len(), 1);
    });
    focus_app(vcx, &view);
    keystrokes(vcx, "cmd-z");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.item_captions.get(&first.id).map(String::as_str),
            Some("First caption")
        );
        assert!(!app.item_captions.contains_key(&ids[1]));
        assert!(!app.item_captions.contains_key(&ids[2]));
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn ui_automation_batch_inspector_metadata_edit_ui_path() {
    let (temp_dir, engine) = seed_library("batch_meta_ui", 3);
    let items = engine.get_item_slice(0..3);
    let first = items[0].clone();
    let second = items[1].clone();
    engine.set_title(first.id, Some("First")).unwrap();

    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    view.update(vcx, |app, cx| {
        app.inspector_open = true;
        app.selected_indices = [0usize, 1].into_iter().collect();
        app.selected_index = Some(0);
        app.selected_item = Some(first.clone());
        app.update_batch_summary();
        cx.notify();
    });
    render_ui(vcx);

    assert!(
        vcx.debug_bounds("batch-title-input").is_some(),
        "the batch inspector must render the title editor"
    );
    assert!(
        vcx.debug_bounds("batch-caption-input").is_some(),
        "the batch inspector must render the caption editor"
    );
    assert!(
        vcx.debug_bounds("batch-title-input-mixed").is_some(),
        "differing titles must show the Mixed marker"
    );

    focus_app(vcx, &view);
    click(vcx, "batch-title-input");
    view.update(vcx, |app, _| {
        assert!(
            app.caption.title_input_active,
            "clicking the batch title row opens the inline editor"
        );
        assert_eq!(
            app.caption.title_input_buffer, "",
            "mixed draft starts empty"
        );
    });
    type_text(vcx, "shared");
    keystrokes(vcx, "enter");

    view.update(vcx, |app, _| {
        assert!(!app.caption.title_input_active, "Enter closes the editor");
        assert_eq!(
            engine.get_item_by_id(first.id).unwrap().title.as_deref(),
            Some("shared")
        );
        assert_eq!(
            engine.get_item_by_id(second.id).unwrap().title.as_deref(),
            Some("shared")
        );
        assert_eq!(app.undo.undo_stack.len(), 1);
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}
