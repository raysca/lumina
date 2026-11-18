//! The downloads popover's rows: Active and Earlier groups, their buttons,
//! copy, and keyboard scrolling across the divider.

use super::downloads::{install, queue_app, row_id};
use super::downloads_panel::queue;
use super::helpers::{app_with_library, focus_app, keystrokes, leak, render_ui};
use crate::app::downloads::{DownloadEntry, DownloadSurface};
use crate::views::downloads_popover_rows::finished_label;
use crate::LuminaApp;
use lumina_core::DownloadState;

const A: &str = "https://example.com/a.mp4";
const B: &str = "https://example.com/b.mp4";
const DAY_MS: i64 = 86_400_000;

/// Adds a row in `state` with `id`, cloned from the first row.
fn add(app: &mut LuminaApp, id: i64, state: DownloadState, finished_epoch_ms: Option<i64>) {
    let mut row = app.downloads.entries[0].row.clone();
    row.id = id;
    row.state = state;
    row.in_library = state == DownloadState::Done;
    row.can_retry = true;
    row.finished_epoch_ms = finished_epoch_ms;
    app.downloads.entries.push(DownloadEntry::for_test(row));
}

fn order(app: &LuminaApp) -> Vec<i64> {
    app.download_display_order(DownloadSurface::Panel)
        .iter()
        .map(|e| e.row.id)
        .collect()
}

#[core::prelude::v1::test]
fn popover_groups_active_and_earlier() {
    let (_dir, engine, fake, mut cx) = queue_app("dlp_groups");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    queue(&view, vcx, &[A, B]);
    let old = chrono::Utc::now().timestamp_millis() - 3 * DAY_MS;
    let (a, b) = view.update(vcx, |app, cx| {
        // Queue order mixes the groups; the popover sorts them.
        add(app, 6004, DownloadState::Done, Some(old));
        add(app, 6003, DownloadState::Failed, None);
        add(app, 6005, DownloadState::Duplicate, Some(old + 2 * DAY_MS));
        add(app, 6006, DownloadState::Done, None);
        add(app, 6002, DownloadState::Waiting, None);
        add(app, 6001, DownloadState::Paused, None);
        app.toggle_downloads_panel(cx);
        (row_id(app, A), row_id(app, B))
    });
    render_ui(vcx);
    // Earlier: newest finish first, across done and duplicate rows.
    let expected = vec![a, b, 6001, 6002, 6003, 6005, 6004, 6006];
    view.update(vcx, |app, _| {
        assert_eq!(
            order(app),
            expected,
            "Active (running, paused, waiting, failed), then Earlier, newest first"
        )
    });

    let tops: Vec<_> = expected
        .iter()
        .map(|id| {
            vcx.debug_bounds(leak(format!("download-row-{id}")))
                .unwrap_or_else(|| panic!("row {id} paints"))
        })
        .collect();
    assert!(
        tops.windows(2).all(|w| w[0].bottom() <= w[1].top()),
        "rows paint top to bottom in display order: {tops:?}"
    );
    let divider = vcx
        .debug_bounds("downloads-divider")
        .expect("a divider separates the groups");
    assert!(
        tops[4].bottom() <= divider.top() && divider.bottom() <= tops[5].top(),
        "the divider sits between the last active and the first earlier row"
    );

    for selector in [
        format!("download-pause-{a}"),
        "download-resume-6001".to_string(),
        "download-retry-6003".to_string(),
        "download-reveal-6004".to_string(),
        "download-again-6005".to_string(),
        "download-meta-text:6006:Added to library".to_string(),
        format!("download-remove-{a}"),
    ] {
        assert!(
            vcx.debug_bounds(leak(selector.clone())).is_some(),
            "{selector} paints"
        );
    }
    assert!(
        vcx.debug_bounds("download-reveal-6005").is_none(),
        "a duplicate row offers Download Again instead of reveal"
    );
    assert!(
        vcx.debug_bounds("download-meta-text:6001:Paused · resumes from the start")
            .is_some(),
        "a paused row says Resume starts over, with no bytes"
    );
    let done = format!(
        "download-meta-text:6004:Added to library · {}",
        finished_label(old)
    );
    assert!(
        vcx.debug_bounds(leak(done.clone())).is_some(),
        "a done row reads {done}"
    );

    focus_app(vcx, &view);
    let mut visited = vec![view.update(vcx, |app, _| app.downloads.focused.unwrap())];
    for _ in 1..expected.len() {
        keystrokes(vcx, "down");
        visited.push(view.update(vcx, |app, _| app.downloads.focused.unwrap()));
    }
    assert_eq!(visited, expected, "↓ visits rows in visual order");
}

#[core::prelude::v1::test]
fn finished_label_reads_a_date_after_a_day() {
    let now = chrono::Utc::now().timestamp_millis();
    assert_eq!(finished_label(now - 5 * 60_000), "5 min ago");
    let old = now - 3 * DAY_MS;
    let label = finished_label(old);
    assert!(
        !label.contains("ago"),
        "older than a day reads as a date: {label}"
    );
    use chrono::TimeZone;
    let date = chrono::Local.timestamp_millis_opt(old).unwrap();
    assert!(
        label.contains(&date.format("%b").to_string()),
        "the date names the month: {label}"
    );
}

#[core::prelude::v1::test]
fn popover_keyboard_scrolls_focused_row() {
    let (_dir, engine, fake, mut cx) = queue_app("dlp_popover_scroll");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    queue(&view, vcx, &[A]);
    view.update(vcx, |app, cx| {
        for n in 0..14 {
            add(app, 7_000 + n, DownloadState::Failed, None);
        }
        for n in 14..29 {
            add(app, 7_000 + n, DownloadState::Done, None);
        }
        app.toggle_downloads_panel(cx);
    });
    render_ui(vcx);
    focus_app(vcx, &view);
    let rows = view.update(vcx, |app, _| order(app));
    assert_eq!(rows.len(), 30, "30 rows across both groups");
    let check = |vcx: &mut gpui::VisualTestContext, id: i64| {
        render_ui(vcx);
        render_ui(vcx);
        let body = vcx
            .debug_bounds("downloads-panel-body")
            .expect("the scroll body paints");
        let row = vcx
            .debug_bounds(leak(format!("download-row-{id}")))
            .expect("the focused row paints");
        assert!(
            row.top() >= body.top() && row.bottom() <= body.bottom(),
            "focused row {id} is scrolled into the body: {row:?} in {body:?}"
        );
    };
    // Down to the first Earlier row (past the divider and head), then on.
    for _ in 1..16 {
        keystrokes(vcx, "down");
    }
    view.update(vcx, |app, _| {
        assert_eq!(app.downloads.focused, Some(rows[15]))
    });
    check(vcx, rows[15]);
    for _ in 16..30 {
        keystrokes(vcx, "down");
    }
    view.update(vcx, |app, _| {
        assert_eq!(app.downloads.focused, rows.last().copied())
    });
    check(vcx, *rows.last().unwrap());
    for _ in 1..30 {
        keystrokes(vcx, "up");
    }
    view.update(vcx, |app, _| {
        assert_eq!(app.downloads.focused, Some(rows[0]))
    });
    check(vcx, rows[0]);
}

/// Whether `row` paints inside the scroll body `body`.
fn shown_in(vcx: &mut gpui::VisualTestContext, body: &'static str, row: String) -> bool {
    render_ui(vcx);
    render_ui(vcx);
    let body = vcx.debug_bounds(body).expect("the scroll body paints");
    vcx.debug_bounds(leak(row))
        .is_some_and(|row| row.top() >= body.top() && row.bottom() <= body.bottom())
}

/// Adding a link that is already listed scrolls its row into view and moves
/// the keyboard focus to it, in the dropdown and on the Downloads screen.
#[core::prelude::v1::test]
fn re_adding_a_listed_link_reveals_its_row() {
    let (_dir, engine, fake, mut cx) = queue_app("dlp_reveal");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    queue(&view, vcx, &[A]);
    let a = view.update(vcx, |app, cx| {
        let a = row_id(app, A);
        app.pause_download(a, cx);
        a
    });
    // Running rows list before paused ones, so A sits below the fold.
    view.update(vcx, |app, cx| {
        for n in 0..25 {
            add(app, 8_000 + n, DownloadState::Running, None);
        }
        app.toggle_downloads_panel(cx);
    });
    focus_app(vcx, &view);
    let row = format!("download-row-{a}");
    assert!(
        !shown_in(vcx, "downloads-panel-body", row.clone()),
        "precondition: A starts below the fold"
    );
    view.update(vcx, |app, cx| {
        app.enqueue_download(A.to_string(), false, None, cx)
    });
    assert!(
        shown_in(vcx, "downloads-panel-body", row),
        "the dropdown scrolls the existing row into view"
    );
    view.update(vcx, |app, _| {
        assert_eq!(app.downloads.highlight, Some(a));
        assert_eq!(
            app.downloads.focused,
            Some(a),
            "the keyboard focus moves to it"
        );
    });

    // The Downloads screen, its keyboard focus on the top row.
    view.update(vcx, |app, cx| {
        app.show_downloads_screen(cx);
        app.downloads.screen_focused = Some(8_000);
    });
    let row = format!("downloads-screen-row-{a}");
    assert!(
        !shown_in(vcx, "downloads-list", row.clone()),
        "precondition: A starts below the screen's fold"
    );
    view.update(vcx, |app, cx| {
        app.enqueue_download(A.to_string(), false, None, cx)
    });
    assert!(
        shown_in(vcx, "downloads-list", row),
        "the screen scrolls the existing row into view"
    );
    view.update(vcx, |app, _| {
        assert_eq!(
            app.downloads.screen_focused,
            Some(a),
            "the screen's focus moves to it"
        )
    });
}
