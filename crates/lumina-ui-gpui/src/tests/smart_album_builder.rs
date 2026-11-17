//! Smart album builder: the Photos-style sheet driven by real clicks and keys.

use gpui::{Entity, VisualTestContext};
use lumina_core::{
    LuminaEngine, MatchMode, MediaId, RuleField, RuleOperator, RuleValue, SmartAlbumRule,
};

use crate::app::smart_draft::DraftFocus;
use crate::tests::ui_automation::{
    app_with_library, click, focus_app, keystrokes, leak, render_ui, seed_library, type_text,
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
    keystrokes(vcx, "alt-secondary-n");
    view.update(vcx, |app, _| {
        assert!(app.album_draft.modal_open, "⌥⌘N opens the builder");
        assert_eq!(app.album_draft.focus, DraftFocus::Title);
    });
}

#[core::prelude::v1::test]
fn builder_creates_album_from_clicks_and_shows_a_live_preview() {
    let (dir, engine, ids) = library("sab_clicks");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_sheet(vcx, &view);

    view.update(vcx, |app, _| {
        assert_eq!(app.album_draft.match_count, Some(2), "Favorite is Yes");
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
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    // ⌘K → "New Smart Album…"
    keystrokes(vcx, "secondary-k n e w space s m a r t enter");
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

    // Past −, + adds a row and focuses its field; Space on − removes it again.
    keystrokes(vcx, "tab tab space");
    view.update(vcx, |app, _| {
        assert_eq!(app.album_draft.rows.len(), 2);
        assert_eq!(app.album_draft.focus, DraftFocus::Field(1));
    });
    keystrokes(vcx, "tab tab tab space");
    view.update(vcx, |app, _| {
        assert_eq!(app.album_draft.rows.len(), 1, "Space on − removes the row");
    });

    // ⌘Enter is OK.
    keystrokes(vcx, "secondary-enter");
    view.update(vcx, |app, _| {
        assert!(!app.album_draft.modal_open);
        assert_eq!(app.user_smart_albums.len(), 1);
        assert_eq!(app.user_smart_albums[0].title, "Untitled Smart Album");
    });

    // Esc is Cancel: nothing new is saved.
    keystrokes(vcx, "alt-secondary-n escape");
    view.update(vcx, |app, _| {
        assert!(!app.album_draft.modal_open);
        assert_eq!(engine.list_smart_albums().len(), 1);
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn invalid_typed_value_blocks_enter_and_says_why() {
    let (dir, engine, _ids) = library("sab_invalid");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    // Edit Smart Album… with no smart album shown says why instead of nothing.
    keystrokes(vcx, "secondary-k e d i t space s m a r t enter");
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
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::User(album.id), cx);
    });
    keystrokes(vcx, "secondary-k e d i t space s m a r t enter");
    view.update(vcx, |app, _| {
        assert_eq!(app.album_draft.editing_album_id, Some(album.id));
        assert_eq!(app.album_draft.album_title, "Best");
        assert_eq!(app.album_draft.match_mode, MatchMode::Any);
        assert_eq!(app.album_draft.rows[0].display_value(), "★★★★");
        assert_eq!(app.album_draft.match_count, Some(1));
    });

    // Stars: four → one (click the first star), then OK.
    click(vcx, "album-rule-star-0-1");
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
    let mut cx = crate::tests::TestApp::single();
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
    keystrokes(vcx, "secondary-enter");
    view.update(vcx, |app, _| {
        assert!(
            app.album_draft.modal_open,
            "OK must not save a rule that cannot run"
        );
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn focused_text_fields_give_the_input_the_whole_field() {
    let (dir, engine, _ids) = library("sab_caret");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_sheet(vcx, &view);
    keystrokes(vcx, "b e a c h");

    // The text box must span the field, not just its own text, so the
    // input has room to show its caret and scroll a long title.
    let field = vcx
        .debug_bounds("album-builder-title")
        .expect("title field");
    let text = vcx
        .debug_bounds("album-builder-title-text")
        .expect("title text");
    // Only the field's 13px padding and 1px border on each side are spare.
    let spare = field.size.width - text.size.width;
    assert!(
        spare <= gpui::px(28.0),
        "title text box is {:?} wide in a {:?} field; it must fill the field",
        text.size.width,
        field.size.width
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn v3_sheet_layout_matches_the_design() {
    let (dir, engine, ids) = library("sab_v3_layout");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_sheet(vcx, &view);
    render_ui(vcx);

    let card = vcx.debug_bounds("album-builder-card").unwrap();
    assert_eq!(f32::from(card.size.width), 840.0, "the dialog is 840 wide");
    let window = vcx.update(|window, _| window.viewport_size());
    let above = f32::from(card.origin.y);
    let below = f32::from(window.height - card.origin.y - card.size.height);
    assert!(
        (above - below).abs() <= 1.0,
        "the dialog is centred vertically: {above} above, {below} below"
    );
    let mut size = |selector: &'static str| {
        let b = vcx.debug_bounds(selector).unwrap();
        (f32::from(b.size.width), f32::from(b.size.height))
    };
    assert_eq!(size("album-builder-badge"), (36.0, 36.0), "badge is 36x36");
    assert_eq!(size("album-builder-title"), (420.0, 42.0), "name is 420x42");
    assert_eq!(size("album-builder-close"), (28.0, 28.0), "close is 28x28");
    assert_eq!(
        size("album-builder-match").0,
        92.0,
        "match picker is 92 wide"
    );
    assert_eq!(
        f32::from(vcx.debug_bounds("album-rule-field-0").unwrap().size.width),
        210.0
    );
    assert_eq!(
        f32::from(vcx.debug_bounds("album-rule-op-0").unwrap().size.width),
        140.0
    );
    let tile = vcx
        .debug_bounds(leak(format!("album-builder-preview-{}", ids[0].0)))
        .unwrap();
    assert_eq!(
        f32::from(tile.size.height),
        96.0,
        "preview tiles are 96 tall"
    );
    for selector in [
        "album-builder-conditions",
        "album-builder-preview-help",
        "album-rule-remove-0",
    ] {
        assert!(
            vcx.debug_bounds(selector).is_some(),
            "{selector} must render"
        );
    }
    let rows = vcx.debug_bounds("album-builder-conditions").unwrap();
    let field = vcx.debug_bounds("album-rule-field-0").unwrap();
    assert!(
        rows.contains(&field.origin),
        "rows sit inside one container"
    );

    // × closes like Cancel: nothing is saved.
    keystrokes(vcx, "b e a c h");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.album_draft.album_title, "beach",
            "the name took the typing"
        )
    });
    click(vcx, "album-builder-close");
    view.update(vcx, |app, _| {
        assert!(!app.album_draft.modal_open, "× closes the builder");
        assert!(app.user_smart_albums.is_empty(), "× must not save");
    });
    assert!(engine.list_smart_albums().is_empty(), "× must not save");

    let _ = std::fs::remove_dir_all(&dir);
}

/// Clicks the open popup's option labelled `label`.
fn pick_option(
    vcx: &mut VisualTestContext,
    view: &Entity<LuminaApp>,
    target: DraftFocus,
    label: &str,
) {
    let i = view
        .update(vcx, |app, _| app.draft_popup_options(target))
        .iter()
        .position(|l| l == label)
        .unwrap_or_else(|| panic!("the popup offers {label}"));
    click(vcx, leak(format!("album-builder-option-{i}")));
}

#[core::prelude::v1::test]
fn condition_rows_match_design() {
    let (dir, engine, _ids) = library("sab_rows_design");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_sheet(vcx, &view);
    render_ui(vcx);

    let size = |vcx: &mut VisualTestContext, selector: &'static str| {
        let b = vcx.debug_bounds(selector).expect(selector);
        (f32::from(b.size.width), f32::from(b.size.height))
    };
    assert_eq!(
        size(vcx, "album-rule-field-0"),
        (210.0, 38.0),
        "field 210x38"
    );
    assert_eq!(size(vcx, "album-rule-op-0").0, 140.0, "operator 140 wide");
    assert_eq!(size(vcx, "album-rule-remove-0"), (34.0, 34.0), "− is 34x34");
    assert_eq!(size(vcx, "album-rule-add-0"), (34.0, 34.0), "+ is 34x34");

    let labels = view.update(vcx, |app, _| app.draft_popup_options(DraftFocus::Field(0)));
    for label in ["Date Captured", "Keywords", "Rating"] {
        assert!(
            labels.iter().any(|l| l == label),
            "the field popup lists {label}: {labels:?}"
        );
    }

    // A second row: Date Captured is in the last N <unit>.
    click(vcx, "album-rule-add-0");
    click(vcx, "album-rule-field-1");
    pick_option(vcx, &view, DraftFocus::Field(1), "Date Captured");
    click(vcx, "album-rule-op-1");
    pick_option(vcx, &view, DraftFocus::Operator(1), "is in the last");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.album_draft.rows[1].field, RuleField::Date);
        assert_eq!(app.album_draft.rows[1].operator, RuleOperator::InTheLast);
    });
    assert_eq!(
        size(vcx, "album-rule-value-1"),
        (66.0, 38.0),
        "number box 66x38"
    );
    assert!(
        size(vcx, "album-rule-unit-1").0 > 66.0,
        "the unit picker fills the rest of the row"
    );

    let first = vcx.debug_bounds("album-rule-field-0").unwrap();
    let second = vcx.debug_bounds("album-rule-field-1").unwrap();
    assert_eq!(
        f32::from(second.top() - first.bottom()),
        10.0,
        "rows are 10 apart with no divider between them"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn rating_stars_click_and_arrow_keys() {
    let (dir, engine, _ids) = library("sab_rating_stars");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_sheet(vcx, &view);
    render_ui(vcx);
    click(vcx, "album-rule-field-0");
    pick_option(vcx, &view, DraftFocus::Field(0), "Rating");
    click(vcx, "album-rule-op-0");
    pick_option(vcx, &view, DraftFocus::Operator(0), "is at least");
    render_ui(vcx);
    for n in 1..=5 {
        assert!(
            vcx.debug_bounds(leak(format!("album-rule-star-0-{n}")))
                .is_some(),
            "the Rating row shows star {n}"
        );
    }

    click(vcx, "album-rule-star-0-4");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.album_draft.rows[0].text, "4", "star 4 sets the value");
        assert_eq!(
            app.album_draft.focus,
            DraftFocus::Value(0),
            "the stars take focus"
        );
    });
    assert!(
        vcx.debug_bounds("album-rule-stars-label-0").is_some(),
        "the stars are followed by their label"
    );
    assert_eq!(
        crate::app::smart_draft::rating_value_label(&RuleOperator::AtLeast, "4"),
        "4 stars & up"
    );
    // The AccessKit tree is not visible to the harness, so the star buttons'
    // aria label is checked through the function that builds it.
    assert_eq!(crate::i18n::stars(4), "4 stars");
    assert_eq!(crate::i18n::stars(1), "1 star");

    keystrokes(vcx, "space enter down");
    view.update(vcx, |app, _| {
        assert_eq!(app.album_draft.popup, None, "a Rating value opens no popup");
        assert_eq!(app.album_draft.rows[0].text, "4");
    });
    keystrokes(vcx, "left");
    view.update(vcx, |app, _| {
        assert_eq!(app.album_draft.rows[0].text, "3", "← lowers the rating")
    });
    keystrokes(vcx, "right right right right");
    view.update(vcx, |app, _| {
        assert_eq!(app.album_draft.rows[0].text, "5", "→ stops at 5")
    });
    keystrokes(vcx, "left left");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.draft_rules().unwrap()[0],
            SmartAlbumRule {
                field: RuleField::Rating,
                operator: RuleOperator::AtLeast,
                value: RuleValue::Integer(3),
            },
            "the rule the builder saves is unchanged"
        )
    });
    keystrokes(vcx, "secondary-enter");
    let saved = engine.list_smart_albums();
    assert_eq!(saved.len(), 1, "⌘↵ saves the album");
    // The catalog's untagged JSON reads a whole number back as Number.
    assert_eq!(
        saved[0].rules[0],
        SmartAlbumRule {
            field: RuleField::Rating,
            operator: RuleOperator::AtLeast,
            value: RuleValue::Number(3.0),
        },
        "the catalog holds Rating at least 3"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn footer_preview_and_pin_option() {
    use crate::views::album_builder::footer::{footer_status, more_label};
    let (dir, engine) = seed_library("sab_footer", 10);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_sheet(vcx, &view);
    render_ui(vcx);

    // Zero matches (no favourites): one empty line, no More tile.
    view.update(vcx, |app, _| {
        assert_eq!(app.album_draft.match_count, Some(0));
        assert_eq!(footer_status(&app.album_draft).0, "No items match");
    });
    assert!(
        vcx.debug_bounds("album-builder-preview-empty").is_some(),
        "zero matches show one empty-state line"
    );
    assert!(
        vcx.debug_bounds("album-builder-more").is_none(),
        "zero matches hide the More tile"
    );

    // Favorite is No: all 10 match; 8 tiles and "+2 more".
    click(vcx, "album-rule-value-0");
    pick_option(vcx, &view, DraftFocus::Value(0), "No");
    render_ui(vcx);
    let preview = view.update(vcx, |app, _| {
        assert_eq!(app.album_draft.match_count, Some(10));
        assert_eq!(footer_status(&app.album_draft).0, "10 items match");
        app.album_draft.preview.clone()
    });
    assert_eq!(preview.len(), 8);
    assert!(
        vcx.debug_bounds("album-builder-more").is_some(),
        "10 > 8 shows More"
    );
    assert!(
        vcx.debug_bounds("album-builder-more-text:+2").is_some(),
        "the More tile reads +2"
    );
    assert_eq!(
        more_label(Some(1248), 8).as_deref(),
        Some("+1,240"),
        "grouped digits"
    );
    assert_eq!(more_label(Some(3), 3), None, "never \"+0\" or negative");
    let tile = vcx
        .debug_bounds(leak(format!("album-builder-preview-{}", preview[0].id.0)))
        .unwrap();
    assert_eq!(f32::from(tile.size.height), 96.0, "tiles are 96 tall");
    let width = f32::from(tile.size.width);
    assert!(
        (68.0..=92.0).contains(&width),
        "tile width {width} is 68-92"
    );

    // Three favourites, Favorite is Yes: no More tile.
    let ids: Vec<_> = engine.get_item_slice(0..3).iter().map(|i| i.id).collect();
    for id in ids {
        engine.toggle_favorite(id, true).unwrap();
    }
    click(vcx, "album-rule-value-0");
    pick_option(vcx, &view, DraftFocus::Value(0), "Yes");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.album_draft.match_count, Some(3))
    });
    assert!(
        vcx.debug_bounds("album-builder-more").is_none(),
        "3 ≤ 8: no More"
    );

    assert!(
        vcx.debug_bounds("album-builder-save-text:Create Smart Album")
            .is_some(),
        "a new album's button reads Create Smart Album"
    );

    // Pin: Space on the focused checkbox, a click toggles it too.
    view.update(vcx, |app, cx| app.set_draft_focus(DraftFocus::Pin, cx));
    keystrokes(vcx, "space");
    view.update(vcx, |app, _| {
        assert!(app.album_draft.pin, "Space checks Pin")
    });
    click(vcx, "album-builder-pin");
    view.update(vcx, |app, _| {
        assert!(!app.album_draft.pin, "a click unchecks it")
    });
    click(vcx, "album-builder-pin");
    click(vcx, "album-builder-save");
    let saved = engine.list_smart_albums();
    assert_eq!(saved.len(), 1);
    assert!(saved[0].pinned, "Pin to sidebar creates the album pinned");

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn editing_a_pinned_album_keeps_or_clears_the_pin() {
    let (dir, engine, _ids) = library("sab_edit_pin");
    let rule = SmartAlbumRule {
        field: RuleField::IsFavorite,
        operator: RuleOperator::Equals,
        value: RuleValue::Bool(true),
    };
    let id = engine
        .create_smart_album("Pinned", "folder", MatchMode::All, &[rule])
        .unwrap()
        .id;
    engine.set_smart_album_pinned(id, true).unwrap();
    let saved = || engine.list_smart_albums().into_iter().next().unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let db = rusqlite::Connection::open(dir.join("catalog.db")).unwrap();
    // Any pin write now fails, so an unchanged save must not write one.
    db.execute_batch(
        "CREATE TRIGGER fail_pin BEFORE UPDATE OF pinned ON smart_albums
         BEGIN SELECT RAISE(ABORT, 'injected'); END;",
    )
    .unwrap();

    let album = saved();
    view.update(vcx, |app, cx| app.open_album_builder_for_edit(album, cx));
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.album_draft.pin, "Edit starts checked")
    });
    assert!(
        vcx.debug_bounds("album-builder-pin-text:on").is_some(),
        "the Pin checkbox renders checked"
    );
    assert!(
        vcx.debug_bounds("album-builder-save-text:Save Smart Album")
            .is_some(),
        "editing reads Save Smart Album"
    );
    click(vcx, "album-builder-save");
    view.update(vcx, |app, _| {
        assert_eq!(app.error_message, None, "an unchanged pin is not written");
        assert!(!app.album_draft.modal_open);
    });
    assert!(saved().pinned, "still pinned");

    db.execute_batch("DROP TRIGGER fail_pin;").unwrap();
    let album = saved();
    view.update(vcx, |app, cx| app.open_album_builder_for_edit(album, cx));
    render_ui(vcx);
    click(vcx, "album-builder-pin");
    click(vcx, "album-builder-save");
    assert!(!saved().pinned, "unchecking Pin unpins the album");

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn pin_failure_shows_the_error_and_keeps_the_album() {
    let (dir, engine, _ids) = library("sab_pin_fail");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    rusqlite::Connection::open(dir.join("catalog.db"))
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER fail_pin BEFORE UPDATE OF pinned ON smart_albums
             BEGIN SELECT RAISE(ABORT, 'injected'); END;",
        )
        .unwrap();
    open_sheet(vcx, &view);
    render_ui(vcx);
    click(vcx, "album-builder-pin");
    click(vcx, "album-builder-save");

    let saved = engine.list_smart_albums();
    assert_eq!(saved.len(), 1, "a failed pin keeps the album");
    assert!(!saved[0].pinned);
    view.update(vcx, |app, _| {
        let error = app.error_message.clone().unwrap_or_default();
        assert!(
            error.contains("injected"),
            "the banner names the cause: {error:?}"
        );
        assert!(!app.album_draft.modal_open, "the album was created");
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn keywords_tokens_add_and_remove() {
    let (dir, engine, ids) = library("sab_tokens");
    engine
        .add_tags_batch(&[ids[2]], &["sunset".to_string()])
        .unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_sheet(vcx, &view);
    render_ui(vcx);
    click(vcx, "album-rule-field-0");
    pick_option(vcx, &view, DraftFocus::Field(0), "Keywords");
    click(vcx, "album-rule-op-0");
    pick_option(vcx, &view, DraftFocus::Operator(0), "include any of");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.album_draft.match_count, None,
            "an empty list is invalid"
        )
    });

    let add = |vcx: &mut VisualTestContext, tag: &str| {
        click(vcx, "album-rule-value-0");
        pick_option(vcx, &view, DraftFocus::Value(0), tag);
        render_ui(vcx);
    };
    add(vcx, "beach");
    let offered = view.update(vcx, |app, _| app.draft_popup_options(DraftFocus::Value(0)));
    assert_eq!(
        offered,
        vec!["sunset".to_string()],
        "a chosen keyword is not offered again"
    );
    add(vcx, "sunset");
    for tag in ["beach", "sunset"] {
        assert!(
            vcx.debug_bounds(leak(format!("album-rule-token-0-{tag}")))
                .is_some(),
            "{tag} shows as a chip"
        );
    }
    view.update(vcx, |app, _| {
        assert_eq!(app.album_draft.match_count, Some(2), "beach or sunset")
    });

    click(vcx, "album-rule-token-remove-0-beach");
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("album-rule-token-0-beach").is_none(),
        "× removes the chip"
    );
    view.update(vcx, |app, _| {
        assert_eq!(app.album_draft.rows[0].list, vec!["sunset".to_string()]);
        assert_eq!(app.album_draft.focus, DraftFocus::Value(0));
    });

    add(vcx, "beach");
    keystrokes(vcx, "backspace");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.album_draft.rows[0].list,
            vec!["sunset".to_string()],
            "⌫ drops the last chip"
        )
    });
    add(vcx, "beach");
    keystrokes(vcx, "secondary-enter");
    let saved = engine.list_smart_albums();
    assert_eq!(saved.len(), 1);
    assert_eq!(
        saved[0].rules[0],
        SmartAlbumRule {
            field: RuleField::Tag,
            operator: RuleOperator::AnyOf,
            value: RuleValue::List(vec!["sunset".into(), "beach".into()]),
        }
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// Sets photo `i`'s camera straight in the catalog file, as a scan would.
fn set_camera(dir: &std::path::Path, id: MediaId, make: Option<&str>, model: Option<&str>) {
    rusqlite::Connection::open(dir.join("catalog.db"))
        .unwrap()
        .execute(
            "UPDATE media_items SET make = ?1, model = ?2 WHERE id = ?3",
            rusqlite::params![make, model, id.0 as i64],
        )
        .unwrap();
}

fn camera_is_row(vcx: &mut VisualTestContext, view: &Entity<LuminaApp>) {
    click(vcx, "album-rule-field-0");
    pick_option(vcx, view, DraftFocus::Field(0), "Camera");
    click(vcx, "album-rule-op-0");
    let ops = view.update(vcx, |app, _| {
        app.draft_popup_options(DraftFocus::Operator(0))
    });
    assert_eq!(
        ops,
        ["is", "is not", "contains", "does not contain"],
        "Camera offers is / is not and keeps contains"
    );
    pick_option(vcx, view, DraftFocus::Operator(0), "is");
    click(vcx, "album-rule-value-0");
    render_ui(vcx);
}

#[core::prelude::v1::test]
fn camera_is_offers_library_cameras() {
    let (dir, engine, ids) = library("sab_camera");
    set_camera(&dir, ids[0], Some("Canon"), Some("Canon EOS R5"));
    set_camera(&dir, ids[1], Some("FUJIFILM"), Some("X-T5"));
    set_camera(&dir, ids[2], None, None);
    set_camera(&dir, ids[3], None, None);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_sheet(vcx, &view);
    render_ui(vcx);
    // Loaded once when the builder opens, not per frame: a camera added
    // now shows the next time the builder opens.
    set_camera(&dir, ids[2], Some("Sony"), Some("ILCE-7M4"));
    camera_is_row(vcx, &view);
    let offered = view.update(vcx, |app, _| app.draft_popup_options(DraftFocus::Value(0)));
    assert_eq!(
        offered,
        ["Canon EOS R5", "FUJIFILM X-T5"],
        "the library's cameras, named as the inspector names them"
    );

    pick_option(vcx, &view, DraftFocus::Value(0), "Canon EOS R5");
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("album-rule-value-text:Canon EOS R5")
            .is_some(),
        "the value picker shows the chosen camera"
    );
    view.update(vcx, |app, _| {
        assert_eq!(app.album_draft.match_count, Some(1), "one Canon photo")
    });
    keystrokes(vcx, "secondary-enter");
    let saved = engine.list_smart_albums();
    assert_eq!(saved.len(), 1);
    assert_eq!(
        saved[0].rules[0],
        SmartAlbumRule {
            field: RuleField::Camera,
            operator: RuleOperator::Equals,
            value: RuleValue::List(vec!["Canon".into(), "Canon EOS R5".into()]),
        },
        "the rule keeps the stored make and model"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn camera_is_with_no_cameras_says_so() {
    let (dir, engine, ids) = library("sab_no_camera");
    for id in ids {
        set_camera(&dir, id, None, None);
    }
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_sheet(vcx, &view);
    render_ui(vcx);
    camera_is_row(vcx, &view);
    assert!(
        vcx.debug_bounds("album-builder-popup-text:No cameras in this library yet")
            .is_some(),
        "an empty camera list says so instead of a blank popup"
    );

    for key in ["enter", "space", "down"] {
        keystrokes(vcx, "escape");
        render_ui(vcx);
        assert!(
            vcx.debug_bounds("album-builder-popup").is_none(),
            "Esc closes the popup"
        );
        view.update(vcx, |app, _| {
            assert_eq!(app.album_draft.focus, DraftFocus::Value(0))
        });
        keystrokes(vcx, key);
        render_ui(vcx);
        assert!(
            vcx.debug_bounds("album-builder-popup-text:No cameras in this library yet")
                .is_some(),
            "{key} opens the same empty popup a click does"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn keyword_chips_remove_from_the_keyboard() {
    let (dir, engine, ids) = library("sab_token_keys");
    engine
        .add_tags_batch(&[ids[2]], &["sunset".to_string()])
        .unwrap();
    engine
        .add_tags_batch(&[ids[3]], &["city".to_string()])
        .unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_sheet(vcx, &view);
    render_ui(vcx);
    click(vcx, "album-rule-field-0");
    pick_option(vcx, &view, DraftFocus::Field(0), "Keywords");
    click(vcx, "album-rule-op-0");
    pick_option(vcx, &view, DraftFocus::Operator(0), "include any of");
    for tag in ["beach", "sunset", "city"] {
        // Enter opens the popup on the focused list; picking adds a chip.
        view.update(vcx, |app, cx| app.set_draft_focus(DraftFocus::Value(0), cx));
        keystrokes(vcx, "enter");
        let i = view
            .update(vcx, |app, _| app.draft_popup_options(DraftFocus::Value(0)))
            .iter()
            .position(|t| t == tag)
            .unwrap();
        for _ in 0..i {
            keystrokes(vcx, "down");
        }
        keystrokes(vcx, "enter");
    }
    let list = |vcx: &mut VisualTestContext| {
        view.update(vcx, |app, _| app.album_draft.rows[0].list.clone())
    };
    assert_eq!(list(vcx), vec!["beach", "sunset", "city"]);

    // ← ← highlights the middle chip; ⌫ removes it, not the last.
    keystrokes(vcx, "left left");
    view.update(vcx, |app, _| assert_eq!(app.album_draft.token, Some(1)));
    keystrokes(vcx, "backspace");
    assert_eq!(
        list(vcx),
        vec!["beach", "city"],
        "⌫ removes the highlighted chip"
    );
    // ← once more highlights the last; → past it clears the highlight.
    keystrokes(vcx, "left right");
    view.update(vcx, |app, _| assert_eq!(app.album_draft.token, None));
    keystrokes(vcx, "left left left backspace");
    assert_eq!(list(vcx), vec!["city"], "← stops at the first chip");

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn chip_highlight_does_not_follow_focus_to_another_row() {
    let (dir, engine, _ids) = library("sab_token_stale");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_sheet(vcx, &view);
    view.update(vcx, |app, cx| {
        let keywords = |list: &[&str]| {
            let mut row = crate::app::smart_draft::DraftRow::new(RuleField::Tag);
            row.set_operator(RuleOperator::AnyOf);
            row.list = list.iter().map(|t| t.to_string()).collect();
            row
        };
        app.album_draft.rows = vec![keywords(&["a", "b"]), keywords(&["c", "d"])];
        app.refresh_draft_preview();
        app.set_draft_focus(DraftFocus::Value(0), cx);
    });
    render_ui(vcx);
    keystrokes(vcx, "left left");
    view.update(vcx, |app, _| assert_eq!(app.album_draft.token, Some(0)));

    // A click on row 1's box moves focus there; row 0's highlight must not.
    click(vcx, "album-rule-value-1");
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert_eq!(app.album_draft.focus, DraftFocus::Value(1));
        assert_eq!(
            app.album_draft.token, None,
            "the highlight stays with its row"
        );
    });
    keystrokes(vcx, "backspace");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.album_draft.rows[1].list,
            vec!["c".to_string()],
            "⌫ removes row 1's last chip, not one the user never chose"
        )
    });

    // Closing and reopening starts without a highlight.
    view.update(vcx, |app, cx| {
        app.set_draft_focus(DraftFocus::Value(0), cx);
    });
    keystrokes(vcx, "left");
    view.update(vcx, |app, cx| {
        app.close_album_builder(cx);
        app.open_album_builder(cx);
        assert_eq!(
            app.album_draft.token, None,
            "a new session has no highlight"
        );
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn too_many_keywords_block_save() {
    let (dir, engine, _ids) = library("sab_too_many_keywords");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_sheet(vcx, &view);
    view.update(vcx, |app, cx| {
        let mut row = crate::app::smart_draft::DraftRow::new(RuleField::Tag);
        row.set_operator(RuleOperator::AnyOf);
        row.list = (0..1_001).map(|i| format!("k{i}")).collect();
        app.album_draft.rows = vec![row];
        app.refresh_draft_preview();
        app.save_draft_smart_album(cx);
        assert_eq!(
            app.album_draft.problem.as_deref(),
            Some("Too many keywords in this album"),
            "the footer says why"
        );
        assert!(app.album_draft.modal_open, "OK refuses to save it");
    });
    assert!(engine.list_smart_albums().is_empty());

    let _ = std::fs::remove_dir_all(&dir);
}

/// Opens the builder at 1200x900 with `rows` conditions.
fn sheet_with_rows(vcx: &mut VisualTestContext, view: &Entity<LuminaApp>, rows: usize) {
    vcx.simulate_resize(gpui::size(gpui::px(1200.0), gpui::px(900.0)));
    open_sheet(vcx, view);
    view.update(vcx, |app, cx| {
        while app.album_draft.rows.len() < rows {
            let last = app.album_draft.rows.len() - 1;
            app.add_draft_row_after(last, cx);
        }
        // Back to the top: adding rows scrolled to the last one.
        app.set_draft_focus(DraftFocus::Field(0), cx);
    });
    render_ui(vcx);
}

#[core::prelude::v1::test]
fn many_conditions_scroll_inside_a_dialog_that_fits_the_window() {
    let (dir, engine, _ids) = library("sab_many_rows");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    sheet_with_rows(vcx, &view, 15);

    let card = vcx.debug_bounds("album-builder-card").unwrap();
    assert!(
        f32::from(card.size.height) <= 900.0 - 80.0,
        "the dialog is at most the window height − 80: {:?}",
        card.size.height
    );
    for selector in ["album-builder-close", "album-builder-save"] {
        let b = vcx.debug_bounds(selector).unwrap();
        assert!(
            card.contains(&b.origin) && card.contains(&b.bottom_right()),
            "{selector} stays visible inside the dialog: {b:?} in {card:?}"
        );
    }
    let overflow = view.update(vcx, |app, _| app.album_draft.scroll.max_offset().y);
    assert!(
        overflow > gpui::px(0.0),
        "15 conditions overflow the conditions box, which scrolls"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn tab_to_a_row_below_the_fold_scrolls_it_into_view() {
    let (dir, engine, _ids) = library("sab_tab_scroll");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    sheet_with_rows(vcx, &view, 15);

    // Shift-Tab from row 0: Match, Title, wrap to Save, Cancel, Pin, row 14's Add.
    let rows = vcx.debug_bounds("album-builder-conditions").unwrap();
    let add = vcx.debug_bounds("album-rule-add-14").unwrap();
    assert!(add.top() > rows.bottom(), "row 14 starts below the fold");
    keystrokes(
        vcx,
        "shift-tab shift-tab shift-tab shift-tab shift-tab shift-tab",
    );
    view.update(vcx, |app, _| {
        assert_eq!(app.album_draft.focus, DraftFocus::Add(14))
    });
    render_ui(vcx);
    render_ui(vcx);
    let rows = vcx.debug_bounds("album-builder-conditions").unwrap();
    let add = vcx.debug_bounds("album-rule-add-14").unwrap();
    assert!(
        add.top() >= rows.top() && add.bottom() <= rows.bottom(),
        "the focused control is scrolled into the conditions box: {add:?} in {rows:?}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn a_popup_on_the_bottom_row_scrolls_into_view() {
    let (dir, engine, _ids) = library("sab_popup_scroll");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    sheet_with_rows(vcx, &view, 15);

    // Open it from state: the row-14 picker is below the fold, so a click cannot reach it.
    view.update(vcx, |app, cx| {
        app.toggle_draft_popup(DraftFocus::Field(14), cx)
    });
    render_ui(vcx);
    render_ui(vcx);
    let rows = vcx.debug_bounds("album-builder-conditions").unwrap();
    let popup = vcx.debug_bounds("album-builder-popup").expect("popup");
    assert!(
        popup.top() >= rows.top() && popup.bottom() <= rows.bottom(),
        "the bottom row's popup is scrolled into the conditions box: {popup:?} in {rows:?}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn removing_the_last_condition_matches_everything() {
    let (dir, engine, _ids) = library("sab_v3_empty");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_sheet(vcx, &view);
    render_ui(vcx);

    click(vcx, "album-rule-remove-0");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.album_draft.rows.is_empty(), "− works on the last row");
        assert_eq!(app.album_draft.match_count, Some(4), "every item matches");
        assert_eq!(app.album_draft.focus, DraftFocus::Add(0));
    });
    assert!(vcx.debug_bounds("album-builder-empty").is_some());

    // Keyboard: Space on the focused Add brings a condition back.
    keystrokes(vcx, "space");
    view.update(vcx, |app, _| {
        assert_eq!(app.album_draft.rows.len(), 1);
        assert_eq!(app.album_draft.match_count, Some(2));
    });
    click(vcx, "album-rule-remove-0");
    keystrokes(vcx, "secondary-enter");
    let saved = engine.list_smart_albums();
    assert_eq!(saved.len(), 1, "a no-condition album saves");
    assert!(saved[0].rules.is_empty());
    view.update(vcx, |app, _| assert_eq!(app.get_active_count(), 4));

    // Reopening for edit keeps it empty; OK (e.g. a rename) must not add a rule.
    view.update(vcx, |app, cx| app.edit_selected_smart_album(cx));
    view.update(vcx, |app, _| {
        assert!(app.album_draft.rows.is_empty(), "no default row on edit")
    });
    keystrokes(vcx, "x secondary-enter");
    let saved = engine.list_smart_albums();
    assert!(saved[0].rules.is_empty(), "the edit kept every item");

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn save_search_opens_prefilled_builder_and_creates_album() {
    let (dir, engine, _ids) = library("sab_save_search");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let query = "photo is:favorite";
    let listed = |app: &LuminaApp| {
        use crate::app::command_palette::{CommandId, PaletteTarget};
        app.palette_rows()
            .iter()
            .any(|row| row.target == PaletteTarget::Command(CommandId::SaveSearch))
    };
    view.update(vcx, |app, _| {
        app.command_query = "save search".into();
        assert!(!listed(app), "no search to save: the command is hidden");
        app.search_query = "   ".into();
        assert!(!listed(app), "a blank search is not saveable either");
        app.search_query.clear();
        app.command_query.clear();
    });

    click(vcx, "search-input");
    type_text(vcx, query);
    let search_count = view.update(vcx, |app, _| {
        assert_eq!(app.search_query, query);
        app.get_active_count()
    });
    assert_eq!(search_count, 2, "the two favourites match the search");
    for _ in 0..2 {
        if view.update(vcx, |app, _| app.search_focused) {
            keystrokes(vcx, "escape");
        }
    }
    focus_app(vcx, &view);

    keystrokes(vcx, "secondary-k s a v e space s e a r c h enter");
    view.update(vcx, |app, _| {
        assert!(
            app.album_draft.modal_open,
            "⌘K Save Search opens the builder"
        );
        assert_eq!(app.album_draft.editing_album_id, None, "a new album");
        assert_eq!(app.album_draft.album_title, query, "the query is the title");
        assert_eq!(
            app.draft_rules(),
            Some(vec![SmartAlbumRule {
                field: RuleField::Query,
                operator: RuleOperator::Equals,
                value: RuleValue::Text(query.into()),
            }]),
            "one Query rule holding the raw search text"
        );
        assert_eq!(
            app.album_draft.match_count,
            Some(search_count),
            "the preview counts what the search showed"
        );
    });

    keystrokes(vcx, "secondary-enter");
    view.update(vcx, |app, _| {
        assert!(!app.album_draft.modal_open, "OK saves and closes the sheet");
        let album = app.user_smart_albums.first().expect("album saved");
        assert_eq!(app.selection, AlbumSelection::User(album.id));
        assert_eq!(
            app.user_album_counts.get(&album.id).copied(),
            Some(search_count),
            "the sidebar row shows the search's count"
        );
        assert_eq!(app.get_active_count(), search_count);
    });
    let saved = engine.list_smart_albums();
    assert_eq!(saved.len(), 1, "the album is persisted in the catalog");
    assert_eq!(
        saved[0].rules[0].value,
        RuleValue::Text(query.into()),
        "the raw query text is stored"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn save_search_title_is_cut_on_a_char_boundary() {
    assert_eq!(
        crate::app::album_builder_actions::saved_search_title(&"é".repeat(70)),
        "é".repeat(60),
        "at most 60 characters, never a split character"
    );
    assert_eq!(
        crate::app::album_builder_actions::saved_search_title("  sunset  "),
        "sunset"
    );
}

#[core::prelude::v1::test]
fn saved_search_counts_follow_caption_and_location_edits() {
    let (dir, engine, ids) = library("sab_saved_counts");
    let saved = |text: &str| {
        engine
            .create_smart_album(
                text,
                "sparkles",
                MatchMode::All,
                &[SmartAlbumRule {
                    field: RuleField::Query,
                    operator: RuleOperator::Equals,
                    value: RuleValue::Text(text.into()),
                }],
            )
            .expect("create saved search")
            .id
    };
    let sunset = saved("sunset");
    let located = saved("has:location");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let counts = |vcx: &mut VisualTestContext| {
        view.update(vcx, |app, _| {
            (
                app.user_album_counts.get(&sunset).copied(),
                app.user_album_counts.get(&located).copied(),
            )
        })
    };
    assert_eq!(counts(vcx), (Some(0), Some(0)), "nothing matches yet");

    view.update(vcx, |app, cx| {
        app.selected_item = engine.get_item_by_id(ids[2]);
        app.start_caption_input(cx);
        app.caption.input_buffer = "Sunset over the bay".into();
        app.commit_caption_input(cx);
    });
    assert_eq!(
        counts(vcx),
        (Some(1), Some(0)),
        "a caption edit alone refreshes saved-search counts"
    );
    view.update(vcx, |app, cx| {
        app.start_location_input(cx);
        app.caption.location_input_buffer = "48.8566, 2.3522".into();
        app.commit_location_input(cx);
    });
    assert_eq!(
        counts(vcx),
        (Some(1), Some(1)),
        "a location edit alone refreshes saved-search counts"
    );

    view.update(vcx, |app, cx| {
        app.select_all(cx);
        app.apply_batch_caption(Some("sunset glow".into()), cx);
    });
    assert_eq!(
        counts(vcx),
        (Some(4), Some(1)),
        "a batch caption edit refreshes saved-search counts"
    );
    view.update(vcx, |app, cx| {
        app.apply_batch_location("35.6762, 139.6503", cx);
    });
    assert_eq!(
        counts(vcx),
        (Some(4), Some(4)),
        "a batch location edit refreshes saved-search counts"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// The placeholder the smart-album input shows while it is empty.
fn draft_placeholder(vcx: &mut VisualTestContext, view: &Entity<LuminaApp>) -> String {
    view.update(vcx, |app, cx| {
        match app.field_input_for(crate::app::text_input::TextTarget::AlbumDraft) {
            Some(crate::app::text_input::FieldEditor::Line(state)) => {
                state.read(cx).presentation().placeholder().to_string()
            }
            _ => panic!("a smart-album field takes typing"),
        }
    })
}

#[core::prelude::v1::test]
fn focused_empty_name_field_shows_the_untitled_placeholder() {
    let (dir, engine, _ids) = library("sab_placeholder");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_sheet(vcx, &view);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(
            app.album_draft.album_title.is_empty(),
            "a new album has no name"
        )
    });
    assert_eq!(
        draft_placeholder(vcx, &view),
        "Untitled Smart Album",
        "the name field is focused on open and must still show its placeholder"
    );

    // A condition's value is the same input target: it gets no name hint.
    view.update(vcx, |app, cx| {
        app.album_draft.rows[0] = crate::app::smart_draft::DraftRow::new(RuleField::Date);
        app.set_draft_focus(DraftFocus::Value(0), cx);
    });
    render_ui(vcx);
    assert_eq!(
        draft_placeholder(vcx, &view),
        "",
        "a condition value field does not show the album-name placeholder"
    );

    view.update(vcx, |app, cx| app.set_draft_focus(DraftFocus::Title, cx));
    render_ui(vcx);
    assert_eq!(
        draft_placeholder(vcx, &view),
        "Untitled Smart Album",
        "back on the name field the placeholder returns"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
