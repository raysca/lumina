//! Metadata templates (plan 4, Task 9): the editor, applying a template to
//! the selection from the palette and the batch inspector, one undo, sidecars
//! and the config round trip.

use crate::app::templates::MetadataTemplate;
use crate::config::AppConfig;
use crate::tests::ui_automation::{
    app_with_library, click, keystrokes, render_ui, seed_library, type_text,
};
use crate::LuminaApp;
use gpui::{Entity, VisualTestContext};
use lumina_core::{LuminaEngine, MediaId, Rights};

fn studio() -> MetadataTemplate {
    MetadataTemplate {
        name: "Studio".into(),
        rights: Rights {
            creator: Some("Ada Lovelace".into()),
            copyright: Some("© 2026 Ada".into()),
            ..Default::default()
        },
        keywords: vec!["trip".into(), "work/clients".into()],
    }
}

fn rights_of(engine: &LuminaEngine, id: MediaId) -> Rights {
    engine
        .catalog()
        .get_rights(&[id])
        .unwrap()
        .remove(&id)
        .unwrap_or_default()
}

fn ids(engine: &LuminaEngine, n: usize) -> Vec<MediaId> {
    engine
        .get_item_slice(0..n)
        .into_iter()
        .map(|item| item.id)
        .collect()
}

fn with_templates(
    view: &Entity<LuminaApp>,
    vcx: &mut VisualTestContext,
    list: Vec<MetadataTemplate>,
) {
    view.update(vcx, |app, cx| {
        app.config.templates = list;
        cx.notify();
    });
    render_ui(vcx);
}

#[core::prelude::v1::test]
fn template_apply_and_single_undo() {
    let (dir, engine) = seed_library("template_apply", 3);
    let all = ids(&engine, 3);
    let own = Rights {
        creator: Some("Grace".into()),
        usage_terms: Some("Editorial only".into()),
        ..Default::default()
    };
    engine
        .catalog()
        .set_rights(&[(all[0], own.clone())])
        .unwrap();
    engine.add_tags_batch(&[all[0]], &["trip".into()]).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    with_templates(&view, vcx, vec![studio()]);

    // Keyboard only: select all, palette, the command, the template.
    keystrokes(vcx, "cmd-a");
    // Get Info has the selected item's rights loaded before the apply.
    view.update(vcx, |app, cx| app.ensure_info_details(cx));
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        let item = app.selected_item.clone().expect("⌘A selects an item");
        assert!(app.info_details.as_ref().is_some_and(|d| d.is_for(&item)));
    });
    keystrokes(vcx, "cmd-k");
    type_text(vcx, "apply metadata");
    keystrokes(vcx, "enter");
    assert!(
        vcx.debug_bounds("palette-template-Studio").is_some(),
        "Apply Metadata Template… lists the template names in the palette"
    );
    keystrokes(vcx, "enter");
    vcx.run_until_parked();

    for &id in &all {
        let rights = rights_of(&engine, id);
        assert_eq!(rights.creator.as_deref(), Some("Ada Lovelace"));
        assert_eq!(rights.copyright.as_deref(), Some("© 2026 Ada"));
        let mut tags = engine.get_tags_for_item(id);
        tags.sort();
        assert_eq!(tags, vec!["trip", "work/clients"], "keywords added to all");
        let sidecar = lumina_core::read_sidecar(&engine.get_item_by_id(id).unwrap().path)
            .unwrap()
            .expect("applying a template writes the sidecar");
        assert_eq!(
            sidecar.rights.creator.as_deref(),
            Some("Ada Lovelace"),
            "the sidecar carries the template's rights"
        );
        assert!(sidecar.tags.contains(&"work/clients".to_string()));
    }
    assert_eq!(
        rights_of(&engine, all[0]).usage_terms.as_deref(),
        Some("Editorial only"),
        "a field the template leaves unset keeps the item's value"
    );
    view.update(vcx, |app, cx| {
        assert!(!app.command_palette_open, "applying closes the palette");
        app.ensure_info_details(cx);
    });
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        assert_eq!(
            app.info_details
                .as_ref()
                .and_then(|d| d.rights.creator.as_deref()),
            Some("Ada Lovelace"),
            "Get Info shows the applied rights, not its cached ones"
        );
    });

    keystrokes(vcx, "cmd-z");
    vcx.run_until_parked();
    assert_eq!(rights_of(&engine, all[0]), own, "one undo restores item 1");
    assert_eq!(
        engine.get_tags_for_item(all[0]),
        vec!["trip"],
        "its own tag stays"
    );
    for &id in &all[1..] {
        assert_eq!(rights_of(&engine, id), Rights::default(), "and the others");
        assert!(engine.get_tags_for_item(id).is_empty());
    }
    let sidecar = lumina_core::read_sidecar(&engine.get_item_by_id(all[0]).unwrap().path)
        .unwrap()
        .expect("the undone item keeps a sidecar for its own rights");
    assert_eq!(
        sidecar.rights.creator.as_deref(),
        Some("Grace"),
        "undo rewrites the sidecar"
    );
    view.update(vcx, |app, _| {
        assert!(
            app.undo.undo_stack.is_empty(),
            "the apply was one undo entry"
        );
    });

    keystrokes(vcx, "shift-cmd-z");
    vcx.run_until_parked();
    for &id in &all {
        assert_eq!(
            rights_of(&engine, id).creator.as_deref(),
            Some("Ada Lovelace"),
            "redo applies the template again"
        );
        let sidecar = lumina_core::read_sidecar(&engine.get_item_by_id(id).unwrap().path)
            .unwrap()
            .expect("redo writes the sidecar");
        assert_eq!(
            sidecar.rights.creator.as_deref(),
            Some("Ada Lovelace"),
            "redo rewrites the sidecar"
        );
        assert!(sidecar.tags.contains(&"work/clients".to_string()));
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn batch_inspector_applies_picked_template() {
    let (dir, engine) = seed_library("template_batch", 2);
    let all = ids(&engine, 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let second = MetadataTemplate {
        name: "Press".into(),
        rights: Rights {
            creator: Some("Press Desk".into()),
            ..Default::default()
        },
        keywords: Vec::new(),
    };
    with_templates(&view, vcx, vec![studio(), second]);
    view.update(vcx, |app, cx| {
        app.select_all(cx);
        app.inspector_open = true;
        cx.notify();
    });
    render_ui(vcx);

    click(vcx, "batch-template-picker");
    click(vcx, "batch-template-apply");
    for &id in &all {
        assert_eq!(
            rights_of(&engine, id).creator.as_deref(),
            Some("Press Desk"),
            "the picker's second click shows, and Apply applies, the second template"
        );
        assert!(
            engine.get_tags_for_item(id).is_empty(),
            "it has no keywords"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn template_editor_creates_edits_and_deletes_by_keyboard() {
    let (dir, engine) = seed_library("template_editor", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    keystrokes(vcx, "cmd-k");
    type_text(vcx, "metadata templates");
    keystrokes(vcx, "enter");
    assert!(
        vcx.debug_bounds("template-editor").is_some(),
        "the palette opens the editor"
    );
    assert!(vcx.debug_bounds("template-field-name").is_some());

    type_text(vcx, "Studio");
    keystrokes(vcx, "tab");
    type_text(vcx, "Ada");
    // Tab past copyright, usage terms, email and website to the keywords.
    keystrokes(vcx, "tab tab tab tab tab");
    type_text(vcx, "Work, Places / Paris");
    keystrokes(vcx, "shift-tab");
    type_text(vcx, "https://ada.example");
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert!(app.templates.editor.is_none(), "Enter saves and closes");
        assert_eq!(
            app.config.templates,
            vec![MetadataTemplate {
                name: "Studio".into(),
                rights: Rights {
                    creator: Some("Ada".into()),
                    contact_url: Some("https://ada.example".into()),
                    ..Default::default()
                },
                keywords: vec!["work".into(), "places/paris".into()],
            }],
            "the typed fields make the template; keywords are cleaned paths"
        );
    });

    // Edit it: the editor opens on the first template.
    keystrokes(vcx, "cmd-k");
    type_text(vcx, "metadata templates");
    keystrokes(vcx, "enter");
    keystrokes(vcx, "tab");
    type_text(vcx, " L");
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.config.templates.len(),
            1,
            "editing replaces, never adds"
        );
        assert_eq!(
            app.config.templates[0].rights.creator.as_deref(),
            Some("Ada L")
        );
    });

    // A second name that clashes is refused with a reason.
    keystrokes(vcx, "cmd-k");
    type_text(vcx, "metadata templates");
    keystrokes(vcx, "enter");
    keystrokes(vcx, "down");
    type_text(vcx, "studio");
    keystrokes(vcx, "enter");
    assert!(
        vcx.debug_bounds("template-error").is_some(),
        "a taken name shows why"
    );
    view.update(vcx, |app, _| assert_eq!(app.config.templates.len(), 1));

    // Escape drops the typing; reopened on Studio, ⇧⌘⌫ deletes it.
    keystrokes(vcx, "escape");
    keystrokes(vcx, "cmd-k");
    type_text(vcx, "metadata templates");
    keystrokes(vcx, "enter");
    keystrokes(vcx, "shift-cmd-backspace");
    view.update(vcx, |app, _| {
        assert!(
            app.config.templates.is_empty(),
            "⇧⌘⌫ deletes the shown template"
        );
        assert!(
            app.templates.editor.is_none(),
            "the editor closes so the toast's Undo can be reached"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn template_editor_escape_discards() {
    let (dir, engine) = seed_library("template_escape", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    keystrokes(vcx, "home");
    let selected = view.update(vcx, |app, _| app.selected_item.clone().unwrap().id);

    view.update(vcx, |app, cx| app.open_template_editor(cx));
    render_ui(vcx);
    type_text(vcx, "Draft");
    // Grid shortcuts must not reach the library behind the sheet: ⌘⌫ in an
    // empty field would move the selection to the trash, ↓ moves it, Space
    // opens the viewer.
    keystrokes(vcx, "tab");
    keystrokes(vcx, "cmd-backspace down space");
    view.update(vcx, |app, _| {
        assert_eq!(app.selected_item.as_ref().map(|i| i.id), Some(selected));
        assert!(!app.is_fullscreen_viewer, "Space stayed in the field");
    });
    assert!(
        !engine.get_item_by_id(selected).unwrap().is_trashed,
        "⌘⌫ in the editor never trashes the selection"
    );

    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(app.templates.editor.is_none(), "Escape closes the editor");
        assert!(app.config.templates.is_empty(), "and saves nothing");
    });
    assert!(vcx.debug_bounds("template-editor").is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn template_apply_write_failure_reverts_and_banners() {
    let (dir, engine) = seed_library("template_failure", 2);
    let all = ids(&engine, 2);
    engine
        .catalog()
        .set_alias("nyc", "places/new york")
        .unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    // The editor would refuse nothing here: "nyc/x" is a valid path. The
    // catalog refuses it because it sits below an alias.
    let mut bad = studio();
    bad.keywords = vec!["trip".into(), "nyc/x".into()];
    with_templates(&view, vcx, vec![bad]);

    view.update(vcx, |app, cx| {
        app.select_all(cx);
        app.apply_template(0, cx);
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("error-banner").is_some(),
        "the failure is shown"
    );
    for &id in &all {
        assert_eq!(
            rights_of(&engine, id),
            Rights::default(),
            "no rights written"
        );
        assert!(
            engine.get_tags_for_item(id).is_empty(),
            "no keywords written"
        );
    }
    view.update(vcx, |app, _| {
        assert!(
            app.undo.undo_stack.is_empty(),
            "a failed apply is not undoable"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn template_save_failure_keeps_list_and_typing() {
    let (dir, engine) = seed_library("template_save_fail", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| app.open_template_editor(cx));
    render_ui(vcx);
    type_text(vcx, "Studio");
    crate::app::state::SAVE_FAILS.with(|f| f.set(true));
    keystrokes(vcx, "enter");
    crate::app::state::SAVE_FAILS.with(|f| f.set(false));
    view.update(vcx, |app, _| {
        assert!(
            app.config.templates.is_empty(),
            "a failed save restores the list"
        );
        assert!(app.error_message.is_some(), "and says why");
        let editor = app
            .templates
            .editor
            .as_ref()
            .expect("the editor stays open");
        assert_eq!(editor.values[0], "Studio", "with the typing kept");
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn config_round_trip_keeps_templates() {
    // A temp file, never the real config (AGENTS §2).
    let dir = std::env::temp_dir().join(format!(
        "lumina_templates_config_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.json");
    let config = AppConfig {
        templates: vec![studio()],
        ..Default::default()
    };
    config.save_to(&path).unwrap();
    assert_eq!(AppConfig::load_from(&path).templates, vec![studio()]);

    std::fs::write(&path, "{\"last_library_path\": null}").unwrap();
    assert!(
        AppConfig::load_from(&path).templates.is_empty(),
        "a config from before templates loads with none"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

fn named(name: &str) -> MetadataTemplate {
    MetadataTemplate {
        name: name.into(),
        ..Default::default()
    }
}

#[core::prelude::v1::test]
fn template_editor_never_stacks_with_other_sheets() {
    let (dir, engine) = seed_library("template_stacking", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, cx| app.open_album_builder(cx));
    render_ui(vcx);
    vcx.dispatch_action(crate::actions::ManageTemplates);
    vcx.run_until_parked();
    view.update(vcx, |app, cx| {
        assert!(
            app.templates.editor.is_none(),
            "the menu item must not open the editor under the smart album sheet"
        );
        app.close_album_builder(cx);
    });
    render_ui(vcx);

    vcx.dispatch_action(crate::actions::ManageTemplates);
    vcx.run_until_parked();
    view.update(vcx, |app, _| assert!(app.templates.editor.is_some()));
    let picks = crate::app::import::IMPORT_PICKS.with(|n| n.get());
    vcx.dispatch_action(crate::actions::ImportNetworkVideo);
    vcx.dispatch_action(crate::actions::ImportMedia);
    vcx.dispatch_action(crate::actions::ShowCommandPalette);
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        assert!(
            app.network_import.stage.is_none(),
            "Import Video from URL… waits until the editor is closed"
        );
        assert!(!app.command_palette_open, "so does the command palette");
        assert!(app.templates.editor.is_some());
    });
    assert_eq!(
        crate::app::import::IMPORT_PICKS.with(|n| n.get()),
        picks,
        "Import… waits until the editor is closed"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn template_editor_keeps_unsaved_typing() {
    let (dir, engine) = seed_library("template_unsaved", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    with_templates(&view, vcx, vec![named("Alpha"), named("Beta")]);
    view.update(vcx, |app, cx| app.open_template_editor(cx));
    render_ui(vcx);
    type_text(vcx, " two");

    keystrokes(vcx, "down");
    click(vcx, "template-row-1");
    crate::tests::ui_automation::click_point(vcx, 4.0, 4.0);
    view.update(vcx, |app, _| {
        let editor = app
            .templates
            .editor
            .as_ref()
            .expect("the editor stays open");
        assert_eq!(editor.editing, Some(0), "↓, a row and the backdrop keep it");
        assert_eq!(editor.values[0], "Alpha two", "with the typing");
        assert!(editor.error.is_some(), "and say how to save or discard");
    });

    keystrokes(vcx, "escape");
    view.update(vcx, |app, cx| {
        assert!(app.templates.editor.is_none(), "Escape still discards");
        app.open_template_editor(cx);
    });
    render_ui(vcx);
    keystrokes(vcx, "down");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.templates.editor.as_ref().and_then(|e| e.editing),
            Some(1),
            "with nothing typed ↓ shows the next template"
        );
    });
    crate::tests::ui_automation::click_point(vcx, 4.0, 4.0);
    view.update(vcx, |app, _| {
        assert!(app.templates.editor.is_none(), "and the backdrop closes it")
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn template_delete_can_be_undone() {
    let (dir, engine) = seed_library("template_delete_undo", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    with_templates(&view, vcx, vec![named("Alpha"), named("Beta")]);
    view.update(vcx, |app, cx| app.open_template_editor(cx));
    render_ui(vcx);
    keystrokes(vcx, "down");
    keystrokes(vcx, "shift-cmd-backspace");
    view.update(vcx, |app, _| {
        assert_eq!(app.config.templates, vec![named("Alpha")]);
        assert!(app.templates.editor.is_none());
    });

    click(vcx, "toast-undo");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.config.templates,
            vec![named("Alpha"), named("Beta")],
            "the toast's Undo puts the template back in its place"
        );
    });
    keystrokes(vcx, "shift-cmd-z");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.config.templates,
            vec![named("Alpha")],
            "redo deletes it again"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn undoing_a_template_delete_never_duplicates_a_name() {
    let (dir, engine) = seed_library("template_delete_undo_name", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    with_templates(&view, vcx, vec![named("Alpha"), named("Beta")]);
    view.update(vcx, |app, cx| app.open_template_editor(cx));
    render_ui(vcx);
    keystrokes(vcx, "down");
    keystrokes(vcx, "shift-cmd-backspace");
    view.update(vcx, |app, _| app.config.templates.push(named("beta")));

    keystrokes(vcx, "cmd-z");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.config.templates,
            vec![named("Alpha"), named("beta")],
            "undo must not restore a template whose name is now taken"
        );
        assert!(
            app.error_message.is_some(),
            "the banner says why the undo was refused"
        );
        assert_eq!(app.undo.undo_stack.len(), 1, "the undo stays available");
    });
    let _ = std::fs::remove_dir_all(&dir);
}
