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
fn queue(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, urls: &[&str]) {
    for (n, url) in urls.iter().enumerate() {
        submit(view, vcx, url);
        settle(vcx, view, "the row is stored", |app| {
            app.downloads.entries.len() == n + 1
                && app.downloads.entries.iter().all(|e| e.row.id > 0)
        });
    }
    let expected = urls.len().min(crate::app::downloads::MAX_RUNNING);
    settle(vcx, view, "the downloads run", |app| {
        running(app) == expected
    });
    render_ui(vcx);
}

const A: &str = "https://example.com/a.mp4";
const B: &str = "https://example.com/b.mp4";

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
    view.update(vcx, |app, _| assert_eq!(app.active_download_count(), 2));

    fake.finish(A, imported(&engine));
    fake.finish(B, imported(&engine));
    settle(vcx, &view, "both finish", |app| {
        app.active_download_count() == 0
    });
    // The bounds map only grows in GPUI 0.2.2 test support, so check the
    // count behind the badge rather than the badge's absence.
    view.update(vcx, |app, _| {
        assert_eq!(app.active_download_count(), 0, "no badge once all are done");
        assert_eq!(app.downloads.entries.len(), 2, "finished rows stay listed");
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
        assert!(app.downloads.entries.is_empty(), "✕ removes the row")
    });
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
    queue(&view, vcx, &[A, B]);
    fake.finish(A, imported(&engine));
    settle(vcx, &view, "A is done", |app| {
        states(app).contains(&DownloadState::Done)
    });
    view.update(vcx, |app, cx| app.toggle_downloads_panel(cx));
    render_ui(vcx);
    click(vcx, "downloads-clear-finished");
    view.update(vcx, |app, _| {
        assert_eq!(
            states(app),
            [DownloadState::Running],
            "only finished rows go"
        );
    });
    settle(vcx, &view, "the clear is stored", |_| {
        persisted(&engine) == [(B.to_string(), DownloadState::Running)]
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
