//! Drag selection in the grid: ranges, rectangles, Cmd and Escape.

use crate::tests::helpers::*;
use crate::*;

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
