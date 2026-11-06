//! Grid density zoom: ⌘-wheel, trackpad ⌘-scroll, pinch and ⌘= / ⌘- all walk
//! the same `SIZE_COLUMNS` ladder and keep the anchored item on screen.

use super::ui_automation::{app_with_library, leak, render_ui, seed_library, wheel};
use crate::app::LuminaApp;
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
    let (dir, engine) = seed_library("gz_ladder", 40);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    grid_at(&view, vcx, 6);
    let pointer = grid_pointer(vcx);

    wheel(
        vcx,
        pointer,
        gpui::ScrollDelta::Lines(gpui::point(0.0, -1.0)),
        gpui::Modifiers::command(),
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
        gpui::Modifiers::command(),
    );
    assert_eq!(columns(&view, vcx), 6, "one notch back in returns to 6");
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn trackpad_cmd_scroll_burst_steps_once_per_threshold() {
    let (dir, engine) = seed_library("gz_burst", 40);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    grid_at(&view, vcx, 4);
    let pointer = grid_pointer(vcx);

    for _ in 0..10 {
        wheel(
            vcx,
            pointer,
            gpui::ScrollDelta::Pixels(gpui::point(gpui::px(0.0), gpui::px(5.0))),
            gpui::Modifiers::command(),
        );
    }
    assert_eq!(
        columns(&view, vcx),
        3,
        "50px of trackpad ⌘-scroll is one 40px step, not one step per event"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn keyboard_grid_zoom_keeps_selection_visible() {
    let (dir, engine) = seed_library("gz_keyboard", 120);
    let mut cx = gpui::TestAppContext::single();
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

    super::ui_automation::keystrokes(vcx, "secondary-= secondary-=");
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
    let _ = std::fs::remove_dir_all(&dir);
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

    let (dir, engine) = seed_library("gz_pinch", 40);
    let mut cx = gpui::TestAppContext::single();
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
    let _ = std::fs::remove_dir_all(&dir);
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

    let (dir, engine) = seed_library("gz_off_ladder", 40);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    grid_at(&view, vcx, 7);
    let pointer = grid_pointer(vcx);
    wheel(
        vcx,
        pointer,
        gpui::ScrollDelta::Pixels(gpui::point(gpui::px(0.0), gpui::px(5.0))),
        gpui::Modifiers::command(),
    );
    assert_eq!(
        columns(&view, vcx),
        7,
        "a ⌘-scroll below the step threshold must not change the density"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
