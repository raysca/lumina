//! Compare / survey mode: C with 2-9 selected items, focus keys,
//! flags and ratings on the focused pane by id, ⌫ drops a pane (never media),
//! shared zoom, reduced decode size for survey, texture eviction on close,
//! ↵ to the viewer and back, and the keys compare absorbs.

use std::sync::Arc;

use crate::app::present::COMPARE_DECODE_SIZES;
use crate::tests::culling::{card, flags, ids, rating};
use crate::tests::helpers::{
    app_with_library, click, keystrokes, leak, render_ui, seed_library, seed_library_undecodable,
};
use crate::*;
use lumina_core::{Flag, MediaId};

fn pane(id: MediaId) -> &'static str {
    leak(format!("compare-pane-{}", id.0))
}

fn media(id: MediaId) -> &'static str {
    leak(format!("compare-media-{}", id.0))
}

fn texture() -> Arc<gpui::RenderImage> {
    Arc::new(gpui::RenderImage::new(vec![image::Frame::new(
        image::RgbaImage::new(4, 4),
    )]))
}

/// Selects the first `n` cards: a click, then ⇧→.
fn select_first(vcx: &mut VisualTestContext, ids: &[MediaId], n: usize) {
    click(vcx, card(ids[0]));
    for _ in 1..n {
        keystrokes(vcx, "shift-right");
    }
}

/// Lands a decoded image for `id` as the decode callback would.
fn land(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, id: MediaId) -> Arc<RenderImage> {
    let image = texture();
    let copy = image.clone();
    view.update(vcx, |app, cx| {
        let generation = app.compare.as_ref().expect("compare is open").generation;
        app.land_compare_image(generation, id, copy, cx);
    });
    render_ui(vcx);
    image
}

fn toast(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> Option<String> {
    view.update(vcx, |app, _| app.toast.current().map(|t| t.message.clone()))
}

fn focused(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> Option<MediaId> {
    view.update(vcx, |app, _| {
        app.compare.as_ref().and_then(|c| c.focused_id())
    })
}

#[core::prelude::v1::test]
fn c_opens_compare_with_selection_and_esc_closes() {
    let (_dir, engine) = seed_library("compare_open", 4);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let ids = ids(&view, vcx);
    select_first(vcx, &ids, 2);

    keystrokes(vcx, "c");
    assert!(
        vcx.debug_bounds("compare-view").is_some(),
        "C with two selected opens compare"
    );
    for id in &ids[..2] {
        assert!(vcx.debug_bounds(pane(*id)).is_some(), "a pane per item");
    }
    assert!(
        vcx.debug_bounds(pane(ids[2])).is_none(),
        "only the selection"
    );
    assert!(vcx.debug_bounds("compare-pane-focused").is_some());
    assert_eq!(
        focused(&view, vcx),
        Some(ids[0]),
        "the first item has focus"
    );

    keystrokes(vcx, "escape");
    assert!(
        vcx.debug_bounds("compare-view").is_none(),
        "Esc closes compare"
    );
    view.update(vcx, |app, _| {
        assert!(app.compare.is_none());
        assert_eq!(app.selected_indices.len(), 2, "the grid selection stays");
    });

    // View > Compare (the menu action) opens it too.
    vcx.dispatch_action(crate::actions::Compare);
    render_ui(vcx);
    assert!(vcx.debug_bounds("compare-view").is_some(), "View > Compare");
    keystrokes(vcx, "escape");
}

#[core::prelude::v1::test]
fn compare_flag_acts_on_focused_pane() {
    let (_dir, engine) = seed_library("compare_flag", 4);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let ids = ids(&view, vcx);
    select_first(vcx, &ids, 3);
    let before = view.update(vcx, |app, _| {
        (app.selected_indices.clone(), app.selected_index)
    });

    keystrokes(vcx, "c");
    keystrokes(vcx, "right");
    assert_eq!(focused(&view, vcx), Some(ids[1]), "→ moves focus");
    keystrokes(vcx, "p");
    assert_eq!(
        flags(&engine),
        [(ids[1], Flag::Pick)].into_iter().collect(),
        "P picks the focused pane only, not the selection"
    );
    keystrokes(vcx, "3");
    assert_eq!(rating(&engine, ids[1]), 3, "3 rates the focused pane");
    assert_eq!(rating(&engine, ids[0]), 0, "other panes keep their rating");

    keystrokes(vcx, "secondary-z");
    assert_eq!(rating(&engine, ids[1]), 0, "⌘Z passes through compare");
    keystrokes(vcx, "shift-secondary-z");
    assert_eq!(rating(&engine, ids[1]), 3, "⇧⌘Z passes through compare");

    keystrokes(vcx, "tab");
    assert_eq!(focused(&view, vcx), Some(ids[2]), "Tab moves focus");
    keystrokes(vcx, "x");
    assert_eq!(flags(&engine).get(&ids[2]), Some(&Flag::Reject));
    keystrokes(vcx, "u");
    assert_eq!(flags(&engine).get(&ids[2]), None, "U unflags the pane");
    keystrokes(vcx, "left");
    keystrokes(vcx, "left");
    assert_eq!(focused(&view, vcx), Some(ids[0]));
    assert!(!flags(&engine).contains_key(&ids[0]));
    view.update(vcx, |app, _| {
        assert_eq!(
            (app.selected_indices.clone(), app.selected_index),
            before,
            "compare keys never move the grid selection"
        );
        assert!(app.compare.is_some(), "still comparing");
    });
}

#[core::prelude::v1::test]
fn compare_backspace_removes_pane_not_media() {
    let (_dir, engine) = seed_library("compare_drop", 4);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let ids = ids(&view, vcx);
    select_first(vcx, &ids, 3);
    keystrokes(vcx, "c");
    let dropped = land(&view, vcx, ids[0]);

    keystrokes(vcx, "backspace");
    assert!(vcx.debug_bounds(pane(ids[0])).is_none(), "⌫ drops the pane");
    assert!(vcx.debug_bounds(pane(ids[1])).is_some());
    assert!(vcx.debug_bounds(pane(ids[2])).is_some());
    assert_eq!(focused(&view, vcx), Some(ids[1]), "focus moves to the next");
    view.update(vcx, |app, _| {
        assert!(app.delete_confirm_state.is_none(), "no Delete dialog");
        assert_eq!(app.get_active_count(), 4, "no media left the library");
    });
    assert!(
        engine
            .catalog()
            .get_item_by_id(ids[0])
            .unwrap()
            .is_some_and(|i| !i.is_trashed),
        "the dropped item is neither trashed nor deleted"
    );
    assert_eq!(
        Arc::strong_count(&dropped),
        1,
        "the dropped pane's texture was evicted (no holder left)"
    );

    keystrokes(vcx, "backspace");
    keystrokes(vcx, "backspace");
    view.update(vcx, |app, _| {
        assert!(
            app.compare.is_none(),
            "dropping the last pane closes compare"
        );
        assert_eq!(app.get_active_count(), 4);
    });
}

#[core::prelude::v1::test]
fn two_up_zoom_is_synchronized() {
    let (_dir, engine) = seed_library("compare_zoom", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let ids = ids(&view, vcx);
    select_first(vcx, &ids, 2);
    keystrokes(vcx, "c");
    land(&view, vcx, ids[0]);
    land(&view, vcx, ids[1]);
    let size = |vcx: &mut VisualTestContext, id| vcx.debug_bounds(media(id)).unwrap().size;
    let fit = (size(vcx, ids[0]), size(vcx, ids[1]));

    keystrokes(vcx, "=");
    keystrokes(vcx, "=");
    let (a, b) = (size(vcx, ids[0]), size(vcx, ids[1]));
    assert!(a.width > fit.0.width, "+ zooms the focused pane");
    assert_eq!(a, b, "both panes share one zoom");
    assert!(b.width > fit.1.width, "the other pane zooms with it");

    keystrokes(vcx, "secondary-0");
    assert_eq!(
        (size(vcx, ids[0]), size(vcx, ids[1])),
        fit,
        "⌘0 fits both panes"
    );
    keystrokes(vcx, "z");
    assert_eq!(size(vcx, ids[0]), size(vcx, ids[1]), "Z zooms both");
    assert!(size(vcx, ids[0]).width > fit.0.width);
}

#[core::prelude::v1::test]
fn survey_uses_reduced_decode_size() {
    let (_dir, engine) = seed_library("compare_survey", 5);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let ids = ids(&view, vcx);

    COMPARE_DECODE_SIZES.with(|s| s.borrow_mut().clear());
    select_first(vcx, &ids, 4);
    keystrokes(vcx, "c");
    assert_eq!(
        COMPARE_DECODE_SIZES.with(|s| s.borrow().clone()),
        vec![1024; 4],
        "survey (more than two) decodes at 1024 px, never 2048"
    );
    assert!(
        vcx.debug_bounds(pane(ids[3])).is_some(),
        "2×2 shows all four"
    );
    keystrokes(vcx, "escape");

    COMPARE_DECODE_SIZES.with(|s| s.borrow_mut().clear());
    select_first(vcx, &ids, 2);
    keystrokes(vcx, "c");
    assert_eq!(
        COMPARE_DECODE_SIZES.with(|s| s.borrow().clone()),
        vec![2048; 2],
        "two-up decodes at the viewer size"
    );
}

#[core::prelude::v1::test]
fn compare_needs_two_items() {
    let (_dir, engine) = seed_library("compare_count", 10);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let ids = ids(&view, vcx);
    COMPARE_DECODE_SIZES.with(|s| s.borrow_mut().clear());

    click(vcx, card(ids[0]));
    keystrokes(vcx, "c");
    assert!(
        view.update(vcx, |app, _| app.compare.is_none()),
        "1 item: no mode"
    );
    assert_eq!(
        toast(&view, vcx).as_deref(),
        Some("Select 2 to 9 items to compare")
    );

    keystrokes(vcx, "secondary-a");
    keystrokes(vcx, "c");
    assert!(
        view.update(vcx, |app, _| app.compare.is_none()),
        "10 items: no mode"
    );
    assert_eq!(toast(&view, vcx).as_deref(), Some("Compare up to 9 items"));
    assert!(
        COMPARE_DECODE_SIZES.with(|s| s.borrow().is_empty()),
        "nothing is decoded when compare does not open"
    );
}

#[core::prelude::v1::test]
fn compare_close_drops_textures() {
    let (_dir, engine) = seed_library_undecodable("compare_textures", 4);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let ids = ids(&view, vcx);
    select_first(vcx, &ids, 3);
    keystrokes(vcx, "c");
    let generation = view.update(vcx, |app, _| app.compare.as_ref().unwrap().generation);
    let images: Vec<_> = ids[..3].iter().map(|id| land(&view, vcx, *id)).collect();
    assert!(
        vcx.debug_bounds(media(ids[0])).is_some(),
        "landed images paint"
    );

    // Close without rendering: the textures wait in the eviction queue.
    view.update(vcx, |app, cx| {
        app.close_compare(cx);
        for image in &images {
            assert!(
                app.retired_images.iter().any(|r| r.id == image.id),
                "closing compare queues every texture for cx.drop_image"
            );
        }
    });
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.retired_images.is_empty(), "the render evicted them");
    });
    for image in &images {
        assert_eq!(Arc::strong_count(image), 1, "nothing holds a texture");
    }

    // A decode that lands after compare closed is evicted, never kept.
    let late = texture();
    view.update(vcx, |app, cx| {
        app.land_compare_image(generation, ids[0], late.clone(), cx);
        assert!(app.retired_images.iter().any(|r| r.id == late.id));
    });
    render_ui(vcx);
    // Nor one from an earlier session that lands while a new compare of
    // the same item is open.
    keystrokes(vcx, "c");
    let stale = texture();
    view.update(vcx, |app, cx| {
        assert!(app.compare.as_ref().unwrap().generation != generation);
        app.land_compare_image(generation, ids[0], stale.clone(), cx);
        assert!(
            app.retired_images.iter().any(|r| r.id == stale.id),
            "an earlier session's decode is evicted"
        );
        assert!(
            app.compare.as_ref().unwrap().textures.is_empty(),
            "and never shown in the new compare"
        );
    });
}

#[core::prelude::v1::test]
fn compare_enter_then_esc_returns_to_compare() {
    let (_dir, engine) = seed_library("compare_enter", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let ids = ids(&view, vcx);
    select_first(vcx, &ids, 2);
    keystrokes(vcx, "c");
    keystrokes(vcx, "right");

    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert!(app.is_fullscreen_viewer, "↵ opens the focused item");
        assert_eq!(app.selected_item.as_ref().map(|i| i.id), Some(ids[1]));
    });
    assert!(vcx.debug_bounds("viewer-viewport").is_some());

    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| assert!(!app.is_fullscreen_viewer));
    assert!(
        vcx.debug_bounds("compare-view").is_some(),
        "Esc in the viewer returns to compare, not the grid"
    );
    assert_eq!(focused(&view, vcx), Some(ids[1]), "focus is kept");
    keystrokes(vcx, "escape");
    assert!(vcx.debug_bounds("compare-view").is_none());
}

#[core::prelude::v1::test]
fn compare_absorbs_cmd_a_and_delete_dialog() {
    let (_dir, engine) = seed_library("compare_absorb", 4);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let ids = ids(&view, vcx);
    // A reject in the view, so Delete Rejected would have something to open.
    engine.set_flags(&[(ids[3], Flag::Reject)]).unwrap();
    select_first(vcx, &ids, 2);
    let inspector = view.update(vcx, |app, _| app.inspector_open);
    keystrokes(vcx, "c");

    for keys in [
        "secondary-a",
        "secondary-backspace",
        "f",
        "secondary-k",
        "i",
        "r",
        "shift-r",
        "e",
    ] {
        keystrokes(vcx, keys);
        view.update(vcx, |app, _| {
            assert_eq!(app.inspector_open, inspector, "{keys}: inspector unchanged");
            assert!(app.edits.session.is_none(), "{keys}: no edit mode");
            assert_eq!(app.selected_indices.len(), 2, "{keys}: selection unchanged");
            assert!(
                app.delete_confirm_state.is_none(),
                "{keys}: no Delete dialog"
            );
            assert!(!app.command_palette_open, "{keys}: no palette");
            assert!(app.compare.is_some(), "{keys}: compare stays open");
        });
    }
    // Menu actions that would open the Delete dialog or act on the grid
    // selection are disabled while comparing.
    vcx.dispatch_action(crate::actions::DeleteRejected);
    vcx.dispatch_action(crate::actions::AddToAlbum);
    vcx.dispatch_action(crate::actions::RotateLeft);
    vcx.dispatch_action(crate::actions::EditPhoto);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(
            app.delete_confirm_state.is_none(),
            "no Delete Rejected dialog"
        );
        assert!(app.albums.picker.is_none());
        assert!(app.edits.session.is_none(), "Edit Photo is disabled");
    });
    let rotations = engine.catalog().get_rotations(&ids[..2]).unwrap();
    assert!(
        rotations.values().all(|q| *q == 0),
        "R, ⇧R and Rotate Left did not rotate through compare: {rotations:?}"
    );
    assert!(
        !engine
            .catalog()
            .get_item_by_id(ids[0])
            .unwrap()
            .unwrap()
            .is_favorite,
        "F did not favorite through compare"
    );
}

#[core::prelude::v1::test]
fn compare_enter_makes_viewer_act_on_focused_item_only() {
    let (_dir, engine) = seed_library("compare_enter_keys", 5);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let ids = ids(&view, vcx);
    select_first(vcx, &ids, 3);
    keystrokes(vcx, "c");
    keystrokes(vcx, "right");
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.get_selected_ids(),
            vec![ids[1]],
            "the viewer opened from compare selects only the focused item"
        );
        assert_eq!(app.selected_index, Some(1), "at its real grid position");
    });

    keystrokes(vcx, "p");
    assert_eq!(
        flags(&engine),
        [(ids[1], Flag::Pick)].into_iter().collect(),
        "P in the viewer flags only the shown item"
    );
    let shown = |view: &Entity<LuminaApp>, vcx: &mut VisualTestContext| {
        view.update(vcx, |app, _| app.selected_item.as_ref().map(|i| i.id))
    };
    assert_eq!(
        shown(&view, vcx),
        Some(ids[2]),
        "P advances to the real next item"
    );
    keystrokes(vcx, "right");
    assert_eq!(
        shown(&view, vcx),
        Some(ids[3]),
        "→ steps to the real neighbour"
    );
    keystrokes(vcx, "escape");
    assert!(
        vcx.debug_bounds("compare-view").is_some(),
        "back in compare"
    );
}

#[core::prelude::v1::test]
fn compare_write_in_show_rejected_keeps_grid_selection_by_id() {
    let mut cx = crate::tests::TestApp::single();
    let (_dir, engine, view, vcx, r) =
        crate::tests::culling_filtered::rejects_view(&mut cx, "compare_rejected", 5);
    click(vcx, card(r[1]));
    keystrokes(vcx, "shift-right");
    keystrokes(vcx, "c");
    keystrokes(vcx, "u");
    assert_eq!(
        flags(&engine).get(&r[1]),
        None,
        "U unflags the focused pane"
    );
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.get_selected_ids(),
            vec![r[2]],
            "after compare the grid selection is the chosen items still in view, by id"
        );
    });

    // Both chosen items leave the view: the selection is cleared, never
    // moved onto rows the user did not choose.
    keystrokes(vcx, "shift-right");
    keystrokes(vcx, "c");
    keystrokes(vcx, "u");
    keystrokes(vcx, "right");
    keystrokes(vcx, "u");
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        let left = app.get_selected_ids();
        assert!(
            left.is_empty(),
            "no unchosen row is selected after compare: {left:?}"
        );
    });
}

#[core::prelude::v1::test]
fn compare_failed_flag_shows_banner_in_compare() {
    let (_dir, engine) = seed_library("compare_banner", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let ids = ids(&view, vcx);
    select_first(vcx, &ids, 2);
    keystrokes(vcx, "c");
    crate::app::culling::FLAG_WRITE_FAILS.with(|f| f.set(true));
    keystrokes(vcx, "p");
    crate::app::culling::FLAG_WRITE_FAILS.with(|f| f.set(false));
    assert!(
        flags(&engine).is_empty(),
        "the failed write changed nothing"
    );
    assert!(
        vcx.debug_bounds("compare-banner").is_some(),
        "the error banner shows inside compare"
    );
    keystrokes(vcx, "escape");
    assert!(view.update(vcx, |app, _| app.error_message.is_none()));
    assert!(
        vcx.debug_bounds("compare-view").is_some(),
        "Esc took the banner first"
    );

    keystrokes(vcx, "p");
    assert!(
        vcx.debug_bounds("compare-toast").is_some(),
        "the undo toast shows inside compare"
    );
}

#[core::prelude::v1::test]
fn compare_undo_passes_only_flag_and_rating() {
    let (_dir, engine) = seed_library("compare_undo", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let ids = ids(&view, vcx);
    select_first(vcx, &ids, 2);
    keystrokes(vcx, "f");
    let favorite = |id| {
        engine
            .catalog()
            .get_item_by_id(id)
            .unwrap()
            .unwrap()
            .is_favorite
    };
    assert!(favorite(ids[0]), "test premise: F favorited the selection");
    keystrokes(vcx, "c");

    keystrokes(vcx, "secondary-z");
    assert!(favorite(ids[0]), "⌘Z does not undo a favorite from compare");
    keystrokes(vcx, "p");
    keystrokes(vcx, "secondary-z");
    assert!(flags(&engine).is_empty(), "⌘Z undoes the compare flag");
    keystrokes(vcx, "shift-secondary-z");
    assert_eq!(
        flags(&engine).get(&ids[0]),
        Some(&Flag::Pick),
        "⇧⌘Z redoes it"
    );
    keystrokes(vcx, "secondary-z");
    keystrokes(vcx, "secondary-z");
    assert!(
        favorite(ids[0]),
        "older history is not replayed from compare"
    );
    assert!(favorite(ids[1]));
}

#[core::prelude::v1::test]
fn compare_drops_pane_whose_item_was_trashed() {
    let (_dir, engine) = seed_library("compare_stale", 4);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let ids = ids(&view, vcx);
    select_first(vcx, &ids, 3);
    keystrokes(vcx, "c");
    engine.catalog().set_trashed(ids[0], true).unwrap();

    keystrokes(vcx, "p");
    assert!(flags(&engine).is_empty(), "no write to a trashed item");
    assert!(
        vcx.debug_bounds(pane(ids[0])).is_none(),
        "its pane is dropped"
    );
    assert_eq!(focused(&view, vcx), Some(ids[1]));
    keystrokes(vcx, "p");
    assert_eq!(flags(&engine).get(&ids[1]), Some(&Flag::Pick));
}

#[core::prelude::v1::test]
fn compare_enter_after_viewer_round_trip_selects_real_position() {
    let (_dir, engine) = seed_library("compare_round_trip", 8);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let ids = ids(&view, vcx);
    select_first(vcx, &ids, 3);
    keystrokes(vcx, "c");
    keystrokes(vcx, "enter");
    for _ in 0..4 {
        keystrokes(vcx, "right");
    }
    keystrokes(vcx, "escape");
    assert!(
        vcx.debug_bounds("compare-view").is_some(),
        "back in compare"
    );
    keystrokes(vcx, "tab");
    keystrokes(vcx, "tab");
    assert_eq!(focused(&view, vcx), Some(ids[2]));
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_index,
            Some(2),
            "a second ↵ selects the pane's real grid position"
        );
        assert_eq!(app.get_selected_ids(), vec![ids[2]]);
    });
    keystrokes(vcx, "p");
    assert_eq!(flags(&engine).get(&ids[2]), Some(&Flag::Pick));
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_item.as_ref().map(|i| i.id),
            Some(ids[3]),
            "P still auto-advances to the real next item"
        );
    });
}

/// `n` picked items shown through Show Picks (plus one hidden unflagged
/// item), in view order.
fn picks_view<'a>(
    cx: &'a mut gpui::TestAppContext,
    prefix: &str,
    n: usize,
) -> (
    crate::tests::helpers::TempLibrary,
    Arc<lumina_core::LuminaEngine>,
    Entity<LuminaApp>,
    &'a mut VisualTestContext,
    Vec<MediaId>,
) {
    let (dir, engine) = seed_library(prefix, n + 1);
    let (view, vcx) = app_with_library(cx, engine.clone());
    let all = ids(&view, vcx);
    let picks: Vec<(MediaId, Flag)> = all[..n].iter().map(|id| (*id, Flag::Pick)).collect();
    engine.set_flags(&picks).unwrap();
    view.update(vcx, |app, cx| {
        app.refresh_album_counts();
        app.toggle_quick_filter("is:pick", cx);
    });
    render_ui(vcx);
    let shown = ids(&view, vcx);
    assert_eq!(shown.len(), n, "test premise: only the picks are shown");
    (dir, engine, view, vcx, shown)
}

fn position_of(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, id: MediaId) -> usize {
    ids(view, vcx)
        .iter()
        .position(|x| *x == id)
        .expect("in view")
}

#[core::prelude::v1::test]
fn compare_enter_after_viewer_undo_then_write_selects_real_position() {
    let mut cx = crate::tests::TestApp::single();
    let (_dir, _engine, view, vcx, p) = picks_view(&mut cx, "compare_undo_write", 6);
    // An earlier step the viewer will undo: p0 leaves Show Picks.
    click(vcx, card(p[0]));
    keystrokes(vcx, "x");
    click(vcx, card(p[2]));
    keystrokes(vcx, "shift-right");
    keystrokes(vcx, "c");
    keystrokes(vcx, "enter");
    // ⌘Z brings p0 back (every row moves down one), then X on p4 pushes a
    // new step: both stacks end at their old lengths.
    keystrokes(vcx, "secondary-z");
    keystrokes(vcx, "right");
    keystrokes(vcx, "right");
    view.update(vcx, |app, _| {
        assert_eq!(app.selected_item.as_ref().map(|i| i.id), Some(p[4]))
    });
    keystrokes(vcx, "x");
    keystrokes(vcx, "escape");
    keystrokes(vcx, "tab");
    assert_eq!(focused(&view, vcx), Some(p[3]));
    keystrokes(vcx, "enter");
    let real = position_of(&view, vcx, p[3]);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_index,
            Some(real),
            "↵ after an undo and a write in the viewer selects the pane's real position"
        );
    });
}

#[core::prelude::v1::test]
fn compare_enter_after_viewer_writes_on_full_undo_stack_selects_real_position() {
    let mut cx = crate::tests::TestApp::single();
    let (_dir, _engine, view, vcx, p) = picks_view(&mut cx, "compare_full_stack", 6);
    view.update(vcx, |app, _| {
        for _ in 0..crate::app::undo::MAX_UNDO {
            app.push_undo(crate::app::undo::UndoAction::Favorite { changes: vec![] });
        }
    });
    click(vcx, card(p[3]));
    keystrokes(vcx, "shift-right");
    keystrokes(vcx, "c");
    keystrokes(vcx, "enter");
    // Two rows before the compare set leave Show Picks; the full stack
    // keeps its length.
    for _ in 0..3 {
        keystrokes(vcx, "left");
    }
    view.update(vcx, |app, _| {
        assert_eq!(app.selected_item.as_ref().map(|i| i.id), Some(p[0]))
    });
    keystrokes(vcx, "x");
    keystrokes(vcx, "x");
    keystrokes(vcx, "escape");
    keystrokes(vcx, "tab");
    assert_eq!(focused(&view, vcx), Some(p[4]));
    keystrokes(vcx, "enter");
    let real = position_of(&view, vcx, p[4]);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_index,
            Some(real),
            "↵ after writes on a full undo stack selects the pane's real position"
        );
    });
}
