//! UI automation and state tests for Network Video Import (Direct, YouTube, HLS).

use super::ui_automation::{
    app_with_library, click, click_point, focus_app, keystrokes, render_ui, seed_library,
};
use crate::app::network_import::NetworkImportStage;
use lumina_core::net_import::{
    helper::YT_DLP_ENV, DownloadPhase, NetworkDownloadProgress, NetworkMediaInfo, NetworkSourceKind,
};
use std::path::PathBuf;
use std::sync::atomic::Ordering;

/// Points the yt-dlp lookup at a path that does not exist, so no test ever
/// runs a real yt-dlp against the network.
fn without_yt_dlp() {
    std::env::set_var(YT_DLP_ENV, "/nonexistent/lumina-test/yt-dlp");
}

fn sample_info() -> NetworkMediaInfo {
    NetworkMediaInfo {
        url: "https://example.com/test.mp4".to_string(),
        kind: NetworkSourceKind::DirectUrl,
        title: "Test Video".to_string(),
        duration_secs: Some(125.0),
        uploader: Some("Tester".to_string()),
        expected_bytes: Some(1024 * 1024 * 10),
        extension: "mp4",
    }
}

#[core::prelude::v1::test]
fn network_import_dialog_open_and_dismiss_via_cancel() {
    let (_library, engine) = seed_library("net_import_open", 1);
    let mut cx = gpui::TestAppContext::single();
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
    let mut cx = gpui::TestAppContext::single();
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
    let mut cx = gpui::TestAppContext::single();
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
    view.update(vcx, |app, cx| app.inspect_network_url(String::new(), cx));
    view.update(vcx, |app, _| match &app.network_import.stage {
        Some(NetworkImportStage::Input { error, .. }) => {
            assert!(error.is_some(), "empty URL must surface an error");
        }
        _ => panic!("expected Input stage with error"),
    });

    // Inspect non-http URL -> shows error
    view.update(vcx, |app, cx| {
        app.inspect_network_url("ftp://example.com/video.mp4".to_string(), cx)
    });
    view.update(vcx, |app, _| match &app.network_import.stage {
        Some(NetworkImportStage::Input { error, .. }) => {
            assert!(error.is_some(), "non-http URL must surface an error");
        }
        _ => panic!("expected Input stage with error"),
    });
}

#[core::prelude::v1::test]
fn network_import_missing_helper_for_youtube() {
    without_yt_dlp();
    let (_library, engine) = seed_library("net_import_yt_helper", 1);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    focus_app(vcx, &view);

    view.update(vcx, |app, cx| app.open_network_import_dialog(cx));
    view.update(vcx, |app, cx| {
        app.inspect_network_url(
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ".to_string(),
            cx,
        );
    });
    // The tool lookup runs off the UI thread.
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        assert!(
            matches!(
                app.network_import.stage,
                Some(NetworkImportStage::MissingHelper { .. })
            ),
            "missing yt-dlp must transition to MissingHelper stage"
        );
    });
    render_ui(vcx);
    assert!(vcx.debug_bounds("net-import-retry").is_some());

    // Enter checks again; still missing, so the stage stays.
    keystrokes(vcx, "enter");
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        assert!(matches!(
            app.network_import.stage,
            Some(NetworkImportStage::MissingHelper { .. })
        ));
    });
}

#[core::prelude::v1::test]
fn network_import_hls_needs_helper_before_inspecting() {
    without_yt_dlp();
    let (_library, engine) = seed_library("net_import_hls_helper", 1);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| app.open_network_import_dialog(cx));
    view.update(vcx, |app, cx| {
        app.inspect_network_url("https://example.com/stream/index.m3u8".to_string(), cx);
    });
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        assert!(
            matches!(
                app.network_import.stage,
                Some(NetworkImportStage::MissingHelper { .. })
            ),
            "HLS downloads need yt-dlp, so say so before inspecting"
        );
    });
}

#[core::prelude::v1::test]
fn network_import_ready_options_and_favorites() {
    let (_library, engine) = seed_library("net_import_ready", 1);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    focus_app(vcx, &view);

    view.update(vcx, |app, _| {
        app.network_import.stage = Some(NetworkImportStage::Ready {
            info: sample_info(),
            target_path: PathBuf::from("/tmp/Test Video.mp4"),
            is_favorite: false,
            already_imported: Some(PathBuf::from("/lib/Imports/Test Video.mp4")),
        });
    });

    render_ui(vcx);
    assert!(vcx.debug_bounds("network-import-card").is_some());
    assert!(
        vcx.debug_bounds("net-import-already-imported").is_some(),
        "a URL imported before shows a duplicate notice"
    );

    let favorite = |app: &crate::LuminaApp| match &app.network_import.stage {
        Some(NetworkImportStage::Ready { is_favorite, .. }) => *is_favorite,
        _ => panic!("expected Ready stage"),
    };
    click(vcx, "net-import-favorite-toggle");
    view.update(vcx, |app, _| {
        assert!(favorite(app), "a click toggles favorite")
    });
    keystrokes(vcx, "f");
    view.update(vcx, |app, _| {
        assert!(!favorite(app), "F toggles favorite from the keyboard")
    });
}

fn start_fake_download(view: &gpui::Entity<crate::LuminaApp>, vcx: &mut gpui::VisualTestContext) {
    view.update(vcx, |app, _| {
        app.network_import.stage = Some(NetworkImportStage::Downloading {
            title: "Long Video".to_string(),
            progress: NetworkDownloadProgress {
                phase: DownloadPhase::Downloading,
                bytes_downloaded: 5 * 1024 * 1024,
                total_bytes: Some(10 * 1024 * 1024),
                speed_bytes_per_sec: 1024 * 1024,
                message: "Downloading video".to_string(),
            },
            confirm_cancel: false,
        });
    });
    render_ui(vcx);
}

#[core::prelude::v1::test]
fn network_import_download_survives_stray_click_and_single_escape() {
    let (_library, engine) = seed_library("net_import_cancel", 1);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    focus_app(vcx, &view);
    start_fake_download(&view, vcx);
    let flag = view.update(vcx, |app, _| app.network_import.cancel_flag.clone());

    click_point(vcx, 4.0, 4.0);
    view.update(vcx, |app, _| {
        assert!(
            app.network_import.is_busy(),
            "a backdrop click must not stop a running download"
        );
    });

    keystrokes(vcx, "escape");
    render_ui(vcx);
    assert!(vcx.debug_bounds("net-import-confirm-cancel").is_some());
    view.update(vcx, |app, _| {
        assert!(app.network_import.is_busy(), "one Esc only asks to confirm");
    });
    assert!(!flag.load(Ordering::Relaxed));

    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(app.network_import.stage.is_none(), "a second Esc stops it");
    });
    assert!(
        flag.load(Ordering::Relaxed),
        "stopping sets the cancel flag"
    );
}

#[core::prelude::v1::test]
fn network_import_cancel_button_stops_download_at_once() {
    let (_library, engine) = seed_library("net_import_cancel_btn", 1);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    start_fake_download(&view, vcx);
    click(vcx, "net-import-cancel");
    view.update(vcx, |app, _| {
        assert!(
            app.network_import.stage.is_none(),
            "Cancel Download stops it"
        );
    });
}

#[core::prelude::v1::test]
fn network_import_command_palette_trigger() {
    let (_library, engine) = seed_library("net_import_palette", 1);
    let mut cx = gpui::TestAppContext::single();
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
    let mut cx = gpui::TestAppContext::single();
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
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    focus_app(vcx, &view);

    // Press ⌥⌘U via keystrokes
    keystrokes(vcx, "alt-cmd-u");

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
    let mut cx = gpui::TestAppContext::single();
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
