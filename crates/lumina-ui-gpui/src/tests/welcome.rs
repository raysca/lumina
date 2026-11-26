//! The welcome screen, recent libraries and "Building your library".

use crate::app::first_scan::FirstScanState;
use crate::tests::helpers::{
    app_with_library, click, focus_app, keystrokes, leak, render_ui, seed_library, wheel,
};
use crate::{LuminaApp, UiMessage};
use gpui::{Entity, VisualTestContext};
use lumina_core::EngineEvent;

/// The first-run empty state is centred in the main
/// column, with Choose a Folder… beside Import into New Library….
#[core::prelude::v1::test]
fn first_run_empty_state_is_centred_with_folder_and_import_actions() {
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    view.update(vcx, |app, cx| {
        assert!(app.grid_state.is_none(), "a fresh app shows the welcome");
        app.has_desktop_pics = true;
        cx.notify();
    });
    render_ui(vcx);

    let header = vcx.debug_bounds("page-header").expect("header renders");
    let glyph = vcx
        .debug_bounds("welcome-glyph")
        .expect("the glyph renders");
    let side = (f32::from(glyph.size.width), f32::from(glyph.size.height));
    assert_eq!(side, (150.0, 150.0), "the glyph is a 150 px box");
    let offset = f32::from(glyph.center().x - header.center().x);
    assert!(
        offset.abs() <= 1.0,
        "the glyph is centred in the main column, off by {offset}"
    );
    assert!(
        vcx.debug_bounds("welcome-title-text:Your library is empty")
            .is_some(),
        "the headline says the library is empty"
    );
    let open = vcx.debug_bounds("welcome-open-btn").unwrap();
    let import = vcx.debug_bounds("welcome-import-new-btn").unwrap();
    assert_eq!(open.origin.y, import.origin.y, "side by side");
    assert!(
        open.right() <= import.left(),
        "Choose a Folder… comes first"
    );
    for (button, icon) in [
        (open, "welcome-open-btn-icon"),
        (import, "welcome-import-new-btn-icon"),
    ] {
        let icon = vcx.debug_bounds(icon).expect("each button has an icon");
        assert!(
            f32::from(icon.left() - button.left()) <= 19.0,
            "the icon leads the label, right after the 18 px padding: {icon:?} in {button:?}"
        );
    }
    let sample = vcx.debug_bounds("welcome-sample-btn").unwrap();
    assert!(
        sample.top() >= open.bottom(),
        "the sample link sits below the buttons"
    );
    let hints: Vec<_> = ["welcome-hint-0", "welcome-hint-1", "welcome-hint-2"]
        .into_iter()
        .map(|hint| vcx.debug_bounds(hint).expect("three hint cards"))
        .collect();
    assert!(
        hints
            .iter()
            .all(|h| f32::from(h.size.width - hints[0].size.width).abs() <= 1.0),
        "the hint cards have equal widths (±1 for pixel snapping): {hints:?}"
    );
    assert!(
        hints[0].top() >= sample.bottom(),
        "the hints sit below the links"
    );
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
    assert_eq!(picks(), folders + 1, "Choose a Folder… opens the picker");
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

/// With no library the window keeps the app shell, a
/// dormant sidebar and the "All Media" header, around the welcome body.
#[core::prelude::v1::test]
fn first_run_has_sidebar_and_header() {
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    render_ui(vcx);

    assert!(
        vcx.debug_bounds("sidebar").is_some(),
        "the welcome paints the sidebar"
    );
    assert!(
        vcx.debug_bounds("page-title-text:All Media").is_some(),
        "the header reads All Media"
    );
    assert!(
        vcx.debug_bounds("page-meta-text:No media yet").is_some(),
        "the meta line says there is no media"
    );
    let actions = vcx.debug_bounds("page-actions").expect("header renders");
    assert_eq!(
        f32::from(actions.size.width),
        0.0,
        "the first-run header has no actions"
    );
    for marker in [
        "sidebar-nav-dormant-text:all",
        "sidebar-nav-dormant-text:videos",
        "sidebar-nav-dormant-text:favorites",
    ] {
        assert!(
            vcx.debug_bounds(marker).is_some(),
            "{marker}: a dormant row reads as unavailable"
        );
    }
    for absent in [
        "sidebar-nav-recently-added",
        "search-input",
        "search-shortcut",
    ] {
        assert!(
            vcx.debug_bounds(absent).is_none(),
            "{absent}: the dormant sidebar has three rows and no live controls"
        );
    }
    let before = view.update(vcx, |app, _| app.selection.clone());
    click(vcx, "sidebar-nav-all");
    click(vcx, "sidebar-nav-videos");
    view.update(vcx, |app, _| {
        assert!(
            app.grid_state.is_none(),
            "a dormant row does nothing without a library"
        );
        assert_eq!(
            app.selection, before,
            "a dormant row never changes the selection"
        );
    });
    assert!(
        vcx.debug_bounds("welcome-column").is_some(),
        "the welcome body is still the main content"
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
    let (_dir, engine) = seed_library("welcome_progress", 3);
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
}

#[core::prelude::v1::test]
fn enter_opens_the_library_and_cmd_period_stops_the_scan() {
    let (_dir, engine) = seed_library("welcome_keys", 1);
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
}

#[core::prelude::v1::test]
fn only_an_empty_catalog_gets_the_scan_screen() {
    let (populated, _engine) = seed_library("welcome_populated", 2);
    let empty = std::env::temp_dir().join(format!(
        "lumina_uitest_welcome_empty_{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&empty).unwrap();
    let empty = crate::tests::helpers::TempLibrary(empty);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));

    view.update(vcx, |app, cx| {
        crate::tests::helpers::install_library(app, empty.clone(), cx);
        assert!(
            app.first_scan.is_some(),
            "a new library shows the scan screen"
        );
        crate::tests::helpers::install_library(app, populated.clone(), cx);
        assert!(
            app.first_scan.is_none(),
            "a scanned library opens straight away"
        );
    });
}

#[core::prelude::v1::test]
fn a_replaced_librarys_late_scan_complete_is_ignored() {
    let (_dir, engine) = seed_library("welcome_stale", 1);
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
}

// ---- Recent libraries and the missing-library notice ----

use crate::app::command_palette::{CommandId, PaletteTarget};
use crate::app::downloads::DownloadsConfirm;
use crate::app::library::FOLDER_PICKS;
use crate::app::recents::startup_library;
use crate::tests::downloads::{install, queue_app, settle, submit};
use crate::tests::helpers::TempLibrary;
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

/// RECENT LIBRARIES sits under the hint cards, and keeps its keys.
#[core::prelude::v1::test]
fn recents_list_sits_under_the_hints_and_keeps_keys() {
    let (a, b, c) = (
        temp_folder("keys_a"),
        temp_folder("keys_b"),
        temp_folder("keys_c"),
    );
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = welcome_with_recents(
        &mut cx,
        vec![a.to_path_buf(), b.to_path_buf(), c.to_path_buf()],
    );
    let hint = vcx.debug_bounds("welcome-hint-0").unwrap();
    let row = vcx.debug_bounds("welcome-recent-0").unwrap();
    assert!(
        row.top() >= hint.bottom(),
        "recents sit under the hint cards"
    );
    assert_eq!(f32::from(row.size.height), 32.0, "rows are 32 high");

    keystrokes(vcx, "down backspace");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.config.recent_libraries,
            vec![b.to_path_buf(), c.to_path_buf()],
            "⌫ forgets the focused row"
        );
    });
    keystrokes(vcx, "down enter");
    view.update(vcx, |app, _| {
        assert_eq!(
            open_root(app).as_deref(),
            Some(c.as_path()),
            "↓ ↵ opens recent 1"
        );
    });
}

/// A long path is one line cut in the middle (GPUI's
/// `text_ellipsis_middle`), inside its row, and the tooltip and the
/// test marker carry the full path.
#[core::prelude::v1::test]
fn long_recent_path_is_one_line_inside_its_row() {
    let base = temp_folder("long_path");
    let deep = base
        .join("a-very-long-folder-name-that-will-not-fit-".repeat(4))
        .join("Holiday Photos");
    std::fs::create_dir_all(&deep).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (_view, vcx) = welcome_with_recents(&mut cx, vec![deep.clone()]);
    let row = vcx.debug_bounds("welcome-recent-0").unwrap();
    let line = vcx
        .debug_bounds("welcome-recent-path-0")
        .expect("the path line renders");
    assert!(
        line.right() <= row.right() && line.left() >= row.left(),
        "the path stays inside its row: {line:?} in {row:?}"
    );
    assert!(
        f32::from(line.size.height) < 20.0,
        "the path is one line, not wrapped: {line:?}"
    );
    assert!(
        painted_text(vcx, format!("welcome-recent-path-text:{}", deep.display())),
        "the row carries the full path"
    );
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

/// × on a row above the focused one keeps the focus on the same library.
#[core::prelude::v1::test]
fn removing_an_earlier_recent_keeps_the_focused_library() {
    let (a, b, c) = (
        temp_folder("shift_a"),
        temp_folder("shift_b"),
        temp_folder("shift_c"),
    );
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = welcome_with_recents(
        &mut cx,
        vec![a.to_path_buf(), b.to_path_buf(), c.to_path_buf()],
    );
    keystrokes(vcx, "down down");
    click(vcx, "welcome-recent-remove-0");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.config.recent_libraries,
            vec![b.to_path_buf(), c.to_path_buf()],
            "× forgets its row"
        );
        let focused = app
            .welcome
            .focused_recent
            .and_then(|i| app.config.recent_libraries.get(i).cloned());
        assert_eq!(
            focused,
            Some(b.to_path_buf()),
            "the focus stays on the library it was on, not the next one down"
        );
    });
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
    assert!(
        painted_text(vcx, format!("missing-library-path-text:{}", gone.display())),
        "the state names the folder"
    );
    view.update(vcx, |app, _| {
        assert_eq!(app.welcome.missing.as_deref(), Some(gone.as_path()));
        assert!(app.grid_state.is_none());
    });
    let before = picks();
    click(vcx, "missing-library-choose");
    keystrokes(vcx, "cmd-o");
    assert_eq!(picks(), before + 2, "Choose and ⌘O stay usable");
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
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(app.welcome.missing.is_none(), "Esc goes Back")
    });
    assert!(
        vcx.debug_bounds("welcome-recent-1").is_some(),
        "Back shows the recents again"
    );
    keystrokes(vcx, "down enter");
    view.update(vcx, |app, _| {
        assert_eq!(
            open_root(app).as_deref(),
            Some(here.as_path()),
            "the available recent is reachable after Back"
        )
    });
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
    let current = dir;
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
    });
    // The folder is checked off the UI thread before the question.
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
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
    let current = dir;
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
    });
    // The open fails in the background.
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        assert!(app.grid_state.is_none(), "a file is not a library");
        assert!(
            !app.welcome.opening(),
            "the welcome comes back once the open has failed"
        );
        assert_eq!(
            app.config.last_library_path, None,
            "a failed open must not become the startup library"
        );
        assert!(app.config.recent_libraries.is_empty(), "nor a recent");
        assert!(app.error_message.is_some(), "the banner says it failed");
    });
}

// ---- Menu actions and the key gate on the welcome screen ----

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

#[cfg(target_os = "linux")]
#[core::prelude::v1::test]
fn linux_ctrl_q_quits_from_the_welcome_screen() {
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = welcome_with_recents(&mut cx, Vec::new());
    focus_app(vcx, &view);

    keystrokes(vcx, "ctrl-q");

    view.update(vcx, |app, _| {
        assert!(
            app.downloads.quit_requested,
            "Ctrl+Q follows the normal quit path"
        );
    });
}

/// Regression: Ctrl+Q was checked after the dialog key handler, so an open
/// dialog swallowed it.
#[cfg(target_os = "linux")]
#[core::prelude::v1::test]
fn linux_ctrl_q_quits_while_a_dialog_is_open() {
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = welcome_with_recents(&mut cx, Vec::new());
    view.update(vcx, |app, cx| {
        app.shortcuts_overlay_open = true;
        cx.notify();
    });
    render_ui(vcx);
    focus_app(vcx, &view);

    keystrokes(vcx, "ctrl-q");

    view.update(vcx, |app, _| {
        assert!(
            app.downloads.quit_requested,
            "an open dialog does not swallow Ctrl+Q"
        );
    });
}

#[core::prelude::v1::test]
fn first_scan_quit_reaches_the_app() {
    let (_dir, engine) = seed_library("first_scan_quit", 1);
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
}

#[core::prelude::v1::test]
fn welcome_preferences_paint_and_escape_closes() {
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = welcome_with_recents(&mut cx, Vec::new());
    vcx.simulate_resize(gpui::size(gpui::px(1024.0), gpui::px(560.0)));

    vcx.dispatch_action(crate::actions::OpenPreferences);
    render_ui(vcx);
    let page = vcx
        .debug_bounds("settings-page")
        .expect("settings paint on the welcome screen when opened");
    assert_eq!(
        (f32::from(page.size.width), f32::from(page.size.height)),
        (1024.0, 560.0),
        "with no library, settings fill the window"
    );
    let sidebar = vcx
        .debug_bounds("settings-sidebar")
        .expect("settings bring their own category sidebar");
    assert_eq!(
        f32::from(sidebar.size.height),
        560.0,
        "the category sidebar runs the window's full height"
    );
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(!app.preferences_open, "Esc closes Preferences")
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("settings-page").is_none(),
        "Esc returns to the welcome screen"
    );

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

// ---- Welcome paths: open, import into a new library, Pictures ----

fn new_library_picks() -> usize {
    crate::app::import::NEW_LIBRARY_PICKS.with(|n| n.get())
}

fn import_picks() -> usize {
    crate::app::import::IMPORT_PICKS.with(|n| n.get())
}

/// Runs `then` once, in its own update right after the update in which a
/// library open lands (`grid_state` set), so no other task runs between.
fn on_open_landed(
    view: &gpui::Entity<LuminaApp>,
    vcx: &mut gpui::VisualTestContext,
    then: impl FnOnce(&mut LuminaApp, &mut gpui::Context<LuminaApp>) + 'static,
) -> gpui::Subscription {
    let mut then = Some(then);
    vcx.update(|_, cx| {
        cx.observe(view, move |view, cx| {
            if view.read(cx).grid_state.is_some() {
                if let Some(then) = then.take() {
                    view.update(cx, then);
                }
            }
        })
    })
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
    // What the app holds the moment the open lands: an observer runs right
    // after that update, before the real scan's ScanComplete can be read.
    let landed = std::rc::Rc::new(std::cell::Cell::new(None));
    let _watch = on_open_landed(&view, vcx, {
        let landed = landed.clone();
        move |app, _| landed.set(Some((app.import.pending_import_after_open, import_picks())))
    });
    view.update(vcx, |app, cx| {
        app.handle_ui_message(UiMessage::NewLibraryPicked(folder.to_path_buf()), cx);
        assert!(
            app.grid_state.is_none() && !app.import.pending_import_after_open,
            "the engine opens in the background; nothing is asked before it has opened"
        );
    });
    vcx.run_until_parked();
    assert_eq!(
        landed.get(),
        Some((true, before)),
        "once the open lands the import waits for the first scan: nothing is asked yet"
    );
    // The real scan's own ScanComplete may already have arrived, so the
    // import is either still waiting for it or asked exactly once.
    view.update(vcx, |app, _| {
        assert!(
            app.import.pending_import_after_open != (import_picks() == before + 1),
            "the import waits for the first scan, or that scan has asked once"
        );
        assert_eq!(
            open_root(app).as_deref(),
            Some(folder.as_ref()),
            "the chosen folder opens as the new library"
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
    });
    // The open fails in the background.
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
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
    // The user rescans as the open lands, before the real first scan's
    // ScanComplete is read (as if that scan had failed and sent none).
    let rescanned = std::rc::Rc::new(std::cell::Cell::new(None));
    let _watch = on_open_landed(&view, vcx, {
        let rescanned = rescanned.clone();
        move |app, cx| {
            let pending = app.import.pending_import_after_open;
            app.rescan_library(cx);
            rescanned.set(Some((pending, app.import.pending_import_after_open)));
        }
    });
    view.update(vcx, |app, cx| {
        app.handle_ui_message(UiMessage::NewLibraryPicked(folder.to_path_buf()), cx);
    });
    vcx.run_until_parked();
    assert_eq!(
        rescanned.get(),
        Some((true, false)),
        "the open set the pending import; the user's own rescan cancels it"
    );
    feed(&view, vcx, EngineEvent::ScanComplete { total_items: 0 });
    assert_eq!(
        import_picks(),
        before,
        "a rescan's ScanComplete must not open the import picker out of nowhere"
    );
}

// ---- Explore a Sample Library ----

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

/// While the sample is being written the link is inert.
#[core::prelude::v1::test]
fn sample_link_is_inert_while_preparing() {
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = welcome_with_recents(&mut cx, Vec::new());
    view.update(vcx, |app, cx| {
        app.welcome.generating_sample = true;
        cx.notify();
    });
    render_ui(vcx);
    let before = sample_opens();
    click(vcx, "welcome-sample-btn");
    assert_eq!(
        sample_opens(),
        before,
        "a click on Preparing the Sample Library… asks for nothing"
    );
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

// ---- Library unreachable state ----

/// Clicks `selector` without letting background work run, so a test can see
/// what the UI shows while that work is still pending.
fn click_unparked(vcx: &mut VisualTestContext, selector: &'static str) {
    let position = crate::tests::helpers::bounds_center(vcx, selector);
    vcx.update(|window, cx| {
        let (modifiers, button) = (gpui::Modifiers::default(), gpui::MouseButton::Left);
        window.dispatch_event(
            gpui::PlatformInput::MouseDown(gpui::MouseDownEvent {
                position,
                modifiers,
                button,
                click_count: 1,
                first_mouse: false,
            }),
            cx,
        );
        window.dispatch_event(
            gpui::PlatformInput::MouseUp(gpui::MouseUpEvent {
                position,
                modifiers,
                button,
                click_count: 1,
            }),
            cx,
        );
        window.refresh();
        window.draw(cx).clear(cx);
    });
}

/// The welcome screen after startup found `last_library_path` missing.
fn missing_at_startup<'a>(
    cx: &'a mut gpui::TestAppContext,
    gone: &std::path::Path,
) -> (Entity<LuminaApp>, &'a mut VisualTestContext) {
    let (view, vcx) = welcome_with_recents(cx, Vec::new());
    view.update(vcx, |app, cx| {
        app.config.last_library_path = Some(gone.to_path_buf());
        let startup = startup_library(&app.config);
        app.apply_startup_library(startup, cx);
    });
    render_ui(vcx);
    (view, vcx)
}

fn painted_text(vcx: &mut VisualTestContext, selector: String) -> bool {
    vcx.debug_bounds(crate::tests::helpers::leak(selector))
        .is_some()
}

#[core::prelude::v1::test]
fn missing_library_state_retries_and_chooses() {
    // Removed now, made again before Retry; the guard deletes it at the end.
    let library = temp_folder("unreachable");
    std::fs::remove_dir(library.as_path()).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = missing_at_startup(&mut cx, &library);

    for selector in [
        "welcome-missing-library",
        "missing-library-headline",
        "missing-library-path",
        "missing-library-retry",
        "missing-library-choose",
    ] {
        assert!(
            vcx.debug_bounds(selector).is_some(),
            "{selector}: the unreachable state shows its parts"
        );
    }
    assert!(
        painted_text(
            vcx,
            "missing-library-headline-text:Can't reach your library".into()
        ),
        "the headline says what is wrong"
    );
    assert!(
        painted_text(
            vcx,
            format!("missing-library-path-text:{}", library.display())
        ),
        "the pill shows the library's path"
    );
    // GPUI's test window builds no accessibility tree; a test-only marker
    // sits beside the `.role(Role::Alert)` call instead.
    assert!(
        painted_text(vcx, "welcome-missing-library-role:alert".into()),
        "the state is announced as an alert (Role::Alert)"
    );

    let before = picks();
    click(vcx, "missing-library-choose");
    assert_eq!(
        picks(),
        before + 1,
        "Choose another library… picks a folder"
    );

    std::fs::create_dir_all(library.as_path()).unwrap();
    click_unparked(vcx, "missing-library-retry");
    view.update(vcx, |app, _| {
        assert!(
            app.grid_state.is_none(),
            "Retry must not open the library on the UI thread"
        );
        assert!(app.welcome.retrying, "Retry is checking in the background");
    });
    assert!(
        painted_text(vcx, "missing-library-retry-text:Checking…".into()),
        "the Retry button says it is checking"
    );
    vcx.run_until_parked();
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            open_root(app).as_deref(),
            Some(library.as_path()),
            "Retry opens the library once it is back"
        );
        assert!(app.welcome.missing.is_none(), "the state is gone");
        assert!(!app.welcome.retrying, "the checking state is cleared");
    });
}

#[core::prelude::v1::test]
fn missing_library_retry_that_fails_keeps_the_state() {
    let gone = temp_folder("still_gone").to_path_buf(); // deleted at once
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = missing_at_startup(&mut cx, &gone);

    click(vcx, "missing-library-retry");
    view.update(vcx, |app, _| {
        assert!(app.grid_state.is_none(), "a missing folder is not opened");
        assert_eq!(
            app.welcome.missing.as_deref(),
            Some(gone.as_path()),
            "the state stays for the same library"
        );
        assert!(!app.welcome.retrying, "the checking state is cleared");
        assert!(
            app.welcome.missing_error.is_none(),
            "a folder that is not there has no error string to show"
        );
    });
    assert!(
        painted_text(vcx, "missing-library-retry-text:Retry".into()),
        "Retry is offered again"
    );
    assert!(
        vcx.debug_bounds("missing-library-error").is_none(),
        "no details line without a real error"
    );
    assert!(!gone.exists(), "Retry creates nothing");
}

#[core::prelude::v1::test]
fn missing_library_retry_open_error_shows_details() {
    // A file where the folder should be: `is_dir` fails the same way, so
    // make it a folder whose catalog cannot be opened instead.
    let library = temp_folder("bad_catalog");
    std::fs::create_dir_all(library.join("catalog.db")).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = welcome_with_recents(&mut cx, Vec::new());
    view.update(vcx, |app, cx| {
        app.welcome.missing = Some(library.to_path_buf());
        cx.notify();
    });
    render_ui(vcx);
    click(vcx, "missing-library-retry");
    view.update(vcx, |app, _| {
        assert!(app.grid_state.is_none(), "a failed open opens nothing");
        assert!(
            app.welcome.missing_error.is_some(),
            "the open error is kept to show"
        );
        assert!(
            app.error_message.is_none(),
            "the state shows the failure; no second banner"
        );
    });
    assert!(
        vcx.debug_bounds("missing-library-error").is_some(),
        "the real error string is shown under the state"
    );
}

#[core::prelude::v1::test]
fn missing_library_keys_retry_and_choose() {
    let gone = temp_folder("keys_gone").to_path_buf();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = missing_at_startup(&mut cx, &gone);
    let before = picks();
    keystrokes(vcx, "tab tab enter");
    assert_eq!(picks(), before + 1, "Tab reaches Choose and ↵ presses it");
    keystrokes(vcx, "tab enter");
    view.update(vcx, |app, _| {
        assert_eq!(app.welcome.missing_focus, Some(0), "Tab wraps to Retry");
        assert!(app.grid_state.is_none());
    });
    assert_eq!(picks(), before + 1, "↵ on Retry picks no folder");
    keystrokes(vcx, "cmd-o");
    assert_eq!(picks(), before + 2, "⌘O still chooses a library");
}

#[core::prelude::v1::test]
fn missing_library_long_path_stays_in_the_column() {
    let gone = std::env::temp_dir()
        .join("lumina_uitest_never_created")
        .join("a-very-long-folder-name-that-will-not-fit-".repeat(6))
        .join("Lumina.library");
    let mut cx = crate::tests::TestApp::single();
    let (_view, vcx) = missing_at_startup(&mut cx, &gone);
    let state = vcx.debug_bounds("welcome-missing-library").unwrap();
    let pill = vcx.debug_bounds("missing-library-path").unwrap();
    assert!(
        f32::from(state.size.width) <= 450.0,
        "the state column is at most 450 wide"
    );
    assert!(
        pill.size.width <= state.size.width && pill.origin.x >= state.origin.x,
        "a long path is truncated inside the pill, not widening it: {pill:?} in {state:?}"
    );
    assert_eq!(
        f32::from(pill.size.height),
        30.0,
        "the pill stays one 30 px line"
    );
    let line = vcx.debug_bounds("missing-library-path-line").unwrap();
    assert!(
        line.right() <= pill.right() && f32::from(line.size.height) < 20.0,
        "the path is one line cut in the middle, not wrapped or spilling: {line:?} in {pill:?}"
    );
    let retry = vcx.debug_bounds("missing-library-retry").unwrap();
    let choose = vcx.debug_bounds("missing-library-choose").unwrap();
    assert_eq!(
        retry.origin.y, choose.origin.y,
        "the buttons stay on one row"
    );
}

#[core::prelude::v1::test]
fn missing_library_stale_retry_never_replaces_an_opened_library() {
    let library = temp_folder("stale_retry");
    let other = temp_folder("stale_other");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = welcome_with_recents(&mut cx, Vec::new());
    view.update(vcx, |app, cx| {
        // The folder is back, but the state still shows it as missing.
        app.welcome.missing = Some(library.to_path_buf());
        cx.notify();
    });
    render_ui(vcx);
    click_unparked(vcx, "missing-library-retry");
    view.update(vcx, |app, cx| {
        assert!(app.welcome.retrying, "control: Retry is in flight");
        // The user chooses another library before Retry answers.
        app.open_library(other.to_path_buf(), cx);
    });
    vcx.run_until_parked();
    render_ui(vcx);
    // The engine exposes no scan-cancelled state, so the cancel of the
    // stale engine is not observable here; the open library is.
    view.update(vcx, |app, _| {
        assert_eq!(
            open_root(app).as_deref(),
            Some(other.as_path()),
            "a late Retry result must not replace the library the user opened"
        );
        assert!(!app.welcome.retrying, "no checking state is left behind");
    });
}

#[core::prelude::v1::test]
fn missing_library_back_returns_to_welcome_and_drops_retry() {
    let library = temp_folder("back_retry");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = welcome_with_recents(&mut cx, vec![library.to_path_buf()]);
    view.update(vcx, |app, cx| {
        app.welcome.missing = Some(library.to_path_buf());
        app.has_desktop_pics = true;
        cx.notify();
    });
    render_ui(vcx);
    let (pictures, samples, imports) = (picture_opens(), sample_opens(), new_library_picks());
    keystrokes(vcx, "shift-enter alt-enter cmd-shift-i");
    assert_eq!(
        picture_opens(),
        pictures,
        "hidden Pictures path takes no ⇧↵"
    );
    assert_eq!(sample_opens(), samples, "hidden sample path takes no ⌥↵");
    assert_eq!(
        new_library_picks(),
        imports,
        "hidden import path takes no ⇧⌘I"
    );
    // Esc dismisses an error banner before it leaves the state.
    view.update(vcx, |app, cx| app.set_error("Could not open folder", cx));
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(
            app.error_message.is_none(),
            "the first Esc dismisses the banner"
        );
        assert!(app.welcome.missing.is_some(), "and stays in the state");
    });
    click_unparked(vcx, "missing-library-retry");
    click_unparked(vcx, "missing-library-back");
    vcx.run_until_parked();
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.welcome.missing.is_none(), "Back leaves the state");
        assert!(!app.welcome.retrying, "Back clears the checking state");
        assert!(
            app.grid_state.is_none(),
            "a Retry dropped by Back must not open the library afterwards"
        );
    });
    for selector in ["welcome-open-btn", "welcome-recent-0"] {
        assert!(
            vcx.debug_bounds(selector).is_some(),
            "{selector}: Back shows the normal welcome screen"
        );
    }
}

#[core::prelude::v1::test]
fn missing_library_retry_dropped_by_back_stays_dropped_on_return() {
    let library = temp_folder("back_again");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = welcome_with_recents(&mut cx, Vec::new());
    view.update(vcx, |app, cx| {
        app.welcome.set_missing(Some(library.to_path_buf()));
        cx.notify();
    });
    render_ui(vcx);
    click_unparked(vcx, "missing-library-retry");
    click_unparked(vcx, "missing-library-back");
    // The same library shows as missing again before the old Retry answers.
    view.update(vcx, |app, cx| {
        app.welcome.set_missing(Some(library.to_path_buf()));
        cx.notify();
    });
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        assert!(
            app.grid_state.is_none(),
            "a Retry the user left with Back must not open the library later"
        );
        assert!(!app.welcome.retrying, "and shows no checking state");
    });
}

#[core::prelude::v1::test]
fn recent_row_checks_its_folder_off_the_ui_thread() {
    let gone = temp_folder("recent_bg_gone").to_path_buf();
    let here = temp_folder("recent_bg_here");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = welcome_with_recents(&mut cx, vec![gone.clone(), here.to_path_buf()]);

    click_unparked(vcx, "welcome-recent-0");
    view.update(vcx, |app, _| {
        assert!(
            app.welcome.missing.is_none(),
            "the folder check runs off the UI thread, not inside the click"
        );
    });
    vcx.run_until_parked();
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.grid_state.is_none(), "a gone folder is not opened");
        assert_eq!(
            app.welcome.missing.as_deref(),
            Some(gone.as_path()),
            "a gone folder still shows the missing-library notice"
        );
    });
    assert!(vcx.debug_bounds("welcome-missing-library").is_some());

    keystrokes(vcx, "escape");
    click_unparked(vcx, "welcome-recent-1");
    view.update(vcx, |app, _| {
        assert!(app.grid_state.is_none(), "control: not opened in the click");
    });
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        assert_eq!(
            open_root(app).as_deref(),
            Some(here.as_path()),
            "a folder that is there opens once the check answers"
        );
    });
}

#[core::prelude::v1::test]
fn late_recent_folder_check_is_dropped() {
    let here = temp_folder("recent_late_here");
    let other = temp_folder("recent_late_other");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = welcome_with_recents(&mut cx, vec![here.to_path_buf()]);

    // Back before the answer: the late answer opens nothing.
    click_unparked(vcx, "welcome-recent-0");
    view.update(vcx, |app, cx| app.leave_missing_library(cx));
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        assert!(
            app.grid_state.is_none(),
            "a folder check answered after Back must not open the library"
        );
    });

    // Another library opened before the answer: it stays open.
    render_ui(vcx);
    click_unparked(vcx, "welcome-recent-0");
    view.update(vcx, |app, cx| app.open_library(other.to_path_buf(), cx));
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        assert_eq!(
            open_root(app).as_deref(),
            Some(other.as_path()),
            "a late folder check must not replace the library the user opened"
        );
    });
}

/// Lumina › Preferences… toggles like ⌘, on the welcome screen too.
#[core::prelude::v1::test]
fn welcome_preferences_menu_item_toggles_settings() {
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = welcome_with_recents(&mut cx, Vec::new());
    vcx.dispatch_action(crate::actions::OpenPreferences);
    render_ui(vcx);
    assert!(view.update(vcx, |app, _| app.preferences_open), "it opens");
    vcx.dispatch_action(crate::actions::OpenPreferences);
    render_ui(vcx);
    assert!(
        !view.update(vcx, |app, _| app.preferences_open),
        "Preferences… while settings is open closes it, as ⌘, does"
    );
}

// ---- Ten recents and Reset to defaults with no library open ----

/// The welcome screen with `config` as the startup config (never saved).
fn welcome_with_config(
    cx: &mut gpui::TestAppContext,
    config: crate::config::AppConfig,
) -> (Entity<LuminaApp>, &mut VisualTestContext) {
    crate::app::state::TEST_CONFIG.with(|c| *c.borrow_mut() = Some(config));
    let (view, vcx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    render_ui(vcx);
    focus_app(vcx, &view);
    (view, vcx)
}

/// Ten recents in a 900×600 window: each row is one line, the welcome sits
/// under the header, and the body scrolls the tenth fully into view.
#[core::prelude::v1::test]
fn ten_recents_are_one_line_rows_and_the_tenth_scrolls_into_view() {
    let folders: Vec<TempLibrary> = (0..10).map(|i| temp_folder(&format!("ten_{i}"))).collect();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = welcome_with_config(
        &mut cx,
        crate::config::AppConfig {
            recent_libraries: folders.iter().map(|f| f.to_path_buf()).collect(),
            ..Default::default()
        },
    );
    vcx.simulate_resize(gpui::size(gpui::px(900.0), gpui::px(600.0)));
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.grid_state.is_none(), "precondition: no library is open")
    });

    for i in 0..10 {
        let row = vcx
            .debug_bounds(leak(format!("welcome-recent-{i}")))
            .unwrap_or_else(|| panic!("recent {i} is listed: all ten recents show"));
        let line = vcx
            .debug_bounds(leak(format!("welcome-recent-path-{i}")))
            .unwrap_or_else(|| panic!("recent {i} has a path line"));
        assert_eq!(
            f32::from(row.size.height),
            32.0,
            "recent {i} must be one 32 px row"
        );
        assert!(
            f32::from(line.size.height) < 20.0
                && line.left() >= row.left()
                && line.right() <= row.right(),
            "recent {i}'s path must be one line inside its row: {line:?} in {row:?}"
        );
    }

    let header = vcx.debug_bounds("page-header").expect("header renders");
    let open = vcx
        .debug_bounds("welcome-open-btn")
        .expect("buttons render");
    assert!(
        open.top() >= header.bottom(),
        "the welcome buttons never overlap the header"
    );
    let tenth = vcx.debug_bounds("welcome-recent-9").unwrap();
    assert!(
        tenth.top() > gpui::px(600.0),
        "precondition: at 900×600 the tenth recent starts below the window: {tenth:?}"
    );
    let body = vcx.debug_bounds("welcome-body").expect("body renders");
    for _ in 0..8 {
        wheel(
            vcx,
            body.center(),
            gpui::ScrollDelta::Pixels(gpui::point(gpui::px(0.0), gpui::px(-400.0))),
            gpui::Modifiers::default(),
        );
    }
    let tenth = vcx.debug_bounds("welcome-recent-9").unwrap();
    assert!(
        tenth.top() >= header.bottom() && tenth.bottom() <= gpui::px(600.0),
        "scrolling must bring the tenth recent fully into the window: {tenth:?}"
    );
}

/// ↑/↓ keep the focused recent inside the scrolling welcome body (nearest
/// edge), in a short window with ten recents.
#[core::prelude::v1::test]
fn arrow_focus_scrolls_the_recent_into_view() {
    let folders: Vec<TempLibrary> = (0..10)
        .map(|i| temp_folder(&format!("arrow_{i}")))
        .collect();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = welcome_with_config(
        &mut cx,
        crate::config::AppConfig {
            recent_libraries: folders.iter().map(|f| f.to_path_buf()).collect(),
            ..Default::default()
        },
    );
    vcx.simulate_resize(gpui::size(gpui::px(960.0), gpui::px(600.0)));
    render_ui(vcx);
    let inside = |vcx: &mut VisualTestContext, i: usize| {
        let body = vcx.debug_bounds("welcome-body").expect("body renders");
        let row = vcx
            .debug_bounds(leak(format!("welcome-recent-{i}")))
            .unwrap_or_else(|| panic!("recent {i} renders"));
        (
            row.top() >= body.top() && row.bottom() <= body.bottom(),
            row,
            body,
        )
    };
    assert!(
        !inside(vcx, 9).0,
        "precondition: at 960×600 the tenth recent starts outside the body"
    );
    for i in 0..10 {
        keystrokes(vcx, "down");
        let (shown, row, body) = inside(vcx, i);
        assert!(
            shown,
            "↓ to recent {i} must scroll it into the body: {row:?} in {body:?}"
        );
    }
    view.update(vcx, |app, _| {
        assert_eq!(app.welcome.focused_recent, Some(9))
    });
    for i in (0..9).rev() {
        keystrokes(vcx, "up");
        let (shown, row, body) = inside(vcx, i);
        assert!(
            shown,
            "↑ to recent {i} must scroll it into the body: {row:?} in {body:?}"
        );
    }
}

/// Reset to defaults from the welcome screen asks first, then
/// resets the UI preferences only, like the library case
/// (`tests::preferences::reset_to_defaults_asks_then_resets_ui_preferences_only`).
#[core::prelude::v1::test]
fn reset_to_defaults_from_welcome_resets_ui_preferences_only() {
    use crate::config::{AlbumSort, AppConfig, BrowseLayout, GroupLevel, LaunchView};
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = welcome_with_config(
        &mut cx,
        AppConfig {
            sidebar_width: 320.0,
            inspector_width: 360.0,
            sidebar_visible: false,
            inspector_visible: true,
            grid_columns: 6,
            card_fit_mode: false,
            sort_field: Some("name".into()),
            sort_direction: Some("asc".into()),
            browse_layout: BrowseLayout::Mosaic,
            group_level: GroupLevel::Months,
            album_sort: AlbumSort::Created,
            album_panel_hidden: true,
            launch_view: LaunchView::Library,
            recent_libraries: vec!["/nonexistent/lumina-welcome-reset/Library".into()],
            external_editor: Some("com.example.editor".into()),
            slideshow_interval_secs: 10,
            video_speed: Some(1.5),
            ..AppConfig::default()
        },
    );
    vcx.simulate_resize(gpui::size(gpui::px(1280.0), gpui::px(800.0)));
    // The config the next save writes from (the live layout is checked below).
    let snapshot = |view: &Entity<LuminaApp>, vcx: &mut VisualTestContext| {
        view.update(vcx, |app, _| app.config.clone())
    };
    let before = snapshot(&view, vcx);
    view.update(vcx, |app, _| {
        assert!(app.grid_state.is_none(), "precondition: no library is open")
    });
    keystrokes(vcx, "secondary-,");
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("settings-page").is_some(),
        "precondition: settings paint on the welcome screen"
    );

    click(vcx, "settings-reset");
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("settings-reset-dialog").is_some(),
        "Reset asks first on the welcome screen too"
    );
    click(vcx, "settings-reset-cancel");
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("settings-reset-dialog").is_none(),
        "Cancel closes the confirmation"
    );
    assert_eq!(
        serde_json::to_value(snapshot(&view, vcx)).unwrap(),
        serde_json::to_value(&before).unwrap(),
        "Cancel changes nothing"
    );

    let saves = crate::app::state::SAVE_COUNT.with(|c| c.get());
    click(vcx, "settings-reset");
    click(vcx, "settings-reset-confirm");
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("settings-reset-dialog").is_none(),
        "Reset closes the confirmation"
    );
    assert!(
        vcx.debug_bounds("settings-page").is_some(),
        "settings stay open after the reset"
    );
    assert!(
        crate::app::state::SAVE_COUNT.with(|c| c.get()) > saves,
        "the reset is saved (a test-only no-op save, AGENTS §2)"
    );

    let mut expected = before.clone();
    expected.reset_ui_preferences();
    assert_eq!(
        serde_json::to_value(snapshot(&view, vcx)).unwrap(),
        serde_json::to_value(&expected).unwrap(),
        "Reset puts the UI preferences back to the defaults and keeps every other value"
    );
    let d = AppConfig::default();
    view.update(vcx, |app, _| {
        assert_eq!(
            (
                app.sidebar_collapsed,
                app.inspector_open,
                app.sidebar_width,
                app.inspector_width,
                app.card_fit_mode,
            ),
            (
                !d.sidebar_visible,
                d.inspector_visible,
                d.sidebar_width,
                d.inspector_width,
                d.card_fit_mode,
            ),
            "the live layout follows the reset"
        );
        assert_eq!(
            app.sort,
            d.restored_sort(),
            "the live sort follows the reset"
        );
        assert!(app.error_message.is_none(), "a good save shows no error");
        assert_eq!(
            app.last_action.as_deref(),
            Some("Settings reset to defaults"),
            "the reset is reported as in the library case"
        );
    });
    let view_menu: Vec<String> = vcx.update(|_, cx| {
        cx.get_menus()
            .unwrap_or_default()
            .into_iter()
            .filter(|m| m.name.as_ref() == "View")
            .flat_map(|m| m.items)
            .filter_map(|item| match item {
                gpui::OwnedMenuItem::Action { name, .. } => Some(name.to_string()),
                _ => None,
            })
            .collect()
    });
    assert!(
        view_menu.iter().any(|n| n == "Hide Sidebar"),
        "the View menu follows the reset (sidebar shown): {view_menu:?}"
    );
}

#[core::prelude::v1::test]
fn large_folder_open_reports_counting_opening_and_status_progress() {
    // 1. Opening a folder from the welcome screen renders an opening indicator,
    //    not a blank void.
    {
        let mut cx = crate::tests::TestApp::single();
        let (view, vcx) = welcome_with_recents(&mut cx, Vec::new());
        view.update(vcx, |app, cx| {
            app.welcome.retry_generation = 1;
            app.welcome.opening = Some((1, PathBuf::from("/tmp/Big Photos")));
            cx.notify();
        });
        render_ui(vcx);
        assert!(
            vcx.debug_bounds("welcome-opening").is_some(),
            "welcome screen must show what folder is opening while open_engine runs"
        );
    }

    // 2. First scan: ScanCounting updates the counting readout, reading_started
    //    anchors ETA after counting, and is_complete shows Finishing catalog.
    let (_dir, engine) = seed_library("welcome_large_scan", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.first_scan = Some(FirstScanState::new());
        app.grid_state.as_mut().unwrap().is_scanning = true;
        cx.notify();
    });
    feed(&view, vcx, EngineEvent::ScanCounting { files_found: 1200 });
    view.update(vcx, |app, _| {
        let scan = app.first_scan.as_ref().unwrap();
        assert_eq!(scan.counting_found, 1200);
        assert_eq!(
            scan.progress_line(scan.started),
            "Counting files… 1,200 found"
        );
    });
    feed(&view, vcx, progress(0, 1200, 0, "/x/a.jpg", false));
    view.update(vcx, |app, _| {
        let scan = app.first_scan.as_ref().unwrap();
        assert!(
            scan.reading_started.is_some(),
            "reading_started records when reading begins"
        );
    });
    feed(&view, vcx, progress(1200, 1200, 0, "/x/z.jpg", true));
    view.update(vcx, |app, _| {
        let scan = app.first_scan.as_ref().unwrap();
        assert!(scan.finishing);
        assert_eq!(scan.progress_line(scan.started), "Finishing catalog…");
        assert!(
            app.grid_state.as_ref().unwrap().is_scanning,
            "is_scanning stays true until ScanComplete finishes catalog work"
        );
    });

    // 3. Dismissing first_scan keeps total + progress bar + Stop in status bar.
    view.update(vcx, |app, cx| app.dismiss_first_scan(cx));
    render_ui(vcx);
    assert!(vcx.debug_bounds("scan-progress").is_some());
    assert!(vcx.debug_bounds("scan-stop-running").is_some());
    view.update(vcx, |app, _| {
        assert_eq!(app.status_right_text(), "Scanning… 1,200 of 1,200");
        app.welcome.retry_generation = 2;
        app.welcome.opening = Some((2, PathBuf::from("/tmp/Other Large Folder")));
        assert_eq!(app.status_right_text(), "Opening Other Large Folder…");
    });
}
