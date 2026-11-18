//! Similar shots guards: no hidden selection while the
//! tab shows, a stale list never flags items that left the library, every
//! way back onto Duplicates starts on Exact copies, and the error state.

use crate::actions::{AddToAlbum, EditPhoto, RotateLeft};
use crate::app::command_palette::CommandId;
use crate::app::stacks::{DuplicatesTab, StacksList};
use crate::tests::helpers::{app_with_library, click, focus_app, keystrokes, leak, render_ui};
use crate::tests::stacks::{
    flags, loaded_ids, open_similar_shots, stacked_library, toast, undo_len,
};
use crate::views::sidebar::AlbumSelection;
use crate::*;
use lumina_core::Flag;

fn selection_is_empty(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> bool {
    view.update(vcx, |app, _| {
        app.selected_item.is_none()
            && app.selected_indices.is_empty()
            && app.get_selected_ids().is_empty()
    })
}

#[core::prelude::v1::test]
fn similar_shots_holds_no_selection_and_selection_actions_do_nothing() {
    let f = stacked_library("stacks_noselect");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, f.engine.clone());
    open_similar_shots(vcx);
    let copies = view.update(vcx, |app, _| app.get_active_count());
    assert!(
        copies >= 2,
        "Exact copies has items a hidden selection could take"
    );
    focus_app(vcx, &view);

    keystrokes(vcx, "secondary-a");
    assert!(
        selection_is_empty(&view, vcx),
        "⌘A selects nothing on Similar shots"
    );
    keystrokes(vcx, "shift-secondary-a");
    view.update(vcx, |app, cx| {
        assert!(app.albums.picker.is_none(), "⇧⌘A opens no album picker");
        app.select_all(cx);
        app.select_index(0, cx);
    });
    assert!(
        selection_is_empty(&view, vcx),
        "select_all / select_index refuse"
    );

    let before = undo_len(&view, vcx);
    vcx.dispatch_action(RotateLeft);
    vcx.dispatch_action(EditPhoto);
    vcx.dispatch_action(AddToAlbum);
    view.update(vcx, |app, cx| {
        app.execute_command(CommandId::Flag(Flag::Reject), cx);
        app.execute_command(CommandId::RotateRight, cx);
        app.execute_command(CommandId::ToggleFavorite, cx);
    });
    let all: Vec<_> = f.engine.get_item_slice(0..100);
    let ids: Vec<_> = all.iter().map(|i| i.id).collect();
    assert!(flags(&f.engine).is_empty(), "palette Reject flags nothing");
    assert!(
        f.engine
            .catalog()
            .get_rotations(&ids)
            .unwrap()
            .values()
            .all(|q| *q == 0),
        "Rotate (menu or palette) turns nothing"
    );
    assert!(
        all.iter().all(|i| !i.is_favorite),
        "Favorite changes nothing"
    );
    view.update(vcx, |app, _| {
        assert!(app.edits.session.is_none(), "Edit Photo opens nothing");
        assert!(app.albums.picker.is_none(), "Add to Album opens nothing");
    });
    assert_eq!(
        undo_len(&view, vcx),
        before,
        "nothing reached the undo stack"
    );

    drop(view);
}

#[core::prelude::v1::test]
fn back_and_forward_onto_duplicates_start_on_exact_copies() {
    let f = stacked_library("stacks_back");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, f.engine.clone());
    open_similar_shots(vcx);
    focus_app(vcx, &view);
    // Straight from Similar shots: no sidebar click resets the tab on the way.
    keystrokes(vcx, "secondary-[");
    keystrokes(vcx, "secondary-]");
    let check = |view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, step: &str| {
        view.update(vcx, |app, _| {
            assert_eq!(
                app.selection,
                AlbumSelection::Duplicates,
                "{step} lands on Duplicates"
            );
            assert_eq!(
                app.stacks.tab,
                DuplicatesTab::Exact,
                "{step} opens Exact copies"
            );
            assert!(
                !matches!(app.stacks.list, StacksList::Loaded(_)),
                "{step} drops the old list"
            );
            assert!(!app.similar_shots_shown());
        });
    };
    check(&view, vcx, "Back then Forward");
    click(vcx, "duplicates-kind-similar");
    click(vcx, "sidebar-nav-favorites");
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-[");
    check(&view, vcx, "Back");
    drop(view);
}

#[core::prelude::v1::test]
fn stale_list_never_flags_trashed_members_and_rereads() {
    let f = stacked_library("stacks_stale_list");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, f.engine.clone());
    open_similar_shots(vcx);

    // Trashed outside this view while the list is shown.
    f.engine.batch_set_trashed(&[f.shots[1]], true).unwrap();
    click(vcx, "stack-reject-others-1");
    assert!(
        flags(&f.engine).is_empty(),
        "nothing is flagged from a stale stack"
    );
    assert_eq!(toast(&view, vcx), Some(crate::i18n::t("stacks.changed")));
    assert_eq!(
        loaded_ids(&view, vcx),
        vec![vec![f.jpg, f.raw]],
        "the list is re-read: two shots left are no burst"
    );

    // Leaving and coming Back re-reads too.
    f.engine.batch_set_trashed(&[f.raw], true).unwrap();
    click(vcx, "sidebar-nav-favorites");
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-[");
    click(vcx, "duplicates-kind-similar");
    view.update(vcx, |app, _| {
        assert!(
            matches!(&app.stacks.list, StacksList::Loaded(s) if s.is_empty()),
            "after Back the trashed pair member is not listed"
        );
    });
    assert!(flags(&f.engine).is_empty());

    drop(view);
}

#[core::prelude::v1::test]
fn stacks_load_failure_shows_error_state_and_enter_retries() {
    let f = stacked_library("stacks_error");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, f.engine.clone());
    crate::app::stacks::STACKS_LOAD_FAILS.with(|c| c.set(true));
    open_similar_shots(vcx);
    crate::app::stacks::STACKS_LOAD_FAILS.with(|c| c.set(false));

    assert!(
        vcx.debug_bounds("stacks-error").is_some(),
        "a failed read shows the error state, not an empty list"
    );
    assert!(vcx.debug_bounds("stacks-empty").is_none());
    assert!(vcx.debug_bounds("stack-0").is_none());

    focus_app(vcx, &view);
    keystrokes(vcx, "enter");
    assert_eq!(
        loaded_ids(&view, vcx).len(),
        2,
        "↵ on the error state tries again"
    );
    assert!(vcx.debug_bounds("stacks-error").is_none());

    drop(view);
}

#[core::prelude::v1::test]
fn many_stacks_render_only_visible_rows_and_focus_scrolls() {
    const PAIRS: usize = 300;
    // The fixture's pair image is the template row (camera set).
    let f = stacked_library("stacks_many");
    let tpl = f.engine.get_item_by_id(f.jpg).unwrap();
    let items: Vec<_> = (0..PAIRS)
        .flat_map(|i| {
            ["jpg", "arw"].map(|ext| {
                let mut it = tpl.clone();
                it.file_name = format!("MANY_{i:04}.{ext}");
                it.path = f.dir.join(&it.file_name);
                if ext == "arw" {
                    it.media_type = lumina_core::MediaType::Raw;
                }
                // Far from the fixture's own stacks, 10 s apart.
                it.timestamp_epoch_ms += 3_600_000 + i as i64 * 10_000;
                it
            })
        })
        .collect();
    f.engine.catalog().insert_or_update_batch(&items).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, f.engine.clone());
    open_similar_shots(vcx);
    let total = view.update(vcx, |app, _| match &app.stacks.list {
        StacksList::Loaded(s) => s.len(),
        _ => 0,
    });
    assert_eq!(total, PAIRS + 2, "every pair and the fixture's stacks");

    let rendered = |vcx: &mut VisualTestContext| {
        (0..total)
            .filter(|i| vcx.debug_bounds(leak(format!("stack-{i}"))).is_some())
            .count()
    };
    let shown = rendered(vcx);
    assert!(
        (1..=20).contains(&shown),
        "only the visible rows are built, not all {total} ({shown} were)"
    );
    let last = total - 1;
    assert!(vcx.debug_bounds(leak(format!("stack-{last}"))).is_none());

    focus_app(vcx, &view);
    keystrokes(vcx, &vec!["down"; last].join(" "));
    render_ui(vcx);
    assert_eq!(view.update(vcx, |app, _| app.stacks.focused), last);
    let bounds = vcx
        .debug_bounds(leak(format!("stack-{last}")))
        .expect("↓ to the last stack scrolls it into view and renders it");
    let window = vcx.update(|window, _| window.viewport_size());
    assert!(
        bounds.top() >= gpui::px(0.0) && bounds.bottom() <= window.height,
        "the focused stack is inside the window"
    );
    assert!(vcx
        .debug_bounds(leak(format!("stack-focus-{last}")))
        .is_some());
    assert!(rendered(vcx) <= 20, "scrolling keeps the rows bounded");

    drop(view);
}
