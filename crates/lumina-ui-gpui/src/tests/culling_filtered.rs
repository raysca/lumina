//! Culling in filtered views (Show Rejected, a `rating:` search): a flag
//! or rating can drop the selection from the view, and the next key must
//! still act on the items the user chose (by id, never by position).

use super::culling::{card, flags, ids, rating};
use crate::tests::ui_automation::{
    app_with_library, click, focus_app, keystrokes, render_ui, seed_library,
};
use crate::*;
use lumina_core::{Flag, LuminaEngine, MediaId};

/// A library of `n` items, all rejected, shown through Show Rejected (plus
/// one unflagged item the filter hides). Returns the rejects in view order.
fn rejects_view<'a>(
    cx: &'a mut gpui::TestAppContext,
    prefix: &str,
    n: usize,
) -> (
    std::path::PathBuf,
    std::sync::Arc<LuminaEngine>,
    Entity<LuminaApp>,
    &'a mut VisualTestContext,
    Vec<MediaId>,
) {
    let (dir, engine) = seed_library(prefix, n + 1);
    let (view, vcx) = app_with_library(cx, engine.clone());
    let all = ids(&view, vcx);
    let rejects: Vec<(MediaId, Flag)> = all[..n].iter().map(|id| (*id, Flag::Reject)).collect();
    engine.set_flags(&rejects).unwrap();
    view.update(vcx, |app, cx| {
        app.refresh_album_counts();
        app.toggle_quick_filter("is:reject", cx);
    });
    render_ui(vcx);
    let shown = ids(&view, vcx);
    assert_eq!(shown.len(), n, "test premise: only the rejects are shown");
    (dir, engine, view, vcx, shown)
}

#[core::prelude::v1::test]
fn viewer_unflag_in_show_rejected_lands_on_next_reject() {
    let mut cx = crate::tests::TestApp::single();
    let (dir, engine, view, vcx, r) = rejects_view(&mut cx, "cull_view_rej", 4);
    click(vcx, card(r[0]));
    focus_app(vcx, &view);
    keystrokes(vcx, "space");
    let shown = |view: &Entity<LuminaApp>, vcx: &mut VisualTestContext| {
        view.update(vcx, |app, _| {
            (app.selected_index, app.selected_item.as_ref().map(|i| i.id))
        })
    };

    keystrokes(vcx, "u");
    assert_eq!(flags(&engine).get(&r[0]), None);
    assert_eq!(
        shown(&view, vcx),
        (Some(0), Some(r[1])),
        "U drops the item from Show Rejected and lands on the next reject, skipping none"
    );
    keystrokes(vcx, "u");
    assert_eq!(
        shown(&view, vcx),
        (Some(0), Some(r[2])),
        "again on the next reject"
    );
    // r2 is now second to last: the next reject is the last one.
    keystrokes(vcx, "u");
    assert_eq!(
        shown(&view, vcx),
        (Some(0), Some(r[3])),
        "from the second-to-last item U lands on the last reject at its new index"
    );
    assert_eq!(
        flags(&engine).get(&r[3]),
        Some(&Flag::Reject),
        "the last reject is untouched"
    );
    view.update(vcx, |app, _| assert!(app.is_fullscreen_viewer));

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn grid_unflag_in_show_rejected_never_retargets_the_selection() {
    let mut cx = crate::tests::TestApp::single();
    let (dir, engine, view, vcx, r) = rejects_view(&mut cx, "cull_grid_rej", 4);
    click(vcx, card(r[1]));
    focus_app(vcx, &view);
    keystrokes(vcx, "shift-right");
    keystrokes(vcx, "u");
    let want: std::collections::HashMap<_, _> = [(r[0], Flag::Reject), (r[3], Flag::Reject)]
        .into_iter()
        .collect();
    assert_eq!(
        flags(&engine),
        want,
        "U unflags only the two selected rejects"
    );
    view.update(vcx, |app, _| {
        assert!(
            app.get_selected_ids().is_empty(),
            "items that left the view leave the selection, got {:?}",
            app.get_selected_ids()
        );
    });
    keystrokes(vcx, "x");
    assert_eq!(
        flags(&engine),
        want,
        "X after the selection left the view changes no other item"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn grid_rating_in_rating_search_never_retargets_the_selection() {
    let (dir, engine) = seed_library("cull_grid_rate", 4);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let all = ids(&view, vcx);
    engine
        .set_ratings(&all.iter().map(|id| (*id, 3)).collect::<Vec<_>>())
        .unwrap();
    view.update(vcx, |app, cx| {
        app.refresh_album_counts();
        app.toggle_quick_filter("rating:>=3", cx);
    });
    render_ui(vcx);
    let r = ids(&view, vcx);
    assert_eq!(r.len(), 4);

    click(vcx, card(r[1]));
    focus_app(vcx, &view);
    keystrokes(vcx, "shift-right");
    keystrokes(vcx, "1");
    assert_eq!((rating(&engine, r[1]), rating(&engine, r[2])), (1, 1));
    keystrokes(vcx, "5");
    for id in [r[0], r[3]] {
        assert_eq!(
            rating(&engine, id),
            3,
            "a rating after the selection left the view must not reach another item"
        );
    }
    assert_eq!((rating(&engine, r[1]), rating(&engine, r[2])), (1, 1));

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn rating_while_sorted_by_rating_keeps_the_rated_items_selected() {
    let (dir, engine) = seed_library("cull_sort_rate", 5);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    view.update(vcx, |app, cx| {
        app.set_sort(
            lumina_core::SortField::Rating,
            lumina_core::SortDirection::Descending,
            cx,
        )
    });
    render_ui(vcx);
    let r = ids(&view, vcx);

    // Two items in the middle jump to the top when rated 5.
    click(vcx, card(r[2]));
    focus_app(vcx, &view);
    keystrokes(vcx, "shift-right");
    keystrokes(vcx, "5");
    assert_eq!((rating(&engine, r[2]), rating(&engine, r[3])), (5, 5));
    assert_ne!(
        ids(&view, vcx)[..2].to_vec(),
        vec![r[0], r[1]],
        "test premise: the rated items moved to the top"
    );
    keystrokes(vcx, "3");
    assert_eq!(
        (rating(&engine, r[2]), rating(&engine, r[3])),
        (3, 3),
        "the next key still rates both items the user chose"
    );
    for id in [r[0], r[1], r[4]] {
        assert_eq!(rating(&engine, id), 0, "no other item gets the rating");
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn undo_unflag_in_viewer_keeps_index_on_the_shown_item() {
    let mut cx = crate::tests::TestApp::single();
    let (dir, engine, view, vcx, r) = rejects_view(&mut cx, "cull_undo_rej", 4);
    click(vcx, card(r[0]));
    focus_app(vcx, &view);
    keystrokes(vcx, "space");
    keystrokes(vcx, "u");
    keystrokes(vcx, "secondary-z");
    assert_eq!(
        flags(&engine).get(&r[0]),
        Some(&Flag::Reject),
        "undo rejects r0 again"
    );
    view.update(vcx, |app, _| {
        let shown = app.selected_item.as_ref().map(|i| i.id);
        assert_eq!(shown, Some(r[1]), "undo leaves the shown item alone");
        assert_eq!(
            app.selected_index
                .and_then(|i| app.navigation_item(i))
                .map(|i| i.id),
            shown,
            "selected_index points at the shown item after undo"
        );
    });
    keystrokes(vcx, "]");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_item.as_ref().map(|i| i.id),
            Some(r[2]),
            "] after undo moves to the next reject"
        );
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn viewer_rating_that_swaps_past_the_next_item_still_advances_to_it() {
    let (dir, engine) = seed_library("cull_sort_swap", 4);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let all = ids(&view, vcx);
    engine
        .set_ratings(&[(all[0], 5), (all[1], 4), (all[2], 3), (all[3], 1)])
        .unwrap();
    view.update(vcx, |app, cx| {
        app.refresh_album_counts();
        app.set_sort(
            lumina_core::SortField::Rating,
            lumina_core::SortDirection::Descending,
            cx,
        )
    });
    render_ui(vcx);
    let r = ids(&view, vcx);
    assert_eq!(r, all, "test premise: sorted 5, 4, 3, 1");

    click(vcx, card(r[1]));
    focus_app(vcx, &view);
    keystrokes(vcx, "space");
    // 2 sorts r1 below r2: r1 moves to row 2 and the next item, r2, to row 1.
    keystrokes(vcx, "2");
    assert_eq!(rating(&engine, r[1]), 2);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_item.as_ref().map(|i| i.id),
            Some(r[2]),
            "the viewer lands on the item that was next, wherever it moved"
        );
        assert_eq!(app.selected_index, Some(1), "at its new row");
    });

    let _ = std::fs::remove_dir_all(&dir);
}
