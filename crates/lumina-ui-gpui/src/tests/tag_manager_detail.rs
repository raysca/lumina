//! Tags page detail panel: the 359.5 px panel, the pencil
//! rename, PREVIEW with View all, ALIASES with "+ Add", ORGANISE and Delete
//! tag with its real count and undo.

use super::tag_manager::{count, open};
use crate::tests::helpers::{
    app_with_library, click, keystrokes, leak, render_ui, seed_library, type_text,
};

#[core::prelude::v1::test]
fn tag_detail_panel_sections() {
    let (_dir, engine) = seed_library("tm_detail_panel", 9);
    let ids: Vec<_> = engine.get_item_slice(0..9).iter().map(|i| i.id).collect();
    engine.add_tags_batch(&ids, &["beach".into()]).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    vcx.simulate_resize(gpui::size(gpui::px(1440.0), gpui::px(900.0)));
    open(&view, vcx);
    click(vcx, "tag-manager-row-beach");
    render_ui(vcx);

    assert!(
        vcx.debug_bounds("tag-manager-detail").is_some(),
        "detail panel"
    );

    // PREVIEW: 2 rows × 4 tiles.
    assert!(vcx.debug_bounds("tag-manager-items").is_some());
    let tiles: Vec<_> = (0..8)
        .map(|i| {
            vcx.debug_bounds(leak(format!("tag-manager-item-{i}")))
                .unwrap_or_else(|| panic!("preview tile {i} must render"))
        })
        .collect();
    for (i, tile) in tiles.iter().enumerate() {
        let (row, col) = (i / 4, i % 4);
        assert_eq!(tile.top(), tiles[row * 4].top(), "tile {i} is in row {row}");
        assert_eq!(
            tile.left(),
            tiles[col].left(),
            "tile {i} is in column {col}"
        );
    }
    assert!(
        tiles[4].top() > tiles[0].top() && tiles[1].left() > tiles[0].left(),
        "the tiles form 2 rows of 4"
    );
    assert!(
        vcx.debug_bounds("tag-manager-item-8").is_none(),
        "the preview shows 8 items, not every item"
    );

    // Delete hint with the real count; ORGANISE keeps Move to….
    assert!(
        vcx.debug_bounds(
            "tag-detail-delete-hint-text:Removes it from 9 items. Your files are untouched."
        )
        .is_some(),
        "the delete hint shows the real item count"
    );
    assert!(vcx.debug_bounds("tag-detail-move").is_some());

    // The pencil turns the name into the rename field; Esc cancels.
    assert!(vcx.debug_bounds("tag-manager-rename-field").is_none());
    click(vcx, "tag-detail-edit");
    render_ui(vcx);
    let name = vcx.debug_bounds("tag-manager-detail-name").unwrap();
    let field = vcx
        .debug_bounds("tag-manager-rename-field")
        .expect("the pencil opens the rename field");
    assert!(
        name.contains(&field.center()),
        "the name itself becomes the field"
    );
    keystrokes(vcx, "x escape");
    view.update(vcx, |app, _| assert!(app.tag_manager.editing.is_none()));
    assert_eq!(count(&view, vcx, "beach"), Some(9), "Esc renames nothing");

    // ↵ renames.
    click(vcx, "tag-detail-edit");
    keystrokes(
        vcx,
        "backspace backspace backspace backspace backspace s a n d enter",
    );
    assert_eq!(count(&view, vcx, "sand"), Some(9), "↵ renames in place");
    view.update(vcx, |app, _| {
        assert!(
            app.tag_manager.search.is_empty(),
            "rename typing must not filter"
        )
    });
    assert_eq!(count(&view, vcx, "beach"), None);
    render_ui(vcx);

    // ALIASES: "+ Add" opens the field; ⌫ on empty removes the last.
    assert!(vcx.debug_bounds("tag-detail-alias-input").is_none());
    click(vcx, "tag-detail-alias-add");
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("tag-detail-alias-input").is_some(),
        "+ Add opens the alias field"
    );
    type_text(vcx, "dune");
    keystrokes(vcx, "enter");
    assert_eq!(
        engine.aliases().unwrap(),
        [("dune".to_string(), "sand".to_string())]
    );
    view.update(vcx, |app, _| {
        assert!(
            app.tag_manager.search.is_empty(),
            "alias typing must not filter"
        )
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("tag-detail-alias-dune").is_some(),
        "alias chip"
    );
    click(vcx, "tag-detail-alias-add");
    keystrokes(vcx, "backspace escape");
    assert!(engine.aliases().unwrap().is_empty(), "⌫ removes the last");

    // Delete tag, then undo.
    click(vcx, "tag-manager-delete-sand");
    assert_eq!(count(&view, vcx, "sand"), None, "Delete tag deletes");
    keystrokes(vcx, "secondary-z");
    assert_eq!(count(&view, vcx, "sand"), Some(9), "and ⌘Z restores it");

    // View all searches the library for the tag.
    click(vcx, "tag-manager-row-sand");
    render_ui(vcx);
    click(vcx, "tag-detail-view-all");
    view.update(vcx, |app, _| {
        assert_eq!(app.search_query, "tag:sand", "View all searches the tag");
        assert!(app.error_message.is_none());
    });
}

#[core::prelude::v1::test]
fn view_all_quotes_a_spaced_tag() {
    let (_dir, engine) = seed_library("tm_view_all_spaced", 3);
    let ids: Vec<_> = engine.get_item_slice(0..3).iter().map(|i| i.id).collect();
    engine
        .add_tags_batch(&ids[..2], &["golden hour".into()])
        .unwrap();
    engine
        .add_tags_batch(&ids[2..], &["golden".into()])
        .unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open(&view, vcx);
    click(vcx, "tag-manager-row-golden hour");
    render_ui(vcx);
    click(vcx, "tag-detail-view-all");
    vcx.run_until_parked();
    let query = view.update(vcx, |app, _| app.search_query.clone());
    assert_eq!(
        query, "tag:\"golden hour\"",
        "a spaced tag is quoted, not searched as tag:golden plus hour"
    );
    assert_eq!(
        engine.search_query_count(&query),
        2,
        "the search shows exactly the items tagged golden hour"
    );
    view.update(vcx, |app, _| {
        let meta = app.page_meta().unwrap_or_default();
        assert!(
            meta.starts_with("2 items"),
            "the view shows 2 items: {meta}"
        )
    });
}

#[core::prelude::v1::test]
fn delete_hint_names_children_again_after_undo() {
    let (_dir, engine) = seed_library("tm_hint_undo", 2);
    let ids: Vec<_> = engine.get_item_slice(0..2).iter().map(|i| i.id).collect();
    engine.add_tags_batch(&ids[..1], &["a".into()]).unwrap();
    engine.add_tags_batch(&ids[1..], &["a/b".into()]).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open(&view, vcx);
    click(vcx, "tag-manager-row-a");
    render_ui(vcx);
    let with_children =
        "tag-detail-delete-hint-text:Removes it and the keywords below it from 2 items. Your files are untouched.";
    assert!(vcx.debug_bounds(with_children).is_some(), "a has a child");

    view.update(vcx, |app, cx| app.bulk_delete_tags(vec!["a/b".into()], cx));
    render_ui(vcx);
    assert!(
        vcx.debug_bounds(
            "tag-detail-delete-hint-text:Removes it from 1 item. Your files are untouched."
        )
        .is_some(),
        "without a/b the hint drops the children"
    );

    keystrokes(vcx, "secondary-z");
    render_ui(vcx);
    assert_eq!(count(&view, vcx, "a/b"), Some(1), "⌘Z restores a/b");
    assert!(
        vcx.debug_bounds(with_children).is_some(),
        "after ⌘Z the hint names the keywords below it again"
    );
}

#[core::prelude::v1::test]
fn preview_asks_for_thumbnails_it_has_not_got() {
    let (_dir, engine) = seed_library("tm_detail_preview_decode", 8);
    let ids: Vec<_> = engine.get_item_slice(0..8).iter().map(|i| i.id).collect();
    engine.add_tags_batch(&ids, &["dusk".into()]).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    vcx.simulate_resize(gpui::size(gpui::px(1440.0), gpui::px(900.0)));
    open(&view, vcx);
    // As if the grid never decoded these items (they were off-screen).
    view.update(vcx, |app, _| {
        let state = app.grid_state.as_ref().unwrap();
        state.texture_cache.lock().clear();
        state.in_flight.lock().clear();
    });
    click(vcx, "tag-manager-row-dusk");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        let state = app.grid_state.as_ref().unwrap();
        let (cache, in_flight) = (state.texture_cache.lock(), state.in_flight.lock());
        for id in &ids {
            assert!(
                in_flight.contains(id) || cache.contains(id),
                "a preview tile without a thumbnail must ask for one ({id:?})"
            );
        }
    });
}
