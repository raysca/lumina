//! The Downloads screen: its route, top bar, sidebar row, restore and
//! empty state. Rows are set directly (no job runs), so nothing touches the
//! network.

use super::downloads::{imported, install, persisted, queue_app, settle, states, submit};
use super::ui_automation::{app_with_library, click, render_ui};
use crate::app::downloads::{DownloadEntry, JobOutcome};
use crate::config::{AppConfig, LastView};
use crate::views::sidebar::AlbumSelection;
use crate::LuminaApp;
use gpui::{Entity, VisualTestContext};
use lumina_core::net_import::{DownloadPhase, NetworkDownloadProgress};
use lumina_core::{DownloadRow, DownloadState};

fn row(id: i64, state: DownloadState) -> DownloadRow {
    DownloadRow {
        id,
        url: format!("https://example.com/{id}.mp4"),
        canonical_url: None,
        title: Some(format!("Video {id}")),
        state,
        error: None,
        can_retry: true,
        media_id: None,
        added_epoch_ms: id,
        finished_epoch_ms: None,
        in_library: false,
    }
}

/// Puts rows in the list as if they had loaded; no job runs.
fn set_rows(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, states: &[DownloadState]) {
    view.update(vcx, |app, cx| {
        app.downloads.entries = states
            .iter()
            .enumerate()
            .map(|(n, state)| DownloadEntry::for_test(row(n as i64 + 1, *state)))
            .collect();
        cx.notify();
    });
}

fn meta_marker(vcx: &mut VisualTestContext, meta: &str) -> bool {
    let selector: &'static str = Box::leak(format!("page-meta-text:{meta}").into_boxed_str());
    vcx.debug_bounds(selector).is_some()
}

fn assert_on_screen(vcx: &mut VisualTestContext, how: &str) {
    assert!(
        vcx.debug_bounds("downloads-screen").is_some(),
        "{how} shows the Downloads screen"
    );
    for gone in ["media-grid-surface", "status-bar"] {
        assert!(
            vcx.debug_bounds(gone).is_none(),
            "{how}: the screen replaces {gone}"
        );
    }
}

#[core::prelude::v1::test]
fn downloads_screen_replaces_grid() {
    let (_dir, engine, fake, mut cx) = queue_app("dls_route");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    set_rows(
        &view,
        vcx,
        &[
            DownloadState::Running,
            DownloadState::Running,
            DownloadState::Waiting,
        ],
    );
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::Downloads, cx)
    });
    render_ui(vcx);
    assert_on_screen(vcx, "selecting Downloads");
    assert!(
        vcx.debug_bounds("page-title-text:Downloads").is_some(),
        "the top bar is titled Downloads"
    );
    assert!(
        meta_marker(vcx, "2 downloading · 1 queued"),
        "the meta counts rows by state, leaving out zero parts and unknown speed"
    );
    for button in [
        "downloads-pause-all",
        "downloads-clear-completed",
        "downloads-add-url",
    ] {
        assert!(
            vcx.debug_bounds(button).is_some(),
            "the top bar has {button}"
        );
    }

    click(vcx, "downloads-add-url");
    view.update(vcx, |app, _| {
        assert!(
            app.network_import.stage.is_some(),
            "Add URL opens the import dialog"
        )
    });
    view.update(vcx, |app, cx| app.close_network_import_dialog(cx));

    // From another view, the palette's Show All Downloads opens it.
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::AllLibrary, cx)
    });
    render_ui(vcx);
    assert!(vcx.debug_bounds("downloads-screen").is_none());
    view.update(vcx, |app, cx| {
        app.open_command_palette(cx);
        app.command_query = "show all downloads".to_string();
        cx.notify();
    });
    view.update(vcx, |app, cx| app.execute_selected_command(cx));
    render_ui(vcx);
    assert_on_screen(vcx, "the palette's Show All Downloads");
}

#[core::prelude::v1::test]
fn downloads_meta_adds_paused_and_speed() {
    let (_dir, engine, fake, mut cx) = queue_app("dls_meta");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    set_rows(&view, vcx, &[DownloadState::Running, DownloadState::Paused]);
    view.update(vcx, |app, cx| {
        let mut progress = NetworkDownloadProgress::new(DownloadPhase::Downloading, "");
        progress.speed_bytes_per_sec = 26_004_685; // 24.8 MB/s
        app.downloads.entries[0].progress = Some(progress);
        app.select_album(AlbumSelection::Downloads, cx);
    });
    render_ui(vcx);
    assert!(
        meta_marker(vcx, "1 downloading · 1 paused · 24.8 MB/s"),
        "the combined speed joins the meta once known"
    );
}

#[core::prelude::v1::test]
fn downloads_sidebar_row_opens_screen() {
    let (_dir, engine, fake, mut cx) = queue_app("dls_sidebar");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    set_rows(&view, vcx, &[DownloadState::Running]);
    render_ui(vcx);
    assert!(vcx.debug_bounds("sidebar-nav-downloads-active").is_none());
    click(vcx, "sidebar-nav-downloads");
    assert_on_screen(vcx, "the sidebar Downloads row");
    assert!(
        vcx.debug_bounds("sidebar-nav-downloads-active").is_some(),
        "the Downloads row is the active one while the screen shows"
    );
    view.update(vcx, |app, _| {
        assert!(
            !app.downloads.open,
            "the row opens the screen, not the dropdown"
        )
    });

    // Empty queue: the row stays while the screen is selected, then goes.
    set_rows(&view, vcx, &[]);
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("sidebar-nav-downloads").is_some(),
        "the selected screen keeps its row"
    );
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::AllLibrary, cx)
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("sidebar-nav-downloads").is_none(),
        "no row once the queue is empty and the screen isn't selected"
    );
}

#[core::prelude::v1::test]
fn last_view_restores_downloads() {
    assert_eq!(LastView::from_token("downloads"), LastView::Downloads);
    assert_eq!(LastView::Downloads.token(), "downloads");
    assert_eq!(
        LastView::from_selection(&AlbumSelection::Downloads),
        LastView::Downloads,
        "the screen is saved as the last view"
    );
    // Built in memory: nothing reads or writes a real config file.
    let config = AppConfig {
        last_view: Some("downloads".to_string()),
        ..AppConfig::default()
    };
    let view = config.restored_session().view;
    assert_eq!(view, LastView::Downloads);
    assert_eq!(
        crate::app::library::resolve_restored_view(view.clone(), &[], &[], false),
        AlbumSelection::Downloads,
        "with downloads listed, the screen comes back"
    );
    assert_eq!(
        crate::app::library::resolve_restored_view(view, &[], &[], true),
        AlbumSelection::AllLibrary,
        "with an empty queue, All comes back instead"
    );
}

#[core::prelude::v1::test]
fn downloads_screen_empty_state() {
    let (_dir, engine, fake, mut cx) = queue_app("dls_empty");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::Downloads, cx)
    });
    render_ui(vcx);
    assert_on_screen(vcx, "an empty queue");
    assert!(
        vcx.debug_bounds("downloads-empty-title-text:No downloads yet")
            .is_some(),
        "an empty queue says so"
    );
    click(vcx, "downloads-empty-add-url");
    view.update(vcx, |app, _| {
        assert!(
            app.network_import.stage.is_some(),
            "the empty state's Add URL opens the import dialog"
        )
    });
}

fn marker(vcx: &mut VisualTestContext, selector: String) -> bool {
    let selector: &'static str = Box::leak(selector.into_boxed_str());
    vcx.debug_bounds(selector).is_some()
}

#[core::prelude::v1::test]
fn downloads_screen_buttons_act() {
    const A: &str = "https://example.com/a.mp4";
    const B: &str = "https://example.com/b.mp4";
    let (_dir, engine, fake, mut cx) = queue_app("dls_buttons");
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    for (n, url) in [A, B].into_iter().enumerate() {
        submit(&view, vcx, url);
        settle(vcx, &view, "the row runs", |app| {
            app.downloads.entries.len() == n + 1
                && states(app).iter().all(|s| *s == DownloadState::Running)
        });
    }
    view.update(vcx, |app, cx| app.show_downloads_screen(cx));
    render_ui(vcx);
    assert!(marker(vcx, "downloads-pause-all-text:Pause all".into()));

    // Clear completed with nothing completed: dimmed, and a click does nothing.
    click(vcx, "downloads-clear-completed");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.downloads.entries.len(),
            2,
            "nothing to clear, nothing cleared"
        )
    });

    click(vcx, "downloads-pause-all");
    view.update(vcx, |app, _| {
        assert_eq!(
            states(app),
            [DownloadState::Paused; 2],
            "Pause all pauses every row"
        )
    });
    settle(vcx, &view, "the pauses are stored", |_| {
        persisted(&engine)
            .iter()
            .all(|(_, s)| *s == DownloadState::Paused)
    });
    render_ui(vcx);
    assert!(
        marker(vcx, "downloads-pause-all-text:Resume all".into()),
        "with nothing running and rows paused, the button reads Resume all"
    );
    click(vcx, "downloads-pause-all");
    settle(vcx, &view, "Resume all requeues the rows", |app| {
        states(app) == [DownloadState::Running; 2]
    });

    fake.finish(A, imported(&engine));
    fake.finish(B, JobOutcome::Failed("This video is private.".into()));
    settle(vcx, &view, "A is done and B failed", |app| {
        let s = states(app);
        s.contains(&DownloadState::Done) && s.contains(&DownloadState::Failed)
    });
    render_ui(vcx);
    assert!(
        marker(vcx, "page-meta-text:1 failed · 1 completed".into()),
        "the meta counts failed and completed rows"
    );
    click(vcx, "downloads-clear-completed");
    view.update(vcx, |app, _| {
        assert_eq!(
            states(app),
            [DownloadState::Failed],
            "Clear completed removes the done row and keeps the failed one"
        )
    });
    settle(vcx, &view, "the clear is stored", |_| {
        persisted(&engine).len() == 1
    });
    assert_eq!(
        persisted(&engine),
        [(B.to_string(), DownloadState::Failed)],
        "the failed row stays stored with its link"
    );

    // Nothing to pause or resume: dimmed, and a click does nothing.
    render_ui(vcx);
    click(vcx, "downloads-pause-all");
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        assert_eq!(states(app), [DownloadState::Failed])
    });
}
