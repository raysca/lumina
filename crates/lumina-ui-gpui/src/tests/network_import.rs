//! UI automation and state tests for Network Video Import (Direct, YouTube, HLS).

use super::ui_automation::{
    app_with_library, click, click_point, focus_app, keystrokes, render_ui, seed_library,
};
use crate::app::downloads::{real_job, JobOutcome, JobRequest};
use crate::app::network_import::NetworkImportStage;
use lumina_core::net_import::{helper::YT_DLP_ENV, NetworkSourceKind};
use std::sync::atomic::AtomicBool;

/// Points the yt-dlp lookup at a path that does not exist, so no test ever
/// runs a real yt-dlp against the network.
pub(crate) fn without_yt_dlp() {
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
fn net_import_dialog_has_header_close_and_footer() {
    let (_library, engine) = seed_library("net_import_shell", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    focus_app(vcx, &view);
    vcx.simulate_resize(gpui::size(gpui::px(1280.0), gpui::px(800.0)));

    view.update(vcx, |app, cx| app.open_network_import_dialog(cx));
    render_ui(vcx);

    let card = vcx.debug_bounds("network-import-card").expect("the card");
    assert_eq!(
        card.size.width,
        gpui::px(644.0),
        "the card is 644 wide in a wide window"
    );
    let field = vcx.debug_bounds("net-import-url-input").expect("URL field");
    let footer = vcx.debug_bounds("net-import-footer").expect("footer bar");
    assert!(
        footer.top() >= field.bottom(),
        "the footer sits below the URL field"
    );
    let cancel = vcx.debug_bounds("net-import-cancel").expect("Cancel");
    assert!(
        footer.contains(&cancel.center()),
        "Cancel sits in the footer bar"
    );
    let close = vcx.debug_bounds("net-import-close").expect("close button");
    assert!(
        close.bottom() <= field.top(),
        "the close button sits in the header, above the field"
    );

    // Below 700 px the card is the window width minus 32.
    vcx.simulate_resize(gpui::size(gpui::px(600.0), gpui::px(800.0)));
    render_ui(vcx);
    let card = vcx.debug_bounds("network-import-card").expect("the card");
    assert_eq!(
        card.size.width,
        gpui::px(568.0),
        "a narrow window leaves 16 px each side of the card"
    );

    click(vcx, "net-import-close");
    view.update(vcx, |app, _| {
        assert!(
            app.network_import.stage.is_none(),
            "the close button closes the dialog"
        );
    });
}

#[core::prelude::v1::test]
fn net_import_card_absorbs_clicks_and_backdrop_closes() {
    let (_library, engine) = seed_library("net_import_overlay", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    focus_app(vcx, &view);
    vcx.simulate_resize(gpui::size(gpui::px(1280.0), gpui::px(800.0)));

    view.update(vcx, |app, cx| app.open_network_import_dialog(cx));
    render_ui(vcx);

    // The footer's left edge holds only the hint, not a control.
    let footer = vcx.debug_bounds("net-import-footer").expect("footer bar");
    let y = f32::from(footer.center().y);
    click_point(vcx, f32::from(footer.left()) + 8.0, y);
    view.update(vcx, |app, _| {
        assert!(
            app.network_import.stage.is_some(),
            "a click on the card must not reach the backdrop"
        );
    });

    let card = vcx.debug_bounds("network-import-card").expect("the card");
    assert!(
        card.left() > gpui::px(8.0),
        "the test clicks left of the card"
    );
    click_point(vcx, 4.0, y);
    view.update(vcx, |app, _| {
        assert!(
            app.network_import.stage.is_none(),
            "a click on the backdrop outside the card closes the dialog"
        );
    });
}

#[core::prelude::v1::test]
fn net_import_url_field_is_46_tall_with_clear_button() {
    let (_library, engine) = seed_library("net_import_field", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    focus_app(vcx, &view);
    vcx.simulate_resize(gpui::size(gpui::px(1280.0), gpui::px(800.0)));

    view.update(vcx, |app, cx| app.open_network_import_dialog(cx));
    render_ui(vcx);
    let field = vcx.debug_bounds("net-import-url-input").expect("URL field");
    assert_eq!(
        field.size.height,
        gpui::px(46.0),
        "the URL field is 46 tall"
    );

    let long_url = format!("https://example.com/watch?v={}", "x".repeat(400));
    view.update(vcx, |app, cx| app.set_network_import_url(long_url, cx));
    render_ui(vcx);
    let field = vcx.debug_bounds("net-import-url-input").expect("URL field");
    assert_eq!(
        field.size.height,
        gpui::px(46.0),
        "a very long URL stays on one line"
    );
    let clear = vcx
        .debug_bounds("net-import-clear-btn")
        .expect("clear button");
    assert_eq!(
        (clear.size.width, clear.size.height),
        (gpui::px(20.0), gpui::px(20.0)),
        "the clear button is a 20×20 circle"
    );
    assert!(
        clear.right() <= field.right(),
        "the clear button stays inside the field, not pushed out by the URL"
    );
    vcx.simulate_mouse_move(clear.center(), None, gpui::Modifiers::default());
    vcx.background_executor
        .advance_clock(std::time::Duration::from_millis(600));
    vcx.run_until_parked();
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("tooltip-Clear URL").is_some(),
        "the icon-only clear button names itself in a tooltip"
    );

    click(vcx, "net-import-clear-btn");
    view.update(vcx, |app, _| match &app.network_import.stage {
        Some(NetworkImportStage::Input { url, .. }) => {
            assert!(url.is_empty(), "the clear button empties the URL");
        }
        _ => panic!("expected Input stage"),
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
            info: None,
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
