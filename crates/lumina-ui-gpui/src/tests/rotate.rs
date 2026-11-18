//! Rotation: controls, shortcuts, palette, batch undo and guards.

use crate::tests::helpers::*;
use crate::*;

/// Builds a temporary library with a real catalog so rotation persistence and
/// thumbnail invalidation are exercised end to end.
fn rotation_test_library(
    prefix: &str,
    files: &[&str],
) -> (
    crate::tests::helpers::TempLibrary,
    std::sync::Arc<lumina_core::LuminaEngine>,
) {
    let temp_dir = std::env::temp_dir().join(format!(
        "lumina_{prefix}_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock must be after the Unix epoch")
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).expect("temp library dir must be creatable");
    // Not images: the rotation tests check that a thumbnail is dropped, so
    // no re-decode may land during the test (every decode of these fails).
    for file in files {
        std::fs::write(temp_dir.join(file), b"test-jpeg-content")
            .expect("temp media file must be writable");
    }
    let (engine, rx) = lumina_core::LuminaEngine::open_with_paths(
        temp_dir.clone(),
        temp_dir.join("catalog.db"),
        Some(temp_dir.join("cache")),
    )
    .expect("temp engine must open");
    crate::tests::helpers::wait_for_scan(|timeout| rx.recv_timeout(timeout).ok());
    (crate::tests::helpers::TempLibrary(temp_dir), engine)
}

#[core::prelude::v1::test]
fn test_viewer_rotate_controls_persist_and_invalidate_thumbnail() {
    use crate::views::grid::GridState;

    let (_temp_dir, engine) =
        rotation_test_library("viewer_rotate_controls", &["photo0.jpg", "photo1.jpg"]);
    let item = engine
        .get_item_slice(0..1)
        .into_iter()
        .next()
        .expect("scanned photo must be cataloged");
    let item_id = item.id;

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));

    let texture = std::sync::Arc::new(gpui::RenderImage::new(vec![image::Frame::new(
        image::RgbaImage::new(4, 4),
    )]));
    view.update(vcx, |app, cx| {
        app.grid_state = Some(GridState::new(engine.clone()));
        app.select_item_at(item.clone(), 0, cx);
        app.open_viewer(item.clone(), cx);
        app.window_bounds = Some((0.0, 0.0, 1440.0, 900.0));
        let state = app
            .grid_state
            .as_ref()
            .expect("grid state must be installed");
        state.texture_cache.lock().put(item_id, texture.clone());
        app.viewer_image = Some(texture);
        cx.notify();
    });
    vcx.simulate_resize(gpui::size(gpui::px(1440.0), gpui::px(900.0)));
    render_viewer_ui(vcx);
    render_viewer_ui(vcx);

    assert_eq!(
        engine
            .catalog()
            .get_rotation(item_id)
            .expect("rotation read must work"),
        0,
        "a scanned photo must start unrotated"
    );

    // Rotation lives in the viewer's right-click menu (the top bar is slim).
    let rotate = |vcx: &mut gpui::VisualTestContext, selector: &'static str| {
        let viewport = vcx.debug_bounds("viewer-viewport").expect("viewport");
        vcx.simulate_event(gpui::MouseDownEvent {
            position: viewport.center(),
            modifiers: gpui::Modifiers::default(),
            button: gpui::MouseButton::Right,
            click_count: 1,
            first_mouse: false,
        });
        render_viewer_ui(vcx);
        let control = vcx
            .debug_bounds(selector)
            .expect("the viewer right-click menu must offer rotation");
        vcx.simulate_click(control.center(), gpui::Modifiers::default());
        render_viewer_ui(vcx);
    };
    assert!(
        vcx.debug_bounds("quick-edit-rotate-left").is_none(),
        "the viewer top bar has no rotate buttons"
    );

    rotate(vcx, "quick-edit-rotate-right");
    view.update(vcx, |app, _| {
        assert_eq!(
            engine
                .catalog()
                .get_rotation(item_id)
                .expect("rotation read must work"),
            1,
            "Rotate Right must persist one clockwise quarter turn"
        );
        let state = app
            .grid_state
            .as_ref()
            .expect("grid state must be installed");
        assert!(
            !state.texture_cache.lock().contains(&item_id),
            "the cached thumbnail must be invalidated so the grid re-decodes"
        );
        assert!(
            app.viewer_image.is_none(),
            "the viewer image must be cleared for the rotated re-request"
        );
        assert_eq!(app.last_action.as_deref(), Some("Rotated right"));
    });

    rotate(vcx, "quick-edit-rotate-right");
    rotate(vcx, "quick-edit-rotate-left");
    view.update(vcx, |app, _| {
        assert_eq!(
            engine
                .catalog()
                .get_rotation(item_id)
                .expect("rotation read must work"),
            1,
            "right, right, left must land back on one quarter turn"
        );
        assert!(
            app.error_message.is_none(),
            "rotation must not raise a banner"
        );
    });

    // The context menu exposes the same quick edits. The viewer is closed so the
    // shared debug selector is unique to the menu (the top bar is not painted).
    view.update(vcx, |app, cx| {
        app.close_viewer(cx);
        app.open_card_context_menu(
            item.clone(),
            0,
            gpui::point(gpui::px(40.0), gpui::px(40.0)),
            cx,
        );
        cx.notify();
    });
    render_viewer_ui(vcx);
    let menu_right = vcx
        .debug_bounds("quick-edit-rotate-right")
        .expect("the context menu must offer Rotate Right");
    vcx.simulate_click(menu_right.center(), gpui::Modifiers::default());
    render_viewer_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            engine
                .catalog()
                .get_rotation(item_id)
                .expect("rotation read must work"),
            2,
            "the context-menu rotate control must persist the same edit"
        );
        assert!(
            app.context_menu_state.is_none(),
            "the context menu must close after rotating"
        );
    });
}

#[core::prelude::v1::test]
fn test_viewer_rotate_video_is_a_clear_no_op() {
    use crate::views::viewer_state::ViewerRotation;

    let mut cx = crate::tests::TestApp::single();
    let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    let video = get_sample_items()[1].clone();

    view.update(cx, |app, cx| {
        app.grid_state = None;
        app.select_item_at(video.clone(), 1, cx);
        app.open_viewer(video, cx);
        app.rotate_selection(ViewerRotation::Right, cx);
        assert_eq!(
            app.last_action.as_deref(),
            Some("Rotation is not supported for video"),
            "video rotation must report a clear no-op"
        );
        assert!(
            app.error_message.is_none(),
            "an unsupported video rotation is not an error"
        );
    });
    cx.run_until_parked();
}

/// Three photos and a clip; returns the photo ids (in catalog order) and the
/// clip's id.
fn rotation_batch_library(
    prefix: &str,
) -> (
    crate::tests::helpers::TempLibrary,
    std::sync::Arc<lumina_core::LuminaEngine>,
    Vec<lumina_core::MediaId>,
    lumina_core::MediaId,
) {
    let (dir, engine) = rotation_test_library(prefix, &["a.jpg", "b.jpg", "c.jpg", "clip.mp4"]);
    let items = engine.get_item_slice(0..4);
    assert_eq!(items.len(), 4, "three photos and a clip must be cataloged");
    let photos = items
        .iter()
        .filter(|item| item.media_type != lumina_core::MediaType::Video)
        .map(|item| item.id)
        .collect();
    let video = items
        .iter()
        .find(|item| item.media_type == lumina_core::MediaType::Video)
        .expect("the clip must be cataloged as video")
        .id;
    (dir, engine, photos, video)
}

fn rotations(engine: &lumina_core::LuminaEngine, ids: &[lumina_core::MediaId]) -> Vec<u8> {
    ids.iter()
        .map(|id| {
            engine
                .catalog()
                .get_rotation(*id)
                .expect("rotation read must work")
        })
        .collect()
}

/// Opens the viewer on the first photo, with the app focused.
fn open_rotation_viewer(
    view: &gpui::Entity<LuminaApp>,
    vcx: &mut gpui::VisualTestContext,
    engine: &lumina_core::LuminaEngine,
    id: lumina_core::MediaId,
) {
    let item = engine.get_item_by_id(id).expect("photo must be cataloged");
    view.update(vcx, |app, cx| {
        app.select_item_at(item.clone(), 0, cx);
        app.open_viewer(item, cx);
    });
    render_viewer_ui(vcx);
    focus_app(vcx, view);
}

#[core::prelude::v1::test]
fn rotate_selection_by_shortcut_persists_all_and_undoes_once() {
    let (_dir, engine, photos, video) = rotation_batch_library("rotate_batch_shortcut");
    // Different starting angles, so undo must restore each item's own value.
    let seeded = [0_u8, 1, 3];
    for (id, quarters) in photos.iter().zip(seeded) {
        engine
            .set_rotations(&[(*id, quarters)])
            .expect("seed rotation");
    }
    engine
        .set_rotations(&[(video, 2)])
        .expect("seed video rotation");

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    keystrokes(vcx, "cmd-a");
    keystrokes(vcx, "r");

    assert_eq!(
        rotations(&engine, &photos),
        vec![1, 2, 0],
        "R must turn every selected photo one quarter clockwise"
    );
    assert_eq!(
        engine.catalog().get_rotation(video).expect("rotation read"),
        2,
        "R must leave a selected video untouched"
    );
    view.update(vcx, |app, _| {
        assert!(app.error_message.is_none(), "{:?}", app.error_message);
        assert_eq!(
            app.undo.undo_stack.len(),
            1,
            "a batch rotation is one undo step"
        );
        // A thumbnail painted at the new angle, which undo must drop.
        let texture = std::sync::Arc::new(gpui::RenderImage::new(vec![image::Frame::new(
            image::RgbaImage::new(4, 4),
        )]));
        app.grid_state
            .as_ref()
            .expect("grid state must be installed")
            .texture_cache
            .lock()
            .put(photos[0], texture);
    });

    keystrokes(vcx, "cmd-z");
    view.update(vcx, |app, _| {
        let state = app.grid_state.as_ref().expect("grid state");
        assert!(
            !state.texture_cache.lock().contains(&photos[0]),
            "undo must drop the thumbnail painted at the undone angle"
        );
    });
    assert_eq!(
        rotations(&engine, &photos),
        seeded.to_vec(),
        "one ⌘Z must restore each photo's own previous rotation"
    );
    assert_eq!(
        engine.catalog().get_rotation(video).expect("rotation read"),
        2
    );

    // Redo turns them again and must drop what was painted at the old angle.
    put_test_texture(&view, vcx, photos[0]);
    keystrokes(vcx, "cmd-shift-z");
    assert_eq!(
        rotations(&engine, &photos),
        vec![1, 2, 0],
        "⇧⌘Z must reapply each photo's own rotation"
    );
    view.update(vcx, |app, _| {
        let state = app.grid_state.as_ref().expect("grid state");
        assert!(
            !state.texture_cache.lock().contains(&photos[0]),
            "redo must drop the thumbnail painted at the redone angle"
        );
    });
}

#[core::prelude::v1::test]
fn shift_r_rotates_left_in_viewer() {
    let (_dir, engine, photos, _video) = rotation_batch_library("rotate_shift_r_viewer");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_rotation_viewer(&view, vcx, &engine, photos[0]);

    keystrokes(vcx, "shift-r");
    assert_eq!(
        rotations(&engine, &photos),
        vec![3, 0, 0],
        "⇧R in the viewer must turn only the open photo counter-clockwise"
    );
    view.update(vcx, |app, _| {
        assert!(
            app.is_fullscreen_viewer,
            "rotating must keep the viewer open"
        );
        assert_eq!(app.undo.undo_stack.len(), 1);
    });
}

#[core::prelude::v1::test]
fn rotate_from_palette_in_viewer() {
    let (_dir, engine, photos, _video) = rotation_batch_library("rotate_palette_viewer");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_rotation_viewer(&view, vcx, &engine, photos[0]);

    keystrokes(vcx, "cmd-k");
    type_text(vcx, "rotate right");
    keystrokes(vcx, "enter");
    assert_eq!(
        rotations(&engine, &photos),
        vec![1, 0, 0],
        "⌘K Rotate Right must turn the open photo clockwise"
    );
    view.update(vcx, |app, _| {
        assert!(
            !app.command_palette_open,
            "running a command closes the palette"
        );
        assert!(app.is_fullscreen_viewer);
    });
}

/// Caches a placeholder thumbnail for `id`, as if painted at its angle.
fn put_test_texture(
    view: &gpui::Entity<LuminaApp>,
    vcx: &mut gpui::VisualTestContext,
    id: lumina_core::MediaId,
) {
    let texture = std::sync::Arc::new(gpui::RenderImage::new(vec![image::Frame::new(
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

/// Asserts nothing was rotated and no undo step was recorded.
fn assert_not_rotated(
    view: &gpui::Entity<LuminaApp>,
    vcx: &mut gpui::VisualTestContext,
    engine: &lumina_core::LuminaEngine,
    photos: &[lumina_core::MediaId],
    why: &str,
) {
    assert_eq!(rotations(engine, photos), vec![0, 0, 0], "{why}");
    view.update(vcx, |app, _| {
        assert!(app.undo.undo_stack.is_empty(), "{why}: no undo step");
    });
}

#[core::prelude::v1::test]
fn image_menu_rotate_is_refused_behind_sheets() {
    let (_dir, engine, photos, _video) = rotation_batch_library("rotate_menu_shield");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    keystrokes(vcx, "cmd-a");

    view.update(vcx, |app, cx| app.open_preferences(cx));
    render_ui(vcx);
    vcx.dispatch_action(crate::actions::RotateRight);
    vcx.run_until_parked();
    assert_not_rotated(
        &view,
        vcx,
        &engine,
        &photos,
        "Image > Rotate must not turn the selection hidden behind Preferences",
    );
    view.update(vcx, |app, cx| {
        app.close_preferences(cx);
        app.open_tag_manager(cx);
    });
    render_ui(vcx);
    vcx.dispatch_action(crate::actions::RotateRight);
    vcx.run_until_parked();
    assert_not_rotated(
        &view,
        vcx,
        &engine,
        &photos,
        "Image > Rotate on the Tags page has no media selection to turn",
    );

    view.update(vcx, |app, cx| {
        // The Tags page dropped the selection; select everything again.
        app.select_album(crate::views::sidebar::AlbumSelection::AllLibrary, cx);
        app.select_all(cx);
        app.import.dialog = Some(crate::app::import::ImportDialog::Planning {
            count: 1,
            checked: None,
        });
    });
    render_ui(vcx);
    vcx.dispatch_action(crate::actions::RotateRight);
    vcx.run_until_parked();
    assert_not_rotated(
        &view,
        vcx,
        &engine,
        &photos,
        "Image > Rotate must not turn the selection hidden behind Import…",
    );

    view.update(vcx, |app, _| app.import.dialog = None);
    render_ui(vcx);
    focus_app(vcx, &view);
    keystrokes(vcx, "cmd-f");
    vcx.dispatch_action(crate::actions::RotateRight);
    vcx.run_until_parked();
    assert_not_rotated(
        &view,
        vcx,
        &engine,
        &photos,
        "Image > Rotate must not act while the search field has focus",
    );

    // With nothing open the same menu action rotates the selection.
    view.update(vcx, |app, cx| app.blur_search(cx));
    vcx.dispatch_action(crate::actions::RotateRight);
    vcx.run_until_parked();
    assert_eq!(
        rotations(&engine, &photos),
        vec![1, 1, 1],
        "Image > Rotate Right turns the selection when no sheet is open"
    );
}

#[core::prelude::v1::test]
fn rotate_is_refused_in_recently_deleted() {
    let (_dir, engine, photos, _video) = rotation_batch_library("rotate_in_trash");
    engine
        .batch_set_trashed(&photos, true)
        .expect("photos must move to Recently Deleted");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    view.update(vcx, |app, cx| {
        app.select_album(crate::AlbumSelection::Trash, cx)
    });
    render_ui(vcx);
    keystrokes(vcx, "cmd-a");
    keystrokes(vcx, "r");
    view.update(vcx, |app, _| {
        assert!(
            !app.selected_indices.is_empty(),
            "⌘A selects the deleted photos"
        )
    });
    assert_not_rotated(
        &view,
        vcx,
        &engine,
        &photos,
        "R must not rotate in Recently Deleted, where the context menu hides Rotate",
    );
}

#[core::prelude::v1::test]
fn option_r_does_not_rotate_in_grid() {
    let (_dir, engine, photos, _video) = rotation_batch_library("rotate_option_r");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    keystrokes(vcx, "cmd-a");
    keystrokes(vcx, "alt-r");
    assert_not_rotated(&view, vcx, &engine, &photos, "only R and ⇧R rotate, not ⌥R");
}

#[core::prelude::v1::test]
fn r_in_text_field_does_not_rotate() {
    let (_dir, engine, photos, _video) = rotation_batch_library("rotate_r_in_search");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    keystrokes(vcx, "cmd-a");
    keystrokes(vcx, "cmd-f");
    keystrokes(vcx, "r");
    keystrokes(vcx, "shift-r");

    view.update(vcx, |app, _| {
        assert!(app.search_focused, "⌘F focuses the search field");
        assert_eq!(
            app.search_query, "rR",
            "R and ⇧R type into the search field"
        );
        assert!(
            app.undo.undo_stack.is_empty(),
            "typing must not record a rotation"
        );
    });
    assert_eq!(
        rotations(&engine, &photos),
        vec![0, 0, 0],
        "typing R in a text field must not rotate the selection"
    );
}
