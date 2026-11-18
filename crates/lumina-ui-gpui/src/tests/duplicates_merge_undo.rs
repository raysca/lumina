//! UI automation for undo/redo of a duplicate merge and for what Merge all
//! refuses while it runs.

use super::duplicates_review::seed_two_groups;
use super::helpers::{app_with_library, click, focus_app, keystrokes, render_ui, seed_library};
use lumina_core::{DeletionTarget, LuminaEngine, MediaId};

/// A two-copy library in Duplicates, the plain copy chosen as keeper and
/// the other tagged "beach"; returns (copy, keep).
fn merge_tagged_copy(
    engine: &std::sync::Arc<LuminaEngine>,
    vcx: &mut gpui::VisualTestContext,
) -> (MediaId, MediaId) {
    let ids: Vec<_> = engine.get_item_slice(0..2).iter().map(|i| i.id).collect();
    let (copy, keep) = (ids[0], ids[1]);
    click(vcx, "sidebar-nav-duplicates");
    render_ui(vcx);
    click(
        vcx,
        super::helpers::leak(format!("duplicate-keep-{}", keep.0)),
    );
    click(vcx, "duplicates-merge-group");
    assert!(
        engine
            .get_tags_for_item(keep)
            .contains(&"beach".to_string()),
        "control: the merge gave the keeper the tag"
    );
    (copy, keep)
}

fn tagged_library(prefix: &str) -> (super::helpers::TempLibrary, std::sync::Arc<LuminaEngine>) {
    let (temp_dir, engine) = seed_library(prefix, 2);
    let copy = engine.get_item_slice(0..1)[0].id;
    engine.add_tags_batch(&[copy], &["beach".into()]).unwrap();
    (temp_dir, engine)
}

/// Regression: undo after Empty Recently Deleted stripped the purged
/// copy's tag from the keeper and put nothing back, losing it for good.
#[core::prelude::v1::test]
fn undo_after_the_copy_was_purged_leaves_the_keeper_merged() {
    let (_temp_dir, engine) = tagged_library("dup_undo_purged");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    render_ui(vcx);
    let (_copy, keep) = merge_tagged_copy(&engine, vcx);
    engine.empty_trash(DeletionTarget::CatalogOnly).unwrap();

    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-z");
    render_ui(vcx);
    assert_eq!(
        engine.get_tags_for_item(keep),
        vec!["beach".to_string()],
        "the purged copy's tag stays on the keeper"
    );
    view.update(vcx, |app, _| {
        assert!(app.error_message.is_none(), "{:?}", app.error_message);
        let toast = app.toast.current().map(|t| t.message.clone());
        assert!(
            toast.as_deref().is_some_and(|m| m.contains("stays merged")),
            "the toast says the group stayed merged: {toast:?}"
        );
    });
    drop(view);
}

/// Regression: redo only moved the copy back to Recently Deleted, so the
/// keeper lacked its tag and a later Empty Recently Deleted lost it.
#[core::prelude::v1::test]
fn redo_merges_again_so_emptying_the_trash_keeps_the_tag() {
    let (_temp_dir, engine) = tagged_library("dup_redo_merge");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    render_ui(vcx);
    let (copy, keep) = merge_tagged_copy(&engine, vcx);

    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-z");
    render_ui(vcx);
    assert!(
        engine.get_tags_for_item(keep).is_empty(),
        "control: undo took the tag off"
    );
    keystrokes(vcx, "secondary-shift-z");
    render_ui(vcx);
    assert_eq!(
        engine.get_item_slice(0..10).len(),
        1,
        "redo moves the copy back to Recently Deleted"
    );
    assert!(
        engine
            .get_tags_for_item(keep)
            .contains(&"beach".to_string()),
        "redo copies the tag onto the keeper again"
    );
    engine.empty_trash(DeletionTarget::CatalogOnly).unwrap();
    assert!(
        engine
            .get_tags_for_item(keep)
            .contains(&"beach".to_string()),
        "emptying Recently Deleted after a redo keeps the tag"
    );
    assert!(engine.get_item_by_id(copy).is_none(), "control: purged");
    view.update(vcx, |app, _| {
        assert!(app.error_message.is_none(), "{:?}", app.error_message);
    });
    drop(view);
}

/// Undoing a merge lists its group again: `refresh_after_history` reads the
/// groups through `refresh_album_counts` while Duplicates is open.
#[core::prelude::v1::test]
fn undoing_a_merge_brings_its_group_back() {
    let (_temp_dir, engine) = tagged_library("dup_undo_groups");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    render_ui(vcx);
    merge_tagged_copy(&engine, vcx);
    view.update(vcx, |app, _| {
        assert!(app.duplicate_groups().is_empty(), "control: merged away");
    });
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-z");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.duplicate_groups().len(),
            1,
            "undo lists the restored group again"
        );
    });
    drop(view);
}

/// Regression: a single merge, keeper change, skip or undo while Merge all
/// ran raced it: the background merge read the groups before they changed.
#[core::prelude::v1::test]
fn review_actions_and_undo_wait_for_merge_all() {
    let (_temp_dir, engine) = seed_two_groups("dup_merge_all_busy");
    let rated = engine.get_item_slice(0..1)[0].id;
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    render_ui(vcx);
    click(vcx, "sidebar-nav-duplicates");
    render_ui(vcx);
    view.update(vcx, |app, cx| {
        app.set_rating_on(&[rated], 3, false, cx);
        assert_eq!(app.undo.undo_stack.len(), 1, "control: one entry");
        app.merge_all_duplicates(cx);
        app.duplicates_merge_shown(cx);
        app.duplicates_pick_keeper(1, cx);
        app.duplicates_skip_shown(cx);
        app.duplicates_skip_all(cx);
        app.undo_last(cx);
        app.redo_last(cx);
        assert!(app.stacks.review.keepers.is_empty(), "no keeper changed");
        assert!(app.stacks.review.skipped.is_empty(), "nothing skipped");
        assert_eq!(app.undo.undo_stack.len(), 1, "the rating was not undone");
    });
    assert_eq!(
        engine.get_ratings_map().get(&rated).copied(),
        Some(3),
        "undo during Merge all changed nothing"
    );
    assert_eq!(
        engine.get_item_slice(0..10).len(),
        5,
        "no single merge ran beside Merge all"
    );
    assert!(
        vcx.debug_bounds("merge-all-duplicates").is_some(),
        "the busy button is still drawn"
    );

    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(!app.merge_all_running(), "Merge all finished");
        assert_eq!(
            app.undo.undo_stack.len(),
            2,
            "the rating, then Merge all on top"
        );
    });
    assert_eq!(engine.get_item_slice(0..10).len(), 2, "one copy per group");
    drop(view);
}

/// Regression: a Merge all still running in the previous library blocked
/// Merge all in the one opened since; and its result must not land there.
#[core::prelude::v1::test]
fn merge_all_from_a_closed_library_neither_blocks_nor_lands() {
    let (_old_dir, old) = seed_two_groups("dup_merge_all_old");
    let (new_dir, new) = seed_two_groups("dup_merge_all_new");
    drop(new);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, old.clone());
    render_ui(vcx);
    click(vcx, "sidebar-nav-duplicates");
    render_ui(vcx);
    view.update(vcx, |app, cx| {
        app.merge_all_duplicates(cx);
        crate::tests::helpers::install_library(app, new_dir.0.clone(), cx);
        assert!(
            !app.merge_all_running(),
            "the old library's Merge all does not block the new one"
        );
    });
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(
            app.undo.undo_stack.is_empty(),
            "the old library's merge is not undoable in the new one"
        );
        assert!(app.toast.current().is_none(), "no merge toast");
        assert!(!app.merge_all_running());
    });
    assert_eq!(
        old.get_item_slice(0..10).len(),
        2,
        "control: the old merge ran"
    );

    view.update(vcx, |app, cx| {
        app.refresh_duplicates();
        assert_eq!(
            app.duplicate_groups().len(),
            2,
            "the new library's groups are untouched"
        );
        app.merge_all_duplicates(cx);
        assert!(app.merge_all_running(), "Merge all runs in the new library");
    });
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.undo.undo_stack.len(), 1, "the new merge is undoable");
        assert!(app.duplicate_groups().is_empty());
    });
    drop(view);
}
