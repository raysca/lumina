//! Mosaic geometry (plan 01b Task 6): arrow keys and Page keys follow the
//! uneven rows, a marquee selects the reading-order range, the rail and
//! the saved scroll anchor read the Mosaic list, and zoom keeps its anchor
//! item in place.

use std::sync::Arc;

use gpui::{px, size, Entity, Modifiers, MouseButton, VisualTestContext};
use lumina_core::{LuminaEngine, MediaId};

use crate::tests::mosaic::{card, day_app, day_library, mosaic_app};
use crate::tests::ui_automation::{
    bounds_center, click, focus_app, keystrokes, leak, render_ui, seed_library_undecodable, wheel,
};
use crate::views::mosaic::{MosaicLayout, RowKind};
use crate::LuminaApp;

fn secondary(vcx: &mut VisualTestContext) -> Modifiers {
    vcx.update(|_, cx| Modifiers::secondary_key(cx))
}

/// `count` texture-less items with aspects cycling 3:2, 2:3, 1:1, 2:1,
/// 3:4, so rows hold uneven numbers of tiles.
fn uneven_library(prefix: &str, count: usize) -> (std::path::PathBuf, Arc<LuminaEngine>) {
    let (dir, engine) = seed_library_undecodable(prefix, count);
    let sizes = [
        (3000, 2000),
        (2000, 3000),
        (1000, 1000),
        (2000, 1000),
        (1500, 2000),
    ];
    let ids: Vec<MediaId> = engine
        .get_item_slice(0..count)
        .iter()
        .map(|i| i.id)
        .collect();
    let conn = rusqlite::Connection::open(dir.join("catalog.db")).expect("catalog opens");
    for (n, id) in ids.iter().enumerate() {
        let (w, h) = sizes[n % sizes.len()];
        conn.execute(
            "UPDATE media_items SET width = ?1, height = ?2 WHERE id = ?3",
            [w, h, id.0 as i64],
        )
        .unwrap();
    }
    drop(conn);
    (dir, engine)
}

fn tall_mosaic(
    cx: &mut gpui::TestAppContext,
    engine: Arc<LuminaEngine>,
) -> (Entity<LuminaApp>, &mut VisualTestContext) {
    let (view, vcx) = mosaic_app(cx, engine);
    vcx.simulate_resize(size(px(1440.0), px(1000.0)));
    render_ui(vcx);
    render_ui(vcx);
    (view, vcx)
}

fn layout(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> Arc<MosaicLayout> {
    view.update(vcx, |app, _| {
        app.mosaic_layout().expect("the layout landed")
    })
}

fn item_id(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, index: usize) -> MediaId {
    view.update(vcx, |app, _| app.displayed_rows(index..index + 1)[0].id)
}

fn top_ix(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> usize {
    view.update(vcx, |app, _| {
        app.grid_state
            .as_ref()
            .unwrap()
            .mosaic
            .logical_scroll_top()
            .item_ix
    })
}

fn select(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, index: usize) {
    view.update(vcx, |app, cx| app.select_index(index, cx));
    render_ui(vcx);
    focus_app(vcx, view);
}

fn selected(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> Option<usize> {
    view.update(vcx, |app, _| app.selected_index)
}

/// A tile in the second tile row whose ↓ neighbour is not one Grid column
/// step away, so the test tells the two geometries apart.
fn probe(layout: &MosaicLayout, columns: usize) -> usize {
    let tile_rows: Vec<_> = layout
        .rows
        .iter()
        .filter(|r| r.kind == RowKind::Tiles)
        .collect();
    let row = tile_rows[1];
    (row.first..row.first + row.len)
        .find(|&i| layout.vertical_neighbour(i, true) != Some(i + columns))
        .expect("an uneven row")
}

#[core::prelude::v1::test]
fn arrow_keys_use_nearest_x_in_uneven_rows() {
    let (dir, engine) = uneven_library("mosaic_arrows", 60);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = tall_mosaic(&mut cx, engine);
    let layout = layout(&view, vcx);
    let columns = view.update(vcx, |app, _| app.grid_columns);
    let from = probe(&layout, columns);
    let down = layout.vertical_neighbour(from, true).unwrap();

    select(&view, vcx, from);
    keystrokes(vcx, "down");
    render_ui(vcx);
    assert_eq!(selected(&view, vcx), Some(down), "↓ lands on the nearest x");
    let surface = vcx.debug_bounds("media-grid-surface").unwrap();
    let down_id = item_id(&view, vcx, down);
    let b = card(vcx, down_id);
    assert!(
        surface.contains(&b.origin) && surface.contains(&b.bottom_right()),
        "the selected card is on screen: {b:?} in {surface:?}"
    );
    keystrokes(vcx, "up");
    render_ui(vcx);
    assert_eq!(
        selected(&view, vcx),
        layout.vertical_neighbour(down, false),
        "↑ uses the same geometry"
    );

    select(&view, vcx, from);
    keystrokes(vcx, "shift-down");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_index,
            Some(down),
            "⇧↓ extends to the same tile"
        );
        assert_eq!(
            app.selected_indices.iter().copied().collect::<Vec<_>>(),
            (from..=down).collect::<Vec<_>>(),
            "the range runs from the anchor"
        );
    });

    select(&view, vcx, from);
    keystrokes(vcx, "pagedown");
    render_ui(vcx);
    let moved = selected(&view, vcx).unwrap();
    let rows_moved = layout.rows[layout.row_of(from)..layout.row_of(moved)]
        .iter()
        .filter(|r| r.kind == RowKind::Tiles)
        .count();
    assert!(
        rows_moved >= 3,
        "PageDown moves by the visible rows: {rows_moved}"
    );
    keystrokes(vcx, "pageup");
    render_ui(vcx);
    assert!(
        selected(&view, vcx).unwrap() <= from + 8,
        "PageUp moves back up"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn marquee_selects_the_reading_order_range() {
    let (dir, engine) = seed_library_undecodable("mosaic_marquee_drag", 12);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = mosaic_app(&mut cx, engine);
    let layout = layout(&view, vcx);
    assert_ne!(
        layout.row_of(2),
        layout.row_of(7),
        "tiles 2 and 7 are in different rows"
    );
    let (two, seven) = (item_id(&view, vcx, 2), item_id(&view, vcx, 7));
    let start = bounds_center(vcx, leak(format!("media-card-{}", two.0)));
    let end = bounds_center(vcx, leak(format!("media-card-{}", seven.0)));
    vcx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    render_ui(vcx);
    vcx.simulate_mouse_move(end, Some(MouseButton::Left), Modifiers::default());
    render_ui(vcx);
    vcx.simulate_mouse_up(end, MouseButton::Left, Modifiers::default());
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_indices.iter().copied().collect::<Vec<_>>(),
            (2..=7).collect::<Vec<_>>(),
            "a sweep selects the contiguous range, not a grid rectangle"
        )
    });
    let _ = std::fs::remove_dir_all(&dir);
}

/// Months seeded newest first: 2025-06 (20), 2025-05 (8), 2024-12 (8),
/// 2024-03 (24). The first 2024 item is index 28.
fn rail_library(prefix: &str) -> (std::path::PathBuf, Arc<LuminaEngine>) {
    let months = [
        ("2025-06-15", 20usize),
        ("2025-05-15", 8),
        ("2024-12-15", 8),
        ("2024-03-15", 24),
    ];
    let (dir, engine) = seed_library_undecodable(prefix, 60);
    let items = engine.get_item_slice(0..60);
    let mut items = items.iter();
    let mut changes = Vec::new();
    for (date, count) in months {
        let ms =
            chrono::NaiveDateTime::parse_from_str(&format!("{date} 12:00:00"), "%Y-%m-%d %H:%M:%S")
                .unwrap()
                .and_utc()
                .timestamp_millis();
        changes.extend(items.by_ref().take(count).map(|i| (i.id, ms)));
    }
    engine.set_timestamps(&changes).unwrap();
    (dir, engine)
}

#[core::prelude::v1::test]
fn rail_year_click_and_zoom_keep_position_in_mosaic() {
    let (dir, engine) = rail_library("mosaic_rail");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = tall_mosaic(&mut cx, engine);
    assert!(
        vcx.debug_bounds("timeline-year-2024").is_some(),
        "the rail shows"
    );
    click(vcx, "timeline-year-2024");
    render_ui(vcx);
    render_ui(vcx);
    let layout = layout(&view, vcx);
    let top = top_ix(&view, vcx);
    assert_eq!(
        layout.rows[top].first, 28,
        "the 2024 day's first row is at the top: row {top} {:?}",
        layout.rows[top]
    );
    view.update(vcx, |app, _| {
        let visible = app.grid_visible_items();
        assert!(
            visible.contains(&28),
            "the rail's visible items follow the Mosaic: {visible:?}"
        );
        assert!(
            app.timeline_bubble_label()
                .is_some_and(|label| label.contains("2024")),
            "the lens covers the visible months"
        );
    });
    assert!(vcx.debug_bounds("timeline-lens").is_some());

    // ± keeps the selected tile where it is.
    let pick = 40;
    select(&view, vcx, pick);
    render_ui(vcx);
    let id = item_id(&view, vcx, pick);
    let before = card(vcx, id);
    let level = view.update(vcx, |app, _| app.grid_columns);
    click(vcx, "grid-zoom-in");
    render_ui(vcx);
    render_ui(vcx);
    let after = card(vcx, id);
    view.update(vcx, |app, _| {
        assert_ne!(app.grid_columns, level, "the size stepped one level")
    });
    assert!(
        (f32::from(after.origin.y) - f32::from(before.origin.y)).abs() <= 1.0,
        "the selected tile keeps its y: {before:?} -> {after:?}"
    );
    assert!(
        after.size.height > before.size.height,
        "and grows with the new level"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn wheel_zoom_keeps_the_item_under_the_pointer() {
    let (dir, engine) = seed_library_undecodable("mosaic_wheel", 150);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = tall_mosaic(&mut cx, engine);
    view.update(vcx, |app, _| {
        app.grid_state
            .as_ref()
            .unwrap()
            .mosaic
            .scroll_to(gpui::ListOffset {
                item_ix: 8,
                offset_in_item: px(40.0),
            });
    });
    render_ui(vcx);
    render_ui(vcx);
    let row = layout(&view, vcx).row_of(55);
    let index = layout(&view, vcx).rows[row].first + 1;
    let id = item_id(&view, vcx, index);
    let before = card(vcx, id);
    let pointer = gpui::point(
        before.origin.x + before.size.width / 2.0,
        before.origin.y + before.size.height * 0.25,
    );
    let level = view.update(vcx, |app, _| app.grid_columns);
    let mods = secondary(vcx);
    wheel(
        vcx,
        pointer,
        gpui::ScrollDelta::Lines(gpui::point(0.0, -1.0)),
        mods,
    );
    render_ui(vcx);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_ne!(app.grid_columns, level, "⌘-wheel stepped the size")
    });
    let after = card(vcx, id);
    let frac = |b: gpui::Bounds<gpui::Pixels>| (pointer.y - b.origin.y) / b.size.height;
    assert!(
        after.contains(&gpui::point(after.center().x, pointer.y)),
        "the item stays under the pointer: {after:?} at {pointer:?}"
    );
    assert!(
        (frac(after) - frac(before)).abs() < 0.05,
        "at the same point of the tile: {} vs {}",
        frac(before),
        frac(after)
    );

    // At the ladder's end ⌘-wheel changes nothing, and must not scroll.
    view.update(vcx, |app, cx| {
        app.grid_columns = 8;
        cx.notify();
    });
    render_ui(vcx);
    render_ui(vcx);
    let top = view.update(vcx, |app, _| {
        app.grid_state.as_ref().unwrap().mosaic.logical_scroll_top()
    });
    let any = layout(&view, vcx).rows[top.item_ix + 1].first;
    let any_id = item_id(&view, vcx, any);
    let centre = bounds_center(vcx, leak(format!("media-card-{}", any_id.0)));
    let mods = secondary(vcx);
    wheel(
        vcx,
        centre,
        gpui::ScrollDelta::Lines(gpui::point(0.0, -3.0)),
        mods,
    );
    render_ui(vcx);
    let now = view.update(vcx, |app, _| {
        app.grid_state.as_ref().unwrap().mosaic.logical_scroll_top()
    });
    assert_eq!(
        (now.item_ix, now.offset_in_item),
        (top.item_ix, top.offset_in_item),
        "⌘-wheel never scrolls the Mosaic"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn saved_anchor_reads_the_mosaic_and_grid_does_not_jump_back() {
    let (dir, engine) = seed_library_undecodable("mosaic_anchor", 200);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = tall_mosaic(&mut cx, engine);
    view.update(vcx, |app, _| {
        app.grid_state
            .as_ref()
            .unwrap()
            .mosaic
            .scroll_to(gpui::ListOffset {
                item_ix: 12,
                offset_in_item: px(0.0),
            });
    });
    render_ui(vcx);
    render_ui(vcx);
    let first = layout(&view, vcx).rows[12].first;
    let id = item_id(&view, vcx, first);
    view.update(vcx, |app, _| {
        assert_eq!(app.grid_top_row(), 12, "the top row is the list's");
        assert_eq!(
            app.grid_top_item_id(),
            Some(id),
            "the relaunch anchor is the Mosaic's top item"
        );
    });

    // Keyboard steps deep into the Mosaic must not queue a Grid scroll.
    select(&view, vcx, 150);
    keystrokes(vcx, "right");
    render_ui(vcx);
    click(vcx, "view-mode-grid");
    render_ui(vcx);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.grid_top_row(),
            0,
            "Grid opens where it was, not at row 37"
        )
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn select_dims_while_the_layout_is_updating() {
    let (dir, engine) = day_library("mosaic_select_dim");
    let writer = engine.clone();
    let id = engine.get_item_slice(0..1)[0].id;
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = day_app(&mut cx, engine);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.mosaic_day_header(20250614, 3).select_updating,
            None,
            "a current layout's Select is live"
        )
    });
    std::thread::spawn(move || writer.toggle_favorite(id, true).unwrap())
        .join()
        .unwrap();
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.mosaic_day_header(20250614, 3)
                .select_updating
                .as_deref(),
            Some("Updating…"),
            "a stale layout's Select is dimmed and says why"
        )
    });
    assert!(vcx.debug_bounds("mosaic-day-select-20250614").is_some());
    let _ = std::fs::remove_dir_all(&dir);
}
