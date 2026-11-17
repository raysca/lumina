//! Mosaic layout: the header's Mosaic segment swaps the
//! uniform grid for justified rows, persisted as `browse_layout`. The
//! layout is fetched and justified off the UI thread; scrolling reads only
//! the cached layout and rows.

use std::sync::Arc;

use gpui::{px, size, Bounds, Pixels, VisualTestContext};
use lumina_core::{LuminaEngine, MediaId};

use crate::app::active_view::ROWS_READ;
use crate::app::mosaic_layout::LAYOUT_FETCHES;
use crate::config::BrowseLayout;
use crate::tests::duplicates::run_palette_command;
use crate::tests::ui_automation::{
    app_with_library, click, leak, render_ui, seed_library, seed_library_undecodable,
};
use crate::views::sidebar::AlbumSelection;

fn layout(view: &gpui::Entity<crate::LuminaApp>, vcx: &mut VisualTestContext) -> BrowseLayout {
    view.update(vcx, |app, _| app.config.browse_layout)
}

pub(crate) fn card(vcx: &mut VisualTestContext, id: MediaId) -> Bounds<Pixels> {
    let selector = leak(format!("media-card-{}", id.0));
    vcx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} must be drawn"))
}

#[core::prelude::v1::test]
fn mosaic_segment_switches_layout_and_persists_in_config() {
    let (dir, engine) = seed_library("mosaic_segment", 4);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    render_ui(vcx);
    // GPUI's test window builds no accessibility tree, so `aria_toggled`
    // is read through the state it mirrors (`view_mode_on`).
    assert_eq!(
        layout(&view, vcx),
        BrowseLayout::Grid,
        "Grid is the default"
    );
    view.update(vcx, |app, _| {
        assert_eq!(
            app.view_mode_on(),
            (false, true, false),
            "the Grid segment is the toggled one by default"
        )
    });
    assert!(vcx.debug_bounds("media-grid").is_some(), "the uniform grid");
    assert!(vcx.debug_bounds("mosaic-list").is_none());
    let mosaic = vcx
        .debug_bounds("view-mode-mosaic")
        .expect("Mosaic segment");
    let grid = vcx.debug_bounds("view-mode-grid").expect("Grid segment");
    assert!(
        mosaic.origin.x < grid.origin.x,
        "Mosaic is the first segment"
    );

    click(vcx, "view-mode-mosaic");
    render_ui(vcx);
    render_ui(vcx);
    assert_eq!(
        layout(&view, vcx),
        BrowseLayout::Mosaic,
        "the segment sets the config's browse_layout, which the next save writes"
    );
    view.update(vcx, |app, _| {
        assert_eq!(app.view_mode_on(), (true, false, false), "Mosaic is on")
    });
    assert!(vcx.debug_bounds("mosaic-list").is_some(), "the mosaic list");
    assert!(vcx.debug_bounds("media-grid").is_none(), "no uniform grid");
    assert!(
        vcx.debug_bounds("grid-month-header").is_none(),
        "the month band is hidden in Mosaic"
    );

    click(vcx, "view-mode-grid");
    render_ui(vcx);
    assert_eq!(layout(&view, vcx), BrowseLayout::Grid);
    assert!(vcx.debug_bounds("media-grid").is_some(), "Grid comes back");
    assert!(vcx.debug_bounds("mosaic-list").is_none());

    // A failed save keeps the screen on the saved layout and says so.
    crate::app::state::SAVE_FAILS.with(|f| f.set(true));
    click(vcx, "view-mode-mosaic");
    crate::app::state::SAVE_FAILS.with(|f| f.set(false));
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.config.browse_layout,
            BrowseLayout::Grid,
            "a failed save reverts the layout"
        );
        assert!(
            app.error_message.is_some(),
            "the failure is in the error banner"
        );
        app.error_message = None;
    });

    // Keyboard path: ⌘K Show Mosaic and Show Grid.
    run_palette_command(vcx, &view, "Show Mosaic");
    render_ui(vcx);
    assert_eq!(layout(&view, vcx), BrowseLayout::Mosaic, "⌘K Show Mosaic");
    run_palette_command(vcx, &view, "Show Grid");
    render_ui(vcx);
    assert_eq!(layout(&view, vcx), BrowseLayout::Grid, "⌘K Show Grid");

    // Views with no layout scope fall back to the Grid body.
    run_palette_command(vcx, &view, "Show Mosaic");
    render_ui(vcx);
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::User(987_654), cx)
    });
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(
            app.view_query().layout_scope().is_none(),
            "a missing smart album has no layout scope"
        )
    });
    assert!(
        vcx.debug_bounds("mosaic-list").is_none(),
        "a view without a layout scope draws the Grid body"
    );
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::Duplicates, cx)
    });
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.view_query().layout_scope().is_none());
        assert_eq!(app.config.browse_layout, BrowseLayout::Mosaic, "kept");
    });
    assert!(
        vcx.debug_bounds("mosaic-list").is_none(),
        "Duplicates keeps its own screen in Mosaic"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn mosaic_tiles_keep_aspect_and_fill_rows() {
    let (dir, engine) = seed_library("mosaic_tiles", 6);
    let ids: Vec<MediaId> = engine.get_item_slice(0..6).iter().map(|i| i.id).collect();
    let sizes = [(3000, 2000), (2000, 3000), (1000, 1000)];
    let conn = rusqlite::Connection::open(dir.join("catalog.db")).expect("catalog opens");
    for (n, id) in ids.iter().enumerate() {
        let (w, h) = sizes[n % 3];
        conn.execute(
            "UPDATE media_items SET width = ?1, height = ?2 WHERE id = ?3",
            [w, h, id.0 as i64],
        )
        .unwrap();
    }
    drop(conn);
    crate::app::state::TEST_CONFIG.with(|c| {
        *c.borrow_mut() = Some(crate::config::AppConfig {
            browse_layout: BrowseLayout::Mosaic,
            ..Default::default()
        })
    });
    let mut cx = crate::tests::TestApp::single();
    let (_view, vcx) = app_with_library(&mut cx, engine);
    vcx.simulate_resize(size(px(1440.0), px(900.0)));
    render_ui(vcx);
    render_ui(vcx);
    assert!(vcx.debug_bounds("mosaic-list").is_some(), "restored Mosaic");
    let surface = vcx.debug_bounds("media-grid-surface").expect("surface");
    let mut cards: Vec<(usize, Bounds<Pixels>)> = ids
        .iter()
        .enumerate()
        .map(|(n, &id)| (n, card(vcx, id)))
        .collect();
    for (n, b) in &cards {
        let (w, h) = sizes[n % 3];
        let want = w as f32 / h as f32;
        let got = f32::from(b.size.width) / f32::from(b.size.height);
        assert!(
            (got / want - 1.0).abs() < 0.02,
            "tile {n} keeps its {want:.3} aspect: {got:.3}"
        );
    }
    cards.sort_by(|a, b| {
        (a.1.origin.y, a.1.origin.x)
            .partial_cmp(&(b.1.origin.y, b.1.origin.x))
            .unwrap()
    });
    let mut rows: Vec<Vec<Bounds<Pixels>>> = Vec::new();
    for (_, b) in cards {
        match rows.last_mut() {
            Some(row) if (row[0].origin.y - b.origin.y).abs() < px(0.5) => row.push(b),
            _ => rows.push(vec![b]),
        }
    }
    assert!(!rows.is_empty(), "six tiles make at least one row");
    let left = f32::from(surface.origin.x) + 28.0;
    let right = f32::from(surface.origin.x + surface.size.width) - 20.0;
    for (r, row) in rows.iter().enumerate() {
        let h = row[0].size.height;
        assert!(
            row.iter().all(|b| b.size.height == h),
            "tiles in row {r} share a height"
        );
        assert!(
            (f32::from(row[0].origin.x) - left).abs() <= 1.0,
            "row {r} starts at the surface's left padding (28)"
        );
        // The day's last row is re-flowed to fill too (frame: no ragged rows).
        let end = row.last().unwrap();
        assert!(
            (f32::from(end.origin.x + end.size.width) - right).abs() <= 1.0,
            "row {r} ends at the surface's right padding (20), the last row included"
        );
        assert!(
            (158.0..=224.0).contains(&f32::from(h)),
            "row {r} is 158-224 high: {h:?}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// A Mosaic app on `count` texture-less items (aspect 1.0), layout landed.
pub(crate) fn mosaic_app(
    cx: &mut gpui::TestAppContext,
    engine: Arc<LuminaEngine>,
) -> (gpui::Entity<crate::LuminaApp>, &mut VisualTestContext) {
    crate::app::state::TEST_CONFIG.with(|c| {
        *c.borrow_mut() = Some(crate::config::AppConfig {
            browse_layout: BrowseLayout::Mosaic,
            ..Default::default()
        })
    });
    let (view, vcx) = app_with_library(cx, engine);
    render_ui(vcx);
    render_ui(vcx);
    (view, vcx)
}

#[core::prelude::v1::test]
fn mosaic_scroll_reads_no_sql() {
    let (dir, engine) = seed_library_undecodable("mosaic_scroll", 300);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = mosaic_app(&mut cx, engine);
    let rows = view.update(vcx, |app, _| {
        app.mosaic_layout()
            .expect("the layout has landed")
            .rows
            .len()
    });
    assert!(rows > 50, "300 items make many rows: {rows}");
    let fetches = LAYOUT_FETCHES.with(|n| n.get());
    let read = ROWS_READ.with(|n| n.get());
    let first = view.update(vcx, |app, _| {
        let state = app.grid_state.as_ref().unwrap();
        state.mosaic.scroll_to(gpui::ListOffset {
            item_ix: 40,
            offset_in_item: px(0.0),
        });
        app.mosaic_layout().unwrap().rows[40].first
    });
    // Each pass must park: a list that re-asks for rows every frame would
    // hang here (the uniform_list regression in grid_scroll.rs).
    render_ui(vcx);
    render_ui(vcx);
    assert_eq!(
        LAYOUT_FETCHES.with(|n| n.get()),
        fetches,
        "scrolling never fetches the layout again"
    );
    assert_eq!(
        ROWS_READ.with(|n| n.get()),
        read,
        "scrolling never reads rows synchronously"
    );
    view.update(vcx, |app, _| {
        let top = app.grid_state.as_ref().unwrap().mosaic.logical_scroll_top();
        assert_eq!(top.item_ix, 40, "the scroll held at row 40");
        assert!(
            !app.displayed_rows(first..first + 1).is_empty(),
            "the visible rows were fetched in the background (want_view_rows)"
        );
    });
    let id = view.update(vcx, |app, _| app.displayed_rows(first..first + 1)[0].id);
    card(vcx, id);
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn mosaic_layout_refetch_coalesces_scan_writes() {
    let (dir, engine) = seed_library_undecodable("mosaic_coalesce", 12);
    let writer = engine.clone();
    let ids: Vec<MediaId> = engine.get_item_slice(0..12).iter().map(|i| i.id).collect();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = mosaic_app(&mut cx, engine);
    let fetches = LAYOUT_FETCHES.with(|n| n.get());
    let headers = |view: &gpui::Entity<crate::LuminaApp>, vcx: &mut VisualTestContext| {
        view.update(vcx, |app, _| {
            let layout = app.mosaic_layout().unwrap();
            layout.rows.len() - layout.rows.iter().filter(|r| r.len > 0).count()
        })
    };
    let before = headers(&view, vcx);
    // A scan writes from its own thread, three times in a row.
    for (n, chunk) in ids.chunks(4).enumerate() {
        let writer = writer.clone();
        let changes: Vec<(MediaId, i64)> = chunk
            .iter()
            .map(|&id| (id, 1_700_000_000_000 - n as i64 * 86_400_000 * 3))
            .collect();
        std::thread::spawn(move || writer.set_timestamps(&changes).unwrap())
            .join()
            .unwrap();
        render_ui(vcx);
    }
    assert_eq!(
        LAYOUT_FETCHES.with(|n| n.get()),
        fetches,
        "scan writes right after a fetch wait for the cool-down"
    );
    vcx.executor()
        .advance_clock(crate::app::mosaic_layout::SCAN_REFETCH_COOLDOWN);
    render_ui(vcx);
    render_ui(vcx);
    assert_eq!(
        LAYOUT_FETCHES.with(|n| n.get()),
        fetches + 1,
        "the three writes coalesce into one refetch"
    );
    assert!(
        headers(&view, vcx) == 3 && before != 3,
        "the refetched layout has the three new days"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// 120 texture-less items over two days, 60 each (newest day first), so
/// either day's header can scroll to the top.
fn two_day_library(prefix: &str) -> (std::path::PathBuf, Arc<LuminaEngine>, Vec<MediaId>) {
    let (dir, engine) = seed_library_undecodable(prefix, 120);
    let ids: Vec<MediaId> = engine.get_item_slice(0..120).iter().map(|i| i.id).collect();
    let changes: Vec<(MediaId, i64)> = ids
        .iter()
        .enumerate()
        .map(|(n, &id)| (id, 1_700_000_000_000 - (n / 60) as i64 * 86_400_000 * 3))
        .collect();
    engine.set_timestamps(&changes).unwrap();
    (dir, engine, ids)
}

fn top_row(
    view: &gpui::Entity<crate::LuminaApp>,
    vcx: &mut VisualTestContext,
) -> (usize, crate::views::mosaic::MosaicRow) {
    view.update(vcx, |app, _| {
        let ix = app
            .grid_state
            .as_ref()
            .unwrap()
            .mosaic
            .logical_scroll_top()
            .item_ix;
        (ix, app.mosaic_layout().unwrap().rows[ix])
    })
}

fn is_header(row: &crate::views::mosaic::MosaicRow) -> bool {
    matches!(row.kind, crate::views::mosaic::RowKind::Header { .. })
}

#[core::prelude::v1::test]
fn relayout_keeps_a_header_on_top() {
    let (dir, engine, _) = two_day_library("mosaic_relayout");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = mosaic_app(&mut cx, engine);
    let relayout = |view: &gpui::Entity<crate::LuminaApp>, vcx: &mut VisualTestContext| {
        view.update(vcx, |app, cx| {
            app.grid_columns = if app.grid_columns == 4 { 3 } else { 4 };
            cx.notify();
        });
        render_ui(vcx);
        render_ui(vcx);
    };
    relayout(&view, vcx);
    let (ix, row) = top_row(&view, vcx);
    assert!(
        ix == 0 && is_header(&row),
        "a relayout at the top keeps the first day header on top: row {ix} {row:?}"
    );

    let (second, first_item) = view.update(vcx, |app, _| {
        let layout = app.mosaic_layout().unwrap();
        let r = (1..layout.rows.len())
            .find(|&r| is_header(&layout.rows[r]))
            .expect("a second day header");
        app.grid_state
            .as_ref()
            .unwrap()
            .mosaic
            .scroll_to(gpui::ListOffset {
                item_ix: r,
                offset_in_item: px(0.0),
            });
        (r, layout.rows[r].first)
    });
    render_ui(vcx);
    assert_eq!(
        top_row(&view, vcx).0,
        second,
        "scrolled to the second header"
    );
    relayout(&view, vcx);
    let (_, row) = top_row(&view, vcx);
    assert!(
        is_header(&row) && row.first == first_item,
        "a mid-list header on top stays the top row: {row:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn header_rows_are_their_layout_height_and_absent_without_date_sort() {
    let (dir, engine, _) = two_day_library("mosaic_headers");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = mosaic_app(&mut cx, engine);
    let headers: Vec<(u32, f32)> = view.update(vcx, |app, _| {
        app.mosaic_layout()
            .unwrap()
            .rows
            .iter()
            .filter_map(|r| match r.kind {
                crate::views::mosaic::RowKind::Header { day, .. } => Some((day, r.height)),
                _ => None,
            })
            .collect()
    });
    assert_eq!(headers.len(), 2, "two days, two headers");
    // The second header is below the fold at the default size; bring the
    // first day's last row and the second header on screen.
    for (n, (day, height)) in headers.iter().enumerate() {
        if n == 1 {
            view.update(vcx, |app, _| {
                let layout = app.mosaic_layout().unwrap();
                let r = layout.rows.iter().rposition(is_header).unwrap();
                app.grid_state
                    .as_ref()
                    .unwrap()
                    .mosaic
                    .scroll_to(gpui::ListOffset {
                        item_ix: r,
                        offset_in_item: px(0.0),
                    });
            });
            render_ui(vcx);
        }
        let row = vcx
            .debug_bounds(leak(format!("mosaic-day-row-{day}")))
            .unwrap_or_else(|| panic!("header row {day} drawn"));
        assert!(
            (f32::from(row.size.height) - height).abs() < 0.5,
            "the header element is exactly its layout row's height: {row:?} vs {height}"
        );
    }

    view.update(vcx, |app, cx| {
        app.sort.field = lumina_core::SortField::FileName;
        cx.notify();
    });
    render_ui(vcx);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(
            app.mosaic_layout()
                .unwrap()
                .rows
                .iter()
                .all(|r| !is_header(r)),
            "no sections under a non-date sort"
        )
    });
    for (day, _) in &headers {
        assert!(
            vcx.debug_bounds(leak(format!("mosaic-day-{day}")))
                .is_none(),
            "no day header drawn under a non-date sort"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn own_write_refetches_the_layout_without_the_cool_down() {
    let (dir, engine) = seed_library_undecodable("mosaic_own_write", 12);
    let writer = engine.clone();
    let id = engine.get_item_slice(0..1)[0].id;
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = mosaic_app(&mut cx, engine);
    let fetches = LAYOUT_FETCHES.with(|n| n.get());
    let tiles = |view: &gpui::Entity<crate::LuminaApp>, vcx: &mut VisualTestContext| {
        view.update(vcx, |app, _| app.mosaic_layout().unwrap().tiles.len())
    };
    assert_eq!(tiles(&view, vcx), 12);
    // The UI thread trashes an item (as Delete does), inside the cool-down.
    writer.set_trashed(id, true).unwrap();
    render_ui(vcx);
    render_ui(vcx);
    assert_eq!(
        LAYOUT_FETCHES.with(|n| n.get()),
        fetches + 1,
        "the app's own write refetches at once"
    );
    assert_eq!(tiles(&view, vcx), 11, "the trashed item left the layout");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Seven items: three on 2025-06-14 (two in Lisbon, PT, one in Porto,
/// PT), two on 2025-06-13 with no city, two undated. In view order (date,
/// newest first): 0-2 the 14th, 3-4 the 13th, 5-6 undated.
pub(crate) fn day_library(prefix: &str) -> (std::path::PathBuf, Arc<LuminaEngine>) {
    let (dir, engine) = seed_library_undecodable(prefix, 7);
    let ids: Vec<MediaId> = engine.get_item_slice(0..7).iter().map(|i| i.id).collect();
    let noon = |d: &str| {
        chrono::NaiveDateTime::parse_from_str(&format!("{d} 12:00:00"), "%Y-%m-%d %H:%M:%S")
            .unwrap()
            .and_utc()
            .timestamp_millis()
    };
    let days = [
        "2025-06-14",
        "2025-06-14",
        "2025-06-14",
        "2025-06-13",
        "2025-06-13",
    ];
    let mut changes: Vec<(MediaId, i64)> =
        ids.iter().zip(days).map(|(&id, d)| (id, noon(d))).collect();
    changes.extend(ids[5..].iter().map(|&id| (id, 0)));
    engine.set_timestamps(&changes).unwrap();
    let conn = rusqlite::Connection::open(dir.join("catalog.db")).expect("catalog opens");
    for (id, city) in [(ids[0], "Lisbon"), (ids[1], "Porto"), (ids[2], "Lisbon")] {
        conn.execute(
            "UPDATE media_items SET city = ?1, country_code = 'PT' WHERE id = ?2",
            rusqlite::params![city, id.0 as i64],
        )
        .unwrap();
    }
    drop(conn);
    (dir, engine)
}

pub(crate) fn day_app(
    cx: &mut gpui::TestAppContext,
    engine: Arc<LuminaEngine>,
) -> (gpui::Entity<crate::LuminaApp>, &mut VisualTestContext) {
    let (view, vcx) = mosaic_app(cx, engine);
    vcx.simulate_resize(size(px(1440.0), px(1400.0)));
    render_ui(vcx);
    render_ui(vcx);
    (view, vcx)
}

#[core::prelude::v1::test]
fn day_header_shows_date_place_and_count() {
    let (dir, engine) = day_library("mosaic_day_header");
    let ids: Vec<MediaId> = engine.get_item_slice(0..7).iter().map(|i| i.id).collect();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = day_app(&mut cx, engine);
    let order: Vec<MediaId> = view.update(vcx, |app, _| {
        app.displayed_rows(0..7).iter().map(|i| i.id).collect()
    });
    assert_eq!(order, ids, "the view order is the seeded order");

    view.update(vcx, |app, _| {
        let h = app.mosaic_day_header(20250614, 3);
        assert_eq!(h.date, "Sat, Jun 14, 2025");
        assert_eq!(
            h.place.as_deref(),
            Some("Lisbon, PT"),
            "the most common city"
        );
        assert_eq!(h.count, "3 items");
        assert_eq!(h.aria_label, "Sat, Jun 14, 2025, Lisbon, PT, 3 items");
        assert_eq!(h.select_label, "Select 3 items from Sat, Jun 14, 2025");
        let h = app.mosaic_day_header(20250613, 2);
        assert_eq!(h.place, None, "a day with no city has no place");
        assert_eq!(h.aria_label, "Fri, Jun 13, 2025, 2 items");
        let h = app.mosaic_day_header(0, 2);
        assert_eq!(h.date, "Undated");
        assert_eq!(h.place, None);
        let layout = app.mosaic_layout().unwrap();
        let last_header = layout.rows.iter().rev().find_map(|r| match r.kind {
            crate::views::mosaic::RowKind::Header { day, .. } => Some(day),
            _ => None,
        });
        assert_eq!(last_header, Some(0), "Undated comes last");
    });

    for selector in [
        "mosaic-day-20250614",
        "mosaic-day-place-20250614",
        "mosaic-day-count-20250614",
        "mosaic-day-select-20250614",
        "mosaic-day-20250613",
        "mosaic-day-count-20250613",
        "mosaic-day-0",
    ] {
        assert!(vcx.debug_bounds(selector).is_some(), "{selector} is drawn");
    }
    assert!(
        vcx.debug_bounds("mosaic-day-place-20250613").is_none(),
        "no place element for a day with no city"
    );
    assert!(
        vcx.debug_bounds("mosaic-day-place-0").is_none(),
        "Undated has no place"
    );

    let tiles_bottom = ids[..3]
        .iter()
        .map(|&id| {
            let b = card(vcx, id);
            f32::from(b.origin.y + b.size.height)
        })
        .fold(0.0, f32::max);
    let second = vcx.debug_bounds("mosaic-day-20250613").unwrap();
    assert!(
        (f32::from(second.origin.y) - tiles_bottom - 26.0).abs() <= 1.0,
        "the second header sits 26 below the tiles above it: {second:?} vs {tiles_bottom}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn select_day_selects_exactly_that_days_items() {
    use crate::app::command_palette::CommandId;
    let (dir, engine) = day_library("mosaic_select_day");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = day_app(&mut cx, engine);
    let selected = |view: &gpui::Entity<crate::LuminaApp>, vcx: &mut VisualTestContext| {
        view.update(vcx, |app, _| {
            app.selected_indices.iter().copied().collect::<Vec<_>>()
        })
    };

    click(vcx, "mosaic-day-select-20250613");
    render_ui(vcx);
    assert_eq!(
        selected(&view, vcx),
        vec![3, 4],
        "Select picks that day's items"
    );
    view.update(vcx, |app, _| {
        assert_eq!(app.selected_index, Some(3));
        assert_eq!(app.batch_summary.count, 2, "the batch summary follows");
    });

    view.update(vcx, |app, cx| app.select_index(1, cx));
    render_ui(vcx);
    run_palette_command(vcx, &view, "Select Day");
    render_ui(vcx);
    assert_eq!(
        selected(&view, vcx),
        vec![0, 1, 2],
        "⌘K Select Day picks the selected item's day"
    );
    view.update(vcx, |app, _| assert_eq!(app.batch_summary.count, 3));

    crate::tests::ui_automation::keystrokes(vcx, "escape");
    render_ui(vcx);
    assert!(
        selected(&view, vcx).is_empty(),
        "Escape clears the selection"
    );

    view.update(vcx, |app, cx| app.clear_selection(cx));
    run_palette_command(vcx, &view, "Select Day");
    render_ui(vcx);
    assert_eq!(
        selected(&view, vcx),
        vec![0, 1, 2],
        "with nothing selected, the top visible day"
    );

    view.update(vcx, |app, cx| {
        assert!(
            app.command_applies(CommandId::SelectDay),
            "in Mosaic, date sort"
        );
        app.sort.field = lumina_core::SortField::FileName;
        cx.notify();
        assert!(
            !app.command_applies(CommandId::SelectDay),
            "not under a non-date sort"
        );
        app.sort.field = lumina_core::SortField::DateTaken;
        app.set_browse_layout(BrowseLayout::Grid, cx);
        assert!(!app.command_applies(CommandId::SelectDay), "not in Grid");
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn place_label_joins_city_and_country_code() {
    use crate::app::render::mosaic::place_label;
    assert_eq!(
        place_label(&("Lisbon".into(), Some("PT".into()))),
        "Lisbon, PT"
    );
    assert_eq!(place_label(&("Lisbon".into(), None)), "Lisbon");
}

#[core::prelude::v1::test]
fn select_day_does_nothing_in_the_full_screen_viewer() {
    use crate::app::command_palette::CommandId;
    let (dir, engine) = day_library("mosaic_select_viewer");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = day_app(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.select_index(4, cx);
        let item = app.selected_item.clone().expect("item 4 selected");
        app.open_viewer(item, cx);
    });
    render_ui(vcx);
    view.update(vcx, |app, cx| {
        assert!(app.is_fullscreen_viewer, "the viewer is open");
        let shown = app.selected_item.as_ref().map(|i| i.id);
        assert!(
            !app.command_applies(CommandId::SelectDay),
            "Select Day is not offered over the viewer"
        );
        app.execute_command(CommandId::SelectDay, cx);
        assert_eq!(
            app.selected_item.as_ref().map(|i| i.id),
            shown,
            "the photo on screen stays the selected one"
        );
        assert_eq!(
            app.selected_indices.iter().copied().collect::<Vec<_>>(),
            vec![4],
            "the selection is unchanged"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn select_waits_for_a_layout_behind_the_catalog() {
    use crate::app::command_palette::CommandId;
    let (dir, engine) = day_library("mosaic_select_stale");
    let writer = engine.clone();
    let id = engine.get_item_slice(0..1)[0].id;
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = day_app(&mut cx, engine);
    // A scan writes from its own thread inside the cool-down: the layout
    // stays a revision behind.
    std::thread::spawn(move || writer.toggle_favorite(id, true).unwrap())
        .join()
        .unwrap();
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(!app.mosaic_layout_current(), "the layout is behind");
        assert!(
            !app.command_applies(CommandId::SelectDay),
            "Select Day waits for the next layout"
        );
    });
    click(vcx, "mosaic-day-select-20250613");
    render_ui(vcx);
    view.update(vcx, |app, cx| {
        assert!(
            app.selected_indices.is_empty(),
            "Select on a stale layout selects nothing"
        );
        app.execute_command(CommandId::SelectDay, cx);
        assert!(app.selected_indices.is_empty(), "nor does ⌘K Select Day");
    });
    vcx.executor()
        .advance_clock(crate::app::mosaic_layout::SCAN_REFETCH_COOLDOWN);
    render_ui(vcx);
    render_ui(vcx);
    click(vcx, "mosaic-day-select-20250613");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_indices.iter().copied().collect::<Vec<_>>(),
            vec![3, 4],
            "once the layout catches up, Select works"
        )
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn select_day_closes_open_inputs() {
    let (dir, engine) = day_library("mosaic_select_inputs");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = day_app(&mut cx, engine);
    view.update(vcx, |app, _| {
        app.tag_input_active = true;
        app.tag_input_buffer = "half typed".into();
        app.caption.input_active = true;
        app.caption.input_buffer = "draft".into();
    });
    click(vcx, "mosaic-day-select-20250614");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.selected_indices.len(), 3);
        assert!(
            !app.tag_input_active && app.tag_input_buffer.is_empty(),
            "the tag input belongs to the old selection"
        );
        assert!(
            !app.caption.input_active && app.caption.input_buffer.is_empty(),
            "so does the caption input"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}
