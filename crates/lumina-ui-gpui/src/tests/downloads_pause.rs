//! Pause and Pause all: a paused download stops its job, keeps its row and
//! comes back on Resume, which starts over. Driven through the fake job.

use super::downloads::{
    imported, install, persisted, queue_app, row_id, settle, states, submit, FakeJobs,
};
use super::ui_automation::{app_with_library, click, focus_app, keystrokes, render_ui};
use crate::app::command_palette::{CommandId, PaletteTarget};
use crate::app::downloads::{JobFn, JobOutcome, MAX_RUNNING};
use crate::LuminaApp;
use gpui::{Entity, VisualTestContext};
use lumina_core::{DownloadState, LuminaEngine};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const A: &str = "https://example.com/a.mp4";

fn url(n: usize) -> String {
    format!("https://example.com/{n}.mp4")
}

fn count(app: &LuminaApp, state: DownloadState) -> usize {
    states(app).into_iter().filter(|s| *s == state).count()
}

/// Queues `n` downloads: the first three run, the rest wait.
fn queue_n(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, n: usize) {
    for k in 1..=n {
        submit(view, vcx, &url(k));
        settle(vcx, view, "the row is stored", |app| {
            app.downloads.entries.len() == k && app.downloads.entries.iter().all(|e| e.row.id > 0)
        });
    }
    settle(vcx, view, "the downloads run", |app| {
        count(app, DownloadState::Running) == n.min(MAX_RUNNING)
    });
}

fn persisted_state(engine: &LuminaEngine, url: &str) -> DownloadState {
    persisted(engine)
        .into_iter()
        .find(|(u, _)| u == url)
        .map(|(_, state)| state)
        .unwrap_or_else(|| panic!("no stored row for {url}"))
}

fn started(fake: &FakeJobs, url: &str) -> usize {
    fake.requests().iter().filter(|r| r.url == url).count()
}

#[core::prelude::v1::test]
fn pause_download_stops_job_and_keeps_row() {
    let (_dir, engine, fake, mut cx) = queue_app("dl_pause");
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    queue_n(&view, vcx, 4);

    view.update(vcx, |app, cx| {
        let id = row_id(app, &url(1));
        app.pause_download(id, cx);
        assert_eq!(
            app.downloads.entries[0].row.state,
            DownloadState::Paused,
            "Pause shows at once"
        );
    });
    settle(vcx, &view, "the waiting row starts", |app| {
        count(app, DownloadState::Running) == MAX_RUNNING
    });
    assert_eq!(started(&fake, &url(4)), 1, "the freed slot starts the next");
    // Paused is stored only once the cancelled job has ended.
    settle(vcx, &view, "the pause is stored", |_| {
        persisted_state(&engine, &url(1)) == DownloadState::Paused
    });
    view.update(vcx, |app, _| {
        assert_eq!(app.downloads.entries.len(), 4, "a paused row stays listed");
        assert_eq!(
            app.downloads.entries[0].row.state,
            DownloadState::Paused,
            "the cancelled job never removes the row or marks it failed"
        );
        assert!(app.error_message.is_none());
    });

    // Pausing a waiting row just stores it paused.
    let (_dir2, engine2, fake2, mut cx2) = queue_app("dl_pause_waiting");
    let (view2, vcx2) = app_with_library(&mut cx2, engine2.clone());
    install(&view2, vcx2, &fake2);
    queue_n(&view2, vcx2, 4);
    view2.update(vcx2, |app, cx| {
        let id = row_id(app, &url(4));
        app.pause_download(id, cx);
    });
    settle(vcx2, &view2, "the pause is stored", |_| {
        persisted_state(&engine2, &url(4)) == DownloadState::Paused
    });
    // Free a slot: a waiting row would start now; the paused one doesn't.
    fake2.finish(&url(1), imported(&engine2));
    settle(vcx2, &view2, "a slot frees", |app| {
        count(app, DownloadState::Done) == 1
    });
    vcx2.run_until_parked();
    assert_eq!(
        started(&fake2, &url(4)),
        0,
        "a paused waiting row never takes a free slot"
    );
    view2.update(vcx2, |app, _| {
        assert_eq!(app.downloads.entries[3].row.state, DownloadState::Paused)
    });
}

/// Job steps that ignore the cancel flag (cataloguing, the prior-download
/// lookup) can end a paused run after Resume started the next one.
#[core::prelude::v1::test]
fn late_finish_of_paused_run_leaves_resumed_run_alone() {
    let (_dir, engine, fake, mut cx) = queue_app("dl_pause_stale");
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    // Run A of url 1 ignores Pause and ends when the test says; every other
    // run stops on its cancel flag. `live` counts job threads still running.
    let (stale_tx, stale_rx) = channel::<JobOutcome>();
    let stale_rx = Arc::new(Mutex::new(stale_rx));
    let live = Arc::new(AtomicUsize::new(0));
    let first_run = Arc::new(AtomicBool::new(true));
    let stale_ended = Arc::new(AtomicBool::new(false));
    let job: JobFn = {
        let live = live.clone();
        let stale_ended = stale_ended.clone();
        Arc::new(move |request, cancel, _events| {
            live.fetch_add(1, Ordering::SeqCst);
            let outcome = if request.url == url(1) && first_run.swap(false, Ordering::SeqCst) {
                let outcome = stale_rx
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(10))
                    .unwrap_or(JobOutcome::Cancelled);
                stale_ended.store(true, Ordering::SeqCst);
                outcome
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
    view.update(vcx, |app, _| app.downloads.job = Some(job));
    queue_n(&view, vcx, 3);

    let id = view.update(vcx, |app, _| row_id(app, &url(1)));
    view.update(vcx, |app, cx| app.pause_download(id, cx));
    view.update(vcx, |app, cx| app.resume_download(id, cx));
    settle(vcx, &view, "run B starts", |app| {
        app.downloads
            .entries
            .iter()
            .any(|e| e.row.id == id && e.row.state == DownloadState::Running)
    });
    submit(&view, vcx, &url(4));
    settle(vcx, &view, "url 4 waits", |app| {
        app.downloads.entries.len() == 4 && count(app, DownloadState::Waiting) == 1
    });

    // Run A ends with a result after all.
    stale_tx.send(imported(&engine)).unwrap();
    // Its thread stores Done before it reports back.
    settle(vcx, &view, "run A has ended and stored its result", |_| {
        stale_ended.load(Ordering::SeqCst)
            && persisted_state(&engine, &url(1)) == DownloadState::Done
    });
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        let row = app
            .downloads
            .entries
            .iter()
            .find(|e| e.row.id == id)
            .unwrap();
        assert_eq!(
            row.row.state,
            DownloadState::Running,
            "a late result of the paused run never overwrites the resumed run"
        );
        assert_eq!(count(app, DownloadState::Waiting), 1, "url 4 still waits");
    });
    assert_eq!(
        live.load(Ordering::SeqCst),
        MAX_RUNNING,
        "no extra job starts on a slot that isn't free"
    );

    // The stored row converges on run B's end, whatever run A wrote.
    view.update(vcx, |app, cx| app.pause_download(id, cx));
    settle(vcx, &view, "run B's pause is stored", |_| {
        persisted_state(&engine, &url(1)) == DownloadState::Paused
    });
}

#[core::prelude::v1::test]
fn clicking_a_running_row_does_not_pause_it() {
    let (_dir, engine, fake, mut cx) = queue_app("dl_pause_click");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    queue_n(&view, vcx, 1);
    view.update(vcx, |app, cx| app.toggle_downloads_panel(cx));
    render_ui(vcx);
    let id = view.update(vcx, |app, _| row_id(app, &url(1)));
    let selector: &'static str = Box::leak(format!("download-row-{id}").into_boxed_str());
    click(vcx, selector);
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        assert_eq!(
            states(app),
            [DownloadState::Running],
            "a stray click never pauses: Resume would start over"
        );
        assert_eq!(app.downloads.focused, Some(id), "the click focuses the row");
    });
}

#[core::prelude::v1::test]
fn pause_all_then_resume_all_round_trips() {
    let (_dir, engine, fake, mut cx) = queue_app("dl_pause_all");
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    queue_n(&view, vcx, 5);

    view.update(vcx, |app, cx| app.pause_all_downloads(cx));
    view.update(vcx, |app, _| {
        assert_eq!(
            states(app),
            [DownloadState::Paused; 5],
            "every running and waiting row pauses"
        )
    });
    settle(vcx, &view, "every pause is stored", |_| {
        persisted(&engine)
            .iter()
            .all(|(_, s)| *s == DownloadState::Paused)
    });
    vcx.run_until_parked();
    assert_eq!(fake.requests().len(), 3, "nothing new starts while paused");

    view.update(vcx, |app, cx| app.resume_all_downloads(cx));
    settle(vcx, &view, "three run again, two wait", |app| {
        count(app, DownloadState::Running) == MAX_RUNNING && count(app, DownloadState::Waiting) == 2
    });
    let order: Vec<String> = (1..=5).map(url).collect();
    view.update(vcx, |app, _| {
        let listed: Vec<String> = app
            .downloads
            .entries
            .iter()
            .map(|e| e.row.url.clone())
            .collect();
        assert_eq!(listed, order, "Resume all keeps the list order");
        assert_eq!(
            states(app),
            [
                DownloadState::Running,
                DownloadState::Running,
                DownloadState::Running,
                DownloadState::Waiting,
                DownloadState::Waiting
            ],
            "the first three in list order run"
        );
    });
    let stored: Vec<String> = persisted(&engine).into_iter().map(|(u, _)| u).collect();
    assert_eq!(stored, order, "the stored queue keeps the list order");
}

#[core::prelude::v1::test]
fn enter_pauses_running_and_resumes_paused() {
    let (_dir, engine, fake, mut cx) = queue_app("dl_pause_enter");
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    queue_n(&view, vcx, 1);
    focus_app(vcx, &view);
    keystrokes(vcx, "alt-secondary-l");
    let id = view.update(vcx, |app, _| {
        assert!(app.downloads.open, "⌥⌘L opens the list");
        row_id(app, &url(1))
    });
    view.update(vcx, |app, _| assert_eq!(app.downloads.focused, Some(id)));

    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert_eq!(
            states(app),
            [DownloadState::Paused],
            "↵ pauses a running row"
        );
        assert_eq!(app.downloads.focused, Some(id), "focus stays on the row");
    });
    settle(vcx, &view, "the pause is stored", |_| {
        persisted_state(&engine, &url(1)) == DownloadState::Paused
    });

    keystrokes(vcx, "enter");
    settle(vcx, &view, "↵ resumes the paused row", |app| {
        states(app) == [DownloadState::Running]
    });
    assert_eq!(started(&fake, &url(1)), 2, "Resume starts the job over");
}

#[core::prelude::v1::test]
fn pause_as_the_job_finishes_stays_done() {
    let (_dir, engine, fake, mut cx) = queue_app("dl_pause_race");
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    queue_n(&view, vcx, 1);
    // The job finishes as Pause is pressed; whichever lands first, the
    // download did finish, so the row says so.
    fake.finish(&url(1), imported(&engine));
    view.update(vcx, |app, cx| {
        let id = row_id(app, &url(1));
        app.pause_download(id, cx);
    });
    settle(vcx, &view, "the job's result arrives", |app| {
        states(app) == [DownloadState::Done]
    });
    settle(vcx, &view, "the result is stored", |_| {
        persisted_state(&engine, &url(1)) == DownloadState::Done
    });
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        assert_eq!(
            states(app),
            [DownloadState::Done],
            "a finished job stays Done"
        )
    });
    assert_eq!(persisted_state(&engine, &url(1)), DownloadState::Done);
}

#[core::prelude::v1::test]
fn pause_write_failure_reverts_and_shows_banner() {
    let (dir, engine, fake, mut cx) = queue_app("dl_pause_fail");
    rusqlite::Connection::open(dir.join("catalog.db"))
        .expect("catalog must open")
        .execute_batch(
            "CREATE TRIGGER fail_pause BEFORE UPDATE OF state ON download_queue
             WHEN NEW.state = 'paused'
             BEGIN SELECT RAISE(ABORT, 'injected'); END;",
        )
        .expect("trigger must install");
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    queue_n(&view, vcx, 4);

    // A waiting row: the pause is undone.
    view.update(vcx, |app, cx| {
        let id = row_id(app, &url(4));
        app.pause_download(id, cx);
    });
    settle(vcx, &view, "the failed pause is reverted", |app| {
        app.error_message.is_some()
    });
    view.update(vcx, |app, _| {
        assert_eq!(
            app.downloads.entries[3].row.state,
            DownloadState::Waiting,
            "a pause that couldn't be saved is undone"
        );
    });
    assert_eq!(persisted_state(&engine, &url(4)), DownloadState::Waiting);

    // A running row: its job has stopped, so the row stays paused in the
    // list, and the banner says the stored list may be out of date.
    view.update(vcx, |app, cx| {
        app.error_message = None;
        let id = row_id(app, &url(1));
        app.pause_download(id, cx);
    });
    settle(vcx, &view, "the failed write is reported", |app| {
        app.error_message.is_some()
    });
    view.update(vcx, |app, _| {
        assert_eq!(app.downloads.entries[0].row.state, DownloadState::Paused);
    });
    assert_eq!(
        persisted_state(&engine, &url(1)),
        DownloadState::Running,
        "the stored row is untouched; it comes back paused at the next open"
    );
}

/// Whether the palette lists `id` for `query`.
fn offers(
    view: &Entity<LuminaApp>,
    vcx: &mut VisualTestContext,
    id: CommandId,
    query: &str,
) -> bool {
    view.update(vcx, |app, _| {
        app.command_query = query.into();
        let shown = app
            .palette_rows()
            .iter()
            .any(|row| row.target == PaletteTarget::Command(id));
        app.command_query.clear();
        shown
    })
}

fn run_palette(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, query: &str) {
    view.update(vcx, |app, cx| {
        app.open_command_palette(cx);
        app.command_query = query.to_string();
        cx.notify();
    });
    view.update(vcx, |app, cx| app.execute_selected_command(cx));
}

#[core::prelude::v1::test]
fn download_palette_entries() {
    let (_dir, engine, fake, mut cx) = queue_app("dl_pause_palette");
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    install(&view, vcx, &fake);
    let (pause, resume, clear) = (
        CommandId::PauseAllDownloads,
        CommandId::ResumeAllDownloads,
        CommandId::ClearFinishedDownloads,
    );
    assert!(
        !offers(&view, vcx, pause, "pause all downloads"),
        "nothing to pause"
    );
    assert!(
        !offers(&view, vcx, resume, "resume all downloads"),
        "nothing to resume"
    );
    assert!(
        !offers(&view, vcx, clear, "clear finished downloads"),
        "nothing to clear"
    );

    submit(&view, vcx, A);
    settle(vcx, &view, "A runs", |app| {
        states(app) == [DownloadState::Running]
    });
    assert!(offers(&view, vcx, pause, "pause all downloads"));
    assert!(!offers(&view, vcx, resume, "resume all downloads"));

    run_palette(&view, vcx, "pause all downloads");
    view.update(vcx, |app, _| {
        assert_eq!(
            states(app),
            [DownloadState::Paused],
            "Pause All Downloads acts"
        )
    });
    settle(vcx, &view, "the pause is stored", |_| {
        persisted_state(&engine, A) == DownloadState::Paused
    });
    assert!(!offers(&view, vcx, pause, "pause all downloads"));
    assert!(offers(&view, vcx, resume, "resume all downloads"));

    run_palette(&view, vcx, "resume all downloads");
    settle(vcx, &view, "Resume All Downloads acts", |app| {
        states(app) == [DownloadState::Running]
    });
    assert!(!offers(&view, vcx, resume, "resume all downloads"));
    assert!(!offers(&view, vcx, clear, "clear finished downloads"));

    fake.finish(A, imported(&engine));
    settle(vcx, &view, "A is done", |app| {
        states(app) == [DownloadState::Done]
    });
    assert!(offers(&view, vcx, clear, "clear finished downloads"));
    run_palette(&view, vcx, "clear finished downloads");
    view.update(vcx, |app, _| {
        assert!(
            app.downloads.entries.is_empty(),
            "Clear Finished Downloads acts"
        )
    });
    assert!(!offers(&view, vcx, clear, "clear finished downloads"));
}
