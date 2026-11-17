//! Pin, duplicate, and smart album rule files. The file
//! dialogs are stubbed: tests count the request and call the post-dialog
//! step with a path in the test library's temp folder.

use super::ui_automation::{
    app_with_library, click, focus_app, keystrokes, leak, render_ui, seed_library, type_text,
};
use crate::views::sidebar::AlbumSelection;
use lumina_core::{
    to_portable_json, LuminaEngine, MatchMode, RuleField, RuleOperator, RuleValue, SmartAlbum,
    SmartAlbumRule,
};

fn rule(field: RuleField, operator: RuleOperator, value: RuleValue) -> SmartAlbumRule {
    SmartAlbumRule {
        field,
        operator,
        value,
    }
}

fn two_rule_album(engine: &LuminaEngine, title: &str) -> SmartAlbum {
    let rules = [
        rule(
            RuleField::IsFavorite,
            RuleOperator::Equals,
            RuleValue::Bool(true),
        ),
        rule(
            RuleField::Filename,
            RuleOperator::Contains,
            RuleValue::Text("photo".into()),
        ),
    ];
    engine
        .create_smart_album(title, "folder", MatchMode::Any, &rules)
        .unwrap()
}

fn dialogs() -> usize {
    crate::app::smart_album_share::RULE_FILE_DIALOGS.with(std::cell::Cell::get)
}

fn palette(vcx: &mut gpui::VisualTestContext, command: &str) {
    keystrokes(vcx, "cmd-k");
    type_text(vcx, command);
    keystrokes(vcx, "enter");
    render_ui(vcx);
}

fn top(vcx: &mut gpui::VisualTestContext, selector: String) -> f32 {
    let bounds = vcx
        .debug_bounds(leak(selector.clone()))
        .unwrap_or_else(|| panic!("{selector} is drawn"));
    f32::from(bounds.top())
}

#[core::prelude::v1::test]
fn pinned_albums_render_first() {
    let (dir, engine) = seed_library("album_pinned_first", 1);
    let loose = engine.create_album("Loose").unwrap();
    let kept = engine.create_album("Kept").unwrap();
    assert!(engine.set_album_pinned(kept.id, true).unwrap());
    let smart_loose = two_rule_album(&engine, "Smart loose");
    let smart_kept = two_rule_album(&engine, "Smart kept");
    assert!(engine.set_smart_album_pinned(smart_kept.id, true).unwrap());
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    let pinned = top(vcx, "sidebar-pinned-header".into());
    let albums = top(vcx, "sidebar-albums-header".into());
    let kept_row = top(vcx, format!("album-row-{}", kept.id));
    let smart_kept_row = top(vcx, format!("smart-album-row-{}", smart_kept.id));
    let loose_row = top(vcx, format!("album-row-{}", loose.id));
    let smart_loose_row = top(vcx, format!("smart-album-row-{}", smart_loose.id));
    assert!(
        pinned < kept_row && kept_row < smart_kept_row && smart_kept_row < albums,
        "pinned albums, then pinned smart albums, sit in the Pinned group above ALBUMS"
    );
    assert!(
        albums < loose_row && loose_row < smart_loose_row,
        "unpinned albums stay in their own sections"
    );
    // debug_bounds keeps one entry per selector, so count by height: the
    // Pinned group holds its header and exactly two rows, ALBUMS its header
    // and exactly one plus the New Album row. A pinned row drawn twice
    // would add a row to ALBUMS.
    let row = 32.0;
    let height = |vcx: &mut gpui::VisualTestContext, selector: &'static str| {
        f32::from(vcx.debug_bounds(selector).expect(selector).size.height)
    };
    let pinned_rows = height(vcx, "sidebar-pinned-section") - height(vcx, "sidebar-pinned-header");
    let album_rows = height(vcx, "sidebar-albums-section") - height(vcx, "sidebar-albums-header");
    assert!(
        (2.0 * row..3.0 * row).contains(&pinned_rows),
        "the Pinned group draws each pinned row once: {pinned_rows}px of rows"
    );
    assert!(
        (2.0 * row..3.0 * row).contains(&album_rows),
        "ALBUMS draws only the unpinned album, once, then New Album: {album_rows}px of rows"
    );

    // Keyboard path: open the loose album, ⌘K Pin Album.
    click(vcx, leak(format!("album-row-{}", loose.id)));
    focus_app(vcx, &view);
    palette(vcx, "pin album");
    let stored = engine.list_albums_with_counts();
    assert!(
        stored.iter().any(|(a, _)| a.id == loose.id && a.pinned),
        "Pin Album writes the pin to the catalog: {stored:?}"
    );
    assert!(
        top(vcx, format!("album-row-{}", loose.id)) < top(vcx, "sidebar-albums-header".into()),
        "a newly pinned album moves into the Pinned group"
    );

    palette(vcx, "unpin album");
    assert!(
        engine
            .list_albums_with_counts()
            .iter()
            .any(|(a, _)| a.id == loose.id && !a.pinned),
        "Unpin Album clears the pin in the catalog"
    );
    assert!(
        top(vcx, format!("album-row-{}", loose.id)) > top(vcx, "sidebar-albums-header".into()),
        "an unpinned album returns to ALBUMS"
    );

    // Smart albums pin from their own view too.
    click(vcx, leak(format!("smart-album-row-{}", smart_loose.id)));
    focus_app(vcx, &view);
    palette(vcx, "pin album");
    assert!(
        engine
            .list_smart_albums()
            .iter()
            .any(|a| a.id == smart_loose.id && a.pinned),
        "Pin Album pins the open smart album"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn pin_write_failure_shows_banner_and_keeps_order() {
    let (dir, engine) = seed_library("album_pin_fail", 1);
    let album = engine.create_album("Trip").unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    click(vcx, leak(format!("album-row-{}", album.id)));
    focus_app(vcx, &view);

    crate::app::albums::ALBUM_WRITE_FAILS.with(|f| f.set(true));
    palette(vcx, "pin album");
    crate::app::albums::ALBUM_WRITE_FAILS.with(|f| f.set(false));

    assert!(
        !engine.list_albums_with_counts()[0].0.pinned,
        "nothing was written"
    );
    view.update(vcx, |app, _| {
        assert!(
            app.error_message
                .as_deref()
                .is_some_and(|m| m.starts_with("Could not pin album")),
            "the failure is in the banner: {:?}",
            app.error_message
        );
        assert!(!app.albums.list[0].pinned, "the sidebar shows no false pin");
    });
    assert!(
        vcx.debug_bounds("sidebar-pinned-header").is_none(),
        "no Pinned group without a pinned album"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn duplicate_smart_album_copies_rules() {
    let (dir, engine) = seed_library("smart_duplicate", 1);
    let original = two_rule_album(&engine, "Faves");
    assert!(engine.set_smart_album_pinned(original.id, true).unwrap());
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    click(vcx, leak(format!("smart-album-row-{}", original.id)));
    focus_app(vcx, &view);

    palette(vcx, "duplicate smart album");

    let albums = engine.list_smart_albums();
    assert_eq!(albums.len(), 2, "the catalog holds the original and a copy");
    let copy = albums
        .iter()
        .find(|a| a.id != original.id)
        .expect("the copy");
    assert_eq!(copy.title, "Faves copy");
    assert_eq!(copy.match_mode, MatchMode::Any, "the match mode is copied");
    assert_eq!(copy.rules, original.rules, "every rule is copied");
    assert!(!copy.pinned, "a copy starts unpinned");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::User(copy.id),
            "the copy opens, ready to edit"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn import_rejected_rule_shows_banner() {
    let (dir, engine) = seed_library("smart_import_rejected", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    let before = dialogs();
    palette(vcx, "import smart album rules");
    assert_eq!(dialogs(), before + 1, "the palette asks for a file");

    // A saved search that may not show deleted items, and a rule field
    // from a newer build.
    let trashed = SmartAlbum::new(
        "Deleted".into(),
        "folder".into(),
        MatchMode::All,
        vec![rule(
            RuleField::Query,
            RuleOperator::Equals,
            RuleValue::Text("is:trashed".into()),
        )],
    );
    let bad = dir.join("trashed.json");
    std::fs::write(&bad, to_portable_json(&trashed).unwrap()).unwrap();
    let newer = dir.join("newer.json");
    let newer_json = to_portable_json(&SmartAlbum::new(
        "Picks".into(),
        "folder".into(),
        MatchMode::All,
        vec![rule(
            RuleField::IsFavorite,
            RuleOperator::Equals,
            RuleValue::Bool(true),
        )],
    ))
    .unwrap()
    .replace(r#""is_favorite""#, r#""sharpness""#);
    std::fs::write(&newer, newer_json).unwrap();
    // Over 1 MiB, and not text: only the size check before reading can
    // say "too large".
    let huge = dir.join("huge.json");
    std::fs::write(&huge, vec![0xFF; lumina_core::MAX_PORTABLE_BYTES + 1]).unwrap();

    for (path, why) in [
        (&bad, "cannot show deleted items"),
        (&newer, "does not know"),
        (&huge, "too large"),
    ] {
        view.update(vcx, |app, cx| {
            app.import_smart_album_rules_from(path.clone(), cx)
        });
        vcx.run_until_parked();
        view.update(vcx, |app, cx| {
            let message = app.error_message.clone().unwrap_or_default();
            assert!(
                message.starts_with("Could not import smart album rules") && message.contains(why),
                "{path:?}: the banner says why, got {message:?}"
            );
            app.dismiss_error(cx);
        });
    }
    assert!(
        engine.list_smart_albums().is_empty(),
        "a rejected file saves no smart album"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn export_then_import_round_trips_rules() {
    let (dir, engine) = seed_library("smart_export_import", 1);
    let original = two_rule_album(&engine, "Faves");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    click(vcx, leak(format!("smart-album-row-{}", original.id)));
    focus_app(vcx, &view);

    let before = dialogs();
    palette(vcx, "export smart album rules");
    assert_eq!(dialogs(), before + 1, "the palette asks where to save");

    // A folder that is not there: the write fails and says so.
    let missing = dir.join("no_such_folder").join("faves.json");
    view.update(vcx, |app, cx| {
        app.export_smart_album_rules_to(original.id, missing.clone(), cx)
    });
    vcx.run_until_parked();
    view.update(vcx, |app, cx| {
        assert!(
            app.error_message
                .as_deref()
                .is_some_and(|m| m.starts_with("Could not export smart album rules")),
            "a failed write is in the banner: {:?}",
            app.error_message
        );
        app.dismiss_error(cx);
    });

    let file = dir.join("faves.json");
    view.update(vcx, |app, cx| {
        app.export_smart_album_rules_to(original.id, file.clone(), cx)
    });
    vcx.run_until_parked();
    assert!(file.exists(), "the rules file is written");
    view.update(vcx, |app, cx| {
        assert!(app.error_message.is_none(), "{:?}", app.error_message);
        app.import_smart_album_rules_from(file.clone(), cx)
    });
    vcx.run_until_parked();

    let albums = engine.list_smart_albums();
    let imported = albums
        .iter()
        .find(|a| a.id != original.id)
        .expect("the import saved a smart album");
    assert_eq!(imported.title, "Faves");
    assert_eq!(imported.match_mode, MatchMode::Any);
    assert_eq!(imported.rules, original.rules, "the rules survive the file");
    view.update(vcx, |app, _| {
        assert_eq!(app.selection, AlbumSelection::User(imported.id));
        assert!(
            app.user_smart_albums.iter().any(|a| a.id == imported.id),
            "the sidebar lists it"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn file_menu_imports_smart_album_rules() {
    let (dir, engine) = seed_library("smart_import_menu", 1);
    let mut cx = crate::tests::TestApp::single();
    let (_view, vcx) = app_with_library(&mut cx, engine);
    let before = dialogs();
    vcx.dispatch_action(crate::actions::ImportSmartAlbumRules);
    assert_eq!(
        dialogs(),
        before + 1,
        "File > Import Smart Album Rules… asks for a file"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
