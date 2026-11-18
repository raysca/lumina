//! The Downloads screen's sections and rows: grouping, keyboard order,
//! Move up, Retry all, Clear queue and the completed row's "…" menu. Jobs
//! are fakes, so nothing touches the network.

use super::downloads::{install, persisted, queue_app, row_id, settle, states, submit};
use super::downloads_pause::{offers, run_palette};
use super::downloads_screen::{marker, set_rows};
use super::helpers::{app_with_library, click, focus_app, keystrokes, leak, render_ui};
use crate::app::command_palette::CommandId;
use crate::app::downloads::{DownloadFilter, DownloadSurface, JobOutcome};
use crate::views::sidebar::AlbumSelection;
use crate::LuminaApp;
use gpui::{Entity, VisualTestContext};
use lumina_core::{DownloadState, MediaId, SystemAlbum};

const URLS: [&str; 7] = [
    "https://example.com/a.mp4",
    "https://example.com/b.mp4",
    "https://example.com/c.mp4",
    "https://example.com/d.mp4",
    "https://example.com/e.mp4",
    "https://example.com/f.mp4",
    "https://example.com/g.mp4",
];

fn top(vcx: &mut VisualTestContext, selector: String) -> f32 {
    let bounds = vcx
        .debug_bounds(leak(selector.clone()))
        .unwrap_or_else(|| panic!("{selector} paints"));
    bounds.origin.y.into()
}

/// Ids 1-7: running, waiting, paused, failed, done (finished first),
/// duplicate (finished later), waiting.
fn mixed_rows(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) {
    set_rows(
        view,
        vcx,
        &[
            DownloadState::Running,
            DownloadState::Waiting,
            DownloadState::Paused,
            DownloadState::Failed,
            DownloadState::Done,
            DownloadState::Duplicate,
            DownloadState::Waiting,
        ],
    );
    view.update(vcx, |app, _| {
        app.downloads.entries[4].row.finished_epoch_ms = Some(100);
        app.downloads.entries[5].row.finished_epoch_ms = Some(200);
    });
}

fn screen_sections(
    view: &Entity<LuminaApp>,
    vcx: &mut VisualTestContext,
) -> Vec<(&'static str, Vec<i64>)> {
    view.update(vcx, |app, _| {
        app.download_sections(DownloadSurface::Screen)
            .into_iter()
            .map(|(key, rows)| (key, rows.iter().map(|e| e.row.id).collect()))
            .collect()
    })
}

/// A wide window, so every row paints without scrolling.
fn widen(vcx: &mut VisualTestContext) {
    vcx.simulate_resize(gpui::size(gpui::px(1600.0), gpui::px(1200.0)));
}

#[core::prelude::v1::test]
fn downloads_screen_groups_rows() {
    let (_dir, engine, fake, mut cx) = queue_app("dls_groups");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    widen(vcx);
    mixed_rows(&view, vcx);
    view.update(vcx, |app, cx| app.show_downloads_screen(cx));
    render_ui(vcx);

    assert_eq!(
        screen_sections(&view, vcx),
        [
            ("active", vec![1]),
            ("attention", vec![3, 4]),
            ("queued", vec![2, 7]),
            ("completed", vec![6, 5]),
        ],
        "running rows are active, paused and failed need attention, waiting rows \
         queue in order, and completed rows list newest first"
    );
    // On screen: each section's header, then its rows, in that order.
    let painted = [
        "downloads-section-active",
        "downloads-screen-row-1",
        "downloads-section-attention",
        "downloads-screen-row-3",
        "downloads-screen-row-4",
        "downloads-section-queued",
        "downloads-screen-row-2",
        "downloads-screen-row-7",
        "downloads-section-completed",
        "downloads-screen-row-6",
        "downloads-screen-row-5",
    ];
    let tops: Vec<f32> = painted.iter().map(|s| top(vcx, s.to_string())).collect();
    assert!(
        tops.windows(2).all(|pair| pair[0] < pair[1]),
        "sections and rows paint top to bottom in that order: {tops:?}"
    );
    for (selector, why) in [
        ("download-pause-1", "a running row has Pause"),
        ("download-resume-3", "a paused row has Resume"),
        ("download-retry-4", "a failed row has Retry"),
        (
            "downloads-screen-up-7",
            "a queued row after the first has Move up",
        ),
        ("downloads-screen-more-6", "a completed row has the … menu"),
        ("downloads-section-attention-action", "Retry all"),
        ("downloads-section-queued-action", "Clear queue"),
        ("downloads-section-completed-action", "View all"),
    ] {
        assert!(marker(vcx, selector.into()), "{why} ({selector})");
    }
    assert!(
        !marker(vcx, "downloads-screen-up-2".into()),
        "the first queued row has nothing to move up past"
    );

    view.update(vcx, |app, cx| {
        app.set_download_filter(DownloadFilter::Failed, cx)
    });
    render_ui(vcx);
    assert_eq!(
        screen_sections(&view, vcx),
        [("attention", vec![4])],
        "the filter keeps only its rows, and empty sections hide"
    );
    assert!(!marker(vcx, "downloads-section-active".into()));
}

#[core::prelude::v1::test]
fn screen_keyboard_order_matches_visual_order() {
    let (_dir, engine, fake, mut cx) = queue_app("dls_keys");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    widen(vcx);
    mixed_rows(&view, vcx);
    view.update(vcx, |app, cx| app.show_downloads_screen(cx));
    render_ui(vcx);
    focus_app(vcx, &view);

    let order: Vec<i64> = view.update(vcx, |app, _| {
        app.download_display_order(DownloadSurface::Screen)
            .iter()
            .map(|e| e.row.id)
            .collect()
    });
    let mut by_top: Vec<(f32, i64)> = order
        .iter()
        .map(|id| (top(vcx, format!("downloads-screen-row-{id}")), *id))
        .collect();
    by_top.sort_by(|a, b| a.0.total_cmp(&b.0));
    assert_eq!(
        by_top.iter().map(|(_, id)| *id).collect::<Vec<_>>(),
        order,
        "the display order is the on-screen order"
    );

    for &expected in &order {
        keystrokes(vcx, "down");
        view.update(vcx, |app, _| {
            assert_eq!(
                app.downloads.screen_focused,
                Some(expected),
                "↓ visits rows in display order"
            );
            assert_eq!(app.downloads.focused, None, "the dropdown's focus is apart");
        });
    }
    keystrokes(vcx, "down");
    keystrokes(vcx, "up");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.downloads.screen_focused,
            Some(6),
            "↓ stops at the end, ↑ goes back"
        )
    });

    // ⌫ removes the focused row; the grid behind keeps its selection.
    keystrokes(vcx, "backspace");
    view.update(vcx, |app, _| {
        assert!(
            app.downloads.entries.iter().all(|e| e.row.id != 6),
            "⌫ removes the focused row"
        );
        assert_eq!(
            app.downloads.screen_focused,
            Some(5),
            "focus moves to the row that took its place"
        );
    });
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert_eq!(app.downloads.screen_focused, None, "Esc clears the focus")
    });

    render_ui(vcx);
    let before = view.update(vcx, |app, _| states(app));
    click(vcx, "downloads-screen-row-2");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.downloads.screen_focused,
            Some(2),
            "a click focuses the row"
        );
        assert_eq!(states(app), before, "a click on the row acts on nothing");
    });
}

#[core::prelude::v1::test]
fn move_queued_download_up_persists() {
    let (_dir, engine, fake, mut cx) = queue_app("dls_move_up");
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    widen(vcx);
    let [a, b, c, d, e, ..] = URLS;
    for url in [a, b, c, d, e] {
        submit(&view, vcx, url);
    }
    settle(vcx, &view, "three run and two wait", |app| {
        states(app)
            == [
                DownloadState::Running,
                DownloadState::Running,
                DownloadState::Running,
                DownloadState::Waiting,
                DownloadState::Waiting,
            ]
            && app.downloads.entries.iter().all(|e| e.row.id > 0)
    });
    let urls = |app: &LuminaApp| -> Vec<String> {
        app.downloads
            .entries
            .iter()
            .map(|e| e.row.url.clone())
            .collect()
    };
    let stored = || -> Vec<String> { persisted(&engine).into_iter().map(|(u, _)| u).collect() };
    let id_e = view.update(vcx, |app, _| row_id(app, e));
    view.update(vcx, |app, cx| app.show_downloads_screen(cx));
    render_ui(vcx);

    click(vcx, leak(format!("downloads-screen-up-{id_e}")));
    view.update(vcx, |app, _| {
        assert_eq!(urls(app), [a, b, c, e, d], "Move up swaps E with D at once")
    });
    settle(vcx, &view, "the swap is stored", |_| {
        stored() == [a, b, c, e, d]
    });

    // ⌥⌘↑ on the focused row: D (last on screen) moves back up.
    focus_app(vcx, &view);
    keystrokes(vcx, "down down down down down");
    let id_d = view.update(vcx, |app, _| row_id(app, d));
    view.update(vcx, |app, _| {
        assert_eq!(app.downloads.screen_focused, Some(id_d))
    });
    keystrokes(vcx, "alt-cmd-up");
    settle(vcx, &view, "⌥⌘↑ moves D up and stores it", |app| {
        urls(app) == [a, b, c, d, e] && stored() == [a, b, c, d, e]
    });
    keystrokes(vcx, "alt-cmd-up");
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        assert_eq!(
            urls(app),
            [a, b, c, d, e],
            "the first queued row stays first"
        );
        assert_eq!(
            app.downloads.screen_focused,
            Some(id_d),
            "focus follows the row"
        );
    });

    // ↵ on a running row pauses it.
    keystrokes(vcx, "up up up");
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert_eq!(
            states(app)[0],
            DownloadState::Paused,
            "↵ pauses a running row"
        )
    });
}

#[core::prelude::v1::test]
fn retry_all_and_clear_queue() {
    let (_dir, engine, fake, mut cx) = queue_app("dls_retry_clear");
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    let [a, b, c, d, e, f, g] = URLS;
    for url in [a, b, c, d, e] {
        submit(&view, vcx, url);
    }
    settle(vcx, &view, "A, B and C run", |app| {
        states(app)[..3] == [DownloadState::Running; 3]
            && app.downloads.entries.iter().all(|e| e.row.id > 0)
    });
    fake.finish(a, JobOutcome::Failed("This video is private.".into()));
    fake.finish(b, JobOutcome::MissingHelper);
    for url in [f, g] {
        submit(&view, vcx, url);
    }
    settle(vcx, &view, "A and B failed, F and G wait", |app| {
        states(app)
            == [
                DownloadState::Failed,
                DownloadState::Failed,
                DownloadState::Running,
                DownloadState::Running,
                DownloadState::Running,
                DownloadState::Waiting,
                DownloadState::Waiting,
            ]
            && app.downloads.entries.iter().all(|e| e.row.id > 0)
    });
    // A failed row says what went wrong and when it was last tried.
    view.update(vcx, |app, cx| app.show_downloads_screen(cx));
    render_ui(vcx);
    let tried = view.update(vcx, |app, _| {
        app.downloads.entries[0]
            .row
            .finished_epoch_ms
            .expect("a failure records when it was tried")
    });
    let clock = {
        use chrono::TimeZone;
        chrono::Local
            .timestamp_millis_opt(tried)
            .single()
            .expect("a valid local time")
            .format("%H:%M")
            .to_string()
    };
    assert!(
        marker(
            vcx,
            format!("downloads-row-meta-text:This video is private. · last tried {clock}")
        ),
        "the failed row's meta reads its message and last tried {clock}"
    );

    let retry = CommandId::RetryFailedDownloads;
    assert!(
        offers(&view, vcx, retry, "retry failed downloads"),
        "a failure that can be retried is offered in ⌘K"
    );
    view.update(vcx, |app, cx| app.show_downloads_screen(cx));
    render_ui(vcx);
    click(vcx, "downloads-section-attention-action");
    let state_of = |app: &LuminaApp, url: &str| {
        app.downloads
            .entries
            .iter()
            .find(|e| e.row.url == url)
            .map(|e| e.row.state)
    };
    settle(vcx, &view, "Retry all requeues A", |app| {
        state_of(app, a) == Some(DownloadState::Waiting)
    });
    view.update(vcx, |app, _| {
        assert_eq!(
            state_of(app, b),
            Some(DownloadState::Failed),
            "Retry all leaves a failure that can't be retried"
        )
    });
    assert!(
        !offers(&view, vcx, retry, "retry failed downloads"),
        "nothing left to retry"
    );

    render_ui(vcx);
    click(vcx, "downloads-section-queued-action");
    view.update(vcx, |app, _| {
        assert_eq!(
            states(app),
            [
                DownloadState::Failed,
                DownloadState::Running,
                DownloadState::Running,
                DownloadState::Running,
            ],
            "Clear queue removes only waiting rows"
        )
    });
    settle(vcx, &view, "the clear is stored", |_| {
        persisted(&engine).len() == 4
    });
    assert!(
        persisted(&engine)
            .iter()
            .all(|(_, s)| *s != DownloadState::Waiting),
        "no waiting row stays stored"
    );

    // ⌘K Retry Failed Downloads runs the same action.
    fake.finish(c, JobOutcome::Failed("Connection lost.".into()));
    settle(vcx, &view, "C failed", |app| {
        state_of(app, c) == Some(DownloadState::Failed)
    });
    run_palette(&view, vcx, "retry failed downloads");
    settle(vcx, &view, "the palette requeues C", |app| {
        matches!(
            state_of(app, c),
            Some(DownloadState::Waiting | DownloadState::Running)
        )
    });
}

/// Installs `sql` (a trigger that makes some write fail) in the test
/// library's catalog before the app opens it.
fn inject(dir: &std::path::Path, sql: &str) {
    rusqlite::Connection::open(dir.join("catalog.db"))
        .expect("catalog must open")
        .execute_batch(sql)
        .expect("trigger must install");
}

/// Queues `urls`: the first three run, the rest wait, all stored.
fn queue_urls(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, urls: &[&str]) {
    for url in urls {
        submit(view, vcx, url);
    }
    settle(vcx, view, "three run, the rest wait, all stored", |app| {
        let running = states(app)
            .iter()
            .filter(|s| **s == DownloadState::Running)
            .count();
        app.downloads.entries.len() == urls.len()
            && running == 3
            && app.downloads.entries.iter().all(|e| e.row.id > 0)
    });
}

fn listed(app: &LuminaApp) -> Vec<(String, i64)> {
    app.downloads
        .entries
        .iter()
        .map(|e| (e.row.url.clone(), e.row.added_epoch_ms))
        .collect()
}

fn stored_urls(engine: &lumina_core::LuminaEngine) -> Vec<String> {
    persisted(engine).into_iter().map(|(u, _)| u).collect()
}

#[core::prelude::v1::test]
fn move_up_write_failure_reverts_and_shows_banner() {
    let (dir, engine, fake, mut cx) = queue_app("dls_move_up_fail");
    inject(
        &dir,
        "CREATE TRIGGER fail_move BEFORE UPDATE OF added_epoch_ms ON download_queue
         BEGIN SELECT RAISE(ABORT, 'injected'); END;",
    );
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    widen(vcx);
    let [a, b, c, d, e, ..] = URLS;
    queue_urls(&view, vcx, &[a, b, c, d, e]);
    let before = view.update(vcx, |app, _| listed(app));
    view.update(vcx, |app, cx| app.show_downloads_screen(cx));
    render_ui(vcx);
    focus_app(vcx, &view);
    // Focus E, last on screen, and move it up.
    keystrokes(vcx, "down down down down down");
    keystrokes(vcx, "alt-cmd-up");
    settle(vcx, &view, "the failed move is reported", |app| {
        app.error_message.is_some()
    });
    view.update(vcx, |app, _| {
        assert_eq!(
            listed(app),
            before,
            "a move that couldn't be saved is undone: order and queue times"
        );
        assert!(!app.downloads.moving, "a failed move frees the next one");
    });
    assert_eq!(
        stored_urls(&engine),
        [a, b, c, d, e],
        "the stored order is untouched"
    );
}

#[core::prelude::v1::test]
fn second_move_up_waits_for_the_first() {
    let (_dir, engine, fake, mut cx) = queue_app("dls_move_up_twice");
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    let [a, b, c, d, e, f, ..] = URLS;
    queue_urls(&view, vcx, &[a, b, c, d, e, f]);
    view.update(vcx, |app, cx| {
        let (id_e, id_f) = (row_id(app, e), row_id(app, f));
        app.move_download_up(id_f, cx);
        // Pressed again before the first move is stored.
        app.move_download_up(id_e, cx);
        let urls: Vec<String> = listed(app).into_iter().map(|(u, _)| u).collect();
        assert_eq!(
            urls,
            [a, b, c, d, f, e],
            "a move while another is being stored does nothing"
        );
    });
    settle(vcx, &view, "the first move is stored", |app| {
        !app.downloads.moving && stored_urls(&engine) == [a, b, c, d, f, e]
    });
    view.update(vcx, |app, cx| {
        let id_f = row_id(app, f);
        app.move_download_up(id_f, cx);
    });
    settle(vcx, &view, "once stored, the next move goes ahead", |app| {
        !app.downloads.moving && stored_urls(&engine) == [a, b, c, f, d, e]
    });
    view.update(vcx, |app, _| {
        let urls: Vec<String> = listed(app).into_iter().map(|(u, _)| u).collect();
        assert_eq!(urls, [a, b, c, f, d, e], "memory and catalog agree");
    });
}

#[core::prelude::v1::test]
fn clear_queue_partial_failure_restores_only_that_row() {
    let (dir, engine, fake, mut cx) = queue_app("dls_clear_fail");
    let [a, b, c, d, e, f, g] = URLS;
    inject(
        &dir,
        &format!(
            "CREATE TRIGGER fail_clear BEFORE DELETE ON download_queue
             WHEN OLD.url = '{f}'
             BEGIN SELECT RAISE(ABORT, 'injected'); END;"
        ),
    );
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    widen(vcx);
    queue_urls(&view, vcx, &[a, b, c, d, e, f, g]);
    view.update(vcx, |app, cx| app.show_downloads_screen(cx));
    render_ui(vcx);
    // ponytail: `click` runs until parked, so the background removal has
    // already finished; only the end state is observable here.
    click(vcx, "downloads-section-queued-action");
    settle(vcx, &view, "the failed removal is reported", |app| {
        app.error_message.is_some()
    });
    view.update(vcx, |app, _| {
        let urls: Vec<String> = listed(app).into_iter().map(|(u, _)| u).collect();
        assert_eq!(
            urls,
            [a, b, c, f],
            "only the row that couldn't be removed comes back, in queue order"
        );
        assert_eq!(
            states(app)[3],
            DownloadState::Waiting,
            "it comes back waiting"
        );
    });
    assert_eq!(
        stored_urls(&engine),
        [a, b, c, f],
        "the other waiting rows stay removed from the catalog"
    );
}

#[core::prelude::v1::test]
fn completed_row_menu() {
    let (_dir, engine, fake, mut cx) = queue_app("dls_menu");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    widen(vcx);
    set_rows(&view, vcx, &[DownloadState::Done, DownloadState::Duplicate]);
    view.update(vcx, |app, _| {
        for (n, entry) in app.downloads.entries.iter_mut().enumerate() {
            entry.row.media_id = Some(MediaId(1));
            entry.row.in_library = true;
            entry.row.finished_epoch_ms = Some(100 + n as i64);
        }
    });
    view.update(vcx, |app, cx| app.show_downloads_screen(cx));
    render_ui(vcx);
    focus_app(vcx, &view);

    click(vcx, "downloads-screen-more-2");
    for item in ["show", "again", "remove"] {
        assert!(
            marker(vcx, format!("downloads-row-menu-{item}")),
            "a duplicate row's menu has {item}"
        );
    }
    keystrokes(vcx, "backspace left");
    view.update(vcx, |app, _| {
        assert_eq!(app.downloads.entries.len(), 2, "the open menu absorbs ⌫");
        assert_eq!(app.downloads.filter, DownloadFilter::All, "and ←");
        assert!(app.downloads.row_menu.is_some());
    });
    keystrokes(vcx, "escape");
    render_ui(vcx);
    assert!(
        !marker(vcx, "downloads-row-menu".into()),
        "Esc closes the menu"
    );

    click(vcx, "downloads-screen-more-2");
    click(vcx, "downloads-row-menu-backdrop");
    assert!(
        !marker(vcx, "downloads-row-menu".into()),
        "a click outside closes the menu"
    );
    view.update(vcx, |app, _| assert_eq!(app.downloads.entries.len(), 2));

    // A done row: no Download Again; ↓ ↵ runs Remove from List.
    click(vcx, "downloads-screen-more-1");
    assert!(!marker(vcx, "downloads-row-menu-again".into()));
    keystrokes(vcx, "down down enter");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.downloads
                .entries
                .iter()
                .map(|e| e.row.id)
                .collect::<Vec<_>>(),
            [2],
            "Remove from List removes the row"
        );
        assert!(app.downloads.row_menu.is_none());
    });

    render_ui(vcx);
    click(vcx, "downloads-screen-more-2");
    click(vcx, "downloads-row-menu-show");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::System(SystemAlbum::Downloaded),
            "Show in Library opens the Downloaded album"
        );
        assert!(app.downloads.row_menu.is_none());
    });
}
