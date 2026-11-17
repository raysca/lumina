//! The Downloads screen: its route, top bar, sidebar row, restore and
//! empty state. Rows are set directly (no job runs), so nothing touches the
//! network.

use super::downloads::{imported, install, persisted, queue_app, settle, states, submit};
use super::ui_automation::{app_with_library, click, focus_app, keystrokes, render_ui};
use crate::app::downloads::{DownloadEntry, DownloadFilter, JobEvent, JobOutcome};
use crate::config::{AppConfig, LastView};
use crate::views::sidebar::AlbumSelection;
use crate::LuminaApp;
use gpui::{Entity, VisualTestContext};
use lumina_core::net_import::{DownloadPhase, NetworkDownloadProgress};
use lumina_core::{DownloadRow, DownloadState};

pub(super) fn row(id: i64, state: DownloadState) -> DownloadRow {
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
pub(super) fn set_rows(
    view: &Entity<LuminaApp>,
    vcx: &mut VisualTestContext,
    states: &[DownloadState],
) {
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
        crate::app::restore::resolve_restored_view(view.clone(), &[], &[], false),
        AlbumSelection::Downloads,
        "with downloads listed, the screen comes back"
    );
    assert_eq!(
        crate::app::restore::resolve_restored_view(view, &[], &[], true),
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

pub(super) fn marker(vcx: &mut VisualTestContext, selector: String) -> bool {
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

/// Which `downloads-screen-row-{id}` rows paint, for ids 1..=7.
fn shown_rows(vcx: &mut VisualTestContext) -> Vec<i64> {
    (1..=7)
        .filter(|id| marker(vcx, format!("downloads-screen-row-{id}")))
        .collect()
}

#[core::prelude::v1::test]
fn download_filter_chips_count_and_filter() {
    let (_dir, engine, fake, mut cx) = queue_app("dls_filter");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    vcx.simulate_resize(gpui::size(gpui::px(1600.0), gpui::px(1000.0)));
    // Ids 1-7: two running, then waiting, paused, failed, done, duplicate.
    set_rows(
        &view,
        vcx,
        &[
            DownloadState::Running,
            DownloadState::Running,
            DownloadState::Waiting,
            DownloadState::Paused,
            DownloadState::Failed,
            DownloadState::Done,
            DownloadState::Duplicate,
        ],
    );
    view.update(vcx, |app, cx| app.show_downloads_screen(cx));
    render_ui(vcx);
    for (key, n) in [
        ("all", 7),
        ("downloading", 2),
        ("queued", 1),
        ("paused", 1),
        ("failed", 1),
        ("completed", 2),
    ] {
        assert!(
            marker(vcx, format!("downloads-filter-{key}")),
            "the {key} chip paints"
        );
        assert!(
            marker(vcx, format!("downloads-filter-count-text:{key} {n}")),
            "the {key} chip counts {n}"
        );
    }
    assert_eq!(
        shown_rows(vcx),
        (1..=7).collect::<Vec<_>>(),
        "All lists every row"
    );
    assert!(
        marker(vcx, "downloads-limits-text:3 at a time".into()),
        "the limits text reads the live concurrency"
    );

    click(vcx, "downloads-filter-failed");
    assert_eq!(shown_rows(vcx), [5], "Failed leaves only failed rows");
    view.update(vcx, |app, _| {
        assert_eq!(app.downloads.filter, DownloadFilter::Failed)
    });

    // Keyboard: ←/→ move between chips, stopping at the ends.
    focus_app(vcx, &view);
    keystrokes(vcx, "right");
    assert_eq!(
        shown_rows(vcx),
        [6, 7],
        "→ moves to Completed (Done and Duplicate)"
    );
    keystrokes(vcx, "right");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.downloads.filter,
            DownloadFilter::Completed,
            "→ stops at the last chip"
        )
    });
    keystrokes(vcx, "left left left");
    assert_eq!(shown_rows(vcx), [3], "← moves back to Queued");
    keystrokes(vcx, "left left left");
    view.update(vcx, |app, _| {
        assert_eq!(app.downloads.filter, DownloadFilter::All, "← stops at All")
    });

    click(vcx, "downloads-settings");
    view.update(vcx, |app, _| {
        assert!(
            app.preferences_open,
            "the settings button opens Preferences"
        )
    });
    view.update(vcx, |app, cx| app.close_preferences(cx));

    // The limits text follows the Simultaneous downloads setting.
    view.update(vcx, |app, cx| app.set_download_concurrency(5, cx));
    render_ui(vcx);
    assert!(
        marker(vcx, "downloads-limits-text:5 at a time".into()),
        "the limits text follows the Simultaneous downloads setting"
    );

    // Narrow main area: the limits text hides; the chips stay.
    vcx.simulate_resize(gpui::size(gpui::px(1000.0), gpui::px(800.0)));
    render_ui(vcx);
    assert!(
        !marker(vcx, "downloads-limits".into()),
        "below 900 px the limits text hides first"
    );
    assert!(marker(vcx, "downloads-filter-completed".into()));
}

const MB: u64 = 1024 * 1024;

/// Gives running row `index` its progress.
fn progress(app: &mut LuminaApp, index: usize, done: u64, total: Option<u64>, speed: u64) {
    let mut p = NetworkDownloadProgress::new(DownloadPhase::Downloading, "");
    p.bytes_downloaded = done;
    p.total_bytes = total;
    p.speed_bytes_per_sec = speed;
    app.downloads.entries[index].progress = Some(p);
}

#[core::prelude::v1::test]
fn downloads_status_bar_sums_running_rows() {
    let (_dir, engine, fake, mut cx) = queue_app("dls_status");
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
        progress(app, 0, 100 * MB, Some(400 * MB), 5 * MB);
        progress(app, 1, 50 * MB, Some(100 * MB), 5 * MB);
        app.show_downloads_screen(cx);
    });
    render_ui(vcx);
    assert!(
        marker(vcx, "downloads-status-bar".into()),
        "a running download shows the bar"
    );
    assert!(
        marker(
            vcx,
            "downloads-status-headline-text:Downloading 2 of 3 items".into()
        ),
        "the headline counts running rows of the unfinished ones"
    );
    assert!(
        marker(vcx, "downloads-overall-text:30%".into()),
        "150 of 500 MB is 30% overall"
    );
    assert!(
        marker(
            vcx,
            "downloads-status-detail-text:150.0 MB of 500.0 MB · 10.0 MB/s combined · about 35 s remaining"
                .into()
        ),
        "the detail sums bytes and speed across running rows"
    );

    set_rows(&view, vcx, &[DownloadState::Paused, DownloadState::Done]);
    render_ui(vcx);
    assert!(
        !marker(vcx, "downloads-status-bar".into()),
        "with nothing running there is no status bar"
    );
}

#[core::prelude::v1::test]
fn downloads_status_bar_drops_unknown_parts() {
    let (_dir, engine, fake, mut cx) = queue_app("dls_status_unknown");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    set_rows(
        &view,
        vcx,
        &[DownloadState::Running, DownloadState::Running],
    );
    view.update(vcx, |app, cx| {
        progress(app, 0, 50 * MB, None, MB);
        progress(app, 1, 10 * MB, Some(20 * MB), 0);
        app.show_downloads_screen(cx);
    });
    render_ui(vcx);
    assert!(
        marker(
            vcx,
            "downloads-status-detail-text:60.0 MB · 1.0 MB/s combined".into()
        ),
        "an unknown total drops the \"of\" and \"remaining\" parts"
    );
    assert!(
        !marker(vcx, "downloads-overall".into()),
        "without a total for every row there is no overall percentage to agree with"
    );

    // No speed known either: the speed part goes too.
    view.update(vcx, |app, cx| {
        progress(app, 0, 50 * MB, None, 0);
        cx.notify();
    });
    render_ui(vcx);
    assert!(marker(vcx, "downloads-status-detail-text:60.0 MB".into()));
}

/// Sends row `index` a progress event through the job path.
fn report(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, index: usize, speed: u64) {
    view.update(vcx, |app, cx| {
        let mut p = NetworkDownloadProgress::new(DownloadPhase::Downloading, "");
        p.bytes_downloaded = 10 * MB;
        p.total_bytes = Some(20 * MB);
        p.speed_bytes_per_sec = speed;
        let (generation, id) = (
            app.downloads.generation,
            app.downloads.entries[index].row.id,
        );
        app.apply_job_events(generation, id, vec![JobEvent::Progress(p)], cx);
    });
    render_ui(vcx);
}

#[core::prelude::v1::test]
fn a_stalled_download_redraws_without_its_speed() {
    let (_dir, engine, fake, mut cx) = queue_app("dls_stalled_speed");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    set_rows(&view, vcx, &[DownloadState::Running]);
    view.update(vcx, |app, cx| app.show_downloads_screen(cx));
    let notified = std::rc::Rc::new(std::cell::Cell::new(0));
    let _watch = {
        let notified = notified.clone();
        vcx.update(|_, cx| cx.observe(&view, move |_, _| notified.set(notified.get() + 1)))
    };
    let wait = |vcx: &mut VisualTestContext, secs: u64| {
        vcx.executor()
            .advance_clock(std::time::Duration::from_secs(secs));
        vcx.run_until_parked();
    };
    let speed_shown = |vcx: &mut VisualTestContext| {
        marker(
            vcx,
            "downloads-row-meta-text:example.com · 10.0 MB of 20.0 MB · 4.0 MB/s".into(),
        )
    };

    report(&view, vcx, 0, 4 * MB);
    assert!(speed_shown(vcx), "a fresh speed shows");
    wait(vcx, 3);
    // Another event restarts the wait: the speed is fresh again.
    report(&view, vcx, 0, 4 * MB);
    wait(vcx, 3);
    render_ui(vcx);
    assert!(
        speed_shown(vcx),
        "the speed stays while events keep arriving"
    );
    assert_eq!(view.update(vcx, |app, _| app.download_speed()), 4 * MB);

    // The only job goes silent: nothing else would redraw.
    let before = notified.get();
    wait(vcx, 3);
    assert!(
        notified.get() > before,
        "a job silent for a few seconds redraws by itself"
    );
    render_ui(vcx);
    assert!(
        marker(
            vcx,
            "downloads-row-meta-text:example.com · 10.0 MB of 20.0 MB".into()
        ),
        "a stalled job shows no speed"
    );
    assert_eq!(
        view.update(vcx, |app, _| app.download_speed()),
        0,
        "a stalled job adds nothing to the combined speed"
    );

    // A row removed while its wait runs: nothing fires for it.
    report(&view, vcx, 0, 4 * MB);
    set_rows(&view, vcx, &[]);
    let before = notified.get();
    wait(vcx, 10);
    assert_eq!(
        notified.get(),
        before,
        "a removed row's wait is dropped with it"
    );
}

#[core::prelude::v1::test]
fn speed_history_clears_when_the_last_download_ends() {
    const A: &str = "https://example.com/a.mp4";
    let (_dir, engine, fake, mut cx) = queue_app("dls_speed_clear");
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    submit(&view, vcx, A);
    settle(vcx, &view, "A runs", |app| {
        states(app) == [DownloadState::Running]
    });
    view.update(vcx, |app, _| {
        app.downloads
            .speed
            .record(std::time::Instant::now(), 5 * MB)
    });
    fake.finish(A, imported(&engine));
    settle(vcx, &view, "A is done", |app| {
        states(app) == [DownloadState::Done]
    });
    view.update(vcx, |app, _| {
        assert_eq!(
            app.downloads.speed.samples().count(),
            0,
            "a later run starts with no old bars"
        )
    });
}
