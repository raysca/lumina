//! UI automation for files gone from disk: badge, Missing view, viewer note.

use super::ui_automation::{
    app_with_library, click, keystrokes, leak, render_ui, right_click, seed_library,
};
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

    click(vcx, "sidebar-nav-missing");
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
        vcx.debug_bounds("sidebar-nav-missing").is_none(),
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
    let id = engine.catalog().mark_missing_under(&gone).unwrap().missing[0];
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let item = engine_item(&view, vcx);
    view.update(vcx, |app, cx| app.open_viewer(item, cx));
    vcx.run_until_parked();
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("viewer-missing-bar").is_some(),
        "the viewer explains why a missing item shows no full image"
    );
    // The file went before any thumbnail was made: nothing cached to show.
    assert!(
        view.update(vcx, |app, _| app.viewer_image.is_none()),
        "this fixture has no cached image"
    );
    assert!(
        has_text(vcx, "viewer-missing-bar-text:The original file is missing."),
        "without a cached image the bar must not promise a cached preview"
    );
    assert!(
        has_text(vcx, "viewer-missing-keep-text:Dismiss"),
        "with no preview there is no preview to keep"
    );
    view.update(vcx, |app, _| {
        app.viewer_image = Some(crate::app::types::generate_gradient_image(
            (40, 60, 90),
            (10, 15, 30),
            64,
            48,
        ));
    });
    render_ui(vcx);
    assert!(
        has_text(
            vcx,
            "viewer-missing-bar-text:The original file is missing — you're looking at a cached preview."
        ),
        "with a cached image shown the bar says it is a cached preview"
    );
    assert!(
        has_text(vcx, "viewer-missing-keep-text:Keep preview"),
        "the card offers Keep preview over a cached image"
    );

    crate::app::missing::LOCATE_REQUESTS.with(|r| r.borrow_mut().clear());
    click(vcx, "viewer-missing-locate");
    assert_eq!(
        crate::app::missing::LOCATE_REQUESTS.with(|r| r.borrow().clone()),
        vec![id],
        "Locate file… runs the Locate path for the item on screen"
    );

    click(vcx, "viewer-missing-keep");
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("viewer-missing-card").is_none(),
        "Keep preview hides the card"
    );
    assert!(
        vcx.debug_bounds("viewer-missing-bar").is_some(),
        "Keep preview keeps the bar"
    );

    // A file on a drive that is not mounted names the drive.
    let drive = "LuminaTestDriveNotMounted";
    assert!(!std::path::Path::new("/Volumes").join(drive).exists());
    // Worked out once on selection, so select the item with its new path.
    on_drive(&view, vcx, &format!("/Volumes/{drive}/DCIM/photo_00.jpg"));
    assert!(
        has_text(vcx, "viewer-missing-drive-text:LuminaTestDriveNotMounted"),
        "the bar names the disconnected drive"
    );
    assert!(
        has_text(
            vcx,
            "viewer-missing-bar-text:“LuminaTestDriveNotMounted” is disconnected — you're looking at a cached preview."
        ),
        "the bar says the drive is disconnected"
    );

    // A very long drive name truncates inside the bar instead of growing it.
    let long = "L".repeat(400);
    on_drive(&view, vcx, &format!("/Volumes/{long}/photo_00.jpg"));
    let bar = vcx.debug_bounds("viewer-missing-bar").unwrap();
    let name = vcx.debug_bounds("viewer-missing-drive").unwrap();
    assert!(
        name.right() <= bar.right() && (f32::from(bar.size.height) - 39.5).abs() < 0.5,
        "a long drive name stays inside the 39.5 px bar, bar {bar:?} name {name:?}"
    );
    drop(view);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn viewer_missing_card_buttons_take_tab_and_enter() {
    let (temp_dir, engine) = seed_library("missing_viewer_keys", 1);
    let gone = temp_dir.join("photo_00.jpg");
    std::fs::remove_file(&gone).unwrap();
    let id = engine.catalog().mark_missing_under(&gone).unwrap().missing[0];
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let item = engine_item(&view, vcx);
    view.update(vcx, |app, cx| app.open_viewer(item, cx));
    vcx.run_until_parked();
    render_ui(vcx);

    crate::app::missing::LOCATE_REQUESTS.with(|r| r.borrow_mut().clear());
    keystrokes(vcx, "tab enter");
    assert_eq!(
        crate::app::missing::LOCATE_REQUESTS.with(|r| r.borrow().clone()),
        vec![id],
        "Tab focuses Locate file… and ↵ presses it"
    );
    view.update(vcx, |app, _| {
        assert!(
            app.is_fullscreen_viewer,
            "↵ on a focused button keeps the viewer open"
        )
    });
    keystrokes(vcx, "tab enter");
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("viewer-missing-card").is_none(),
        "the second Tab stop is Keep preview"
    );
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(!app.is_fullscreen_viewer, "Esc still closes the viewer")
    });
    drop(view);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

/// Reselects the viewer item as if its file lived at `path`, with a cached
/// image on screen.
fn on_drive(view: &gpui::Entity<crate::LuminaApp>, vcx: &mut gpui::VisualTestContext, path: &str) {
    view.update(vcx, |app, cx| {
        let mut item = app.selected_item.clone().unwrap();
        item.path = std::path::PathBuf::from(path);
        app.select_item(item, cx);
        app.viewer_image = Some(crate::app::types::generate_gradient_image(
            (40, 60, 90),
            (10, 15, 30),
            64,
            48,
        ));
    });
    render_ui(vcx);
}

/// A library of two photos, both gone, with the viewer open on the first.
fn two_missing<'a>(
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
        app.open_viewer(item, cx);
    });
    vcx.run_until_parked();
    render_ui(vcx);
    (temp_dir, view, vcx)
}

fn locate_count() -> usize {
    crate::app::missing::LOCATE_REQUESTS.with(|r| r.borrow().len())
}

#[core::prelude::v1::test]
fn missing_card_focus_does_not_outlive_the_viewer() {
    let mut cx = crate::tests::TestApp::single();
    let (temp_dir, view, vcx) = two_missing("missing_focus_close", &mut cx);
    crate::app::missing::LOCATE_REQUESTS.with(|r| r.borrow_mut().clear());
    keystrokes(vcx, "tab escape");
    view.update(vcx, |app, cx| {
        assert!(!app.is_fullscreen_viewer, "Esc closes the viewer");
        let item = app.selected_item.clone().unwrap();
        app.open_viewer(item, cx);
    });
    render_ui(vcx);
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert!(
            !app.is_fullscreen_viewer,
            "a reopened viewer has no focused button, so ↵ closes it"
        )
    });
    assert_eq!(locate_count(), 0, "↵ in a reopened viewer must not Locate");
    drop(view);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn missing_card_focus_does_not_follow_to_the_next_item() {
    let mut cx = crate::tests::TestApp::single();
    let (temp_dir, view, vcx) = two_missing("missing_focus_next", &mut cx);
    let first = view.update(vcx, |app, _| app.selected_item.as_ref().unwrap().id);
    crate::app::missing::LOCATE_REQUESTS.with(|r| r.borrow_mut().clear());
    keystrokes(vcx, "tab right");
    view.update(vcx, |app, _| {
        assert_ne!(
            app.selected_item.as_ref().unwrap().id,
            first,
            "→ moves to the other missing item"
        )
    });
    assert!(vcx.debug_bounds("viewer-missing-card").is_some());
    keystrokes(vcx, "enter");
    assert_eq!(locate_count(), 0, "focus stays with the item it was set on");
    drop(view);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn keep_preview_is_per_item() {
    let mut cx = crate::tests::TestApp::single();
    let (temp_dir, view, vcx) = two_missing("missing_keep_each", &mut cx);
    click(vcx, "viewer-missing-keep");
    render_ui(vcx);
    assert!(vcx.debug_bounds("viewer-missing-card").is_none());
    keystrokes(vcx, "right");
    assert!(
        vcx.debug_bounds("viewer-missing-card").is_some(),
        "another missing item still gets its card"
    );
    keystrokes(vcx, "left");
    assert!(
        vcx.debug_bounds("viewer-missing-card").is_none(),
        "the kept item stays dismissed for the session"
    );
    assert!(vcx.debug_bounds("viewer-missing-bar").is_some());
    drop(view);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn missing_card_hides_in_edit_mode_but_the_bar_stays() {
    let mut cx = crate::tests::TestApp::single();
    let (temp_dir, view, vcx) = two_missing("missing_edit", &mut cx);
    assert!(vcx.debug_bounds("viewer-missing-card").is_some());
    keystrokes(vcx, "e");
    view.update(vcx, |app, _| {
        assert!(app.edits.session.is_some(), "E opens edit mode")
    });
    assert!(
        vcx.debug_bounds("viewer-missing-card").is_none(),
        "the card must not cover the edit controls"
    );
    assert!(vcx.debug_bounds("viewer-missing-bar").is_some());
    drop(view);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[cfg(target_os = "macos")] // /Volumes is macOS-only
#[core::prelude::v1::test]
fn disconnected_drive_lists_volumes_by_name() {
    use crate::app::missing::disconnected_drive;
    use std::path::Path;
    assert_eq!(
        disconnected_drive(Path::new("/Volumes/LuminaTestDriveNotMounted/a.jpg")).as_deref(),
        Some("LuminaTestDriveNotMounted"),
        "a name missing from /Volumes is a disconnected drive"
    );
    assert_eq!(disconnected_drive(Path::new("/Users/x/a.jpg")), None);
    if let Some(mounted) = std::fs::read_dir("/Volumes")
        .ok()
        .and_then(|mut d| d.next())
        .and_then(Result::ok)
    {
        let path = Path::new("/Volumes")
            .join(mounted.file_name())
            .join("a.jpg");
        assert_eq!(
            disconnected_drive(&path),
            None,
            "a listed volume is not disconnected"
        );
    }
}

fn has_text(vcx: &mut gpui::VisualTestContext, selector: &str) -> bool {
    vcx.debug_bounds(leak(selector.to_string())).is_some()
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
