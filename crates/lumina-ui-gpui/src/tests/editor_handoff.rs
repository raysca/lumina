//! External-editor handoff (T13): config validation, command construction,
//! and the unset/invalid error paths.
//!
//! T13's rotate/crop slice is deliberately absent; that work is tracked as T33.

use std::path::Path;

use gpui::*;
use lumina_core::{EngineEvent, LuminaEngine};

use crate::app::viewer_actions::{
    external_editor_command, EXTERNAL_EDITOR_FILE_MISSING_MESSAGE, EXTERNAL_EDITOR_INVALID_MESSAGE,
    EXTERNAL_EDITOR_UNSET_MESSAGE,
};
use crate::config::{validate_external_editor, AppConfig};
use crate::LuminaApp;

fn temp_config_path(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "lumina_editor_handoff_test_{}_{}",
        name,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("config.json")
}

fn seed_library(prefix: &str, count: usize) -> (std::path::PathBuf, std::sync::Arc<LuminaEngine>) {
    let temp_dir = std::env::temp_dir().join(format!(
        "lumina_editor_handoff_{}_{}",
        prefix,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    for i in 0..count {
        std::fs::write(
            temp_dir.join(format!("photo_{:02}.jpg", i)),
            crate::tests::ui_automation::tiny_jpeg(),
        )
        .unwrap();
    }
    let (engine, rx) = LuminaEngine::open_with_paths(
        temp_dir.clone(),
        temp_dir.join("catalog.db"),
        Some(temp_dir.join("cache")),
    )
    .unwrap();
    while let Ok(event) = rx.recv_timeout(std::time::Duration::from_secs(10)) {
        if let EngineEvent::ScanComplete { .. } = event {
            break;
        }
    }
    (temp_dir, engine)
}

fn leak(selector: String) -> &'static str {
    Box::leak(selector.into_boxed_str())
}

/// Draws the registered root view so layout, hit-testing and debug_bounds are
/// populated for the current app state.
fn render_ui(cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
    cx.run_until_parked();
}

fn focus_app(cx: &mut VisualTestContext, view: &Entity<LuminaApp>) {
    cx.update(|window, cx| {
        let handle = view.read(cx).focus_handle.clone();
        window.focus(&handle, cx);
    });
}

fn bounds_center(cx: &mut VisualTestContext, selector: &'static str) -> gpui::Point<gpui::Pixels> {
    cx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("no rendered bounds for selector {selector:?}"))
        .center()
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let position = bounds_center(cx, selector);
    cx.simulate_click(position, gpui::Modifiers::default());
    render_ui(cx);
}

fn double_click(cx: &mut VisualTestContext, selector: &'static str) {
    let position = bounds_center(cx, selector);
    cx.simulate_event(gpui::MouseDownEvent {
        position,
        modifiers: gpui::Modifiers::default(),
        button: gpui::MouseButton::Left,
        click_count: 2,
        first_mouse: false,
    });
    cx.simulate_event(gpui::MouseUpEvent {
        position,
        modifiers: gpui::Modifiers::default(),
        button: gpui::MouseButton::Left,
        click_count: 2,
    });
    render_ui(cx);
}

fn right_click(cx: &mut VisualTestContext, selector: &'static str) {
    let position = bounds_center(cx, selector);
    cx.simulate_mouse_down(
        position,
        gpui::MouseButton::Right,
        gpui::Modifiers::default(),
    );
    render_ui(cx);
}

fn app_with_library(
    cx: &mut TestAppContext,
    engine: std::sync::Arc<LuminaEngine>,
) -> (Entity<LuminaApp>, &mut VisualTestContext) {
    let (view, vcx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    view.update(vcx, |app, cx| {
        app.grid_state = Some(crate::views::grid::GridState::new(engine));
        app.refresh_album_counts();
        cx.notify();
    });
    render_ui(vcx);
    focus_app(vcx, &view);
    (view, vcx)
}

fn media_card(engine: &LuminaEngine) -> &'static str {
    let first_id = engine.get_item_slice(0..1)[0].id.0;
    leak(format!("media-card-{}", first_id))
}

#[core::prelude::v1::test]
fn editor_handoff_config_defaults_and_validation() {
    let config = AppConfig::default();
    assert!(
        config.external_editor.is_none(),
        "a new config must start without an external editor"
    );
    assert_eq!(
        config.validated_external_editor(),
        None,
        "an unset editor must not validate"
    );

    for invalid in [
        "",
        "   ",
        "-a",
        "--args",
        "com.apple.\nTextEdit",
        "TextEdit; rm -rf /",
        "Bad App",
        "/",
        ".",
    ] {
        assert_eq!(
            validate_external_editor(Some(invalid)),
            None,
            "editor value {invalid:?} must be rejected before launch"
        );
    }

    assert_eq!(
        validate_external_editor(Some("com.apple.TextEdit")),
        Some("com.apple.TextEdit"),
        "a bundle identifier must validate"
    );
    assert_eq!(
        validate_external_editor(Some("Preview")),
        Some("Preview"),
        "a plain application name is a valid bundle identifier"
    );
    assert_eq!(
        validate_external_editor(Some("  /Applications/TextEdit.app  ")),
        Some("/Applications/TextEdit.app"),
        "an absolute application path must validate after trimming"
    );
    assert_eq!(
        validate_external_editor(Some("/Applications/My Editor.app")),
        Some("/Applications/My Editor.app"),
        "spaces inside an absolute path are allowed because arguments are not shell-parsed"
    );

    let mut configured = AppConfig {
        external_editor: Some("  com.apple.TextEdit  ".to_string()),
        ..AppConfig::default()
    };
    assert_eq!(
        configured.validated_external_editor(),
        Some("com.apple.TextEdit"),
        "the config accessor must trim and validate its value"
    );
    configured.external_editor = Some("   ".to_string());
    assert_eq!(
        configured.validated_external_editor(),
        None,
        "a whitespace-only saved value must not validate"
    );
}

#[core::prelude::v1::test]
fn editor_handoff_config_roundtrip_and_missing_key() {
    let path = temp_config_path("roundtrip");
    let config = AppConfig {
        external_editor: Some("com.apple.TextEdit".to_string()),
        ..AppConfig::default()
    };
    config.save_to(&path).unwrap();

    let loaded = AppConfig::load_from(&path);
    assert_eq!(
        loaded.external_editor.as_deref(),
        Some("com.apple.TextEdit"),
        "the external editor must round-trip through the config file"
    );
    assert_eq!(
        loaded.validated_external_editor(),
        Some("com.apple.TextEdit"),
        "a round-tripped editor must still validate"
    );

    // Older config files predate the field and must still load as unset.
    std::fs::write(&path, br#"{"last_view":"all"}"#).unwrap();
    assert!(
        AppConfig::load_from(&path).external_editor.is_none(),
        "a missing external_editor key must default to unset"
    );

    let _ = std::fs::remove_file(&path);
}

#[core::prelude::v1::test]
fn editor_handoff_command_construction_never_uses_a_shell() {
    let file = Path::new("/Users/example/Pictures/photo 1.jpg");
    let command = external_editor_command("com.apple.TextEdit", file)
        .expect("a valid editor and file must build a command");

    #[cfg(target_os = "macos")]
    {
        use std::ffi::OsStr;
        assert_eq!(
            command.get_program(),
            OsStr::new("open"),
            "macOS must launch through open"
        );
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            vec![
                OsStr::new("-b"),
                OsStr::new("com.apple.TextEdit"),
                file.as_os_str()
            ],
            "a bundle id goes to open -b (open -a takes a name or path), as separate \
             arguments, never a shell string"
        );
    }
    #[cfg(not(target_os = "macos"))]
    {
        use std::ffi::OsStr;
        assert_eq!(command.get_program(), OsStr::new("com.apple.TextEdit"));
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            vec![file.as_os_str()],
            "the file must be a separate argument, never a shell string"
        );
    }

    // Values with spaces stay single arguments; no interpolation happens.
    let spaced = external_editor_command("/Applications/My Editor.app", file)
        .expect("an absolute path with spaces must build a command");
    // macOS passes the app to `open -a`; elsewhere it is the program itself.
    let spaced_app = Path::new("/Applications/My Editor.app").as_os_str();
    assert!(
        spaced.get_program() == spaced_app || spaced.get_args().any(|arg| arg == spaced_app),
        "an application path with spaces must remain one argument"
    );
    assert!(
        spaced.get_args().any(|arg| arg == file.as_os_str()),
        "a file path with spaces must remain one argument"
    );
    #[cfg(target_os = "macos")]
    {
        let flag = |editor: &str| {
            external_editor_command(editor, file)
                .and_then(|c| c.get_args().next().map(|a| a.to_owned()))
        };
        assert_eq!(
            flag("/Applications/My Editor.app").as_deref(),
            Some(std::ffi::OsStr::new("-a")),
            "an application path goes to open -a"
        );
        assert_eq!(
            flag("Preview").as_deref(),
            Some(std::ffi::OsStr::new("-a")),
            "a bare app name goes to open -a, as it always did"
        );
        assert_eq!(
            flag("TextEdit.app").as_deref(),
            Some(std::ffi::OsStr::new("-a")),
            "an app bundle name goes to open -a"
        );
        assert_eq!(
            flag("TextEdit.APP").as_deref(),
            Some(std::ffi::OsStr::new("-a")),
            "an app bundle name goes to open -a whatever the extension's case"
        );
        assert_eq!(
            flag("com.apple.TextEdit").as_deref(),
            Some(std::ffi::OsStr::new("-b")),
            "a bundle id goes to open -b"
        );
    }

    assert!(
        external_editor_command("", file).is_none(),
        "an empty editor must never build a command"
    );
    assert!(
        external_editor_command("   ", file).is_none(),
        "a whitespace-only editor must never build a command"
    );
    assert!(
        external_editor_command("-a", file).is_none(),
        "an option-like editor must never build a command"
    );
    assert!(
        external_editor_command("com.apple.TextEdit", Path::new("")).is_none(),
        "an empty file path must never build a command"
    );
}

#[core::prelude::v1::test]
fn editor_handoff_context_menu_without_editor_shows_error() {
    let (temp_dir, engine) = seed_library("menu", 3);
    let card = media_card(&engine);

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    right_click(vcx, card);
    assert!(
        vcx.debug_bounds("context-menu").is_some(),
        "right click must open the media context menu"
    );
    assert!(
        vcx.debug_bounds("edit-external-menu-btn").is_some(),
        "the context menu must expose Edit in External Editor"
    );

    click(vcx, "edit-external-menu-btn");
    view.update(vcx, |app, _| {
        assert!(
            app.context_menu_state.is_none(),
            "choosing the action must close the context menu"
        );
        assert_eq!(
            app.error_message.as_deref(),
            Some(EXTERNAL_EDITOR_UNSET_MESSAGE),
            "an unset editor must surface the Preferences guidance in the error banner"
        );
        let message = app.error_message.as_deref().unwrap_or_default();
        assert!(
            message.contains("Choose an external editor in Preferences"),
            "the unset error must tell the user where to configure the editor; got {message:?}"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn editor_handoff_viewer_button_reports_unset_and_invalid_editor() {
    let (temp_dir, engine) = seed_library("viewer", 2);
    let card = media_card(&engine);

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    click(vcx, card);
    double_click(vcx, card);
    view.update(vcx, |app, _| {
        assert!(
            app.is_fullscreen_viewer,
            "double click must open the viewer"
        );
    });
    right_click(vcx, "viewer-viewport");
    click(vcx, "edit-external-menu-btn");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.error_message.as_deref(),
            Some(EXTERNAL_EDITOR_UNSET_MESSAGE),
            "the viewer action must share the unset-editor error path"
        );
    });

    // A malformed saved value must be rejected at use time, not launched.
    view.update(vcx, |app, _| {
        app.config.external_editor = Some("   ".to_string());
    });
    right_click(vcx, "viewer-viewport");
    click(vcx, "edit-external-menu-btn");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.error_message.as_deref(),
            Some(EXTERNAL_EDITOR_INVALID_MESSAGE),
            "an invalid saved editor must not launch a process"
        );
    });

    // A valid editor with a missing file must also stop before launching.
    let missing = temp_dir.join("missing.jpg");
    view.update(vcx, |app, cx| {
        app.config.external_editor = Some("com.apple.TextEdit".to_string());
        app.open_item_in_external_editor(&missing, cx);
    });
    view.update(vcx, |app, _| {
        assert_eq!(
            app.error_message.as_deref(),
            Some(EXTERNAL_EDITOR_FILE_MISSING_MESSAGE),
            "a missing file must surface an error instead of launching the editor"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn editor_handoff_preferences_shows_current_editor_and_choice() {
    let (temp_dir, engine) = seed_library("prefs", 1);

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, cx| app.open_preferences(cx));
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.preferences_tab,
            crate::app::preferences::PreferencesTab::General
        );
        assert_eq!(
            app.external_editor_label(),
            "Not set",
            "an unset editor must read Not set in Preferences"
        );
    });
    assert!(
        vcx.debug_bounds("preferences-external-editor").is_some(),
        "Preferences General must show the current external editor"
    );
    assert!(
        vcx.debug_bounds("preferences-choose-editor").is_some(),
        "Preferences General must expose Choose Editor"
    );

    // The test build compiles the native dialog out; the click must be inert
    // and must never write the real user config.
    click(vcx, "preferences-choose-editor");
    view.update(vcx, |app, _| {
        assert!(
            app.preferences_open,
            "choosing an editor must not close Preferences"
        );
        assert_eq!(app.external_editor_label(), "Not set");
    });

    view.update(vcx, |app, cx| {
        app.set_external_editor(Some("/Applications/TextEdit.app".to_string()), cx);
    });
    view.update(vcx, |app, _| {
        assert_eq!(
            app.config.external_editor.as_deref(),
            Some("/Applications/TextEdit.app"),
            "the chosen editor must be stored in the config"
        );
        assert_eq!(
            app.external_editor_label(),
            "/Applications/TextEdit.app",
            "Preferences must show the chosen editor"
        );
    });
    assert!(
        vcx.debug_bounds("preferences-external-editor").is_some(),
        "the editor row must still render after choosing an app"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

/// A launch that fails after spawning (no app for the file, a deleted
/// editor) or cannot spawn at all reaches the banner. Shared by Open in
/// Default App, Edit in External Editor and Reveal in Finder.
#[cfg(unix)]
#[core::prelude::v1::test]
fn launch_failures_reach_the_banner() {
    use crate::app::viewer_actions::{
        launch_result, DEFAULT_APP_FAILED_MESSAGE, EXTERNAL_EDITOR_FAILED_MESSAGE,
        REVEAL_FAILED_MESSAGE,
    };
    use std::os::unix::process::ExitStatusExt;
    use std::process::{ExitStatus, Output};

    let output = |code: i32, stderr: &str| {
        Ok(Output {
            status: ExitStatus::from_raw(code << 8),
            stdout: Vec::new(),
            stderr: stderr.as_bytes().to_vec(),
        })
    };
    assert_eq!(
        launch_result(output(0, ""), DEFAULT_APP_FAILED_MESSAGE),
        Ok(()),
        "success is quiet"
    );
    for stderr in [
        "No application knows how to open file:///x.heic (-10814)\n",
        "",
    ] {
        assert_eq!(
            launch_result(output(1, stderr), DEFAULT_APP_FAILED_MESSAGE),
            Err(DEFAULT_APP_FAILED_MESSAGE.into()),
            "a failed launch shows a plain message; the raw reason goes to the log"
        );
    }
    assert_eq!(
        launch_result(
            output(1, "Unable to find application named 'Gone'\n"),
            EXTERNAL_EDITOR_FAILED_MESSAGE
        ),
        Err(EXTERNAL_EDITOR_FAILED_MESSAGE.into()),
        "a deleted editor app is reported"
    );
    assert_eq!(
        launch_result(output(1, ""), REVEAL_FAILED_MESSAGE),
        Err(REVEAL_FAILED_MESSAGE.into()),
        "a failed Finder reveal is reported"
    );
    let spawn = launch_result(
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "launcher missing",
        )),
        DEFAULT_APP_FAILED_MESSAGE,
    );
    assert_eq!(
        spawn,
        Err(format!(
            "{DEFAULT_APP_FAILED_MESSAGE} Reason: launcher missing."
        )),
        "a launcher that cannot start says why"
    );
}

#[core::prelude::v1::test]
fn context_menu_get_info_opens_the_get_info_window() {
    let (temp_dir, engine) = seed_library("menu_get_info", 3);
    let card = media_card(&engine);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, _| {
        app.inspector_tab = crate::views::inspector::InspectorTab::Location
    });

    right_click(vcx, card);
    click(vcx, "inspect-menu-btn");
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        assert!(
            app.floating_inspector_window.is_some(),
            "Get Info (⌘I) opens the Get Info window, not the docked inspector"
        );
        assert!(!app.inspector_open, "the docked inspector stays closed");
        assert_eq!(
            app.inspector_tab,
            crate::views::inspector::InspectorTab::Info,
            "Get Info opens on its Info tab"
        );
    });
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn cmd_i_opens_get_info_and_i_toggles_the_inspector() {
    let (temp_dir, engine) = seed_library("cmd_i", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let docked = view.update(vcx, |app, _| app.inspector_open);
    crate::tests::ui_automation::keystrokes(vcx, "secondary-i");
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        assert!(
            app.floating_inspector_window.is_none(),
            "⌘I with nothing selected does nothing, as in Finder"
        )
    });

    crate::tests::ui_automation::keystrokes(vcx, "i");
    view.update(vcx, |app, _| {
        assert_ne!(app.inspector_open, docked, "I toggles the docked inspector")
    });
    crate::tests::ui_automation::keystrokes(vcx, "home");
    crate::tests::ui_automation::keystrokes(vcx, "secondary-i");
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        assert!(!app.inspector_open, "⌘I undocks the inspector");
        assert!(
            app.floating_inspector_window.is_some(),
            "⌘I opens the Get Info window"
        );
    });
    let _ = std::fs::remove_dir_all(&temp_dir);
}
