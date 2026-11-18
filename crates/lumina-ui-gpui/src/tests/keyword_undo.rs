//! Undo for global keyword rename, merge and delete:
//! exact rows, aliases, smart-album rules, sidecars and the view, by ⌘Z
//! from the grid and from inside the tag manager.

use crate::tests::helpers::{
    app_with_library, click, keystrokes, render_ui, seed_library, type_text,
};
use crate::tests::keywords::{open_manager, sorted_tags, tag};
use crate::views::sidebar::AlbumSelection;
use crate::LuminaApp;
use gpui::{Entity, VisualTestContext};
use lumina_core::{LuminaEngine, MediaId};

fn tag_rule(value: &str) -> lumina_core::SmartAlbumRule {
    lumina_core::SmartAlbumRule {
        field: lumina_core::RuleField::Tag,
        operator: lumina_core::RuleOperator::Equals,
        value: lumina_core::RuleValue::Text(value.into()),
    }
}

fn album_rule(engine: &LuminaEngine, id: i64) -> lumina_core::RuleValue {
    engine
        .list_smart_albums()
        .into_iter()
        .find(|a| a.id == id)
        .expect("the album exists")
        .rules[0]
        .value
        .clone()
}

/// Clears the Tags page's checks (Esc) and presses ⌘Z.
fn close_and_undo(vcx: &mut VisualTestContext) {
    keystrokes(vcx, "escape");
    keystrokes(vcx, "secondary-z");
}

fn toast(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> String {
    view.update(vcx, |app, _| {
        app.toast
            .current()
            .map(|t| t.message.clone())
            .unwrap_or_default()
    })
}

/// Renames the focused keyword through the detail panel's field.
fn rename_in_manager(vcx: &mut VisualTestContext, from: &'static str, to: &str) {
    click(vcx, from);
    keystrokes(vcx, "enter secondary-backspace");
    type_text(vcx, to);
    keystrokes(vcx, "enter");
}

fn sidecar_tags(engine: &LuminaEngine, id: MediaId) -> Vec<String> {
    let item = engine.get_item_by_id(id).expect("the item exists");
    let mut tags = lumina_core::read_sidecar(&item.path)
        .expect("the sidecar reads")
        .map(|xmp| xmp.tags)
        .unwrap_or_default();
    tags.sort();
    tags
}

#[core::prelude::v1::test]
fn undo_merge_restores_rows_and_rules() {
    let (_dir, engine) = seed_library("kw_undo_merge", 4);
    let items = engine.get_item_slice(0..4);
    let (a, b, c) = (items[0].id, items[1].id, items[2].id);
    tag(&engine, a, &["cat"]);
    tag(&engine, b, &["cat", "cats"]);
    tag(&engine, c, &["cats", "kitty"]);
    tag(&engine, items[3].id, &["cats"]);
    let ids: Vec<MediaId> = items.iter().map(|item| item.id).collect();
    engine.write_metadata_sidecars(&ids).unwrap();
    let album = engine
        .create_smart_album(
            "Cats",
            "tag",
            lumina_core::MatchMode::All,
            &[tag_rule("cat")],
        )
        .unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_manager(&view, vcx);

    // Keyboard merge: check three, ⌘M folds `cat` and `kitty` into the
    // most-used `cats` (two merges, one undo entry).
    click(vcx, "tag-manager-checkbox-cat");
    click(vcx, "tag-manager-checkbox-cats");
    click(vcx, "tag-manager-checkbox-kitty");
    keystrokes(vcx, "secondary-m");
    vcx.run_until_parked();
    assert_eq!(sorted_tags(&engine, a), ["cats"]);
    assert_eq!(sorted_tags(&engine, b), ["cats"]);
    assert_eq!(sorted_tags(&engine, c), ["cats"]);
    assert_eq!(sidecar_tags(&engine, c), ["cats"]);
    assert_eq!(
        album_rule(&engine, album.id),
        lumina_core::RuleValue::Text("cats".into())
    );
    view.update(vcx, |app, _| {
        assert_eq!(app.undo.undo_stack.len(), 1, "the merge is one undo entry");
        assert!(app.toast.current().is_some_and(|t| t.action.is_some()));
    });

    // ⌘Z with the tag manager still open.
    keystrokes(vcx, "secondary-z");
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        assert!(app.tags_page_shown(), "⌘Z undoes inside the tag manager");
        assert!(app.undo.undo_stack.is_empty(), "one ⌘Z undid both merges");
        assert!(
            app.library_tag_counts.iter().any(|(t, _)| t == "kitty"),
            "the manager's list is fresh"
        );
    });
    assert_eq!(sorted_tags(&engine, a), ["cat"], "a is exactly as before");
    assert_eq!(
        sorted_tags(&engine, b),
        ["cat", "cats"],
        "the item that had both keeps both"
    );
    assert_eq!(sorted_tags(&engine, c), ["cats", "kitty"]);
    assert_eq!(
        sidecar_tags(&engine, b),
        ["cat", "cats"],
        "undo rewrites the sidecars of the items it changed"
    );
    assert_eq!(sidecar_tags(&engine, c), ["cats", "kitty"]);
    assert_eq!(
        album_rule(&engine, album.id),
        lumina_core::RuleValue::Text("cat".into()),
        "the smart album rule is back on the old keyword"
    );
    view.update(vcx, |app, _| {
        assert!(app.error_message.is_none());
        let count = |tag: &str| {
            app.library_tag_counts
                .iter()
                .find(|(t, _)| t == tag)
                .map(|(_, n)| *n)
        };
        assert_eq!(count("cat"), Some(2), "sidebar counts follow the undo");
        assert_eq!(count("cats"), Some(3));
        assert_eq!(
            app.user_album_counts.get(&album.id).copied(),
            Some(2),
            "the smart album count follows the restored rule"
        );
    });

    // Redo merges again.
    keystrokes(vcx, "secondary-shift-z");
    assert_eq!(sorted_tags(&engine, a), ["cats"]);
    assert_eq!(sorted_tags(&engine, b), ["cats"]);
}

#[core::prelude::v1::test]
fn undo_rename_restores_alias_targets() {
    let (_dir, engine) = seed_library("kw_undo_alias", 1);
    let id = engine.get_item_slice(0..1)[0].id;
    tag(&engine, id, &["places/nyc"]);
    engine.set_alias("big apple", "places/nyc").unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_manager(&view, vcx);

    // A search would replace the page, so only the detail panel follows
    // (search retargeting: `tag_manager::rename_and_delete_follow_a_selected_child`).
    rename_in_manager(vcx, "tag-manager-row-places", "spots");
    assert_eq!(sorted_tags(&engine, id), ["spots/nyc"]);
    view.update(vcx, |app, _| {
        assert_eq!(app.tag_manager.focused.as_deref(), Some("spots"));
    });
    assert_eq!(
        engine.aliases().unwrap(),
        [("big apple".to_string(), "spots/nyc".to_string())]
    );

    keystrokes(vcx, "secondary-z");
    assert_eq!(sorted_tags(&engine, id), ["places/nyc"]);
    assert_eq!(
        engine.aliases().unwrap(),
        [("big apple".to_string(), "places/nyc".to_string())],
        "undo points the alias back at the old keyword"
    );
    view.update(vcx, |app, _| {
        assert!(app.error_message.is_none());
        assert_eq!(
            app.keywords.aliases,
            [("big apple".to_string(), "places/nyc".to_string())],
            "app state follows"
        );
        assert_eq!(
            app.tag_manager.focused.as_deref(),
            Some("places"),
            "the detail panel points back"
        );
    });
}

#[core::prelude::v1::test]
fn undo_rename_skips_album_edited_since() {
    let (_dir, engine) = seed_library("kw_undo_album_edit", 1);
    let id = engine.get_item_slice(0..1)[0].id;
    tag(&engine, id, &["cat"]);
    let album = engine
        .create_smart_album("C", "tag", lumina_core::MatchMode::All, &[tag_rule("cat")])
        .unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_manager(&view, vcx);

    rename_in_manager(vcx, "tag-manager-row-cat", "kitty");
    assert_eq!(
        album_rule(&engine, album.id),
        lumina_core::RuleValue::Text("kitty".into())
    );
    engine
        .update_smart_album(
            album.id,
            "C",
            "tag",
            lumina_core::MatchMode::All,
            &[tag_rule("dog")],
        )
        .unwrap();

    close_and_undo(vcx);
    assert_eq!(sorted_tags(&engine, id), ["cat"], "the tag rows come back");
    assert_eq!(
        album_rule(&engine, album.id),
        lumina_core::RuleValue::Text("dog".into()),
        "an album edited after the rename keeps the later edit"
    );
    let message = toast(&view, vcx);
    assert!(
        message.contains("1 smart album edited since was kept"),
        "the toast says an album was skipped: {message:?}"
    );
}

#[core::prelude::v1::test]
fn undo_subtree_delete_restores_children() {
    let (_dir, engine) = seed_library("kw_undo_subtree", 2);
    let items = engine.get_item_slice(0..2);
    tag(&engine, items[0].id, &["a", "a/x", "a/x/y", "b"]);
    tag(&engine, items[1].id, &["a/z"]);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_manager(&view, vcx);

    click(vcx, "tag-manager-row-a");
    click(vcx, "tag-manager-delete-a");
    assert_eq!(sorted_tags(&engine, items[0].id), ["b"]);
    assert!(sorted_tags(&engine, items[1].id).is_empty());
    assert!(
        toast(&view, vcx).contains("Deleted tag"),
        "an undo toast shows"
    );

    close_and_undo(vcx);
    assert_eq!(
        sorted_tags(&engine, items[0].id),
        ["a", "a/x", "a/x/y", "b"],
        "every child row comes back"
    );
    assert_eq!(sorted_tags(&engine, items[1].id), ["a/z"]);
    view.update(vcx, |app, _| {
        assert!(app.library_tag_counts.iter().any(|(t, _)| t == "a/x/y"));
    });
}

#[core::prelude::v1::test]
fn bulk_delete_is_one_undo_step() {
    let (_dir, engine) = seed_library("kw_undo_bulk", 2);
    let items = engine.get_item_slice(0..2);
    tag(&engine, items[0].id, &["red", "green"]);
    tag(&engine, items[1].id, &["blue", "red"]);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_manager(&view, vcx);

    for row in [
        "tag-manager-checkbox-red",
        "tag-manager-checkbox-green",
        "tag-manager-checkbox-blue",
    ] {
        click(vcx, row);
    }
    keystrokes(vcx, "secondary-backspace");
    assert!(sorted_tags(&engine, items[0].id).is_empty());
    assert!(sorted_tags(&engine, items[1].id).is_empty());
    view.update(vcx, |app, _| {
        assert_eq!(
            app.undo.undo_stack.len(),
            1,
            "deleting three tags is one undo entry"
        );
    });
    assert_eq!(toast(&view, vcx), "Deleted 3 tags");

    close_and_undo(vcx);
    assert_eq!(sorted_tags(&engine, items[0].id), ["green", "red"]);
    assert_eq!(sorted_tags(&engine, items[1].id), ["blue", "red"]);
    view.update(vcx, |app, _| {
        assert!(app.undo.undo_stack.is_empty(), "one ⌘Z undid all three");
        assert_eq!(app.undo.redo_stack.len(), 1);
    });
}

#[core::prelude::v1::test]
fn undo_after_item_purged_skips_missing_ids() {
    let (_dir, engine) = seed_library("kw_undo_purged", 2);
    let items = engine.get_item_slice(0..2);
    let (a, b) = (items[0].id, items[1].id);
    tag(&engine, a, &["cat"]);
    tag(&engine, b, &["cat"]);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_manager(&view, vcx);

    click(vcx, "tag-manager-row-cat");
    click(vcx, "tag-manager-delete-cat");
    engine
        .permanently_delete_items(&[b], lumina_core::DeletionTarget::CatalogOnly)
        .unwrap();

    close_and_undo(vcx);
    view.update(vcx, |app, _| {
        assert!(
            app.error_message.is_none(),
            "a purged item must not fail the undo: {:?}",
            app.error_message
        );
        assert!(app.undo.undo_stack.is_empty(), "the undo was applied");
        let cat = app.library_tag_counts.iter().find(|(t, _)| t == "cat");
        assert_eq!(cat.map(|(_, n)| *n), Some(1), "only the live item");
    });
    assert_eq!(sorted_tags(&engine, a), ["cat"]);
    assert!(
        engine.get_item_by_id(b).is_none(),
        "the purged item stays purged"
    );
}

#[core::prelude::v1::test]
fn undo_rename_restores_the_tag_view() {
    let (_dir, engine) = seed_library("kw_undo_sidebar", 3);
    let items = engine.get_item_slice(0..3);
    tag(&engine, items[0].id, &["cat"]);
    tag(&engine, items[1].id, &["cat"]);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    // Rename on the Tags page, then open the renamed keyword's view: the
    // undo and redo move that view with the keyword.
    open_manager(&view, vcx);
    rename_in_manager(vcx, "tag-manager-row-cat", "kitty");
    view.update(vcx, |app, _| {
        assert!(app.toast.current().is_some_and(|t| t.action.is_some()));
    });
    assert_eq!(sorted_tags(&engine, items[0].id), ["kitty"]);
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::Tag("kitty".into()), cx)
    });
    render_ui(vcx);

    keystrokes(vcx, "secondary-z");
    assert_eq!(sorted_tags(&engine, items[0].id), ["cat"]);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::Tag("cat".into()),
            "undo puts the view back on the old keyword"
        );
        assert_eq!(app.get_active_count(), 2, "with its items");
    });

    keystrokes(vcx, "secondary-shift-z");
    view.update(vcx, |app, _| {
        assert_eq!(app.selection, AlbumSelection::Tag("kitty".into()), "redo");
        assert_eq!(app.get_active_count(), 2);
    });
}

#[core::prelude::v1::test]
fn undo_is_refused_while_the_old_keyword_is_an_alias() {
    let (_dir, engine) = seed_library("kw_undo_alias_refused", 1);
    let id = engine.get_item_slice(0..1)[0].id;
    tag(&engine, id, &["cat"]);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_manager(&view, vcx);

    rename_in_manager(vcx, "tag-manager-row-cat", "kitty");
    engine.set_alias("cat", "dog").unwrap();

    keystrokes(vcx, "secondary-z");
    view.update(vcx, |app, _| {
        let error = app.error_message.clone().unwrap_or_default();
        assert!(
            error.contains("remove the alias first"),
            "the banner says why and what to do: {error:?}"
        );
        assert_eq!(
            app.undo.undo_stack.len(),
            1,
            "a refused undo stays on the stack for a retry"
        );
        assert!(app.undo.redo_stack.is_empty());
    });
    assert_eq!(
        sorted_tags(&engine, id),
        ["kitty"],
        "nothing was written, so the item keeps its keyword"
    );

    engine.remove_alias("cat").unwrap();
    keystrokes(vcx, "secondary-z");
    assert_eq!(sorted_tags(&engine, id), ["cat"], "the retry succeeds");
    view.update(vcx, |app, _| assert!(app.undo.undo_stack.is_empty()));
}

#[core::prelude::v1::test]
fn cmd_z_with_filter_text_edits_the_filter_not_the_catalog() {
    let (_dir, engine) = seed_library("kw_undo_filter_text", 1);
    let id = engine.get_item_slice(0..1)[0].id;
    tag(&engine, id, &["cat"]);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_manager(&view, vcx);
    rename_in_manager(vcx, "tag-manager-row-cat", "kitty");
    view.update(vcx, |app, _| assert_eq!(app.undo.undo_stack.len(), 1));

    click(vcx, "tag-manager-search");
    type_text(vcx, "ki");
    view.update(vcx, |app, _| assert_eq!(app.tag_manager.search, "ki"));
    keystrokes(vcx, "secondary-z");
    view.update(vcx, |app, _| {
        assert_ne!(app.tag_manager.search, "ki", "⌘Z undid the typing");
        assert_eq!(
            app.undo.undo_stack.len(),
            1,
            "with text in the filter, ⌘Z never reaches the catalog"
        );
    });
    assert_eq!(sorted_tags(&engine, id), ["kitty"]);
}

#[core::prelude::v1::test]
fn merge_clears_grid_positions_but_rename_keeps_them() {
    let (_dir, engine) = seed_library("kw_undo_positions", 3);
    let items = engine.get_item_slice(0..3);
    tag(&engine, items[0].id, &["cat"]);
    tag(&engine, items[1].id, &["cats"]);
    tag(&engine, items[2].id, &["cats"]);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let select_in = |view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, path: &str| {
        view.update(vcx, |app, cx| {
            app.refresh_album_counts();
            app.select_album(AlbumSelection::Tag(path.into()), cx);
            app.select_index(0, cx);
        });
        render_ui(vcx);
        view.update(vcx, |app, _| assert!(app.selected_item.is_some()));
    };

    // The page drops the grid selection on arrival, so the edits run there
    // and their undo runs from the keyword's view with a selection.
    let merge_on_page = |view: &Entity<LuminaApp>, vcx: &mut VisualTestContext| {
        open_manager(view, vcx);
        click(vcx, "tag-manager-checkbox-cat");
        click(vcx, "tag-manager-checkbox-cats");
        keystrokes(vcx, "secondary-m");
    };

    // A rename onto a new path shows the same items: its undo keeps
    // positions.
    open_manager(&view, vcx);
    rename_in_manager(vcx, "tag-manager-row-cat", "kitten");
    select_in(&view, vcx, "kitten");
    keystrokes(vcx, "secondary-z");
    view.update(vcx, |app, _| {
        assert_eq!(app.selection, AlbumSelection::Tag("cat".into()));
        assert!(
            app.selected_item.is_some(),
            "a rename's undo keeps the selection"
        );
    });

    // A merge into `cats` shows other items: its undo clears positions, and
    // a view on the target stays there.
    merge_on_page(&view, vcx);
    assert_eq!(sorted_tags(&engine, items[0].id), ["cats"], "⌘M merged");
    select_in(&view, vcx, "cats");
    keystrokes(vcx, "secondary-z");
    view.update(vcx, |app, _| {
        assert_eq!(app.selection, AlbumSelection::Tag("cats".into()));
        assert!(
            app.selected_indices.is_empty() && app.selected_item.is_none(),
            "a merge's undo must not keep positions into a different item set"
        );
    });
    assert_eq!(sorted_tags(&engine, items[0].id), ["cat"], "⌘Z unmerged");

    // A merge also changes what a view it did not move shows (a parent
    // tag, a `tag:` search): its redo clears positions there too.
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::AllLibrary, cx);
        app.select_index(0, cx);
    });
    render_ui(vcx);
    keystrokes(vcx, "secondary-shift-z");
    assert_eq!(
        sorted_tags(&engine, items[0].id),
        ["cats"],
        "⇧⌘Z merged again"
    );
    view.update(vcx, |app, _| {
        assert_eq!(app.selection, AlbumSelection::AllLibrary);
        assert!(
            app.selected_indices.is_empty() && app.selected_item.is_none(),
            "a merge (here its redo) clears positions even when the view did not move"
        );
    });
}

#[core::prelude::v1::test]
fn redoing_a_merge_moves_a_source_view_and_a_dead_view_falls_back() {
    let (_dir, engine) = seed_library("kw_undo_merge_views", 3);
    let items = engine.get_item_slice(0..3);
    tag(&engine, items[0].id, &["cat"]);
    // The most used is the merge target.
    tag(&engine, items[1].id, &["cats"]);
    tag(&engine, items[2].id, &["cats"]);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_manager(&view, vcx);
    click(vcx, "tag-manager-checkbox-cat");
    click(vcx, "tag-manager-checkbox-cats");
    keystrokes(vcx, "secondary-m");
    keystrokes(vcx, "secondary-z");
    // On the source's view, redo folds it into the target: follow it.
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::Tag("cat".into()), cx)
    });
    keystrokes(vcx, "secondary-shift-z");
    view.update(vcx, |app, _| {
        assert_eq!(app.selection, AlbumSelection::Tag("cats".into()));
    });

    // A deleted keyword's view falls back to All Media on redo.
    open_manager(&view, vcx);
    view.update(vcx, |app, cx| app.delete_tag_globally("cats".into(), cx));
    keystrokes(vcx, "secondary-z");
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::Tag("cats".into()), cx)
    });
    keystrokes(vcx, "secondary-shift-z");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::AllLibrary,
            "no empty view on a keyword that is gone"
        );
    });
}
