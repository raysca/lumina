//! Edit mode in the viewer (E): keyboard and mouse paths, Done/Cancel,
//! undo, write failures, the grid's edited badge and the menu guards.

use crate::tests::ui_automation::{app_with_library, click, focus_app, keystrokes, render_ui};
use crate::*;
use lumina_core::{Edits, LuminaEngine, MediaId, MediaType};
use std::path::PathBuf;
use std::sync::Arc;

pub(crate) fn temp_library(prefix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "lumina_quick_edit_{prefix}_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock must be after the Unix epoch")
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("temp library dir must be creatable");
    dir
}

/// Opens `dir` as a library and waits for its first scan.
pub(crate) fn scanned(dir: &std::path::Path) -> Arc<LuminaEngine> {
    let (engine, rx) = LuminaEngine::open_with_paths(
        dir.to_path_buf(),
        dir.join("catalog.db"),
        Some(dir.join("cache")),
    )
    .expect("temp engine must open");
    while let Ok(event) = rx.recv_timeout(std::time::Duration::from_secs(10)) {
        if let lumina_core::EngineEvent::ScanComplete { .. } = event {
            break;
        }
    }
    engine
}

/// Three real photos and a placeholder clip in a real catalog; returns the photo
/// ids in catalog order and the clip's id.
pub(crate) fn edit_library(prefix: &str) -> (PathBuf, Arc<LuminaEngine>, Vec<MediaId>, MediaId) {
    edit_library_with(prefix, crate::tests::ui_automation::tiny_jpeg())
}

/// [`edit_library`] with photos that never decode during a test (see
/// `seed_library_undecodable`).
pub(crate) fn edit_library_undecodable(
    prefix: &str,
) -> (PathBuf, Arc<LuminaEngine>, Vec<MediaId>, MediaId) {
    edit_library_with(prefix, b"test-jpeg-content")
}

fn edit_library_with(
    prefix: &str,
    photo: &[u8],
) -> (PathBuf, Arc<LuminaEngine>, Vec<MediaId>, MediaId) {
    let dir = temp_library(prefix);
    for file in ["a.jpg", "b.jpg", "c.jpg", "clip.mp4"] {
        // The clip is a placeholder either way.
        let bytes: &[u8] = if file.ends_with(".jpg") {
            photo
        } else {
            b"test-jpeg-content"
        };
        std::fs::write(dir.join(file), bytes).expect("temp media must be written");
    }
    let engine = scanned(&dir);
    let items = engine.get_item_slice(0..4);
    assert_eq!(items.len(), 4, "three photos and a clip must be cataloged");
    let photos = items
        .iter()
        .filter(|item| item.media_type != MediaType::Video)
        .map(|item| item.id)
        .collect();
    let clip = items
        .iter()
        .find(|item| item.media_type == MediaType::Video)
        .expect("the clip must be cataloged as video")
        .id;
    (dir, engine, photos, clip)
}

pub(crate) fn edits_of(engine: &LuminaEngine, id: MediaId) -> Edits {
    engine
        .get_edits(&[id])
        .expect("edits read must work")
        .remove(&id)
        .expect("the item must be cataloged")
}

pub(crate) fn edit_rows(dir: &std::path::Path, id: MediaId) -> i64 {
    rusqlite::Connection::open(dir.join("catalog.db"))
        .expect("catalog must open")
        .query_row(
            "SELECT COUNT(*) FROM media_edits WHERE media_id = ?1",
            [id.0 as i64],
            |row| row.get(0),
        )
        .expect("media_edits must be readable")
}

/// Opens the viewer on `id` with the app focused.
pub(crate) fn open_viewer_on(
    view: &gpui::Entity<LuminaApp>,
    vcx: &mut gpui::VisualTestContext,
    engine: &LuminaEngine,
    id: MediaId,
) {
    let item = engine.get_item_by_id(id).expect("item must be cataloged");
    view.update(vcx, |app, cx| {
        app.select_item_at(item.clone(), 0, cx);
        app.open_viewer(item, cx);
    });
    render_ui(vcx);
    focus_app(vcx, view);
}

pub(crate) fn editing(view: &gpui::Entity<LuminaApp>, vcx: &mut gpui::VisualTestContext) -> bool {
    view.update(vcx, |app, _| app.edits.session.is_some())
}

#[core::prelude::v1::test]
fn keyboard_only_exposure_persists() {
    let (dir, engine, photos, _) = edit_library("exposure_keys");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_viewer_on(&view, vcx, &engine, photos[0]);

    keystrokes(vcx, "e");
    assert!(editing(&view, vcx), "E in the viewer opens edit mode");
    assert!(
        vcx.debug_bounds("edit-bar").is_some(),
        "edit mode shows the edit bar"
    );
    keystrokes(vcx, "= =");
    keystrokes(vcx, "enter");

    let saved = edits_of(&engine, photos[0]);
    assert!(
        (saved.exposure_ev - 2.0 / 3.0).abs() < 1e-3,
        "two = steps save +2/3 EV, got {}",
        saved.exposure_ev
    );
    view.update(vcx, |app, _| {
        assert!(app.edits.session.is_none(), "Enter is Done");
        assert!(app.is_fullscreen_viewer, "Done keeps the viewer open");
        assert_eq!(app.undo.undo_stack.len(), 1, "Done is one undo step");
        assert!(app.error_message.is_none(), "{:?}", app.error_message);
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn escape_cancels_without_writing() {
    let (dir, engine, photos, _) = edit_library("escape_cancels");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_viewer_on(&view, vcx, &engine, photos[0]);

    keystrokes(vcx, "e = a r");
    keystrokes(vcx, "escape");

    assert_eq!(edit_rows(&dir, photos[0]), 0, "Cancel writes no edits row");
    assert_eq!(
        edits_of(&engine, photos[0]),
        Edits::default(),
        "Cancel leaves rotation and edits untouched"
    );
    view.update(vcx, |app, _| {
        assert!(app.edits.session.is_none(), "Escape closes edit mode");
        assert!(
            app.is_fullscreen_viewer,
            "Escape in edit mode must not close the viewer"
        );
        assert!(app.undo.undo_stack.is_empty(), "Cancel records no undo");
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn edit_mode_enter_does_not_close_viewer() {
    let (dir, engine, photos, _) = edit_library("enter_keeps_viewer");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_viewer_on(&view, vcx, &engine, photos[0]);

    keystrokes(vcx, "e");
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert!(app.edits.session.is_none(), "Enter is Done");
        assert!(
            app.is_fullscreen_viewer,
            "Enter in edit mode must not close the viewer"
        );
        assert!(
            app.undo.undo_stack.is_empty(),
            "Done with no change records no undo"
        );
    });
    assert_eq!(
        edit_rows(&dir, photos[0]),
        0,
        "an unchanged Done writes nothing"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Caches a placeholder thumbnail for `id`, as if painted with old edits.
fn put_texture(view: &gpui::Entity<LuminaApp>, vcx: &mut gpui::VisualTestContext, id: MediaId) {
    let texture = Arc::new(gpui::RenderImage::new(vec![image::Frame::new(
        image::RgbaImage::new(4, 4),
    )]));
    view.update(vcx, |app, _| {
        app.grid_state
            .as_ref()
            .expect("grid state must be installed")
            .texture_cache
            .lock()
            .put(id, texture);
    });
}

fn has_texture(
    view: &gpui::Entity<LuminaApp>,
    vcx: &mut gpui::VisualTestContext,
    id: MediaId,
) -> bool {
    view.update(vcx, |app, _| {
        app.grid_state
            .as_ref()
            .expect("grid state")
            .texture_cache
            .lock()
            .contains(&id)
    })
}

fn is_marked_edited(
    view: &gpui::Entity<LuminaApp>,
    vcx: &mut gpui::VisualTestContext,
    id: MediaId,
) -> bool {
    vcx.run_until_parked();
    view.update(vcx, |app, _| app.edits.edited_ids.contains(&id))
}

#[core::prelude::v1::test]
fn done_then_undo_restores_original_once() {
    let (dir, engine, photos, _) = edit_library_undecodable("done_undo");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_viewer_on(&view, vcx, &engine, photos[0]);
    put_texture(&view, vcx, photos[0]);

    keystrokes(vcx, "e a shift-r -");
    keystrokes(vcx, "enter");
    let edited = Edits {
        rotation: 3,
        crop: None,
        exposure_ev: -1.0 / 3.0,
        auto_enhance: true,
    };
    let saved = edits_of(&engine, photos[0]);
    assert_eq!(
        saved.rotation, edited.rotation,
        "⇧R in edit mode turns left"
    );
    assert!(saved.auto_enhance, "A toggles auto-enhance on");
    assert!((saved.exposure_ev - edited.exposure_ev).abs() < 1e-3);
    assert!(
        !has_texture(&view, vcx, photos[0]),
        "Done drops the thumbnail painted without the edits"
    );
    assert!(
        is_marked_edited(&view, vcx, photos[0]),
        "Done marks the photo edited for the grid badge"
    );

    put_texture(&view, vcx, photos[0]);
    keystrokes(vcx, "cmd-z");
    assert_eq!(
        edits_of(&engine, photos[0]),
        Edits::default(),
        "⌘Z restores the original exactly"
    );
    assert_eq!(edit_rows(&dir, photos[0]), 0, "undo removes the edits row");
    assert!(
        !has_texture(&view, vcx, photos[0]),
        "undo drops the thumbnail painted with the edits"
    );
    assert!(
        !is_marked_edited(&view, vcx, photos[0]),
        "undo clears the edited badge"
    );
    view.update(vcx, |app, _| {
        assert!(app.undo.undo_stack.is_empty(), "Done was exactly one step");
        assert_eq!(app.undo.redo_stack.len(), 1);
    });
    keystrokes(vcx, "cmd-z");
    assert_eq!(
        edits_of(&engine, photos[0]),
        Edits::default(),
        "a second ⌘Z has nothing left to undo"
    );

    keystrokes(vcx, "cmd-shift-z");
    assert!(edits_of(&engine, photos[0]).auto_enhance, "redo reapplies");
    assert!(
        is_marked_edited(&view, vcx, photos[0]),
        "redo marks the photo edited again"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn revert_selection_is_one_undo_step() {
    let (dir, engine, photos, _) = edit_library("revert_batch");
    let exposed = Edits {
        exposure_ev: 1.0,
        ..Edits::default()
    };
    let turned = Edits {
        rotation: 2,
        ..Edits::default()
    };
    engine
        .set_edits(&[(photos[0], exposed), (photos[1], turned)])
        .expect("seed edits");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    keystrokes(vcx, "cmd-a");
    keystrokes(vcx, "cmd-k");
    crate::tests::ui_automation::type_text(vcx, "revert to original");
    keystrokes(vcx, "enter");
    for id in &photos {
        assert_eq!(
            edits_of(&engine, *id),
            Edits::default(),
            "Revert to Original resets every selected photo"
        );
    }
    view.update(vcx, |app, _| {
        assert!(app.error_message.is_none(), "{:?}", app.error_message);
        assert_eq!(app.undo.undo_stack.len(), 1, "a batch revert is one step");
    });

    keystrokes(vcx, "cmd-z");
    assert_eq!(
        edits_of(&engine, photos[0]),
        exposed,
        "one ⌘Z restores each"
    );
    assert_eq!(edits_of(&engine, photos[1]), turned, "one ⌘Z restores each");
    assert_eq!(edits_of(&engine, photos[2]), Edits::default());
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn edit_write_failure_keeps_draft_and_banners() {
    let (dir, engine, photos, _) = edit_library("write_failure");
    rusqlite::Connection::open(dir.join("catalog.db"))
        .expect("catalog must open")
        .execute_batch(
            "CREATE TRIGGER fail_edits BEFORE INSERT ON media_edits
             BEGIN SELECT RAISE(ABORT, 'injected'); END;",
        )
        .expect("trigger must install");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_viewer_on(&view, vcx, &engine, photos[0]);

    keystrokes(vcx, "e =");
    keystrokes(vcx, "enter");

    assert_eq!(
        edits_of(&engine, photos[0]),
        Edits::default(),
        "a failed write leaves the catalog as it was"
    );
    view.update(vcx, |app, _| {
        assert!(
            app.error_message
                .as_deref()
                .is_some_and(|m| m.starts_with("Could not save edits")),
            "the failure reaches the banner: {:?}",
            app.error_message
        );
        let session = app
            .edits
            .session
            .as_ref()
            .expect("a failed Done keeps edit mode open");
        assert!(
            (session.draft.exposure_ev - 1.0 / 3.0).abs() < 1e-3,
            "the draft survives the failure"
        );
        assert!(
            app.undo.undo_stack.is_empty(),
            "a failed edit is not undoable"
        );
    });
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(
            app.error_message.is_none(),
            "Esc first dismisses the banner"
        );
        assert!(
            app.edits.session.is_some(),
            "and keeps the draft for another try"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn edited_badge_shows_only_for_edited() {
    let (dir, engine, photos, _) = edit_library("badge");
    engine
        .set_edits(&[(
            photos[0],
            Edits {
                auto_enhance: true,
                ..Edits::default()
            },
        )])
        .expect("seed edits");
    engine
        .set_rotations(&[(photos[1], 1)])
        .expect("seed rotation");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    // As a library open does.
    view.update(vcx, |app, cx| app.reload_edited_ids(cx));
    render_ui(vcx);

    let badge =
        |id: MediaId| crate::tests::ui_automation::leak(format!("grid-edited-badge-{}", id.0));
    assert!(
        vcx.debug_bounds(badge(photos[0])).is_some(),
        "an edited photo shows the badge"
    );
    assert!(
        vcx.debug_bounds(badge(photos[1])).is_some(),
        "a rotated photo counts as edited"
    );
    assert!(
        vcx.debug_bounds(badge(photos[2])).is_none(),
        "an unedited photo has no badge"
    );

    // Rotating refreshes the cache too.
    open_viewer_on(&view, vcx, &engine, photos[2]);
    keystrokes(vcx, "r");
    keystrokes(vcx, "escape");
    assert!(
        vcx.debug_bounds(badge(photos[2])).is_some(),
        "rotating a photo shows its badge"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn edit_bar_buttons_by_mouse() {
    let (dir, engine, photos, _) = edit_library("mouse");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_viewer_on(&view, vcx, &engine, photos[0]);
    keystrokes(vcx, "e");
    for selector in [
        "edit-rotate-left",
        "edit-rotate-right",
        "edit-exposure-down",
        "edit-exposure-value",
        "edit-revert",
        "edit-cancel",
    ] {
        assert!(
            vcx.debug_bounds(selector).is_some(),
            "{selector} is painted"
        );
    }

    click(vcx, "edit-exposure-up");
    click(vcx, "edit-rotate-right");
    click(vcx, "edit-auto");
    click(vcx, "edit-done");
    let saved = edits_of(&engine, photos[0]);
    assert_eq!(saved.rotation, 1, "the rotate button turns the draft");
    assert!(saved.auto_enhance, "the Auto button toggles auto-enhance");
    assert!((saved.exposure_ev - 1.0 / 3.0).abs() < 1e-3);
    assert!(!editing(&view, vcx), "Done closes edit mode");
    let _ = std::fs::remove_dir_all(&dir);
}
