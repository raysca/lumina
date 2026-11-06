//! UI automation for files gone from disk: badge, Missing view, viewer note.

use super::ui_automation::{app_with_library, click, leak, render_ui, right_click, seed_library};
use crate::views::sidebar::AlbumSelection;
use lumina_core::SystemAlbum;

#[core::prelude::v1::test]
fn missing_item_shows_badge_and_view() {
    let (temp_dir, engine) = seed_library("missing_badge", 3);
    let gone = temp_dir.join("photo_01.jpg");
    std::fs::remove_file(&gone).unwrap();
    let change = engine.catalog().mark_missing_under(&gone).unwrap();
    let missing_id = change.missing[0];
    let present_id = engine
        .get_item_slice(0..10)
        .into_iter()
        .find(|item| item.id != missing_id)
        .unwrap()
        .id;

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    render_ui(vcx);

    assert!(
        vcx.debug_bounds(leak(format!("grid-missing-badge-{}", missing_id.0)))
            .is_some(),
        "a missing item carries the badge"
    );
    assert!(
        vcx.debug_bounds(leak(format!("grid-missing-badge-{}", present_id.0)))
            .is_none(),
        "a present item has no badge"
    );

    click(vcx, "sidebar-nav-Missing");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::System(SystemAlbum::Missing),
            "the Missing row opens the Missing view"
        );
        assert_eq!(
            app.displayed_count(),
            1,
            "the view lists only missing items"
        );
        assert_eq!(
            app.total_library_count, 3,
            "missing items stay in the library"
        );
    });

    drop(view);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn missing_row_hidden_when_nothing_is_missing() {
    let (temp_dir, engine) = seed_library("missing_none", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("sidebar-nav-Missing").is_none(),
        "the Missing row only shows while something is missing"
    );
    drop(view);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn viewer_says_the_original_is_missing() {
    let (temp_dir, engine) = seed_library("missing_viewer", 1);
    let gone = temp_dir.join("photo_00.jpg");
    std::fs::remove_file(&gone).unwrap();
    engine.catalog().mark_missing_under(&gone).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let item = engine_item(&view, vcx);
    view.update(vcx, |app, cx| app.open_viewer(item, cx));
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("viewer-missing").is_some(),
        "the viewer explains why a missing item shows no full image"
    );
    drop(view);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

fn engine_item(
    view: &gpui::Entity<crate::LuminaApp>,
    vcx: &mut gpui::VisualTestContext,
) -> lumina_core::MediaItem {
    view.update(vcx, |app, _| {
        let engine = app.grid_state.as_ref().unwrap().engine.clone();
        engine.get_item_slice(0..1).remove(0)
    })
}

#[core::prelude::v1::test]
fn locate_outside_library_shows_banner() {
    let (temp_dir, engine) = seed_library("missing_locate", 1);
    let gone = temp_dir.join("photo_00.jpg");
    std::fs::remove_file(&gone).unwrap();
    let id = engine.catalog().mark_missing_under(&gone).unwrap().missing[0];
    let outside = std::env::temp_dir().join(format!("lumina_outside_{}.jpg", std::process::id()));
    std::fs::write(&outside, b"elsewhere").unwrap();

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| app.apply_locate(id, outside.clone(), cx));
    vcx.run_until_parked();

    view.update(vcx, |app, _| {
        let error = app.error_message.clone().unwrap_or_default();
        assert!(
            error.starts_with("Choose a file inside the library folder"),
            "a file outside the library is refused with a reason, got {error:?}"
        );
        assert!(app.missing_items.contains(&id), "the item stays missing");
    });
    drop(view);
    let _ = std::fs::remove_file(&outside);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn locate_needs_one_missing_item_selected() {
    let (temp_dir, engine) = seed_library("missing_locate_none", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.execute_command(crate::app::command_palette::CommandId::LocateMissing, cx);
        assert_eq!(
            app.toast.current().map(|toast| toast.message.as_str()),
            Some("Select one missing item to locate it"),
            "Locate explains what it needs instead of doing nothing"
        );
    });
    drop(view);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn context_menu_offers_locate_only_for_missing_items() {
    let (temp_dir, engine) = seed_library("missing_menu", 2);
    let gone = temp_dir.join("photo_00.jpg");
    std::fs::remove_file(&gone).unwrap();
    let missing_id = engine.catalog().mark_missing_under(&gone).unwrap().missing[0];
    let present_id = engine
        .get_item_slice(0..10)
        .into_iter()
        .find(|item| item.id != missing_id)
        .unwrap()
        .id;
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    render_ui(vcx);

    right_click(vcx, leak(format!("media-card-{}", missing_id.0)));
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("missing-locate").is_some(),
        "a missing item's menu offers Locate"
    );

    view.update(vcx, |app, cx| app.close_context_menu(cx));
    right_click(vcx, leak(format!("media-card-{}", present_id.0)));
    render_ui(vcx);
    // Debug bounds from the first menu can linger; look inside this one.
    let menu = vcx.debug_bounds("context-menu").expect("the menu opens");
    assert!(
        !vcx.debug_bounds("missing-locate")
            .is_some_and(|locate| menu.contains(&locate.center())),
        "a present item's menu has no Locate"
    );
    drop(view);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn locate_reconnects_a_moved_file() {
    let (temp_dir, engine) = seed_library("missing_locate_ok", 1);
    let gone = temp_dir.join("photo_00.jpg");
    let moved = temp_dir.join("moved.jpg");
    // A copy, not a rename: a new inode, so nothing reconnects it on its own.
    std::fs::copy(&gone, &moved).unwrap();
    std::fs::remove_file(&gone).unwrap();
    let id = engine.catalog().mark_missing_under(&gone).unwrap().missing[0];

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| app.apply_locate(id, moved.clone(), cx));
    vcx.run_until_parked();

    view.update(vcx, |app, _| {
        assert_eq!(app.error_message, None, "a matching file is accepted");
        assert!(
            !app.missing_items.contains(&id),
            "the item is no longer missing"
        );
        let engine = app.grid_state.as_ref().unwrap().engine.clone();
        let item = engine.get_item_by_id(id).expect("the item keeps its id");
        assert_eq!(
            item.path.canonicalize().unwrap(),
            moved.canonicalize().unwrap(),
            "the item points at the located file"
        );
    });
    drop(view);
    let _ = std::fs::remove_dir_all(&temp_dir);
}
