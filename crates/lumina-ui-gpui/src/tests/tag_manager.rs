//! v3 Tag Manager (FIXES §7): selection bar with Merge, usage bars,
//! select-all over shown rows, and the detail panel.

use super::tag_manager_sidecars::sidecar_library;
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

pub(super) fn open(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) {
    view.update(vcx, |app, cx| {
        app.refresh_tag_counts();
        app.open_tag_manager(cx);
    });
    render_ui(vcx);
}

pub(super) fn checkbox(tag: &str) -> &'static str {
    leak(format!("tag-manager-checkbox-{tag}"))
}

pub(super) fn count(
    view: &Entity<LuminaApp>,
    vcx: &mut VisualTestContext,
    tag: &str,
) -> Option<usize> {
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
fn select_all_only_touches_shown_rows_and_esc_deselects() {
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
        assert!(app.tags_page_shown(), "Esc first deselects");
        assert!(app.tag_manager.selected.is_empty());
    });
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(
            app.tags_page_shown(),
            "then does nothing: a page has no close"
        )
    });

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
    assert_eq!(f32::from(row.size.height), 44.0, "rows are 44px");
    let full = vcx.debug_bounds("tag-manager-usage-fill-beach").unwrap();
    let third = vcx.debug_bounds("tag-manager-usage-fill-coast").unwrap();
    assert_eq!(
        f32::from(full.size.width),
        190.0,
        "the most-used tag fills the bar"
    );
    assert!(
        (f32::from(third.size.width) - 190.0 / 3.0).abs() < 0.5,
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

#[core::prelude::v1::test]
fn tags_page_opens_in_main_area() {
    use crate::views::sidebar::AlbumSelection;
    let (dir, engine) = tag_library("tm_page");
    let item = engine.get_item_slice(0..1)[0].clone();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    view.update(vcx, |app, _| app.refresh_tag_counts());
    render_ui(vcx);

    crate::tests::duplicates::run_palette_command(vcx, &view, "Open Tags");
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("tags-page").is_some(),
        "Open Tags shows the page in the main column"
    );
    let title = vcx.debug_bounds("page-title-text:Tags");
    assert!(title.is_some(), "the page header reads Tags");
    for gone in [
        "tag-manager-backdrop",
        "tag-manager-dialog",
        "media-grid-surface",
        "sidebar-tag-rename",
        "sidebar-tag-filter-beach",
        "sidebar-tag-name-beach",
    ] {
        assert!(
            vcx.debug_bounds(gone).is_none(),
            "the Tags page is not a sheet and the sidebar has no keyword tree: {gone}"
        );
    }
    assert!(
        vcx.debug_bounds("sidebar-nav-keywords-active").is_some(),
        "TAGS › Keywords is the active sidebar row"
    );

    // ↑/↓ move the tag focus (A–Z: beach, coast, sea, unique).
    keystrokes(vcx, "down down up");
    view.update(vcx, |app, _| {
        assert_eq!(app.tag_manager.focused.as_deref(), Some("beach"));
    });

    // Grid shortcuts must not reach media behind the page, even with a
    // selection left over.
    view.update(vcx, |app, cx| app.select_item_internal(item.clone(), cx));
    keystrokes(
        vcx,
        "backspace delete secondary-backspace f backspace escape",
    );
    vcx.run_until_parked();
    let after = engine
        .get_item_by_id(item.id)
        .expect("item still in the catalog");
    assert!(
        !after.is_favorite,
        "F on the page filters; it does not favourite"
    );
    assert!(!after.is_trashed, "⌫ on the page does not trash");
    view.update(vcx, |app, _| {
        assert_eq!(app.selection, AlbumSelection::Tags, "Esc keeps the page");
    });

    keystrokes(vcx, "secondary-[");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::AllLibrary,
            "⌘[ returns to the view before the page"
        );
    });
    render_ui(vcx);
    assert!(vcx.debug_bounds("tags-page").is_none());
    assert!(vcx.debug_bounds("media-grid-surface").is_some());

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn inspector_and_open_tags_do_nothing_on_the_tags_page() {
    use crate::app::command_palette::CommandId;
    let (dir, engine) = tag_library("tm_page_inspector");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, _| {
        assert!(app.command_applies(CommandId::OpenTagManager));
    });
    open(&view, vcx);
    let before = view.update(vcx, |app, _| app.inspector_open);
    view.update(vcx, |app, cx| app.toggle_inspector(cx));
    view.update(vcx, |app, _| {
        assert_eq!(
            app.inspector_open, before,
            "the toggle does not flip the hidden inspector setting"
        );
        assert!(
            !app.command_applies(CommandId::ToggleInspector),
            "⌘K does not list Toggle Inspector where it would do nothing"
        );
        assert!(
            !app.command_applies(CommandId::OpenTagManager),
            "nor Open Tags on the Tags page"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}
