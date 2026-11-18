//! A paused run whose job ignores the cancel flag (cataloguing, the
//! prior-download lookup) and ends late: it keeps its slot until then, and
//! its ending never lands on the row's next run. Driven through a fake job.

use super::downloads::{install, queue_app, row_id, settle, submit};
use super::helpers::app_with_library;
use crate::app::downloads::{DownloadsConfirm, JobFn, JobOutcome};
use crate::LuminaApp;
use gpui::{Entity, VisualTestContext};
use lumina_core::DownloadState;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const ONE: &str = "https://example.com/1.mp4";
const TWO: &str = "https://example.com/2.mp4";

/// The first run of `ONE` ignores Pause and ends with what the test sends
/// on `end`; every other run stops on its cancel flag. `live` counts job
/// threads still running, `started` every run of `ONE`.
struct Stubborn {
    end: Sender<JobOutcome>,
    live: Arc<AtomicUsize>,
    started: Arc<AtomicUsize>,
}

fn stubborn(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> Stubborn {
    let (end, rx) = channel::<JobOutcome>();
    let rx = Arc::new(Mutex::new(rx));
    let live = Arc::new(AtomicUsize::new(0));
    let started = Arc::new(AtomicUsize::new(0));
    let first = Arc::new(AtomicBool::new(true));
    let job: JobFn = {
        let (live, started) = (live.clone(), started.clone());
        Arc::new(move |request, cancel, _events| {
            live.fetch_add(1, Ordering::SeqCst);
            let one = request.url == ONE;
            if one {
                started.fetch_add(1, Ordering::SeqCst);
            }
            let outcome = if one && first.swap(false, Ordering::SeqCst) {
                rx.lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(10))
                    .unwrap_or(JobOutcome::Cancelled)
            } else {
                while !cancel.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(2));
                }
                JobOutcome::Cancelled
            };
            live.fetch_sub(1, Ordering::SeqCst);
            outcome
        })
    };
    view.update(vcx, |app, cx| {
        app.downloads.job = Some(job);
        app.set_download_concurrency(1, cx);
    });
    Stubborn { end, live, started }
}

fn state(app: &LuminaApp, url: &str) -> DownloadState {
    let id = row_id(app, url);
    app.downloads
        .entries
        .iter()
        .find(|e| e.row.id == id)
        .unwrap()
        .row
        .state
}

fn queue(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, url: &str, want: DownloadState) {
    submit(view, vcx, url);
    settle(vcx, view, "the row is stored", |app| {
        app.downloads.entries.iter().all(|e| e.row.id > 0) && state(app, url) == want
    });
}

#[core::prelude::v1::test]
fn paused_row_holds_its_slot_until_its_job_ends() {
    let (_dir, engine, fake, mut cx) = queue_app("dl_late_slot");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    let run = stubborn(&view, vcx);
    queue(&view, vcx, ONE, DownloadState::Running);
    queue(&view, vcx, TWO, DownloadState::Waiting);

    view.update(vcx, |app, cx| app.pause_download(row_id(app, ONE), cx));
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        assert_eq!(
            state(app, TWO),
            DownloadState::Waiting,
            "a paused row whose job is still finishing keeps its slot"
        )
    });
    assert_eq!(run.live.load(Ordering::SeqCst), 1, "no extra job starts");

    // Only the paused, still-finishing row is left: quitting would cut it.
    view.update(vcx, |app, cx| app.pause_download(row_id(app, TWO), cx));
    settle(vcx, &view, "url 2 is paused", |app| {
        state(app, TWO) == DownloadState::Paused
    });
    view.update(vcx, |app, cx| {
        app.request_quit(cx);
        assert_eq!(
            app.downloads.confirm,
            Some(DownloadsConfirm::Quit),
            "quit warns while a paused row's job is still running"
        );
        assert!(!app.downloads.quit_requested);
        app.dismiss_downloads_prompt(cx);
        app.resume_download(row_id(app, TWO), cx);
    });
    settle(vcx, &view, "url 2 waits again", |app| {
        state(app, TWO) == DownloadState::Waiting
    });

    // The job really ends: the slot frees.
    run.end.send(JobOutcome::Cancelled).unwrap();
    settle(vcx, &view, "url 2 starts", |app| {
        state(app, TWO) == DownloadState::Running
    });
    assert_eq!(run.live.load(Ordering::SeqCst), 1, "one job at a time");
}

#[core::prelude::v1::test]
fn requeued_row_ignores_the_old_runs_result() {
    let (_dir, engine, fake, mut cx) = queue_app("dl_late_cancel");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    let run = stubborn(&view, vcx);
    queue(&view, vcx, ONE, DownloadState::Running);
    queue(&view, vcx, TWO, DownloadState::Waiting);

    view.update(vcx, |app, cx| app.pause_download(row_id(app, ONE), cx));
    vcx.run_until_parked();
    view.update(vcx, |app, cx| app.resume_download(row_id(app, ONE), cx));
    settle(vcx, &view, "url 1 waits again", |app| {
        state(app, ONE) == DownloadState::Waiting
    });

    run.end
        .send(JobOutcome::Failed("The old run failed.".to_string()))
        .unwrap();
    settle(vcx, &view, "the freed slot starts url 2", |app| {
        state(app, TWO) == DownloadState::Running
    });
    view.update(vcx, |app, _| {
        let entry = &app.downloads.entries[app
            .downloads
            .entries
            .iter()
            .position(|e| e.row.url == ONE)
            .unwrap()];
        assert_eq!(
            (entry.row.state, entry.row.error.as_deref()),
            (DownloadState::Waiting, None),
            "the requeued row's new run owns it; the old run's result is dropped"
        );
    });
    assert_eq!(run.started.load(Ordering::SeqCst), 1);
}

/// Pause, Resume, then the paused run saves a file it can't catalogue.
/// `resume_first`: Resume's write lands before that result, or after it.
fn late_not_catalogued(name: &str, resume_first: bool) {
    let (_dir, engine, fake, mut cx) = queue_app(name);
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    let run = stubborn(&view, vcx);
    queue(&view, vcx, ONE, DownloadState::Running);
    let stored = || {
        engine
            .catalog()
            .list_downloads()
            .unwrap()
            .into_iter()
            .find(|row| row.url == ONE)
            .unwrap()
    };

    view.update(vcx, |app, cx| app.pause_download(row_id(app, ONE), cx));
    vcx.run_until_parked();
    // Resume's write runs on the background executor, which only moves
    // when the test parks.
    view.update(vcx, |app, cx| app.resume_download(row_id(app, ONE), cx));
    if resume_first {
        settle(vcx, &view, "Resume's write lands", |app| {
            state(app, ONE) == DownloadState::Waiting
        });
    }
    run.end
        .send(JobOutcome::NotCatalogued {
            path: "/nowhere/Imports/clip.mp4".into(),
        })
        .unwrap();
    if !resume_first {
        // The job's thread stores its result before Resume's write runs.
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while stored().state != DownloadState::Failed {
            assert!(std::time::Instant::now() < deadline, "the result is stored");
            std::thread::yield_now();
        }
    }
    settle(
        vcx,
        &view,
        "the saved file's result lands on the row",
        |app| state(app, ONE) == DownloadState::Failed,
    );
    vcx.run_until_parked();
    let check = |row: &lumina_core::DownloadRow, what: &str| {
        let error = row.error.as_deref().unwrap_or_default();
        assert!(
            error.contains("\"clip.mp4\"") && error.contains("Imports folder"),
            "{what} keeps why the saved file isn't in the library: {error:?}"
        );
        assert!(!row.can_retry, "{what}: Retry would download it again");
        assert_eq!(row.state, DownloadState::Failed, "{what} ends failed");
    };
    view.update(vcx, |app, _| {
        check(&app.downloads.entries[0].row, "the row")
    });
    check(&stored(), "the stored row");
    assert_eq!(
        run.started.load(Ordering::SeqCst),
        1,
        "the saved file is never downloaded again"
    );
}

#[core::prelude::v1::test]
fn late_not_catalogued_result_keeps_its_message() {
    late_not_catalogued("dl_late_uncatalogued", true);
}

#[core::prelude::v1::test]
fn late_not_catalogued_result_before_resume_lands_keeps_its_message() {
    late_not_catalogued("dl_late_uncatalogued_rev", false);
}

/// A job thread that dies without reporting (a panic) frees its slot.
#[core::prelude::v1::test]
fn panicked_job_frees_its_slot() {
    let (_dir, engine, fake, mut cx) = queue_app("dl_job_panics");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    let job: JobFn = Arc::new(|request, cancel, _events| {
        if request.url == ONE {
            panic!("a job that dies without reporting (expected in this test)");
        }
        while !cancel.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(2));
        }
        JobOutcome::Cancelled
    });
    view.update(vcx, |app, cx| {
        app.downloads.job = Some(job);
        app.set_download_concurrency(1, cx);
    });
    submit(&view, vcx, ONE);
    queue(&view, vcx, TWO, DownloadState::Running);
}
