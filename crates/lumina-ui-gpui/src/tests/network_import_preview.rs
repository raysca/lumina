//! Import Video from URL: the link is inspected in the dialog and shown in a
//! preview card before it is queued. A fake inspector stands in for the
//! real one, so nothing here touches the network.

use super::downloads::{install, queue_app, seed_prior_download, settle, FakeJobs};
use super::helpers::{app_with_library, focus_app, keystrokes, leak, render_ui};
use super::network_import::without_yt_dlp;
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
    super::helpers::TempLibrary,
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

#[core::prelude::v1::test]
fn checking_stage_keeps_the_input_layout() {
    let (_dir, jobs, _inspector, mut cx, engine) =
        dialog_app("net_preview_checking", Answer::Info { uploader: None });
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &jobs);
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stage_cancel = cancel.clone();
    view.update(vcx, |app, cx| {
        app.open_network_import_dialog(cx);
        app.network_import.stage = Some(NetworkImportStage::Checking {
            url: URL.to_string(),
            cancel: stage_cancel,
        });
        cx.notify();
    });
    render_ui(vcx);
    let field = vcx
        .debug_bounds("net-import-url-readonly")
        .expect("the link stays on screen, read-only");
    let line = vcx
        .debug_bounds("net-import-checking")
        .expect("a checking line where the preview will go");
    assert!(
        line.top() >= field.bottom(),
        "the checking line sits below the field"
    );
    assert!(
        painted(
            vcx,
            "net-import-checking-text:Checking example.com…".to_string()
        ),
        "the line names the site being checked"
    );
    assert!(
        vcx.debug_bounds("net-import-url-input").is_none(),
        "the field can't be edited while checking"
    );

    focus_app(vcx, &view);
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert!(
            matches!(
                app.network_import.stage,
                Some(NetworkImportStage::Checking { .. })
            ),
            "↵ while checking does nothing"
        );
        assert!(
            app.downloads.entries.is_empty(),
            "↵ while checking queues nothing"
        );
    });
    assert!(jobs.requests().is_empty(), "no job runs");

    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(app.network_import.stage.is_none(), "Esc closes the dialog");
    });
    assert!(
        cancel.load(Ordering::SeqCst),
        "Esc while checking stops the inspection"
    );
}

#[core::prelude::v1::test]
fn already_downloaded_uses_dialog_shell() {
    let (_dir, jobs, inspector, mut cx, engine) =
        dialog_app("net_preview_already", Answer::Info { uploader: None });
    seed_prior_download(&engine, URL);
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &jobs);
    use_inspector(&view, vcx, &inspector);
    let already = |view: &Entity<LuminaApp>, vcx: &mut VisualTestContext| {
        submit(view, vcx, URL);
        vcx.run_until_parked();
        view.update(vcx, |app, _| {
            assert!(
                matches!(
                    app.network_import.stage,
                    Some(NetworkImportStage::AlreadyDownloaded { .. })
                ),
                "a link downloaded before asks first"
            )
        });
        render_ui(vcx);
    };

    already(&view, vcx);
    let close = vcx.debug_bounds("net-import-close").expect("close button");
    let body = vcx
        .debug_bounds("net-import-already-downloaded")
        .expect("the date line");
    assert!(close.bottom() <= body.top(), "close sits in the header");
    let footer = vcx.debug_bounds("net-import-footer").expect("footer bar");
    for selector in [
        "net-import-cancel",
        "net-import-download-again",
        "net-import-show-in-library",
    ] {
        let button = vcx.debug_bounds(selector).expect(selector);
        assert!(
            footer.contains(&button.center()),
            "{selector} sits in the footer bar"
        );
    }

    // ↵ shows the earlier download and queues nothing.
    focus_app(vcx, &view);
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert!(app.network_import.stage.is_none(), "↵ closes the dialog");
        assert!(app.downloads.entries.is_empty(), "↵ queues nothing");
    });

    // ⌘↵ downloads it again, skipping the same check.
    already(&view, vcx);
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-enter");
    settle(vcx, &view, "⌘↵ queues the link again", |_| {
        jobs.requests().len() == 1
    });
    assert!(jobs.requests()[0].skip_prior_check, "⌘↵ is Download Again");
    assert_eq!(inspector.calls(), 0, "nothing was inspected");
}

#[core::prelude::v1::test]
fn net_import_check_is_dropped_when_another_library_opens() {
    let (_dir, jobs, inspector, mut cx, engine) =
        dialog_app("net_preview_switch", Answer::Info { uploader: None });
    let (other, _) = super::helpers::seed_library("net_preview_switch_other", 1);
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &jobs);
    use_inspector(&view, vcx, &inspector);

    // The library changes while the link is still being checked.
    view.update(vcx, |app, cx| {
        app.open_network_import_dialog(cx);
        app.submit_network_url(URL.to_string(), cx);
        assert!(
            matches!(
                app.network_import.stage,
                Some(NetworkImportStage::Checking { .. })
            ),
            "control: the check is in flight"
        );
        app.open_library(other.to_path_buf(), cx);
    });
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        assert!(
            app.network_import.stage.is_none(),
            "opening another library closes the dialog, so a late result never shows"
        );
    });
}
