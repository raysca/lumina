//! A large selection (⌘A, a long Shift range) is summarised off the UI
//! thread: the batch inspector shows the count at once and the rest when
//! the background read lands, and a newer selection drops a stale result.

use super::culling::{card, ids};
use super::ui_automation::{
    app_with_library, click, keystrokes, render_ui, right_click, seed_library,
};
use crate::app::batch_summary::BATCH_SYNC_LIMIT;
use lumina_core::MediaType;

/// More rows than the batch summary reads on the UI thread.
const LARGE: usize = BATCH_SYNC_LIMIT + 20;

fn rows_read() -> usize {
    crate::app::active_view::ROWS_READ.with(std::cell::Cell::get)
}

/// ⌘A on a large view reads a bounded number of rows on the UI thread.
#[core::prelude::v1::test]
fn select_all_on_a_large_view_reads_bounded_rows() {
    let (dir, engine) = seed_library("large_sel_reads", LARGE);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let ids = ids(&view, vcx);
    click(vcx, card(ids[0]));

    let before = rows_read();
    keystrokes(vcx, "secondary-a");
    let read = rows_read() - before;
    view.update(vcx, |app, _| {
        assert_eq!(app.selected_indices.len(), LARGE, "⌘A selects the view")
    });
    assert!(
        read < 64,
        "⌘A must not read every selected row on the UI thread: {read} rows \
         read for {LARGE} selected"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Once the background read lands, the summary, the shared or mixed fields
/// and the tag counts describe the whole selection; until then nothing
/// stale (a shared title of the last selection) shows.
#[core::prelude::v1::test]
fn large_selection_summary_lands_and_is_right() {
    let (dir, engine) = seed_library("large_sel_summary", LARGE);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let ids = ids(&view, vcx);
    engine
        .set_titles(&[(ids[0], Some("Trip")), (ids[1], Some("Trip"))])
        .unwrap();
    engine
        .catalog()
        .add_tags_batch(&ids[..3], &["beach".to_string()])
        .unwrap();
    engine
        .catalog()
        .add_tags_batch(&ids, &["all".to_string()])
        .unwrap();
    view.update(vcx, |app, cx| app.refresh_active_view(cx));
    render_ui(vcx);

    // Two items with a shared title, then ⌘A before anything runs.
    view.update(vcx, |app, cx| {
        app.select_index(0, cx);
        app.selected_indices = [0, 1].into_iter().collect();
        app.update_batch_summary();
        assert_eq!(
            app.batch_metadata.title.common.as_deref(),
            Some("Trip"),
            "test premise: the two share a title"
        );
        app.select_all(cx);
        assert!(
            app.batch_summary.pending,
            "a large summary loads in the background"
        );
        assert_eq!(app.batch_summary.count, LARGE, "the count shows at once");
        assert_ne!(
            app.caption.title_input_buffer, "Trip",
            "the last selection's shared title must not show for the new one"
        );
        assert_eq!(app.batch_metadata.title.common, None, "no stale title");
    });
    vcx.run_until_parked();

    let expected = view.update(vcx, |app, _| app.get_active_slice(0..LARGE));
    view.update(vcx, |app, _| {
        let s = &app.batch_summary;
        assert!(!s.pending, "the summary landed");
        assert_eq!(s.count, LARGE);
        assert_eq!(s.ids, ids, "ids in view order");
        assert_eq!(
            s.total_bytes,
            expected.iter().map(|i| i.file_size).sum::<u64>()
        );
        let of = |t: MediaType| expected.iter().filter(|i| i.media_type == t).count();
        assert_eq!(s.photos_count, of(MediaType::Image));
        assert_eq!(s.videos_count, of(MediaType::Video));
        assert_eq!(s.raw_count, of(MediaType::Raw));
        let times = expected.iter().map(|i| i.timestamp_epoch_ms);
        assert_eq!(s.oldest_ms, times.clone().min().unwrap());
        assert_eq!(s.newest_ms, times.max().unwrap());
        assert!(app.batch_metadata.title.mixed, "titles disagree: Mixed");
        assert_eq!(app.caption.title_input_buffer, "Mixed");
        assert!(
            !app.batch_metadata.caption.mixed,
            "no captions: shared none"
        );
        let mut counts = app.selected_tag_counts.clone();
        counts.sort();
        assert_eq!(
            counts,
            vec![("all".to_string(), LARGE), ("beach".to_string(), 3)],
            "tag counts over the whole selection"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

/// A selection change before the background read lands wins: the stale
/// result is dropped.
#[core::prelude::v1::test]
fn stale_large_selection_summary_is_dropped() {
    let (dir, engine) = seed_library("large_sel_stale", LARGE);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let ids = ids(&view, vcx);
    view.update(vcx, |app, cx| {
        app.select_all(cx);
        app.select_index(3, cx);
    });
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        assert_eq!(app.selected_indices.len(), 1);
        assert!(!app.batch_summary.pending);
        assert_eq!(app.batch_summary.count, 1, "the single selection's summary");
        assert_eq!(app.batch_summary.ids, vec![ids[3]]);
        assert_eq!(app.selected_tag_counts, Vec::new(), "no batch tag counts");
    });
    let _ = std::fs::remove_dir_all(&dir);
}

/// A selection change that skips the batch summary (the viewer's right-click
/// narrows to the shown item) still drops the large summary loading behind
/// it: its totals and tags must not land on the single item.
#[core::prelude::v1::test]
fn large_summary_does_not_land_after_viewer_right_click() {
    let (dir, engine) = seed_library("large_sel_rclick", LARGE);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let ids = ids(&view, vcx);
    engine
        .catalog()
        .add_tags_batch(&ids[1..3], &["beach".to_string()])
        .unwrap();
    engine
        .catalog()
        .add_tags_batch(&ids[..1], &["shown".to_string()])
        .unwrap();
    view.update(vcx, |app, cx| {
        let first = app.get_active_slice(0..1)[0].clone();
        app.select_item_at(first.clone(), 0, cx);
        app.open_viewer(first, cx);
    });
    render_ui(vcx);

    // ⌘A, and the right-click before the background read runs.
    view.update(vcx, |app, cx| app.select_all(cx));
    right_click(vcx, "viewer-viewport");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_indices.len(),
            1,
            "the menu narrows to the shown item"
        );
        assert_ne!(
            app.batch_summary.ids.len(),
            LARGE,
            "the ⌘A summary must not land on the single item"
        );
        assert!(app.selected_tag_counts.is_empty(), "no batch tag list");
        assert!(
            !app.selected_item_tags.iter().any(|tag| tag == "beach"),
            "the shown item does not get the selection's tags: {:?}",
            app.selected_item_tags
        );
        assert_eq!(
            app.selected_item_tags,
            vec!["shown".to_string()],
            "the shown item's own tags come back"
        );
    });
    keystrokes(vcx, "escape");
    let _ = std::fs::remove_dir_all(&dir);
}
