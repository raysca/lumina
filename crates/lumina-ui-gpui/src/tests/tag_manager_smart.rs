//! Save as Smart Album (frame `L3F8Cs` ORGANISE): the builder prefilled with
//! one Tag-is-`<path>` rule, from the detail row, the row menu and ⌘K.

use super::duplicates::run_palette_command;
use super::tag_manager::open;
use crate::tests::ui_automation::{app_with_library, click, keystrokes, render_ui, seed_library};
use crate::views::sidebar::AlbumSelection;
use lumina_core::{RuleField, RuleOperator, RuleValue, SmartAlbumRule};

fn tag_rule(path: &str) -> SmartAlbumRule {
    SmartAlbumRule {
        field: RuleField::Tag,
        operator: RuleOperator::Equals,
        value: RuleValue::Text(path.into()),
    }
}

#[core::prelude::v1::test]
fn tag_save_as_smart_album_prefills_tag_rule() {
    // Items 0 and 1 carry trips/japan (1 also trips/japan/kyoto); 2 is
    // only trips; 3 is untagged.
    let (dir, engine) = seed_library("tm_save_smart", 4);
    let mut ids: Vec<u64> = engine.get_item_slice(0..4).iter().map(|i| i.id.0).collect();
    ids.sort();
    let ids: Vec<lumina_core::MediaId> = ids.into_iter().map(lumina_core::MediaId).collect();
    engine
        .add_tags_batch(&ids[..1], &["trips/japan".into()])
        .unwrap();
    engine
        .add_tags_batch(&ids[1..2], &["trips/japan/kyoto".into()])
        .unwrap();
    engine
        .add_tags_batch(&ids[2..3], &["trips".into()])
        .unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    vcx.simulate_resize(gpui::size(gpui::px(1440.0), gpui::px(900.0)));
    let listed = |app: &crate::LuminaApp| {
        use crate::app::command_palette::{CommandId, PaletteTarget};
        app.palette_rows()
            .iter()
            .any(|row| row.target == PaletteTarget::Command(CommandId::SaveKeywordSmart))
    };
    view.update(vcx, |app, _| {
        app.command_query = "save tag".into();
        assert!(!listed(app), "off the Tags page the command is hidden");
    });
    open(&view, vcx);
    view.update(vcx, |app, _| {
        assert!(!listed(app), "no focused keyword: still hidden");
        app.command_query.clear();
    });
    click(vcx, "tag-manager-row-trips/japan");
    // An open picker must give way to the sheet.
    click(vcx, "tag-detail-merge");
    render_ui(vcx);

    click(vcx, "tag-detail-save-smart");
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("album-builder-card").is_some(),
        "Save as Smart Album opens the builder"
    );
    view.update(vcx, |app, _| {
        assert!(
            app.tag_manager.merge.is_none() && !app.tag_manager.popover_open(),
            "the sheet closes the page's pickers and menus"
        );
        assert_eq!(app.album_draft.editing_album_id, None, "a new album");
        assert_eq!(
            app.album_draft.album_title, "japan",
            "the leaf is the title"
        );
        assert_eq!(
            app.draft_rules(),
            Some(vec![tag_rule("trips/japan")]),
            "exactly one Tag is <path> rule"
        );
        assert_eq!(
            app.album_draft.match_count,
            Some(2),
            "the keyword and the one below it"
        );
    });

    // Plain ↵ in the title saves; the page's ↵ (rename) must not run.
    keystrokes(vcx, "enter");
    let album_id = view.update(vcx, |app, _| {
        assert!(!app.album_draft.modal_open, "↵ saves and closes the sheet");
        assert!(app.tag_manager.editing.is_none(), "↵ went to the sheet");
        let album = app.user_smart_albums.first().expect("album saved");
        assert_eq!(app.selection, AlbumSelection::User(album.id));
        assert_eq!(
            app.get_active_count(),
            2,
            "the album shows the tagged items"
        );
        album.id
    });
    let saved = engine.list_smart_albums();
    assert_eq!(saved.len(), 1, "the album is persisted in the catalog");
    assert_eq!(saved[0].id, album_id);
    assert_eq!(saved[0].title, "japan");
    assert_eq!(saved[0].rules, [tag_rule("trips/japan")]);
    let mut matched: Vec<_> = engine
        .query_smart_album(&saved[0], 10, 0)
        .iter()
        .map(|item| item.id.0)
        .collect();
    matched.sort();
    assert_eq!(
        matched,
        [ids[0].0, ids[1].0],
        "it matches exactly the tagged items"
    );

    // The row menu offers it too: a top-level keyword titles itself.
    open(&view, vcx);
    click(vcx, "tag-row-menu-trips");
    click(vcx, "tag-row-menu//save-smart");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.album_draft.modal_open, "the row menu opens the builder");
        assert!(app.tag_manager.row_menu.is_none(), "and closes the menu");
        assert_eq!(app.album_draft.album_title, "trips");
        assert_eq!(app.draft_rules(), Some(vec![tag_rule("trips")]));
    });
    keystrokes(vcx, "escape");

    // And ⌘K, only while a keyword is focused on the Tags page.
    open(&view, vcx);
    click(vcx, "tag-manager-row-trips/japan/kyoto");
    run_palette_command(vcx, &view, "Save Tag as Smart");
    view.update(vcx, |app, _| {
        assert!(app.album_draft.modal_open, "the palette opens the builder");
        assert_eq!(app.album_draft.album_title, "kyoto");
        assert_eq!(app.draft_rules(), Some(vec![tag_rule("trips/japan/kyoto")]));
    });

    let _ = std::fs::remove_dir_all(&dir);
}
