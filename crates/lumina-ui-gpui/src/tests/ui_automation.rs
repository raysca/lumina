//! Black-box-style UI automation against the real GPUI view tree.
//!
//! These tests drive the app with synthetic platform input (mouse clicks at
//! rendered element bounds, key events, window resizes) instead of calling app
//! methods directly, so layout, hit-testing, focus, overlays and event wiring are
//! all exercised.
#![allow(dead_code)]

use crate::tests::helpers::*;
use crate::*;

fn test_epoch_ms(value: &str) -> i64 {
    chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S")
        .expect("test fixture timestamps use the editor syntax")
        .and_utc()
        .timestamp_millis()
}

/// Selects items by identity so a rank-changing edit (for example a date
/// rewrite that reorders the grid) cannot silently move the selection to other
/// media before the next step of a test.
fn select_media_ids(app: &mut LuminaApp, ids: &[lumina_core::MediaId]) {
    let order = app.get_active_slice(0..app.get_active_count());
    let mut indices = std::collections::BTreeSet::new();
    for id in ids {
        if let Some(index) = order.iter().position(|item| item.id == *id) {
            indices.insert(index);
        }
    }
    let Some(&first_index) = indices.iter().next() else {
        return;
    };
    app.selected_indices = indices;
    app.selected_index = Some(first_index);
    app.selected_item = order.get(first_index).cloned();
    app.update_batch_summary();
}

#[core::prelude::v1::test]
fn ui_automation_renders_all_core_controls() {
    let (_temp_dir, engine) = seed_library("render", 8);
    let mut cx = crate::tests::TestApp::single();
    let (_view, vcx) = app_with_library(&mut cx, engine);

    assert!(vcx.debug_bounds("app-root").is_some());
    let selectors = [
        "sidebar-toggle",
        "search-input",
        "switch-library",
        "inspector-toggle",
        "page-title",
        "tags-toolbar-btn",
        "new-smart-album",
    ];
    let missing: Vec<_> = selectors
        .iter()
        .filter(|selector| vcx.debug_bounds(selector).is_none())
        .collect();
    assert!(missing.is_empty(), "controls not painted: {missing:?}");
}

#[core::prelude::v1::test]
fn ui_automation_toolbar_buttons_by_mouse() {
    let (_temp_dir, engine) = seed_library("toolbar", 8);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    click(vcx, "sidebar-toggle");
    view.update(vcx, |app, _| assert!(app.sidebar_collapsed));
    // One toggle: the rail has no expand button of its own.
    assert!(vcx.debug_bounds("expand-sidebar").is_none());
    click(vcx, "sidebar-toggle");
    view.update(vcx, |app, _| assert!(!app.sidebar_collapsed));

    click(vcx, "inspector-toggle");
    view.update(vcx, |app, _| assert!(app.inspector_open));
    click(vcx, "inspector-toggle");
    view.update(vcx, |app, _| assert!(!app.inspector_open));
}

#[core::prelude::v1::test]
fn ui_automation_search_click_type_clear() {
    let (_temp_dir, engine) = seed_library("search", 8);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    click(vcx, "search-input");
    view.update(vcx, |app, _| assert!(app.search_focused));

    // A query that matches the seeded photos, so the grid stays painted
    // once the results land.
    type_text(vcx, "photo");
    view.update(vcx, |app, _| {
        assert_eq!(app.search_query, "photo");
        assert!(app.search_focused, "typing keeps search focused");
    });

    // Clicking elsewhere must blur search so shortcuts work again. With the
    // layout switcher gone, an empty grid point is the neutral rendered target.
    let surface = vcx
        .debug_bounds("media-grid-surface")
        .expect("grid surface must be painted");
    let empty_point = gpui::point(
        surface.origin.x + gpui::px(24.0),
        surface.origin.y + surface.size.height - gpui::px(24.0),
    );
    vcx.simulate_click(empty_point, gpui::Modifiers::default());
    render_ui(vcx);
    view.update(vcx, |app, _| assert!(!app.search_focused));
    assert!(vcx.debug_bounds("clear-search").is_some());

    click(vcx, "search-input");
    click(vcx, "clear-search");
    view.update(vcx, |app, _| {
        assert!(app.search_query.is_empty());
        assert!(!app.search_focused);
    });
}

#[core::prelude::v1::test]
fn ui_automation_grid_and_viewer_journey() {
    let (_temp_dir, engine) = seed_library("journey", 8);
    let first_id = engine.get_item_slice(0..1)[0].id.0;
    let card = leak(format!("media-card-{}", first_id));

    let mut cx = crate::tests::TestApp::single();
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
}

#[core::prelude::v1::test]
fn ui_automation_viewer_back_button_returns_to_grid() {
    let (_temp_dir, engine) = seed_library("viewer_back", 8);
    let first_id = engine.get_item_slice(0..1)[0].id.0;
    let card = leak(format!("media-card-{}", first_id));

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    double_click(vcx, card);
    view.update(vcx, |app, _| {
        assert!(
            app.is_fullscreen_viewer,
            "double-click must open the viewer"
        );
    });

    let back = vcx
        .debug_bounds("viewer-back")
        .expect("the viewer must paint its Back button");
    let top_bar = vcx
        .debug_bounds("viewer-top-bar")
        .expect("the viewer must paint its top bar");
    assert!(
        back.left() >= top_bar.left() && back.right() <= top_bar.right(),
        "the Back button must live inside the viewer top bar: back={back:?} top_bar={top_bar:?}"
    );
    assert!(
        vcx.debug_bounds("nav-forward").is_none(),
        "the removed Forward control must not be painted"
    );

    click(vcx, "viewer-back");
    view.update(vcx, |app, _| {
        assert!(
            !app.is_fullscreen_viewer,
            "the viewer Back button must return to the grid"
        );
        assert_eq!(
            app.selected_item.as_ref().map(|item| item.id.0),
            Some(first_id),
            "the Back button must keep the grid selection"
        );
    });

    // Escape remains an equivalent keyboard exit.
    double_click(vcx, card);
    view.update(vcx, |app, _| assert!(app.is_fullscreen_viewer));
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| assert!(!app.is_fullscreen_viewer));
}

#[core::prelude::v1::test]
fn ui_automation_selection_contract_and_empty_space_click() {
    let (_temp_dir, engine) = seed_library("selection_contract", 8);
    let first_id = engine.get_item_slice(0..1)[0].id.0;
    let card = leak(format!("media-card-{}", first_id));

    let mut cx = crate::tests::TestApp::single();
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
}

#[core::prelude::v1::test]
fn ui_automation_keywords_row_opens_the_page_and_enter_renames() {
    let (_temp_dir, engine) = seed_library("tag_rename", 3);
    let item = engine.get_item_slice(0..1)[0].clone();
    engine
        .add_tags_batch(&[item.id], &["travel".to_string()])
        .unwrap();

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.refresh_album_counts();
        cx.notify();
    });
    render_ui(vcx);

    click(vcx, "sidebar-nav-keywords");
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("tag-manager-row-travel").is_some(),
        "TAGS › Keywords opens the Tags page with the tagged item's keyword"
    );
    click(vcx, "tag-manager-row-travel");
    focus_app(vcx, &view);
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.tag_manager.editing.as_deref(),
            Some("travel"),
            "↵ on a focused keyword begins its rename"
        );
    });
}

#[core::prelude::v1::test]
fn ui_automation_card_skeleton_selection_and_open() {
    let (_temp_dir, engine) = seed_library("card_states", 8);
    let first = engine.get_item_slice(0..1)[0].id;
    let first_id = first.0;
    let card = leak(format!("media-card-{}", first_id));
    let photo = leak(format!("media-card-photo-{}", first_id));
    let skeleton = leak(format!("media-card-skeleton-{}", first_id));
    let selected = leak(format!("media-card-selected-{}", first_id));

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_pending(&mut cx, engine, &[first]);

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
}

#[core::prelude::v1::test]
fn ui_automation_thumbnail_chrome_is_minimal() {
    let (_temp_dir, engine) = seed_library("card_chrome", 8);
    let first_id = engine.get_item_slice(0..1)[0].id.0;
    let card = leak(format!("media-card-{}", first_id));
    let selected = leak(format!("media-card-selected-{}", first_id));

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    // Fix the window so the rendered card width and the production row-height
    // formula use the same budget (the resize observer records the bounds).
    vcx.simulate_resize(gpui::size(gpui::px(1280.0), gpui::px(840.0)));
    render_ui(vcx);

    click(vcx, card);
    let card_bounds = vcx
        .debug_bounds(card)
        .expect("the card must still paint its hit target");
    let photo = leak(format!("media-card-photo-{}", first_id));
    let photo_bounds = vcx
        .debug_bounds(photo)
        .expect("the card must paint its photo well");
    let selected_bounds = vcx
        .debug_bounds(selected)
        .expect("selection must still paint its highlight");
    let selected_w = f32::from(selected_bounds.size.width);
    let selected_h = f32::from(selected_bounds.size.height);
    let photo_w = f32::from(photo_bounds.size.width);
    let photo_h = f32::from(photo_bounds.size.height);
    // A 2px ring with a 1px gap: 3px outside the well, half the 6px tile gap,
    // so the rings of two adjacent selected tiles never overlap.
    assert!(
        (selected_w - photo_w - 6.0).abs() <= 0.5 && (selected_h - photo_h - 6.0).abs() <= 0.5,
        "the ring must sit 3px outside the photo well: photo={photo_bounds:?} selected={selected_bounds:?}"
    );
    assert!(
        (f32::from(photo_bounds.left()) - f32::from(selected_bounds.left()) - 3.0).abs() <= 0.5
            && (f32::from(photo_bounds.top()) - f32::from(selected_bounds.top()) - 3.0).abs()
                <= 0.5,
        "the ring must be centred on the well: photo={photo_bounds:?} selected={selected_bounds:?}"
    );
    assert!(
        (selected_w - selected_h).abs() <= 2.0,
        "the thumbnail highlight must be square: selected={selected_bounds:?}"
    );
    assert!(
        photo_w > f32::from(card_bounds.size.width) * 0.8
            && photo_h > f32::from(card_bounds.size.height) * 0.8,
        "the photo well must still fill the card: card={card_bounds:?} photo={photo_bounds:?}"
    );
    // The removed check badge and hover "Open" hint never had debug
    // selectors, so absence assertions on them could not fail; the square,
    // photo-sized highlight above is the observable contract.

    double_click(vcx, card);
    view.update(vcx, |app, _| {
        assert!(
            app.is_fullscreen_viewer,
            "removing the hover hint must not remove the open action"
        );
    });
}

#[core::prelude::v1::test]
fn ui_automation_card_texture_replaces_skeleton() {
    let (_temp_dir, engine) = seed_library("card_reveal", 1);
    let item = engine.get_item_slice(0..1)[0].clone();
    let card_id = item.id.0;
    let photo = leak(format!("media-card-photo-{}", card_id));
    let skeleton = leak(format!("media-card-skeleton-{}", card_id));
    let thumb = leak(format!("media-card-thumb-{}", card_id));

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_pending(&mut cx, engine, &[item.id]);
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
}

#[core::prelude::v1::test]
fn ui_automation_card_photo_well_is_square_at_all_densities() {
    let (_temp_dir, engine) = seed_library("square_well", 8);
    let first = engine.get_item_slice(0..1)[0].clone();
    let first_id = first.id.0;
    engine.catalog().set_rating(first.id, 3).unwrap();
    let photo = leak(format!("media-card-photo-{}", first_id));
    let meta = leak(format!("media-card-meta-{}", first_id));
    let unrated_meta = leak(format!(
        "media-card-meta-{}",
        engine.get_item_slice(1..2)[0].id.0
    ));

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    // Fix the window so the rendered card width and the production formula use
    // the same budget; the resize observer records the new bounds before the
    // next draw.
    vcx.simulate_resize(gpui::size(gpui::px(1280.0), gpui::px(840.0)));
    render_ui(vcx);

    for columns in [2usize, 4, 8] {
        view.update(vcx, |app, cx| {
            app.grid_columns = columns;
            cx.notify();
        });
        render_ui(vcx);

        let photo_bounds = vcx
            .debug_bounds(photo)
            .unwrap_or_else(|| panic!("no photo well painted at {columns} columns"));
        let width = f32::from(photo_bounds.size.width);
        let height = f32::from(photo_bounds.size.height);
        assert!(
            (width - height).abs() <= 2.0,
            "the photo well must be square at {columns} columns, got {width}x{height}"
        );

        let expected = view.update(vcx, |app, _| app.grid_metrics().photo_side);
        assert!(
            (height - expected).abs() <= 2.0,
            "the rendered well ({height}) must use the derived side ({expected}) at {columns} columns"
        );

        // No band under the photo. Rating and label ride in a pill
        // on the thumbnail, and only when there is something to show. Compact
        // tiles hide the pill (small_tiles_keep_state_badges_apart).
        if expected < crate::views::grid::COMPACT_TILE {
            continue;
        }
        let meta_bounds = vcx.debug_bounds(meta).unwrap_or_else(|| {
            panic!("a rated card must paint its meta pill at {columns} columns")
        });
        assert!(
            photo_bounds.contains(&meta_bounds.origin)
                && photo_bounds.contains(&meta_bounds.bottom_right()),
            "the meta pill must sit inside the photo well at {columns} columns"
        );
        assert!(
            vcx.debug_bounds(unrated_meta).is_none(),
            "an unmarked card must not paint an empty meta pill"
        );
    }

    // A live resize must relayout from the same formula, not a stale width.
    view.update(vcx, |app, cx| {
        app.grid_columns = 4;
        cx.notify();
    });
    vcx.simulate_resize(gpui::size(gpui::px(1440.0), gpui::px(900.0)));
    render_ui(vcx);
    let resized = vcx
        .debug_bounds(photo)
        .expect("the photo well must survive a window resize");
    let width = f32::from(resized.size.width);
    let height = f32::from(resized.size.height);
    assert!(
        (width - height).abs() <= 2.0,
        "the photo well must stay square after a resize, got {width}x{height}"
    );

    // The docked inspector narrows the grid: its width and 4px resizer must be
    // part of the same budget.
    view.update(vcx, |app, cx| {
        app.selected_item = app.get_active_slice(0..1).first().cloned();
        app.inspector_open = true;
        cx.notify();
    });
    render_ui(vcx);
    let with_inspector = vcx
        .debug_bounds(photo)
        .expect("the photo well must stay painted with the inspector open");
    let width = f32::from(with_inspector.size.width);
    let height = f32::from(with_inspector.size.height);
    let expected = view.update(vcx, |app, _| app.grid_metrics().photo_side);
    assert!(
        (width - height).abs() <= 2.0 && (height - expected).abs() <= 2.0,
        "the inspector chrome must be budgeted: well {width}x{height}, expected {expected}"
    );
}

#[core::prelude::v1::test]
fn ui_automation_card_tooltip_carries_the_filename() {
    let (_temp_dir, engine) = seed_library("card_tooltip", 1);
    let item = engine.get_item_slice(0..1)[0].clone();
    let card = leak(format!("media-card-{}", item.id.0));

    let mut cx = crate::tests::TestApp::single();
    let (_view, vcx) = app_with_library(&mut cx, engine);

    let center = bounds_center(vcx, card);
    vcx.simulate_mouse_move(center, None, gpui::Modifiers::default());
    // GPUI waits 500ms before building a tooltip.
    vcx.background_executor
        .advance_clock(std::time::Duration::from_millis(600));
    vcx.run_until_parked();
    render_ui(vcx);

    assert!(
        vcx.debug_bounds("media-card-name-tooltip").is_some(),
        "hovering a card must show the filename tooltip that replaced the label"
    );
}

#[core::prelude::v1::test]
fn ui_automation_empty_search_state_offers_a_clear_action() {
    let (_temp_dir, engine) = seed_library("empty_search", 8);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, cx| {
        app.set_search_query("no-such-photo", cx);
    });
    render_ui(vcx);
    assert!(vcx.debug_bounds("empty-state").is_some());
    assert!(
        vcx.debug_bounds("empty-primary").is_none(),
        "no-results offers term chips and Clear all filters, never a primary button"
    );

    click(vcx, "empty-search-term-0");
    view.update(vcx, |app, _| {
        assert!(
            app.search_query.is_empty(),
            "removing the only term's chip clears the search"
        );
        assert!(app.selected_item.is_none());
    });
}

#[core::prelude::v1::test]
fn ui_automation_modals_dismiss_on_backdrop() {
    let (_temp_dir, engine) = seed_library("modals", 8);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    // The Tags page is a page, not a sheet: no backdrop to dismiss.
    click(vcx, "tags-toolbar-btn");
    view.update(vcx, |app, _| assert!(app.tags_page_shown()));
    assert!(vcx.debug_bounds("tags-page").is_some());
    assert!(vcx.debug_bounds("tag-manager-backdrop").is_none());
    view.update(vcx, |app, cx| app.navigate_back(cx));
    view.update(vcx, |app, _| assert!(!app.tags_page_shown()));

    click(vcx, "new-smart-album");
    view.update(vcx, |app, _| assert!(app.album_draft.modal_open));
    assert!(vcx.debug_bounds("album-builder-card").is_some());
    click_point(vcx, 4.0, 4.0);
    view.update(vcx, |app, _| assert!(!app.album_draft.modal_open));
}

#[core::prelude::v1::test]
fn ui_automation_shortcut_cheat_sheet() {
    let (_temp_dir, engine) = seed_library("shortcuts", 8);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    keystrokes(vcx, "secondary-/");
    view.update(vcx, |app, _| assert!(app.shortcuts_overlay_open));
    assert!(vcx.debug_bounds("shortcuts-card").is_some());

    // Keystrokes are captured by the sheet while open.
    keystrokes(vcx, "i");
    view.update(vcx, |app, _| assert!(!app.inspector_open));

    click(vcx, "shortcuts-close");
    view.update(vcx, |app, _| assert!(!app.shortcuts_overlay_open));

    keystrokes(vcx, "secondary-/");
    click_point(vcx, 4.0, 4.0);
    view.update(vcx, |app, _| assert!(!app.shortcuts_overlay_open));
}

/// The sheet is taller than its card, so its lower rows must be reachable
/// without a mouse (AGENTS.md section 11).
#[core::prelude::v1::test]
fn ui_automation_shortcut_sheet_scrolls_from_the_keyboard() {
    let (_temp_dir, engine) = seed_library("shortcuts_scroll", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    keystrokes(vcx, "secondary-/");
    render_ui(vcx);
    let offset = |vcx: &mut VisualTestContext| {
        view.update(vcx, |app, _| {
            let max = app.shortcuts_scroll.max_offset().y;
            assert!(max > px(0.0), "the sheet overflows its card, so it scrolls");
            (app.shortcuts_scroll.offset().y, max)
        })
    };
    assert_eq!(offset(vcx).0, px(0.0), "the sheet opens at the top");

    keystrokes(vcx, "down");
    let (y, _) = offset(vcx);
    assert!(y < px(0.0), "Down scrolls the sheet");
    keystrokes(vcx, "end");
    let (y, max) = offset(vcx);
    assert_eq!(y, -max, "End reaches the last row");
    keystrokes(vcx, "pageup");
    assert!(offset(vcx).0 > -max, "PageUp scrolls back up");
    keystrokes(vcx, "home");
    assert_eq!(offset(vcx).0, px(0.0), "Home returns to the top");
    view.update(vcx, |app, _| {
        assert!(
            app.shortcuts_overlay_open,
            "scroll keys keep the sheet open"
        );
        assert_eq!(app.selected_indices.len(), 0, "and never move the grid");
    });

    keystrokes(vcx, "end");
    keystrokes(vcx, "escape");
    keystrokes(vcx, "secondary-/");
    assert_eq!(offset(vcx).0, px(0.0), "reopening starts at the top again");
}

/// Regression: O / Open in Default App on a file that has gone away did
/// nothing at all; it now says so in the error banner.
#[core::prelude::v1::test]
fn ui_automation_open_in_default_app_reports_a_missing_file() {
    let (_temp_dir, engine) = seed_library("open_default_missing", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    keystrokes(vcx, "home enter");
    let path = view.update(vcx, |app, _| {
        assert!(app.is_fullscreen_viewer, "Enter opens the viewer");
        app.selected_item
            .as_ref()
            .expect("an item is open")
            .path
            .clone()
    });
    std::fs::remove_file(&path).unwrap();
    keystrokes(vcx, "o");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.error_message.as_deref(),
            Some(crate::app::viewer_actions::EXTERNAL_EDITOR_FILE_MISSING_MESSAGE),
            "a vanished file must be reported, not silently ignored"
        );
    });
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
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.selected_item = Some(item.clone());
        app.export_to_folder(
            destination.clone(),
            lumina_core::ExportPreset::Originals,
            cx,
        );
    });
    // The copy runs in the background.
    render_ui(vcx);

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
}

#[core::prelude::v1::test]
fn ui_automation_undo_redo_rating_and_sidecar() {
    let (_temp_dir, engine) = seed_library("undo", 3);
    let item = engine.get_item_slice(0..1)[0].clone();
    let sidecar = lumina_core::sidecar_path(&item.path);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, cx| {
        app.selected_item = Some(item.clone());
        app.set_rating_selected(4, cx);
    });
    // Sidecars are written on the background executor.
    vcx.run_until_parked();
    assert!(sidecar.exists(), "rating should write an XMP sidecar");

    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-z");
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
    keystrokes(vcx, "secondary-shift-z");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.item_ratings.get(&item.id).copied().unwrap_or(0),
            4,
            "redo should restore the rating"
        );
    });
    assert!(sidecar.exists(), "redo should rewrite the sidecar");
}

#[core::prelude::v1::test]
fn ui_automation_undo_tag_and_caption() {
    let (_temp_dir, engine) = seed_library("undo_meta", 3);
    let item = engine.get_item_slice(0..1)[0].clone();
    let mut cx = crate::tests::TestApp::single();
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
    keystrokes(vcx, "secondary-z");
    view.update(vcx, |app, _| {
        assert!(
            !app.selected_item_tags.contains(&"travel".to_string()),
            "undo should remove the tag"
        );
    });
    keystrokes(vcx, "secondary-shift-z");
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
    keystrokes(vcx, "secondary-z");
    view.update(vcx, |app, _| {
        assert!(!app.item_captions.contains_key(&item.id));
    });
}

#[core::prelude::v1::test]
fn ui_automation_title_edit_undo_and_xmp_import() {
    let (_temp_dir, engine) = seed_library("title_import", 3);
    let item = engine.get_item_slice(0..1)[0].clone();
    let sidecar = lumina_core::sidecar_path(&item.path);
    let mut cx = crate::tests::TestApp::single();
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
    vcx.run_until_parked();
    let written = lumina_core::read_sidecar(&item.path).unwrap().unwrap();
    assert_eq!(written.title.as_deref(), Some("Kyoto Trip"));

    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-z");
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
            color_label: 0,
            tags: vec!["imported".to_string()],
            ..Default::default()
        },
    )
    .unwrap();
    view.update(vcx, |app, cx| {
        app.import_sidecars_selected(cx);
    });
    // The sidecars are read in the background.
    render_ui(vcx);
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
}

#[core::prelude::v1::test]
fn ui_automation_date_and_location_edit_undo() {
    let (_temp_dir, engine) = seed_library("edit_time_place", 3);
    let item = engine.get_item_slice(0..1)[0].clone();
    let original_timestamp = item.timestamp_epoch_ms;
    let mut cx = crate::tests::TestApp::single();
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
    keystrokes(vcx, "secondary-z");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_item.as_ref().map(|it| it.timestamp_epoch_ms),
            Some(original_timestamp),
            "undo should restore the original date"
        );
    });
    keystrokes(vcx, "secondary-shift-z");
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
    keystrokes(vcx, "secondary-z");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_item.as_ref().and_then(|it| it.latitude),
            None,
            "undo should clear the location"
        );
    });
}

#[core::prelude::v1::test]
fn ui_automation_command_palette() {
    let (_temp_dir, engine) = seed_library("command_palette", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    render_ui(vcx);
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-k");
    view.update(vcx, |app, _| assert!(app.command_palette_open));

    for key in ["p", "r", "e", "f"] {
        keystrokes(vcx, key);
    }
    view.update(vcx, |app, _| {
        assert_eq!(app.command_query, "pref");
        let titles: Vec<String> = app
            .palette_rows()
            .into_iter()
            .map(|row| row.title)
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
    keystrokes(vcx, "secondary-k");
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
    keystrokes(vcx, "secondary-k");
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| assert!(!app.command_palette_open));
}

#[core::prelude::v1::test]
fn ui_automation_preferences_modal() {
    let (_temp_dir, engine) = seed_library("preferences", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    render_ui(vcx);
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-,");
    view.update(vcx, |app, _| {
        assert!(app.preferences_open, "Cmd+, should open preferences");
        assert!(app.preferences_cache.is_some());
    });

    click(vcx, "settings-category-storage");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.preferences_tab,
            crate::app::preferences::PreferencesTab::Storage
        );
    });
    click(vcx, "settings-category-about");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.preferences_tab,
            crate::app::preferences::PreferencesTab::About
        );
    });
    click(vcx, "preferences-close");
    view.update(vcx, |app, _| assert!(!app.preferences_open));

    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-,");
    view.update(vcx, |app, _| assert!(app.preferences_open));
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(!app.preferences_open, "Escape should close preferences");
    });
}

#[core::prelude::v1::test]
fn ui_automation_sort_menu_orders_grid() {
    let (_temp_dir, engine) = seed_library("sorting", 5);
    let mut cx = crate::tests::TestApp::single();
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
}

#[core::prelude::v1::test]
fn ui_automation_smart_album_edit_and_delete() {
    let (_temp_dir, engine) = seed_library("smart_album_mgmt", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, cx| {
        app.open_album_builder(cx);
        app.album_draft.album_title = "Round Trip".to_string();
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
    click(vcx, "smart-album-delete-confirm");
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
}

#[core::prelude::v1::test]
fn ui_automation_resize_updates_persisted_bounds() {
    let (_temp_dir, engine) = seed_library("resize", 8);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    vcx.simulate_resize(gpui::size(gpui::px(1100.0), gpui::px(720.0)));
    render_ui(vcx);
    view.update(vcx, |app, _| {
        let (_, _, width, height) = app.window_bounds.expect("bounds observer should fire");
        assert!((width - 1100.0).abs() < 2.0, "width was {width}");
        assert!((height - 720.0).abs() < 2.0, "height was {height}");
    });
}

#[core::prelude::v1::test]
fn ui_automation_navigation_album_back() {
    let (_temp_dir, engine) = seed_library("nav_album", 8);
    let mut cx = crate::tests::TestApp::single();
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

    keystrokes(vcx, "secondary-[");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::AllLibrary,
            "Back must return to the previous album"
        );
    });

    // Edge: nothing precedes the oldest entry, so Back is a disabled no-op.
    keystrokes(vcx, "secondary-[");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::AllLibrary,
            "Back at the oldest entry must stay put"
        );
    });
}

#[core::prelude::v1::test]
fn ui_automation_navigation_search_back() {
    let (_temp_dir, engine) = seed_library("nav_search", 8);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, cx| app.set_search_query("is:video", cx));
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.search_query, "is:video");
    });

    keystrokes(vcx, "secondary-[");
    view.update(vcx, |app, _| {
        assert!(
            app.search_query.is_empty(),
            "Back must restore the pre-search view"
        );
        assert_eq!(app.selection, AlbumSelection::AllLibrary);
    });
}

#[core::prelude::v1::test]
fn ui_automation_navigation_edge_no_ops() {
    let (_temp_dir, engine) = seed_library("nav_edges", 8);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    render_ui(vcx);
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-[");
    // Cmd+] with nothing ahead is a no-op.
    keystrokes(vcx, "secondary-]");
    keystrokes(vcx, "secondary-[");
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

    keystrokes(vcx, "secondary-[");
    view.update(vcx, |app, _| {
        assert_eq!(app.selection, AlbumSelection::AllLibrary);
    });
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-[");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::AllLibrary,
            "Cmd+[ at the oldest entry must stay put"
        );
    });
    // Cmd+] redoes the Back; at the newest entry it stays put.
    keystrokes(vcx, "secondary-]");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::Trash,
            "Cmd+] must go Forward to the album Back left"
        );
    });
    keystrokes(vcx, "secondary-]");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::Trash,
            "Cmd+] at the newest entry must stay put"
        );
    });
}

#[core::prelude::v1::test]
fn ui_automation_search_suggestions_filter_and_keyboard_apply() {
    let (_temp_dir, engine) = seed_library("search_suggestions", 3);
    let mut cx = crate::tests::TestApp::single();
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
}

#[core::prelude::v1::test]
fn ui_automation_search_suggestion_click_applies_without_blurring() {
    let (_temp_dir, engine) = seed_library("search_suggestion_click", 3);
    let mut cx = crate::tests::TestApp::single();
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
}

#[core::prelude::v1::test]
fn ui_automation_grid_drag_selects_range_and_suppresses_click() {
    use std::collections::BTreeSet;

    let (_temp_dir, engine) = seed_library("grid_drag_range", 8);
    let items = engine.get_item_slice(0..8);
    let cards: Vec<&'static str> = items
        .iter()
        .map(|item| leak(format!("media-card-{}", item.id.0)))
        .collect();

    let mut cx = crate::tests::TestApp::single();
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
    assert!(
        vcx.debug_bounds("grid-selection-rect").is_none(),
        "the marquee rectangle is gone after mouse-up"
    );

    // Suppression must be consumed: the next genuine click is still a single select.
    click(vcx, cards[1]);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_indices,
            BTreeSet::from([1usize]),
            "a plain click after a drag selects exactly one card"
        );
    });
}

#[core::prelude::v1::test]
fn ui_automation_grid_drag_across_rows_selects_a_rectangle() {
    use std::collections::BTreeSet;

    let (_temp_dir, engine) = seed_library("grid_drag_rect", 12);
    let items = engine.get_item_slice(0..12);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.grid_columns = 4;
        cx.notify();
    });
    render_ui(vcx);

    // Card 1 (row 0, col 1) to card 6 (row 1, col 2).
    let start = bounds_center(vcx, leak(format!("media-card-{}", items[1].id.0)));
    let end = bounds_center(vcx, leak(format!("media-card-{}", items[6].id.0)));
    vcx.simulate_mouse_down(start, gpui::MouseButton::Left, gpui::Modifiers::default());
    render_ui(vcx);
    vcx.simulate_mouse_move(
        end,
        Some(gpui::MouseButton::Left),
        gpui::Modifiers::default(),
    );
    render_ui(vcx);
    vcx.simulate_mouse_up(end, gpui::MouseButton::Left, gpui::Modifiers::default());
    render_ui(vcx);

    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_indices,
            BTreeSet::from([1usize, 2, 5, 6]),
            "a marquee across rows selects the swept rectangle, not the index run between corners"
        );
    });
}

#[core::prelude::v1::test]
fn ui_automation_grid_drag_from_selected_card_preserves_multi_selection() {
    use std::collections::BTreeSet;

    let (_temp_dir, engine) = seed_library("grid_drag_preserve", 8);
    let items = engine.get_item_slice(0..8);
    let cards: Vec<&'static str> = items
        .iter()
        .map(|item| leak(format!("media-card-{}", item.id.0)))
        .collect();

    let mut cx = crate::tests::TestApp::single();
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
}

#[core::prelude::v1::test]
fn ui_automation_grid_drag_with_cmd_is_additive() {
    use std::collections::BTreeSet;

    let (_temp_dir, engine) = seed_library("grid_drag_cmd", 8);
    let items = engine.get_item_slice(0..8);
    let cards: Vec<&'static str> = items
        .iter()
        .map(|item| leak(format!("media-card-{}", item.id.0)))
        .collect();

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    click(vcx, cards[0]);
    view.update(vcx, |app, _| {
        assert_eq!(app.selected_indices, BTreeSet::from([0usize]));
    });

    let cmd = primary_modifier();
    // Same row at the default column count, so the swept rectangle is 2..=3.
    let start = bounds_center(vcx, cards[2]);
    let end = bounds_center(vcx, cards[3]);
    vcx.simulate_mouse_down(start, gpui::MouseButton::Left, cmd);
    render_ui(vcx);
    vcx.simulate_mouse_move(end, Some(gpui::MouseButton::Left), cmd);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_indices,
            BTreeSet::from([0usize, 2, 3]),
            "Cmd-drag adds the swept range to the existing selection"
        );
        assert_eq!(app.grid_drag.anchor, Some(2));
    });

    vcx.simulate_mouse_up(end, gpui::MouseButton::Left, cmd);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_indices,
            BTreeSet::from([0usize, 2, 3]),
            "the additive selection survives the release"
        );
        assert!(!app.grid_drag.active);
    });
}

#[core::prelude::v1::test]
fn ui_automation_grid_drag_from_empty_background_sweeps_and_escape_resets() {
    use std::collections::BTreeSet;

    let (_temp_dir, engine) = seed_library("grid_drag_bg", 8);
    let items = engine.get_item_slice(0..8);
    let cards: Vec<&'static str> = items
        .iter()
        .map(|item| leak(format!("media-card-{}", item.id.0)))
        .collect();

    let mut cx = crate::tests::TestApp::single();
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
}

#[core::prelude::v1::test]
fn ui_automation_viewer_wheel_zoom_is_smooth() {
    let (_temp_dir, engine) = seed_library("wheel_smooth", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let item = view.update(vcx, |app, _| {
        app.get_active_slice(0..1)
            .into_iter()
            .next()
            .expect("seeded library must have an item")
    });
    view.update(vcx, |app, cx| app.open_viewer(item, cx));
    render_ui(vcx);

    let center = vcx
        .debug_bounds("viewer-viewport")
        .expect("the viewer must paint its viewport")
        .center();
    let step = 40.0;
    wheel(
        vcx,
        center,
        gpui::ScrollDelta::Pixels(gpui::point(gpui::px(0.0), gpui::px(step))),
        primary_modifier(),
    );
    let one = view.update(vcx, |app, _| app.viewer_state.zoom_scale);
    assert!(
        (one - 0.1_f32.exp()).abs() < 0.02,
        "40px of ⌘-wheel must zoom smoothly by ~10%, got {one}"
    );
    wheel(
        vcx,
        center,
        gpui::ScrollDelta::Pixels(gpui::point(gpui::px(0.0), gpui::px(step))),
        primary_modifier(),
    );
    let two = view.update(vcx, |app, _| app.viewer_state.zoom_scale);
    assert!(
        (two - one * 0.1_f32.exp()).abs() < 0.02,
        "a second wheel event must zoom by the same ~10% ratio, got {two}"
    );
    assert!(
        (two - 1.25).abs() > 0.02,
        "wheel zoom must not snap to the old 0.25 keyboard step"
    );
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
        column_gap: crate::views::grid::GRID_CARD_GAP,
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
        column_gap: crate::views::grid::GRID_CARD_GAP,
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

    let (_temp_dir, engine) = seed_library("grid_wheel_zoom", 40);
    let mut cx = crate::tests::TestApp::single();
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
            column_gap: app.grid_metrics().column_gap,
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
        primary_modifier(),
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
        primary_modifier(),
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
}

#[core::prelude::v1::test]
fn ui_automation_batch_title_and_caption_mixed_apply_and_grouped_undo() {
    let (_temp_dir, engine) = seed_library("batch_meta", 4);
    let items = engine.get_item_slice(0..4);
    let first = items[0].clone();
    let ids: Vec<_> = items[0..3].iter().map(|item| item.id).collect();
    engine.set_title(first.id, Some("First")).unwrap();
    engine.set_caption(first.id, "First caption").unwrap();

    let mut cx = crate::tests::TestApp::single();
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
    keystrokes(vcx, "secondary-z");
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
    keystrokes(vcx, "secondary-z");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.item_captions.get(&first.id).map(String::as_str),
            Some("First caption")
        );
        assert!(!app.item_captions.contains_key(&ids[1]));
        assert!(!app.item_captions.contains_key(&ids[2]));
    });
}

#[core::prelude::v1::test]
fn ui_automation_batch_date_mixed_apply_validation_and_grouped_undo() {
    let (_temp_dir, engine) = seed_library("batch_date", 4);
    let dates = [
        test_epoch_ms("2020-01-02 03:04:05"),
        test_epoch_ms("2021-02-03 04:05:06"),
        test_epoch_ms("2022-03-04 05:06:07"),
        test_epoch_ms("2023-05-06 07:08:09"),
    ];
    for (item, date) in engine.get_item_slice(0..4).iter().zip(dates) {
        engine.set_timestamp(item.id, date).unwrap();
    }

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    // Derive the selection from the app's own sorted view: changing timestamps
    // reorders the grid, so raw catalog order is not the rendered order.
    let (ids, originals): (Vec<_>, Vec<_>) = view.update(vcx, |app, cx| {
        let batch = app.get_active_slice(0..3);
        app.refresh_after_history(cx);
        app.selected_indices = (0..batch.len()).collect();
        app.selected_index = Some(0);
        app.selected_item = Some(batch[0].clone());
        app.update_batch_summary();
        cx.notify();
        (
            batch.iter().map(|item| item.id).collect(),
            batch.iter().map(|item| item.timestamp_epoch_ms).collect(),
        )
    });
    render_ui(vcx);

    view.update(vcx, |app, cx| {
        assert!(
            app.batch_metadata.date.mixed,
            "differing capture dates must be flagged mixed"
        );
        assert_eq!(app.batch_metadata.date.common, None);
        assert_eq!(
            app.caption.date_input_buffer, "Mixed",
            "the batch inspector shows Mixed for differing dates"
        );

        // Invalid syntax must fail validation without touching any item.
        app.start_date_input(cx);
        app.caption.date_input_buffer = "not-a-date".to_string();
        app.commit_date_input(cx);
    });
    view.update(vcx, |app, _| {
        assert!(
            app.error_message
                .as_deref()
                .unwrap_or("")
                .contains("Enter a date"),
            "invalid date must surface a validation error, got {:?}",
            app.error_message
        );
        assert!(
            app.undo.undo_stack.is_empty(),
            "a rejected draft must not push an undo entry"
        );
    });
    let unchanged: Vec<_> = ids
        .iter()
        .map(|id| engine.get_item_by_id(*id).unwrap().timestamp_epoch_ms)
        .collect();
    assert_eq!(
        unchanged, originals,
        "an invalid draft must not change any timestamp"
    );

    // A Mixed draft opens empty, so committing it untouched is a no-op.
    view.update(vcx, |app, cx| {
        app.dismiss_error(cx);
        app.start_date_input(cx);
        assert_eq!(
            app.caption.date_input_buffer, "",
            "a mixed draft starts empty so an unchanged Enter cannot rewrite N items"
        );
        app.commit_date_input(cx);
    });
    view.update(vcx, |app, _| {
        assert!(
            app.undo.undo_stack.is_empty(),
            "an untouched mixed draft must be a no-op"
        );
    });
    let still_unchanged: Vec<_> = ids
        .iter()
        .map(|id| engine.get_item_by_id(*id).unwrap().timestamp_epoch_ms)
        .collect();
    assert_eq!(still_unchanged, originals);

    view.update(vcx, |app, cx| {
        app.start_date_input(cx);
        app.caption.date_input_buffer = "2023-06-07 08:09:10".to_string();
        app.commit_date_input(cx);
    });
    let applied: Vec<_> = ids
        .iter()
        .map(|id| engine.get_item_by_id(*id).unwrap().timestamp_epoch_ms)
        .collect();
    assert_eq!(
        applied,
        vec![test_epoch_ms("2023-06-07 08:09:10"); ids.len()],
        "the batch date must apply to every selected item"
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
                crate::app::undo::UndoAction::BatchDate { .. }
            ),
            "the entry must be the grouped batch date variant"
        );
        assert!(!app.batch_metadata.date.mixed);
        assert_eq!(
            app.batch_metadata.date.common.as_deref(),
            Some("2023-06-07 08:09:10")
        );
    });

    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-z");
    let restored: Vec<_> = ids
        .iter()
        .map(|id| engine.get_item_by_id(*id).unwrap().timestamp_epoch_ms)
        .collect();
    assert_eq!(
        restored, originals,
        "one Cmd+Z must restore every date touched by the batch"
    );
    view.update(vcx, |app, _| {
        assert!(app.undo.undo_stack.is_empty());
        assert!(
            app.batch_metadata.date.mixed,
            "undo refreshes the mixed-value summary"
        );
    });
}

#[core::prelude::v1::test]
fn ui_automation_batch_location_mixed_apply_validation_and_grouped_undo() {
    let (_temp_dir, engine) = seed_library("batch_location", 4);
    let locations = [
        (Some(48.8566), Some(2.3522)),
        (Some(35.6762), Some(139.6503)),
        (None, None),
        (Some(51.5074), Some(-0.1278)),
    ];
    for (item, (lat, lon)) in engine.get_item_slice(0..4).iter().zip(locations) {
        engine.set_location(item.id, lat, lon).unwrap();
    }

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    // Read the selection from the rendered (sorted) view so the assertions do
    // not depend on raw catalog order.
    let (ids, originals): (Vec<_>, Vec<_>) = view.update(vcx, |app, cx| {
        let batch = app.get_active_slice(0..3);
        app.refresh_after_history(cx);
        app.selected_indices = (0..batch.len()).collect();
        app.selected_index = Some(0);
        app.selected_item = Some(batch[0].clone());
        app.update_batch_summary();
        cx.notify();
        (
            batch.iter().map(|item| item.id).collect(),
            batch
                .iter()
                .map(|item| (item.latitude, item.longitude))
                .collect(),
        )
    });
    render_ui(vcx);

    view.update(vcx, |app, cx| {
        assert!(
            app.batch_metadata.location.mixed,
            "differing coordinates (including none) must be flagged mixed"
        );
        assert_eq!(
            app.caption.location_input_buffer, "Mixed",
            "the batch inspector shows Mixed for differing locations"
        );

        app.start_location_input(cx);
        assert_eq!(
            app.caption.location_input_buffer, "",
            "a mixed location draft starts empty"
        );
        app.commit_location_input(cx);
    });
    view.update(vcx, |app, _| {
        assert!(
            app.undo.undo_stack.is_empty(),
            "an untouched mixed location draft must be a no-op"
        );
    });

    view.update(vcx, |app, cx| {
        app.start_location_input(cx);
        app.caption.location_input_buffer = "not coordinates".to_string();
        app.commit_location_input(cx);
    });
    view.update(vcx, |app, _| {
        assert!(
            app.error_message
                .as_deref()
                .unwrap_or("")
                .contains("Coordinates"),
            "invalid coordinates must surface a validation error, got {:?}",
            app.error_message
        );
        assert!(
            app.undo.undo_stack.is_empty(),
            "a rejected location draft must not push an undo entry"
        );
    });
    let unchanged: Vec<_> = ids
        .iter()
        .map(|id| {
            let item = engine.get_item_by_id(*id).unwrap();
            (item.latitude, item.longitude)
        })
        .collect();
    assert_eq!(
        unchanged, originals,
        "an invalid draft must not change any location"
    );

    let target = (40.7128, -74.0060);
    view.update(vcx, |app, cx| {
        app.dismiss_error(cx);
        app.start_location_input(cx);
        app.caption.location_input_buffer = "40.7128, -74.0060".to_string();
        app.commit_location_input(cx);
    });
    let applied: Vec<_> = ids
        .iter()
        .map(|id| {
            let item = engine.get_item_by_id(*id).unwrap();
            (item.latitude, item.longitude)
        })
        .collect();
    assert_eq!(
        applied,
        vec![(Some(target.0), Some(target.1)); ids.len()],
        "the batch location must apply to every selected item"
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
                crate::app::undo::UndoAction::BatchLocation { .. }
            ),
            "the entry must be the grouped batch location variant"
        );
        assert!(!app.batch_metadata.location.mixed);
    });

    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-z");
    let restored: Vec<_> = ids
        .iter()
        .map(|id| {
            let item = engine.get_item_by_id(*id).unwrap();
            (item.latitude, item.longitude)
        })
        .collect();
    assert_eq!(
        restored, originals,
        "one Cmd+Z must restore every location touched by the batch"
    );
    view.update(vcx, |app, _| {
        assert!(app.undo.undo_stack.is_empty());
        assert!(
            app.batch_metadata.location.mixed,
            "undo refreshes the mixed-value summary"
        );
    });
}

#[core::prelude::v1::test]
fn ui_automation_batch_inspector_metadata_edit_ui_path() {
    let (_temp_dir, engine) = seed_library("batch_meta_ui", 3);
    let annotated = engine.get_item_slice(0..1)[0].clone();
    engine.set_title(annotated.id, Some("First")).unwrap();
    engine
        .set_timestamp(annotated.id, test_epoch_ms("2020-01-02 03:04:05"))
        .unwrap();
    engine
        .set_location(annotated.id, Some(48.8566), Some(2.3522))
        .unwrap();

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    // Only the annotated item differs from its neighbour, so the title, date
    // and location rows all show the Mixed marker.
    let (first, second) = view.update(vcx, |app, cx| {
        app.inspector_open = true;
        let order = app.get_active_slice(0..app.get_active_count());
        let first = order
            .iter()
            .find(|item| item.id == annotated.id)
            .cloned()
            .expect("the annotated item stays in the library");
        let second = order
            .iter()
            .find(|item| item.id != annotated.id)
            .cloned()
            .expect("the seeded library has another item");
        select_media_ids(app, &[first.id, second.id]);
        cx.notify();
        (first, second)
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
    assert!(
        vcx.debug_bounds("batch-date-input").is_some(),
        "the batch inspector must render the date editor"
    );
    assert!(
        vcx.debug_bounds("batch-location-input").is_some(),
        "the batch inspector must render the location editor"
    );
    assert!(
        vcx.debug_bounds("batch-date-input-mixed").is_some(),
        "differing dates must show the Mixed marker"
    );
    assert!(
        vcx.debug_bounds("batch-location-input-mixed").is_some(),
        "differing locations must show the Mixed marker"
    );
    for row in [
        "batch-title-input-value",
        "batch-date-input-value",
        "batch-location-input-value",
    ] {
        assert!(
            vcx.debug_bounds(row).is_none(),
            "{row}: a mixed row says Mixed once, in its marker, not again as its value"
        );
    }
    assert!(
        vcx.debug_bounds("batch-caption-input-value").is_some(),
        "a shared value still shows in its row"
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
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("batch-title-input-value").is_some(),
        "editing a mixed row shows its draft"
    );
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

    // Date and location use the same inline buffer path as title/caption, so
    // they must be driveable through the rendered rows and the keyboard.
    click(vcx, "batch-date-input");
    view.update(vcx, |app, _| {
        assert!(
            app.caption.date_input_active,
            "clicking the batch date row opens the inline editor"
        );
    });
    type_text(vcx, "2020-05-06");
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        for id in [first.id, second.id] {
            let item = engine.get_item_by_id(id).unwrap();
            assert_eq!(
                (item.year, item.month, item.day),
                (2020, 5, 6),
                "the typed batch date must reach every selected item"
            );
        }
        assert_eq!(app.undo.undo_stack.len(), 2);
    });

    // The date rewrite can reorder the grid; re-anchor the selection by id.
    view.update(vcx, |app, cx| {
        select_media_ids(app, &[first.id, second.id]);
        cx.notify();
    });
    render_ui(vcx);

    click(vcx, "batch-location-input");
    view.update(vcx, |app, _| {
        assert!(
            app.caption.location_input_active,
            "clicking the batch location row opens the inline editor"
        );
    });
    type_text(vcx, "40.7128, -74.0060");
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        for id in [first.id, second.id] {
            let item = engine.get_item_by_id(id).unwrap();
            assert_eq!(
                (item.latitude, item.longitude),
                (Some(40.7128), Some(-74.0060)),
                "the typed batch location must reach every selected item"
            );
        }
        assert_eq!(app.undo.undo_stack.len(), 3);
    });
}

#[core::prelude::v1::test]
fn ui_automation_untagged_sidebar_row_selection_and_empty_state() {
    let (_temp_dir, engine) = seed_library("untagged_view", 3);
    let items = engine.get_item_slice(0..3);
    assert_eq!(items.len(), 3, "the fixture must create three media items");
    let tagged: Vec<_> = items.iter().take(2).map(|item| item.id).collect();
    engine
        .add_tags_batch(&tagged, &["travel".to_string()])
        .unwrap();
    let untagged_item = items[2].clone();
    let card = leak(format!("media-card-{}", untagged_item.id.0));

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, _| {
        assert_eq!(
            app.untagged_count, 1,
            "two tagged items must leave exactly one untagged item"
        );
        assert_eq!(app.selection, AlbumSelection::AllLibrary);
    });
    assert!(
        vcx.debug_bounds("sidebar-nav-untagged").is_some(),
        "the TAGS section must paint a live Untagged row"
    );

    click(vcx, "sidebar-nav-untagged");
    view.update(vcx, |app, _| {
        assert_eq!(app.selection, AlbumSelection::Untagged);
        assert_eq!(
            app.get_active_count(),
            1,
            "the Untagged view must count only untagged media"
        );
        let slice = app.get_active_slice(0..1);
        assert_eq!(slice.len(), 1);
        assert_eq!(
            slice[0].id, untagged_item.id,
            "items with tag rows must be filtered out of the Untagged view"
        );
    });

    // Clicking the active row toggles back to All; selecting it again restores.
    click(vcx, "sidebar-nav-untagged");
    view.update(vcx, |app, _| {
        assert_eq!(app.selection, AlbumSelection::AllLibrary)
    });
    click(vcx, "sidebar-nav-untagged");
    view.update(vcx, |app, _| {
        assert_eq!(app.selection, AlbumSelection::Untagged)
    });

    // Tag the last untagged item through the real batch action.
    click(vcx, card);
    view.update(vcx, |app, cx| {
        assert_eq!(
            app.selected_item.as_ref().map(|item| item.id),
            Some(untagged_item.id),
            "clicking the only card selects the untagged item"
        );
        app.batch_add_tag("everything".to_string(), cx);
        assert_eq!(
            app.untagged_count, 0,
            "tagging the last untagged item must refresh the cached count"
        );
        assert_eq!(
            app.selection,
            AlbumSelection::Untagged,
            "the view must remain Untagged while it becomes empty"
        );
        assert!(
            app.selected_item.is_none(),
            "an emptied view must drop the stale position-based selection"
        );
        assert!(app.selected_indices.is_empty());
    });

    render_ui(vcx);
    assert!(
        vcx.debug_bounds("empty-state").is_some(),
        "an empty Untagged view must paint its empty state"
    );
    assert!(
        vcx.debug_bounds("empty-primary").is_some(),
        "the empty Untagged view must offer a next action"
    );
    click(vcx, "empty-primary");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::AllLibrary,
            "Show all must leave the empty Untagged view"
        );
        assert_eq!(app.get_active_count(), 3);
    });
}
