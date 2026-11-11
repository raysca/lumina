//! v3 toasts, error banner and empty states (FIXES §14).

use lumina_core::{MatchMode, RuleField, RuleOperator, RuleValue, SmartAlbumRule};

use super::ui_automation::{
    app_with_library, click, focus_app, keystrokes, leak, render_ui, seed_library,
};
use crate::views::sidebar::AlbumSelection;
use crate::views::toast::TOAST_DURATION;

#[core::prelude::v1::test]
fn toast_sits_centred_over_the_grid_and_waits_while_hovered() {
    let (dir, engine) = seed_library("states_toast", 3);
    let item = engine.get_item_slice(0..1)[0].clone();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    vcx.simulate_resize(gpui::size(gpui::px(1440.0), gpui::px(900.0)));
    view.update(vcx, |app, cx| app.show_undo_toast("Rated 1 item", cx));
    render_ui(vcx);

    let toast = vcx.debug_bounds("toast").expect("toast renders");
    let grid = vcx.debug_bounds("media-grid-surface").unwrap();
    let status = vcx.debug_bounds("status-bar").unwrap();
    let off = f32::from(toast.center().x - grid.center().x).abs();
    assert!(off < 2.0, "toast is centred over the grid, off by {off}");
    let gap = f32::from(status.top() - toast.bottom());
    assert!(
        (gap - 20.0).abs() < 1.0,
        "20px above the status bar, got {gap}"
    );
    let dismiss = vcx.debug_bounds("toast-dismiss").unwrap();
    assert_eq!(f32::from(dismiss.size.width), 26.0, "26px dismiss button");

    // With a selection the toast stacks above the dock instead of covering it.
    click(vcx, leak(format!("media-card-{}", item.id.0)));
    view.update(vcx, |app, cx| app.show_toast("Second", cx));
    render_ui(vcx);
    let dock = vcx.debug_bounds("multi-select-action-dock").unwrap();
    let toast = vcx.debug_bounds("toast").unwrap();
    assert!(
        toast.bottom() <= dock.top(),
        "toast {toast:?} clears dock {dock:?}"
    );

    // Hovering holds it open past its 8 s, even across a replacement under
    // the resting pointer (no new hover event fires); leaving lets it close.
    vcx.simulate_mouse_move(toast.center(), None, gpui::Modifiers::none());
    view.update(vcx, |app, _| {
        assert!(
            app.toast.current().is_some_and(|t| t.hovered),
            "pointer over the toast"
        )
    });
    view.update(vcx, |app, cx| app.show_toast("Third", cx));
    render_ui(vcx);
    vcx.executor().advance_clock(TOAST_DURATION * 2);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.toast.current().is_some(), "a hovered toast stays")
    });
    vcx.simulate_mouse_move(
        gpui::point(gpui::px(5.0), gpui::px(5.0)),
        None,
        gpui::Modifiers::none(),
    );
    vcx.executor().advance_clock(TOAST_DURATION);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(
            app.toast.current().is_none(),
            "it closes once the pointer leaves"
        )
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn error_banner_splits_title_retries_and_esc_dismisses() {
    let (dir, engine) = seed_library("states_error", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.set_error_with_retry(
            "Could not clear thumbnail cache: disk full",
            |app, cx| app.show_toast("retried", cx),
            cx,
        )
    });
    render_ui(vcx);
    for selector in [
        "error-banner-title",
        "error-banner-detail",
        "error-banner-retry",
        "error-banner-dismiss",
    ] {
        assert!(
            vcx.debug_bounds(selector).is_some(),
            "banner shows {selector}"
        );
    }
    click(vcx, "error-banner-retry");
    view.update(vcx, |app, _| {
        assert!(app.error_message.is_none(), "Retry clears the banner");
        assert_eq!(
            app.toast.current().map(|t| t.message.as_str()),
            Some("retried"),
            "Retry runs the failed action"
        );
    });

    // A visible toast takes Esc first; clear it so Esc reaches the banner.
    view.update(vcx, |app, cx| {
        app.dismiss_toast(cx);
        app.set_error("Plain failure", cx)
    });
    view.update(vcx, |app, _| {
        assert!(app.error_retry.is_none(), "no Retry by default")
    });
    focus_app(vcx, &view);
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(app.error_message.is_none(), "Esc dismisses")
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn recently_deleted_counts_notes_and_hides_buttons_when_empty() {
    let (dir, engine) = seed_library("states_trash", 3);
    let item = engine.get_item_slice(0..1)[0].clone();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    // Empty: no bulk buttons, no "Show all photos", new copy.
    view.update(vcx, |app, cx| app.select_album(AlbumSelection::Trash, cx));
    render_ui(vcx);
    assert!(vcx.debug_bounds("empty-state").is_some());
    for absent in ["empty-primary", "header-empty-trash-btn", "trash-note"] {
        assert!(
            vcx.debug_bounds(absent).is_none(),
            "empty trash has no {absent}"
        );
    }

    engine.set_trashed(item.id, true).unwrap();
    view.update(vcx, |app, cx| {
        app.refresh_album_counts();
        app.select_album(AlbumSelection::Trash, cx)
    });
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.page_meta()
                .as_deref()
                .and_then(|meta| meta.split(" · ").next()),
            Some("1 item"),
            "the meta line counts the trash (the seed's date is its file time)"
        )
    });
    for selector in [
        "page-meta",
        "trash-note",
        "header-empty-trash-btn",
        "header-restore-all-btn",
    ] {
        assert!(
            vcx.debug_bounds(selector).is_some(),
            "trash with items shows {selector}"
        );
    }

    // Keyboard paths: ⌘K reaches Empty Trash… and Restore All.
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-k e m p t y space t r a s h enter");
    view.update(vcx, |app, _| {
        assert!(
            app.delete_confirm_state.is_some(),
            "Empty Trash… asks first"
        )
    });
    keystrokes(vcx, "escape");
    keystrokes(vcx, "secondary-k r e s t o r e space a l l enter");
    assert!(
        !engine.get_item_by_id(item.id).unwrap().is_trashed,
        "Restore All puts the item back"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn restore_all_reports_what_it_did() {
    let (dir, engine) = seed_library("states_restore_all", 2);
    let item = engine.get_item_slice(0..1)[0].clone();
    engine.set_trashed(item.id, true).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    view.update(vcx, |app, cx| app.restore_all_trash(cx));
    view.update(vcx, |app, _| {
        assert!(app.error_message.is_none());
        assert_eq!(
            app.toast.current().map(|t| t.message.as_str()),
            Some("Restored 1 item(s)"),
            "Restore All confirms instead of staying silent"
        );
    });
    assert!(!engine.get_item_by_id(item.id).unwrap().is_trashed);
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn empty_smart_album_offers_only_edit_rules_and_library_path_is_mono() {
    let (dir, engine) = seed_library("states_album", 2);
    let rules = [SmartAlbumRule {
        field: RuleField::IsFavorite,
        operator: RuleOperator::Equals,
        value: RuleValue::Bool(true),
    }];
    let album = engine
        .create_smart_album("Keepers", "folder", MatchMode::All, &rules)
        .unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::User(album.id), cx)
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("empty-secondary").is_none(),
        "one button only"
    );
    click(vcx, "empty-primary");
    view.update(vcx, |app, _| {
        assert!(app.album_draft.modal_open, "Edit Rules… opens the builder")
    });
    let _ = std::fs::remove_dir_all(&dir);

    let (empty_dir, empty) = seed_library("states_empty_lib", 0);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, empty);
    view.update(vcx, |app, _| {
        if let Some(state) = app.grid_state.as_mut() {
            state.is_scanning = false;
        }
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("empty-library-path").is_some(),
        "an empty library shows its path on its own mono line"
    );
    let _ = std::fs::remove_dir_all(&empty_dir);
}
