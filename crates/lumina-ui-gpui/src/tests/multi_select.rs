//! v3 multi-select: selection dock and partial tags in the batch inspector.

use super::ui_automation::{
    app_with_library, bounds_center, click, focus_app, keystrokes, leak, render_ui, seed_library,
};

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

    let mut cx = gpui::TestAppContext::single();
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
    // GPUI 0.2.2 never clears `debug_bounds` between frames, so a chip that
    // has painted once cannot be proven gone by bounds; check the counts the
    // batch panel partitions on instead.
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
    keystrokes(vcx, "cmd-z");
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

    let mut cx = gpui::TestAppContext::single();
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
    keystrokes(vcx, "cmd-z");
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
