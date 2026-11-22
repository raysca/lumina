//! Grid density zoom: ⌘-wheel, trackpad ⌘-scroll, pinch and ⌘= / ⌘- all walk
//! the same `SIZE_COLUMNS` ladder and keep the anchored item on screen.

use super::helpers::{app_with_library, leak, render_ui, seed_library, wheel};
use crate::app::LuminaApp;
use crate::tests::helpers::*;
use crate::*;
use gpui::{Entity, VisualTestContext};

fn scroll_y(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> f32 {
    view.update(vcx, |app, _| {
        let handle = app.grid_state.as_ref().unwrap().scroll_handle.clone();
        let y = f32::from(handle.0.borrow().base_handle.offset().y);
        y
    })
}

fn columns(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> usize {
    view.update(vcx, |app, _| app.grid_columns)
}

fn grid_at(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, cols: usize) {
    view.update(vcx, |app, cx| {
        app.grid_columns = cols;
        cx.notify();
    });
    render_ui(vcx);
}

fn grid_pointer(vcx: &mut VisualTestContext) -> gpui::Point<gpui::Pixels> {
    vcx.debug_bounds("media-grid-surface")
        .expect("grid surface must be painted")
        .center()
}

#[core::prelude::v1::test]
fn cmd_wheel_walks_size_ladder() {
    let (_dir, engine) = seed_library("gz_ladder", 40);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    grid_at(&view, vcx, 6);
    let pointer = grid_pointer(vcx);

    wheel(
        vcx,
        pointer,
        gpui::ScrollDelta::Lines(gpui::point(0.0, -1.0)),
        crate::tests::helpers::primary_modifier(),
    );
    assert_eq!(
        columns(&view, vcx),
        8,
        "one ⌘-wheel notch out from 6 columns walks the ladder to 8, never 7"
    );

    wheel(
        vcx,
        pointer,
        gpui::ScrollDelta::Lines(gpui::point(0.0, 1.0)),
        crate::tests::helpers::primary_modifier(),
    );
    assert_eq!(columns(&view, vcx), 6, "one notch back in returns to 6");
}

#[core::prelude::v1::test]
fn trackpad_cmd_scroll_burst_steps_once_per_threshold() {
    let (_dir, engine) = seed_library("gz_burst", 40);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    grid_at(&view, vcx, 4);
    let pointer = grid_pointer(vcx);

    for _ in 0..10 {
        wheel(
            vcx,
            pointer,
            gpui::ScrollDelta::Pixels(gpui::point(gpui::px(0.0), gpui::px(5.0))),
            crate::tests::helpers::primary_modifier(),
        );
    }
    assert_eq!(
        columns(&view, vcx),
        3,
        "50px of trackpad ⌘-scroll is one 40px step, not one step per event"
    );
}

#[core::prelude::v1::test]
fn keyboard_grid_zoom_keeps_selection_visible() {
    let (_dir, engine) = seed_library("gz_keyboard", 120);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    grid_at(&view, vcx, 8);
    let id = view.update(vcx, |app, cx| {
        app.select_index(40, cx);
        app.selected_item.as_ref().map(|item| item.id).unwrap()
    });
    render_ui(vcx);
    render_ui(vcx);
    let card = leak(format!("media-card-{}", id.0));
    assert!(
        vcx.debug_bounds(card).is_some(),
        "the selected card is on screen before zooming"
    );

    super::helpers::keystrokes(vcx, "secondary-= secondary-=");
    render_ui(vcx);
    assert_eq!(columns(&view, vcx), 4, "two ⌘= steps go 8 → 6 → 4");
    let viewport = view.update(vcx, |app, _| {
        let handle = app.grid_state.as_ref().unwrap().scroll_handle.clone();
        let bounds = handle.0.borrow().base_handle.bounds();
        bounds
    });
    let bounds = vcx
        .debug_bounds(card)
        .expect("the selected card must still be rendered after ⌘=");
    assert!(
        bounds.intersects(&viewport),
        "the selected card must stay inside the grid viewport: card={bounds:?} viewport={viewport:?}"
    );
}

fn grid_pinch(vcx: &mut VisualTestContext, position: gpui::Point<gpui::Pixels>, delta: f32) {
    vcx.simulate_mouse_move(position, None, gpui::Modifiers::default());
    vcx.simulate_event(gpui::PinchEvent {
        position,
        delta,
        modifiers: gpui::Modifiers::default(),
        phase: gpui::TouchPhase::Moved,
    });
    render_ui(vcx);
}

#[core::prelude::v1::test]
fn grid_pinch_steps_one_level_per_threshold_and_anchors() {
    use crate::views::grid::{GridZoomAnchor, GridZoomGeometry};

    let (_dir, engine) = seed_library("gz_pinch", 40);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    grid_at(&view, vcx, 4);
    let pointer = grid_pointer(vcx);
    // Scroll away from the top so the anchor has to move the offset.
    wheel(
        vcx,
        pointer,
        gpui::ScrollDelta::Pixels(gpui::point(gpui::px(0.0), gpui::px(-300.0))),
        gpui::Modifiers::default(),
    );
    let before = scroll_y(&view, vcx);
    assert!(before < 0.0, "the setup scroll must move the grid");

    let anchor = view.update(vcx, |app, _| {
        let handle = app.grid_state.as_ref().unwrap().scroll_handle.clone();
        let state = handle.0.borrow();
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
        GridZoomAnchor::at_pointer(f32::from(pointer.x), f32::from(pointer.y), geometry)
            .expect("the pointer is inside the grid viewport")
    });

    // 0.05 + 0.05 stays under the 0.12 step: no density change, no scroll.
    grid_pinch(vcx, pointer, 0.05);
    grid_pinch(vcx, pointer, 0.05);
    assert_eq!(columns(&view, vcx), 4, "a pinch below the threshold waits");
    assert!(
        (scroll_y(&view, vcx) - before).abs() < 0.5,
        "a pinch must never scroll the grid"
    );

    // The third event crosses 0.12: exactly one level, larger cards.
    grid_pinch(vcx, pointer, 0.05);
    assert_eq!(
        columns(&view, vcx),
        3,
        "crossing the threshold steps one level"
    );
    let (row_under_pointer, cols) = view.update(vcx, |app, _| {
        let handle = app.grid_state.as_ref().unwrap().scroll_handle.clone();
        let state = handle.0.borrow();
        let top = f32::from(state.base_handle.bounds().origin.y);
        let y = f32::from(state.base_handle.offset().y);
        let content_y = f32::from(pointer.y) - top - y;
        (
            (content_y / app.row_height()).floor() as usize,
            app.grid_columns,
        )
    });
    assert_eq!(
        row_under_pointer,
        anchor.item / cols,
        "the item under the pinch keeps its new row under the pinch centre"
    );

    // Pinching in again (0.03 left over + 0.05) stays below the next step.
    grid_pinch(vcx, pointer, 0.05);
    assert_eq!(
        columns(&view, vcx),
        3,
        "the remainder carries, one step only"
    );
}

#[core::prelude::v1::test]
fn saved_off_ladder_columns_snap_and_small_scrolls_keep_them() {
    use crate::views::search_bar::ladder_columns;
    assert_eq!(
        ladder_columns(7),
        6,
        "the old wheel's 7 snaps onto the ladder"
    );
    assert_eq!(
        ladder_columns(5),
        4,
        "the old wheel's 5 snaps onto the ladder"
    );
    assert_eq!(ladder_columns(3), 3, "a ladder value is kept");

    let (_dir, engine) = seed_library("gz_off_ladder", 40);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    grid_at(&view, vcx, 7);
    let pointer = grid_pointer(vcx);
    wheel(
        vcx,
        pointer,
        gpui::ScrollDelta::Pixels(gpui::point(gpui::px(0.0), gpui::px(5.0))),
        crate::tests::helpers::primary_modifier(),
    );
    assert_eq!(
        columns(&view, vcx),
        7,
        "a ⌘-scroll below the step threshold must not change the density"
    );
}

#[core::prelude::v1::test]
fn zoom_pill_buttons_walk_the_size_ladder() {
    use super::helpers::click;
    use crate::views::sidebar::AlbumSelection;

    let (_dir, engine) = seed_library("gz_pill", 6);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    grid_at(&view, vcx, 4);

    let pill = vcx
        .debug_bounds("grid-zoom-pill")
        .expect("the pill shows on the grid");
    let status = vcx.debug_bounds("status-bar").expect("status bar");
    // The content column is the grid surface up to the timeline rail; the
    // rail and the inspector are its siblings.
    let assert_centred = |vcx: &mut VisualTestContext, when: &str| {
        let pill = vcx.debug_bounds("grid-zoom-pill").expect("pill");
        let surface = vcx
            .debug_bounds("media-grid-surface")
            .expect("grid surface");
        let rail = vcx
            .debug_bounds("timeline-rail")
            .expect("the seeded library shows the timeline rail");
        let centre = (f32::from(surface.left()) + f32::from(rail.left())) / 2.0;
        assert!(
            (f32::from(pill.center().x) - centre).abs() < 1.0,
            "{when}: the pill centres in the grid area, excluding the rail: \
             pill={pill:?} grid={surface:?} rail={rail:?}"
        );
        surface
    };
    let wide = assert_centred(vcx, "inspector closed");
    view.update(vcx, |app, cx| {
        app.inspector_open = true;
        cx.notify();
    });
    render_ui(vcx);
    let narrow = assert_centred(vcx, "inspector open");
    assert!(
        narrow.right() < wide.right(),
        "the inspector takes width from the grid column: {narrow:?} vs {wide:?}"
    );
    view.update(vcx, |app, cx| {
        app.inspector_open = false;
        cx.notify();
    });
    render_ui(vcx);
    assert!(
        (f32::from(status.top()) - f32::from(pill.bottom()) - 24.0).abs() < 1.0,
        "the pill sits 24 above the status bar: pill={pill:?} status={status:?}"
    );

    click(vcx, "grid-zoom-in");
    assert_eq!(columns(&view, vcx), 3, "grid-zoom-in walks 4 → 3");
    click(vcx, "grid-zoom-out");
    assert_eq!(columns(&view, vcx), 4, "grid-zoom-out walks 3 → 4");

    // A click on the pill's padding is absorbed: it must not reach the grid
    // and clear the selection.
    view.update(vcx, |app, cx| app.select_index(0, cx));
    render_ui(vcx);
    let pill = vcx.debug_bounds("grid-zoom-pill").unwrap();
    let dock = vcx
        .debug_bounds("multi-select-action-dock")
        .expect("the selection dock shows");
    assert!(
        (f32::from(dock.size.height) - crate::app::render::overlays::DOCK_HEIGHT).abs() <= 0.5,
        "DOCK_HEIGHT matches the drawn dock, which the pill and toast stack on: dock={dock:?}"
    );
    assert!(
        pill.bottom() <= dock.top(),
        "the pill stacks above the selection dock: pill={pill:?} dock={dock:?}"
    );
    vcx.simulate_click(
        gpui::point(pill.left() + gpui::px(2.0), pill.center().y),
        gpui::Modifiers::default(),
    );
    render_ui(vcx);
    assert!(
        view.update(vcx, |app, _| app.selected_item.is_some()),
        "a click on the pill must not fall through to the grid"
    );

    // A toast stacks above the pill and the dock.
    view.update(vcx, |app, cx| app.show_toast("Saved", cx));
    render_ui(vcx);
    let toast = vcx.debug_bounds("toast").expect("toast");
    assert!(
        toast.bottom() <= pill.top(),
        "the toast stacks above the pill: toast={toast:?} pill={pill:?}"
    );
    view.update(vcx, |app, cx| {
        app.clear_selection(cx);
        app.dismiss_toast(cx);
    });
    render_ui(vcx);

    // At 2 columns, zoom in is disabled: no change. The button's aria
    // description can't be read here (GPUI's test window builds no
    // AccessKit tree), so the state is read through `grid_zoom_enabled`.
    grid_at(&view, vcx, 2);
    assert!(
        view.update(vcx, |app, _| !app.grid_zoom_enabled(1)),
        "zoom in is disabled at the largest size"
    );
    click(vcx, "grid-zoom-in");
    assert_eq!(columns(&view, vcx), 2, "a disabled zoom in changes nothing");
    grid_at(&view, vcx, 8);
    assert!(
        view.update(vcx, |app, _| !app.grid_zoom_enabled(-1)),
        "zoom out is disabled at the smallest size"
    );
    click(vcx, "grid-zoom-out");
    assert_eq!(
        columns(&view, vcx),
        8,
        "a disabled zoom out changes nothing"
    );

    // Hidden on the map, on Duplicates and in an empty state.
    view.update(vcx, |app, cx| app.set_search_query("zzz-no-match", cx));
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("empty-state").is_some(),
        "empty state shows"
    );
    assert!(
        vcx.debug_bounds("grid-zoom-pill").is_none(),
        "no pill over an empty state"
    );
    view.update(vcx, |app, cx| {
        app.set_search_query("", cx);
        app.select_album(AlbumSelection::Duplicates, cx);
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("grid-zoom-pill").is_none(),
        "no pill on Duplicates"
    );
    view.update(vcx, |app, cx| app.show_map(cx));
    render_ui(vcx);
    assert!(view.update(vcx, |app, _| app.map_shown()), "the map shows");
    assert!(
        vcx.debug_bounds("grid-zoom-pill").is_none(),
        "no pill on the map"
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
fn a_zoom_dips_the_new_grid_in() {
    let (_dir, engine) = seed_library("gz_dip", 40);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    grid_at(&view, vcx, 6);
    let before = view.update(vcx, |app, _| app.grid_dip);
    let pointer = grid_pointer(vcx);
    wheel(
        vcx,
        pointer,
        gpui::ScrollDelta::Lines(gpui::point(0.0, -1.0)),
        crate::tests::helpers::primary_modifier(),
    );
    assert_eq!(columns(&view, vcx), 8);
    assert_eq!(
        view.update(vcx, |app, _| app.grid_dip),
        before + 1,
        "a column change dips the grid in"
    );
    assert!(
        vcx.debug_bounds("media-grid").is_some(),
        "the dipping grid lays out"
    );
}

#[core::prelude::v1::test]
fn a_short_rail_jump_scrolls_there_and_a_long_one_dips_in() {
    crate::motion::REDUCE_MOTION.with(|on| on.set(false));
    let (_dir, engine) = seed_library("gz_smooth", 400);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    grid_at(&view, vcx, 6);
    let row = view.update(vcx, |app, _| app.row_height());

    view.update(vcx, |app, cx| app.scroll_grid_to_offset(6 * 2, cx));
    let start = scroll_y(&view, vcx);
    assert!(
        start.abs() < 0.5,
        "a short jump starts where the grid was: {start}"
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while (scroll_y(&view, vcx) + 2.0 * row).abs() > 0.5 && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(10));
        vcx.executor()
            .advance_clock(std::time::Duration::from_millis(10));
        vcx.run_until_parked();
    }
    assert!(
        (scroll_y(&view, vcx) + 2.0 * row).abs() < 0.5,
        "and lands two rows down: {}",
        scroll_y(&view, vcx)
    );

    let dip = view.update(vcx, |app, _| app.grid_dip);
    view.update(vcx, |app, cx| app.scroll_grid_to_offset(6 * 60, cx));
    assert!(
        scroll_y(&view, vcx) < -10.0 * row,
        "a long jump lands at once: {}",
        scroll_y(&view, vcx)
    );
    assert_eq!(
        view.update(vcx, |app, _| app.grid_dip),
        dip + 1,
        "and dips in instead"
    );
}
