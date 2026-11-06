//! Upgrade path: a catalog written by an early build must open, migrate and
//! accept every everyday write through the real UI. Fresh-catalog tests
//! cannot catch schema drift ("table smart_albums has no column named
//! rules_json" only happened on upgraded catalogs).

use lumina_core::{EngineEvent, LuminaEngine};

use crate::app::command_palette::CommandId;
use crate::tests::ui_automation::{app_with_library, keystrokes};
use crate::views::sidebar::AlbumSelection;

const LEGACY_SCHEMA: &str = include_str!("../../tests/fixtures/legacy_catalog_v7.sql");

/// Two photos and a catalog in the legacy layout holding one saved album.
fn legacy_library() -> (std::path::PathBuf, std::sync::Arc<LuminaEngine>) {
    let dir = std::env::temp_dir().join(format!(
        "lumina_legacy_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    for name in ["photo_00.jpg", "photo_01.jpg"] {
        std::fs::write(dir.join(name), b"test-jpeg-content").unwrap();
    }
    let db = dir.join("catalog.db");
    {
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch(LEGACY_SCHEMA)
            .expect("legacy schema fixture must load");
        conn.execute(
            "INSERT INTO smart_albums (title, match_mode, rule_definition_json, created_at_epoch_ms)
             VALUES ('Old favourites', 'all',
                     '[{\"field\":\"is_favorite\",\"operator\":\"equals\",\"value\":true}]', 1)",
            [],
        )
        .unwrap();
    }
    let (engine, rx) =
        LuminaEngine::open_with_paths(dir.clone(), db, Some(dir.join("cache"))).unwrap();
    while let Ok(event) = rx.recv_timeout(std::time::Duration::from_secs(10)) {
        if let EngineEvent::ScanComplete { .. } = event {
            break;
        }
    }
    (dir, engine)
}

#[core::prelude::v1::test]
fn upgraded_legacy_catalog_accepts_every_everyday_write() {
    let (dir, engine) = legacy_library();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let no_error = |app: &crate::LuminaApp, step: &str| {
        assert_eq!(
            app.error_message, None,
            "{step} must work on an upgraded catalog"
        );
    };

    // Saved albums from the old layout are still listed and open.
    view.update(vcx, |app, _| {
        no_error(app, "opening");
        assert_eq!(app.user_smart_albums.len(), 1, "the old album survives");
        assert_eq!(app.user_smart_albums[0].title, "Old favourites");
        assert_eq!(app.get_active_count(), 2, "scanning wrote media rows");
    });

    // Favourite, rate and tag the first photo with the keyboard paths.
    keystrokes(vcx, "home f");
    view.update(vcx, |app, cx| {
        no_error(app, "favouriting");
        app.execute_command(CommandId::Rate(4), cx);
        no_error(app, "rating");
        // The docked inspector draws the tag field.
        app.inspector_open = true;
        app.start_tag_input(cx);
    });
    keystrokes(vcx, "s u n enter");
    let first = view.update(vcx, |app, _| {
        no_error(app, "tagging");
        app.selected_item.clone().expect("a photo is selected")
    });
    let stored = engine.get_item_by_id(first.id).expect("item exists");
    assert!(stored.is_favorite, "favourite reached the catalog");
    assert_eq!(engine.get_ratings_map().get(&first.id), Some(&4));
    assert_eq!(engine.get_tags_for_item(first.id), ["sun"]);

    // New and edited smart albums save (the reported failure).
    keystrokes(vcx, "alt-secondary-n secondary-enter");
    view.update(vcx, |app, cx| {
        no_error(app, "creating a smart album");
        assert_eq!(app.user_smart_albums.len(), 2);
        let old = app.user_smart_albums[0].clone();
        app.open_album_builder_for_edit(old, cx);
    });
    keystrokes(vcx, "space shift-x secondary-enter");
    view.update(vcx, |app, _| no_error(app, "editing a smart album"));
    let titles: Vec<String> = engine
        .list_smart_albums()
        .into_iter()
        .map(|a| a.title)
        .collect();
    assert!(
        titles.contains(&"Old favourites X".to_string()),
        "{titles:?}"
    );

    // Trash.
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::AllLibrary, cx)
    });
    keystrokes(vcx, "home backspace enter");
    view.update(vcx, |app, _| {
        no_error(app, "moving to Trash");
        assert_eq!(app.get_active_count(), 1);
    });
    assert!(engine.get_item_by_id(first.id).unwrap().is_trashed);

    let _ = std::fs::remove_dir_all(&dir);
}
