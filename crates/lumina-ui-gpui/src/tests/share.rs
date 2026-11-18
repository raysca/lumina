//! Share sheet: palette, context menu and batch dock hand the
//! selected originals to the system share picker. Tests record the request
//! instead of opening the real picker; that is checked by a manual run.

use std::path::PathBuf;

use crate::app::command_palette::CommandId;
use crate::share::{ShareRequest as Request, SHARE_REQUESTS};
use crate::tests::culling::{card, ids};
use crate::tests::helpers::{
    app_with_library, bounds_center, click, keystrokes, render_ui, right_click, seed_library,
    type_text,
};
use crate::*;

/// Lets the background path check finish, then takes what was requested.
fn take_requests(vcx: &mut VisualTestContext) -> Vec<Request> {
    vcx.run_until_parked();
    SHARE_REQUESTS.with(|r| std::mem::take(&mut *r.borrow_mut()))
}

fn paths_of(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> Vec<PathBuf> {
    view.update(vcx, |app, _| {
        app.get_selected_items()
            .into_iter()
            .map(|item| item.path)
            .collect()
    })
}

fn palette_offers_share(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> bool {
    view.update(vcx, |app, _| {
        app.command_query = "share".to_string();
        let offered = app
            .palette_rows()
            .iter()
            .any(|row| row.selector == "command-item-command.share");
        app.command_query.clear();
        offered
    })
}

#[core::prelude::v1::test]
fn share_from_palette_requests_selected_paths() {
    let (_dir, engine) = seed_library("share_palette", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| app.select_all(cx));
    let expected = paths_of(&view, vcx);
    assert_eq!(expected.len(), 3);
    take_requests(vcx);

    // Keyboard only: ⌘K, type, ↵.
    keystrokes(vcx, "secondary-k");
    type_text(vcx, "share");
    keystrokes(vcx, "enter");
    assert_eq!(
        take_requests(vcx),
        vec![(expected, None)],
        "⌘K Share… asks for one picker with every selected original, at the window centre"
    );
    view.update(vcx, |app, _| {
        assert!(!app.command_palette_open, "the palette closes")
    });
}

#[core::prelude::v1::test]
fn share_hidden_for_missing_files() {
    let (_dir, engine) = seed_library("share_missing", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| app.select_all(cx));
    let all = paths_of(&view, vcx);
    std::fs::remove_file(&all[1]).unwrap();
    take_requests(vcx);

    view.update(vcx, |app, cx| app.execute_command(CommandId::Share, cx));
    assert_eq!(
        take_requests(vcx),
        vec![(vec![all[0].clone(), all[2].clone()], None)],
        "a missing original is not offered to the picker"
    );

    // Only the missing one selected: a toast, not an empty picker.
    let order = ids(&view, vcx);
    click(vcx, card(order[1]));
    assert_eq!(paths_of(&view, vcx), vec![all[1].clone()]);
    view.update(vcx, |app, cx| app.execute_command(CommandId::Share, cx));
    assert!(
        take_requests(vcx).is_empty(),
        "no picker for nothing to share"
    );
    view.update(vcx, |app, _| {
        assert_eq!(
            app.toast.current().map(|t| t.message.clone()),
            Some(crate::i18n::t("share.missing_one")),
            "an all-missing selection says why nothing opened"
        )
    });
}

#[core::prelude::v1::test]
fn share_from_context_menu_anchors_at_the_card() {
    let (_dir, engine) = seed_library("share_context", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let order = ids(&view, vcx);
    let target = card(order[2]);
    let at = bounds_center(vcx, target);
    take_requests(vcx);

    right_click(vcx, target);
    click(vcx, "share-menu-btn");
    let path = engine.get_item_by_id(order[2]).unwrap().path;
    assert_eq!(
        take_requests(vcx),
        vec![(vec![path], Some(at))],
        "the context menu shares the right-clicked card, anchored where it was clicked"
    );
    view.update(vcx, |app, _| {
        assert!(app.context_menu_state.is_none(), "the menu closes")
    });
}

#[core::prelude::v1::test]
fn share_from_batch_dock_shares_the_selection() {
    let (_dir, engine) = seed_library("share_dock", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| app.select_all(cx));
    render_ui(vcx);
    let expected = paths_of(&view, vcx);
    let at = bounds_center(vcx, "batch-share-dock-btn");
    take_requests(vcx);

    click(vcx, "batch-share-dock-btn");
    assert_eq!(
        take_requests(vcx),
        vec![(expected, Some(at))],
        "the dock's Share… sends the selection, anchored at the button"
    );
}

#[core::prelude::v1::test]
fn share_unavailable_during_a_modal_or_presentation() {
    let (_dir, engine) = seed_library("share_guard", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| app.select_all(cx));
    assert!(
        palette_offers_share(&view, vcx),
        "with a selection the palette offers Share…"
    );
    take_requests(vcx);

    // The Delete dialog is up: nothing behind it may be shared.
    view.update(vcx, |app, cx| app.prompt_delete_selected(cx));
    render_ui(vcx);
    view.update(vcx, |app, cx| {
        assert!(app.delete_confirm_state.is_some());
        app.share_selection(None, cx);
        app.delete_confirm_state = None;
    });

    // Compare shows: the selection is hidden under it.
    view.update(vcx, |app, cx| {
        app.select_all(cx);
        app.open_compare(cx);
    });
    render_ui(vcx);
    view.update(vcx, |app, cx| {
        assert!(app.compare.is_some(), "compare opened");
        app.share_selection(None, cx);
        app.compare = None;
    });

    // Similar shots holds no selection, so the palette hides Share….
    view.update(vcx, |app, cx| {
        app.execute_command(CommandId::ShowSimilarShots, cx)
    });
    render_ui(vcx);
    view.update(vcx, |app, _| assert!(app.similar_shots_shown()));
    assert!(
        !palette_offers_share(&view, vcx),
        "Similar shots offers no Share…"
    );
    view.update(vcx, |app, cx| app.execute_command(CommandId::Share, cx));
    // Even with a selection forced onto the tab, share_selection refuses.
    view.update(vcx, |app, cx| {
        let engine = app.grid_state.as_ref().unwrap().engine.clone();
        app.selected_item = engine.get_item_slice(0..1).into_iter().next();
        app.selected_indices.insert(0);
        app.selected_index = Some(0);
        assert!(
            !app.get_selected_ids().is_empty(),
            "the forced selection holds"
        );
        app.share_selection(None, cx);
        app.selected_item = None;
        app.selected_indices.clear();
        app.selected_index = None;
    });

    assert!(
        take_requests(vcx).is_empty(),
        "share never runs under a dialog, compare or on Similar shots"
    );
}

#[core::prelude::v1::test]
fn share_refused_while_the_slideshow_plays() {
    let (_dir, engine) = seed_library("share_slideshow", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let order = ids(&view, vcx);
    click(vcx, card(order[0]));
    keystrokes(vcx, "alt-secondary-s");
    take_requests(vcx);
    view.update(vcx, |app, cx| {
        assert!(app.slideshow.is_some(), "⌥⌘S starts the slideshow");
        app.share_selection(None, cx);
    });
    assert!(
        take_requests(vcx).is_empty(),
        "share never runs during a slideshow"
    );
}

#[core::prelude::v1::test]
fn share_refused_in_edit_mode() {
    use crate::tests::quick_edit::{edit_library, open_viewer_on};
    let (_dir, engine, photos, _) = edit_library("share_edit");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_viewer_on(&view, vcx, &engine, photos[0]);
    keystrokes(vcx, "e");
    take_requests(vcx);
    view.update(vcx, |app, cx| {
        assert!(app.edits.session.is_some(), "E opens edit mode");
        app.share_selection(None, cx);
    });
    assert!(
        take_requests(vcx).is_empty(),
        "share never sends the original from under an edit draft"
    );
}

#[core::prelude::v1::test]
fn share_over_the_cap_says_so_and_opens_nothing() {
    let max = crate::share::SHARE_MAX;
    let (_dir, engine) = seed_library("share_cap", max + 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| app.select_all(cx));
    take_requests(vcx);
    view.update(vcx, |app, cx| app.execute_command(CommandId::Share, cx));
    assert!(
        take_requests(vcx).is_empty(),
        "more than {max} items never reach the picker"
    );
    view.update(vcx, |app, _| {
        assert_eq!(
            app.toast.current().map(|t| t.message.clone()),
            Some(crate::i18n::t_with("share.too_many", &[&max.to_string()])),
            "the toast names the limit"
        )
    });
}

#[core::prelude::v1::test]
fn a_dialog_opened_during_the_check_cancels_the_share() {
    let (_dir, engine) = seed_library("share_race", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| app.select_all(cx));
    take_requests(vcx);
    // The file check runs off the UI thread; the Delete dialog opens first.
    view.update(vcx, |app, cx| {
        app.share_selection(None, cx);
        app.prompt_delete_selected(cx);
        assert!(app.delete_confirm_state.is_some());
    });
    assert!(
        take_requests(vcx).is_empty(),
        "a picker never opens over a dialog that appeared while files were checked"
    );
}

#[core::prelude::v1::test]
fn file_menu_share_shares_the_selection() {
    let (_dir, engine) = seed_library("share_menu", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| app.select_all(cx));
    let expected = paths_of(&view, vcx);
    take_requests(vcx);
    vcx.dispatch_action(crate::actions::ShareSelection);
    assert_eq!(
        take_requests(vcx),
        vec![(expected, None)],
        "File > Share… shares the selection at the window centre"
    );
}

#[core::prelude::v1::test]
fn missing_toast_counts_the_files() {
    let (_dir, engine) = seed_library("share_missing_many", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| app.select_all(cx));
    for path in paths_of(&view, vcx) {
        std::fs::remove_file(path).unwrap();
    }
    view.update(vcx, |app, cx| app.execute_command(CommandId::Share, cx));
    assert!(take_requests(vcx).is_empty());
    view.update(vcx, |app, _| {
        assert_eq!(
            app.toast.current().map(|t| t.message.clone()),
            Some(crate::i18n::t_with("share.missing_many", &["2"])),
            "two missing files are counted, in the plural"
        )
    });
}

#[core::prelude::v1::test]
fn a_stale_check_after_a_library_or_selection_change_is_dropped() {
    let (_dir, engine) = seed_library("share_stale", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let order = ids(&view, vcx);
    view.update(vcx, |app, cx| app.select_all(cx));
    take_requests(vcx);

    // Another library opened while the files were checked.
    view.update(vcx, |app, cx| {
        app.share_selection(None, cx);
        app.library_generation += 1;
    });
    assert!(
        take_requests(vcx).is_empty(),
        "a check from the previous library never opens a picker"
    );

    // The selection changed while the files were checked.
    view.update(vcx, |app, cx| {
        app.share_selection(None, cx);
        app.select_index(0, cx);
        assert_eq!(app.get_selected_ids(), vec![order[0]], "one item now");
    });
    assert!(
        take_requests(vcx).is_empty(),
        "a check for the old selection never shares the new one"
    );
}
