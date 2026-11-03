//! The URL download queue: scheduling, removal, retry, duplicates and
//! persistence, driven through a fake job so nothing touches the network.

use super::ui_automation::{app_with_library, render_ui, seed_library};
use crate::app::downloads::{DownloadsConfirm, JobFn, JobOutcome, JobRequest, MAX_RUNNING};
use crate::app::network_import::NetworkImportStage;
use crate::LuminaApp;
use gpui::{Entity, VisualTestContext};
use lumina_core::{DownloadState, LuminaEngine, MediaId, PriorDownload};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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
        Arc::new(move |request, cancel, _events| {
            fake.requests.lock().unwrap().push(request.clone());
            let rx = fake.channel(&request.url).1;
            loop {
                if cancel.load(Ordering::Relaxed) {
                    return JobOutcome::Cancelled;
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

/// A seeded test library, deleted when the test ends (pass or fail).
pub(crate) struct TempLibrary(std::path::PathBuf);

impl std::ops::Deref for TempLibrary {
    type Target = std::path::PathBuf;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for TempLibrary {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(crate) fn queue_app(
    name: &str,
) -> (
    TempLibrary,
    Arc<LuminaEngine>,
    FakeJobs,
    gpui::TestAppContext,
) {
    let (dir, engine) = seed_library(name, 1);
    (
        TempLibrary(dir),
        engine,
        FakeJobs::default(),
        gpui::TestAppContext::single(),
    )
}

pub(crate) fn install(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, fake: &FakeJobs) {
    let job = fake.job();
    view.update(vcx, |app, _| app.downloads.job = Some(job));
}

pub(crate) fn submit(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, url: &str) {
    view.update(vcx, |app, cx| {
        app.open_network_import_dialog(cx);
        app.submit_network_url(url.to_string(), cx);
    });
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
        count(app, DownloadState::Running) == MAX_RUNNING
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
        count(app, DownloadState::Running) == MAX_RUNNING && count(app, DownloadState::Done) == 1
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
        count(app, DownloadState::Running) == MAX_RUNNING
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
    view.update(vcx, |app, cx| {
        let id = row_id(app, URL);
        app.cancel_or_remove_download(id, cx);
    });
    // The job ends with a result anyway (it raced the cancel).
    fake.finish(URL, imported(&engine));
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
fn double_submit_adds_one_row() {
    let (_dir, engine, fake, mut cx) = queue_app("dl_double");
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    view.update(vcx, |app, cx| {
        app.open_network_import_dialog(cx);
        app.submit_network_url(URL.to_string(), cx);
        app.submit_network_url(URL.to_string(), cx);
    });
    settle(vcx, &view, "the job runs", |app| {
        count(app, DownloadState::Running) == 1
    });
    vcx.run_until_parked();
    assert_eq!(persisted(&engine).len(), 1, "one URL, one row");
    view.update(vcx, |app, cx| {
        app.enqueue_download(URL.to_string(), false, cx)
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

fn seed_prior_download(engine: &LuminaEngine, url: &str) -> MediaId {
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

    super::ui_automation::click(vcx, "net-import-download-again");
    settle(vcx, &view, "the job runs", |app| {
        count(app, DownloadState::Running) == 1
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
        count(app, DownloadState::Running) == 1
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
            Some(lumina_core::net_import::MISSING_PART)
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
    super::ui_automation::focus_app(vcx, view);
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
    super::ui_automation::click(vcx, "downloads-confirm-ok");
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
fn confirm_escape_and_backdrop_cancel() {
    let (_dir, engine, fake, mut cx) = queue_app("dl_quit_cancel");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    submit(&view, vcx, URL);
    settle(vcx, &view, "the job runs", |app| {
        count(app, DownloadState::Running) == 1
    });
    quit_via_menu(&view, vcx);
    super::ui_automation::keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(app.downloads.confirm.is_none(), "Esc cancels the prompt");
        assert!(!app.downloads.quit_requested);
    });
    quit_via_menu(&view, vcx);
    render_ui(vcx);
    super::ui_automation::click_point(vcx, 20.0, 20.0);
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
    install(&view, vcx, &fake);
    settle(vcx, &view, "the stored rows load", |app| {
        app.downloads.entries.len() == 4
    });
    view.update(vcx, |app, cx| app.resume_all_downloads(cx));
    settle(vcx, &view, "three run, one waits", |app| {
        count(app, DownloadState::Running) == MAX_RUNNING && count(app, DownloadState::Waiting) == 1
    });
}

#[core::prelude::v1::test]
fn switching_library_drops_old_jobs() {
    let (_dir, engine, fake, mut cx) = queue_app("dl_switch");
    let (dir_b, engine_b) = seed_library("dl_switch_b", 1);
    let dir_b = TempLibrary(dir_b);
    drop(engine_b);
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    submit(&view, vcx, URL);
    settle(vcx, &view, "the job runs", |app| {
        count(app, DownloadState::Running) == 1
    });
    view.update(vcx, |app, cx| app.open_library(dir_b.clone(), cx));
    view.update(vcx, |app, _| {
        assert!(
            matches!(
                app.downloads.confirm,
                Some(DownloadsConfirm::OpenLibrary(_))
            ),
            "switching library with a running download asks first"
        );
    });
    view.update(vcx, |app, cx| app.confirm_downloads_prompt(cx));
    // A late outcome from the old job must not land anywhere new.
    fake.finish(URL, JobOutcome::Failed("late".into()));
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
