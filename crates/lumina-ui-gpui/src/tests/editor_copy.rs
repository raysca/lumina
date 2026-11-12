//! ⌘E / ⌥⌘E: Edit in External Editor and Edit a Copy from the keyboard
//! (plan v1-04 Task 17). No test launches an app: the built command is
//! recorded by `external_edit::test_launches` instead.

use gpui::*;
use lumina_core::{LuminaEngine, MediaId};

use crate::app::external_edit::test_launches;
use crate::app::viewer_actions::EXTERNAL_EDITOR_UNSET_MESSAGE;
use crate::tests::quick_edit::{edit_library, open_viewer_on};
use crate::tests::ui_automation::{
    app_with_library, click, keystrokes, render_ui, right_click, type_text,
};
use crate::LuminaApp;

const EDITOR: &str = "com.apple.TextEdit";

/// Selects `id` in the grid, as a click would.
fn select(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, id: MediaId) {
    view.update(vcx, |app, cx| {
        let index = app.find_active_index(id).expect("the item is in the view");
        let item = app.get_active_slice(index..index + 1).remove(0);
        app.select_item_at(item, index, cx);
    });
    render_ui(vcx);
}

fn set_editor(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) {
    view.update(vcx, |app, _| {
        app.config.external_editor = Some(EDITOR.into())
    });
}

fn file_count(dir: &std::path::Path) -> usize {
    std::fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "jpg"))
        .count()
}

fn path_of(engine: &LuminaEngine, id: MediaId) -> String {
    engine
        .get_item_by_id(id)
        .expect("the item is cataloged")
        .path
        .to_string_lossy()
        .to_string()
}

#[core::prelude::v1::test]
fn edit_in_external_editor_by_shortcut_builds_command() {
    let (dir, engine, photos, _) = edit_library("ext_cmd_e");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    set_editor(&view, vcx);
    select(&view, vcx, photos[0]);
    test_launches::take();

    keystrokes(vcx, "secondary-e");
    vcx.run_until_parked();

    let expected = vec![
        "open".to_string(),
        "-b".to_string(),
        EDITOR.to_string(),
        path_of(&engine, photos[0]),
    ];
    assert_eq!(
        test_launches::take(),
        vec![expected.clone()],
        "⌘E opens the selected original in the configured editor"
    );
    assert_eq!(file_count(&dir), 3, "⌘E edits in place: no copy is made");

    // Image > Edit in External Editor is the same command.
    vcx.dispatch_action(crate::actions::EditInExternalEditor);
    vcx.run_until_parked();
    assert_eq!(test_launches::take(), vec![expected], "the menu action");
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn edit_copy_selects_new_item() {
    let (dir, engine, photos, _) = edit_library("ext_copy");
    engine
        .add_tags_batch(&[photos[0]], &["beach".into()])
        .expect("seed tag");
    let original = engine.get_item_by_id(photos[0]).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    set_editor(&view, vcx);
    select(&view, vcx, photos[0]);
    test_launches::take();

    keystrokes(vcx, "alt-secondary-e");
    vcx.run_until_parked();

    let selected = view.update(vcx, |app, _| {
        assert!(app.error_message.is_none(), "{:?}", app.error_message);
        app.selected_item.clone().expect("the copy is selected")
    });
    assert_ne!(selected.id, original.id, "the new item is selected");
    let stem = original.path.file_stem().unwrap().to_string_lossy();
    assert_eq!(
        selected.path,
        original.path.with_file_name(format!("{stem} (edited).jpg")),
        "the copy sits next to the original"
    );
    assert!(
        engine.get_item_by_id(selected.id).is_some(),
        "the copy is in the catalog"
    );
    assert_eq!(
        engine.get_tags_for_item(selected.id),
        vec!["beach".to_string()],
        "the copy carries the original's tags"
    );
    view.update(vcx, |app, _| {
        let index = app.find_active_index(selected.id);
        assert_eq!(app.selected_index, index, "the grid selection is the copy");
    });
    assert_eq!(
        test_launches::take(),
        vec![vec![
            "open".to_string(),
            "-b".to_string(),
            EDITOR.to_string(),
            selected.path.to_string_lossy().to_string(),
        ]],
        "the editor opens the copy, not the original"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn unset_editor_shortcut_opens_preferences() {
    let (dir, engine, photos, _) = edit_library("ext_unset");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    select(&view, vcx, photos[0]);
    test_launches::take();

    keystrokes(vcx, "alt-secondary-e");
    vcx.run_until_parked();

    view.update(vcx, |app, _| {
        assert!(
            app.preferences_open,
            "with no editor set, ⌥⌘E opens Preferences"
        );
        assert_eq!(
            app.preferences_tab,
            crate::app::preferences::PreferencesTab::General,
            "on the tab with the external editor row"
        );
        assert_eq!(
            app.error_message.as_deref(),
            Some(EXTERNAL_EDITOR_UNSET_MESSAGE)
        );
    });
    assert!(
        vcx.debug_bounds("preferences-choose-editor").is_some(),
        "the editor row is on screen"
    );
    assert_eq!(file_count(&dir), 3, "no copy is made without an editor");

    view.update(vcx, |app, cx| {
        app.close_preferences(cx);
        app.dismiss_error(cx);
    });
    keystrokes(vcx, "secondary-e");
    view.update(vcx, |app, _| {
        assert!(app.preferences_open, "⌘E opens Preferences too");
    });
    assert!(test_launches::take().is_empty(), "nothing is launched");
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn cmd_e_in_text_field_is_ignored() {
    let (dir, engine, photos, _) = edit_library("ext_field");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    set_editor(&view, vcx);
    select(&view, vcx, photos[0]);
    test_launches::take();

    keystrokes(vcx, "secondary-f");
    keystrokes(vcx, "secondary-e");
    keystrokes(vcx, "alt-secondary-e");
    vcx.run_until_parked();

    view.update(vcx, |app, _| {
        assert!(app.search_focused, "the search field keeps focus");
        assert!(
            !app.toast.is_visible(),
            "the field takes ⌘E: no refusal toast either"
        );
    });
    assert!(
        test_launches::take().is_empty(),
        "⌘E and ⌥⌘E in a text field launch nothing"
    );
    assert_eq!(file_count(&dir), 3, "⌥⌘E in a text field makes no copy");
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn external_editor_keys_are_absorbed_in_edit_mode() {
    let (dir, engine, photos, _) = edit_library("ext_edit_mode");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    set_editor(&view, vcx);
    open_viewer_on(&view, vcx, &engine, photos[0]);
    keystrokes(vcx, "e");
    assert!(
        view.update(vcx, |app, _| app.edits.session.is_some()),
        "E opens edit mode"
    );
    test_launches::take();

    keystrokes(vcx, "secondary-e");
    keystrokes(vcx, "alt-secondary-e");
    vcx.dispatch_action(crate::actions::EditInExternalEditor);
    vcx.dispatch_action(crate::actions::EditCopyInExternalEditor);
    vcx.run_until_parked();

    assert!(
        test_launches::take().is_empty(),
        "edit mode absorbs ⌘E / ⌥⌘E and disables the menu items"
    );
    assert_eq!(file_count(&dir), 3, "no copy is made under a draft");
    view.update(vcx, |app, _| {
        assert!(app.edits.session.is_some(), "edit mode stays open");
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn palette_and_context_menu_edit_a_copy() {
    let (dir, engine, photos, _) = edit_library("ext_palette");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    set_editor(&view, vcx);
    select(&view, vcx, photos[0]);
    test_launches::take();

    // Keyboard only: ⌘K, type, ↵.
    keystrokes(vcx, "secondary-k");
    type_text(vcx, "edit a copy");
    keystrokes(vcx, "enter");
    vcx.run_until_parked();
    assert_eq!(file_count(&dir), 4, "⌘K Edit a Copy makes one copy");
    assert_eq!(test_launches::take().len(), 1, "and opens it");

    // The context menu acts on the card it was opened on.
    let card: &'static str = Box::leak(format!("media-card-{}", photos[1].0).into_boxed_str());
    right_click(vcx, card);
    click(vcx, "edit-copy-external-menu-btn");
    vcx.run_until_parked();
    let stem = engine.get_item_by_id(photos[1]).unwrap().path;
    let stem = stem.file_stem().unwrap().to_string_lossy();
    assert!(
        dir.join(format!("{stem} (edited).jpg")).is_file(),
        "the context menu copies the right-clicked item"
    );
    assert_eq!(test_launches::take().len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn edit_copy_finishing_after_a_library_switch_leaves_the_new_library_alone() {
    let (dir, engine, photos, _) = edit_library("ext_switch_old");
    // More items than the old library, so the copy's id (the old library's
    // next id) names an unrelated photo here.
    let other_dir = crate::tests::quick_edit::temp_library("ext_switch_new");
    for i in 0..8 {
        std::fs::write(
            other_dir.join(format!("n{i}.jpg")),
            crate::tests::ui_automation::tiny_jpeg(),
        )
        .unwrap();
    }
    let other = crate::tests::quick_edit::scanned(&other_dir);
    let next_id = MediaId(
        engine
            .get_item_slice(0..4)
            .iter()
            .map(|i| i.id.0)
            .max()
            .unwrap()
            + 1,
    );
    let bystander = other
        .get_item_by_id(next_id)
        .expect("the new library has an item with the copy's id");
    // A tag, so a stray sidecar sync for that id would write a file.
    other
        .add_tags_batch(&[bystander.id], &["bystander".into()])
        .unwrap();
    let xmp = |dir: &std::path::Path| -> Vec<std::path::PathBuf> {
        let mut found: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "xmp"))
            .collect();
        found.sort();
        found
    };
    let sidecars_before = xmp(&other_dir);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    set_editor(&view, vcx);
    select(&view, vcx, photos[0]);
    test_launches::take();

    // The copy is queued, then another library opens before it finishes.
    view.update(vcx, |app, cx| {
        app.edit_selected_externally(true, cx);
        app.grid_state = Some(crate::views::grid::GridState::new(other.clone()));
        app.selected_item = None;
        app.selected_indices.clear();
        app.selected_index = None;
        app.refresh_album_counts();
    });
    vcx.run_until_parked();

    let copy = engine
        .get_item_by_id(next_id)
        .expect("the copy is catalogued in the old library");
    assert!(copy.file_name.contains("(edited)"), "{copy:?}");
    view.update(vcx, |app, _| {
        assert!(
            app.selected_item.as_ref().map(|item| item.id) != Some(bystander.id),
            "the new library's item with the copy's id is not selected"
        );
        assert!(app.selected_item.is_none(), "nothing is selected");
    });
    assert_eq!(
        xmp(&other_dir),
        sidecars_before,
        "no sidecar is written in the new library"
    );
    assert!(
        !lumina_core::sidecar_path(&bystander.path).exists(),
        "the bystander gets no sidecar"
    );
    let launches = test_launches::take();
    assert_eq!(
        launches,
        vec![vec![
            "open".to_string(),
            "-b".to_string(),
            EDITOR.to_string(),
            copy.path.to_string_lossy().to_string(),
        ]],
        "the editor still opens the copy"
    );
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&other_dir);
}

#[core::prelude::v1::test]
fn cmd_e_with_several_selected_does_nothing() {
    let (dir, engine, photos, _) = edit_library("ext_multi");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    set_editor(&view, vcx);
    select(&view, vcx, photos[0]);
    view.update(vcx, |app, cx| {
        let index = app.find_active_index(photos[1]).unwrap();
        app.selected_indices.insert(index);
        cx.notify();
    });
    test_launches::take();

    keystrokes(vcx, "secondary-e");
    keystrokes(vcx, "alt-secondary-e");
    vcx.run_until_parked();

    assert!(test_launches::take().is_empty(), "nothing is launched");
    assert_eq!(file_count(&dir), 3, "no copy is made");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.last_action.as_deref(),
            Some("Select one item to edit in the external editor"),
            "the status bar says why"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

/// A copy finishing while a page with no media shows (the Albums overview,
/// the Tags page) is selected in All Media, so no hidden selection is left
/// on the page.
#[core::prelude::v1::test]
fn edit_copy_finishing_on_a_page_without_media_shows_all_media() {
    type ShowPage = fn(&mut crate::LuminaApp, &mut gpui::Context<crate::LuminaApp>);
    let pages: [(&str, ShowPage); 2] = [
        ("ext_copy_overview", |app, cx| app.show_albums_overview(cx)),
        ("ext_copy_tags", |app, cx| app.open_tag_manager(cx)),
    ];
    for (prefix, show_page) in pages {
        let (dir, engine, photos, _) = edit_library(prefix);
        let mut cx = crate::tests::TestApp::single();
        let (view, vcx) = app_with_library(&mut cx, engine.clone());
        set_editor(&view, vcx);
        select(&view, vcx, photos[0]);
        test_launches::take();
        view.update(vcx, |app, cx| {
            app.edit_selected_externally(true, cx);
            show_page(app, cx);
            assert!(app.non_media_page_shown(), "{prefix}");
        });
        vcx.run_until_parked();
        render_ui(vcx);
        view.update(vcx, |app, _| {
            assert!(app.error_message.is_none(), "{:?}", app.error_message);
            assert_eq!(
                app.selection,
                crate::views::sidebar::AlbumSelection::AllLibrary,
                "{prefix}: the copy shows in All Media, not under the page"
            );
            let copy = app.selected_item.clone().expect("the copy is selected");
            assert_ne!(copy.id, photos[0], "the new item is selected");
            assert_eq!(app.selected_index, app.find_active_index(copy.id));
        });
        assert_eq!(test_launches::take().len(), 1, "the editor opens the copy");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
