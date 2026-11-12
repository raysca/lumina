//! Deleting albums and album write failures (plan 5, Task A2), plus the
//! guards that keep album actions from running under a photo edit or a
//! sheet. Data-changing tests read the catalog rows back.

use super::albums::{album_with_members, catalog_albums, member_ids, only_album};
use super::ui_automation::{
    app_with_library, click, focus_app, keystrokes, leak, render_ui, seed_library, type_text,
};
use crate::views::sidebar::AlbumSelection;
use lumina_core::SortSpec;

#[core::prelude::v1::test]
fn album_write_failure_shows_banner() {
    let (dir, engine) = seed_library("album_write_fail", 1);
    let album = engine.create_album("Kept").unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let fail = |on: bool| crate::app::albums::ALBUM_WRITE_FAILS.with(|f| f.set(on));

    // Create.
    fail(true);
    keystrokes(vcx, "cmd-n");
    fail(false);
    assert_eq!(
        catalog_albums(&engine),
        vec![("Kept".to_string(), 0)],
        "no album was made"
    );
    view.update(vcx, |app, cx| {
        assert!(
            app.error_message
                .as_deref()
                .is_some_and(|m| m.starts_with("Could not create album")),
            "the failure is in the banner: {:?}",
            app.error_message
        );
        assert_eq!(
            app.albums.list.len(),
            1,
            "the sidebar lists no phantom album"
        );
        assert!(
            app.albums.renaming.is_none(),
            "no rename field for an album that is not there"
        );
        app.dismiss_error(cx);
    });

    // Rename.
    click(vcx, leak(format!("album-rename-{}", album.id)));
    keystrokes(vcx, "cmd-a");
    type_text(vcx, "Lost");
    fail(true);
    keystrokes(vcx, "enter");
    fail(false);
    assert_eq!(
        only_album(&engine).title,
        "Kept",
        "the catalog keeps the name"
    );
    view.update(vcx, |app, cx| {
        assert!(app
            .error_message
            .as_deref()
            .is_some_and(|m| m.starts_with("Could not rename album")));
        assert_eq!(
            app.albums.list[0].title, "Kept",
            "the sidebar keeps the name"
        );
        assert!(
            app.undo.undo_stack.is_empty(),
            "a failed rename records no undo"
        );
        assert_eq!(
            app.albums.renaming,
            Some(album.id),
            "the field stays open so the name can be saved again"
        );
        assert_eq!(app.albums.rename_buffer, "Lost", "with the typed name");
        app.dismiss_error(cx);
    });
    keystrokes(vcx, "escape");

    // Delete.
    click(vcx, leak(format!("album-delete-{}", album.id)));
    fail(true);
    keystrokes(vcx, "enter");
    fail(false);
    assert_eq!(catalog_albums(&engine).len(), 1, "the album is still there");
    view.update(vcx, |app, _| {
        assert!(app
            .error_message
            .as_deref()
            .is_some_and(|m| m.starts_with("Could not delete album")));
        assert_eq!(app.albums.list.len(), 1, "the sidebar still lists it");
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn delete_album_confirms_and_undo_restores_members() {
    let (dir, engine) = seed_library("album_delete_undo", 3);
    let (album, ids) = album_with_members(&engine, "Trip", 2);
    let items = engine.total_count();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::Album(album.id), cx)
    });
    render_ui(vcx);
    let delete = leak(format!("album-delete-{}", album.id));

    click(vcx, delete);
    assert!(
        vcx.debug_bounds("album-delete-dialog").is_some(),
        "× asks first"
    );
    assert_eq!(catalog_albums(&engine).len(), 1, "× alone deletes nothing");
    // The dialog absorbs background keys: no select-all, no media delete.
    focus_app(vcx, &view);
    let selected_before = view.update(vcx, |app, _| app.selected_indices.clone());
    keystrokes(vcx, "cmd-a");
    keystrokes(vcx, "backspace");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_indices, selected_before,
            "⌘A does not reach the grid"
        );
        assert!(
            app.delete_confirm_state.is_none(),
            "⌫ does not start a media delete"
        );
    });
    keystrokes(vcx, "escape");
    assert_eq!(catalog_albums(&engine).len(), 1, "Esc keeps the album");

    click(vcx, delete);
    click(vcx, "album-delete-cancel");
    assert_eq!(catalog_albums(&engine).len(), 1, "Cancel keeps the album");

    click(vcx, delete);
    keystrokes(vcx, "enter");
    assert!(catalog_albums(&engine).is_empty(), "↵ deletes the album");
    assert_eq!(
        engine.total_count(),
        items,
        "its photos stay in the library"
    );
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::AlbumsOverview,
            "the deleted album's view falls back to the Albums overview"
        );
    });

    keystrokes(vcx, "cmd-z");
    let restored = only_album(&engine);
    assert_eq!(
        restored.id, album.id,
        "undo brings it back under the same id"
    );
    assert_eq!(
        member_ids(&engine, album.id, SortSpec::default()).len(),
        ids.len(),
        "with every member"
    );
    keystrokes(vcx, "cmd-shift-z");
    assert!(catalog_albums(&engine).is_empty(), "redo deletes it again");

    // The palette reaches the same confirmation.
    keystrokes(vcx, "cmd-z");
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::Album(album.id), cx)
    });
    keystrokes(vcx, "cmd-k");
    type_text(vcx, "delete album");
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.albums.confirm_delete,
            Some(album.id),
            "Delete Album… asks first"
        )
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn deleted_album_selection_falls_back_on_back_navigation() {
    let (dir, engine) = seed_library("album_back", 2);
    let (album, _) = album_with_members(&engine, "Trip", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    click(vcx, leak(format!("album-row-{}", album.id)));
    click(vcx, "sidebar-nav-favorites");
    // Deleted from the catalog behind the app (another window, say).
    engine.delete_album(album.id).unwrap();
    view.update(vcx, |app, _| app.refresh_album_counts());
    keystrokes(vcx, "cmd-[");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::AllLibrary,
            "Back to a deleted album lands on the library"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn failed_new_album_with_selection_leaves_no_album() {
    let (dir, engine) = seed_library("album_with_selection_fail", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    keystrokes(vcx, "right");
    keystrokes(vcx, "shift-right");
    crate::app::albums::ALBUM_WRITE_FAILS.with(|f| f.set(true));
    keystrokes(vcx, "cmd-k");
    type_text(vcx, "new album with selection");
    keystrokes(vcx, "enter");
    crate::app::albums::ALBUM_WRITE_FAILS.with(|f| f.set(false));
    assert!(
        catalog_albums(&engine).is_empty(),
        "no album, empty or full"
    );
    view.update(vcx, |app, _| {
        assert!(app
            .error_message
            .as_deref()
            .is_some_and(|m| m.starts_with("Could not create album")));
        assert!(app.albums.list.is_empty() && app.albums.renaming.is_none());
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn new_album_waits_for_photo_edits_and_sheets() {
    use crate::tests::quick_edit::{edit_library, open_viewer_on};
    let (dir, engine, photos, _) = edit_library("album_in_edit_mode");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_viewer_on(&view, vcx, &engine, photos[0]);
    keystrokes(vcx, "e =");

    // File > New Album has no handler in edit mode (a disabled item); the
    // methods themselves refuse too, for every other path.
    vcx.dispatch_action(crate::actions::NewAlbum);
    render_ui(vcx);
    view.update(vcx, |app, cx| {
        app.new_album(cx);
        app.new_album_with_selection(cx);
        let session = app
            .edits
            .session
            .as_ref()
            .expect("New Album must not close edit mode");
        assert!(
            (session.draft.exposure_ev - 1.0 / 3.0).abs() < 1e-3,
            "the unsaved draft survives"
        );
        assert!(app.is_fullscreen_viewer, "the viewer stays open");
        assert!(app.albums.renaming.is_none(), "no rename field");
    });
    assert!(
        catalog_albums(&engine).is_empty(),
        "no album during an edit"
    );

    // Under a sheet the name field would be hidden behind it.
    keystrokes(vcx, "escape");
    view.update(vcx, |app, cx| {
        app.discard_edit_draft();
        app.close_viewer(cx);
        app.open_album_builder(cx);
    });
    render_ui(vcx);
    vcx.dispatch_action(crate::actions::NewAlbum);
    render_ui(vcx);
    assert!(
        catalog_albums(&engine).is_empty(),
        "no album under the smart album sheet"
    );
    view.update(vcx, |app, _| {
        assert!(app.album_draft.modal_open);
        assert!(app.albums.renaming.is_none(), "no hidden name field");
    });
    assert!(vcx.debug_bounds("album-rename-field").is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn opening_another_library_resets_album_state() {
    let (dir, engine) = seed_library("album_switch", 1);
    let (other, _other_engine) = seed_library("album_switch_other", 1);
    let album = engine.create_album("Trip").unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::Album(album.id), cx);
        app.start_album_rename(album.id, cx);
        app.select_album(AlbumSelection::AllLibrary, cx);
        app.select_index(0, cx);
        app.open_album_picker(cx);
        assert!(app.albums.picker.is_some(), "the picker is open");
        app.albums.confirm_delete = Some(album.id);
        app.open_library_inner(other.clone(), cx);
        assert!(
            app.albums.picker.is_none(),
            "the picker holds old ids and closes on a switch"
        );
        assert!(app.albums.renaming.is_none(), "no rename carries over");
        assert!(app.albums.rename_buffer.is_empty());
        assert!(
            app.albums.confirm_delete.is_none(),
            "ids restart per library"
        );
        assert!(
            app.albums.list.is_empty(),
            "the other library has no albums"
        );
        assert_eq!(app.selection, AlbumSelection::AllLibrary);
    });
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&other);
}

#[core::prelude::v1::test]
fn album_rename_and_delete_wait_for_photo_edits_and_sheets() {
    use crate::tests::quick_edit::{edit_library, open_viewer_on};
    let (dir, engine, photos, _) = edit_library("album_rename_in_edit_mode");
    let album = engine.create_album("Trip").unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_viewer_on(&view, vcx, &engine, photos[0]);
    keystrokes(vcx, "e =");

    view.update(vcx, |app, cx| {
        app.new_album(cx);
        app.new_album_with_selection(cx);
        app.start_album_rename(album.id, cx);
        app.prompt_delete_album(album.id, cx);
        app.open_album_picker(cx);
        assert!(
            app.albums.picker.is_none(),
            "no picker in edit mode (it would take the keys from the draft)"
        );
        let session = app
            .edits
            .session
            .as_ref()
            .expect("rename or delete must not close edit mode");
        assert!(
            (session.draft.exposure_ev - 1.0 / 3.0).abs() < 1e-3,
            "the unsaved draft survives"
        );
        assert!(
            app.albums.renaming.is_none(),
            "no rename field in edit mode"
        );
        assert!(
            app.albums.confirm_delete.is_none(),
            "no dialog in edit mode"
        );
    });

    keystrokes(vcx, "escape");
    view.update(vcx, |app, cx| {
        app.discard_edit_draft();
        app.close_viewer(cx);
        app.select_index(0, cx);
        app.open_album_builder(cx);
        app.open_album_picker(cx);
        assert!(
            app.albums.picker.is_none(),
            "no picker under the smart album sheet"
        );
        app.new_album(cx);
        app.new_album_with_selection(cx);
        app.start_album_rename(album.id, cx);
        app.prompt_delete_album(album.id, cx);
        assert!(app.albums.renaming.is_none(), "no hidden rename field");
        assert!(
            app.albums.confirm_delete.is_none(),
            "no dialog under the smart album sheet"
        );
    });
    assert_eq!(only_album(&engine).title, "Trip", "the album is untouched");
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn opening_another_library_clears_undo_history() {
    let (dir, engine) = seed_library("album_switch_undo", 1);
    let (other, other_engine) = seed_library("album_switch_undo_other", 1);
    let album = engine.create_album("Trip").unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.prompt_delete_album(album.id, cx);
        app.confirm_delete_album(cx);
        assert!(!app.undo.undo_stack.is_empty(), "the delete is undoable");
        app.open_library_inner(other.clone(), cx);
        assert!(
            app.undo.undo_stack.is_empty() && app.undo.redo_stack.is_empty(),
            "undo history belongs to the library it was made in"
        );
        app.undo_last(cx);
    });
    assert!(
        other_engine.list_albums_with_counts().is_empty(),
        "⌘Z after a switch must not restore the old library's album"
    );
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&other);
}

#[core::prelude::v1::test]
fn opening_another_library_closes_dialogs_holding_old_ids() {
    let (dir, engine) = seed_library("switch_dialogs", 2);
    let (other, other_engine) = seed_library("switch_dialogs_other", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.select_index(0, cx);
        app.prompt_delete_selected(cx);
        assert!(
            app.delete_confirm_state.is_some(),
            "the Delete dialog is open"
        );
        // A smart-album sheet editing some id, and the tag manager.
        app.album_draft.modal_open = true;
        app.album_draft.editing_album_id = Some(1);
        app.open_tag_manager(cx);
        app.open_library_inner(other.clone(), cx);
        assert!(
            app.delete_confirm_state.is_none(),
            "a Delete dialog must not outlive its library"
        );
        assert!(
            !app.album_draft.modal_open && app.album_draft.editing_album_id.is_none(),
            "the smart-album sheet closes"
        );
        assert!(!app.tags_page_shown(), "the Tags page is left");
        app.execute_confirmed_deletion(cx);
    });
    let trashed = other_engine.query_system_album_count(lumina_core::SystemAlbum::RecentlyDeleted);
    assert_eq!(trashed, 0, "nothing in the other library was deleted");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&other);
}
