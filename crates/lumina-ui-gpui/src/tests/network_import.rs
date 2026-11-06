//! UI automation and state tests for Network Video Import (Direct, YouTube, HLS).

use super::ui_automation::{
    app_with_library, click, focus_app, keystrokes, render_ui, seed_library,
};
use crate::app::downloads::{real_job, JobOutcome, JobRequest};
use crate::app::network_import::NetworkImportStage;
use lumina_core::net_import::{helper::YT_DLP_ENV, NetworkSourceKind};
use std::sync::atomic::AtomicBool;

/// Points the yt-dlp lookup at a path that does not exist, so no test ever
/// runs a real yt-dlp against the network.
fn without_yt_dlp() {
    std::env::set_var(YT_DLP_ENV, "/nonexistent/lumina-test/yt-dlp");
}

#[core::prelude::v1::test]
fn network_import_dialog_open_and_dismiss_via_cancel() {
    let (_library, engine) = seed_library("net_import_open", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    focus_app(vcx, &view);

    // Open network import dialog
    view.update(vcx, |app, cx| app.open_network_import_dialog(cx));
    view.update(vcx, |app, _| {
        assert!(
            matches!(
                app.network_import.stage,
                Some(NetworkImportStage::Input { .. })
            ),
            "dialog should open in Input stage"
        );
    });

    render_ui(vcx);
    assert!(vcx.debug_bounds("network-import-backdrop").is_some());
    assert!(vcx.debug_bounds("network-import-card").is_some());
    assert!(vcx.debug_bounds("net-import-url-input").is_some());

    // Click cancel button
    click(vcx, "net-import-cancel");
    view.update(vcx, |app, _| {
        assert!(
            app.network_import.stage.is_none(),
            "dialog should be closed"
        );
    });
}

#[core::prelude::v1::test]
fn network_import_dismiss_via_escape() {
    let (_library, engine) = seed_library("net_import_esc", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    focus_app(vcx, &view);

    view.update(vcx, |app, cx| app.open_network_import_dialog(cx));
    render_ui(vcx);

    // Press Escape
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(
            app.network_import.stage.is_none(),
            "escape should close the modal"
        );
    });
}

#[core::prelude::v1::test]
fn network_import_input_validation() {
    let (_library, engine) = seed_library("net_import_val", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    focus_app(vcx, &view);

    view.update(vcx, |app, cx| app.open_network_import_dialog(cx));

    // Set URL via helper
    view.update(vcx, |app, cx| {
        app.set_network_import_url("https://example.com/custom.mp4".to_string(), cx);
    });
    view.update(vcx, |app, _| match &app.network_import.stage {
        Some(NetworkImportStage::Input { url, .. }) => {
            assert_eq!(url, "https://example.com/custom.mp4");
        }
        _ => panic!("expected Input stage"),
    });

    // Inspect empty URL -> shows error
    view.update(vcx, |app, cx| app.submit_network_url(String::new(), cx));
    view.update(vcx, |app, _| match &app.network_import.stage {
        Some(NetworkImportStage::Input { error, .. }) => {
            assert!(error.is_some(), "empty URL must surface an error");
        }
        _ => panic!("expected Input stage with error"),
    });

    // Inspect non-http URL -> shows error
    view.update(vcx, |app, cx| {
        app.submit_network_url("ftp://example.com/video.mp4".to_string(), cx)
    });
    view.update(vcx, |app, _| match &app.network_import.stage {
        Some(NetworkImportStage::Input { error, .. }) => {
            assert!(error.is_some(), "non-http URL must surface an error");
        }
        _ => panic!("expected Input stage with error"),
    });
}

#[core::prelude::v1::test]
fn network_import_command_palette_trigger() {
    let (_library, engine) = seed_library("net_import_palette", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    focus_app(vcx, &view);

    // Open command palette
    view.update(vcx, |app, cx| app.open_command_palette(cx));
    view.update(vcx, |app, cx| {
        app.command_query = "video url".to_string();
        cx.notify();
    });

    view.update(vcx, |app, _| {
        assert!(
            app.command_palette_open,
            "command palette should be open and filter results"
        );
    });

    // Execute matching command
    view.update(vcx, |app, cx| app.execute_selected_command(cx));
    view.update(vcx, |app, _| {
        assert!(
            app.network_import.stage.is_some(),
            "executing command should open network import dialog"
        );
    });
}

#[core::prelude::v1::test]
fn network_import_url_typing_and_backspace() {
    let (_library, engine) = seed_library("net_import_typing", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    focus_app(vcx, &view);

    view.update(vcx, |app, cx| app.open_network_import_dialog(cx));
    render_ui(vcx);

    // Initial state: empty URL
    view.update(vcx, |app, _| {
        app.network_import.stage = Some(NetworkImportStage::Input {
            url: String::new(),
            error: None,
        });
    });

    view.update(vcx, |app, _| {
        assert_eq!(
            app.text_target(),
            Some(crate::app::text_input::TextTarget::NetworkImportUrl)
        );
    });

    // Simulate platform input typing
    vcx.simulate_input("https://example.com/stream.mp4");

    view.update(vcx, |app, _| match &app.network_import.stage {
        Some(NetworkImportStage::Input { url, .. }) => {
            assert_eq!(url, "https://example.com/stream.mp4");
        }
        _ => panic!("expected Input stage with typed URL"),
    });

    // Press Backspace
    keystrokes(vcx, "backspace");
    view.update(vcx, |app, _| match &app.network_import.stage {
        Some(NetworkImportStage::Input { url, .. }) => {
            assert_eq!(url, "https://example.com/stream.mp");
        }
        _ => panic!("expected Input stage after backspace"),
    });
}

#[core::prelude::v1::test]
fn network_import_shortcut_alt_cmd_u() {
    let (_library, engine) = seed_library("net_import_shortcut", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    focus_app(vcx, &view);

    // Press ⌥⌘U via keystrokes
    keystrokes(vcx, "alt-secondary-u");

    view.update(vcx, |app, _| {
        assert!(
            app.network_import.stage.is_some(),
            "⌥⌘U should open the network import dialog"
        );
    });
}

#[core::prelude::v1::test]
fn network_import_long_url_and_clear_button() {
    let (_library, engine) = seed_library("net_import_long", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    focus_app(vcx, &view);

    view.update(vcx, |app, cx| app.open_network_import_dialog(cx));

    // Set a long URL that would previously overflow
    let long_url = "https://ugc-cdn-caching-n3xhcgna8lhaqar6c1.cloudwindow-route.com/engine/playback/v1/manifest.m3u8?token=long_token_string_exceeding_container_bounds";
    view.update(vcx, |app, cx| {
        app.set_network_import_url(long_url.to_string(), cx);
    });

    render_ui(vcx);
    assert!(vcx.debug_bounds("net-import-url-input").is_some());
    assert!(vcx.debug_bounds("net-import-clear-btn").is_some());

    // Click clear button
    click(vcx, "net-import-clear-btn");
    view.update(vcx, |app, _| match &app.network_import.stage {
        Some(NetworkImportStage::Input { url, .. }) => {
            assert!(url.is_empty(), "clear button should empty the URL");
        }
        _ => panic!("expected Input stage"),
    });
}

#[core::prelude::v1::test]
fn download_job_reports_a_missing_helper_without_inspecting() {
    without_yt_dlp();
    let (_library, engine) = seed_library("net_import_yt_helper", 1);
    let job = real_job(engine);
    let (events, _rx) = async_channel::unbounded();
    for (url, kind) in [
        (
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            NetworkSourceKind::YouTube,
        ),
        (
            "https://example.com/stream/index.m3u8",
            NetworkSourceKind::HlsStream,
        ),
    ] {
        let request = JobRequest {
            row_id: 1,
            url: url.to_string(),
            kind,
            skip_prior_check: false,
        };
        assert!(
            matches!(
                job(&request, &AtomicBool::new(false), &events),
                JobOutcome::MissingHelper
            ),
            "{url} needs the bundled helper, so say so before inspecting"
        );
    }
}
