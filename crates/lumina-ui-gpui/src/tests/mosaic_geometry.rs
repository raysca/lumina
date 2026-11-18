//! Mosaic geometry: arrow keys and Page keys follow the
//! uneven rows, a marquee selects the reading-order range, the rail and
//! the saved scroll anchor read the Mosaic list, and zoom keeps its anchor
//! item in place.

use std::sync::Arc;

use gpui::{px, size, Entity, Modifiers, MouseButton, VisualTestContext};
use lumina_core::{LuminaEngine, MediaId};

use crate::tests::helpers::{
    bounds_center, click, focus_app, keystrokes, leak, render_ui, seed_library_undecodable, wheel,
};
use crate::tests::mosaic::{card, day_app, day_library, mosaic_app};
use crate::views::mosaic::{MosaicLayout, RowKind};
use crate::LuminaApp;

fn secondary(vcx: &mut VisualTestContext) -> Modifiers {
    vcx.update(|_, cx| Modifiers::secondary_key(cx))
}

/// `count` texture-less items with aspects cycling 3:2, 2:3, 1:1, 2:1,
/// 3:4, so rows hold uneven numbers of tiles.
fn uneven_library(
    prefix: &str,
    count: usize,
) -> (crate::tests::helpers::TempLibrary, Arc<LuminaEngine>) {
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

pub(crate) fn tall_mosaic(
    cx: &mut gpui::TestAppContext,
    engine: Arc<LuminaEngine>,
) -> (Entity<LuminaApp>, &mut VisualTestContext) {
    let (view, vcx) = mosaic_app(cx, engine);
    vcx.simulate_resize(size(px(1440.0), px(1000.0)));
    render_ui(vcx);
    render_ui(vcx);
    (view, vcx)
}

pub(crate) fn layout(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> Arc<MosaicLayout> {
    view.update(vcx, |app, _| {
        app.mosaic_layout().expect("the layout landed")
    })
}

fn item_id(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, index: usize) -> MediaId {
    view.update(vcx, |app, _| app.displayed_rows(index..index + 1)[0].id)
}

pub(crate) fn top_ix(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> usize {
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
    let (_dir, engine) = uneven_library("mosaic_arrows", 60);
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
}

#[core::prelude::v1::test]
fn marquee_selects_the_reading_order_range() {
    let (_dir, engine) = seed_library_undecodable("mosaic_marquee_drag", 12);
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
}

/// Months seeded newest first: 2025-06 (20), 2025-05 (8), 2024-12 (8),
/// 2024-03 (24). The first 2024 item is index 28.
pub(crate) fn rail_library(
    prefix: &str,
) -> (crate::tests::helpers::TempLibrary, Arc<LuminaEngine>) {
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
    let (_dir, engine) = rail_library("mosaic_rail");
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
}

#[core::prelude::v1::test]
fn wheel_zoom_keeps_the_item_under_the_pointer() {
    let (_dir, engine) = seed_library_undecodable("mosaic_wheel", 150);
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
}

#[core::prelude::v1::test]
fn saved_anchor_reads_the_mosaic_and_grid_does_not_jump_back() {
    let (_dir, engine) = seed_library_undecodable("mosaic_anchor", 200);
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
}

#[core::prelude::v1::test]
fn select_dims_while_the_layout_is_updating() {
    let (_dir, engine) = day_library("mosaic_select_dim");
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
}

#[core::prelude::v1::test]
fn arrows_at_the_first_and_last_tile_rows_keep_the_selection() {
    let (_dir, engine) = uneven_library("mosaic_arrow_ends", 30);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = tall_mosaic(&mut cx, engine);
    let layout = layout(&view, vcx);
    select(&view, vcx, 1);
    keystrokes(vcx, "up");
    render_ui(vcx);
    assert_eq!(selected(&view, vcx), Some(1), "↑ on the top tile row stays");
    let last = layout.tiles.len() - 1;
    select(&view, vcx, last);
    keystrokes(vcx, "down");
    render_ui(vcx);
    assert_eq!(
        selected(&view, vcx),
        Some(last),
        "↓ on the bottom tile row stays"
    );
}

#[core::prelude::v1::test]
fn a_zoom_anchor_never_outlives_its_view() {
    let (_dir, engine) = seed_library_undecodable("mosaic_anchor_view", 200);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = tall_mosaic(&mut cx, engine);
    // Zoom, then switch view (the sort) before the zoomed layout lands.
    view.update(vcx, |app, cx| {
        app.select_index(150, cx);
        app.zoom_in_grid(cx);
        app.sort.direction = match app.sort.direction {
            lumina_core::SortDirection::Ascending => lumina_core::SortDirection::Descending,
            lumina_core::SortDirection::Descending => lumina_core::SortDirection::Ascending,
        };
        app.clear_selection(cx);
    });
    render_ui(vcx);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(
            app.mosaic.zoom_anchor.is_none(),
            "the other view's layout drops the anchor"
        )
    });
    // A relayout of the new view at the same level keeps its top row.
    view.update(vcx, |app, _| {
        app.grid_state
            .as_ref()
            .unwrap()
            .mosaic
            .scroll_to(gpui::ListOffset {
                item_ix: 3,
                offset_in_item: px(0.0),
            });
    });
    render_ui(vcx);
    let first = layout(&view, vcx).rows[top_ix(&view, vcx)].first;
    vcx.simulate_resize(size(px(1400.0), px(1000.0)));
    render_ui(vcx);
    render_ui(vcx);
    let layout = layout(&view, vcx);
    let top = top_ix(&view, vcx);
    assert_eq!(
        top,
        layout.row_of(first),
        "the relayout keeps the top item; no stale zoom jump"
    );
}

/// Before and after the view's first layout lands.
#[core::prelude::v1::test]
fn relaunch_in_mosaic_restores_the_scroll_anchor() {
    for before_layout in [true, false] {
        relaunch_in_mosaic(before_layout, 95);
    }
}

/// A saved selection off-screen from the anchor is revealed, as in Grid.
#[core::prelude::v1::test]
fn relaunch_in_mosaic_reveals_an_off_screen_selection() {
    for before_layout in [true, false] {
        relaunch_in_mosaic(before_layout, 190);
    }
}

fn relaunch_in_mosaic(before_layout: bool, selected: usize) {
    use crate::app::types::UiMessage;
    use crate::config::{LastView, RestoredSession};
    let (_dir, engine) = seed_library_undecodable("mosaic_relaunch", 200);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = tall_mosaic(&mut cx, engine);
    let anchor = view.update(vcx, |app, cx| {
        if before_layout {
            // A new view key: the landed layout is not this view's.
            app.sort.direction = match app.sort.direction {
                lumina_core::SortDirection::Ascending => lumina_core::SortDirection::Descending,
                lumina_core::SortDirection::Descending => lumina_core::SortDirection::Ascending,
            };
        }
        assert_eq!(app.mosaic_layout().is_none(), before_layout);
        // Ids at those positions in the view's order.
        let ids: Vec<MediaId> = app.get_active_slice(0..200).iter().map(|i| i.id).collect();
        app.pending_restore = Some(RestoredSession {
            view: LastView::All,
            search_query: String::new(),
            active_year: None,
            selected_media_id: Some(ids[selected].0),
            scroll_anchor_id: Some(ids[90].0),
        });
        let total = app.grid_state.as_ref().map_or(0, |s| s.total_count);
        app.handle_ui_message(
            UiMessage::Engine(lumina_core::EngineEvent::ScanComplete { total_items: total }),
            cx,
        );
        assert_eq!(app.find_active_index(ids[90]), Some(90));
        (90, selected)
    });
    let (anchor, sel) = anchor;
    render_ui(vcx);
    render_ui(vcx);
    let layout = layout(&view, vcx);
    let top = top_ix(&view, vcx);
    let visible = view.update(vcx, |app, _| {
        assert_eq!(app.selected_index, Some(sel), "the selection is restored");
        app.mosaic_visible_items()
    });
    assert!(
        visible.contains(&sel),
        "the restored selection {sel} is on screen ({visible:?}, before layout: {before_layout})"
    );
    if visible.contains(&anchor) {
        assert_eq!(
            top,
            crate::app::mosaic_nav::row_for_item(&layout, anchor),
            "Mosaic opens with the saved anchor's row on top (before layout: {before_layout})"
        );
    } else {
        assert!(
            selected == 190,
            "the anchor yields only to an off-screen selection"
        );
    }
    assert!(top > 0, "the anchor is below the first screen");
    click(vcx, "view-mode-grid");
    render_ui(vcx);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.grid_top_row(),
            0,
            "no restore was queued on the hidden Grid"
        )
    });
}

#[core::prelude::v1::test]
fn offset_past_the_end_stays_inside_the_last_row() {
    let (_dir, engine) = uneven_library("mosaic_offset_end", 30);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = tall_mosaic(&mut cx, engine);
    let layout = layout(&view, vcx);
    let offset = crate::app::mosaic_nav::offset_for_y(&layout, 1.0e9);
    let last = layout.rows.len() - 1;
    assert_eq!(offset.item_ix, last);
    assert!(
        f32::from(offset.offset_in_item) <= layout.rows[last].height,
        "the offset is clamped to the row height"
    );
}

#[core::prelude::v1::test]
fn a_relaunch_scroll_for_another_view_is_dropped() {
    let (_dir, engine) = seed_library_undecodable("mosaic_stale_relaunch", 200);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = tall_mosaic(&mut cx, engine);
    let flip = |app: &mut LuminaApp| {
        app.sort.direction = match app.sort.direction {
            lumina_core::SortDirection::Ascending => lumina_core::SortDirection::Descending,
            lumina_core::SortDirection::Descending => lumina_core::SortDirection::Ascending,
        };
    };
    view.update(vcx, |app, _| {
        flip(app);
        let other = app.view_key();
        flip(app);
        app.mosaic.pending_scroll = Some(crate::app::mosaic_layout::PendingScroll {
            key: other,
            item: 150,
            reveal: None,
        });
    });
    // A layout for the current view lands (a resize relayouts it).
    vcx.simulate_resize(size(px(1400.0), px(1000.0)));
    render_ui(vcx);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(
            app.mosaic.pending_scroll.is_none(),
            "a layout for another view drops the relaunch scroll"
        )
    });
    // Switching to that view later starts at the top: no stale jump.
    view.update(vcx, |app, cx| {
        flip(app);
        cx.notify();
    });
    render_ui(vcx);
    render_ui(vcx);
    assert_eq!(top_ix(&view, vcx), 0, "the new view starts at the top");
}
