//! Import Video from URL: the link is inspected in the dialog and shown in a
//! preview card before it is queued. A fake inspector stands in for the
//! real one, so nothing here touches the network.

use super::downloads::{install, queue_app, seed_prior_download, settle, FakeJobs};
use super::network_import::without_yt_dlp;
use super::ui_automation::{app_with_library, keystrokes, leak, render_ui};
use crate::app::network_import::{InspectFn, NetworkImportStage};
use crate::LuminaApp;
use gpui::{Entity, VisualTestContext};
use lumina_core::net_import::{classify_url, NetworkMediaInfo, NetworkSourceKind, MISSING_PART};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

const URL: &str = "https://example.com/clip.mp4";
const MB: u64 = 1024 * 1024;

/// What the fake inspector returns for every link.
#[derive(Clone)]
enum Answer {
    Info { uploader: Option<&'static str> },
    Fails(&'static str),
}

/// Counts calls; the title carries the call number so a test can tell
/// which inspection a result came from.
#[derive(Clone)]
struct FakeInspector {
    calls: Arc<AtomicUsize>,
    answer: Answer,
}

impl FakeInspector {
    fn new(answer: Answer) -> Self {
        Self {
            calls: Arc::new(AtomicUsize::new(0)),
            answer,
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    fn inspect(&self) -> InspectFn {
        let fake = self.clone();
        Arc::new(move |url, _cancel| {
            let n = fake.calls.fetch_add(1, Ordering::SeqCst) + 1;
            match fake.answer {
                Answer::Info { uploader } => Ok(NetworkMediaInfo {
                    url: url.to_string(),
                    kind: classify_url(url).unwrap_or(NetworkSourceKind::DirectUrl),
                    title: format!("A Walk Through the Hills, take {n}"),
                    duration_secs: Some(252.0),
                    uploader: uploader.map(str::to_string),
                    expected_bytes: Some(384 * MB),
                    extension: "mp4",
                }),
                Answer::Fails(message) => Err(anyhow::anyhow!(message)),
            }
        })
    }
}

fn dialog_app(
    name: &str,
    answer: Answer,
) -> (
    super::downloads::TempLibrary,
    FakeJobs,
    FakeInspector,
    crate::tests::TestApp,
    Arc<lumina_core::LuminaEngine>,
) {
    let (dir, engine, fake, cx) = queue_app(name);
    (dir, fake, FakeInspector::new(answer), cx, engine)
}

fn use_inspector(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, fake: &FakeInspector) {
    let inspect = fake.inspect();
    view.update(vcx, |app, _| app.network_import.inspect = Some(inspect));
}

fn submit(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, url: &str) {
    view.update(vcx, |app, cx| {
        app.open_network_import_dialog(cx);
        app.submit_network_url(url.to_string(), cx);
    });
}

fn painted(vcx: &mut VisualTestContext, selector: String) -> bool {
    vcx.debug_bounds(leak(selector)).is_some()
}

fn input_error(app: &LuminaApp) -> Option<String> {
    match &app.network_import.stage {
        Some(NetworkImportStage::Input { error, .. }) => error.clone(),
        _ => None,
    }
}

#[core::prelude::v1::test]
fn net_import_shows_preview_before_queueing() {
    let (_dir, jobs, inspector, mut cx, engine) = dialog_app(
        "net_preview",
        Answer::Info {
            uploader: Some("Lumina Films"),
        },
    );
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &jobs);
    use_inspector(&view, vcx, &inspector);

    submit(&view, vcx, URL);
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        assert!(
            matches!(
                app.network_import.stage,
                Some(NetworkImportStage::Ready { .. })
            ),
            "the dialog shows the inspected link before queueing it"
        );
        assert!(
            app.downloads.entries.is_empty(),
            "nothing is queued before Import is pressed"
        );
    });
    render_ui(vcx);
    assert!(vcx.debug_bounds("net-import-preview").is_some());
    let title = "A Walk Through the Hills, take 1";
    for (selector, text) in [
        ("net-import-preview-title", title),
        ("net-import-preview-duration", "04:12"),
        ("net-import-preview-meta", "example.com · Lumina Films"),
        ("net-import-preview-format", "MP4"),
        ("net-import-preview-size", "384 MB"),
        ("net-import-hint", "Joins your downloads · 384 MB"),
    ] {
        assert!(
            painted(vcx, format!("{selector}-text:{text}")),
            "{selector} shows {text:?}"
        );
    }

    keystrokes(vcx, "enter");
    settle(vcx, &view, "the inspected link is queued", |_| {
        jobs.requests().len() == 1
    });
    view.update(vcx, |app, _| {
        assert!(
            app.network_import.stage.is_none(),
            "Import closes the dialog"
        );
        assert_eq!(app.downloads.entries.len(), 1, "one row is queued");
    });
    let request = &jobs.requests()[0];
    assert_eq!(
        request.info.as_ref().map(|info| info.title.as_str()),
        Some(title),
        "the job gets the dialog's inspection, so it need not inspect again"
    );
    assert_eq!(inspector.calls(), 1, "the link is inspected once in total");
}

#[core::prelude::v1::test]
fn net_import_preview_without_uploader_has_no_dangling_dot() {
    let (_dir, jobs, inspector, mut cx, engine) =
        dialog_app("net_preview_no_uploader", Answer::Info { uploader: None });
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &jobs);
    use_inspector(&view, vcx, &inspector);

    submit(&view, vcx, URL);
    vcx.run_until_parked();
    render_ui(vcx);
    assert!(
        painted(vcx, "net-import-preview-meta-text:example.com".to_string()),
        "with no uploader the meta line is only the host, with no \"·\""
    );
}

#[core::prelude::v1::test]
fn net_import_cancel_stops_inspection() {
    let (_dir, jobs, inspector, mut cx, engine) =
        dialog_app("net_preview_cancel", Answer::Info { uploader: None });
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &jobs);
    use_inspector(&view, vcx, &inspector);

    let cancel = view.update(vcx, |app, cx| {
        app.open_network_import_dialog(cx);
        app.submit_network_url(URL.to_string(), cx);
        let Some(NetworkImportStage::Checking { cancel, .. }) = &app.network_import.stage else {
            panic!("expected the Checking stage");
        };
        let cancel = cancel.clone();
        // ↵ while checking does nothing: no second inspection.
        app.submit_network_url(URL.to_string(), cx);
        app.close_network_import_dialog(cx);
        cancel
    });
    assert!(
        cancel.load(Ordering::SeqCst),
        "closing the dialog stops the inspection"
    );
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        assert!(
            app.network_import.stage.is_none(),
            "a result arriving after the close doesn't reopen the dialog"
        );
        assert!(app.downloads.entries.is_empty(), "nothing is queued");
    });
    assert!(jobs.requests().is_empty(), "no job runs");
    assert!(
        inspector.calls() <= 1,
        "↵ while checking adds no inspection"
    );
}

#[core::prelude::v1::test]
fn net_import_ignores_a_late_result_after_cancel() {
    let (_dir, jobs, inspector, mut cx, engine) =
        dialog_app("net_preview_late", Answer::Info { uploader: None });
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &jobs);
    use_inspector(&view, vcx, &inspector);

    // The first check is cancelled and the same link submitted again before
    // either inspection reports back. The fake ignores the cancel flag, so
    // the first result still arrives, late.
    view.update(vcx, |app, cx| {
        app.open_network_import_dialog(cx);
        app.submit_network_url(URL.to_string(), cx);
        app.close_network_import_dialog(cx);
        app.open_network_import_dialog(cx);
        app.submit_network_url(URL.to_string(), cx);
    });
    vcx.run_until_parked();
    assert_eq!(inspector.calls(), 2);
    view.update(vcx, |app, _| match &app.network_import.stage {
        Some(NetworkImportStage::Ready { info, .. }) => assert_eq!(
            info.title, "A Walk Through the Hills, take 2",
            "the cancelled check's result must not replace the current one"
        ),
        other => panic!("expected the Ready stage, got {other:?}"),
    });
    assert!(jobs.requests().is_empty(), "nothing is queued");
}

#[core::prelude::v1::test]
fn net_import_inspection_failure_returns_to_input() {
    let (_dir, jobs, inspector, mut cx, engine) = dialog_app(
        "net_preview_fails",
        Answer::Fails("This page has no video to download."),
    );
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &jobs);
    use_inspector(&view, vcx, &inspector);

    submit(&view, vcx, URL);
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        assert_eq!(
            input_error(app).as_deref(),
            Some("This page has no video to download."),
            "a failed inspection shows its message under the field"
        );
        assert!(app.downloads.entries.is_empty(), "nothing is queued");
    });
    assert!(jobs.requests().is_empty());
}

#[core::prelude::v1::test]
fn net_import_dialog_reports_missing_helper_without_inspecting() {
    without_yt_dlp();
    let (_dir, jobs, inspector, mut cx, engine) =
        dialog_app("net_preview_helper", Answer::Info { uploader: None });
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &jobs);
    use_inspector(&view, vcx, &inspector);

    submit(&view, vcx, "https://www.youtube.com/watch?v=dQw4w9WgXcQ");
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        assert_eq!(
            input_error(app).as_deref(),
            Some(MISSING_PART),
            "a missing helper is reported in the dialog"
        );
        assert!(app.downloads.entries.is_empty(), "nothing is queued");
    });
    assert_eq!(inspector.calls(), 0, "the link is not inspected");
}

#[core::prelude::v1::test]
fn net_import_link_already_in_list_is_not_inspected() {
    let (_dir, jobs, inspector, mut cx, engine) =
        dialog_app("net_preview_in_list", Answer::Info { uploader: None });
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &jobs);
    use_inspector(&view, vcx, &inspector);
    view.update(vcx, |app, cx| {
        app.enqueue_download(URL.to_string(), false, None, cx)
    });
    vcx.run_until_parked();

    submit(&view, vcx, URL);
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        assert!(app.network_import.stage.is_none(), "the dialog closes");
        assert!(app.downloads.open, "the downloads list opens");
        assert_eq!(
            app.downloads.highlight,
            app.downloads.entries.first().map(|entry| entry.row.id),
            "the row already in the list is highlighted"
        );
        assert_eq!(app.downloads.entries.len(), 1, "no second row");
    });
    assert_eq!(inspector.calls(), 0, "the link is not inspected");
}

#[core::prelude::v1::test]
fn net_import_offers_an_earlier_download_even_without_the_helper() {
    without_yt_dlp();
    const YOUTUBE: &str = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";
    let (_dir, jobs, inspector, mut cx, engine) = dialog_app(
        "net_preview_prior_no_helper",
        Answer::Info { uploader: None },
    );
    let id = seed_prior_download(&engine, YOUTUBE);
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &jobs);
    use_inspector(&view, vcx, &inspector);

    submit(&view, vcx, YOUTUBE);
    vcx.run_until_parked();
    view.update(vcx, |app, _| match &app.network_import.stage {
        Some(NetworkImportStage::AlreadyDownloaded { prior, .. }) => assert_eq!(
            prior.media_id, id,
            "Show in Library needs no helper, so the earlier download is offered"
        ),
        other => panic!("expected Already Downloaded, got {other:?}"),
    });
    assert_eq!(inspector.calls(), 0, "the link is not inspected");
}
