//! The URL download queue: scheduling, removal, retry, duplicates and
//! persistence, driven through a fake job so nothing touches the network.

use super::helpers::{app_with_library, render_ui, seed_library, TempLibrary};
use crate::app::downloads::{DownloadsConfirm, JobFn, JobOutcome, JobRequest};
use crate::app::network_import::NetworkImportStage;
use crate::config::DEFAULT_DOWNLOAD_CONCURRENCY;
use crate::LuminaApp;
use gpui::{Entity, VisualTestContext};
use lumina_core::{DownloadState, LuminaEngine, MediaId, PriorDownload};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub(crate) fn title_for(url: &str) -> String {
    format!("Title of {url}")
}

/// One URL's outcome channel: the test sends, the job thread receives.
type OutcomeChannel = (Sender<JobOutcome>, Arc<Mutex<Receiver<JobOutcome>>>);

/// Jobs that wait until the test finishes them. Each URL gets a channel the
/// test sends the outcome on; a cancelled job returns `Cancelled`.
#[derive(Clone, Default)]
pub(crate) struct FakeJobs {
    requests: Arc<Mutex<Vec<JobRequest>>>,
    channels: Arc<Mutex<HashMap<String, OutcomeChannel>>>,
}

impl FakeJobs {
    fn channel(&self, url: &str) -> OutcomeChannel {
        self.channels
            .lock()
            .unwrap()
            .entry(url.to_string())
            .or_insert_with(|| {
                let (tx, rx) = channel();
                (tx, Arc::new(Mutex::new(rx)))
            })
            .clone()
    }

    pub(crate) fn finish(&self, url: &str, outcome: JobOutcome) {
        self.channel(url).0.send(outcome).unwrap();
    }

    pub(crate) fn requests(&self) -> Vec<JobRequest> {
        self.requests.lock().unwrap().clone()
    }

    pub(crate) fn job(&self) -> JobFn {
        let fake = self.clone();
        Arc::new(move |request, cancel, events| {
            fake.requests.lock().unwrap().push(request.clone());
            // Like the real job: the link is inspected before anything else.
            let _ = events.send_blocking(crate::app::downloads::JobEvent::Inspected {
                canonical_url: request.url.clone(),
                title: title_for(&request.url),
            });
            let rx = fake.channel(&request.url).1;
            loop {
                if cancel.load(Ordering::Relaxed) {
                    // A result the test sent before the cancel still wins,
                    // like a job that finished as it was stopped.
                    return rx
                        .lock()
                        .unwrap()
                        .recv_timeout(Duration::from_millis(20))
                        .unwrap_or(JobOutcome::Cancelled);
                }
                if let Ok(outcome) = rx.lock().unwrap().recv_timeout(Duration::from_millis(2)) {
                    return outcome;
                }
            }
        })
    }
}

/// Lets job threads report back: runs the app until `done` holds. The
/// jobs run on real threads, so this waits on a condition (bounded), never
/// on a fixed delay.
pub(crate) fn settle(
    vcx: &mut VisualTestContext,
    view: &Entity<LuminaApp>,
    what: &str,
    done: impl Fn(&LuminaApp) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        vcx.run_until_parked();
        if view.update(vcx, |app, _| done(app)) {
            return;
        }
        assert!(Instant::now() < deadline, "timed out waiting until {what}");
        std::thread::yield_now();
    }
}

pub(crate) fn queue_app(
    name: &str,
) -> (
    TempLibrary,
    Arc<LuminaEngine>,
    FakeJobs,
    crate::tests::TestApp,
) {
    let (dir, engine) = seed_library(name, 1);
    (
        dir,
        engine,
        FakeJobs::default(),
        crate::tests::TestApp::single(),
    )
}

/// Installs the fake job, and an inspector that answers at once without
/// the network.
pub(crate) fn install(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, fake: &FakeJobs) {
    let job = fake.job();
    let inspect: crate::app::network_import::InspectFn = Arc::new(|url, _cancel| {
        Ok(lumina_core::net_import::NetworkMediaInfo {
            url: url.to_string(),
            kind: lumina_core::net_import::classify_url(url)?,
            title: title_for(url),
            duration_secs: None,
            uploader: None,
            expected_bytes: None,
            extension: "mp4",
        })
    });
    view.update(vcx, |app, _| {
        app.downloads.job = Some(job);
        app.network_import.inspect = Some(inspect);
    });
}

/// Types `url` into the dialog and, once its preview shows, presses Import.
pub(crate) fn submit(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, url: &str) {
    view.update(vcx, |app, cx| {
        app.open_network_import_dialog(cx);
        app.submit_network_url(url.to_string(), cx);
    });
    vcx.run_until_parked();
    view.update(vcx, |app, cx| app.import_inspected_link(cx));
}

pub(crate) fn states(app: &LuminaApp) -> Vec<DownloadState> {
    app.downloads.entries.iter().map(|e| e.row.state).collect()
}

fn count(app: &LuminaApp, state: DownloadState) -> usize {
    states(app).into_iter().filter(|s| *s == state).count()
}

pub(crate) fn row_id(app: &LuminaApp, url: &str) -> i64 {
    app.downloads
        .entries
        .iter()
        .find(|e| e.row.url == url)
        .unwrap_or_else(|| panic!("no row for {url}"))
        .row
        .id
}

pub(crate) fn persisted(engine: &LuminaEngine) -> Vec<(String, DownloadState)> {
    engine
        .catalog()
        .list_downloads()
        .unwrap()
        .into_iter()
        .map(|row| (row.url, row.state))
        .collect()
}

pub(crate) fn imported(engine: &LuminaEngine) -> JobOutcome {
    let item = engine
        .catalog()
        .get_item_by_id(MediaId(1))
        .unwrap()
        .expect("the seeded library has one item");
    JobOutcome::Imported {
        item: Box::new(item),
        timeline_error: None,
    }
}

const URL: &str = "https://example.com/a.mp4";
const URL_B: &str = "https://example.com/b.mp4";

#[core::prelude::v1::test]
fn submit_closes_dialog_and_adds_a_row() {
    let (_dir, engine, fake, mut cx) = queue_app("dl_submit");
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);

    submit(&view, vcx, URL);
    settle(vcx, &view, "the job starts", |app| {
        count(app, DownloadState::Running) == 1
    });
    view.update(vcx, |app, _| {
        assert!(
            app.network_import.stage.is_none(),
            "Import closes the dialog at once"
        );
        assert!(
            !app.downloads.open,
            "adding a download leaves the list closed"
        );
    });
    assert_eq!(
        persisted(&engine),
        [(URL.to_string(), DownloadState::Running)]
    );
    assert_eq!(fake.requests().len(), 1);
}

#[core::prelude::v1::test]
fn fourth_download_waits_until_a_slot_frees() {
    let (_dir, engine, fake, mut cx) = queue_app("dl_fourth");
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);

    for n in 1..=4 {
        submit(&view, vcx, &format!("https://example.com/{n}.mp4"));
        settle(vcx, &view, "the row is stored", |app| {
            app.downloads.entries.len() == n && app.downloads.entries.iter().all(|e| e.row.id > 0)
        });
    }
    settle(vcx, &view, "three run", |app| {
        count(app, DownloadState::Running) == DEFAULT_DOWNLOAD_CONCURRENCY
    });
    view.update(vcx, |app, _| {
        assert_eq!(
            states(app),
            [
                DownloadState::Running,
                DownloadState::Running,
                DownloadState::Running,
                DownloadState::Waiting
            ],
            "only three download at once; the fourth waits"
        )
    });

    fake.finish("https://example.com/2.mp4", imported(&engine));
    settle(vcx, &view, "the fourth starts", |app| {
        count(app, DownloadState::Running) == DEFAULT_DOWNLOAD_CONCURRENCY
            && count(app, DownloadState::Done) == 1
    });
    assert_eq!(fake.requests().len(), 4);
}

#[core::prelude::v1::test]
fn cancel_while_inspecting_starts_next() {
    let (_dir, engine, fake, mut cx) = queue_app("dl_cancel");
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    for n in 1..=4 {
        submit(&view, vcx, &format!("https://example.com/{n}.mp4"));
        settle(vcx, &view, "the row is stored", |app| {
            app.downloads.entries.len() == n && app.downloads.entries.iter().all(|e| e.row.id > 0)
        });
    }
    settle(vcx, &view, "three run", |app| {
        count(app, DownloadState::Running) == DEFAULT_DOWNLOAD_CONCURRENCY
    });

    // No progress has arrived: the job is still inspecting.
    view.update(vcx, |app, cx| {
        let id = row_id(app, "https://example.com/1.mp4");
        app.cancel_or_remove_download(id, cx);
    });
    settle(vcx, &view, "the waiting row starts", |app| {
        app.downloads.entries.len() == 3 && count(app, DownloadState::Running) == 3
    });
    settle(vcx, &view, "the removal is stored", |_| {
        persisted(&engine).len() == 3
    });
    assert!(
        !persisted(&engine)
            .iter()
            .any(|(url, _)| url == "https://example.com/1.mp4"),
        "✕ removes the row from the stored queue"
    );
}

#[core::prelude::v1::test]
fn finished_job_for_removed_row_is_ignored() {
    let (_dir, engine, fake, mut cx) = queue_app("dl_removed");
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    submit(&view, vcx, URL);
    settle(vcx, &view, "the job runs", |app| {
        count(app, DownloadState::Running) == 1
    });
    // The job ends with a result anyway (it raced the cancel).
    fake.finish(URL, imported(&engine));
    view.update(vcx, |app, cx| {
        let id = row_id(app, URL);
        app.cancel_or_remove_download(id, cx);
    });
    settle(vcx, &view, "the removal is stored", |_| {
        persisted(&engine).is_empty()
    });
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        assert!(
            app.downloads.entries.is_empty(),
            "a removed download must not come back when its job ends"
        )
    });
    assert!(persisted(&engine).is_empty());
}

#[core::prelude::v1::test]
fn import_for_removed_row_still_refreshes_library() {
    let (dir, engine, fake, mut cx) = queue_app("dl_removed_refresh");
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    submit(&view, vcx, URL);
    settle(vcx, &view, "the job runs", |app| {
        count(app, DownloadState::Running) == 1
    });
    // The file made it into the catalog just as ✕ was pressed.
    let item = downloaded_item(&engine, &dir, URL);
    fake.finish(
        URL,
        JobOutcome::Imported {
            item: Box::new(item),
            timeline_error: None,
        },
    );
    view.update(vcx, |app, cx| {
        let id = row_id(app, URL);
        app.cancel_or_remove_download(id, cx);
    });
    settle(vcx, &view, "the library learns of the import", |app| {
        downloaded_count(app) == 1
    });
    view.update(vcx, |app, _| {
        assert!(app.downloads.entries.is_empty(), "the row stays removed")
    });
}

#[core::prelude::v1::test]
fn double_submit_adds_one_row() {
    let (_dir, engine, fake, mut cx) = queue_app("dl_double");
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    view.update(vcx, |app, cx| {
        app.open_network_import_dialog(cx);
        app.submit_network_url(URL.to_string(), cx);
        app.submit_network_url(URL.to_string(), cx);
    });
    vcx.run_until_parked();
    view.update(vcx, |app, cx| {
        app.import_inspected_link(cx);
        app.import_inspected_link(cx);
    });
    settle(vcx, &view, "the job runs", |app| {
        count(app, DownloadState::Running) == 1
    });
    vcx.run_until_parked();
    assert_eq!(persisted(&engine).len(), 1, "one URL, one row");
    view.update(vcx, |app, cx| {
        app.enqueue_download(URL.to_string(), false, None, cx)
    });
    vcx.run_until_parked();
    assert_eq!(
        persisted(&engine).len(),
        1,
        "queueing a URL already in the list adds nothing"
    );
}

#[core::prelude::v1::test]
fn same_url_in_queue_highlights_existing_row() {
    let (_dir, engine, fake, mut cx) = queue_app("dl_same");
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    submit(&view, vcx, URL);
    settle(vcx, &view, "the job runs", |app| {
        count(app, DownloadState::Running) == 1
    });

    submit(&view, vcx, URL);
    view.update(vcx, |app, _| {
        assert!(app.network_import.stage.is_none(), "the dialog closes");
        assert!(app.downloads.open, "the list opens on the existing row");
        assert_eq!(app.downloads.highlight, Some(row_id(app, URL)));
        assert_eq!(app.downloads.entries.len(), 1);
    });
}

pub(crate) fn seed_prior_download(engine: &LuminaEngine, url: &str) -> MediaId {
    let item = engine
        .catalog()
        .get_item_by_id(MediaId(1))
        .unwrap()
        .unwrap();
    engine
        .catalog()
        .insert_with_source(&item, url, None)
        .unwrap()
        .id
}

#[core::prelude::v1::test]
fn prior_download_prompts_and_download_again_skips_check() {
    let (_dir, engine, fake, mut cx) = queue_app("dl_prior");
    let id = seed_prior_download(&engine, URL);
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);

    submit(&view, vcx, URL);
    settle(vcx, &view, "the prompt shows", |app| {
        matches!(
            app.network_import.stage,
            Some(NetworkImportStage::AlreadyDownloaded { .. })
        )
    });
    view.update(vcx, |app, _| {
        let Some(NetworkImportStage::AlreadyDownloaded { prior, .. }) = &app.network_import.stage
        else {
            unreachable!()
        };
        assert_eq!(prior.media_id, id);
        assert!(app.downloads.entries.is_empty(), "nothing queued yet");
    });
    render_ui(vcx);
    assert!(vcx.debug_bounds("net-import-download-again").is_some());

    super::helpers::click(vcx, "net-import-download-again");
    // The row turns Running when its job is spawned, a moment before the
    // job (on its own thread) records the request.
    settle(vcx, &view, "the job runs", |app| {
        count(app, DownloadState::Running) == 1 && !fake.requests().is_empty()
    });
    let requests = fake.requests();
    assert!(
        requests[0].skip_prior_check,
        "Download Again must not stop at the same check again"
    );
}

#[core::prelude::v1::test]
fn trashed_prior_download_downloads_normally() {
    let (_dir, engine, fake, mut cx) = queue_app("dl_trashed");
    let id = seed_prior_download(&engine, URL);
    engine.catalog().set_trashed(id, true).unwrap();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    submit(&view, vcx, URL);
    settle(vcx, &view, "the job runs", |app| {
        count(app, DownloadState::Running) == 1 && !fake.requests().is_empty()
    });
    assert!(!fake.requests()[0].skip_prior_check);
}

#[core::prelude::v1::test]
fn canonical_duplicate_becomes_duplicate_row() {
    let (_dir, engine, fake, mut cx) = queue_app("dl_canonical");
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    submit(&view, vcx, URL);
    settle(vcx, &view, "the job runs", |app| {
        count(app, DownloadState::Running) == 1
    });
    fake.finish(
        URL,
        JobOutcome::AlreadyInLibrary(PriorDownload {
            media_id: MediaId(1),
            path: "/x.mp4".into(),
            imported_epoch_ms: 1,
        }),
    );
    settle(vcx, &view, "the row is a duplicate", |app| {
        count(app, DownloadState::Duplicate) == 1
    });
    settle(vcx, &view, "the duplicate is stored", |_| {
        persisted(&engine) == [(URL.to_string(), DownloadState::Duplicate)]
    });
}

#[core::prelude::v1::test]
fn failure_sets_red_dot_and_retry_requeues_last() {
    let (_dir, engine, fake, mut cx) = queue_app("dl_retry");
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    submit(&view, vcx, URL);
    settle(vcx, &view, "the job runs", |app| {
        count(app, DownloadState::Running) == 1
    });
    fake.finish(URL, JobOutcome::Failed("This video is private".into()));
    settle(vcx, &view, "the row fails", |app| {
        count(app, DownloadState::Failed) == 1
    });
    view.update(vcx, |app, _| {
        assert!(app.downloads.unseen_failure, "a failure shows the red dot");
        let row = &app.downloads.entries[0].row;
        assert_eq!(row.error.as_deref(), Some("This video is private"));
        assert!(row.can_retry);
    });

    submit(&view, vcx, "https://example.com/b.mp4");
    settle(vcx, &view, "the second runs", |app| {
        count(app, DownloadState::Running) == 1
    });
    view.update(vcx, |app, cx| {
        let id = row_id(app, URL);
        app.retry_download(id, cx);
    });
    settle(vcx, &view, "the retry runs", |app| {
        count(app, DownloadState::Running) == 2
    });
    view.update(vcx, |app, _| {
        let urls: Vec<&str> = app
            .downloads
            .entries
            .iter()
            .map(|e| e.row.url.as_str())
            .collect();
        assert_eq!(
            urls,
            ["https://example.com/b.mp4", URL],
            "Retry moves the download to the end of the queue"
        );
    });
}

#[core::prelude::v1::test]
fn missing_helper_failure_has_no_retry() {
    let (_dir, engine, fake, mut cx) = queue_app("dl_helper");
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    submit(&view, vcx, URL);
    settle(vcx, &view, "the job runs", |app| {
        count(app, DownloadState::Running) == 1
    });
    fake.finish(URL, JobOutcome::MissingHelper);
    settle(vcx, &view, "the row fails", |app| {
        count(app, DownloadState::Failed) == 1
    });
    view.update(vcx, |app, _| {
        let row = &app.downloads.entries[0].row;
        assert!(!row.can_retry, "Retry can't fix a damaged install");
        assert_eq!(
            row.error.as_deref(),
            Some(lumina_core::net_import::missing_part())
        );
    });
}

#[core::prelude::v1::test]
fn rows_persist_states() {
    let (_dir, engine, fake, mut cx) = queue_app("dl_persist");
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    submit(&view, vcx, URL);
    settle(vcx, &view, "the job runs", |app| {
        count(app, DownloadState::Running) == 1
    });
    assert_eq!(
        persisted(&engine),
        [(URL.to_string(), DownloadState::Running)]
    );
    fake.finish(URL, imported(&engine));
    settle(vcx, &view, "the row is done", |app| {
        count(app, DownloadState::Done) == 1
    });
    let rows = engine.catalog().list_downloads().unwrap();
    assert_eq!(rows[0].state, DownloadState::Done);
    assert_eq!(rows[0].media_id, Some(MediaId(1)));
    assert!(rows[0].finished_epoch_ms.is_some());
}

#[core::prelude::v1::test]
fn dialog_opens_while_downloads_run() {
    let (_dir, engine, fake, mut cx) = queue_app("dl_dialog");
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    submit(&view, vcx, URL);
    settle(vcx, &view, "the job runs", |app| {
        count(app, DownloadState::Running) == 1
    });
    view.update(vcx, |app, cx| app.open_network_import_dialog(cx));
    view.update(vcx, |app, _| {
        assert!(
            matches!(
                app.network_import.stage,
                Some(NetworkImportStage::Input { .. })
            ),
            "the dialog opens while a download runs"
        )
    });
}

fn quit_via_menu(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) {
    super::helpers::focus_app(vcx, view);
    vcx.dispatch_action(crate::actions::QuitApp);
    vcx.run_until_parked();
}

#[core::prelude::v1::test]
fn quit_with_running_downloads_asks_first() {
    let (_dir, engine, fake, mut cx) = queue_app("dl_quit_ask");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    submit(&view, vcx, URL);
    settle(vcx, &view, "the job runs", |app| {
        count(app, DownloadState::Running) == 1
    });
    quit_via_menu(&view, vcx);
    view.update(vcx, |app, _| {
        assert!(
            matches!(app.downloads.confirm, Some(DownloadsConfirm::Quit)),
            "quitting with a running download asks first"
        );
        assert!(!app.downloads.quit_requested, "the app has not quit yet");
    });
    render_ui(vcx);
    super::helpers::click(vcx, "downloads-confirm-ok");
    view.update(vcx, |app, _| {
        assert!(app.downloads.quit_requested, "Quit confirms");
        assert!(app.downloads.entries.is_empty());
    });
}

#[core::prelude::v1::test]
fn quit_without_downloads_quits_at_once() {
    let (_dir, engine, _fake, mut cx) = queue_app("dl_quit_now");
    let (view, vcx) = app_with_library(&mut cx, engine);
    quit_via_menu(&view, vcx);
    view.update(vcx, |app, _| {
        assert!(app.downloads.confirm.is_none(), "nothing to ask about");
        assert!(app.downloads.quit_requested, "the app quits at once");
    });
}

#[core::prelude::v1::test]
fn closing_the_main_window_quits_without_downloads() {
    let (_dir, engine, _fake, mut cx) = queue_app("dl_close_now");
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        assert!(
            !app.should_close_main_window(cx),
            "the platform close is cancelled; quitting closes the app"
        );
        assert!(app.downloads.quit_requested, "the app quits at once");
    });
}

#[core::prelude::v1::test]
fn closing_the_main_window_asks_about_running_downloads() {
    let (_dir, engine, fake, mut cx) = queue_app("dl_close_ask");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    submit(&view, vcx, URL);
    settle(vcx, &view, "the job runs", |app| {
        count(app, DownloadState::Running) == 1
    });
    view.update(vcx, |app, cx| {
        assert!(
            !app.should_close_main_window(cx),
            "the window stays open while the prompt is up"
        );
        assert!(
            matches!(app.downloads.confirm, Some(DownloadsConfirm::Quit)),
            "closing with a running download asks first"
        );
        assert!(!app.downloads.quit_requested, "the app has not quit yet");
    });
    render_ui(vcx);
    super::helpers::click(vcx, "downloads-confirm-ok");
    view.update(vcx, |app, _| {
        assert!(app.downloads.quit_requested, "confirming quits");
        assert!(app.downloads.entries.is_empty());
    });
}

#[core::prelude::v1::test]
fn confirm_escape_and_backdrop_cancel() {
    let (_dir, engine, fake, mut cx) = queue_app("dl_quit_cancel");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    submit(&view, vcx, URL);
    settle(vcx, &view, "the job runs", |app| {
        count(app, DownloadState::Running) == 1
    });
    quit_via_menu(&view, vcx);
    super::helpers::keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(app.downloads.confirm.is_none(), "Esc cancels the prompt");
        assert!(!app.downloads.quit_requested);
    });
    quit_via_menu(&view, vcx);
    render_ui(vcx);
    super::helpers::click_point(vcx, 20.0, 20.0);
    view.update(vcx, |app, _| {
        assert!(app.downloads.confirm.is_none(), "the backdrop cancels");
        assert!(!app.downloads.quit_requested);
        assert_eq!(
            states(app),
            [DownloadState::Running],
            "the download keeps going"
        );
    });
}

#[core::prelude::v1::test]
fn downloads_confirm_buttons_in_footer() {
    let (_dir, engine, fake, mut cx) = queue_app("dl_quit_footer");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    submit(&view, vcx, URL);
    settle(vcx, &view, "the job runs", |app| {
        count(app, DownloadState::Running) == 1
    });
    quit_via_menu(&view, vcx);
    render_ui(vcx);
    let footer = vcx
        .debug_bounds("downloads-confirm-footer")
        .expect("the prompt has a footer bar");
    let card = vcx
        .debug_bounds("downloads-confirm-card")
        .expect("the prompt card paints");
    assert!(
        footer.bottom() >= card.bottom() - gpui::px(2.0),
        "the footer is the card's bottom bar"
    );
    for button in ["downloads-confirm-cancel", "downloads-confirm-ok"] {
        let bounds = vcx
            .debug_bounds(button)
            .unwrap_or_else(|| panic!("{button} paints"));
        assert!(
            footer.contains(&bounds.center()),
            "{button} sits in the footer"
        );
    }
}

#[core::prelude::v1::test]
fn paused_rows_load_without_starting() {
    let (dir, engine, fake, mut cx) = queue_app("dl_reopen");
    for (url, state) in [
        (URL, DownloadState::Running),
        (URL_B, DownloadState::Waiting),
    ] {
        let mut row = engine.catalog().add_download(url).unwrap();
        row.state = state;
        engine.catalog().update_download(&row).unwrap();
    }
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| app.open_library_inner(dir.clone(), cx));
    vcx.run_until_parked();
    install(&view, vcx, &fake);
    settle(vcx, &view, "the stored rows load", |app| {
        app.downloads.entries.len() == 2
    });
    view.update(vcx, |app, _| {
        assert_eq!(
            states(app),
            [DownloadState::Paused, DownloadState::Paused],
            "unfinished downloads come back paused"
        );
    });
    vcx.run_until_parked();
    assert!(fake.requests().is_empty(), "nothing downloads at launch");
}

#[core::prelude::v1::test]
fn resume_all_starts_up_to_three() {
    let (dir, engine, fake, mut cx) = queue_app("dl_resume_all");
    for n in 0..4 {
        let mut row = engine
            .catalog()
            .add_download(&format!("https://example.com/{n}.mp4"))
            .unwrap();
        row.state = DownloadState::Paused;
        engine.catalog().update_download(&row).unwrap();
    }
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| app.open_library_inner(dir.clone(), cx));
    vcx.run_until_parked();
    install(&view, vcx, &fake);
    settle(vcx, &view, "the stored rows load", |app| {
        app.downloads.entries.len() == 4
    });
    view.update(vcx, |app, cx| app.resume_all_downloads(cx));
    settle(vcx, &view, "three run, one waits", |app| {
        count(app, DownloadState::Running) == DEFAULT_DOWNLOAD_CONCURRENCY
            && count(app, DownloadState::Waiting) == 1
    });
}

#[core::prelude::v1::test]
fn switching_library_drops_old_jobs() {
    let (_dir, engine, fake, mut cx) = queue_app("dl_switch");
    let (dir_b, engine_b) = seed_library("dl_switch_b", 1);
    drop(engine_b);
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    submit(&view, vcx, URL);
    settle(vcx, &view, "the job runs", |app| {
        count(app, DownloadState::Running) == 1
    });
    view.update(vcx, |app, cx| app.open_library(dir_b.clone(), cx));
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        assert!(
            matches!(
                app.downloads.confirm,
                Some(DownloadsConfirm::OpenLibrary(_))
            ),
            "switching library with a running download asks first"
        );
    });
    // A late outcome from the old job must not land anywhere new.
    fake.finish(URL, JobOutcome::Failed("late".into()));
    view.update(vcx, |app, cx| app.confirm_downloads_prompt(cx));
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        assert!(
            app.downloads.entries.is_empty(),
            "the old library's rows are gone"
        );
        let engine = app.grid_state.as_ref().unwrap().engine.clone();
        assert!(
            engine.catalog().list_downloads().unwrap().is_empty(),
            "the old job never writes into the new library"
        );
    });
}

/// A second item in the seeded library, catalogued as downloaded from `url`.
fn downloaded_item(
    engine: &LuminaEngine,
    dir: &std::path::Path,
    url: &str,
) -> lumina_core::MediaItem {
    let mut item = engine
        .catalog()
        .get_item_by_id(MediaId(1))
        .unwrap()
        .expect("the seeded library has one item");
    item.path = dir.join("Imports").join("clip.mp4");
    std::fs::create_dir_all(item.path.parent().unwrap()).unwrap();
    std::fs::write(&item.path, b"clip").unwrap();
    engine
        .catalog()
        .insert_with_source(&item, url, None)
        .unwrap()
}

fn downloaded_count(app: &LuminaApp) -> usize {
    app.system_album_counts
        .get(&lumina_core::SystemAlbum::Downloaded)
        .copied()
        .unwrap_or(0)
}

#[core::prelude::v1::test]
fn downloaded_album_appears_after_a_download() {
    let (_dir, engine, _fake, mut cx) = queue_app("dl_album_row");
    engine
        .catalog()
        .record_media_source(MediaId(1), "https://example.com/sidecar", None)
        .unwrap();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    view.update(vcx, |app, _| assert_eq!(downloaded_count(app), 0));
    assert!(
        vcx.debug_bounds("sidebar-nav-downloaded").is_none(),
        "a sidecar source alone is not a download"
    );

    downloaded_item(&engine, &_dir, URL);
    view.update(vcx, |app, cx| {
        app.refresh_album_counts();
        cx.notify();
    });
    render_ui(vcx);
    view.update(vcx, |app, _| assert_eq!(downloaded_count(app), 1));
    assert!(
        vcx.debug_bounds("sidebar-nav-downloaded").is_some(),
        "the sidebar lists Downloaded once something was downloaded"
    );
}

fn assert_shows_in_downloaded(app: &LuminaApp, id: MediaId) {
    assert_eq!(
        app.selection,
        crate::views::sidebar::AlbumSelection::System(lumina_core::SystemAlbum::Downloaded),
        "Show in Library opens the Downloaded album"
    );
    assert_eq!(
        app.selected_item.as_ref().map(|item| item.id),
        Some(id),
        "and selects the downloaded item"
    );
}

#[core::prelude::v1::test]
fn done_row_click_selects_item_in_downloaded_album() {
    let (dir, engine, fake, mut cx) = queue_app("dl_show_row");
    let item = downloaded_item(&engine, &dir, URL);
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    view.update(vcx, |app, cx| {
        app.enqueue_download(URL.to_string(), true, None, cx)
    });
    settle(vcx, &view, "the job runs", |app| {
        count(app, DownloadState::Running) == 1
    });
    fake.finish(
        URL,
        JobOutcome::Imported {
            item: Box::new(item.clone()),
            timeline_error: None,
        },
    );
    settle(vcx, &view, "the row is done", |app| {
        count(app, DownloadState::Done) == 1
    });
    view.update(vcx, |app, cx| app.toggle_downloads_panel(cx));
    render_ui(vcx);
    let id = view.update(vcx, |app, _| row_id(app, URL));
    super::helpers::click(
        vcx,
        Box::leak(format!("download-row-{id}").into_boxed_str()),
    );
    view.update(vcx, |app, _| {
        assert_shows_in_downloaded(app, item.id);
        assert!(!app.downloads.open, "the list closes");
    });
}

#[core::prelude::v1::test]
fn done_row_for_trashed_item_says_removed() {
    let (dir, engine, fake, mut cx) = queue_app("dl_show_trashed");
    let item = downloaded_item(&engine, &dir, URL);
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    view.update(vcx, |app, cx| {
        app.enqueue_download(URL.to_string(), true, None, cx)
    });
    settle(vcx, &view, "the job runs", |app| {
        count(app, DownloadState::Running) == 1
    });
    fake.finish(
        URL,
        JobOutcome::Imported {
            item: Box::new(item.clone()),
            timeline_error: None,
        },
    );
    settle(vcx, &view, "the row is done", |app| {
        count(app, DownloadState::Done) == 1
    });
    // Trashed later in the same session.
    engine.set_trashed(item.id, true).unwrap();
    view.update(vcx, |app, cx| {
        let id = row_id(app, URL);
        app.download_main_action(id, cx);
    });
    view.update(vcx, |app, _| {
        assert!(
            app.selected_item.is_none(),
            "nothing is selected for a trashed item"
        );
        assert!(
            !app.downloads.entries[0].row.in_library,
            "the row learns the item left the library, so it says so"
        );
    });
}

#[core::prelude::v1::test]
fn already_downloaded_enter_shows_in_library() {
    let (dir, engine, fake, mut cx) = queue_app("dl_show_dialog");
    let item = downloaded_item(&engine, &dir, URL);
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    submit(&view, vcx, URL);
    settle(vcx, &view, "the dialog says it was downloaded", |app| {
        matches!(
            app.network_import.stage,
            Some(NetworkImportStage::AlreadyDownloaded { .. })
        )
    });
    super::helpers::focus_app(vcx, &view);
    super::helpers::keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert!(app.network_import.stage.is_none(), "the dialog closes");
        assert_shows_in_downloaded(app, item.id);
        assert!(app.downloads.entries.is_empty(), "nothing is queued");
    });
}

#[core::prelude::v1::test]
fn finished_rows_keep_title_and_state() {
    let (_dir, engine, fake, mut cx) = queue_app("dl_title");
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    // Queued before the job starts, so it ends right after inspecting: the
    // title write and the final write race.
    let prior = PriorDownload {
        media_id: MediaId(1),
        path: std::path::PathBuf::from("/x.mp4"),
        imported_epoch_ms: 0,
    };
    fake.finish(URL, JobOutcome::AlreadyInLibrary(prior));
    view.update(vcx, |app, cx| {
        app.enqueue_download(URL.to_string(), false, None, cx)
    });
    settle(vcx, &view, "the row is a duplicate", |app| {
        count(app, DownloadState::Duplicate) == 1
    });
    view.update(vcx, |app, _| {
        assert_eq!(
            app.downloads.entries[0].row.title.as_deref(),
            Some(title_for(URL).as_str()),
            "a finished row keeps the title it showed while running"
        );
    });
    settle(vcx, &view, "every write lands", |_| {
        let rows = engine.catalog().list_downloads().unwrap();
        rows[0].title.is_some()
    });
    let rows = engine.catalog().list_downloads().unwrap();
    assert_eq!(
        rows[0].state,
        DownloadState::Duplicate,
        "the title write never puts a finished row back to running"
    );
    assert_eq!(rows[0].title.as_deref(), Some(title_for(URL).as_str()));
}

#[core::prelude::v1::test]
fn retry_pressed_twice_runs_one_job() {
    let (_dir, engine, fake, mut cx) = queue_app("dl_retry_twice");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    submit(&view, vcx, URL);
    settle(vcx, &view, "the job runs", |app| {
        count(app, DownloadState::Running) == 1
    });
    fake.finish(URL, JobOutcome::Failed("The connection dropped".into()));
    settle(vcx, &view, "it fails", |app| {
        count(app, DownloadState::Failed) == 1
    });
    let id = view.update(vcx, |app, _| row_id(app, URL));
    // Both presses land before either requeue write returns.
    view.update(vcx, |app, cx| {
        app.retry_download(id, cx);
        app.retry_download(id, cx);
    });
    settle(vcx, &view, "the retry runs", |app| {
        count(app, DownloadState::Running) == 1
    });
    vcx.run_until_parked();
    let started = fake.requests().iter().filter(|r| r.url == URL).count();
    assert_eq!(started, 2, "one first try and one retry, never two retries");
}

#[core::prelude::v1::test]
fn quit_prompt_over_delete_confirm_takes_enter() {
    let (_dir, engine, fake, mut cx) = queue_app("dl_quit_over_delete");
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    submit(&view, vcx, URL);
    settle(vcx, &view, "the job runs", |app| {
        count(app, DownloadState::Running) == 1
    });
    view.update(vcx, |app, cx| {
        let item = app.get_active_slice(0..1).remove(0);
        app.select_item_at(item, 0, cx);
        app.prompt_delete_selected(cx);
        assert!(app.delete_confirm_state.is_some());
        // Quit from the menu while the delete prompt is up.
        app.request_quit(cx);
    });
    super::helpers::focus_app(vcx, &view);
    super::helpers::keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert!(app.downloads.quit_requested, "↵ answers the prompt on top");
    });
    assert_eq!(
        engine.query_system_album_count(lumina_core::SystemAlbum::RecentlyDeleted),
        0,
        "the delete prompt underneath never takes the keystroke"
    );
}

#[core::prelude::v1::test]
fn url_dialog_waits_while_downloads_list_is_open() {
    let (_dir, engine, fake, mut cx) = queue_app("dl_dialog_over_list");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    submit(&view, vcx, URL);
    settle(vcx, &view, "the job runs", |app| {
        count(app, DownloadState::Running) == 1
    });
    view.update(vcx, |app, cx| {
        app.toggle_downloads_panel(cx);
        app.open_network_import_dialog(cx);
        assert!(
            app.network_import.stage.is_none(),
            "the URL dialog doesn't open under the downloads list"
        );
    });
}

#[core::prelude::v1::test]
fn command_enter_downloads_again_from_dialog_and_list() {
    let (dir, engine, fake, mut cx) = queue_app("dl_again_keys");
    downloaded_item(&engine, &dir, URL);
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    submit(&view, vcx, URL);
    settle(vcx, &view, "the dialog says it was downloaded", |app| {
        matches!(
            app.network_import.stage,
            Some(NetworkImportStage::AlreadyDownloaded { .. })
        )
    });
    super::helpers::focus_app(vcx, &view);
    super::helpers::keystrokes(vcx, "secondary-enter");
    settle(vcx, &view, "⌘↵ downloads it again", |app| {
        count(app, DownloadState::Running) == 1
    });
    view.update(vcx, |app, _| {
        assert!(app.network_import.stage.is_none(), "the dialog closes");
        assert!(app.downloads.entries[0].skip_prior_check);
    });

    // In the list: a Duplicate row gets ⌘↵ too.
    let prior = PriorDownload {
        media_id: MediaId(1),
        path: std::path::PathBuf::from("/x.mp4"),
        imported_epoch_ms: 0,
    };
    fake.finish(URL, JobOutcome::AlreadyInLibrary(prior));
    settle(vcx, &view, "the row is a duplicate", |app| {
        count(app, DownloadState::Duplicate) == 1
    });
    view.update(vcx, |app, cx| app.toggle_downloads_panel(cx));
    super::helpers::keystrokes(vcx, "secondary-enter");
    settle(vcx, &view, "⌘↵ on the row downloads it again", |app| {
        count(app, DownloadState::Running) == 1
    });
}

#[core::prelude::v1::test]
fn concurrency_setting_limits_running_jobs() {
    let (_dir, engine, fake, mut cx) = queue_app("dl_concurrency");
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    view.update(vcx, |app, cx| app.set_download_concurrency(1, cx));
    let url = |n: usize| format!("https://example.com/{n}.mp4");
    let tally = |app: &LuminaApp| {
        (
            count(app, DownloadState::Running),
            count(app, DownloadState::Waiting),
        )
    };

    for n in 1..=4 {
        submit(&view, vcx, &url(n));
        settle(vcx, &view, "the row is stored", |app| {
            app.downloads.entries.len() == n && app.downloads.entries.iter().all(|e| e.row.id > 0)
        });
    }
    settle(vcx, &view, "one runs", |app| tally(app) == (1, 3));
    assert_eq!(
        fake.requests().len(),
        1,
        "with the setting at 1 only one of four queued rows runs"
    );

    view.update(vcx, |app, cx| app.set_download_concurrency(3, cx));
    settle(vcx, &view, "raising to 3 starts two more at once", |app| {
        tally(app) == (3, 1)
    });

    view.update(vcx, |app, cx| app.set_download_concurrency(1, cx));
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            tally(app),
            (3, 1),
            "lowering the setting never cancels running jobs"
        )
    });

    // Slots free up, but nothing new starts until fewer than 1 run.
    for (n, running) in [(1, 2), (2, 1)] {
        fake.finish(&url(n), imported(&engine));
        settle(vcx, &view, "the finished job ends", |app| {
            count(app, DownloadState::Done) == n
        });
        view.update(vcx, |app, _| {
            assert_eq!(
                tally(app),
                (running, 1),
                "a freed slot above the lowered setting starts nothing"
            )
        });
        assert_eq!(fake.requests().len(), 3, "no new job was requested");
    }
    fake.finish(&url(3), imported(&engine));
    settle(
        vcx,
        &view,
        "the last running job ends; the waiting row starts",
        |app| count(app, DownloadState::Done) == 3 && tally(app) == (1, 0),
    );
    assert_eq!(fake.requests().len(), 4);
}
