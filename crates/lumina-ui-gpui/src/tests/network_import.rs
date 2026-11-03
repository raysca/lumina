//! UI automation and state tests for Network Video Import (Direct, YouTube, HLS).

use super::ui_automation::{
    app_with_library, click, focus_app, keystrokes, render_ui, seed_library,
};
use crate::app::network_import::NetworkImportStage;
use lumina_core::net_import::{NetworkMediaInfo, NetworkSourceKind};
use std::path::PathBuf;

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
    let (_library, engine) = seed_library("net_import_yt_helper", 1);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    focus_app(vcx, &view);

    view.update(vcx, |app, cx| app.open_network_import_dialog(cx));

    // Inspect YouTube URL without yt-dlp installed in test environment
    view.update(vcx, |app, cx| {
        app.inspect_network_url(
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ".to_string(),
            cx,
        );
    });

    view.update(vcx, |app, _| {
        // If yt-dlp is not on test machine, it moves to MissingHelper with instructions
        if lumina_core::net_import::helper::find_yt_dlp(None).is_none() {
            assert!(
                matches!(
                    app.network_import.stage,
                    Some(NetworkImportStage::MissingHelper { .. })
                ),
                "missing yt-dlp must transition to MissingHelper stage"
            );
        }
    });
}

#[core::prelude::v1::test]
fn network_import_ready_options_and_favorites() {
    let (_library, engine) = seed_library("net_import_ready", 1);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    focus_app(vcx, &view);

    let dummy_info = NetworkMediaInfo {
        url: "https://example.com/test.mp4".to_string(),
        kind: NetworkSourceKind::DirectUrl,
        title: "Test Video".to_string(),
        duration_secs: Some(125.0),
        uploader: Some("Tester".to_string()),
        thumbnail_url: None,
        expected_bytes: Some(1024 * 1024 * 10),
    };

    view.update(vcx, |app, _| {
        app.network_import.stage = Some(NetworkImportStage::Ready {
            info: dummy_info,
            target_path: PathBuf::from("/tmp/Test Video.mp4"),
            tags: vec!["nature".to_string()],
            is_favorite: false,
        });
    });

    render_ui(vcx);
    assert!(vcx.debug_bounds("network-import-card").is_some());
    assert!(vcx.debug_bounds("net-import-favorite-toggle").is_some());

    // Toggle favorite
    click(vcx, "net-import-favorite-toggle");
    view.update(vcx, |app, _| match &app.network_import.stage {
        Some(NetworkImportStage::Ready { is_favorite, .. }) => {
            assert!(*is_favorite, "favorite should be toggled to true");
        }
        _ => panic!("expected Ready stage"),
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
