//! UI automation tests for the Duplicates view, navigation, and merge actions.

use super::duplicates_review::seed_two_groups;
use super::helpers::{app_with_library, click, focus_app, keystrokes, render_ui, seed_library};
use crate::views::sidebar::AlbumSelection;

pub(crate) fn run_palette_command(
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
    let (_temp_dir, engine) = seed_library("dup_empty", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    render_ui(vcx);

    // Click "Duplicates" in the sidebar
    click(vcx, "sidebar-nav-duplicates");

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
}

#[core::prelude::v1::test]
fn duplicates_view_renders_group_and_merges() {
    // 2 items seeded with same content are duplicates
    let (_temp_dir, engine) = seed_library("dup_merge", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    render_ui(vcx);

    // Navigate to duplicates
    click(vcx, "sidebar-nav-duplicates");
    render_ui(vcx);

    view.update(vcx, |app, _| {
        assert_eq!(app.selection, AlbumSelection::Duplicates);
        assert_eq!(app.duplicate_items_count, 2, "two duplicate items found");
        assert_eq!(app.duplicate_groups.len(), 1, "one duplicate group found");
    });

    // Merge the group
    click(vcx, "duplicates-merge-group");
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
}

#[core::prelude::v1::test]
fn command_palette_navigates_to_duplicates() {
    let (_temp_dir, engine) = seed_library("dup_palette", 2);
    let mut cx = crate::tests::TestApp::single();
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
}

#[core::prelude::v1::test]
fn command_palette_merges_all_duplicates() {
    let (_temp_dir, engine) = seed_library("dup_pal_merge", 3);
    let mut cx = crate::tests::TestApp::single();
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
}

#[core::prelude::v1::test]
fn failed_merge_uses_the_error_banner() {
    let (temp_dir, engine) = seed_library("dup_merge_fail", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    render_ui(vcx);
    click(vcx, "sidebar-nav-duplicates");
    render_ui(vcx);
    rusqlite::Connection::open(temp_dir.join("catalog.db"))
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER fail_merge BEFORE UPDATE ON media_items
             BEGIN SELECT RAISE(ABORT, 'injected'); END;",
        )
        .unwrap();

    click(vcx, "duplicates-merge-group");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(
            app.error_message
                .as_deref()
                .is_some_and(|m| m.starts_with("Could not merge duplicates")),
            "a failed merge reaches the banner: {:?}",
            app.error_message
        );
        assert!(
            app.toast.current().is_none(),
            "no success-style toast for a failure"
        );
        assert!(
            app.undo.undo_stack.is_empty(),
            "a failed merge is not undoable"
        );
        assert_eq!(app.duplicate_items_count, 2, "the group is still shown");
    });
    assert_eq!(engine.get_item_slice(0..10).len(), 2, "nothing was trashed");

    drop(view);
}

#[core::prelude::v1::test]
fn duplicates_review_lists_groups_and_compares_selected() {
    let (_temp_dir, engine) = seed_two_groups("dup_review");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    render_ui(vcx);
    click(vcx, "sidebar-nav-duplicates");
    render_ui(vcx);

    let groups: Vec<Vec<u64>> = view.update(vcx, |app, _| {
        app.duplicate_groups
            .iter()
            .map(|g| g.items.iter().map(|i| i.id.0).collect())
            .collect()
    });
    assert_eq!(groups.len(), 2, "two exact-copy groups are listed");
    assert_eq!(groups[0].len(), 3, "the three-copy group sorts first");

    let list = vcx
        .debug_bounds("duplicates-group-list")
        .expect("the group list pane is drawn");
    let width: f32 = list.size.width.into();
    assert!(
        (width - 372.0).abs() <= 1.0,
        "the group list pane is 372 px wide, got {width}"
    );
    for row in ["duplicates-group-row-0", "duplicates-group-row-1"] {
        assert!(vcx.debug_bounds(row).is_some(), "{row} is drawn");
    }
    assert!(
        vcx.debug_bounds("page-title-text:Review Duplicates")
            .is_some(),
        "the page header reads Review Duplicates"
    );

    let compare = vcx
        .debug_bounds("duplicates-compare")
        .expect("the compare pane is drawn");
    let shown = |vcx: &mut gpui::VisualTestContext, ids: &[u64]| {
        ids.iter().all(|id| {
            vcx.debug_bounds(super::helpers::leak(format!("duplicate-candidate-{id}")))
                .is_some_and(|b| compare.contains(&b.origin))
        })
    };
    assert!(
        shown(vcx, &groups[0]),
        "group 0's copies are compared first"
    );
    assert!(
        !shown(vcx, &groups[1]),
        "group 1's copies are not shown yet"
    );

    click(vcx, "duplicates-group-row-1");
    assert!(shown(vcx, &groups[1]), "clicking row 1 compares group 1");
    assert!(
        !shown(vcx, &groups[0]),
        "group 0's copies are no longer shown"
    );

    drop(view);
}

#[core::prelude::v1::test]
fn clicked_candidate_shows_it_is_selected() {
    let (_temp_dir, engine) = seed_two_groups("dup_selected");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    render_ui(vcx);
    click(vcx, "sidebar-nav-duplicates");
    render_ui(vcx);
    let second = view.update(vcx, |app, _| app.duplicate_groups[0].items[1].id.0);
    let marker = super::helpers::leak(format!("duplicate-selected-text:{second}"));
    assert!(
        vcx.debug_bounds(marker).is_none(),
        "no card reads as selected before a click"
    );

    click(
        vcx,
        super::helpers::leak(format!("duplicate-candidate-{second}")),
    );
    view.update(vcx, |app, _| {
        assert_eq!(app.selected_item.as_ref().map(|i| i.id.0), Some(second));
    });
    assert!(
        vcx.debug_bounds(marker).is_some(),
        "the clicked card shows the selection the global keys act on"
    );

    drop(view);
}

#[core::prelude::v1::test]
fn review_hides_the_inspector_and_ignores_its_toggle() {
    let (_temp_dir, engine) = seed_two_groups("dup_inspector");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, _| app.inspector_open = true);
    render_ui(vcx);
    assert!(vcx.debug_bounds("inspector-panel").is_some());

    click(vcx, "sidebar-nav-duplicates");
    assert!(
        vcx.debug_bounds("inspector-panel").is_none(),
        "the Exact copies review hides the inspector"
    );
    view.update(vcx, |app, cx| {
        app.toggle_inspector(cx);
        assert!(
            app.inspector_open,
            "a toggle on the review must not flip the hidden setting"
        );
    });

    click(vcx, "sidebar-nav-all");
    assert!(
        vcx.debug_bounds("inspector-panel").is_some(),
        "Info is still on elsewhere"
    );

    drop(view);
}

#[core::prelude::v1::test]
fn failed_merge_all_keeps_every_copy_and_uses_the_error_banner() {
    let (temp_dir, engine) = seed_two_groups("dup_merge_all_fail");
    // Removed when the test ends, pass or fail.
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    render_ui(vcx);
    click(vcx, "sidebar-nav-duplicates");
    render_ui(vcx);
    let (groups_before, selected_before) = view.update(vcx, |app, _| {
        let groups: Vec<Vec<u64>> = app
            .duplicate_groups
            .iter()
            .map(|g| g.items.iter().map(|i| i.id.0).collect())
            .collect();
        (groups, app.selected_item.as_ref().map(|i| i.id.0))
    });
    assert_eq!(groups_before.len(), 2, "two groups to merge");
    // Fail only on the last group's copies, so the first group's trash
    // writes have already run inside the transaction when it aborts.
    let last = groups_before.last().unwrap();
    let ids = last
        .iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(",");
    rusqlite::Connection::open(temp_dir.join("catalog.db"))
        .unwrap()
        .execute_batch(&format!(
            "CREATE TRIGGER fail_merge_all BEFORE UPDATE OF is_trashed ON media_items
             WHEN NEW.is_trashed = 1 AND OLD.id IN ({ids})
             BEGIN SELECT RAISE(ABORT, 'injected'); END;"
        ))
        .unwrap();

    click(vcx, "merge-all-duplicates");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(
            app.error_message
                .as_deref()
                .is_some_and(|m| m.starts_with("Could not merge duplicates")),
            "a failed Merge all reaches the banner: {:?}",
            app.error_message
        );
        assert!(
            app.toast.current().is_none(),
            "no success-style toast for a failure"
        );
        assert!(
            app.undo.undo_stack.is_empty(),
            "a failed Merge all is not undoable"
        );
        let groups: Vec<Vec<u64>> = app
            .duplicate_groups
            .iter()
            .map(|g| g.items.iter().map(|i| i.id.0).collect())
            .collect();
        assert_eq!(groups, groups_before, "every group is still listed");
        assert_eq!(
            app.selected_item.as_ref().map(|i| i.id.0),
            selected_before,
            "the selection is not cleared by a merge that did not happen"
        );
    });
    assert!(
        vcx.debug_bounds("error-banner").is_some(),
        "the error banner is drawn"
    );
    assert_eq!(
        engine.get_item_slice(0..10).len(),
        5,
        "the merge is atomic: no copy of either group was trashed"
    );
}
