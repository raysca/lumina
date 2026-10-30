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
    let mut cx = gpui::TestAppContext::single();
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
    let mut cx = gpui::TestAppContext::single();
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
    let mut cx = gpui::TestAppContext::single();
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
    let mut cx = gpui::TestAppContext::single();
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
    let mut cx = gpui::TestAppContext::single();
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
    let mut cx = gpui::TestAppContext::single();
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
