//! Adding to and removing from albums (plan 5, Task A3): the picker (⇧⌘A,
//! context menu, palette), ⌫ in an album view, and their undo. Every
//! data-changing test reads the catalog rows back.

use super::albums::{album_with_members, catalog_albums, member_ids};
use super::ui_automation::{
    app_with_library, click, focus_app, keystrokes, leak, render_ui, right_click, seed_library,
    type_text,
};
use crate::views::sidebar::AlbumSelection;
use gpui::{Entity, VisualTestContext};
use lumina_core::{LuminaEngine, MediaId, SortSpec};

fn members(engine: &LuminaEngine, album: i64) -> Vec<MediaId> {
    let mut ids = member_ids(engine, album, SortSpec::default());
    ids.sort_by_key(|id| id.0);
    ids
}

fn sorted(mut ids: Vec<MediaId>) -> Vec<MediaId> {
    ids.sort_by_key(|id| id.0);
    ids
}

fn selected_ids(view: &Entity<crate::LuminaApp>, vcx: &mut VisualTestContext) -> Vec<MediaId> {
    view.update(vcx, |app, _| {
        sorted(app.get_selected_items().iter().map(|i| i.id).collect())
    })
}

fn toast(view: &Entity<crate::LuminaApp>, vcx: &mut VisualTestContext) -> Option<String> {
    view.update(vcx, |app, _| app.toast.current().map(|t| t.message.clone()))
}

fn picker_open(view: &Entity<crate::LuminaApp>, vcx: &mut VisualTestContext) -> bool {
    view.update(vcx, |app, _| app.albums.picker.is_some())
}

#[core::prelude::v1::test]
fn add_to_album_via_keyboard_picker() {
    let (dir, engine) = seed_library("picker_keys", 3);
    let kyoto = engine.create_album("Kyoto").unwrap();
    let paris = engine.create_album("Paris").unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    keystrokes(vcx, "right");
    keystrokes(vcx, "shift-right");
    let chosen = selected_ids(&view, vcx);
    assert_eq!(chosen.len(), 2);

    keystrokes(vcx, "cmd-shift-a");
    assert!(picker_open(&view, vcx), "⇧⌘A opens the picker");
    assert!(
        vcx.debug_bounds("album-picker").is_some(),
        "its card is drawn"
    );
    assert!(vcx.debug_bounds("album-picker-backdrop").is_some());
    assert!(vcx.debug_bounds("album-picker-new").is_some());
    let kyoto_row = leak(format!("album-picker-row-{}", kyoto.id));
    let paris_row = leak(format!("album-picker-row-{}", paris.id));
    assert!(vcx.debug_bounds(kyoto_row).is_some());

    // Typing filters; ↵ adds to the highlighted (first shown) album.
    type_text(vcx, "par");
    assert!(
        vcx.debug_bounds(kyoto_row).is_none(),
        "the filter hides albums whose name does not match"
    );
    assert!(vcx.debug_bounds(paris_row).is_some());
    keystrokes(vcx, "enter");
    assert_eq!(members(&engine, paris.id), chosen, "↵ adds the selection");
    assert!(members(&engine, kyoto.id).is_empty());
    assert!(!picker_open(&view, vcx), "↵ closes the picker");
    assert_eq!(
        toast(&view, vcx).as_deref(),
        Some("Added 2 items to “Paris”"),
        "the toast says what was added where"
    );

    // ↓ moves the highlight: Kyoto, Paris, then New Album….
    keystrokes(vcx, "cmd-shift-a");
    keystrokes(vcx, "down");
    keystrokes(vcx, "up");
    keystrokes(vcx, "enter");
    assert_eq!(
        members(&engine, kyoto.id),
        chosen,
        "↓ then ↑ is Kyoto again"
    );

    keystrokes(vcx, "cmd-shift-a");
    keystrokes(vcx, "down down");
    keystrokes(vcx, "enter");
    let albums = catalog_albums(&engine);
    assert_eq!(albums.len(), 3, "New Album… makes an album: {albums:?}");
    assert_eq!(albums[2].1, 2, "holding the selection");
    view.update(vcx, |app, _| {
        assert!(app.albums.picker.is_none());
        assert!(app.albums.renaming.is_some(), "and starts in rename mode");
    });
    keystrokes(vcx, "escape");

    // With no match, ↵ is New Album…, named after the typed filter.
    keystrokes(vcx, "cmd-shift-a");
    type_text(vcx, "  Lisbon ");
    keystrokes(vcx, "enter");
    let albums = catalog_albums(&engine);
    assert_eq!(
        albums.last(),
        Some(&("Lisbon".to_string(), 2)),
        "the trimmed filter names the new album: {albums:?}"
    );
    view.update(vcx, |app, _| {
        assert!(app.albums.renaming.is_some(), "still in rename mode");
        assert_eq!(app.albums.rename_buffer, "Lisbon");
    });
    keystrokes(vcx, "escape");

    // Esc closes without adding.
    keystrokes(vcx, "cmd-shift-a");
    keystrokes(vcx, "escape");
    assert!(!picker_open(&view, vcx), "Esc closes the picker");

    // The palette reaches the picker too.
    keystrokes(vcx, "cmd-k");
    type_text(vcx, "add to album");
    keystrokes(vcx, "enter");
    assert!(picker_open(&view, vcx), "Add to Album… in the palette");
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn context_menu_add_to_album() {
    let (dir, engine) = seed_library("picker_context", 2);
    let album = engine.create_album("Trip").unwrap();
    let first = engine.get_item_slice(0..1)[0].id;
    let card = leak(format!("media-card-{}", first.0));
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    right_click(vcx, card);
    assert!(
        vcx.debug_bounds("context-remove-from-album").is_none(),
        "Remove from Album is offered only in an album"
    );
    click(vcx, "context-add-to-album");
    view.update(vcx, |app, _| {
        assert!(app.context_menu_state.is_none(), "the menu closes");
        assert!(app.albums.picker.is_some(), "and the picker opens");
    });
    click(vcx, leak(format!("album-picker-row-{}", album.id)));
    assert_eq!(members(&engine, album.id), vec![first], "a click adds");
    assert!(!picker_open(&view, vcx));

    // In the album, the menu removes.
    click(vcx, leak(format!("album-row-{}", album.id)));
    right_click(vcx, card);
    click(vcx, "context-remove-from-album");
    assert!(
        members(&engine, album.id).is_empty(),
        "removed from the album"
    );
    assert!(
        engine.get_item_by_id(first).is_some_and(|i| !i.is_trashed),
        "and still in the library"
    );

    // A click on the backdrop closes the picker.
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::AllLibrary, cx)
    });
    render_ui(vcx);
    right_click(vcx, card);
    click(vcx, "context-add-to-album");
    let card_bounds = vcx.debug_bounds("album-picker").expect("picker card");
    let corner = card_bounds.origin + gpui::point(gpui::px(-20.0), gpui::px(-20.0));
    vcx.simulate_click(corner, gpui::Modifiers::default());
    render_ui(vcx);
    assert!(!picker_open(&view, vcx), "the backdrop dismisses it");
    assert!(members(&engine, album.id).is_empty(), "without adding");
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn backspace_in_album_removes_membership_only() {
    let (dir, engine) = seed_library("album_backspace", 3);
    let (album, ids) = album_with_members(&engine, "Trip", 2);
    let total = engine.total_count();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    click(vcx, leak(format!("album-row-{}", album.id)));
    focus_app(vcx, &view);
    keystrokes(vcx, "right");
    let removed = selected_ids(&view, vcx);
    assert_eq!(removed.len(), 1);
    keystrokes(vcx, "backspace");

    view.update(vcx, |app, _| {
        assert!(
            app.delete_confirm_state.is_none(),
            "⌫ in an album never opens the Delete dialog"
        );
        assert_eq!(app.albums.counts.get(&album.id), Some(&1));
    });
    let kept: Vec<MediaId> = ids.iter().copied().filter(|id| *id != removed[0]).collect();
    assert_eq!(
        members(&engine, album.id),
        kept,
        "⌫ removes it from the album"
    );
    let item = engine
        .get_item_by_id(removed[0])
        .expect("still in the catalog");
    assert!(
        !item.is_trashed,
        "⌫ in an album never trashes (is_trashed == 0)"
    );
    assert!(item.path.exists(), "the file is still on disk");
    assert_eq!(engine.total_count(), total, "the library keeps every item");
    view.update(vcx, |app, _| {
        assert_eq!(app.get_active_count(), 1, "the album view drops it");
    });
    assert_eq!(
        toast(&view, vcx).as_deref(),
        Some("Removed 1 item from “Trip”")
    );

    // The palette offers the same in an album.
    keystrokes(vcx, "right");
    keystrokes(vcx, "cmd-k");
    type_text(vcx, "remove from album");
    keystrokes(vcx, "enter");
    assert!(members(&engine, album.id).is_empty(), "Remove from Album");
    assert_eq!(engine.total_count(), total);
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn cmd_backspace_in_album_opens_delete_dialog() {
    let (dir, engine) = seed_library("album_cmd_backspace", 2);
    let (album, ids) = album_with_members(&engine, "Trip", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    click(vcx, leak(format!("album-row-{}", album.id)));
    focus_app(vcx, &view);
    keystrokes(vcx, "right");
    keystrokes(vcx, "cmd-backspace");
    view.update(vcx, |app, _| {
        assert!(
            app.delete_confirm_state.is_some(),
            "⌘⌫ still opens the Delete dialog in an album"
        );
    });
    assert_eq!(
        members(&engine, album.id),
        sorted(ids),
        "⌘⌫ alone changes no membership"
    );
    keystrokes(vcx, "escape");
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn album_membership_undo_redo() {
    let (dir, engine) = seed_library("album_membership_undo", 3);
    // The first item is already a member: undoing the add must keep it.
    let (album, before) = album_with_members(&engine, "Trip", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    view.update(vcx, |app, cx| {
        app.select_index(0, cx);
        app.extend_selection(1, cx);
    });
    let chosen = selected_ids(&view, vcx);
    assert!(
        chosen.contains(&before[0]),
        "the member is in the selection"
    );
    keystrokes(vcx, "cmd-shift-a");
    keystrokes(vcx, "enter");
    assert_eq!(members(&engine, album.id), chosen);
    assert_eq!(
        toast(&view, vcx).as_deref(),
        Some("Added 1 item to “Trip”"),
        "only the new member counts"
    );

    keystrokes(vcx, "cmd-z");
    assert_eq!(
        members(&engine, album.id),
        before,
        "undo removes only what the add put in"
    );
    keystrokes(vcx, "cmd-shift-z");
    assert_eq!(members(&engine, album.id), chosen, "redo adds it again");

    // Remove in the album, then the toast's Undo and ⇧⌘Z.
    click(vcx, leak(format!("album-row-{}", album.id)));
    focus_app(vcx, &view);
    keystrokes(vcx, "cmd-a");
    keystrokes(vcx, "backspace");
    assert!(members(&engine, album.id).is_empty());
    click(vcx, "toast-action");
    assert_eq!(members(&engine, album.id), chosen, "Undo puts them back");
    view.update(vcx, |app, _| {
        assert_eq!(app.albums.counts.get(&album.id), Some(&2));
        assert_eq!(app.get_active_count(), 2, "the album view shows them");
    });
    focus_app(vcx, &view);
    keystrokes(vcx, "cmd-shift-z");
    assert!(members(&engine, album.id).is_empty(), "redo removes again");
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn shift_cmd_a_opens_picker_not_select_all() {
    let (dir, engine) = seed_library("shift_cmd_a", 3);
    engine.create_album("Trip").unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    keystrokes(vcx, "right");
    keystrokes(vcx, "cmd-shift-a");
    view.update(vcx, |app, _| {
        assert!(app.albums.picker.is_some(), "⇧⌘A opens the picker");
        assert_eq!(app.selected_indices.len(), 1, "⇧⌘A is not Select All");
    });
    keystrokes(vcx, "cmd-a");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_indices.len(),
            1,
            "⌘A in the picker does not reach the grid"
        );
    });
    keystrokes(vcx, "escape");
    keystrokes(vcx, "cmd-a");
    view.update(vcx, |app, _| {
        assert_eq!(app.selected_indices.len(), 3, "⌘A still selects all");
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn album_picker_absorbs_delete_and_text() {
    let (dir, engine) = seed_library("picker_absorbs", 2);
    let (album, ids) = album_with_members(&engine, "Trip", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    click(vcx, leak(format!("album-row-{}", album.id)));
    focus_app(vcx, &view);
    keystrokes(vcx, "right");
    keystrokes(vcx, "cmd-shift-a");
    // ⌫ and ⌘⌫ in the empty filter, then grid letters (F favourites, T tags).
    keystrokes(vcx, "backspace");
    keystrokes(vcx, "cmd-backspace");
    keystrokes(vcx, "delete");
    type_text(vcx, "ft");
    view.update(vcx, |app, _| {
        let picker = app.albums.picker.as_ref().expect("the picker stays open");
        assert_eq!(picker.filter, "ft", "letters go to the filter");
        assert!(app.delete_confirm_state.is_none(), "no Delete dialog");
        assert!(!app.tag_input_active, "T does not start tagging");
    });
    keystrokes(vcx, "backspace");
    view.update(vcx, |app, _| {
        assert_eq!(app.albums.picker.as_ref().unwrap().filter, "f");
    });
    assert_eq!(
        members(&engine, album.id),
        sorted(ids.clone()),
        "⌫ in the picker removes nothing from the album"
    );
    assert!(
        ids.iter().all(|id| engine
            .get_item_by_id(*id)
            .is_some_and(|i| !i.is_trashed && !i.is_favorite)),
        "nothing was trashed or favourited"
    );
    keystrokes(vcx, "escape");
    assert!(!picker_open(&view, vcx));
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn album_membership_write_failure_shows_banner() {
    let (dir, engine) = seed_library("membership_fail", 2);
    let (album, ids) = album_with_members(&engine, "Trip", 1);
    let other = engine.create_album("Other").unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let fail = |on: bool| crate::app::albums::ALBUM_WRITE_FAILS.with(|f| f.set(on));

    keystrokes(vcx, "right");
    keystrokes(vcx, "cmd-shift-a");
    type_text(vcx, "other");
    fail(true);
    keystrokes(vcx, "enter");
    fail(false);
    assert!(members(&engine, other.id).is_empty(), "nothing was added");
    view.update(vcx, |app, cx| {
        assert!(
            app.error_message
                .as_deref()
                .is_some_and(|m| m.starts_with("Could not add to album")),
            "the failure is in the banner: {:?}",
            app.error_message
        );
        assert!(app.undo.undo_stack.is_empty(), "no undo for a failed add");
        assert!(app.albums.picker.is_none(), "the banner is not hidden");
        app.dismiss_error(cx);
    });

    click(vcx, leak(format!("album-row-{}", album.id)));
    focus_app(vcx, &view);
    keystrokes(vcx, "right");
    fail(true);
    keystrokes(vcx, "backspace");
    fail(false);
    assert_eq!(members(&engine, album.id), ids, "the member stays");
    view.update(vcx, |app, _| {
        assert!(app
            .error_message
            .as_deref()
            .is_some_and(|m| m.starts_with("Could not remove from album")));
        assert!(app.undo.undo_stack.is_empty());
        assert_eq!(app.selected_indices.len(), 1, "the selection stays");
        assert_eq!(app.albums.counts.get(&album.id), Some(&1));
    });
    let _ = std::fs::remove_dir_all(&dir);
}
