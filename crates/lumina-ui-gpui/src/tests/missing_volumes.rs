//! Missing originals in the viewer: a drive that mounts while its item is
//! on screen, and Locate from a viewer opened on a multi-selection.

use super::ui_automation::{app_with_library, leak, render_ui, seed_library};
use crate::app::missing::{LOCATE_REQUESTS, VOLUMES_DIR, VOLUME_POLL};

/// A library of two photos, both gone, both selected, with the viewer open
/// on the first.
fn two_missing_selected<'a>(
    name: &str,
    cx: &'a mut crate::tests::TestApp,
) -> (
    std::path::PathBuf,
    gpui::Entity<crate::LuminaApp>,
    &'a mut gpui::VisualTestContext,
) {
    let (temp_dir, engine) = seed_library(name, 2);
    for file in ["photo_00.jpg", "photo_01.jpg"] {
        let gone = temp_dir.join(file);
        std::fs::remove_file(&gone).unwrap();
        engine.catalog().mark_missing_under(&gone).unwrap();
    }
    let (view, vcx) = app_with_library(cx, engine);
    render_ui(vcx);
    view.update(vcx, |app, cx| {
        app.select_index(0, cx);
        let item = app.selected_item.clone().unwrap();
        app.extend_selection(1, cx);
        app.open_viewer(item, cx);
        assert_eq!(
            app.get_selected_ids().len(),
            2,
            "precondition: two selected"
        );
    });
    vcx.run_until_parked();
    render_ui(vcx);
    (temp_dir, view, vcx)
}

#[core::prelude::v1::test]
fn viewer_locate_acts_on_the_item_on_screen() {
    let mut cx = crate::tests::TestApp::single();
    let (temp_dir, view, vcx) = two_missing_selected("missing_locate_viewer_multi", &mut cx);
    let before = LOCATE_REQUESTS.with(|r| r.borrow().len());
    let shown = view.update(vcx, |app, cx| {
        assert!(app.is_fullscreen_viewer);
        app.locate_missing_selected(cx);
        assert_eq!(
            app.toast.current().map(|toast| toast.message.as_str()),
            None,
            "the viewer never asks for a single selection"
        );
        app.selected_item.as_ref().unwrap().id
    });
    let requests = LOCATE_REQUESTS.with(|r| r.borrow()[before..].to_vec());
    assert_eq!(requests, [shown], "Locate opens for the item being viewed");
    drop(view);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

const DRIVE_TEXT: &str = "viewer-missing-drive-text:LuminaTestDrive";

/// Points `/Volumes` at an empty `Volumes` folder in `temp_dir`.
fn fake_volumes(temp_dir: &std::path::Path) -> std::path::PathBuf {
    let volumes = temp_dir.join("Volumes");
    std::fs::create_dir_all(&volumes).unwrap();
    VOLUMES_DIR.with(|d| *d.borrow_mut() = Some(volumes.clone()));
    volumes
}

/// Reselects the viewer item as if its file lived on `LuminaTestDrive`.
fn on_test_drive(view: &gpui::Entity<crate::LuminaApp>, vcx: &mut gpui::VisualTestContext) {
    view.update(vcx, |app, cx| {
        let mut item = app.selected_item.clone().unwrap();
        item.path = "/Volumes/LuminaTestDrive/DCIM/photo_00.jpg".into();
        app.select_item(item, cx);
    });
    render_ui(vcx);
}

fn names_drive(vcx: &mut gpui::VisualTestContext) -> bool {
    vcx.debug_bounds(leak(DRIVE_TEXT.to_string())).is_some()
}

/// Mounts `LuminaTestDrive` and lets one check run; the selection must not
/// change on the way.
fn mount_and_tick(
    view: &gpui::Entity<crate::LuminaApp>,
    vcx: &mut gpui::VisualTestContext,
    volumes: &std::path::Path,
) {
    let selected = view.update(vcx, |app, _| {
        (
            app.selected_item.as_ref().map(|i| i.id),
            app.get_selected_ids(),
        )
    });
    std::fs::create_dir(volumes.join("LuminaTestDrive")).unwrap();
    vcx.executor().advance_clock(VOLUME_POLL);
    vcx.run_until_parked();
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            (
                app.selected_item.as_ref().map(|i| i.id),
                app.get_selected_ids()
            ),
            selected,
            "the bar refreshes on its own, not through a new selection"
        )
    });
}

#[core::prelude::v1::test]
fn mounting_the_drive_refreshes_the_offline_bar() {
    let mut cx = crate::tests::TestApp::single();
    let (temp_dir, view, vcx) = two_missing_selected("missing_drive_mounts", &mut cx);
    let volumes = fake_volumes(&temp_dir);
    on_test_drive(&view, vcx);
    assert!(
        names_drive(vcx),
        "precondition: the bar names the disconnected drive"
    );

    mount_and_tick(&view, vcx, &volumes);
    assert!(
        !names_drive(vcx),
        "a mounted drive is no longer named as disconnected"
    );
    assert!(
        vcx.debug_bounds("viewer-missing-bar").is_some(),
        "the file is still missing, so the bar stays"
    );
    vcx.executor().advance_clock(VOLUME_POLL);
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        assert!(!app.missing_card.watching, "the check stops once it mounts")
    });

    VOLUMES_DIR.with(|d| *d.borrow_mut() = None);
    drop(view);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn drive_unplugged_on_screen_then_mounted_refreshes_the_bar() {
    let (temp_dir, engine) = seed_library("missing_drive_unplugged", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    render_ui(vcx);
    view.update(vcx, |app, cx| {
        app.select_index(0, cx);
        let item = app.selected_item.clone().unwrap();
        app.open_viewer(item, cx);
    });
    vcx.run_until_parked();
    let volumes = fake_volumes(&temp_dir);
    // On screen and online: the drive is plugged in, so nothing shows.
    std::fs::create_dir(volumes.join("LuminaTestDrive")).unwrap();
    on_test_drive(&view, vcx);
    assert!(vcx.debug_bounds("viewer-missing-bar").is_none());

    // Unplugged: the watcher marks the file missing and the set refreshes.
    std::fs::remove_dir(volumes.join("LuminaTestDrive")).unwrap();
    let gone = temp_dir.join("photo_00.jpg");
    std::fs::remove_file(&gone).unwrap();
    let missing = engine.catalog().mark_missing_under(&gone).unwrap().missing;
    view.update(vcx, |app, cx| {
        app.handle_ui_message(
            crate::app::types::UiMessage::Engine(lumina_core::EngineEvent::ItemsMissing(missing)),
            cx,
        )
    });
    render_ui(vcx);
    assert!(names_drive(vcx), "the bar names the drive that went away");

    // Plugged back in, with the same item on screen.
    mount_and_tick(&view, vcx, &volumes);
    assert!(
        !names_drive(vcx),
        "mounting refreshes the bar without a selection change"
    );

    VOLUMES_DIR.with(|d| *d.borrow_mut() = None);
    drop(view);
    let _ = std::fs::remove_dir_all(&temp_dir);
}
