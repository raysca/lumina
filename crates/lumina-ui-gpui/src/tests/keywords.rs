//! Keyword hierarchy in the UI (plan 4, Task 4): tree rows in the tag
//! manager, path-aware autocomplete with aliases, Move to…, the alias editor,
//! and the Untagged view surviving a relaunch.

use crate::config::{AppConfig, LastView};
use crate::tests::ui_automation::{
    app_with_library, click, keystrokes, leak, render_ui, seed_library, type_text,
};
use crate::views::sidebar::AlbumSelection;
use crate::LuminaApp;
use gpui::{Entity, VisualTestContext};
use lumina_core::{LuminaEngine, MediaId};

pub(super) fn tag(engine: &LuminaEngine, id: MediaId, tags: &[&str]) {
    let tags: Vec<String> = tags.iter().map(|t| t.to_string()).collect();
    engine.add_tags_batch(&[id], &tags).unwrap();
}

pub(super) fn open_manager(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) {
    view.update(vcx, |app, cx| {
        app.refresh_album_counts();
        app.open_tag_manager(cx);
    });
    render_ui(vcx);
}

/// Selects the first grid item with the keyboard and returns it and the
/// other item's id.
fn select_first(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> (MediaId, MediaId) {
    keystrokes(vcx, "home");
    view.update(vcx, |app, _| {
        let first = app.selected_item.clone().expect("Home selects an item").id;
        let other = app
            .get_active_slice(0..2)
            .into_iter()
            .map(|item| item.id)
            .find(|id| *id != first)
            .expect("a second item");
        (first, other)
    })
}

pub(super) fn sorted_tags(engine: &LuminaEngine, id: MediaId) -> Vec<String> {
    let mut tags = engine.get_tags_for_item(id);
    tags.sort();
    tags
}

#[core::prelude::v1::test]
fn keyword_tree_rows_indent_by_depth() {
    let (dir, engine) = seed_library("kw_tree", 1);
    let id = engine.get_item_slice(0..1)[0].id;
    // `a/b` has no items of its own: the tree still draws it.
    tag(&engine, id, &["a", "a/b/c", "a-z"]);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_manager(&view, vcx);

    let row = |vcx: &mut VisualTestContext, path: &str| {
        vcx.debug_bounds(leak(format!("tag-row-{path}")))
            .unwrap_or_else(|| panic!("row {path} must render"))
    };
    let (a, ab, abc, az) = (
        row(vcx, "a"),
        row(vcx, "a/b"),
        row(vcx, "a/b/c"),
        row(vcx, "a-z"),
    );
    for depth in 0..3 {
        assert!(
            vcx.debug_bounds(leak(format!("tag-row-indent-{depth}")))
                .is_some(),
            "a row at depth {depth} renders its indent"
        );
    }
    assert!(
        a.origin.y < ab.origin.y && ab.origin.y < abc.origin.y && abc.origin.y < az.origin.y,
        "rows sort by path, children right under their parent: {a:?} {ab:?} {abc:?} {az:?}"
    );
    let x = |b: gpui::Bounds<gpui::Pixels>| f32::from(b.origin.x);
    assert_eq!(x(ab) - x(a), 16.0, "one level indents 16 px");
    assert_eq!(x(abc) - x(a), 32.0, "two levels indent 32 px");
    assert_eq!(x(az), x(a), "a sibling with a similar name is not a child");

    // ↓ walks onto the synthesized row; its count is the whole subtree.
    keystrokes(vcx, "down down");
    view.update(vcx, |app, _| {
        assert_eq!(app.tag_manager.focused.as_deref(), Some("a/b"));
        assert_eq!(
            app.tag_manager.focused_count, 1,
            "an ancestor with no items of its own counts its subtree"
        );
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn typing_alias_suggests_canonical_and_applies_it() {
    let (dir, engine) = seed_library("kw_alias_suggest", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let (first, other) = select_first(&view, vcx);
    tag(&engine, other, &["places/usa/new york"]);
    engine.set_alias("nyc", "places/usa/new york").unwrap();
    view.update(vcx, |app, _| app.refresh_album_counts());

    // `ny` is neither the alias nor a keyword: only ↓↵ on the suggestion
    // can add `places/usa/new york` (↵ alone would add `ny`).
    keystrokes(vcx, "t n y");
    assert!(
        vcx.debug_bounds("tag-suggestion-places/usa/new york")
            .is_some(),
        "an alias suggests its keyword"
    );
    view.update(vcx, |app, _| {
        let labels: Vec<String> = app
            .tag_input_suggestions()
            .iter()
            .map(|s| s.label())
            .collect();
        assert_eq!(labels, ["places › usa › new york (nyc)"]);
    });
    keystrokes(vcx, "down enter");
    assert_eq!(
        sorted_tags(&engine, first),
        ["places/usa/new york"],
        "the suggestion applies the canonical keyword, never the alias"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn segment_prefix_suggests_child() {
    let (dir, engine) = seed_library("kw_segment", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let (first, other) = select_first(&view, vcx);
    tag(&engine, other, &["places/europe/paris", "party"]);
    view.update(vcx, |app, _| app.refresh_album_counts());

    keystrokes(vcx, "t p a r");
    for path in ["places/europe/paris", "party"] {
        assert!(
            vcx.debug_bounds(leak(format!("tag-suggestion-{path}")))
                .is_some(),
            "{path} matches `par` by a segment or the whole path"
        );
    }
    assert!(
        vcx.debug_bounds("tag-suggestion-places").is_none(),
        "only stored keywords are suggested"
    );
    // Equal counts sort A–Z: party, then places/europe/paris.
    keystrokes(vcx, "down down enter");
    assert_eq!(sorted_tags(&engine, first), ["places/europe/paris"]);

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn move_keyword_by_keyboard_updates_catalog() {
    let (dir, engine) = seed_library("kw_move", 2);
    let items = engine.get_item_slice(0..2);
    tag(&engine, items[0].id, &["paris", "paris/louvre"]);
    tag(&engine, items[1].id, &["europe", "paris"]);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_manager(&view, vcx);

    // A–Z: europe, paris, paris/louvre.
    keystrokes(vcx, "down down secondary-shift-m");
    view.update(vcx, |app, _| {
        assert_eq!(app.tag_manager.focused.as_deref(), Some("paris"));
        assert!(app.tag_manager.moving, "⇧⌘M starts Move to…");
        assert_eq!(app.tag_manager.focused_count, 2, "the prompt's item count");
    });
    assert!(
        vcx.debug_bounds("tag-detail-touches").is_some(),
        "the prompt says how many items the move changes"
    );
    type_text(vcx, "europe");
    keystrokes(vcx, "enter");

    assert_eq!(
        sorted_tags(&engine, items[0].id),
        ["europe/paris", "europe/paris/louvre"],
        "the keyword and its children move under the new parent"
    );
    assert_eq!(
        sorted_tags(&engine, items[1].id),
        ["europe", "europe/paris"]
    );
    view.update(vcx, |app, _| {
        assert!(app.error_message.is_none());
        assert!(!app.tag_manager.moving);
        assert_eq!(app.tag_manager.focused.as_deref(), Some("europe/paris"));
    });

    // From the sidebar's tag view the palette reaches the same prompt; an
    // empty parent moves the keyword to the top level.
    keystrokes(vcx, "escape");
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::Tag("europe/paris".into()), cx)
    });
    keystrokes(vcx, "secondary-k");
    type_text(vcx, "move keyword");
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert!(app.tags_page_shown(), "the command opens the tag manager");
        assert!(!app.command_palette_open);
        assert!(app.tag_manager.moving, "Move Keyword… opens the prompt");
        assert_eq!(app.tag_manager.editing_buffer, "europe");
    });
    keystrokes(vcx, "secondary-backspace enter");
    assert_eq!(sorted_tags(&engine, items[1].id), ["europe", "paris"]);

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn detail_panel_adds_and_removes_aliases() {
    let (dir, engine) = seed_library("kw_alias_detail", 1);
    let id = engine.get_item_slice(0..1)[0].id;
    tag(&engine, id, &["places/usa/new york"]);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_manager(&view, vcx);

    click(vcx, "tag-row-places/usa/new york");
    click(vcx, "tag-detail-alias-input");
    type_text(vcx, "NYC");
    keystrokes(vcx, "enter");
    assert_eq!(
        engine.aliases().unwrap(),
        [("nyc".to_string(), "places/usa/new york".to_string())],
        "Enter stores the alias for the focused keyword"
    );
    assert!(vcx.debug_bounds("tag-detail-alias-nyc").is_some());

    // Keyboard path: ⇧⌘A, then the same field.
    keystrokes(vcx, "secondary-shift-a");
    view.update(vcx, |app, _| {
        assert!(app.tag_manager.alias_editing, "⇧⌘A opens the alias field")
    });
    type_text(vcx, "big apple");
    keystrokes(vcx, "enter");
    assert_eq!(engine.aliases().unwrap().len(), 2);

    click(vcx, "tag-detail-alias-remove-nyc");
    assert_eq!(
        engine.aliases().unwrap(),
        [("big apple".to_string(), "places/usa/new york".to_string())],
        "× removes that alias"
    );
    // ⌫ in the empty field removes the last alias.
    keystrokes(vcx, "secondary-shift-a backspace escape");
    assert!(engine.aliases().unwrap().is_empty());
    view.update(vcx, |app, _| {
        assert!(app.keywords.aliases.is_empty(), "app state follows");
        assert!(app.error_message.is_none());
    });

    // An alias that is already a keyword is refused through the banner.
    click(vcx, "tag-detail-alias-input");
    type_text(vcx, "places");
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert!(app.error_message.is_some(), "the refusal is surfaced");
    });
    assert!(engine.aliases().unwrap().is_empty());

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn untagged_view_restores_after_relaunch() {
    let (dir, engine) = seed_library("kw_untagged_restore", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::Untagged, cx)
    });
    // What try_save_config writes, without touching the real config file.
    let saved = view.update(vcx, |app, _| AppConfig {
        last_view: Some(LastView::from_selection(&app.selection).token()),
        ..AppConfig::default()
    });
    assert_eq!(saved.last_view.as_deref(), Some("untagged"));

    let mut relaunch = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut relaunch, engine);
    view.update(vcx, |app, cx| {
        app.pending_restore = Some(saved.restored_session());
        app.handle_ui_message(
            crate::app::types::UiMessage::Engine(lumina_core::EngineEvent::ScanComplete {
                total_items: 2,
            }),
            cx,
        );
    });
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::Untagged,
            "Untagged comes back after a relaunch"
        )
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn palette_on_the_tags_page_cannot_rate_hidden_media() {
    let (dir, engine) = seed_library("kw_palette_over_page", 1);
    let id = engine.get_item_slice(0..1)[0].id;
    tag(&engine, id, &["beach"]);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    keystrokes(vcx, "home");
    open_manager(&view, vcx);

    keystrokes(vcx, "secondary-k");
    view.update(vcx, |app, _| {
        assert!(app.command_palette_open, "⌘K opens the palette on the page");
    });
    type_text(vcx, "rate 5");
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert!(app.tags_page_shown());
        assert!(
            app.selected_item.is_none(),
            "going to the page drops the grid selection"
        );
    });
    assert_eq!(
        engine.get_ratings_map().get(&id).copied().unwrap_or(0),
        0,
        "a palette command on the Tags page must not rate the media left behind"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn bulk_delete_reloads_the_focused_count() {
    let (dir, engine) = seed_library("kw_bulk_count", 2);
    let items = engine.get_item_slice(0..2);
    tag(&engine, items[0].id, &["a", "z"]);
    tag(&engine, items[1].id, &["a/x"]);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_manager(&view, vcx);

    click(vcx, "tag-manager-row-a");
    view.update(vcx, |app, _| assert_eq!(app.tag_manager.focused_count, 2));
    click(vcx, "tag-manager-checkbox-a/x");
    click(vcx, "tag-manager-checkbox-z");
    keystrokes(vcx, "secondary-backspace");
    view.update(vcx, |app, _| {
        assert_eq!(app.tag_manager.focused.as_deref(), Some("a"));
        assert_eq!(
            app.tag_manager.focused_count, 1,
            "the detail count follows a bulk delete below the focused keyword"
        );
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn filtered_select_all_never_reaches_an_unmatched_ancestor() {
    let (dir, engine) = seed_library("kw_filter_flat", 2);
    let items = engine.get_item_slice(0..2);
    tag(&engine, items[0].id, &["places", "places/rome", "other"]);
    tag(&engine, items[1].id, &["places/paris"]);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_manager(&view, vcx);

    // `r` matches places/paris, places/rome and other, not places.
    keystrokes(vcx, "r");
    assert!(
        vcx.debug_bounds("tag-row-places").is_none(),
        "a filtered list does not add the unmatched parent"
    );
    keystrokes(vcx, "secondary-a");
    view.update(vcx, |app, _| {
        let mut checked: Vec<String> = app.tag_manager.selected.iter().cloned().collect();
        checked.sort();
        assert_eq!(checked, ["other", "places/paris", "places/rome"]);
    });
    click(vcx, "bulk-delete-tags-btn");
    assert_eq!(
        sorted_tags(&engine, items[0].id),
        ["places"],
        "the unmatched parent keeps its own row"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn typing_after_a_highlight_adds_the_typed_text() {
    let (dir, engine) = seed_library("kw_stale_highlight", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let (first, other) = select_first(&view, vcx);
    let third = engine
        .get_item_slice(0..3)
        .into_iter()
        .map(|item| item.id)
        .find(|id| *id != first && *id != other)
        .unwrap();
    // Most used first: nebula, then north.
    tag(&engine, other, &["nebula", "north"]);
    tag(&engine, third, &["nebula"]);
    view.update(vcx, |app, _| app.refresh_album_counts());

    keystrokes(vcx, "t n down o enter");
    assert_eq!(
        sorted_tags(&engine, first),
        ["no"],
        "new text drops the highlight, so ↵ adds what was typed"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn sidebar_keywords_row_counts_keywords_and_lists_no_tree() {
    let (dir, engine) = seed_library("kw_sidebar_tree", 1);
    let id = engine.get_item_slice(0..1)[0].id;
    tag(&engine, id, &["a", "a/b/c"]);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, _| app.refresh_album_counts());
    render_ui(vcx);

    view.update(vcx, |app, _| {
        let keywords = crate::views::sidebar::nav::nav_entries(app)
            .into_iter()
            .find(|entry| entry.key == "keywords")
            .expect("TAGS has a Keywords row");
        assert_eq!(keywords.count, 2, "it counts the keywords items carry");
        assert_eq!(keywords.selection, AlbumSelection::Tags);
    });
    assert!(vcx.debug_bounds("sidebar-nav-keywords-count").is_some());
    for path in ["a", "a/b", "a/b/c"] {
        assert!(
            vcx.debug_bounds(leak(format!("sidebar-tag-name-{path}")))
                .is_none(),
            "the keyword tree lives on the Tags page, not the sidebar: {path}"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}
