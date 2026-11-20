//! Removing a library from Lumina: the catalog and thumbnails go, the files
//! stay, and the app returns to the welcome screen.

use crate::app::command_palette::CommandId;
use crate::tests::helpers::*;
use crate::LuminaApp;
use crate::*;

#[core::prelude::v1::test]
fn deleting_the_open_library_keeps_its_files_and_shows_the_welcome() {
    let (dir, engine) = seed_library("library_delete_open", 2);
    let library = engine.library_root().to_path_buf();
    let photo = engine.get_item_slice(0..1)[0].path.clone();
    let catalog = dir.join("catalog.db");
    assert!(
        catalog.exists(),
        "setup: the test catalog lives in the library"
    );

    let cache_owner = engine.clone();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let cache_dir = cache_owner.cache_dir().to_path_buf();
    std::fs::write(cache_dir.join("thumb.bin"), b"thumb").unwrap();
    let sibling = cache_owner.cache_root().join("catalog_sibling");
    std::fs::create_dir_all(&sibling).unwrap();
    std::fs::write(sibling.join("keep.bin"), b"keep").unwrap();
    vcx.simulate_resize(gpui::size(gpui::px(1280.0), gpui::px(840.0)));
    render_ui(vcx);

    view.update(vcx, |app, cx| {
        app.config.last_library_path = Some(library.clone());
        app.config.recent_libraries = vec![library.clone()];
        app.execute_command(CommandId::DeleteLibrary, cx);
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("library-delete-dialog").is_some(),
        "Delete Library must ask before removing anything"
    );

    // Esc leaves the library untouched.
    keystrokes(vcx, "escape");
    assert!(
        vcx.debug_bounds("library-delete-dialog").is_none(),
        "Esc closes the confirmation"
    );
    assert!(catalog.exists(), "cancelling never deletes the catalog");
    assert!(photo.exists());

    view.update(vcx, |app, cx| {
        app.execute_command(CommandId::DeleteLibrary, cx)
    });
    render_ui(vcx);
    keystrokes(vcx, "enter");
    render_ui(vcx);

    view.update(vcx, |app, _| {
        assert!(
            app.library_delete.is_none(),
            "confirming closes the confirmation"
        );
        assert!(
            app.grid_state.is_none(),
            "the removed library is closed and the welcome screen shows"
        );
        assert_eq!(
            app.config.last_library_path, None,
            "the removed library is no longer the startup library"
        );
        assert!(
            app.config.recent_libraries.is_empty(),
            "the removed library leaves the recents"
        );
    });
    assert!(!catalog.exists(), "the catalog is removed");
    assert!(
        !cache_dir.exists(),
        "the open library's whole thumbnail folder is removed"
    );
    assert!(
        sibling.exists(),
        "another library's thumbnail folder is left alone"
    );
    assert!(photo.exists(), "the user's photo stays where it is");
    assert!(
        library.is_dir(),
        "deleting a library never deletes its folder"
    );
    drop(view);
}

#[core::prelude::v1::test]
fn a_recent_library_can_be_deleted_from_the_welcome_screen() {
    let (dir, engine) = seed_library("library_delete_recent", 1);
    let library = engine.library_root().to_path_buf();
    let photo = engine.get_item_slice(0..1)[0].clone();

    // The catalog and thumbnail cache a closed library resolves to, kept in
    // temp dirs so the test never touches the user's data dir.
    let catalogs = dir.join("catalogs");
    let thumbs = dir.join("thumbnails");
    std::fs::create_dir_all(&catalogs).unwrap();
    let catalog_db = lumina_core::library::catalog_path_in(&catalogs, &library);
    {
        let catalog = lumina_core::catalog::Catalog::open(&catalog_db).unwrap();
        catalog
            .insert_or_update_batch(std::slice::from_ref(&photo))
            .unwrap();
    }
    let cache_id = catalog_db
        .file_stem()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let cache = lumina_core::cache::DiskCache::for_library(&thumbs, &cache_id).unwrap();
    let thumb = cache.cache_file_path(&lumina_core::cache::DiskCache::compute_key(
        &photo.path,
        photo.file_size,
        photo.mtime_epoch_ms,
        320,
    ));
    std::fs::write(&thumb, b"thumb").unwrap();

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    view.update(vcx, |app, cx| {
        app.catalog_dir_override = Some(catalogs.clone());
        app.cache_dir_override = Some(thumbs.clone());
        app.config.recent_libraries = vec![library.clone()];
        cx.notify();
    });
    render_ui(vcx);

    assert!(
        vcx.debug_bounds("welcome-recent-delete-0").is_some(),
        "the delete action is painted"
    );
    click(vcx, leak("welcome-recent-delete-0".to_string()));
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.library_delete.is_some(), "the click reached the action");
    });
    assert!(
        vcx.debug_bounds("library-delete-dialog").is_some(),
        "a recent's delete action must ask first"
    );

    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.config.recent_libraries,
            vec![library.clone()],
            "cancelling keeps the recent"
        );
    });
    assert!(catalog_db.exists(), "cancelling keeps the catalog");

    click(vcx, leak("welcome-recent-delete-0".to_string()));
    render_ui(vcx);
    keystrokes(vcx, "enter");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(
            app.config.recent_libraries.is_empty(),
            "confirming drops the library from the recents"
        );
    });
    assert!(
        !catalog_db.exists(),
        "the closed library's catalog is removed"
    );
    assert!(
        !thumbs.join(&cache_id).exists(),
        "its whole thumbnail folder is removed"
    );
    assert!(!thumb.exists(), "no cached thumbnails are left behind");
    assert!(photo.path.exists(), "the user's photo stays where it is");
    drop(view);
}

#[core::prelude::v1::test]
fn the_palette_can_delete_the_focused_recent() {
    let (_dir, engine) = seed_library("library_delete_palette", 1);
    let library = engine.library_root().to_path_buf();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    view.update(vcx, |app, cx| {
        app.config.recent_libraries = vec![library];
        cx.notify();
    });
    render_ui(vcx);
    focus_app(vcx, &view);
    // The welcome screen's keyboard focus chooses the row the palette acts on.
    keystrokes(vcx, "down");
    view.update(vcx, |app, cx| {
        assert!(
            app.welcome.focused_recent.is_some(),
            "setup: a recent is focused"
        );
        assert!(
            app.command_applies(CommandId::DeleteLibrary),
            "the palette offers Delete Library on a focused recent"
        );
        app.execute_command(CommandId::DeleteLibrary, cx);
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("library-delete-dialog").is_some(),
        "the palette asks before removing the focused recent"
    );
    drop(view);
}

#[core::prelude::v1::test]
fn delete_library_only_applies_when_one_is_open() {
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    render_ui(vcx);
    view.update(vcx, |app, cx| {
        assert!(
            !app.command_applies(CommandId::DeleteLibrary),
            "the welcome screen has no open library to delete"
        );
        app.execute_command(CommandId::DeleteLibrary, cx);
        assert!(
            app.library_delete.is_none(),
            "running it without a library asks nothing"
        );
    });
}

#[core::prelude::v1::test]
fn settings_lists_known_libraries_and_deletes_one() {
    use crate::app::preferences::PreferencesTab;

    let (dir, engine) = seed_library("settings_delete", 1);
    let library = engine.library_root().to_path_buf();
    let photo = engine.get_item_slice(0..1)[0].path.clone();
    let catalog = dir.join("catalog.db");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    render_ui(vcx);

    view.update(vcx, |app, cx| {
        app.config.recent_libraries = vec![library.clone()];
        app.open_preferences_at(PreferencesTab::Library, cx);
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("settings-known-library-0").is_some(),
        "Settings › Library lists the known library"
    );
    assert!(
        vcx.debug_bounds("settings-remove-library-0").is_some(),
        "each known library has a Remove action"
    );

    click(vcx, "settings-remove-library-0");
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("library-delete-dialog").is_some(),
        "Remove asks before deleting the catalog"
    );
    keystrokes(vcx, "enter");
    render_ui(vcx);

    view.update(vcx, |app, _| {
        assert!(app.library_delete.is_none());
        assert!(
            app.grid_state.is_none(),
            "the removed library closes to the welcome screen"
        );
        assert!(
            !app.preferences_open,
            "Settings closes rather than showing an empty Library tab"
        );
    });
    assert!(!catalog.exists(), "the catalog is removed");
    assert!(photo.exists(), "the user's photo stays");
    drop(view);
}
