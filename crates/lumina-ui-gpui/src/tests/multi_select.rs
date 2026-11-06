//! v3 multi-select: selection dock and partial tags in the batch inspector.

use super::ui_automation::{
    app_with_library, bounds_center, click, focus_app, keystrokes, leak, render_ui, seed_library,
};
use crate::views::inspector::batch::common_rating;

/// Click the first item, ⌘-click the second, dock the inspector at 1440×900.
fn select_two<'a>(
    prefix: &str,
    cx: &'a mut gpui::TestAppContext,
) -> (
    std::path::PathBuf,
    std::sync::Arc<lumina_core::LuminaEngine>,
    [lumina_core::MediaItem; 2],
    gpui::Entity<crate::LuminaApp>,
    &'a mut gpui::VisualTestContext,
) {
    let (dir, engine) = seed_library(prefix, 3);
    let items = engine.get_item_slice(0..2);
    let (a, b) = (items[0].clone(), items[1].clone());
    let (view, vcx) = app_with_library(cx, engine.clone());
    vcx.simulate_resize(gpui::size(gpui::px(1440.0), gpui::px(900.0)));
    render_ui(vcx);
    click(vcx, leak(format!("media-card-{}", a.id.0)));
    let second = bounds_center(vcx, leak(format!("media-card-{}", b.id.0)));
    vcx.simulate_click(second, gpui::Modifiers::command());
    click(vcx, "inspector-toggle");
    render_ui(vcx);
    (dir, engine, [a, b], view, vcx)
}

#[core::prelude::v1::test]
fn batch_inspector_marks_partial_tags_and_applies_them_to_all() {
    let (temp_dir, engine) = seed_library("multi_partial", 3);
    let items = engine.get_item_slice(0..3);
    let (a, b) = (items[0].clone(), items[1].clone());
    let catalog = engine.catalog();
    catalog
        .add_tags_batch(&[a.id, b.id], &["trip".to_string()])
        .unwrap();
    catalog
        .add_tags_batch(&[a.id], &["street".to_string()])
        .unwrap();

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    vcx.simulate_resize(gpui::size(gpui::px(1440.0), gpui::px(900.0)));
    render_ui(vcx);

    // Select both through the real input path: click, then ⌘-click.
    click(vcx, leak(format!("media-card-{}", a.id.0)));
    let second = bounds_center(vcx, leak(format!("media-card-{}", b.id.0)));
    vcx.simulate_click(second, gpui::Modifiers::command());
    view.update(vcx, |app, cx| {
        assert_eq!(
            app.selected_indices.len(),
            2,
            "⌘-click must extend the selection"
        );
        app.inspector_open = true;
        cx.notify();
    });
    render_ui(vcx);

    assert!(
        vcx.debug_bounds("batch-dock-count").is_some(),
        "a multi-selection must show the dock with its count"
    );
    assert!(
        vcx.debug_bounds("batch-partial-tag-street").is_some(),
        "a tag on one of two items must show as partial"
    );
    assert!(
        vcx.debug_bounds("batch-partial-tag-trip").is_none(),
        "a tag on every selected item must not show as partial"
    );

    let chip = vcx.debug_bounds("batch-partial-tag-street").unwrap();
    assert!(
        chip.bottom() <= gpui::px(900.0),
        "partial tags must be visible without scrolling, got {chip:?}"
    );
    click(vcx, "batch-partial-tag-street");
    assert!(
        catalog
            .get_tags_for_item(b.id)
            .unwrap()
            .contains(&"street".to_string()),
        "clicking a partial tag must add it to every selected item"
    );
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("batch-partial-tag-street").is_none(),
        "a tag on every selected item is no longer shown as partial"
    );
    let street_count = |vcx: &mut gpui::VisualTestContext| {
        view.update(vcx, |app, _| {
            app.selected_tag_counts
                .iter()
                .find(|(tag, _)| tag == "street")
                .map(|(_, n)| *n)
        })
    };
    assert_eq!(
        street_count(vcx),
        Some(2),
        "once applied the tag is on both items, so it is no longer partial"
    );

    // Undo must refresh the batch counts, not just the focused item's tags.
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-z");
    assert_eq!(
        street_count(vcx),
        Some(1),
        "undo must bring back the partial count for the batch"
    );
    assert!(
        catalog
            .get_tags_for_item(a.id)
            .unwrap()
            .contains(&"street".to_string()),
        "undo must not strip the tag from the item that already had it"
    );
    assert!(
        !catalog
            .get_tags_for_item(b.id)
            .unwrap()
            .contains(&"street".to_string()),
        "undo must remove the tag from the item it was added to"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn partial_tag_remove_clears_all_and_undo_restores_only_its_owner() {
    let (temp_dir, engine) = seed_library("multi_partial_remove", 3);
    let items = engine.get_item_slice(0..3);
    let (a, b) = (items[0].clone(), items[1].clone());
    let catalog = engine.catalog();
    catalog
        .add_tags_batch(&[a.id], &["street".to_string()])
        .unwrap();

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    vcx.simulate_resize(gpui::size(gpui::px(1440.0), gpui::px(900.0)));
    render_ui(vcx);

    click(vcx, leak(format!("media-card-{}", a.id.0)));
    let second = bounds_center(vcx, leak(format!("media-card-{}", b.id.0)));
    vcx.simulate_click(second, gpui::Modifiers::command());
    view.update(vcx, |app, cx| {
        app.inspector_open = true;
        cx.notify();
    });
    render_ui(vcx);

    click(vcx, "batch-partial-tag-remove-street");
    let has = |id| {
        catalog
            .get_tags_for_item(id)
            .unwrap()
            .contains(&"street".to_string())
    };
    assert!(
        !has(a.id) && !has(b.id),
        "the partial chip's × must remove the tag from every selected item"
    );

    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-z");
    assert!(
        has(a.id),
        "undo must restore the tag on the item that had it"
    );
    assert!(
        !has(b.id),
        "undo must not add the tag to an item that never had it"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn batch_panel_shows_header_shared_rating_and_one_tag_list() {
    let mut cx = crate::tests::TestApp::single();
    let (dir, engine, [a, b], view, vcx) = select_two("batch_v3_panel", &mut cx);

    for selector in [
        "batch-stack",
        "batch-count",
        "batch-mix",
        "batch-size-range",
    ] {
        assert!(
            vcx.debug_bounds(selector).is_some(),
            "the batch header shows {selector}"
        );
    }
    assert!(
        vcx.debug_bounds("close-batch-inspector-btn").is_none(),
        "v3 batch panel has no header strip"
    );
    let panel = vcx.debug_bounds("inspector-batch").expect("batch panel");
    let hint = vcx.debug_bounds("inspector-batch-hint").unwrap();
    assert_eq!(
        hint.bottom(),
        panel.bottom(),
        "the selection hint is pinned to the panel bottom"
    );
    view.update(vcx, |app, _| {
        assert_eq!(app.batch_summary.ids, vec![a.id, b.id]);
        assert_eq!(common_rating(app), Some(0), "both start unrated");
    });

    // Stars set every item; the shared value reads back.
    click(vcx, "inspector-star-4");
    let catalog = engine.catalog();
    for id in [a.id, b.id] {
        assert_eq!(catalog.get_rating(id).unwrap(), 4, "a star click sets all");
    }
    view.update(vcx, |app, _| {
        assert_eq!(common_rating(app), Some(4));
        // One item diverging makes the rating mixed.
        app.item_ratings.insert(a.id, 2);
        assert_eq!(common_rating(app), None, "disagreeing ratings read mixed");
    });

    // "Add a tag to all N…" then the solid chip's × removes it from all.
    click(vcx, "batch-tag-add");
    keystrokes(vcx, "s e a enter");
    for id in [a.id, b.id] {
        assert_eq!(
            catalog.get_tags_for_item(id).unwrap(),
            vec!["sea".to_string()]
        );
    }
    render_ui(vcx);
    click(vcx, "batch-tag-remove-sea");
    for id in [a.id, b.id] {
        assert!(
            catalog.get_tags_for_item(id).unwrap().is_empty(),
            "× on an all-items chip removes it from every item"
        );
    }
    view.update(vcx, |app, _| assert!(app.error_message.is_none()));

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn dock_shows_for_one_item_over_the_grid_column_above_the_status_bar() {
    let (dir, engine) = seed_library("batch_v3_dock", 3);
    let item = engine.get_item_slice(0..1)[0].clone();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    vcx.simulate_resize(gpui::size(gpui::px(1440.0), gpui::px(900.0)));
    render_ui(vcx);
    click(vcx, leak(format!("media-card-{}", item.id.0)));
    click(vcx, "inspector-toggle");
    render_ui(vcx);

    let dock = vcx
        .debug_bounds("multi-select-action-dock")
        .expect("one selected item shows the dock");
    let grid = vcx.debug_bounds("media-grid-surface").unwrap();
    let status = vcx.debug_bounds("status-bar").unwrap();
    let inspector = vcx.debug_bounds("inspector-footer").unwrap();
    let gap = f32::from(status.top() - dock.bottom());
    assert!(
        (gap - 20.0).abs() < 1.0,
        "dock sits 20px above the status bar, got {gap}"
    );
    assert!(
        dock.left() > grid.left() && dock.right() < inspector.left(),
        "dock stays within the grid column: {dock:?} grid {grid:?}"
    );
    let off_centre = f32::from(dock.center().x - grid.center().x).abs();
    assert!(
        off_centre < 2.0,
        "dock is centred over the grid column, off by {off_centre}"
    );
    assert!(
        vcx.debug_bounds("batch-tags-dock-key").is_some(),
        "Tag shows its T key"
    );

    // Favorite with one item writes that item's flag.
    click(vcx, "batch-fav-dock-btn");
    assert!(
        engine.get_item_by_id(item.id).unwrap().is_favorite,
        "the dock's Favorite favorites a single selected item"
    );

    // T is the keyboard path for the dock's Tag.
    focus_app(vcx, &view);
    keystrokes(vcx, "t");
    view.update(vcx, |app, _| {
        assert!(app.tag_input_active, "T starts tagging the selection")
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn a_batch_rating_or_label_is_one_undo_step() {
    let mut cx = crate::tests::TestApp::single();
    let (temp_dir, engine, [a, b], view, vcx) = select_two("multi_undo_step", &mut cx);
    let catalog = engine.catalog();

    click(vcx, "inspector-star-4");
    for id in [a.id, b.id] {
        assert_eq!(catalog.get_rating(id).unwrap(), 4, "a star click rates all");
    }
    view.update(vcx, |app, cx| app.set_color_label_selected(3, cx));
    let labels = catalog.get_color_labels_map().unwrap();
    assert_eq!((labels.get(&a.id), labels.get(&b.id)), (Some(&3), Some(&3)));
    view.update(vcx, |app, _| {
        assert_eq!(
            app.undo.undo_stack.len(),
            2,
            "rating and labelling a selection is one undo entry each"
        )
    });

    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-z");
    let labels = catalog.get_color_labels_map().unwrap();
    assert!(
        !labels.contains_key(&a.id) && !labels.contains_key(&b.id),
        "one ⌘Z restores the label on every item"
    );
    keystrokes(vcx, "secondary-z");
    for id in [a.id, b.id] {
        assert_eq!(
            catalog.get_rating(id).unwrap(),
            0,
            "one ⌘Z restores the rating on every item"
        );
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn a_failed_batch_rating_changes_nothing_and_says_so() {
    let mut cx = crate::tests::TestApp::single();
    let (temp_dir, engine, [a, b], view, vcx) = select_two("multi_failed_rating", &mut cx);
    rusqlite::Connection::open(temp_dir.join("catalog.db"))
        .unwrap()
        .execute_batch(&format!(
            "CREATE TRIGGER fail_second BEFORE UPDATE ON media_items WHEN new.id = {}
             BEGIN SELECT RAISE(ABORT, 'injected'); END;",
            b.id.0
        ))
        .unwrap();

    click(vcx, "inspector-star-4");
    assert_eq!(
        engine.catalog().get_rating(a.id).unwrap(),
        0,
        "the batch rolls back the item before the failure"
    );
    view.update(vcx, |app, _| {
        assert!(
            app.item_ratings.is_empty(),
            "the rating map never leads the catalog"
        );
        assert!(
            app.undo.undo_stack.is_empty(),
            "a failed edit is not undoable"
        );
        assert!(
            app.error_message
                .as_deref()
                .is_some_and(|m| m.starts_with("Could not save rating")),
            "the failure reaches the banner: {:?}",
            app.error_message
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}
