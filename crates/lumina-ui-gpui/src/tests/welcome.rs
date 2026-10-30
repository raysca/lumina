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
    let mut cx = gpui::TestAppContext::single();
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
    let choose = vcx.debug_bounds("pick-folder-btn").unwrap();
    let pictures = vcx.debug_bounds("pick-desktop-pics-btn").unwrap();
    assert_eq!(choose.size.width, pictures.size.width, "two equal buttons");
    assert_eq!(choose.origin.y, pictures.origin.y, "side by side");
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
    let mut cx = gpui::TestAppContext::single();
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
    let mut cx = gpui::TestAppContext::single();
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
    let mut cx = gpui::TestAppContext::single();
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
    let mut cx = gpui::TestAppContext::single();
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
