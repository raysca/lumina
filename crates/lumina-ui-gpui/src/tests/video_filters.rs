//! The Videos chip row. Only "All clips" and "4K" ship;
//! HDR, slow motion and Live Photos wait for catalog data.

use super::helpers::{click, focus_app, keystrokes, render_ui, type_text};
use super::video_tiles::{library_with_clips, open_videos};
use crate::app::command_palette::CommandId;
use crate::app::LuminaApp;
use crate::views::sidebar::AlbumSelection;
use gpui::{Entity, VisualTestContext};
use lumina_core::{LuminaEngine, MediaId, SystemAlbum};

/// Gives the rows at `sizes` new pixel dimensions (rows only, no files).
fn resize(engine: &LuminaEngine, sizes: &[(MediaId, u32, u32)]) {
    let total = engine.total_count();
    let rows: Vec<_> = engine
        .get_item_slice(0..total)
        .into_iter()
        .filter_map(|mut row| {
            let &(_, w, h) = sizes.iter().find(|(id, ..)| *id == row.id)?;
            (row.width, row.height) = (w, h);
            Some(row)
        })
        .collect();
    engine.catalog().insert_or_update_batch(&rows).unwrap();
}

/// The view's ids, in view order.
fn shown(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> Vec<MediaId> {
    view.update(vcx, |app, _| {
        let n = app.get_active_count();
        app.get_active_slice(0..n)
            .into_iter()
            .map(|item| item.id)
            .collect()
    })
}

/// Back in plain Videos: the sidebar selection, no query, both clips.
fn assert_all_clips(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, how: &str) {
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::System(SystemAlbum::Videos),
            "{how} returns to Videos"
        );
        assert_eq!(app.search_query, "", "{how} leaves no filter behind");
        assert!(!app.videos_4k_active());
    });
    assert_eq!(shown(view, vcx).len(), 2, "{how} shows both clips again");
}

#[core::prelude::v1::test]
fn videos_4k_chip_filters_by_width() {
    let (_dir, engine, clips) = library_with_clips("vf_4k", 2);
    let photo = engine
        .get_item_slice(0..3)
        .into_iter()
        .find(|item| !clips.contains(&item.id))
        .expect("the seed photo")
        .id;
    // A 4K photo too: the Videos 4K chip must still show clips only.
    resize(
        &engine,
        &[
            (clips[0], 3840, 2160),
            (clips[1], 1920, 1080),
            (photo, 3840, 2160),
        ],
    );
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = super::helpers::app_with_library(&mut cx, engine);
    vcx.simulate_resize(gpui::size(gpui::px(1600.0), gpui::px(1000.0)));
    open_videos(&view, vcx);

    for selector in ["filter-chip-all", "filter-chip-4k"] {
        assert!(
            vcx.debug_bounds(selector).is_some(),
            "{selector}: the Videos chip row is painted"
        );
    }
    assert!(
        vcx.debug_bounds("filter-clear-all").is_none(),
        "plain Videos has nothing to clear, so Clear all is hidden"
    );
    for selector in ["filter-chip-hdr", "filter-chip-slowmo", "filter-chip-live"] {
        assert!(
            vcx.debug_bounds(selector).is_none(),
            "{selector}: chips without catalog data stay hidden"
        );
    }
    assert_eq!(shown(&view, vcx).len(), 2);

    click(vcx, "filter-chip-4k");
    render_ui(vcx);
    assert_eq!(
        shown(&view, vcx),
        [clips[0]],
        "4K leaves the 3840-wide clip: not the 1920-wide one, not the 4K photo"
    );
    view.update(vcx, |app, _| {
        assert!(app.videos_4k_active());
        assert!(
            app.videos_view(),
            "the 4K query keeps the Videos view (title, clips wording, totals)"
        );
        assert_eq!(app.view_label(), "Videos");
        assert!(
            app.sidebar_highlights(&AlbumSelection::System(SystemAlbum::Videos)),
            "the sidebar keeps Videos highlighted"
        );
    });
    assert!(
        vcx.debug_bounds("sidebar-nav-videos-active").is_some()
            && vcx.debug_bounds("sidebar-nav-all-active").is_none(),
        "the painted sidebar shows Videos as the active row while 4K is on"
    );
    view.update(vcx, |app, _| {
        let meta = app.page_meta().unwrap_or_default();
        assert!(
            meta.starts_with("1 clip · 1 m · "),
            "the header counts the 4K clip and sums only its 1:18, not every clip in Videos: {meta:?}"
        );
        assert_eq!(app.count_label(1), "1 clip");
    });
    assert!(
        vcx.debug_bounds("filter-chip-4k").is_some() && vcx.debug_bounds("filter-bar").is_some(),
        "the Videos row stays, the generic chip row does not take over"
    );
    assert!(
        vcx.debug_bounds("filter-chip-0").is_none(),
        "no generic \"Videos\" / \"4K\" removable chips"
    );

    click(vcx, "filter-clear-all");
    assert_all_clips(&view, vcx, "Clear all");

    click(vcx, "filter-chip-4k");
    click(vcx, "filter-chip-4k");
    assert_all_clips(&view, vcx, "4K a second time");

    click(vcx, "filter-chip-4k");
    click(vcx, "filter-chip-all");
    assert_all_clips(&view, vcx, "All clips");

    // The keyboard path: ⌘K, type, Enter runs Filter: 4K Videos.
    palette_4k(&view, vcx);
    assert_eq!(shown(&view, vcx), [clips[0]], "Filter: 4K Videos");
    // Back undoes the filter like any search: plain Videos again.
    view.update(vcx, |app, cx| app.navigate_back(cx));
    assert_all_clips(&view, vcx, "Back after 4K");
    palette_4k(&view, vcx);
    palette_4k(&view, vcx);
    assert_all_clips(&view, vcx, "Filter: 4K Videos a second time");

    // Clear Filters (the palette's Clear all) lands in Videos, not All Media.
    view.update(vcx, |app, cx| app.execute_command(CommandId::Videos4k, cx));
    view.update(vcx, |app, cx| {
        app.execute_command(CommandId::ClearFilters, cx)
    });
    assert_all_clips(&view, vcx, "Clear Filters");
}

/// Runs Filter: 4K Videos through the palette as a user does: ⌘K, type
/// its name, Enter.
fn palette_4k(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) {
    focus_app(vcx, view);
    keystrokes(vcx, "secondary-k");
    type_text(vcx, "4k videos");
    view.update(vcx, |app, _| {
        assert!(app.command_palette_open, "⌘K opens the palette");
        assert_eq!(
            app.palette_rows()
                .first()
                .map(|row| row.title.clone())
                .as_deref(),
            Some("Filter: 4K Videos"),
            "typing its name puts Filter: 4K Videos first"
        );
    });
    keystrokes(vcx, "enter");
    render_ui(vcx);
}

#[core::prelude::v1::test]
fn filter_4k_videos_is_offered_only_in_videos() {
    let (_dir, engine, _clips) = library_with_clips("vf_4k_scope", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = super::helpers::app_with_library(&mut cx, engine);
    let offered = |app: &LuminaApp| app.command_applies(CommandId::Videos4k);
    view.update(vcx, |app, cx| {
        assert!(
            !offered(app),
            "not in All Media, where it would replace the search"
        );
        app.set_search_query("beach", cx);
        assert!(!offered(app), "not over a typed search either");
        app.set_search_query("", cx);
    });
    open_videos(&view, vcx);
    view.update(vcx, |app, cx| {
        assert!(offered(app), "plain Videos offers it");
        app.toggle_videos_4k(cx);
        assert!(offered(app), "and 4K Videos, to turn it off");
    });
    // A search typed in Videos leaves the Videos view: no offer to wipe it.
    view.update(vcx, |app, cx| app.set_search_query("is:video beach", cx));
    view.update(vcx, |app, _| assert!(!offered(app)));
}

#[core::prelude::v1::test]
fn videos_4k_with_no_match_shows_clips_empty_state_and_clear_all() {
    let (_dir, engine, clips) = library_with_clips("vf_4k_none", 2);
    resize(&engine, &[(clips[0], 1920, 1080), (clips[1], 1280, 720)]);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = super::helpers::app_with_library(&mut cx, engine);
    open_videos(&view, vcx);

    click(vcx, "filter-chip-4k");
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("empty-state").is_some(),
        "a filter that matches nothing shows the empty state"
    );
    view.update(vcx, |app, _| {
        let (title, body) = app.videos_empty_copy().expect("Videos empty copy");
        assert!(
            title.contains("clips") && body.contains("Clear all"),
            "the empty state speaks of clips and points at Clear all: {title:?} / {body:?}"
        );
        let meta = app.page_meta().unwrap_or_default();
        assert!(
            meta.starts_with("0 clips"),
            "the header counts no clips: {meta:?}"
        );
    });
    assert!(
        vcx.debug_bounds("filter-clear-all").is_some(),
        "Clear all stays reachable over an empty result"
    );
    click(vcx, "filter-clear-all");
    assert_all_clips(&view, vcx, "Clear all from the empty state");
}
