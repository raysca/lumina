//! v3 Preferences sheet: grouped rows per tab, and the sheet fits the window.

use super::ui_automation::{app_with_library, click, render_ui, seed_library};
use crate::app::preferences::{thumbnail_cache_dir, CacheUsage};

#[core::prelude::v1::test]
fn preferences_tabs_group_actions_and_fit_a_small_window() {
    let (temp_dir, engine) = seed_library("prefs_v3", 2);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    vcx.simulate_resize(gpui::size(gpui::px(1024.0), gpui::px(560.0)));
    view.update(vcx, |app, cx| app.open_preferences(cx));
    render_ui(vcx);

    let card = vcx.debug_bounds("preferences-card").expect("sheet renders");
    assert!(
        card.bottom() <= gpui::px(560.0),
        "the Preferences sheet must fit inside a 560px-tall window"
    );
    for selector in [
        "preferences-change-library",
        "preferences-rescan",
        "preferences-choose-editor",
    ] {
        assert!(
            vcx.debug_bounds(selector).is_some(),
            "General must offer {selector}"
        );
    }

    view.update(vcx, |app, _| {
        if let Some(state) = app.grid_state.as_mut() {
            state.is_scanning = false;
        }
    });
    click(vcx, "preferences-rescan");
    view.update(vcx, |app, _| {
        assert!(
            app.grid_state.as_ref().is_some_and(|s| s.is_scanning),
            "Rescan Library in Preferences must start a scan"
        );
        assert!(
            app.preferences_open,
            "rescanning must keep Preferences open"
        );
    });

    click(vcx, "preferences-tab-storage");
    for selector in [
        "preferences-clear-cache",
        "preferences-reveal-config",
        "preferences-reveal-logs",
    ] {
        assert!(
            vcx.debug_bounds(selector).is_some(),
            "Storage must offer {selector}"
        );
    }

    click(vcx, "preferences-tab-about");
    assert!(
        vcx.debug_bounds("preferences-version").is_some(),
        "About must show the version"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn preferences_v3_counts_thumbnails_confirms_clear_and_opens_license() {
    let (temp_dir, engine) = seed_library("prefs_v3_details", 2);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.handle_ui_message(
            crate::app::types::UiMessage::Engine(lumina_core::EngineEvent::ScanComplete {
                total_items: 2,
            }),
            cx,
        );
        assert!(
            app.last_scan_at.is_some(),
            "a finished scan records its time"
        );
    });
    view.update(vcx, |app, cx| {
        app.last_scan_at =
            std::time::Instant::now().checked_sub(std::time::Duration::from_secs(125));
        app.open_preferences(cx);
    });
    render_ui(vcx);
    assert!(vcx.debug_bounds("preferences-indexed").is_some());
    assert!(
        vcx.debug_bounds("preferences-reveal-library").is_none(),
        "v3 drops Reveal from the Library folder row"
    );

    // Storage: "size · N thumbnails", then Clear Cache confirms inline.
    let cache = thumbnail_cache_dir();
    std::fs::create_dir_all(cache.join("ab")).unwrap();
    std::fs::write(cache.join("ab/1.jpg"), b"thumb").unwrap();
    std::fs::write(cache.join("2.jpg"), b"thumb").unwrap();
    click(vcx, "preferences-tab-storage");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.preferences_cache,
            Some(CacheUsage {
                bytes: 10,
                files: 2
            }),
            "Size on disk counts thumbnails, nested ones too"
        );
    });
    assert!(vcx.debug_bounds("preferences-cache-size").is_some());
    click(vcx, "preferences-clear-cache");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.preferences_cache.map(|u| u.files), Some(0));
        assert!(app.preferences_cache_cleared, "Clear Cache confirms inline");
        assert!(app.error_message.is_none());
    });
    assert!(vcx.debug_bounds("preferences-cache-cleared").is_some());
    view.update(vcx, |app, cx| {
        app.close_preferences(cx);
        app.open_preferences(cx);
        assert!(
            !app.preferences_cache_cleared,
            "reopening hides the confirmation"
        );
    });

    // About: MIT License and Acknowledgements resolve to real files.
    view.update(vcx, |app, cx| app.open_about(cx));
    render_ui(vcx);
    click(vcx, "preferences-license");
    click(vcx, "preferences-acknowledgements");
    view.update(vcx, |app, _| {
        assert!(app.error_message.is_none(), "both documents are found");
    });
    for name in ["LICENSE", "THIRD_PARTY_NOTICES.md"] {
        assert!(
            crate::app::preferences::bundled_doc(name).is_some(),
            "{name} resolves"
        );
    }

    // Keyboard path: ⌘K runs both documents too.
    for query in ["l i c e n s e enter", "a c k n o w enter"] {
        super::ui_automation::focus_app(vcx, &view);
        super::ui_automation::keystrokes(vcx, &format!("secondary-k {query}"));
        view.update(vcx, |app, _| {
            assert!(!app.command_palette_open, "{query} ran a command");
            assert!(app.error_message.is_none(), "{query} found its document");
        });
    }

    // Switching library drops the previous library's scan time.
    let (other_dir, _) = seed_library("prefs_v3_other", 1);
    view.update(vcx, |app, cx| {
        app.open_library(other_dir.clone(), cx);
        assert!(
            app.last_scan_at.is_none(),
            "a new library has no scan time yet"
        );
    });

    let _ = std::fs::remove_dir_all(&cache);
    let _ = std::fs::remove_dir_all(&temp_dir);
    let _ = std::fs::remove_dir_all(&other_dir);
}

#[core::prelude::v1::test]
fn library_folder_reads_from_home() {
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| crate::LuminaApp::new(cx));
    let home = dirs::home_dir().expect("tests run with a home directory");
    view.update(vcx, |app, _| {
        app.grid_state = None;
        app.config.last_library_path = Some(home.join("Pictures").join("Lumina"));
        assert_eq!(
            app.library_path_label(),
            "~/Pictures/Lumina",
            "the Library folder row shortens the home directory to ~"
        );
    });
}
