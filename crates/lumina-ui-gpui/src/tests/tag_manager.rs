//! v3 Tag Manager (FIXES §7): selection bar with Merge, usage bars,
//! select-all over shown rows, and the detail panel.

use crate::tests::ui_automation::{
    app_with_library, click, keystrokes, leak, render_ui, seed_library,
};
use crate::LuminaApp;
use gpui::{Entity, VisualTestContext};

/// beach on 3 items, sea on 2, coast and unique on 1.
fn tag_library(
    prefix: &str,
) -> (
    std::path::PathBuf,
    std::sync::Arc<lumina_core::LuminaEngine>,
) {
    let (dir, engine) = seed_library(prefix, 3);
    let ids: Vec<_> = engine.get_item_slice(0..3).iter().map(|i| i.id).collect();
    let tag = |ids: &[lumina_core::MediaId], name: &str| {
        engine.add_tags_batch(ids, &[name.to_string()]).unwrap();
    };
    tag(&ids, "beach");
    tag(&ids[..2], "sea");
    tag(&ids[2..], "coast");
    tag(&ids[..1], "unique");
    (dir, engine)
}

fn open(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) {
    view.update(vcx, |app, cx| {
        app.refresh_tag_counts();
        app.open_tag_manager(cx);
    });
    render_ui(vcx);
}

fn checkbox(tag: &str) -> &'static str {
    leak(format!("tag-manager-checkbox-{tag}"))
}

fn count(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, tag: &str) -> Option<usize> {
    view.update(vcx, |app, _| {
        app.library_tag_counts
            .iter()
            .find(|(t, _)| t == tag)
            .map(|(_, n)| *n)
    })
}

#[core::prelude::v1::test]
fn bar_shows_at_two_and_merge_folds_into_the_most_used() {
    let (dir, engine) = tag_library("tm_merge");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open(&view, vcx);

    click(vcx, checkbox("coast"));
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("tag-selection-bar").is_none(),
        "one checked tag uses the detail panel, not the bar"
    );
    click(vcx, checkbox("sea"));
    render_ui(vcx);
    let bar = vcx
        .debug_bounds("tag-selection-bar")
        .expect("two checked show the bar");
    assert_eq!(f32::from(bar.size.height), 48.0);
    for icon_button in ["bulk-delete-tags-btn", "clear-tag-selection-btn"] {
        let b = vcx.debug_bounds(icon_button).unwrap();
        assert_eq!(f32::from(b.size.width), 28.0, "{icon_button} is icon-only");
    }
    view.update(vcx, |app, _| {
        assert_eq!(
            app.tag_merge_target().as_deref(),
            Some("sea"),
            "the most-used wins"
        )
    });

    click(vcx, "merge-tags-btn");
    assert_eq!(count(&view, vcx, "coast"), None, "the merged tag is gone");
    assert_eq!(
        count(&view, vcx, "sea"),
        Some(3),
        "its items moved to the target"
    );
    view.update(vcx, |app, _| {
        assert!(app.tag_manager.selected.is_empty());
        assert_eq!(app.tag_manager.focused.as_deref(), Some("sea"));
        assert!(app.error_message.is_none());
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn select_all_only_touches_shown_rows_and_esc_deselects_then_closes() {
    let (dir, engine) = tag_library("tm_select_all");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open(&view, vcx);

    click(vcx, checkbox("unique"));
    keystrokes(vcx, "s");
    click(vcx, "tag-manager-select-all");
    let selected = |view: &Entity<LuminaApp>, vcx: &mut VisualTestContext| {
        view.update(vcx, |app, _| {
            let mut s: Vec<String> = app.tag_manager.selected.iter().cloned().collect();
            s.sort();
            s
        })
    };
    assert_eq!(selected(&view, vcx), ["coast", "sea", "unique"]);
    keystrokes(vcx, "secondary-a");
    assert_eq!(
        selected(&view, vcx),
        ["unique"],
        "⌘A clears only the shown rows; the hidden check stays"
    );

    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(app.tag_manager.open, "Esc first deselects");
        assert!(app.tag_manager.selected.is_empty());
    });
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| assert!(!app.tag_manager.open, "then closes"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn keyboard_checks_and_merges_and_trash_deletes() {
    let (dir, engine) = tag_library("tm_keys");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open(&view, vcx);

    // A–Z: beach, coast, sea, unique.
    keystrokes(vcx, "down space down space secondary-m");
    assert_eq!(
        count(&view, vcx, "coast"),
        None,
        "⌘M merges the checked tags"
    );
    assert_eq!(count(&view, vcx, "beach"), Some(3));

    click(vcx, checkbox("sea"));
    click(vcx, checkbox("unique"));
    render_ui(vcx);
    click(vcx, "bulk-delete-tags-btn");
    assert_eq!(count(&view, vcx, "sea"), None);
    assert_eq!(count(&view, vcx, "unique"), None);

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn rows_have_usage_bars_and_detail_shows_items_and_renames_in_place() {
    let (dir, engine) = tag_library("tm_detail");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open(&view, vcx);

    let row = vcx.debug_bounds("tag-manager-row-beach").unwrap();
    assert_eq!(f32::from(row.size.height), 36.0, "rows are 36px");
    let full = vcx.debug_bounds("tag-manager-usage-fill-beach").unwrap();
    let third = vcx.debug_bounds("tag-manager-usage-fill-coast").unwrap();
    assert_eq!(
        f32::from(full.size.width),
        110.0,
        "the most-used tag fills the bar"
    );
    assert!(
        (f32::from(third.size.width) - 110.0 / 3.0).abs() < 0.5,
        "bars are proportional: {third:?}"
    );

    click(vcx, "tag-manager-row-beach");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.tag_manager.tagged_items.len(), 3)
    });
    for selector in [
        "tag-manager-detail-name",
        "tag-manager-items",
        "tag-manager-delete-note",
        "tag-manager-show-beach",
    ] {
        assert!(
            vcx.debug_bounds(selector).is_some(),
            "{selector} must render"
        );
    }
    let name = vcx.debug_bounds("tag-manager-detail-name").unwrap();
    assert!(f32::from(name.size.height) >= 24.0, "the name is 24px");

    click(vcx, "tag-manager-rename-field");
    keystrokes(
        vcx,
        "backspace backspace backspace backspace backspace s h o r e enter",
    );
    assert_eq!(
        count(&view, vcx, "shore"),
        Some(3),
        "the name field renames in place"
    );
    assert_eq!(count(&view, vcx, "beach"), None);

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn cmd_backspace_edits_the_filter_and_only_deletes_with_the_bar_showing() {
    let (dir, engine) = tag_library("tm_cmd_backspace");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open(&view, vcx);

    click(vcx, checkbox("sea"));
    click(vcx, checkbox("coast"));
    keystrokes(vcx, "s secondary-backspace");
    assert_eq!(
        count(&view, vcx, "sea"),
        Some(2),
        "⌘⌫ while filtering must not delete"
    );
    keystrokes(vcx, "backspace");
    click(vcx, checkbox("coast"));
    keystrokes(vcx, "secondary-backspace");
    assert_eq!(
        count(&view, vcx, "sea"),
        Some(2),
        "one checked tag: no bar, no delete"
    );
    click(vcx, checkbox("coast"));
    keystrokes(vcx, "secondary-backspace");
    assert_eq!(
        count(&view, vcx, "sea"),
        None,
        "⌘⌫ deletes the checked tags"
    );
    assert_eq!(count(&view, vcx, "coast"), None);

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn moving_focus_cancels_a_rename_in_progress() {
    let (dir, engine) = tag_library("tm_rename_focus");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open(&view, vcx);

    click(vcx, "tag-manager-row-beach");
    render_ui(vcx);
    click(vcx, "tag-manager-rename-field");
    keystrokes(vcx, "x");
    click(vcx, "tag-manager-row-sea");
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert_eq!(app.tag_manager.focused.as_deref(), Some("sea"));
    });
    assert_eq!(
        count(&view, vcx, "beach"),
        Some(3),
        "the hidden rename was dropped"
    );
    assert_eq!(count(&view, vcx, "beachx"), None);

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn rename_keyword_rewrites_sidecars() {
    let (dir, engine) = seed_library("tm_rename_sidecar", 1);
    let item = engine.get_item_slice(0..1)[0].clone();
    engine
        .add_tags_batch(
            &[item.id],
            &["beach".to_string(), "beach/dunes".to_string()],
        )
        .unwrap();
    engine.write_metadata_sidecars(&[item.id]).unwrap();
    let sidecar_tags = || {
        let mut tags = lumina_core::read_sidecar(&item.path)
            .unwrap()
            .map(|xmp| xmp.tags)
            .unwrap_or_default();
        tags.sort();
        tags
    };
    assert_eq!(sidecar_tags(), ["beach", "beach/dunes"], "Lumina sidecar");

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open(&view, vcx);
    click(vcx, "tag-manager-row-beach");
    render_ui(vcx);
    click(vcx, "tag-manager-rename-field");
    keystrokes(
        vcx,
        "backspace backspace backspace backspace backspace s h o r e enter",
    );
    // The sidecar pass runs on the background executor.
    vcx.run_until_parked();
    assert_eq!(count(&view, vcx, "shore/dunes"), Some(1), "the child moved");
    assert_eq!(
        sidecar_tags(),
        ["shore", "shore/dunes"],
        "a rename rewrites the sidecar of every item it changed"
    );

    view.update(vcx, |app, cx| app.delete_tag_globally("shore".into(), cx));
    vcx.run_until_parked();
    assert!(
        sidecar_tags().is_empty(),
        "a delete rewrites the sidecar too, so Import XMP cannot bring the keyword back"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// One item tagged `tags`, with a Lumina sidecar listing them.
fn sidecar_library(
    prefix: &str,
    tags: &[&str],
) -> (
    std::path::PathBuf,
    std::sync::Arc<lumina_core::LuminaEngine>,
    lumina_core::MediaItem,
) {
    let (dir, engine) = seed_library(prefix, 2);
    let items = engine.get_item_slice(0..2);
    let tags: Vec<String> = tags.iter().map(|t| t.to_string()).collect();
    engine.add_tags_batch(&[items[0].id], &tags).unwrap();
    // A second item makes `beach` the most used, so merges fold into it.
    engine
        .add_tags_batch(&[items[1].id], &["beach".to_string()])
        .unwrap();
    engine.write_metadata_sidecars(&[items[0].id]).unwrap();
    (dir, engine, items[0].clone())
}

fn sidecar_tags(item: &lumina_core::MediaItem) -> Vec<String> {
    let mut tags = lumina_core::read_sidecar(&item.path)
        .unwrap()
        .map(|xmp| xmp.tags)
        .unwrap_or_default();
    tags.sort();
    tags
}

#[core::prelude::v1::test]
fn merge_and_bulk_delete_rewrite_sidecars() {
    let (dir, engine, item) =
        sidecar_library("tm_merge_sidecar", &["beach", "coast", "sea", "unique"]);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open(&view, vcx);

    click(vcx, checkbox("beach"));
    click(vcx, checkbox("coast"));
    render_ui(vcx);
    click(vcx, "merge-tags-btn");
    vcx.run_until_parked();
    assert_eq!(
        sidecar_tags(&item),
        ["beach", "sea", "unique"],
        "a merge rewrites the sidecars of the items it changed"
    );

    click(vcx, checkbox("sea"));
    click(vcx, checkbox("unique"));
    render_ui(vcx);
    click(vcx, "bulk-delete-tags-btn");
    vcx.run_until_parked();
    assert_eq!(count(&view, vcx, "sea"), None);
    assert_eq!(
        sidecar_tags(&item),
        ["beach"],
        "a bulk delete rewrites the sidecars of the items it changed"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn rename_and_delete_follow_a_selected_child() {
    let (dir, engine, _item) = sidecar_library("tm_child_refs", &["a/x"]);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open(&view, vcx);
    view.update(vcx, |app, cx| {
        app.set_search_query("tag:a/x", cx);
        app.selection = crate::views::sidebar::AlbumSelection::Tag("a/x".into());
        app.tag_manager.focused = Some("a/x".into());
        app.tag_manager.selected.insert("a/x".into());
        app.tag_manager.editing = Some("a".into());
        app.tag_manager.editing_buffer = "b".into();
        app.commit_modal_tag_rename(cx);
    });
    view.update(vcx, |app, _| {
        assert!(app.error_message.is_none());
        assert_eq!(
            app.selection,
            crate::views::sidebar::AlbumSelection::Tag("b/x".into()),
            "a selected child follows its parent's rename"
        );
        assert_eq!(app.search_query, "tag:b/x");
        assert_eq!(app.tag_manager.focused.as_deref(), Some("b/x"));
        assert!(app.tag_manager.selected.contains("b/x"));
        assert!(!app.tag_manager.selected.contains("a/x"));
    });

    view.update(vcx, |app, cx| app.delete_tag_globally("b".into(), cx));
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            crate::views::sidebar::AlbumSelection::AllLibrary,
            "deleting a parent drops a selected child"
        );
        assert!(app.search_query.is_empty());
        assert!(app.tag_manager.focused.is_none());
        assert!(app.tag_manager.selected.is_empty());
    });

    let _ = std::fs::remove_dir_all(&dir);
}
