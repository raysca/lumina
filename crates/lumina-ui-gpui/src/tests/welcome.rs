//! v3 Welcome and "Building your library" (FIXES §9).

use crate::app::first_scan::FirstScanState;
use crate::tests::ui_automation::{
    app_with_library, click, focus_app, keystrokes, render_ui, seed_library,
};
use crate::{LuminaApp, UiMessage};
use gpui::{Entity, VisualTestContext};
use lumina_core::EngineEvent;

#[core::prelude::v1::test]
fn welcome_is_a_left_aligned_column_with_two_equal_buttons() {
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    view.update(vcx, |app, cx| {
        assert!(app.grid_state.is_none(), "a fresh app shows the welcome");
        app.has_desktop_pics = true;
        cx.notify();
    });
    render_ui(vcx);

    let column = vcx.debug_bounds("welcome-column").expect("welcome renders");
    assert_eq!(f32::from(column.size.width), 520.0);
    assert_eq!(
        f32::from(column.origin.y),
        120.0,
        "the column starts 120px down"
    );
    let logo = vcx.debug_bounds("welcome-logo").unwrap();
    assert_eq!(f32::from(logo.size.width), 60.0);
    assert_eq!(logo.origin.x, column.origin.x, "content is left-aligned");
    let open = vcx.debug_bounds("welcome-open-btn").unwrap();
    let import = vcx.debug_bounds("welcome-import-new-btn").unwrap();
    assert_eq!(open.size.width, import.size.width, "two equal buttons");
    assert_eq!(open.origin.y, import.origin.y, "side by side");
    let pictures = vcx
        .debug_bounds("welcome-pictures-link")
        .expect("Use Pictures Folder stays as a secondary link");
    assert!(
        pictures.origin.y > open.origin.y,
        "the Pictures link sits below the two buttons"
    );
    assert!(
        pictures.size.height < open.size.height,
        "the Pictures link is quieter than a button"
    );
    for hint in [
        "welcome-open-key",
        "welcome-import-new-key",
        "welcome-pictures-key",
        "welcome-sample-key",
    ] {
        assert!(
            vcx.debug_bounds(hint).is_some(),
            "{hint}: every welcome path shows its keyboard shortcut"
        );
    }

    let (folders, new_libraries, pictures_opens) = (picks(), new_library_picks(), picture_opens());
    click(vcx, "welcome-open-btn");
    click(vcx, "welcome-import-new-btn");
    click(vcx, "welcome-pictures-link");
    assert_eq!(
        picks(),
        folders + 1,
        "Open Existing Library… opens the picker"
    );
    assert_eq!(
        new_library_picks(),
        new_libraries + 1,
        "Import into New Library… asks for the new library's folder"
    );
    assert_eq!(
        picture_opens(),
        pictures_opens + 1,
        "the link opens the Pictures folder"
    );
}

fn feed(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, event: EngineEvent) {
    view.update(vcx, |app, cx| {
        app.handle_ui_message(UiMessage::Engine(event), cx)
    });
    render_ui(vcx);
}

fn progress(found: usize, total: usize, skipped: usize, file: &str, done: bool) -> EngineEvent {
    EngineEvent::ScanProgress {
        items_found: found,
        is_complete: done,
        total,
        skipped,
        current_file: Some(file.into()),
    }
}

#[core::prelude::v1::test]
fn first_scan_shows_progress_counts_and_opens_the_library_when_done() {
    let (dir, engine) = seed_library("welcome_progress", 3);
    let items = engine.get_item_slice(0..3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.first_scan = Some(FirstScanState::new());
        cx.notify();
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("first-scan").is_some(),
        "the scan screen replaces the grid"
    );

    feed(&view, vcx, progress(0, 4, 0, "/x/a.jpg", false));
    feed(
        &view,
        vcx,
        EngineEvent::BatchDiscovered {
            items: items.clone(),
        },
    );
    feed(&view, vcx, progress(3, 4, 1, "/x/photo_02.jpg", false));
    view.update(vcx, |app, _| {
        let scan = app.first_scan.as_ref().unwrap();
        assert_eq!(
            (scan.photos, scan.videos, scan.raw, scan.skipped),
            (3, 0, 0, 1)
        );
        assert_eq!((scan.processed, scan.total), (4, 4));
        assert_eq!(scan.just_added.len(), 3, "JUST ADDED holds the new items");
        assert_eq!(
            scan.current_file.as_deref(),
            Some(std::path::Path::new("/x/photo_02.jpg"))
        );
    });
    for selector in [
        "first-scan-bar",
        "first-scan-file",
        "first-scan-progress",
        "first-scan-photos",
        "first-scan-skipped",
        "first-scan-just-added",
        "first-scan-open",
        "first-scan-stop",
    ] {
        assert!(
            vcx.debug_bounds(selector).is_some(),
            "{selector} must render"
        );
    }

    feed(&view, vcx, EngineEvent::ScanComplete { total_items: 3 });
    view.update(vcx, |app, _| {
        assert!(
            app.first_scan.is_none(),
            "the library opens when the scan is done"
        )
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn enter_opens_the_library_and_cmd_period_stops_the_scan() {
    let (dir, engine) = seed_library("welcome_keys", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let start = |view: &Entity<LuminaApp>, vcx: &mut VisualTestContext| {
        view.update(vcx, |app, cx| {
            app.first_scan = Some(FirstScanState::new());
            cx.notify();
        });
        render_ui(vcx);
        focus_app(vcx, view);
    };

    start(&view, vcx);
    keystrokes(vcx, "f");
    view.update(vcx, |app, _| {
        assert!(
            app.first_scan.is_some(),
            "grid shortcuts don't reach behind the screen"
        );
        assert!(
            !app.selected_item.as_ref().is_some_and(|i| i.is_favorite),
            "F must not favourite a hidden item"
        );
    });
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert!(app.first_scan.is_none(), "↵ opens the library")
    });

    start(&view, vcx);
    keystrokes(vcx, "secondary-.");
    view.update(vcx, |app, _| {
        assert!(app.first_scan.is_none(), "⌘. stops the scan");
        assert!(
            app.toast.current().is_some(),
            "and says how to finish later"
        );
    });

    start(&view, vcx);
    click(vcx, "first-scan-open");
    view.update(vcx, |app, _| assert!(app.first_scan.is_none()));

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn only_an_empty_catalog_gets_the_scan_screen() {
    let (populated, _engine) = seed_library("welcome_populated", 2);
    let empty = std::env::temp_dir().join(format!(
        "lumina_uitest_welcome_empty_{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&empty).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));

    view.update(vcx, |app, cx| {
        app.open_library_inner(empty.clone(), cx);
        assert!(
            app.first_scan.is_some(),
            "a new library shows the scan screen"
        );
        app.open_library_inner(populated.clone(), cx);
        assert!(
            app.first_scan.is_none(),
            "a scanned library opens straight away"
        );
    });

    let _ = std::fs::remove_dir_all(&populated);
    let _ = std::fs::remove_dir_all(&empty);
}

#[core::prelude::v1::test]
fn a_replaced_librarys_late_scan_complete_is_ignored() {
    let (dir, engine) = seed_library("welcome_stale", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.library_generation = 2;
        app.first_scan = Some(FirstScanState::new());
        // The previous library's cancelled scan finishing late.
        app.handle_ui_message(
            UiMessage::LibraryEvent {
                generation: 1,
                event: EngineEvent::ScanComplete { total_items: 9 },
            },
            cx,
        );
        assert!(
            app.first_scan.is_some(),
            "an old library's ScanComplete must not dismiss the new scan"
        );
        app.handle_ui_message(
            UiMessage::LibraryEvent {
                generation: 2,
                event: EngineEvent::ScanComplete { total_items: 1 },
            },
            cx,
        );
        assert!(
            app.first_scan.is_none(),
            "the current library's event applies"
        );
    });

    let _ = std::fs::remove_dir_all(&dir);
}

// ---- Recent libraries and the missing-library notice (UX-1 Task 6) ----

use crate::app::command_palette::{CommandId, PaletteTarget};
use crate::app::downloads::DownloadsConfirm;
use crate::app::library::FOLDER_PICKS;
use crate::app::recents::{missing_library_message, startup_library};
use crate::tests::downloads::{install, queue_app, settle, submit, TempLibrary};
use std::path::PathBuf;

/// An empty temporary folder, deleted when the test ends.
fn temp_folder(name: &str) -> TempLibrary {
    let dir = std::env::temp_dir().join(format!(
        "lumina_uitest_recent_{name}_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    TempLibrary(dir.canonicalize().unwrap())
}

fn welcome_with_recents(
    cx: &mut gpui::TestAppContext,
    recents: Vec<PathBuf>,
) -> (Entity<LuminaApp>, &mut VisualTestContext) {
    let (view, vcx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    view.update(vcx, |app, cx| {
        app.config.recent_libraries = recents;
        cx.notify();
    });
    render_ui(vcx);
    focus_app(vcx, &view);
    (view, vcx)
}

fn picks() -> usize {
    FOLDER_PICKS.with(|n| n.get())
}

fn open_root(app: &LuminaApp) -> Option<PathBuf> {
    app.grid_state
        .as_ref()
        .map(|state| state.engine.library_root().to_path_buf())
}

#[core::prelude::v1::test]
fn welcome_lists_recents_and_enter_opens_focused() {
    let (a, b) = (temp_folder("list_a"), temp_folder("list_b"));
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = welcome_with_recents(&mut cx, vec![a.to_path_buf(), b.to_path_buf()]);
    for selector in ["welcome-recent-0", "welcome-recent-1"] {
        assert!(
            vcx.debug_bounds(selector).is_some(),
            "{selector}: each recent library is listed"
        );
    }
    let before = picks();
    keystrokes(vcx, "down down");
    view.update(vcx, |app, _| {
        assert_eq!(app.welcome.focused_recent, Some(1), "↓ moves the focus")
    });
    keystrokes(vcx, "up down enter");
    view.update(vcx, |app, _| {
        assert_eq!(
            open_root(app).as_deref(),
            Some(b.as_path()),
            "↵ opens the focused recent"
        );
        assert_eq!(picks(), before, "↵ on a recent opens no folder picker");
        assert_eq!(
            app.config.recent_libraries,
            vec![b.to_path_buf(), a.to_path_buf()],
            "the opened library moves to the front"
        );
    });
}

#[core::prelude::v1::test]
fn welcome_click_opens_a_recent() {
    let a = temp_folder("click_a");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = welcome_with_recents(&mut cx, vec![a.to_path_buf()]);
    click(vcx, "welcome-recent-0");
    view.update(vcx, |app, _| {
        assert_eq!(
            open_root(app).as_deref(),
            Some(a.as_path()),
            "a click opens it"
        )
    });
}

#[core::prelude::v1::test]
fn welcome_backspace_removes_focused_recent() {
    let (a, b, c) = (
        temp_folder("rm_a"),
        temp_folder("rm_b"),
        temp_folder("rm_c"),
    );
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = welcome_with_recents(
        &mut cx,
        vec![a.to_path_buf(), b.to_path_buf(), c.to_path_buf()],
    );
    keystrokes(vcx, "down down backspace");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.config.recent_libraries,
            vec![a.to_path_buf(), c.to_path_buf()],
            "⌫ forgets the focused recent"
        );
        assert_eq!(app.welcome.focused_recent, Some(1), "focus stays in place");
    });
    render_ui(vcx);
    click(vcx, "welcome-recent-remove-0");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.config.recent_libraries,
            vec![c.to_path_buf()],
            "× forgets its row"
        );
        assert!(app.grid_state.is_none(), "× does not open the row");
    });
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert_eq!(app.welcome.focused_recent, None, "Esc clears the focus")
    });
    for dir in [&a, &b, &c] {
        assert!(dir.is_dir(), "forgetting a recent never deletes its folder");
    }
}

#[core::prelude::v1::test]
fn failed_recents_save_reverts_and_shows_banner() {
    let (a, b) = (temp_folder("save_a"), temp_folder("save_b"));
    let mut cx = crate::tests::TestApp::single();
    let recents = vec![a.to_path_buf(), b.to_path_buf()];
    let (view, vcx) = welcome_with_recents(&mut cx, recents.clone());
    view.update(vcx, |app, _| {
        assert!(app.error_message.is_none(), "control: no banner yet")
    });
    crate::app::state::SAVE_FAILS.with(|f| f.set(true));
    keystrokes(vcx, "down backspace");
    view.update(vcx, |app, cx| {
        assert_eq!(
            app.config.recent_libraries, recents,
            "a failed save puts the removed recent back"
        );
        assert!(
            app.error_message
                .as_deref()
                .is_some_and(|m| m.starts_with("Could not save settings")),
            "and the banner says why, got {:?}",
            app.error_message
        );
        app.clear_recents(cx);
        assert_eq!(
            app.config.recent_libraries, recents,
            "clear is reverted too"
        );
    });
    crate::app::state::SAVE_FAILS.with(|f| f.set(false));
}

#[core::prelude::v1::test]
fn missing_last_library_shows_notice() {
    let gone = temp_folder("gone").to_path_buf(); // deleted at once
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = welcome_with_recents(&mut cx, Vec::new());
    view.update(vcx, |app, cx| {
        app.config.last_library_path = Some(gone.clone());
        let startup = startup_library(&app.config);
        app.apply_startup_library(startup, cx);
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("welcome-missing-library").is_some(),
        "a missing last library is explained, not skipped silently"
    );
    let name = gone.file_name().unwrap().to_string_lossy().into_owned();
    assert!(
        missing_library_message(&gone).contains(&name),
        "the notice names the folder"
    );
    view.update(vcx, |app, _| {
        assert_eq!(app.welcome.missing.as_deref(), Some(gone.as_path()));
        assert!(app.grid_state.is_none());
    });
    let before = picks();
    click(vcx, "welcome-open-btn");
    keystrokes(vcx, "enter");
    assert_eq!(picks(), before + 2, "the Open button and ↵ stay usable");
}

#[core::prelude::v1::test]
fn enter_on_unavailable_recent_shows_notice_not_open() {
    let gone = temp_folder("unavail").to_path_buf();
    let here = temp_folder("avail");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = welcome_with_recents(&mut cx, vec![gone.clone(), here.to_path_buf()]);
    // The off-thread check has answered; draw what it found.
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(
            app.welcome
                .unavailable
                .as_ref()
                .is_some_and(|set| set.contains(&gone) && !set.contains(here.as_path())),
            "the availability check finds the gone folder only"
        )
    });
    assert!(
        vcx.debug_bounds("welcome-recent-unavailable-0").is_some(),
        "an unavailable recent says so in text"
    );
    assert!(vcx.debug_bounds("welcome-recent-unavailable-1").is_none());
    keystrokes(vcx, "down enter");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.grid_state.is_none(), "a gone folder is not opened");
        assert_eq!(app.welcome.missing.as_deref(), Some(gone.as_path()));
        assert_eq!(app.config.recent_libraries.len(), 2, "it stays in the list");
    });
    assert!(vcx.debug_bounds("welcome-missing-library").is_some());
}

fn library_rows(app: &LuminaApp) -> Vec<PathBuf> {
    app.palette_rows()
        .into_iter()
        .filter_map(|row| match row.target {
            PaletteTarget::Library(path) => Some(path),
            _ => None,
        })
        .collect()
}

#[core::prelude::v1::test]
fn palette_opens_a_recent_library() {
    let (dir, engine) = seed_library("recent_palette", 1);
    let current = TempLibrary(dir);
    let other = temp_folder("palette_other");
    let gone = temp_folder("palette_gone").to_path_buf();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, _| {
        // While a library is open it is the first recent (open_library_inner).
        app.config.recent_libraries = vec![
            current.canonicalize().unwrap(),
            other.to_path_buf(),
            gone.clone(),
        ];
        app.welcome.unavailable = Some([gone.clone()].into_iter().collect());
        assert_eq!(
            library_rows(app),
            vec![other.to_path_buf()],
            "rows leave out the open library and unavailable ones"
        );
    });
    keystrokes(vcx, "secondary-k o p e n space r e c e n t");
    view.update(vcx, |app, _| {
        let row = app
            .palette_rows()
            .into_iter()
            .find(|row| matches!(row.target, PaletteTarget::Library(_)))
            .expect("typing finds the recent");
        assert!(
            row.title
                .contains(&other.file_name().unwrap().to_string_lossy().into_owned()),
            "the row names the folder: {}",
            row.title
        );
    });
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert!(!app.command_palette_open);
        assert_eq!(
            open_root(app).as_deref(),
            Some(other.as_path()),
            "↵ opens it"
        );
    });
}

#[core::prelude::v1::test]
fn palette_recent_while_downloading_asks_first() {
    let (dir, engine, fake, mut cx) = queue_app("recent_dl");
    let other = temp_folder("dl_other");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    submit(&view, vcx, "https://example.com/a.mp4");
    settle(vcx, &view, "the job runs", |app| {
        app.running_or_waiting_count() == 1
    });
    view.update(vcx, |app, cx| {
        app.config.recent_libraries = vec![dir.canonicalize().unwrap(), other.to_path_buf()];
        app.run_palette_target(PaletteTarget::Library(other.to_path_buf()), cx);
        assert_eq!(
            app.downloads.confirm,
            Some(DownloadsConfirm::OpenLibrary(other.to_path_buf())),
            "switching library with a running download asks first"
        );
        assert_eq!(
            open_root(app).as_deref(),
            Some(dir.as_path()),
            "the library is unchanged"
        );
    });
}

#[core::prelude::v1::test]
fn clear_recents_empties_the_list_only() {
    let (dir, engine) = seed_library("recent_clear", 1);
    let current = TempLibrary(dir);
    let other = temp_folder("clear_other");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let clear_listed = |app: &LuminaApp| {
        app.palette_rows()
            .iter()
            .any(|row| row.target == PaletteTarget::Command(CommandId::ClearRecents))
    };
    view.update(vcx, |app, cx| {
        app.command_query = "clear recent".into();
        assert!(
            !clear_listed(app),
            "nothing to clear: the command is hidden"
        );
        app.config.recent_libraries = vec![current.canonicalize().unwrap(), other.to_path_buf()];
        assert!(clear_listed(app), "⌘K offers Clear Recent Libraries");
        app.execute_command(CommandId::ClearRecents, cx);
        assert!(app.config.recent_libraries.is_empty(), "the list is empty");
        assert!(!clear_listed(app));
        assert_eq!(
            open_root(app).as_deref(),
            Some(current.as_path()),
            "the open library stays open"
        );
    });
    assert!(current.join("catalog.db").is_file(), "its catalog is kept");
    assert!(other.is_dir(), "the other folder is kept");
}

#[core::prelude::v1::test]
fn failed_open_does_not_become_last_library() {
    let folder = temp_folder("not_a_dir");
    let file = folder.join("photo.jpg");
    std::fs::write(&file, b"not a folder").unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    view.update(vcx, |app, cx| {
        app.open_library_inner(file.clone(), cx);
        assert!(app.grid_state.is_none(), "a file is not a library");
        assert_eq!(
            app.config.last_library_path, None,
            "a failed open must not become the startup library"
        );
        assert!(app.config.recent_libraries.is_empty(), "nor a recent");
        assert!(app.error_message.is_some(), "the banner says it failed");
    });
}

// ---- Menu actions and the key gate on the welcome screen (UX-1 Task 7a) ----

#[core::prelude::v1::test]
fn welcome_cmd_k_does_not_open_an_invisible_palette() {
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = welcome_with_recents(&mut cx, Vec::new());
    keystrokes(vcx, "secondary-k secondary-/");
    view.update(vcx, |app, _| {
        assert!(
            !app.command_palette_open,
            "⌘K must not open a palette the welcome screen never paints"
        );
        assert!(
            !app.shortcuts_overlay_open,
            "⌘/ must not open a sheet the welcome screen never paints"
        );
    });
    let before = picks();
    keystrokes(vcx, "enter");
    assert_eq!(
        picks(),
        before + 1,
        "↵ still reaches the welcome picker after the gated keys"
    );
}

#[core::prelude::v1::test]
fn welcome_menu_actions_reach_the_app() {
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = welcome_with_recents(&mut cx, Vec::new());
    let before = picks();
    vcx.dispatch_action(crate::actions::OpenLibrary);
    assert_eq!(
        picks(),
        before + 1,
        "File > Open Library… works on the welcome screen"
    );
    vcx.dispatch_action(crate::actions::QuitApp);
    view.update(vcx, |app, _| {
        assert!(
            app.downloads.quit_requested,
            "Quit works on the welcome screen"
        )
    });
}

#[core::prelude::v1::test]
fn first_scan_quit_reaches_the_app() {
    let (dir, engine) = seed_library("first_scan_quit", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.first_scan = Some(FirstScanState::new());
        cx.notify();
    });
    render_ui(vcx);
    focus_app(vcx, &view);
    let before = picks();
    vcx.dispatch_action(crate::actions::OpenLibrary);
    assert_eq!(
        picks(),
        before + 1,
        "File > Open Library… works while the first scan runs"
    );
    vcx.dispatch_action(crate::actions::QuitApp);
    view.update(vcx, |app, _| {
        assert!(
            app.downloads.quit_requested,
            "Quit works while the first scan runs"
        )
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn welcome_preferences_paint_and_escape_closes() {
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = welcome_with_recents(&mut cx, Vec::new());

    vcx.dispatch_action(crate::actions::OpenPreferences);
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("preferences-card").is_some(),
        "Preferences paints on the welcome screen when opened"
    );
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(!app.preferences_open, "Esc closes Preferences")
    });

    keystrokes(vcx, "secondary-,");
    view.update(vcx, |app, _| {
        assert!(app.preferences_open, "⌘, opens Preferences on welcome")
    });
    keystrokes(vcx, "escape");

    vcx.dispatch_action(crate::actions::ShowAbout);
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("preferences-version").is_some(),
        "About paints on the welcome screen when opened"
    );
    keystrokes(vcx, "escape");

    let before = picks();
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert!(!app.preferences_open, "Esc closed About");
    });
    assert_eq!(
        picks(),
        before + 1,
        "↵ reaches the welcome picker once Preferences is closed"
    );
}

#[core::prelude::v1::test]
fn welcome_cmd_o_and_escape_work_and_modified_enter_is_gated() {
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = welcome_with_recents(&mut cx, Vec::new());
    let before = picks();
    keystrokes(vcx, "secondary-o");
    assert_eq!(picks(), before + 1, "⌘O on welcome opens the folder picker");
    keystrokes(vcx, "secondary-enter alt-enter");
    assert_eq!(
        picks(),
        before + 1,
        "⌘↵ and ⌥↵ are not plain ↵ (⌥↵ is kept for the sample library)"
    );

    view.update(vcx, |app, cx| app.set_error("boom", cx));
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(
            app.error_message.is_none(),
            "Esc on welcome dismisses the error banner"
        );
    });
}

// ---- Welcome paths: open, import into a new library, Pictures (Task 7b) ----

fn new_library_picks() -> usize {
    crate::app::import::NEW_LIBRARY_PICKS.with(|n| n.get())
}

fn import_picks() -> usize {
    crate::app::import::IMPORT_PICKS.with(|n| n.get())
}

fn picture_opens() -> usize {
    crate::app::first_scan::PICTURE_OPENS.with(|n| n.get())
}

#[core::prelude::v1::test]
fn import_into_new_library_opens_then_asks_for_sources() {
    let folder = temp_folder("new_library");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = welcome_with_recents(&mut cx, Vec::new());
    let before = import_picks();
    // Checked in the same update: the real scan's own ScanComplete may
    // arrive at the next run-until-parked.
    view.update(vcx, |app, cx| {
        app.handle_ui_message(UiMessage::NewLibraryPicked(folder.to_path_buf()), cx);
        assert_eq!(
            open_root(app).as_deref(),
            Some(folder.as_ref()),
            "the chosen folder opens as the new library"
        );
        assert!(
            app.first_scan.is_some(),
            "an empty library shows the first-scan screen"
        );
        assert!(app.import.pending_import_after_open);
        assert_eq!(
            import_picks(),
            before,
            "the import picker waits for the first scan: that screen paints no import dialog"
        );
    });

    feed(&view, vcx, EngineEvent::ScanComplete { total_items: 0 });
    assert_eq!(
        import_picks(),
        before + 1,
        "the first ScanComplete of the new library asks for the photos to copy in"
    );
    feed(&view, vcx, EngineEvent::ScanComplete { total_items: 0 });
    assert_eq!(
        import_picks(),
        before + 1,
        "a later scan (⌘R) must not ask again"
    );
}

#[core::prelude::v1::test]
fn new_library_open_failure_asks_nothing() {
    let folder = temp_folder("new_library_fail");
    let file = folder.join("photo.jpg");
    std::fs::write(&file, b"not a folder").unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = welcome_with_recents(&mut cx, Vec::new());
    let before = import_picks();
    view.update(vcx, |app, cx| {
        app.handle_ui_message(UiMessage::NewLibraryPicked(file.clone()), cx);
        assert!(app.grid_state.is_none(), "a file is not a library");
        assert!(app.error_message.is_some(), "the banner says it failed");
        assert!(
            !app.import.pending_import_after_open,
            "a failed open must not leave an import waiting for a later library"
        );
    });
    assert_eq!(
        import_picks(),
        before,
        "no import picker after a failed open"
    );
}

#[core::prelude::v1::test]
fn welcome_keyboard_paths() {
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = welcome_with_recents(&mut cx, Vec::new());
    view.update(vcx, |app, cx| {
        app.has_desktop_pics = true;
        cx.notify();
    });
    render_ui(vcx);
    let (folders, new_libraries, pictures, imports) = (
        picks(),
        new_library_picks(),
        picture_opens(),
        import_picks(),
    );

    keystrokes(vcx, "enter");
    assert_eq!(picks(), folders + 1, "↵ opens an existing library");
    keystrokes(vcx, "shift-enter");
    assert_eq!(picture_opens(), pictures + 1, "⇧↵ uses the Pictures folder");
    keystrokes(vcx, "secondary-shift-i");
    assert_eq!(
        new_library_picks(),
        new_libraries + 1,
        "⇧⌘I imports into a new library on the welcome screen"
    );
    view.update(vcx, |app, _| {
        assert!(
            app.toast.current().is_none(),
            "⇧⌘I on welcome must not say \"Open a library before importing\""
        );
    });
    vcx.dispatch_action(crate::actions::ImportMedia);
    assert_eq!(
        new_library_picks(),
        new_libraries + 2,
        "File > Import… on welcome imports into a new library"
    );
    keystrokes(vcx, "secondary-i alt-shift-secondary-i");
    assert_eq!(
        new_library_picks(),
        new_libraries + 2,
        "⌘I and ⌥⇧⌘I are not ⇧⌘I"
    );
    assert_eq!(
        import_picks(),
        imports,
        "no import picker without a library"
    );
    assert_eq!(picks(), folders + 1, "no stray folder pickers");
}

#[core::prelude::v1::test]
fn rescan_before_the_first_scan_completes_drops_the_pending_import() {
    let folder = temp_folder("new_library_rescan");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = welcome_with_recents(&mut cx, Vec::new());
    let before = import_picks();
    view.update(vcx, |app, cx| {
        app.handle_ui_message(UiMessage::NewLibraryPicked(folder.to_path_buf()), cx);
        assert!(app.import.pending_import_after_open);
        // A failed first scan sends no ScanComplete; the user then rescans.
        app.rescan_library(cx);
        assert!(
            !app.import.pending_import_after_open,
            "the user's own rescan cancels the new-library import prompt"
        );
    });
    feed(&view, vcx, EngineEvent::ScanComplete { total_items: 0 });
    assert_eq!(
        import_picks(),
        before,
        "a rescan's ScanComplete must not open the import picker out of nowhere"
    );
}

// ---- Explore a Sample Library (Task 8) ----

fn sample_opens() -> usize {
    crate::app::sample::SAMPLE_OPENS.with(|n| n.get())
}

fn sample_files(dir: &std::path::Path) -> usize {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| e.file_name().to_string_lossy().starts_with("sample_"))
                .count()
        })
        .unwrap_or(0)
}

#[core::prelude::v1::test]
fn sample_button_opens_the_sample_library() {
    let folder = temp_folder("sample");
    let dest = folder.join("Sample Library");
    let other = folder.join("Second Request");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = welcome_with_recents(&mut cx, Vec::new());

    let before = sample_opens();
    click(vcx, "welcome-sample-btn");
    assert_eq!(
        sample_opens(),
        before + 1,
        "the button asks for the sample library"
    );

    view.update(vcx, |app, cx| {
        app.open_sample_library_at(dest.clone(), cx);
        assert!(
            app.grid_state.is_none(),
            "the photos are written off the UI thread before the library opens"
        );
        app.open_sample_library_at(other.clone(), cx);
    });
    vcx.run_until_parked();
    render_ui(vcx);

    assert_eq!(
        sample_files(&dest),
        lumina_core::sample::SAMPLE_COUNT,
        "every sample photo is written"
    );
    assert!(
        !other.exists(),
        "a second request while the first is generating is ignored"
    );
    view.update(vcx, |app, _| {
        assert_eq!(
            open_root(app),
            dest.canonicalize().ok(),
            "the sample library opens once its photos are written"
        );
        assert!(app.error_message.is_none(), "no banner on success");
        assert!(!app.welcome.generating_sample, "the job is cleared");
    });
}

#[core::prelude::v1::test]
fn sample_generation_failure_reaches_the_banner() {
    let folder = temp_folder("sample_fail");
    let file = folder.join("Sample Library");
    std::fs::write(&file, b"not a folder").unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = welcome_with_recents(&mut cx, Vec::new());
    view.update(vcx, |app, cx| app.open_sample_library_at(file.clone(), cx));
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        assert!(app.grid_state.is_none(), "no library opens after a failure");
        let message = app.error_message.clone().unwrap_or_default();
        assert!(
            message.contains("sample library"),
            "the banner says the sample library could not be prepared, got {message:?}"
        );
        assert!(
            !app.welcome.generating_sample,
            "a failure clears the job so the user can try again"
        );
    });
}

#[core::prelude::v1::test]
fn welcome_alt_enter_explores_the_sample_library() {
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = welcome_with_recents(&mut cx, Vec::new());
    view.update(vcx, |app, cx| {
        app.has_desktop_pics = true;
        cx.notify();
    });
    render_ui(vcx);
    let (samples, folders, pictures) = (sample_opens(), picks(), picture_opens());
    keystrokes(vcx, "alt-enter");
    assert_eq!(
        sample_opens(),
        samples + 1,
        "⌥↵ explores the sample library"
    );
    keystrokes(vcx, "alt-shift-enter secondary-alt-enter");
    assert_eq!(sample_opens(), samples + 1, "⌥⇧↵ and ⌥⌘↵ are not ⌥↵");
    assert_eq!(picks(), folders, "⌥↵ is not ↵");
    assert_eq!(picture_opens(), pictures, "⌥↵ is not ⇧↵");
}

#[core::prelude::v1::test]
fn sample_ready_after_another_library_opened_keeps_that_library() {
    let folder = temp_folder("sample_meanwhile");
    let dest = folder.join("Sample Library");
    let chosen = folder.join("Chosen");
    std::fs::create_dir_all(&chosen).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = welcome_with_recents(&mut cx, Vec::new());
    view.update(vcx, |app, cx| {
        app.open_sample_library_at(dest.clone(), cx);
        // The user picks a library while the sample photos are written.
        app.open_library(chosen.clone(), cx);
    });
    vcx.run_until_parked();
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            open_root(app).as_deref(),
            Some(chosen.as_path()),
            "a sample that finishes later must not replace the library the user opened"
        );
        assert!(!app.welcome.generating_sample, "the busy state is cleared");
    });
}
