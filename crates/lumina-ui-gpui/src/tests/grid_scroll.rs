//! The virtualized grid settles after a deep scroll.

use super::ui_automation::{
    app_with_library, click, leak, render_ui, seed_library, seed_library_undecodable,
};

/// Regression: `uniform_list` also renders row 0 on every layout just to
/// measure it. That call used to move the fetch window and the month band
/// back to row 0, so a scroll past the first fetch window (16 rows at 4
/// columns) swapped them every frame and the window never stopped redrawing.
/// Without the fix this test never parks (it hangs rather than failing an
/// assertion).
#[core::prelude::v1::test]
fn deep_scroll_settles_and_keeps_the_visible_rows() {
    let (dir, engine) = seed_library("grid_deep_scroll", 200);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.grid_columns = 4;
        cx.notify();
    });
    render_ui(vcx);
    let target = view.update(vcx, |app, _| {
        let item = app.get_active_slice(160..161).into_iter().next();
        app.grid_state
            .as_ref()
            .unwrap()
            .scroll_handle
            .scroll_to_item_strict(40, gpui::ScrollStrategy::Top);
        item.expect("the scan must produce 200 items")
    });
    render_ui(vcx);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.grid_top_row(), 40, "the scroll held at row 40");
        let top = app.displayed_rows(160..161);
        assert_eq!(
            top.first().map(|item| item.id),
            Some(target.id),
            "the fetch window must stay on the visible rows, not jump back to row 0"
        );
        assert_eq!(
            app.month_header.top,
            Some((target.year, target.month)),
            "the month band must follow the top visible row"
        );
    });

    let _ = std::fs::remove_dir_all(&dir);
}

/// Grid columns in the rail tests: few cards per frame keeps them fast.
const COLS: usize = 4;

/// Months seeded newest first, with their item counts: 60 items, so at 4
/// columns 2025-06 fills rows 0-4, 2025-05 starts at item 20 (row 5),
/// 2024-12 at item 28 (row 7) and 2024-03 at item 36 (rows 9-14).
const RAIL_MONTHS: [(&str, usize); 4] = [
    ("2025-06-15 12:00:00", 20),
    ("2025-05-15 12:00:00", 8),
    ("2024-12-15 12:00:00", 8),
    ("2024-03-15 12:00:00", 24),
];

fn seed_rail_library(
    prefix: &str,
) -> (
    std::path::PathBuf,
    std::sync::Arc<lumina_core::LuminaEngine>,
) {
    let total = RAIL_MONTHS.iter().map(|(_, n)| n).sum();
    let (dir, engine) = seed_library_undecodable(prefix, total);
    let items = engine.get_item_slice(0..total);
    let mut items = items.iter();
    let mut changes = Vec::new();
    for (date, count) in RAIL_MONTHS {
        let ms = chrono::NaiveDateTime::parse_from_str(date, "%Y-%m-%d %H:%M:%S")
            .unwrap()
            .and_utc()
            .timestamp_millis();
        for item in items.by_ref().take(count) {
            changes.push((item.id, ms));
        }
    }
    engine.set_timestamps(&changes).unwrap();
    (dir, engine)
}

fn rail_app(
    cx: &mut gpui::TestAppContext,
    engine: std::sync::Arc<lumina_core::LuminaEngine>,
) -> (gpui::Entity<crate::LuminaApp>, &mut gpui::VisualTestContext) {
    let (view, vcx) = app_with_library(cx, engine);
    view.update(vcx, |app, cx| {
        app.grid_columns = COLS;
        cx.notify();
    });
    render_ui(vcx);
    render_ui(vcx);
    (view, vcx)
}

fn bounds(vcx: &mut gpui::VisualTestContext, selector: &'static str) -> gpui::Bounds<gpui::Pixels> {
    vcx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} must be drawn"))
}

#[core::prelude::v1::test]
fn rail_draws_month_bars_and_scrolls_to_a_year() {
    let (dir, engine) = seed_rail_library("rail_bars");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = rail_app(&mut cx, engine);

    assert!(
        vcx.debug_bounds("timeline-rail").is_some(),
        "the rail renders"
    );
    assert!(
        vcx.debug_bounds("timeline-label").is_some(),
        "TIMELINE label"
    );
    assert!(
        vcx.debug_bounds("timeline-year-2025").is_some(),
        "2025 year"
    );
    let june = f32::from(bounds(vcx, "timeline-bar-fill-2025-06").size.width);
    let may = f32::from(bounds(vcx, "timeline-bar-fill-2025-05").size.width);
    let march = f32::from(bounds(vcx, "timeline-bar-fill-2024-03").size.width);
    assert!(
        (may / march - 8.0 / 24.0).abs() < 0.05 && (june / march - 20.0 / 24.0).abs() < 0.05,
        "bar width is proportional to the month count: jun {june}, may {may}, mar {march}"
    );
    // Newest first: June 2025 is above March 2024.
    assert!(
        bounds(vcx, "timeline-bar-2025-06").origin.y < bounds(vcx, "timeline-bar-2024-03").origin.y,
        "months run newest first"
    );

    click(vcx, "timeline-year-2024");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.active_year, None, "the rail scrolls; it never filters");
        assert_eq!(
            app.grid_top_row(),
            28 / COLS,
            "the first 2024 item (offset 28) is on the top row"
        );
        let first = app.get_active_slice(28..29);
        assert_eq!(first[0].year, 2024, "item 28 is the newest 2024 item");
        assert_eq!(app.get_active_slice(27..28)[0].year, 2025);
    });

    // A month bar scrolls to that month: May 2025 starts at item 20.
    click(vcx, "timeline-bar-2025-05");
    view.update(vcx, |app, _| {
        assert_eq!(app.grid_top_row(), 20 / COLS, "May 2025 (offset 20) on top");
    });

    // To the bottom: only March 2024 (rows 9-14) is on screen.
    view.update(vcx, |app, cx| {
        let rows = app.rail.total.div_ceil(COLS);
        app.grid_state
            .as_ref()
            .unwrap()
            .scroll_handle
            .scroll_to_item_strict(rows - 1, gpui::ScrollStrategy::Top);
        cx.notify();
    });
    render_ui(vcx);
    render_ui(vcx);

    // The click left the pointer on the rail; move it onto the grid first.
    let grid = bounds(vcx, "media-grid-surface").center();
    vcx.simulate_mouse_move(grid, None, gpui::Modifiers::default());
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("timeline-bubble").is_none(),
        "no bubble while the pointer is off the rail"
    );
    let bar = bounds(vcx, "timeline-bar-2024-03").center();
    vcx.simulate_mouse_move(bar, None, gpui::Modifiers::default());
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("timeline-bubble").is_some(),
        "hovering the rail shows the bubble"
    );
    view.update(vcx, |app, _| {
        assert_eq!(
            app.timeline_bubble_label().as_deref(),
            Some("MAR 2024"),
            "the bubble names the months on screen"
        );
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn rail_hides_outside_the_unfiltered_date_view() {
    let (dir, engine) = seed_rail_library("rail_hides");
    let (album, _) = super::albums::album_with_members(&engine, "Trip", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = rail_app(&mut cx, engine);
    let shown = |vcx: &mut gpui::VisualTestContext| vcx.debug_bounds("timeline-rail").is_some();

    assert!(shown(vcx), "All Media, date sort, no query: the rail shows");

    view.update(vcx, |app, cx| {
        app.select_album(crate::views::sidebar::AlbumSelection::Album(album.id), cx)
    });
    render_ui(vcx);
    assert!(!shown(vcx), "a user album has no rail");

    view.update(vcx, |app, cx| app.set_search_query("city:Lisbon", cx));
    render_ui(vcx);
    assert!(!shown(vcx), "a query has no rail");

    view.update(vcx, |app, cx| {
        app.select_album(crate::views::sidebar::AlbumSelection::AllLibrary, cx);
        app.set_sort_field(lumina_core::SortField::FileName, cx);
    });
    render_ui(vcx);
    assert!(!shown(vcx), "name sort has no rail");

    view.update(vcx, |app, cx| {
        app.set_sort_field(lumina_core::SortField::DateTaken, cx)
    });
    render_ui(vcx);
    assert!(shown(vcx), "back to date sort: the rail shows");

    view.update(vcx, |app, cx| app.set_active_year(Some(2024), cx));
    render_ui(vcx);
    assert!(!shown(vcx), "a year filter (palette) has no rail");

    view.update(vcx, |app, cx| {
        app.select_album(crate::views::sidebar::AlbumSelection::Duplicates, cx)
    });
    render_ui(vcx);
    assert!(!shown(vcx), "Duplicates has no rail");

    view.update(vcx, |app, cx| app.show_map(cx));
    render_ui(vcx);
    assert!(!shown(vcx), "the map has no rail");

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn rail_lens_tracks_scroll() {
    let (dir, engine) = seed_rail_library("rail_lens");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = rail_app(&mut cx, engine);
    let covers = |vcx: &mut gpui::VisualTestContext, bar: &'static str| {
        let lens = bounds(vcx, "timeline-lens");
        let y = bounds(vcx, bar).center().y;
        lens.origin.y <= y && y <= lens.origin.y + lens.size.height
    };

    assert!(
        covers(vcx, "timeline-bar-2025-06"),
        "at the top the lens is on June 2025"
    );
    assert!(
        !covers(vcx, "timeline-bar-2024-03"),
        "March 2024 is not visible yet"
    );

    // To the last row: only March 2024 (rows 9-14) is on screen.
    view.update(vcx, |app, _| {
        app.grid_state
            .as_ref()
            .unwrap()
            .scroll_handle
            .scroll_to_item_strict(14, gpui::ScrollStrategy::Top);
    });
    render_ui(vcx);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.grid_top_row() >= 9, "scrolled into March 2024");
    });
    assert!(
        covers(vcx, "timeline-bar-2024-03"),
        "the lens follows to March 2024"
    );
    assert!(
        !covers(vcx, "timeline-bar-2025-06"),
        "June 2025 left the lens"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn rail_follows_ascending_sort() {
    let (dir, engine) = seed_rail_library("rail_ascending");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = rail_app(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.set_sort_direction(lumina_core::SortDirection::Ascending, cx)
    });
    render_ui(vcx);
    render_ui(vcx);

    // Oldest first: 2024-03 (24) and 2024-12 (8) come before 2025.
    click(vcx, "timeline-year-2025");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.get_active_slice(31..32)[0].year, 2024);
        assert_eq!(
            app.get_active_slice(32..33)[0].year,
            2025,
            "item 32 is the oldest 2025 item"
        );
        let top = app.grid_top_row();
        assert!(
            top <= 32 / COLS && app.grid_visible_items().contains(&32),
            "the first 2025 item (offset 32 ascending) is on screen: top row {top}"
        );
    });
    assert!(
        {
            let lens = bounds(vcx, "timeline-lens");
            let y = bounds(vcx, "timeline-bar-2025-05").center().y;
            lens.origin.y <= y && y <= lens.origin.y + lens.size.height
        },
        "the lens is on May 2025, the first 2025 month ascending"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn rail_lens_shows_for_a_library_within_one_row() {
    let (dir, engine) = seed_library("rail_tiny", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = rail_app(&mut cx, engine);
    let month = view.update(vcx, |app, _| app.rail.months[0]);
    let bar = leak(format!("timeline-bar-{}-{:02}", month.year, month.month));
    let lens = bounds(vcx, "timeline-lens");
    let y = bounds(vcx, bar).center().y;
    assert!(
        lens.origin.y <= y && y <= lens.origin.y + lens.size.height,
        "3 items in one row still get a lens over their month"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// 30 years (1997-2026), one item each, newest first: year Y is item
/// 2026 - Y. More years than the short test window's rail fits.
fn seed_many_years(
    prefix: &str,
) -> (
    std::path::PathBuf,
    std::sync::Arc<lumina_core::LuminaEngine>,
) {
    let (dir, engine) = seed_library_undecodable(prefix, 30);
    let items = engine.get_item_slice(0..30);
    let changes: Vec<_> = items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let year = 2026 - i as i32;
            let ms = chrono::NaiveDate::from_ymd_opt(year, 6, 15)
                .unwrap()
                .and_hms_opt(12, 0, 0)
                .unwrap()
                .and_utc()
                .timestamp_millis();
            (item.id, ms)
        })
        .collect();
    engine.set_timestamps(&changes).unwrap();
    (dir, engine)
}

#[core::prelude::v1::test]
fn rail_collapses_older_years_and_stays_inside_its_bounds() {
    let (dir, engine) = seed_many_years("rail_older");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = rail_app(&mut cx, engine);
    vcx.simulate_resize(gpui::size(gpui::px(1280.0), gpui::px(480.0)));
    // The banner shrinks the rail; the measured height collapses more years.
    view.update(vcx, |app, cx| app.set_error("Could not save", cx));
    render_ui(vcx);
    render_ui(vcx);

    let rail = bounds(vcx, "timeline-rail");
    let older = bounds(vcx, "timeline-older");
    assert!(
        older.origin.y + older.size.height <= rail.origin.y + rail.size.height + gpui::px(0.5),
        "the older row is inside the rail: {older:?} in {rail:?}"
    );
    let status = bounds(vcx, "status-bar");
    assert!(
        older.origin.y + older.size.height <= status.origin.y,
        "no year paints over the status bar"
    );

    // The older row names the newest hidden year and jumps to it.
    let year = (1997..=2026)
        .rev()
        .find(|y| {
            vcx.debug_bounds(leak(format!("timeline-year-{y}")))
                .is_none()
        })
        .expect("some years collapse");
    assert!(
        year < 2020,
        "most recent years are drawn, {year} is the first hidden"
    );
    click(vcx, "timeline-older");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        let item = (2026 - year) as usize;
        let top = app.grid_top_row();
        assert!(
            top <= item / COLS && app.grid_visible_items().contains(&item),
            "the {year} item ({item}) is on screen: top row {top}"
        );
    });
    // Visible months are collapsed: the lens sits on the older row.
    let lens = bounds(vcx, "timeline-lens");
    let y = older.center().y;
    assert!(
        lens.origin.y <= y && y <= lens.origin.y + lens.size.height,
        "the lens clamps to the older row: {lens:?} vs {older:?}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// The lens and every drawn year label (and the "{year} ↓" row) do not
/// overlap: the lens never covers a date.
fn assert_lens_clears_labels(vcx: &mut gpui::VisualTestContext, when: &str) {
    let lens = bounds(vcx, "timeline-lens");
    let slack = gpui::px(0.5);
    let labels = (1990..=2026)
        .map(|y| leak(format!("timeline-year-{y}")))
        .chain(["timeline-older"]);
    for label in labels {
        let Some(b) = vcx.debug_bounds(label) else {
            continue;
        };
        let apart = lens.origin.x + slack >= b.origin.x + b.size.width
            || b.origin.x + slack >= lens.origin.x + lens.size.width
            || lens.origin.y + slack >= b.origin.y + b.size.height
            || b.origin.y + slack >= lens.origin.y + lens.size.height;
        assert!(apart, "{when}: the lens {lens:?} covers {label} {b:?}");
    }
}

#[core::prelude::v1::test]
fn rail_lens_never_covers_a_year_label() {
    // At the top the lens is on June 2025, right under the 2025 label.
    let (dir, engine) = seed_rail_library("rail_lens_labels");
    let mut cx = crate::tests::TestApp::single();
    let (_view, vcx) = rail_app(&mut cx, engine);
    assert_lens_clears_labels(vcx, "the first month of a year");
    let year = bounds(vcx, "timeline-year-2025");
    let bar = bounds(vcx, "timeline-bar-fill-2025-06");
    assert!(
        bar.origin.y >= year.origin.y + year.size.height,
        "bars sit on their own lines under the year label: {bar:?} vs {year:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);

    // One item per year: the screen spans many years, and so does the lens.
    let (dir, engine) = seed_many_years("rail_lens_years");
    let mut cx = crate::tests::TestApp::single();
    let (_view, vcx) = rail_app(&mut cx, engine);
    assert_lens_clears_labels(vcx, "a lens over several years");
    // On the older row the lens starts right of "{year} ↓".
    vcx.simulate_resize(gpui::size(gpui::px(1280.0), gpui::px(480.0)));
    render_ui(vcx);
    render_ui(vcx);
    click(vcx, "timeline-older");
    render_ui(vcx);
    assert_lens_clears_labels(vcx, "the older row");
    let _ = std::fs::remove_dir_all(&dir);
}
