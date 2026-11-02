//! UI automation tests for the Duplicates view, navigation, and merge actions.

use super::ui_automation::{
    app_with_library, click, focus_app, keystrokes, render_ui, seed_library,
};
use crate::views::sidebar::AlbumSelection;

fn run_palette_command(
    vcx: &mut gpui::VisualTestContext,
    view: &gpui::Entity<crate::LuminaApp>,
    query: &str,
) {
    focus_app(vcx, view);
    keystrokes(vcx, "secondary-k");
    for ch in query.chars() {
        let key = if ch == ' ' {
            "space".to_string()
        } else {
            ch.to_string()
        };
        keystrokes(vcx, &key);
    }
    keystrokes(vcx, "enter");
}

#[core::prelude::v1::test]
fn duplicates_navigation_and_empty_state() {
    // 1 item means 0 duplicates in library
    let (temp_dir, engine) = seed_library("dup_empty", 1);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    render_ui(vcx);

    // Click "Duplicates" in the sidebar
    click(vcx, "sidebar-nav-Duplicates");

    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::Duplicates,
            "clicking sidebar nav must switch to Duplicates view"
        );
        assert_eq!(app.duplicate_items_count, 0);
    });

    render_ui(vcx);
    drop(view);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn duplicates_view_renders_group_and_merges() {
    // 2 items seeded with same content are duplicates
    let (temp_dir, engine) = seed_library("dup_merge", 2);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    render_ui(vcx);

    // Navigate to duplicates
    click(vcx, "sidebar-nav-Duplicates");
    render_ui(vcx);

    view.update(vcx, |app, _| {
        assert_eq!(app.selection, AlbumSelection::Duplicates);
        assert_eq!(app.duplicate_items_count, 2, "two duplicate items found");
        assert_eq!(app.duplicate_groups.len(), 1, "one duplicate group found");
    });

    // Merge the group
    click(vcx, "merge-group-0");
    render_ui(vcx);

    view.update(vcx, |app, _| {
        assert_eq!(
            app.duplicate_items_count, 0,
            "duplicate resolved after merge"
        );
        assert_eq!(app.undo.undo_stack.len(), 1, "merge creates undo entry");
    });

    // Verify catalog has 1 active item and 1 trashed item
    let items = engine.get_item_slice(0..10);
    assert_eq!(items.len(), 1, "only 1 active item remains in library");

    // Undo the merge
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-z");
    render_ui(vcx);

    view.update(vcx, |app, _| {
        app.refresh_duplicates();
        assert_eq!(app.duplicate_items_count, 2, "undo restored duplicates");
    });

    drop(view);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn command_palette_navigates_to_duplicates() {
    let (temp_dir, engine) = seed_library("dup_palette", 2);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    render_ui(vcx);

    run_palette_command(vcx, &view, "Duplicates");
    render_ui(vcx);

    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::Duplicates,
            "palette command 'Duplicates' must navigate to Duplicates view"
        );
    });

    drop(view);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn command_palette_merges_all_duplicates() {
    let (temp_dir, engine) = seed_library("dup_pal_merge", 3);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    render_ui(vcx);

    view.update(vcx, |app, _| {
        app.refresh_duplicates();
        assert_eq!(app.duplicate_items_count, 3);
        assert_eq!(app.duplicate_groups.len(), 1);
    });

    run_palette_command(vcx, &view, "Merge All Duplicates");
    render_ui(vcx);

    view.update(vcx, |app, _| {
        assert_eq!(app.duplicate_items_count, 0, "all duplicates merged");
        assert_eq!(
            app.undo.undo_stack.len(),
            1,
            "undo entry created for batch merge"
        );
    });

    let items = engine.get_item_slice(0..10);
    assert_eq!(items.len(), 1, "only 1 active item remains after merge all");

    drop(view);
    let _ = std::fs::remove_dir_all(&temp_dir);
}
