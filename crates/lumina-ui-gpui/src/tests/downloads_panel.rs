//! The downloads toolbar button and dropdown: badge, red dot, mouse and
//! keyboard paths, Clear finished, long rows.

use super::downloads::{imported, install, persisted, queue_app, row_id, settle, states, submit};
use super::ui_automation::{app_with_library, click, focus_app, keystrokes, render_ui};
use crate::app::downloads::JobOutcome;
use crate::LuminaApp;
use gpui::{Entity, VisualTestContext};
use lumina_core::DownloadState;

fn leak(selector: String) -> &'static str {
    Box::leak(selector.into_boxed_str())
}

fn running(app: &LuminaApp) -> usize {
    states(app)
        .into_iter()
        .filter(|s| *s == DownloadState::Running)
        .count()
}

/// Queues `urls` and waits until they are stored and as many as fit run.
pub(super) fn queue(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, urls: &[&str]) {
    for (n, url) in urls.iter().enumerate() {
        submit(view, vcx, url);
        settle(vcx, view, "the row is stored", |app| {
            app.downloads.entries.len() == n + 1
                && app.downloads.entries.iter().all(|e| e.row.id > 0)
        });
    }
    let expected = urls.len().min(crate::config::DEFAULT_DOWNLOAD_CONCURRENCY);
    settle(vcx, view, "the downloads run", |app| {
        running(app) == expected
    });
    render_ui(vcx);
}

const A: &str = "https://example.com/a.mp4";
const B: &str = "https://example.com/b.mp4";
const C: &str = "https://example.com/c.mp4";

#[core::prelude::v1::test]
fn toolbar_icon_hidden_without_downloads() {
    let (_dir, engine, _fake, mut cx) = queue_app("dlp_hidden");
    let (_view, vcx) = app_with_library(&mut cx, engine);
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("downloads-toolbar-btn").is_none(),
        "a library that never downloaded keeps today's toolbar"
    );
}

#[core::prelude::v1::test]
fn badge_counts_active_rows() {
    let (_dir, engine, fake, mut cx) = queue_app("dlp_badge");
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    queue(&view, vcx, &[A, B]);
    assert!(vcx.debug_bounds("downloads-toolbar-btn").is_some());
    assert!(vcx.debug_bounds("downloads-badge").is_some());
    view.update(vcx, |app, _| assert_eq!(app.running_or_waiting_count(), 2));

    fake.finish(A, imported(&engine));
    fake.finish(B, imported(&engine));
    settle(vcx, &view, "both finish", |app| {
        app.running_or_waiting_count() == 0
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("downloads-badge").is_none(),
        "no badge once all are done"
    );
    view.update(vcx, |app, _| {
        assert_eq!(
            app.running_or_waiting_count(),
            0,
            "no badge once all are done"
        );
        assert_eq!(app.downloads.entries.len(), 2, "finished rows stay listed");
    });
    view.update(vcx, |app, _| {
        app.downloads.entries[0].row.state = DownloadState::Paused;
        assert_eq!(
            app.download_badge_count(),
            1,
            "after reopening, paused rows show there is unfinished work"
        );
        assert_eq!(
            app.running_or_waiting_count(),
            0,
            "but quitting needn't ask about them"
        );
    });
}

#[core::prelude::v1::test]
fn clicking_icon_toggles_panel() {
    let (_dir, engine, fake, mut cx) = queue_app("dlp_click");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    queue(&view, vcx, &[A]);
    click(vcx, "downloads-toolbar-btn");
    view.update(vcx, |app, _| {
        assert!(app.downloads.open, "a click opens the list")
    });
    assert!(vcx.debug_bounds("downloads-panel").is_some());
    click(vcx, "downloads-toolbar-btn");
    view.update(vcx, |app, _| {
        assert!(!app.downloads.open, "a second click closes it")
    });
}

#[core::prelude::v1::test]
fn backdrop_click_closes_panel() {
    let (_dir, engine, fake, mut cx) = queue_app("dlp_backdrop");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    queue(&view, vcx, &[A]);
    view.update(vcx, |app, cx| app.toggle_downloads_panel(cx));
    render_ui(vcx);
    super::ui_automation::click_point(vcx, 40.0, 400.0);
    view.update(vcx, |app, _| {
        assert!(!app.downloads.open, "a click outside closes the list")
    });
}

#[core::prelude::v1::test]
fn shortcut_toggles_panel() {
    let (_dir, engine, fake, mut cx) = queue_app("dlp_shortcut");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    queue(&view, vcx, &[A]);
    focus_app(vcx, &view);
    keystrokes(vcx, "alt-secondary-l");
    view.update(vcx, |app, _| {
        assert!(app.downloads.open, "⌥⌘L opens the list")
    });
    keystrokes(vcx, "alt-secondary-l");
    view.update(vcx, |app, _| {
        assert!(!app.downloads.open, "⌥⌘L closes it again")
    });
}

#[core::prelude::v1::test]
fn palette_toggles_panel() {
    let (_dir, engine, fake, mut cx) = queue_app("dlp_palette");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    queue(&view, vcx, &[A]);
    view.update(vcx, |app, cx| {
        app.open_command_palette(cx);
        app.command_query = "show downloads".to_string();
        cx.notify();
    });
    view.update(vcx, |app, cx| app.execute_selected_command(cx));
    view.update(vcx, |app, _| {
        assert!(
            app.downloads.open,
            "the palette has a keyboard path to the list"
        )
    });
}

#[core::prelude::v1::test]
fn remove_button_cancels_running_row() {
    let (_dir, engine, fake, mut cx) = queue_app("dlp_remove");
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    queue(&view, vcx, &[A]);
    view.update(vcx, |app, cx| app.toggle_downloads_panel(cx));
    render_ui(vcx);
    let id = view.update(vcx, |app, _| row_id(app, A));
    click(vcx, leak(format!("download-remove-{id}")));
    view.update(vcx, |app, _| {
        assert!(app.downloads.entries.is_empty(), "✕ removes the row");
        assert!(
            !app.downloads.open,
            "removing the last row closes the list, as Clear finished does"
        );
    });
    assert!(
        vcx.debug_bounds("downloads-backdrop").is_none(),
        "no invisible backdrop is left over the window"
    );
    settle(vcx, &view, "the removal is stored", |_| {
        persisted(&engine).is_empty()
    });
}

#[core::prelude::v1::test]
fn keyboard_arrows_enter_backspace_escape() {
    let (_dir, engine, fake, mut cx) = queue_app("dlp_keys");
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    queue(&view, vcx, &[A, B]);
    fake.finish(A, JobOutcome::Failed("The connection dropped".into()));
    settle(vcx, &view, "A fails", |app| {
        states(app).contains(&DownloadState::Failed)
    });
    focus_app(vcx, &view);
    keystrokes(vcx, "alt-secondary-l");
    let (a, b) = view.update(vcx, |app, _| (row_id(app, A), row_id(app, B)));
    view.update(vcx, |app, _| {
        assert_eq!(
            app.downloads.focused,
            Some(b),
            "focus starts on the first row"
        );
    });

    // Downloading (B) is listed above Failed (A).
    keystrokes(vcx, "down");
    view.update(vcx, |app, _| assert_eq!(app.downloads.focused, Some(a)));
    keystrokes(vcx, "up");
    view.update(vcx, |app, _| assert_eq!(app.downloads.focused, Some(b)));
    keystrokes(vcx, "down");
    keystrokes(vcx, "enter");
    settle(vcx, &view, "⏎ retries the failed row", |app| {
        running(app) == 2
    });

    keystrokes(vcx, "backspace");
    view.update(vcx, |app, _| {
        assert!(
            app.downloads.entries.iter().all(|e| e.row.id != a),
            "⌫ cancels and removes the focused row"
        );
    });
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(!app.downloads.open, "Esc closes the list")
    });
}

#[core::prelude::v1::test]
fn panel_absorbs_grid_shortcuts() {
    let (_dir, engine, fake, mut cx) = queue_app("dlp_absorb");
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    queue(&view, vcx, &[A]);
    view.update(vcx, |app, cx| {
        let item = app.get_active_slice(0..1).remove(0);
        app.select_item_at(item, 0, cx);
    });
    // The panel's own row takes ⌫; with no rows focused it must still not
    // reach the grid.
    view.update(vcx, |app, cx| {
        app.toggle_downloads_panel(cx);
        app.downloads.focused = None;
    });
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-backspace");
    keystrokes(vcx, "backspace");
    view.update(vcx, |app, _| {
        assert!(app.selected_item.is_some(), "the selection is untouched");
        assert!(app.delete_confirm_state.is_none(), "no delete prompt opens");
    });
    assert_eq!(
        engine.query_system_album_count(lumina_core::SystemAlbum::RecentlyDeleted),
        0,
        "keys in the open list never trash grid items"
    );
}

#[core::prelude::v1::test]
fn opening_panel_clears_failure_dot() {
    let (_dir, engine, fake, mut cx) = queue_app("dlp_dot");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    queue(&view, vcx, &[A]);
    fake.finish(A, JobOutcome::Failed("This video is private".into()));
    settle(vcx, &view, "A fails", |app| app.downloads.unseen_failure);
    render_ui(vcx);
    assert!(vcx.debug_bounds("downloads-failure-dot").is_some());
    view.update(vcx, |app, cx| app.toggle_downloads_panel(cx));
    view.update(vcx, |app, _| {
        assert!(
            !app.downloads.unseen_failure,
            "opening the list clears the red dot"
        )
    });
}

#[core::prelude::v1::test]
fn clear_finished_keeps_active_rows() {
    let (_dir, engine, fake, mut cx) = queue_app("dlp_clear");
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    queue(&view, vcx, &[A, B, C]);
    fake.finish(A, imported(&engine));
    fake.finish(C, JobOutcome::Failed("The connection dropped".into()));
    settle(vcx, &view, "A is done and C failed", |app| {
        states(app).contains(&DownloadState::Done) && states(app).contains(&DownloadState::Failed)
    });
    settle(vcx, &view, "both outcomes are stored", |_| {
        persisted(&engine).len() == 3
            && persisted(&engine)
                .iter()
                .any(|(_, s)| *s == DownloadState::Failed)
    });
    view.update(vcx, |app, cx| app.toggle_downloads_panel(cx));
    render_ui(vcx);
    click(vcx, "downloads-clear-finished");
    view.update(vcx, |app, _| {
        assert_eq!(
            states(app),
            [DownloadState::Running, DownloadState::Failed],
            "the Earlier head's Clear removes only Earlier rows; the failed row above stays for Retry"
        );
    });
    settle(vcx, &view, "the clear is stored", |_| {
        persisted(&engine)
            == [
                (B.to_string(), DownloadState::Running),
                (C.to_string(), DownloadState::Failed),
            ]
    });
}

#[core::prelude::v1::test]
fn dropdown_truncates_and_scrolls() {
    let (_dir, engine, fake, mut cx) = queue_app("dlp_long");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    queue(&view, vcx, &[A]);
    view.update(vcx, |app, cx| {
        let template = app.downloads.entries[0].row.clone();
        for n in 0..40 {
            let mut row = template.clone();
            row.id = 1_000 + n;
            row.state = DownloadState::Done;
            row.title = Some("A very long title ".repeat(17));
            app.downloads
                .entries
                .push(crate::app::downloads::DownloadEntry::for_test(row));
        }
        app.toggle_downloads_panel(cx);
    });
    render_ui(vcx);
    let window = vcx.update(|window, _| window.viewport_size());
    let panel = vcx
        .debug_bounds("downloads-panel")
        .expect("the list paints");
    assert!(
        panel.bottom() <= window.height && panel.right() <= window.width,
        "the list stays inside the window: {panel:?} in {window:?}"
    );
    let row = vcx
        .debug_bounds("download-row-1000")
        .expect("the first long row paints");
    assert!(
        row.size.width <= panel.size.width,
        "a long title truncates instead of widening the list"
    );
}

#[core::prelude::v1::test]
fn keyboard_focus_scrolls_into_view() {
    let (_dir, engine, fake, mut cx) = queue_app("dlp_scroll_focus");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    queue(&view, vcx, &[A]);
    view.update(vcx, |app, cx| {
        let template = app.downloads.entries[0].row.clone();
        for n in 0..40 {
            let mut row = template.clone();
            row.id = 2_000 + n;
            row.state = DownloadState::Done;
            // Earlier lists newest first: 2_039 finished first, so it's last.
            row.finished_epoch_ms = Some(10_000 - n);
            app.downloads
                .entries
                .push(crate::app::downloads::DownloadEntry::for_test(row));
        }
        app.toggle_downloads_panel(cx);
    });
    render_ui(vcx);
    focus_app(vcx, &view);
    for _ in 0..40 {
        keystrokes(vcx, "down");
    }
    render_ui(vcx);
    render_ui(vcx);
    let last = view.update(vcx, |app, _| app.downloads.focused.unwrap());
    assert_eq!(last, 2_039, "focus reached the last row");
    let body = vcx
        .debug_bounds("downloads-panel-body")
        .expect("the scroll body paints");
    let row = vcx
        .debug_bounds(leak(format!("download-row-{last}")))
        .expect("the focused row paints");
    assert!(
        row.top() >= body.top() && row.bottom() <= body.bottom(),
        "the focused row is scrolled into the body, not under the header or footer: \
         {row:?} in {body:?}"
    );
}

#[core::prelude::v1::test]
fn long_failure_reason_wraps() {
    let (_dir, engine, fake, mut cx) = queue_app("dlp_reason");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    queue(&view, vcx, &[A]);
    let reasons = [
        (3_000, "This video is private.".to_string()),
        (
            3_001,
            "The video was saved as \"A long clip name.mp4\" in your Imports folder \
             but couldn't be added to Lumina. Rescan the library to add it."
                .to_string(),
        ),
        (
            3_002,
            "The site stopped answering partway through. ".repeat(20),
        ),
    ];
    view.update(vcx, |app, cx| {
        let template = app.downloads.entries[0].row.clone();
        for (id, reason) in &reasons {
            let mut failed = template.clone();
            failed.id = *id;
            failed.state = DownloadState::Failed;
            failed.error = Some(reason.clone());
            app.downloads
                .entries
                .push(crate::app::downloads::DownloadEntry::for_test(failed));
        }
        app.toggle_downloads_panel(cx);
    });
    render_ui(vcx);
    let meta = |vcx: &mut VisualTestContext, id: i64| {
        vcx.debug_bounds(leak(format!("download-meta-{id}")))
            .unwrap_or_else(|| panic!("row {id}'s meta paints"))
            .size
            .height
    };
    let one_line = meta(vcx, 3_000);
    assert!(
        meta(vcx, 3_001) > one_line,
        "a long reason wraps onto more lines instead of losing its last sentence"
    );
    let clamped = meta(vcx, 3_002);
    assert!(
        clamped > one_line * 2.0 && clamped <= one_line * 3.0 + gpui::px(1.0),
        "a very long reason stops at 3 lines: {clamped:?} with lines of {one_line:?}"
    );
}

#[core::prelude::v1::test]
fn downloads_row_follows_the_queue() {
    let (_dir, engine, fake, mut cx) = queue_app("dlp_sidebar_row");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("sidebar-nav-downloads").is_none(),
        "no Downloads row while the download list is empty"
    );
    queue(&view, vcx, &[A, B]);
    assert!(
        vcx.debug_bounds("sidebar-nav-downloads-count").is_some(),
        "the row shows the unfinished count once something is queued"
    );
    click(vcx, "sidebar-nav-downloads");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            crate::views::sidebar::AlbumSelection::Downloads,
            "the row opens the Downloads screen; ⌥⌘L keeps the panel"
        );
    });
}

/// True when `outer` covers the point (`x`, `y`).
fn covers(outer: gpui::Bounds<gpui::Pixels>, x: gpui::Pixels, y: gpui::Pixels) -> bool {
    outer.left() <= x && x <= outer.right() && outer.top() <= y && y <= outer.bottom()
}

#[core::prelude::v1::test]
fn downloads_badge_and_dot_sit_on_the_34px_button() {
    let (_dir, engine, fake, mut cx) = queue_app("dlp_badge_dot");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    queue(&view, vcx, &[A, B]);
    let button = vcx
        .debug_bounds("downloads-toolbar-btn")
        .expect("the button paints");
    let badge = vcx
        .debug_bounds("downloads-badge")
        .expect("the badge paints");
    assert!(
        covers(badge, button.right(), button.top()),
        "the badge sits on the button's top-right corner: {badge:?} on {button:?}"
    );
    assert!(
        vcx.debug_bounds("downloads-failure-dot").is_none(),
        "no failure dot before anything fails"
    );

    fake.finish(A, JobOutcome::Failed("This video is private".into()));
    settle(vcx, &view, "A fails", |app| app.downloads.unseen_failure);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.download_badge_count(),
            1,
            "the badge counts unfinished rows; the failure shows as the dot"
        )
    });
    let button = vcx
        .debug_bounds("downloads-toolbar-btn")
        .expect("the button paints");
    let dot = vcx
        .debug_bounds("downloads-failure-dot")
        .expect("a failure while the list is closed shows the dot");
    assert!(
        covers(dot, button.right(), button.bottom()),
        "the dot sits on the button's bottom-right corner: {dot:?} on {button:?}"
    );

    click(vcx, "downloads-toolbar-btn");
    assert!(
        vcx.debug_bounds("downloads-failure-dot").is_none(),
        "opening the list clears the dot"
    );
}

#[core::prelude::v1::test]
fn downloads_popover_shell() {
    let (_dir, engine, fake, mut cx) = queue_app("dlp_shell");
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    queue(&view, vcx, &[A, B]);
    fake.finish(A, imported(&engine));
    settle(vcx, &view, "A is done", |app| {
        states(app).contains(&DownloadState::Done)
    });
    view.update(vcx, |app, cx| app.toggle_downloads_panel(cx));
    render_ui(vcx);

    let window = vcx.update(|window, _| window.viewport_size());
    let panel = vcx
        .debug_bounds("downloads-panel")
        .expect("the popover paints");
    let button = vcx
        .debug_bounds("downloads-toolbar-btn")
        .expect("the button paints");
    assert!(
        panel.size.height <= gpui::px(550.0),
        "the popover is at most 550 tall: {panel:?}"
    );
    assert!(
        (panel.right() - button.right()).abs() <= gpui::px(1.0),
        "the button anchors the popover's right edge: {panel:?} under {button:?}"
    );
    // The frame's "right 28" is where its button sits; the button anchors
    // the popover, wherever the header puts it.
    assert!(
        panel.right() <= window.width,
        "the popover stays inside the window: {panel:?} in {window:?}"
    );
    let header = vcx
        .debug_bounds("downloads-header")
        .expect("the header paints");
    assert!(
        vcx.debug_bounds("downloads-header-count-text:1 active")
            .is_some(),
        "the header pill reads \"1 active\""
    );
    view.update(vcx, |app, _| {
        assert_eq!(app.running_or_waiting_count(), 1, "\"1 active\" counts B")
    });
    assert!(
        vcx.debug_bounds("downloads-header-resume").is_none(),
        "no Resume all while something runs"
    );
    assert!(
        vcx.debug_bounds("downloads-header-settings")
            .is_some_and(|b| header.contains(&b.center())),
        "the header has the settings button"
    );
    let head = vcx
        .debug_bounds("downloads-earlier-head")
        .expect("the Earlier head paints");
    let clear = vcx
        .debug_bounds("downloads-clear-finished")
        .expect("Clear paints");
    assert!(
        head.left() <= clear.left()
            && clear.right() <= head.right()
            && head.top() <= clear.top()
            && clear.bottom() <= head.bottom(),
        "Clear lives in the Earlier head: {clear:?} in {head:?}"
    );
    let footer = vcx
        .debug_bounds("downloads-show-all")
        .expect("the footer paints");
    assert!(
        header.bottom() <= head.top() && head.bottom() <= footer.top(),
        "header on top, footer at the bottom"
    );
    for gone in ["downloads-resume-all", "downloads-show-album"] {
        assert!(
            vcx.debug_bounds(gone).is_none(),
            "{gone} moved to the header or the Downloads screen"
        );
    }

    // Pause all, then the same button resumes all.
    click(vcx, "downloads-header-pause");
    view.update(vcx, |app, _| {
        assert_eq!(running(app), 0, "the header's pause pauses all")
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("downloads-header-pause").is_none(),
        "pause turns into Resume all once nothing runs"
    );
    click(vcx, "downloads-header-resume");
    settle(vcx, &view, "Resume all requeues the paused row", |app| {
        running(app) == 1
    });

    click(vcx, "downloads-show-all");
    view.update(vcx, |app, _| {
        assert!(!app.downloads.open, "Show all downloads closes the popover");
        assert_eq!(
            app.selection,
            crate::views::sidebar::AlbumSelection::Downloads,
            "and opens the Downloads screen"
        );
    });
    view.update(vcx, |app, cx| {
        app.select_album(crate::views::sidebar::AlbumSelection::AllLibrary, cx);
        app.toggle_downloads_panel(cx);
    });
    render_ui(vcx);
    click(vcx, "downloads-header-expand");
    view.update(vcx, |app, _| {
        assert!(
            !app.downloads.open,
            "the header's expand closes the popover"
        );
        assert_eq!(
            app.selection,
            crate::views::sidebar::AlbumSelection::Downloads,
            "and opens the Downloads screen too"
        );
    });
}
