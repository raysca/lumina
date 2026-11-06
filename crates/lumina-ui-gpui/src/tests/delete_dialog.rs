//! v3 Delete dialog (FIXES §6): every delete from the library asks first,
//! with Move to Trash (undoable) preselected.

use crate::tests::ui_automation::{
    app_with_library, click, focus_app, keystrokes, leak, render_ui, right_click, seed_library,
};
use crate::{DeleteIntent, LuminaApp};
use gpui::{Entity, VisualTestContext};
use lumina_core::{DeletionTarget, MediaItem};

fn first(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> MediaItem {
    keystrokes(vcx, "home");
    view.update(vcx, |app, _| app.selected_item.clone().unwrap())
}

fn target(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> Option<DeletionTarget> {
    view.update(vcx, |app, _| {
        app.delete_confirm_state.as_ref().map(|s| s.target)
    })
}

#[core::prelude::v1::test]
fn backspace_asks_first_then_enter_moves_to_recently_deleted() {
    let (dir, engine) = seed_library("del_backspace", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let item = first(&view, vcx);

    keystrokes(vcx, "backspace");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        let st = app
            .delete_confirm_state
            .as_ref()
            .expect("⌫ opens the dialog");
        assert_eq!(st.intent, DeleteIntent::LibrarySelection);
        assert_eq!(
            st.target,
            DeletionTarget::SystemTrash,
            "Move to Trash is preselected"
        );
        assert_eq!((st.photos, st.videos), (1, 0));
    });
    assert!(
        !engine.get_item_by_id(item.id).unwrap().is_trashed,
        "nothing is trashed until the dialog is confirmed"
    );
    for selector in [
        "delete-title",
        "delete-summary",
        "opt-system-trash",
        "opt-catalog-only",
        "opt-disk-permanent",
        "cancel-delete-btn",
        "confirm-delete-btn",
        "delete-hint",
    ] {
        assert!(
            vcx.debug_bounds(selector).is_some(),
            "{selector} must render"
        );
    }
    let dialog = vcx.debug_bounds("delete-confirm-dialog").unwrap();
    let confirm = vcx.debug_bounds("confirm-delete-btn").unwrap();
    let cancel = vcx.debug_bounds("cancel-delete-btn").unwrap();
    assert!(
        cancel.right() <= confirm.left() && f32::from(dialog.right() - confirm.right()) < 24.0,
        "Cancel then the primary button, right-aligned: {cancel:?} {confirm:?} {dialog:?}"
    );

    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert!(app.delete_confirm_state.is_none(), "↵ confirms and closes");
        assert!(app.error_message.is_none());
    });
    assert!(
        engine.get_item_by_id(item.id).unwrap().is_trashed,
        "Move to Trash from the library is Lumina's soft trash"
    );
    assert!(item.path.exists(), "the file stays on disk");

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn escape_and_cancel_close_without_changes() {
    let (dir, engine) = seed_library("del_cancel", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let item = first(&view, vcx);

    keystrokes(vcx, "backspace escape");
    assert!(target(&view, vcx).is_none(), "Esc cancels");
    keystrokes(vcx, "backspace");
    render_ui(vcx);
    click(vcx, "cancel-delete-btn");
    assert!(target(&view, vcx).is_none(), "Cancel closes");
    assert!(!engine.get_item_by_id(item.id).unwrap().is_trashed);

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn context_menu_opens_dialog_and_remove_from_library_keeps_the_file() {
    let (dir, engine) = seed_library("del_menu", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let item = first(&view, vcx);

    right_click(vcx, leak(format!("media-card-{}", item.id.0)));
    render_ui(vcx);
    click(vcx, "trash-menu-btn");
    render_ui(vcx);
    assert!(target(&view, vcx).is_some(), "Delete… opens the dialog");

    keystrokes(vcx, "down");
    assert_eq!(
        target(&view, vcx),
        Some(DeletionTarget::CatalogOnly),
        "↓ picks the next option"
    );
    keystrokes(vcx, "up up");
    assert_eq!(
        target(&view, vcx),
        Some(DeletionTarget::SystemTrash),
        "↑ stops at the first"
    );
    click(vcx, "opt-catalog-only");
    click(vcx, "confirm-delete-btn");

    assert!(target(&view, vcx).is_none());
    assert!(
        engine.get_item_by_id(item.id).is_none(),
        "Remove from Library drops the catalog row"
    );
    assert!(item.path.exists(), "…and keeps the file on disk");

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn permanent_delete_needs_the_acknowledgement() {
    let (dir, engine) = seed_library("del_permanent", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let item = first(&view, vcx);

    keystrokes(vcx, "backspace down down");
    assert_eq!(target(&view, vcx), Some(DeletionTarget::PermanentDisk));
    render_ui(vcx);
    assert!(vcx.debug_bounds("delete-permanent-ack-row").is_some());

    keystrokes(vcx, "enter");
    assert!(
        target(&view, vcx).is_some(),
        "↵ without the acknowledgement must not delete or close"
    );
    assert!(item.path.exists());

    keystrokes(vcx, "space enter");
    assert!(target(&view, vcx).is_none());
    assert!(engine.get_item_by_id(item.id).is_none());
    assert!(!item.path.exists(), "Delete permanently removes the file");

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn dock_delete_asks_for_the_whole_selection() {
    let (dir, engine) = seed_library("del_dock", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    keystrokes(vcx, "home secondary-a");
    render_ui(vcx);
    focus_app(vcx, &view);
    click(vcx, "batch-trash-dock-btn");
    view.update(vcx, |app, _| {
        let st = app
            .delete_confirm_state
            .as_ref()
            .expect("dock Delete opens the dialog");
        assert_eq!(st.item_count, 3, "the dialog covers every selected item");
        assert_eq!(st.preview_ids.len(), 3, "up to three header thumbnails");
    });
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| assert_eq!(app.get_active_count(), 0));

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn single_delete_moves_to_the_next_item_and_undoes() {
    let (dir, engine) = seed_library("del_next", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let item = first(&view, vcx);

    keystrokes(vcx, "backspace enter");
    view.update(vcx, |app, _| {
        let next = app
            .selected_item
            .as_ref()
            .expect("selection stays on the grid");
        assert_ne!(next.id, item.id, "⌫↵ selects the next item for the next ⌫");
        assert_eq!(app.selected_index, Some(0));
    });
    keystrokes(vcx, "secondary-z");
    assert!(
        !engine.get_item_by_id(item.id).unwrap().is_trashed,
        "⌘Z restores a library delete"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn trash_view_keeps_the_macos_trash_meaning() {
    let (dir, engine) = seed_library("del_trash_view", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let item = first(&view, vcx);
    engine.set_trashed(item.id, true).unwrap();
    view.update(vcx, |app, cx| {
        app.refresh_album_counts();
        app.select_album(crate::AlbumSelection::Trash, cx)
    });
    render_ui(vcx);
    view.update(vcx, |app, cx| app.select_index(0, cx));
    keystrokes(vcx, "backspace");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        let st = app.delete_confirm_state.as_ref().expect("⌫ in Trash asks");
        assert_eq!(st.intent, DeleteIntent::DeleteSelected);
    });
    assert!(
        vcx.debug_bounds("delete-hint").is_none(),
        "the Recently Deleted hint is for library deletes only"
    );
    // ponytail: not confirmed here, it would move a file to the real ~/.Trash.
    keystrokes(vcx, "escape");

    let _ = std::fs::remove_dir_all(&dir);
}
