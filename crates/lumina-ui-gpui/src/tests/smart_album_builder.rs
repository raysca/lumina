//! Smart album builder: the Photos-style sheet driven by real clicks and keys.

use gpui::{Entity, VisualTestContext};
use lumina_core::{
    LuminaEngine, MatchMode, MediaId, RuleField, RuleOperator, RuleValue, SmartAlbumRule,
};

use crate::app::smart_draft::DraftFocus;
use crate::tests::ui_automation::{
    app_with_library, click, focus_app, keystrokes, leak, render_ui, seed_library,
};
use crate::views::sidebar::AlbumSelection;
use crate::LuminaApp;

/// Four photos: 00 and 01 are favourites, 01 is tagged "beach".
fn library(
    prefix: &str,
) -> (
    std::path::PathBuf,
    std::sync::Arc<LuminaEngine>,
    Vec<MediaId>,
) {
    let (dir, engine) = seed_library(prefix, 4);
    let mut items = engine.get_item_slice(0..4);
    items.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    let ids: Vec<MediaId> = items.iter().map(|item| item.id).collect();
    engine.toggle_favorite(ids[0], true).unwrap();
    engine.toggle_favorite(ids[1], true).unwrap();
    engine
        .add_tags_batch(&[ids[1]], &["beach".to_string()])
        .unwrap();
    (dir, engine, ids)
}

fn open_sheet(vcx: &mut VisualTestContext, view: &Entity<LuminaApp>) {
    focus_app(vcx, view);
    keystrokes(vcx, "alt-cmd-n");
    view.update(vcx, |app, _| {
        assert!(app.album_draft.modal_open, "⌥⌘N opens the builder");
        assert_eq!(app.album_draft.focus, DraftFocus::Title);
    });
}

#[core::prelude::v1::test]
fn builder_creates_album_from_clicks_and_shows_a_live_preview() {
    let (dir, engine, ids) = library("sab_clicks");
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_sheet(vcx, &view);

    view.update(vcx, |app, _| {
        assert_eq!(app.album_draft.match_count, Some(2), "Favourite is Yes");
        let preview: Vec<MediaId> = app.album_draft.preview.iter().map(|i| i.id).collect();
        assert_eq!(preview.len(), 2, "the preview shows the real matches");
        assert!(preview.contains(&ids[0]) && preview.contains(&ids[1]));
    });
    assert!(
        vcx.debug_bounds(leak(format!("album-builder-preview-{}", ids[1].0)))
            .is_some(),
        "matching items render in the preview strip"
    );

    // Capitals come from key_char, so Shift works in the name field.
    keystrokes(vcx, "shift-b e a c h space shift-f a v s");
    view.update(vcx, |app, _| {
        assert_eq!(app.album_draft.album_title, "Beach Favs")
    });

    click(vcx, "album-rule-add-0");
    click(vcx, "album-rule-field-1");
    assert!(vcx.debug_bounds("album-builder-popup").is_some());
    click(vcx, "album-builder-option-3"); // Tag
    view.update(vcx, |app, _| {
        assert_eq!(app.album_draft.rows[1].field, RuleField::Tag);
        assert_eq!(
            app.album_draft.match_count, None,
            "a tag row with no tag is invalid"
        );
    });

    // OK is disabled while a row is invalid.
    click(vcx, "album-builder-save");
    view.update(vcx, |app, _| {
        assert!(app.album_draft.modal_open, "OK must not save invalid rules");
        assert!(app.user_smart_albums.is_empty());
    });

    click(vcx, "album-rule-value-1");
    click(vcx, "album-builder-option-0"); // the library's only tag: beach
    view.update(vcx, |app, _| {
        assert_eq!(app.album_draft.rows[1].text, "beach");
        assert_eq!(app.album_draft.match_count, Some(1));
    });

    click(vcx, "album-builder-save");
    view.update(vcx, |app, _| {
        assert!(!app.album_draft.modal_open);
        let album = app.user_smart_albums.first().expect("album saved");
        assert_eq!(album.title, "Beach Favs");
        assert_eq!(album.rules.len(), 2);
        assert_eq!(
            app.selection,
            AlbumSelection::User(album.id),
            "OK shows the new album"
        );
        assert_eq!(app.get_active_count(), 1);
    });
    let saved = engine.list_smart_albums();
    assert_eq!(saved.len(), 1, "the album is persisted in the catalog");
    assert_eq!(
        saved[0].rules[1],
        SmartAlbumRule {
            field: RuleField::Tag,
            operator: RuleOperator::Equals,
            value: RuleValue::Text("beach".into()),
        }
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn builder_is_fully_keyboard_driven() {
    let (dir, engine, _ids) = library("sab_keys");
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    // ⌘K → "New Smart Album…"
    keystrokes(vcx, "cmd-k n e w space s m a r t enter");
    view.update(vcx, |app, _| assert!(app.album_draft.modal_open));

    // Match: Tab, Space opens, Down, Enter picks "any".
    keystrokes(vcx, "tab space down enter");
    view.update(vcx, |app, _| {
        assert_eq!(app.album_draft.match_mode, MatchMode::Any);
        assert_eq!(app.album_draft.popup, None);
    });

    // Field: Favourite → Date (one down).
    keystrokes(vcx, "tab enter down enter");
    view.update(vcx, |app, _| {
        assert_eq!(app.album_draft.rows[0].field, RuleField::Date)
    });

    // Escape inside a popup closes only the popup.
    keystrokes(vcx, "tab down escape");
    view.update(vcx, |app, _| {
        assert!(app.album_draft.modal_open);
        assert_eq!(app.album_draft.popup, None);
        assert_eq!(app.album_draft.focus, DraftFocus::Operator(0));
    });

    // Operator: is → is in the last (four down), value 30 → 7, unit weeks.
    keystrokes(
        vcx,
        "down down down down down enter tab backspace backspace 7 tab space down enter",
    );
    view.update(vcx, |app, _| {
        let rules = app.draft_rules().expect("rows are valid");
        assert_eq!(
            rules[0],
            SmartAlbumRule {
                field: RuleField::Date,
                operator: RuleOperator::InTheLast,
                value: RuleValue::Text("7w".into()),
            }
        );
    });

    // + adds a row and focuses its field; Space on − removes it again.
    keystrokes(vcx, "tab space");
    view.update(vcx, |app, _| {
        assert_eq!(app.album_draft.rows.len(), 2);
        assert_eq!(app.album_draft.focus, DraftFocus::Field(1));
    });
    keystrokes(vcx, "tab tab tab space");
    view.update(vcx, |app, _| {
        assert_eq!(app.album_draft.rows.len(), 1, "Space on − removes the row");
    });

    // ⌘Enter is OK.
    keystrokes(vcx, "cmd-enter");
    view.update(vcx, |app, _| {
        assert!(!app.album_draft.modal_open);
        assert_eq!(app.user_smart_albums.len(), 1);
        assert_eq!(app.user_smart_albums[0].title, "Untitled Smart Album");
    });

    // Esc is Cancel: nothing new is saved.
    keystrokes(vcx, "alt-cmd-n escape");
    view.update(vcx, |app, _| {
        assert!(!app.album_draft.modal_open);
        assert_eq!(engine.list_smart_albums().len(), 1);
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn invalid_typed_value_blocks_enter_and_says_why() {
    let (dir, engine, _ids) = library("sab_invalid");
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    // Edit Smart Album… with no smart album shown says why instead of nothing.
    keystrokes(vcx, "cmd-k e d i t space s m a r t enter");
    view.update(vcx, |app, _| {
        assert!(!app.album_draft.modal_open);
        assert!(app.toast.current().is_some(), "the no-op must be explained");
    });

    open_sheet(vcx, &view);

    view.update(vcx, |app, cx| {
        app.album_draft.rows[0] = crate::app::smart_draft::DraftRow::new(RuleField::Date);
        app.set_draft_focus(DraftFocus::Value(0), cx);
    });
    keystrokes(vcx, "backspace backspace");
    view.update(vcx, |app, _| assert_eq!(app.album_draft.match_count, None));
    render_ui(vcx);
    assert!(vcx.debug_bounds("album-builder-count").is_some());

    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert!(app.album_draft.modal_open, "Enter must not save a bad date");
        assert!(engine.list_smart_albums().is_empty());
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn editing_restores_rows_and_updates_the_album() {
    let (dir, engine, ids) = library("sab_edit");
    engine.set_rating(ids[2], 5).unwrap();
    let album = engine
        .create_smart_album(
            "Best",
            "folder",
            MatchMode::Any,
            &[SmartAlbumRule {
                field: RuleField::Rating,
                operator: RuleOperator::AtLeast,
                value: RuleValue::Integer(4),
            }],
        )
        .unwrap();
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::User(album.id), cx);
    });
    keystrokes(vcx, "cmd-k e d i t space s m a r t enter");
    view.update(vcx, |app, _| {
        assert_eq!(app.album_draft.editing_album_id, Some(album.id));
        assert_eq!(app.album_draft.album_title, "Best");
        assert_eq!(app.album_draft.match_mode, MatchMode::Any);
        assert_eq!(app.album_draft.rows[0].display_value(), "★★★★");
        assert_eq!(app.album_draft.match_count, Some(1));
    });

    // Stars: ★★★★ → ★ (option 0), then OK.
    click(vcx, "album-rule-value-0");
    click(vcx, "album-builder-option-0");
    click(vcx, "album-builder-save");
    let saved = engine.list_smart_albums();
    assert_eq!(saved.len(), 1, "editing updates, never duplicates");
    // The catalog stores values as JSON, so 1 may come back as 1.0.
    assert_eq!(
        crate::app::smart_draft::DraftRow::from_rule(&saved[0].rules[0]).text,
        "1"
    );
    view.update(vcx, |app, _| {
        assert_eq!(app.selection, AlbumSelection::User(album.id));
        assert_eq!(app.get_active_count(), 1, "the grid reloads the new rules");
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn a_saved_rule_the_engine_rejects_is_reported_not_counted_as_zero() {
    let (dir, engine, _ids) = library("sab_rejected");
    // Valid as a row (a well-formed date) but not as a query: the old
    // builder could save "Date is not <text>", which no longer compiles.
    let album = engine
        .create_smart_album(
            "Legacy",
            "folder",
            MatchMode::All,
            &[SmartAlbumRule {
                field: RuleField::Date,
                operator: RuleOperator::NotEquals,
                value: RuleValue::Text("2024-05-01".into()),
            }],
        )
        .unwrap();
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    view.update(vcx, |app, cx| {
        app.open_album_builder_for_edit(album.clone(), cx)
    });
    render_ui(vcx);

    view.update(vcx, |app, _| {
        assert!(app.draft_rules().is_some(), "the row itself is well formed");
        assert_eq!(app.album_draft.match_count, None, "not \"No items match\"");
        assert!(
            app.album_draft.problem.is_some(),
            "the engine's error is kept for the footer"
        );
    });
    keystrokes(vcx, "cmd-enter");
    view.update(vcx, |app, _| {
        assert!(
            app.album_draft.modal_open,
            "OK must not save a rule that cannot run"
        );
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn focused_text_fields_have_room_for_the_caret() {
    let (dir, engine, _ids) = library("sab_caret");
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_sheet(vcx, &view);
    keystrokes(vcx, "b e a c h");

    // The text box must span the field, not just its own text: GPUI's
    // ellipsis measures per character and dropped the trailing "|" caret
    // when the box was only as wide as the shaped text.
    let field = vcx
        .debug_bounds("album-builder-title")
        .expect("title field");
    let text = vcx
        .debug_bounds("album-builder-title-text")
        .expect("title text");
    let spare = field.size.width - text.size.width;
    assert!(
        spare < gpui::px(24.0),
        "title text box is {:?} wide in a {:?} field; it must fill the field",
        text.size.width,
        field.size.width
    );

    let _ = std::fs::remove_dir_all(&dir);
}
