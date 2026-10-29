//! UI automation for the bottom-right toast: rendering, undo, dismissal, and
//! stale-timer handling.
//!
//! Helpers mirror `tests/ui_automation.rs` so this file stays self-contained and
//! can reuse the same deterministic rendering pattern without editing it.

use crate::*;
use lumina_core::{EngineEvent, LuminaEngine};
use std::time::Duration;

fn seed_library(prefix: &str, count: usize) -> (std::path::PathBuf, std::sync::Arc<LuminaEngine>) {
    let temp_dir = std::env::temp_dir().join(format!(
        "lumina_toast_{}_{}",
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
            b"test-jpeg-content",
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

/// Draws the registered root view so layout, hit-testing and debug_bounds are
/// populated for the current app state.
fn render_ui(cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear();
    });
    cx.run_until_parked();
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
    (view, vcx)
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

#[core::prelude::v1::test]
fn toast_renders_with_message_and_dismiss_control() {
    let (temp_dir, engine) = seed_library("render", 4);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, cx| app.show_toast("Exported 4 item(s)", cx));
    render_ui(vcx);

    assert!(vcx.debug_bounds("toast").is_some(), "toast must be painted");
    assert!(
        vcx.debug_bounds("toast-message").is_some(),
        "toast must render its message"
    );
    assert!(
        vcx.debug_bounds("toast-dismiss").is_some(),
        "toast must expose a dismiss control"
    );
    assert!(
        vcx.debug_bounds("toast-undo").is_none(),
        "a non-reversible action must not offer Undo"
    );
    view.update(vcx, |app, _| {
        let toast = app.toast.current().expect("toast state must be visible");
        assert_eq!(toast.message, "Exported 4 item(s)");
        assert!(!toast.undo_available);
    });

    click(vcx, "toast-dismiss");
    view.update(vcx, |app, _| {
        assert!(
            app.toast.current().is_none(),
            "dismiss must clear the toast"
        );
    });
    render_ui(vcx);

    // GPUI 0.2.2 retains historical debug_bounds entries across frames, so a
    // dismissed element cannot be asserted absent; assert the channel instead:
    // a later toast renders with only its own message.
    view.update(vcx, |app, cx| app.show_toast("Second action", cx));
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.toast.current().map(|toast| toast.message.as_str()),
            Some("Second action"),
            "a new toast must replace the dismissed one"
        );
    });
    assert!(vcx.debug_bounds("toast").is_some());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn toast_undo_invokes_undo_last() {
    let (temp_dir, engine) = seed_library("undo", 3);
    let item = engine.get_item_slice(0..1)[0].clone();
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, cx| {
        app.selected_item = Some(item.clone());
        app.selected_indices.insert(0);
        app.batch_add_tag("travel".to_string(), cx);
        // Reversible actions call this immediately after pushing their undo entry.
        app.show_undo_toast("Added tag \"travel\"", cx);
    });
    render_ui(vcx);

    assert!(
        vcx.debug_bounds("toast-undo").is_some(),
        "a toast for an undoable action must offer Undo"
    );
    view.update(vcx, |app, _| {
        assert!(
            app.selected_item_tags.contains(&"travel".to_string()),
            "tag should be applied before Undo"
        );
    });

    click(vcx, "toast-undo");
    view.update(vcx, |app, _| {
        assert!(
            !app.selected_item_tags.contains(&"travel".to_string()),
            "toast Undo must run undo_last and restore the previous tags"
        );
        let toast = app
            .toast
            .current()
            .expect("undo should confirm with a fresh toast");
        assert!(
            toast.message.starts_with("Undid"),
            "got {:?}",
            toast.message
        );
        assert!(
            !toast.undo_available,
            "an undo does not push another undo entry"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn toast_auto_dismisses_and_a_replaced_timer_is_stale() {
    let (temp_dir, engine) = seed_library("timer", 2);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, cx| app.show_toast("First", cx));
    render_ui(vcx);
    // Advance most of the way through the first toast's lifetime.
    vcx.executor().advance_clock(Duration::from_secs(3));
    view.update(vcx, |app, cx| app.show_toast("Second", cx));
    render_ui(vcx);

    // The first toast's timer fires here; it must not clear its replacement.
    vcx.executor().advance_clock(Duration::from_secs(2));
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.toast.current().map(|toast| toast.message.as_str()),
            Some("Second"),
            "a stale timer must not dismiss a newer toast"
        );
    });
    assert!(
        vcx.debug_bounds("toast").is_some(),
        "replacement toast must stay painted"
    );

    // The replacement then expires on its own clock.
    vcx.executor()
        .advance_clock(crate::views::toast::TOAST_DURATION);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(
            app.toast.current().is_none(),
            "toast must auto-dismiss after its timeout"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn toast_export_and_import_success_are_hooked() {
    let (temp_dir, engine) = seed_library("actions", 3);
    let item = engine.get_item_slice(0..1)[0].clone();
    let destination = temp_dir.join("exported");
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, cx| {
        app.selected_item = Some(item.clone());
        app.export_to_folder(destination.clone(), cx);
    });
    render_ui(vcx);
    view.update(vcx, |app, _| {
        let toast = app
            .toast
            .current()
            .expect("export must confirm with a toast");
        assert!(
            toast.message.contains("Exported"),
            "got {:?}",
            toast.message
        );
        assert!(!toast.undo_available, "export is not reversible");
        assert!(
            app.last_action
                .as_deref()
                .unwrap_or("")
                .contains("Exported"),
            "status-bar feedback must be preserved"
        );
    });
    assert!(vcx.debug_bounds("toast").is_some());

    lumina_core::write_sidecar(
        &item.path,
        &lumina_core::SidecarMetadata {
            title: Some("Imported title".to_string()),
            caption: None,
            rating: 4,
            tags: vec!["imported".to_string()],
        },
    )
    .unwrap();
    view.update(vcx, |app, cx| {
        app.import_sidecars_selected(cx);
    });
    render_ui(vcx);
    view.update(vcx, |app, _| {
        let toast = app
            .toast
            .current()
            .expect("import must confirm with a toast");
        assert!(
            toast.message.contains("Imported"),
            "got {:?}",
            toast.message
        );
        assert!(
            !toast.undo_available,
            "import is informational, not undoable"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}
