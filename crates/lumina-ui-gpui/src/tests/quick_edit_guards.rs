//! Edit mode's guards: absorbed keys, menu actions behind sheets and in
//! edit mode, undo under a draft, videos, and the export note.

use crate::tests::quick_edit::{edit_library, edit_rows, editing, edits_of, open_viewer_on};
use crate::tests::ui_automation::{app_with_library, keystrokes, render_ui};
use crate::*;
use lumina_core::{Edits, MediaId};

#[core::prelude::v1::test]
fn edit_mode_absorbs_delete_shortcut() {
    let (dir, engine, photos, _) = edit_library("absorbs_delete");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_viewer_on(&view, vcx, &engine, photos[0]);

    keystrokes(vcx, "e");
    keystrokes(vcx, "secondary-backspace");
    keystrokes(vcx, "backspace");
    keystrokes(vcx, "right");
    crate::tests::ui_automation::right_click(vcx, "viewer-viewport");

    let item = engine
        .get_item_by_id(photos[0])
        .expect("item still cataloged");
    assert!(!item.is_trashed, "⌘⌫ in edit mode must not trash the photo");
    view.update(vcx, |app, _| {
        assert!(
            app.delete_confirm_state.is_none(),
            "⌘⌫ in edit mode must not open the Delete dialog"
        );
        assert!(
            app.context_menu_state.is_none(),
            "right-click in edit mode must not open the menu with Delete"
        );
        assert!(app.edits.session.is_some(), "edit mode stays open");
        assert_eq!(
            app.selected_item.as_ref().map(|item| item.id),
            Some(photos[0]),
            "→ in edit mode must not step to the next photo"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn e_on_video_does_nothing() {
    let (dir, engine, _, clip) = edit_library("video");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_viewer_on(&view, vcx, &engine, clip);

    keystrokes(vcx, "e");
    assert!(!editing(&view, vcx), "E on a video does not open edit mode");
    assert!(vcx.debug_bounds("edit-bar").is_none());
    assert_eq!(edit_rows(&dir, clip), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn image_menu_edit_actions_are_guarded() {
    let (dir, engine, photos, _) = edit_library("menu_guard");
    let exposed = Edits {
        exposure_ev: 1.0,
        ..Edits::default()
    };
    engine
        .set_edits(&[(photos[0], exposed)])
        .expect("seed edits");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_viewer_on(&view, vcx, &engine, photos[0]);

    view.update(vcx, |app, cx| app.open_preferences(cx));
    render_ui(vcx);
    vcx.dispatch_action(crate::actions::EditPhoto);
    vcx.dispatch_action(crate::actions::RevertToOriginal);
    vcx.run_until_parked();
    assert!(
        !editing(&view, vcx),
        "Image > Edit Photo must not open edit mode behind Preferences"
    );
    assert_eq!(
        edits_of(&engine, photos[0]),
        exposed,
        "Image > Revert to Original must not act behind Preferences"
    );

    // In edit mode the menu works on the draft, never the catalog.
    view.update(vcx, |app, cx| app.close_preferences(cx));
    render_ui(vcx);
    vcx.dispatch_action(crate::actions::EditPhoto);
    vcx.run_until_parked();
    assert!(editing(&view, vcx), "Image > Edit Photo opens edit mode");
    vcx.dispatch_action(crate::actions::RotateRight);
    vcx.dispatch_action(crate::actions::RevertToOriginal);
    vcx.run_until_parked();
    assert_eq!(
        edits_of(&engine, photos[0]),
        exposed,
        "menu actions in edit mode must not write the catalog"
    );
    view.update(vcx, |app, _| {
        let draft = app.edits.session.as_ref().expect("still editing").draft;
        assert_eq!(
            draft,
            Edits::default(),
            "Revert to Original resets the draft"
        );
        assert!(app.undo.undo_stack.is_empty());
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn export_report_notes_edits_are_not_included() {
    let (dir, engine, photos, _) = edit_library("export_note");
    engine
        .set_rotations(&[(photos[0], 1)])
        .expect("seed rotation");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    view.update(vcx, |app, cx| app.reload_edited_ids(cx));
    render_ui(vcx);

    let export = |id: MediaId, folder: &str, vcx: &mut gpui::VisualTestContext| {
        let item = engine.get_item_by_id(id).expect("photo");
        let destination = dir.join(folder);
        view.update(vcx, |app, cx| {
            app.selected_item = Some(item);
            app.selected_indices.clear();
            app.export_to_folder(destination, cx);
        });
        render_ui(vcx);
        view.update(vcx, |app, _| app.last_action.clone().unwrap_or_default())
    };
    let edited = export(photos[0], "out_edited", vcx);
    assert!(
        edited.starts_with("Exported 1") && edited.contains("without Lumina's edits"),
        "exporting an edited photo says the edits stay behind: {edited:?}"
    );
    let plain = export(photos[1], "out_plain", vcx);
    assert!(
        plain.starts_with("Exported 1") && !plain.contains("without Lumina's edits"),
        "an unedited export has no note: {plain:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn undo_waits_while_edit_mode_is_open() {
    let (dir, engine, photos, _) = edit_library("undo_waits");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_viewer_on(&view, vcx, &engine, photos[0]);
    keystrokes(vcx, "e =");
    keystrokes(vcx, "enter");
    keystrokes(vcx, "e");

    // The toast's Undo button calls this directly, past the key shield.
    view.update(vcx, |app, cx| app.undo_last(cx));
    assert!(
        edits_of(&engine, photos[0]).exposure_ev > 0.0,
        "undo must not change the catalog under an open draft"
    );
    view.update(vcx, |app, _| {
        assert_eq!(app.undo.undo_stack.len(), 1, "the step stays undoable");
    });
    let _ = std::fs::remove_dir_all(&dir);
}

/// The draft after E and one `=`; asserts edit mode is still open.
fn assert_draft_intact(
    view: &gpui::Entity<LuminaApp>,
    vcx: &mut gpui::VisualTestContext,
    why: &str,
) {
    view.update(vcx, |app, _| {
        let session = app
            .edits
            .session
            .as_ref()
            .unwrap_or_else(|| panic!("{why}: edit mode closed"));
        assert!(
            (session.draft.exposure_ev - 1.0 / 3.0).abs() < 1e-3,
            "{why}: the draft changed"
        );
    });
}

#[core::prelude::v1::test]
fn menu_actions_do_not_open_or_act_in_edit_mode() {
    let (dir, engine, photos, _) = edit_library("menu_in_edit_mode");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_viewer_on(&view, vcx, &engine, photos[0]);
    keystrokes(vcx, "e =");

    vcx.dispatch_action(crate::actions::ShowCommandPalette);
    vcx.dispatch_action(crate::actions::OpenPreferences);
    vcx.dispatch_action(crate::actions::ImportXmp);
    vcx.dispatch_action(crate::actions::ManageTemplates);
    vcx.dispatch_action(crate::actions::NewSmartAlbum);
    vcx.dispatch_action(crate::actions::ToggleDownloads);
    vcx.dispatch_action(crate::actions::GoBack);
    render_ui(vcx);
    view.update(vcx, |app, cx| {
        // Other paths to the two overlays refuse too.
        app.open_command_palette(cx);
        app.open_preferences(cx);
        assert!(!app.command_palette_open, "no palette over edit mode");
        assert!(!app.preferences_open, "no Preferences over edit mode");
        assert!(app.jobs.xmp.is_none(), "no XMP import under the draft");
        assert!(app.templates.editor.is_none(), "no template sheet");
        assert!(!app.album_draft.modal_open, "no smart album sheet");
        assert!(!app.downloads.open, "no downloads list");
        assert!(
            app.is_fullscreen_viewer,
            "Go > Back must not leave the viewer"
        );
        assert_eq!(
            app.selected_item.as_ref().map(|item| item.id),
            Some(photos[0]),
            "the photo under the draft stays shown"
        );
    });
    assert_draft_intact(&view, vcx, "menu actions in edit mode");
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn changing_the_viewer_item_drops_the_draft() {
    let (dir, engine, photos, _) = edit_library("item_changes");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_viewer_on(&view, vcx, &engine, photos[0]);
    keystrokes(vcx, "e =");

    // As trashing the photo or a watcher removal steps the viewer on.
    let other = engine.get_item_by_id(photos[1]).expect("photo");
    view.update(vcx, |app, cx| app.select_item(other, cx));
    assert!(
        !editing(&view, vcx),
        "a draft must not follow the viewer to another photo"
    );
    view.update(vcx, |app, _| {
        assert_eq!(
            app.last_action.as_deref(),
            Some("Edits discarded"),
            "dropping a draft says so"
        )
    });
    keystrokes(vcx, "enter");
    for id in &photos {
        assert_eq!(edit_rows(&dir, *id), 0, "nothing is written to any photo");
        assert_eq!(edits_of(&engine, *id), Edits::default());
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn clearing_the_selection_drops_the_draft() {
    let (dir, engine, photos, _) = edit_library("selection_cleared");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_viewer_on(&view, vcx, &engine, photos[0]);
    keystrokes(vcx, "e =");
    // As a count refresh drops a selection that left the view (a watcher
    // removal), without going through select_item.
    view.update(vcx, |app, _| app.clear_selection_fields());
    assert!(
        !editing(&view, vcx),
        "a draft must not outlive the photo it edits"
    );
    keystrokes(vcx, "enter");
    assert_eq!(edit_rows(&dir, photos[0]), 0, "nothing is written");
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn closing_the_viewer_discards_and_says_so() {
    let (dir, engine, photos, _) = edit_library("close_discards");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_viewer_on(&view, vcx, &engine, photos[0]);
    keystrokes(vcx, "e =");
    view.update(vcx, |app, cx| app.close_viewer(cx));
    view.update(vcx, |app, _| {
        assert!(
            app.edits.session.is_none(),
            "closing the viewer ends edit mode"
        );
        assert_eq!(app.last_action.as_deref(), Some("Edits discarded"));
    });
    assert_eq!(edit_rows(&dir, photos[0]), 0, "nothing is saved");
    let _ = std::fs::remove_dir_all(&dir);
}
