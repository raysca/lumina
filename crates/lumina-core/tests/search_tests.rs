//! Search integration tests: full-text and structured search, saved
//! searches, smart albums and their query plans.

mod common;

use common::*;
use lumina_core::{
    Catalog, DeletionTarget, Flag, MatchMode, MediaId, MediaItem, MediaType, RuleField,
    RuleOperator, RuleValue, SmartAlbumRule, SortDirection, SortField, SortSpec, StructuredQuery,
    SystemAlbum, SCHEMA_VERSION,
};
use std::time::Instant;

#[test]
fn test_fts5_search() {
    let catalog = Catalog::open_in_memory().expect("open catalog");

    let item1 = sample_media_item(
        "/photos/vacation/sunset_kyoto.jpg",
        MediaType::Image,
        1000,
        Some("Kyoto"),
        Some("A7IV"),
    );
    let item2 = sample_media_item(
        "/photos/paris/eiffel_tower.jpg",
        MediaType::Image,
        2000,
        Some("Paris"),
        Some("EOS R5"),
    );

    catalog.insert_or_update_batch(&[item1, item2]).unwrap();

    let matches_kyoto = catalog
        .search_structured(&StructuredQuery::parse("Kyoto"), 10, 0)
        .unwrap();
    assert_eq!(matches_kyoto.len(), 1);
    assert_eq!(matches_kyoto[0].city.as_deref(), Some("Kyoto"));

    let matches_paris = catalog
        .search_structured(&StructuredQuery::parse("Paris"), 10, 0)
        .unwrap();
    assert_eq!(matches_paris.len(), 1);
    assert_eq!(matches_paris[0].city.as_deref(), Some("Paris"));

    let matches_camera = catalog
        .search_structured(&StructuredQuery::parse("A7IV"), 10, 0)
        .unwrap();
    assert_eq!(matches_camera.len(), 1);
    assert_eq!(matches_camera[0].model.as_deref(), Some("A7IV"));
}

#[test]
fn test_fts5_caption_searchable() {
    let catalog = Catalog::open_in_memory().expect("open catalog");

    let item = sample_media_item(
        "/photos/kyoto/sunset.jpg",
        MediaType::Image,
        1000,
        Some("Kyoto"),
        Some("A7IV"),
    );
    let inserted = catalog
        .insert_or_update_batch(std::slice::from_ref(&item))
        .unwrap();
    let id = inserted[0].id;

    catalog
        .set_caption(id, "A lone elephant drinking at the river")
        .unwrap();

    let matches = catalog
        .search_structured(&StructuredQuery::parse("elephant"), 10, 0)
        .unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].id, id);

    // Metadata columns remain searchable alongside the new caption column.
    let by_city = catalog
        .search_structured(&StructuredQuery::parse("Kyoto"), 10, 0)
        .unwrap();
    assert_eq!(by_city.len(), 1);
    assert_eq!(by_city[0].id, id);

    // Re-scanning the same file fires the UPDATE trigger, which must keep the
    // caption terms in the contentless index.
    let mut rescanned = item;
    rescanned.mtime_epoch_ms = 2000;
    catalog.insert_or_update_batch(&[rescanned]).unwrap();
    assert_eq!(
        catalog
            .search_structured(&StructuredQuery::parse("elephant"), 10, 0)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn test_fts5_caption_edit_reindexes() {
    let catalog = Catalog::open_in_memory().expect("open catalog");

    let item = sample_media_item(
        "/photos/caption/edit.jpg",
        MediaType::Image,
        1000,
        None,
        None,
    );
    let inserted = catalog.insert_or_update_batch(&[item]).unwrap();
    let id = inserted[0].id;

    catalog
        .set_caption(id, "sleepy walrus on the dock")
        .unwrap();
    assert_eq!(
        catalog
            .search_structured(&StructuredQuery::parse("walrus"), 10, 0)
            .unwrap()
            .len(),
        1
    );

    catalog.set_caption(id, "crimson giraffe at dawn").unwrap();
    assert!(catalog
        .search_structured(&StructuredQuery::parse("walrus"), 10, 0)
        .unwrap()
        .is_empty());
    let matches = catalog
        .search_structured(&StructuredQuery::parse("giraffe"), 10, 0)
        .unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].id, id);
}

#[test]
fn test_fts5_caption_clear_unindexes() {
    let catalog = Catalog::open_in_memory().expect("open catalog");

    let item = sample_media_item(
        "/photos/caption/clear.jpg",
        MediaType::Image,
        1000,
        None,
        None,
    );
    let inserted = catalog.insert_or_update_batch(&[item]).unwrap();
    let id = inserted[0].id;

    catalog.set_caption(id, "hidden badger").unwrap();
    assert_eq!(
        catalog
            .search_structured(&StructuredQuery::parse("badger"), 10, 0)
            .unwrap()
            .len(),
        1
    );

    catalog.set_caption(id, "   ").unwrap();
    assert!(catalog
        .search_structured(&StructuredQuery::parse("badger"), 10, 0)
        .unwrap()
        .is_empty());
    assert!(!catalog.get_captions_map().unwrap().contains_key(&id));

    // Clearing an already-empty caption stays a no-op.
    catalog.set_caption(id, "").unwrap();
    assert!(!catalog.get_captions_map().unwrap().contains_key(&id));
}

#[test]
fn test_fts5_caption_removed_when_media_deleted() {
    let tmp_dir = unique_temp_dir("caption_delete");
    let db_path = tmp_dir.join("catalog.db");
    let catalog = Catalog::open(&db_path).expect("open catalog");

    let keep = sample_media_item(
        "/photos/deletion/keep.jpg",
        MediaType::Image,
        1000,
        Some("Kyoto"),
        Some("A7IV"),
    );
    let doomed = sample_media_item(
        "/photos/deletion/doomed.jpg",
        MediaType::Image,
        2000,
        Some("Paris"),
        Some("EOS R5"),
    );
    let inserted = catalog.insert_or_update_batch(&[keep, doomed]).unwrap();
    let keep_id = inserted[0].id;
    let doomed_id = inserted[1].id;

    catalog
        .set_caption(doomed_id, "unique narwhal sighting")
        .unwrap();
    catalog.set_caption(keep_id, "landscape panorama").unwrap();
    assert_eq!(
        catalog
            .search_structured(&StructuredQuery::parse("narwhal"), 10, 0)
            .unwrap()
            .len(),
        1
    );

    let removed = catalog
        .permanently_delete_items(&[doomed_id], DeletionTarget::CatalogOnly)
        .unwrap();
    assert_eq!(removed, 1);

    // The deleted media row no longer surfaces through the FTS join.
    assert!(catalog
        .search_structured(&StructuredQuery::parse("narwhal"), 10, 0)
        .unwrap()
        .is_empty());
    // The FTS index remains usable for the surviving rows.
    assert_eq!(
        catalog
            .search_structured(&StructuredQuery::parse("panorama"), 10, 0)
            .unwrap()
            .len(),
        1
    );
    // The caption row itself was removed by the foreign-key cascade.
    assert!(!catalog.get_captions_map().unwrap().contains_key(&doomed_id));

    // The contentless FTS row must no longer contain the caption tokens, not just
    // be filtered out by the media_items join. This holds because the delete
    // trigger now runs BEFORE the FK cascade removes media_captions.
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let stale: i64 = conn
        .query_row(
            "SELECT count(*) FROM media_search_fts WHERE media_search_fts MATCH 'narwhal'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        stale, 0,
        "caption tokens must be removed from the FTS index"
    );

    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_fts5_caption_schema_migration_backfills() {
    let tmp_dir = unique_temp_dir("caption_migration");
    let db_path = tmp_dir.join("catalog.db");

    // Seed a catalog, then downgrade its FTS schema to the pre-caption layout.
    {
        let catalog = Catalog::open(&db_path).expect("open catalog");
        let item = sample_media_item(
            "/photos/migration/elephant.jpg",
            MediaType::Image,
            1000,
            Some("Kyoto"),
            Some("A7IV"),
        );
        let inserted = catalog.insert_or_update_batch(&[item]).unwrap();
        catalog
            .set_caption(inserted[0].id, "elephant crossing the river")
            .unwrap();
    }
    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            r#"
            DROP TRIGGER IF EXISTS media_fts_ai;
            DROP TRIGGER IF EXISTS media_fts_ad;
            DROP TRIGGER IF EXISTS media_fts_au;
            DROP TABLE IF EXISTS media_search_fts;

            CREATE VIRTUAL TABLE media_search_fts USING fts5(
                file_name,
                parent_dir,
                city,
                admin1_region,
                country_code,
                make,
                model,
                lens,
                year,
                tags,
                content='',
                tokenize='unicode61 remove_diacritics 2'
            );

            INSERT INTO media_search_fts(
                rowid, file_name, parent_dir, city, admin1_region, country_code, make, model, lens, year, tags
            )
            SELECT
                m.id, m.file_name, m.parent_dir, m.city, m.admin1_region, m.country_code, m.make, m.model, m.lens, CAST(m.year AS TEXT),
                COALESCE((SELECT GROUP_CONCAT(tag, ' ') FROM media_tags WHERE media_id = m.id), '')
            FROM media_items m;
            "#,
        )
        .unwrap();
    }

    // Reopening migrates the FTS table and backfills the stored caption.
    let catalog = Catalog::open(&db_path).expect("reopen catalog");
    let matches = catalog
        .search_structured(&StructuredQuery::parse("elephant"), 10, 0)
        .unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(
        catalog
            .get_captions_map()
            .unwrap()
            .get(&matches[0].id)
            .map(String::as_str),
        Some("elephant crossing the river")
    );

    // Reopening again is idempotent: the caption column is detected and kept.
    drop(catalog);
    let catalog = Catalog::open(&db_path).expect("reopen catalog again");
    assert_eq!(
        catalog
            .search_structured(&StructuredQuery::parse("elephant"), 10, 0)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(catalog.total_count().unwrap(), 1);
    drop(catalog);

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let has_caption_col: bool = conn
        .prepare("SELECT 1 FROM pragma_table_info('media_search_fts') WHERE name = 'caption'")
        .unwrap()
        .exists([])
        .unwrap();
    assert!(has_caption_col);
}

#[test]
fn test_structured_search_engine() {
    let catalog = Catalog::open_in_memory().expect("open catalog");

    let mut item1 = sample_media_item(
        "/photos/sunset_kyoto.jpg",
        MediaType::Image,
        1789430400000,
        Some("Kyoto"),
        Some("ILCE-7M4"),
    );
    item1.make = Some("Sony".to_string());
    item1.model = Some("Sony A7IV".to_string());
    item1.lens = Some("FE 35mm F1.4 GM".to_string());
    item1.iso = Some(800);
    item1.f_number = Some(2.8);
    item1.is_favorite = true;
    item1.year = 2026;
    item1.month = 9;
    item1.day = 15;

    let mut item2 = sample_media_item(
        "/photos/shibuya_night.jpg",
        MediaType::Image,
        1786320000000,
        Some("Tokyo"),
        Some("EOS R5"),
    );
    item2.make = Some("Canon".to_string());
    item2.model = Some("Canon EOS R5".to_string());
    item2.lens = Some("RF 50mm F1.2L".to_string());
    item2.iso = Some(1600);
    item2.f_number = Some(1.2);
    item2.is_favorite = false;
    item2.year = 2026;
    item2.month = 8;
    item2.day = 10;

    let mut item3 = sample_media_item(
        "/photos/eiffel_tower.jpg",
        MediaType::Image,
        1746057600000,
        Some("Paris"),
        Some("Z8"),
    );
    item3.make = Some("Nikon".to_string());
    item3.model = Some("Nikon Z8".to_string());
    item3.iso = Some(200);
    item3.f_number = Some(1.8);
    item3.is_favorite = true;
    item3.year = 2025;
    item3.month = 5;
    item3.day = 1;

    let mut item4 = sample_media_item(
        "/videos/kyoto_temple.mp4",
        MediaType::Video,
        1789862400000,
        Some("Kyoto"),
        Some("FX3"),
    );
    item4.make = Some("Sony".to_string());
    item4.model = Some("Sony FX3".to_string());
    item4.iso = Some(3200);
    item4.is_favorite = false;
    item4.year = 2026;
    item4.month = 9;
    item4.day = 20;

    let mut item5 = sample_media_item(
        "/raw/arashiyama.arw",
        MediaType::Raw,
        1790035200000,
        Some("Kyoto"),
        Some("Sony A7IV"),
    );
    item5.make = Some("Sony".to_string());
    item5.model = Some("Sony A7IV".to_string());
    item5.iso = Some(100);
    item5.is_favorite = false;
    item5.year = 2026;
    item5.month = 9;
    item5.day = 22;

    catalog
        .insert_or_update_batch(&[item1, item2, item3, item4, item5])
        .expect("insert items");

    // 1. Search camera:Sony
    let q_sony = StructuredQuery::parse("camera:Sony");
    let sony_res = catalog
        .search_structured(&q_sony, 10, 0)
        .expect("search sony");
    assert_eq!(sony_res.len(), 3);

    // 2. Search camera:Sony is:favorite
    let q_sony_fav = StructuredQuery::parse("camera:Sony is:favorite");
    let sony_fav_res = catalog
        .search_structured(&q_sony_fav, 10, 0)
        .expect("search sony fav");
    assert_eq!(sony_fav_res.len(), 1);
    assert_eq!(sony_fav_res[0].file_name, "sunset_kyoto.jpg");

    // 3. Search city:Kyoto is:video
    let q_kyoto_vid = StructuredQuery::parse("city:Kyoto is:video");
    let kyoto_vid_res = catalog
        .search_structured(&q_kyoto_vid, 10, 0)
        .expect("search kyoto video");
    assert_eq!(kyoto_vid_res.len(), 1);
    assert_eq!(kyoto_vid_res[0].file_name, "kyoto_temple.mp4");

    // 4. Search iso:>1000
    let q_iso = StructuredQuery::parse("iso:>1000");
    let iso_res = catalog
        .search_structured(&q_iso, 10, 0)
        .expect("search iso");
    assert_eq!(iso_res.len(), 2);

    // 5. Search year:2026 month:9
    let q_sep = StructuredQuery::parse("year:2026 month:september");
    let sep_res = catalog
        .search_structured(&q_sep, 10, 0)
        .expect("search sep");
    assert_eq!(sep_res.len(), 3);

    // 6. Search is:raw
    let q_raw = StructuredQuery::parse("is:raw");
    let raw_res = catalog
        .search_structured(&q_raw, 10, 0)
        .expect("search raw");
    assert_eq!(raw_res.len(), 1);
    assert_eq!(raw_res[0].file_name, "arashiyama.arw");

    // 7. Search date range
    let q_date = StructuredQuery::parse("date:2026-09-01..2026-09-30");
    let date_res = catalog
        .search_structured(&q_date, 10, 0)
        .expect("search date range");
    assert_eq!(date_res.len(), 3);

    // 8. FTS Keyword + Structured
    let q_comb = StructuredQuery::parse("sunset camera:Sony");
    let comb_res = catalog
        .search_structured(&q_comb, 10, 0)
        .expect("search comb");
    assert_eq!(comb_res.len(), 1);
    assert_eq!(comb_res[0].file_name, "sunset_kyoto.jpg");

    // Benchmark search latency: must be < 5ms
    let start = Instant::now();
    for _ in 0..100 {
        let _ = catalog.search_structured(&q_comb, 10, 0);
    }
    let elapsed = start.elapsed();
    let per_query_ms = elapsed.as_secs_f64() * 1000.0 / 100.0;
    println!("Structured search latency: {:.3} ms/query", per_query_ms);
    // Debug builds under parallel test load are noisy; release keeps the 5ms gate.
    let budget_ms = if cfg!(debug_assertions) { 100.0 } else { 5.0 };
    assert!(
        per_query_ms < budget_ms,
        "Structured search must execute in < {budget_ms}ms (got {per_query_ms:.3}ms)"
    );
}

#[test]
fn test_system_smart_albums() {
    let catalog = Catalog::open_in_memory().expect("open catalog");

    let mut item_fav = sample_media_item(
        "/photos/fav.jpg",
        MediaType::Image,
        1000,
        None,
        Some("A7IV"),
    );
    item_fav.is_favorite = true;

    let item_vid = sample_media_item(
        "/photos/clip.mp4",
        MediaType::Video,
        2000,
        None,
        Some("FX3"),
    );

    let item_raw = sample_media_item("/photos/raw.cr3", MediaType::Raw, 3000, None, Some("R5"));

    let mut item_shot = sample_media_item(
        "/photos/Screenshot_2026.png",
        MediaType::Image,
        4000,
        None,
        None,
    );
    item_shot.width = 2560;
    item_shot.height = 1440;
    item_shot.lens = None;
    item_shot.focal_length = None;
    item_shot.iso = None;
    item_shot.make = None;

    let mut item_long = sample_media_item(
        "/photos/waterfall.jpg",
        MediaType::Image,
        5000,
        None,
        Some("A7IV"),
    );
    item_long.exposure_time = Some("2.5s".to_string());

    let mut item_pano = sample_media_item(
        "/photos/horizon.jpg",
        MediaType::Image,
        6000,
        None,
        Some("A7IV"),
    );
    item_pano.width = 3000;
    item_pano.height = 1000;

    // Bursts: 2 photos within 200ms from same camera model
    let item_burst1 = sample_media_item(
        "/photos/burst1.jpg",
        MediaType::Image,
        7000,
        None,
        Some("ILCE-1"),
    );
    let item_burst2 = sample_media_item(
        "/photos/burst2.jpg",
        MediaType::Image,
        7200,
        None,
        Some("ILCE-1"),
    );

    let mut item_trash =
        sample_media_item("/photos/deleted.jpg", MediaType::Image, 8000, None, None);
    item_trash.is_trashed = true;

    let mut item_places = sample_media_item("/photos/gps.jpg", MediaType::Image, 9000, None, None);
    item_places.latitude = Some(35.6895);
    item_places.longitude = Some(139.6917);

    catalog
        .insert_or_update_batch(&[
            item_fav,
            item_vid,
            item_raw,
            item_shot,
            item_long,
            item_pano,
            item_burst1,
            item_burst2,
            item_trash,
            item_places,
        ])
        .expect("insert batch");

    let expected = [
        (SystemAlbum::Favorites, 1),
        (SystemAlbum::Videos, 1),
        (SystemAlbum::Raw, 1),
        (SystemAlbum::Screenshots, 1),
        (SystemAlbum::LongExposures, 1),
        (SystemAlbum::Panoramas, 1),
        (SystemAlbum::Bursts, 2),
        (SystemAlbum::RecentlyDeleted, 1),
        (SystemAlbum::Places, 1),
    ];
    for (album, count) in expected {
        assert_eq!(
            catalog.query_system_album(album, 10, 0).unwrap().len(),
            count,
            "{album:?} must hold exactly its seeded items"
        );
    }

    // Verify count queries
    for album in SystemAlbum::all() {
        let count = catalog.query_system_album_count(*album).unwrap();
        let items = catalog.query_system_album(*album, 10, 0).unwrap();
        assert_eq!(count, items.len(), "Count mismatch for {:?}", album);
    }

    // Benchmark evaluation latency (< 8ms)
    let start = Instant::now();
    for album in SystemAlbum::all() {
        let _ = catalog.query_system_album(*album, 10, 0);
    }
    let elapsed = start.elapsed();
    let per_album_ms = elapsed.as_secs_f64() * 1000.0 / SystemAlbum::all().len() as f64;
    println!(
        "System smart album evaluation latency: {:.3} ms/album",
        per_album_ms
    );
    assert!(
        per_album_ms < 8.0,
        "System smart album evaluation must be < 8ms"
    );
}

#[test]
fn test_user_defined_smart_albums_json_engine() {
    let catalog = Catalog::open_in_memory().expect("open catalog");

    let mut item1 = sample_media_item(
        "/photos/sony_fav.jpg",
        MediaType::Image,
        1000,
        Some("Kyoto"),
        Some("ILCE-7M4"),
    );
    item1.make = Some("Sony".to_string());
    item1.is_favorite = true;
    item1.iso = Some(1600);

    let mut item2 = sample_media_item(
        "/photos/sony_regular.jpg",
        MediaType::Image,
        2000,
        Some("Tokyo"),
        Some("ILCE-7M4"),
    );
    item2.make = Some("Sony".to_string());
    item2.is_favorite = false;
    item2.iso = Some(400);

    let mut item3 = sample_media_item(
        "/photos/canon_fav.jpg",
        MediaType::Image,
        3000,
        Some("Paris"),
        Some("EOS R5"),
    );
    item3.make = Some("Canon".to_string());
    item3.is_favorite = true;
    item3.iso = Some(1600);

    // Matches make and favourite, so only the ISO rule keeps it out.
    let mut item4 = sample_media_item(
        "/photos/sony_fav_low_iso.jpg",
        MediaType::Image,
        4000,
        Some("Osaka"),
        Some("ILCE-7M4"),
    );
    item4.make = Some("Sony".to_string());
    item4.is_favorite = true;
    item4.iso = Some(400);

    catalog
        .insert_or_update_batch(&[item1, item2, item3, item4])
        .unwrap();

    let rules = vec![
        SmartAlbumRule {
            field: RuleField::CameraMake,
            operator: RuleOperator::Contains,
            value: RuleValue::Text("Sony".to_string()),
        },
        SmartAlbumRule {
            field: RuleField::IsFavorite,
            operator: RuleOperator::Equals,
            value: RuleValue::Bool(true),
        },
        SmartAlbumRule {
            field: RuleField::Iso,
            operator: RuleOperator::GreaterThan,
            value: RuleValue::Integer(800),
        },
    ];

    let created = catalog
        .create_smart_album("High ISO Sony Favorites", "heart", MatchMode::All, &rules)
        .expect("create smart album");

    assert!(created.id > 0);
    assert_eq!(created.title, "High ISO Sony Favorites");
    assert_eq!(created.rules.len(), 3);

    // List smart albums
    let albums = catalog.list_smart_albums().expect("list smart albums");
    assert_eq!(albums.len(), 1);
    assert_eq!(albums[0].title, "High ISO Sony Favorites");

    // Execute smart album query
    let results = catalog
        .query_smart_album(&albums[0], 10, 0)
        .expect("query smart album");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].file_name, "sony_fav.jpg");

    let count = catalog
        .query_smart_album_count(&albums[0])
        .expect("count smart album");
    assert_eq!(count, 1);

    // Test MatchMode::Any
    let any_rules = vec![
        SmartAlbumRule {
            field: RuleField::CameraMake,
            operator: RuleOperator::Contains,
            value: RuleValue::Text("Canon".to_string()),
        },
        SmartAlbumRule {
            field: RuleField::City,
            operator: RuleOperator::Equals,
            value: RuleValue::Text("Kyoto".to_string()),
        },
    ];

    let any_album = catalog
        .create_smart_album("Canon or Kyoto", "folder", MatchMode::Any, &any_rules)
        .unwrap();

    let any_results = catalog.query_smart_album(&any_album, 10, 0).unwrap();
    assert_eq!(any_results.len(), 2);

    // Benchmark multi-condition rule evaluation latency (< 8ms)
    let start = Instant::now();
    for _ in 0..100 {
        let _ = catalog.query_smart_album(&albums[0], 10, 0);
    }
    let elapsed = start.elapsed();
    let per_eval_ms = elapsed.as_secs_f64() * 1000.0 / 100.0;
    println!(
        "Multi-condition smart album evaluation latency: {:.3} ms/eval",
        per_eval_ms
    );
    assert!(per_eval_ms < 8.0, "Rule evaluation must execute in < 8ms");

    // Delete smart album
    let deleted = catalog.delete_smart_album(created.id).unwrap();
    assert!(deleted);
    let remaining = catalog.list_smart_albums().unwrap();
    assert_eq!(remaining.len(), 1);
}

#[test]
fn test_ratings_and_color_labels_persistence_and_search() {
    let catalog = Catalog::open_in_memory().expect("open catalog");

    let item1 = sample_media_item(
        "/photos/rated_five.jpg",
        MediaType::Image,
        1789430400000,
        Some("Tokyo"),
        Some("Sony A7IV"),
    );
    let item2 = sample_media_item(
        "/photos/rated_three.jpg",
        MediaType::Image,
        1789430500000,
        Some("Kyoto"),
        Some("Sony A7IV"),
    );

    let inserted = catalog
        .insert_or_update_batch(&[item1, item2])
        .expect("insert items");
    let id1 = inserted[0].id;
    let id2 = inserted[1].id;

    catalog.set_rating(id1, 5).expect("set rating 5");
    catalog.set_rating(id2, 3).expect("set rating 3");
    catalog.set_color_label(id1, 1).expect("set red label");

    let ratings = catalog.get_ratings_map().expect("get ratings map");
    assert_eq!(ratings.get(&id1), Some(&5));
    assert_eq!(ratings.get(&id2), Some(&3));

    let colors = catalog
        .get_color_labels_map()
        .expect("get color labels map");
    assert_eq!(colors.get(&id1), Some(&1));
    assert_eq!(colors.get(&id2), None);

    let q_high = StructuredQuery::parse("rating:>=4");
    let res_high = catalog.search_structured(&q_high, 10, 0).unwrap();
    assert_eq!(res_high.len(), 1);
    assert_eq!(res_high[0].id, id1);

    let q_red = StructuredQuery::parse("color:red");
    let res_red = catalog.search_structured(&q_red, 10, 0).unwrap();
    assert_eq!(res_red.len(), 1);
    assert_eq!(res_red[0].id, id1);

    let q_has_color = StructuredQuery::parse("has:color");
    let res_has_color = catalog.search_structured(&q_has_color, 10, 0).unwrap();
    assert_eq!(res_has_color.len(), 1);
    assert_eq!(res_has_color[0].id, id1);

    // Smart Album rules using Rating and ColorLabel
    let smart_rules = vec![
        SmartAlbumRule {
            field: RuleField::Rating,
            operator: RuleOperator::GreaterThan,
            value: RuleValue::Integer(4),
        },
        SmartAlbumRule {
            field: RuleField::ColorLabel,
            operator: RuleOperator::Equals,
            value: RuleValue::Integer(1),
        },
    ];
    let smart_album = catalog
        .create_smart_album("5-Star Red Selects", "star", MatchMode::All, &smart_rules)
        .expect("create rating/color smart album");
    let album_matches = catalog.query_smart_album(&smart_album, 10, 0).unwrap();
    assert_eq!(album_matches.len(), 1);
    assert_eq!(album_matches[0].id, id1);
}

#[test]
fn test_sort_orders_library_search_and_albums() {
    let catalog = Catalog::open_in_memory().unwrap();
    let specs = [
        ("/photos/sort/c.jpg", 1_600_000_000_300, 300),
        ("/photos/sort/a.jpg", 1_600_000_000_100, 100),
        ("/photos/sort/b.jpg", 1_600_000_000_200, 200),
    ];
    let items: Vec<MediaItem> = specs
        .iter()
        .map(|(path, timestamp, size)| {
            let mut item = sample_media_item(path, MediaType::Image, *timestamp, None, None);
            item.file_size = *size;
            item
        })
        .collect();
    let inserted = catalog.insert_or_update_batch(&items).unwrap();
    let ids: Vec<MediaId> = inserted.iter().map(|item| item.id).collect();
    catalog.batch_set_favorite(&ids, true).unwrap();
    catalog.set_rating(ids[0], 1).unwrap();
    catalog.set_rating(ids[1], 3).unwrap();
    catalog.set_rating(ids[2], 5).unwrap();

    let names = |items: Vec<MediaItem>| -> Vec<String> {
        items.into_iter().map(|item| item.file_name).collect()
    };

    let by_name_asc = SortSpec {
        field: SortField::FileName,
        direction: SortDirection::Ascending,
    };
    assert_eq!(
        names(catalog.get_item_slice_sorted(0, 10, by_name_asc).unwrap()),
        vec!["a.jpg", "b.jpg", "c.jpg"]
    );
    let by_size_asc = SortSpec {
        field: SortField::FileSize,
        direction: SortDirection::Ascending,
    };
    assert_eq!(
        names(catalog.get_item_slice_sorted(0, 10, by_size_asc).unwrap()),
        vec!["a.jpg", "b.jpg", "c.jpg"]
    );
    let by_rating_desc = SortSpec {
        field: SortField::Rating,
        direction: SortDirection::Descending,
    };
    assert_eq!(
        names(
            catalog
                .get_item_slice_sorted(0, 10, by_rating_desc)
                .unwrap()
        ),
        vec!["b.jpg", "a.jpg", "c.jpg"]
    );

    // Verify composite index usage in EXPLAIN QUERY PLAN per AGENTS.md §5.3
    for sort in [by_name_asc, by_size_asc, by_rating_desc] {
        let sql = format!(
            "SELECT id FROM media_items WHERE is_trashed = 0 AND is_hidden = 0 ORDER BY {} LIMIT 10 OFFSET 0",
            sort.order_by("")
        );
        let plan = catalog.explain_query_plan(&sql).unwrap();
        let plan_str = plan.join(" ");
        assert!(
            plan_str.contains("INDEX idx_media_"),
            "sort {:?} query plan must use an index, got: {}",
            sort,
            plan_str
        );
    }

    let favorites = StructuredQuery::parse("is:favorite");
    assert_eq!(
        names(
            catalog
                .search_structured_sorted(&favorites, 10, 0, by_name_asc)
                .unwrap()
        ),
        vec!["a.jpg", "b.jpg", "c.jpg"]
    );
    assert_eq!(
        names(
            catalog
                .query_system_album_sorted(SystemAlbum::Favorites, 10, 0, by_name_asc)
                .unwrap()
        ),
        vec!["a.jpg", "b.jpg", "c.jpg"]
    );

    let album = catalog
        .create_smart_album(
            "Rated",
            "folder",
            MatchMode::All,
            &[SmartAlbumRule {
                field: RuleField::Rating,
                operator: RuleOperator::GreaterThan,
                value: RuleValue::Integer(0),
            }],
        )
        .unwrap();
    assert_eq!(
        names(
            catalog
                .query_smart_album_sorted(&album, 10, 0, by_name_asc)
                .unwrap()
        ),
        vec!["a.jpg", "b.jpg", "c.jpg"]
    );
}

#[test]
fn test_update_smart_album_title_match_mode_and_rules() {
    let catalog = Catalog::open_in_memory().unwrap();
    let album = catalog
        .create_smart_album(
            "Old title",
            "folder",
            MatchMode::All,
            &[SmartAlbumRule {
                field: RuleField::IsFavorite,
                operator: RuleOperator::Equals,
                value: RuleValue::Bool(true),
            }],
        )
        .unwrap();

    let updated = catalog
        .update_smart_album(
            album.id,
            "New title",
            "star",
            MatchMode::Any,
            &[
                SmartAlbumRule {
                    field: RuleField::Rating,
                    operator: RuleOperator::GreaterThan,
                    value: RuleValue::Integer(3),
                },
                SmartAlbumRule {
                    field: RuleField::Tag,
                    operator: RuleOperator::Equals,
                    value: RuleValue::Text("travel".to_string()),
                },
            ],
        )
        .unwrap();
    assert!(updated);

    let albums = catalog.list_smart_albums().unwrap();
    let found = albums.iter().find(|a| a.id == album.id).unwrap();
    assert_eq!(found.title, "New title");
    assert_eq!(found.icon_name, "star");
    assert_eq!(found.match_mode, MatchMode::Any);
    assert_eq!(found.rules.len(), 2);

    assert!(catalog.delete_smart_album(album.id).unwrap());
    assert!(catalog.list_smart_albums().unwrap().is_empty());
}

#[test]
fn smart_album_builder_rules_match_expected_rows() {
    let catalog = Catalog::open_in_memory().expect("open catalog");
    let day = |s: &str| lumina_core::parse_rule_date(s).expect("valid date");

    let mut kyoto = sample_media_item(
        "/p/kyoto.jpg",
        MediaType::Image,
        day("2024-05-01") + 3_600_000,
        Some("Kyoto"),
        Some("ILCE-7M4"),
    );
    kyoto.is_favorite = true;
    kyoto.make = Some("Sony".to_string());
    let paris = sample_media_item(
        "/p/paris.dng",
        MediaType::Raw,
        day("2024-05-02") + 3_600_000,
        Some("Paris"),
        None,
    );
    let clip = sample_media_item(
        "/p/clip.mp4",
        MediaType::Video,
        day("2024-06-15"),
        None,
        None,
    );
    // The helper makes everything a Sony; only Kyoto has a camera here.
    let (mut paris, mut clip) = (paris, clip);
    paris.make = None;
    clip.make = None;
    catalog
        .insert_or_update_batch(&[kyoto, paris, clip])
        .unwrap();
    let ids: Vec<_> = catalog
        .query_smart_album(
            &lumina_core::SmartAlbum::new("all".into(), "f".into(), MatchMode::All, vec![]),
            10,
            0,
        )
        .unwrap()
        .into_iter()
        .map(|item| (item.file_name.clone(), item.id))
        .collect();
    let id = |name: &str| ids.iter().find(|(n, _)| n == name).unwrap().1;
    catalog.set_rating(id("kyoto.jpg"), 4).unwrap();
    catalog.set_rating(id("paris.dng"), 2).unwrap();

    let names = |field: RuleField, operator: RuleOperator, value: RuleValue| {
        let album = lumina_core::SmartAlbum::new(
            "t".into(),
            "f".into(),
            MatchMode::All,
            vec![SmartAlbumRule {
                field,
                operator,
                value,
            }],
        );
        let mut names: Vec<String> = catalog
            .query_smart_album(&album, 10, 0)
            .expect("rule compiles and runs")
            .into_iter()
            .map(|item| item.file_name)
            .collect();
        names.sort();
        names
    };
    let text = |s: &str| RuleValue::Text(s.to_string());

    assert_eq!(
        names(RuleField::MediaType, RuleOperator::Equals, text("photo")),
        ["kyoto.jpg", "paris.dng"],
        "Kind is Photo must include RAW stills and exclude videos"
    );
    assert_eq!(
        names(RuleField::MediaType, RuleOperator::NotEquals, text("photo")),
        ["clip.mp4"]
    );
    assert_eq!(
        names(RuleField::Date, RuleOperator::Equals, text("2024-05-01")),
        ["kyoto.jpg"],
        "Date is a day must cover that whole day only"
    );
    assert_eq!(
        names(RuleField::Date, RuleOperator::LessThan, text("2024-05-02")),
        ["kyoto.jpg"],
        "before excludes the named day"
    );
    assert_eq!(
        names(
            RuleField::Date,
            RuleOperator::GreaterThan,
            text("2024-05-01")
        ),
        ["clip.mp4", "paris.dng"],
        "after excludes the named day"
    );
    assert_eq!(
        names(
            RuleField::Date,
            RuleOperator::Between,
            text("2024-05-02 to 2024-06-15")
        ),
        ["clip.mp4", "paris.dng"],
        "a range includes both end days"
    );
    assert_eq!(
        names(
            RuleField::Rating,
            RuleOperator::AtLeast,
            RuleValue::Integer(4)
        ),
        ["kyoto.jpg"]
    );
    assert_eq!(
        names(
            RuleField::Rating,
            RuleOperator::AtMost,
            RuleValue::Integer(2)
        ),
        ["clip.mp4", "paris.dng"],
        "unrated items have zero stars"
    );
    assert_eq!(
        names(RuleField::Place, RuleOperator::Contains, text("fr")),
        ["paris.dng"],
        "Place matches the country code"
    );
    assert_eq!(
        names(RuleField::Place, RuleOperator::Contains, text("kyo")),
        ["kyoto.jpg"]
    );
    assert_eq!(
        names(RuleField::Place, RuleOperator::NotContains, text("paris")),
        ["clip.mp4", "kyoto.jpg"],
        "does not contain keeps items with no place"
    );
    assert_eq!(
        names(RuleField::Camera, RuleOperator::Contains, text("sony")),
        ["kyoto.jpg"],
        "Camera matches the make even when the model doesn't name it"
    );
    assert_eq!(
        names(RuleField::Camera, RuleOperator::Contains, text("7m4")),
        ["kyoto.jpg"],
        "Camera matches the model"
    );
    assert_eq!(
        names(RuleField::Camera, RuleOperator::NotContains, text("sony")),
        ["clip.mp4", "paris.dng"],
        "does not contain keeps items with no camera"
    );
    let bad_date = lumina_core::SmartAlbum::new(
        "t".into(),
        "f".into(),
        MatchMode::All,
        vec![SmartAlbumRule {
            field: RuleField::Date,
            operator: RuleOperator::NotEquals,
            value: text("2024-05-01"),
        }],
    );
    assert!(
        bad_date.build_sql().is_err(),
        "date text with an unsupported operator must not compile to a match-all"
    );
    assert!(lumina_core::split_date_range("2024-01-01..nope").is_none());
}

fn names_for(catalog: &Catalog, query: &str) -> Vec<String> {
    let q = StructuredQuery::parse(query);
    let mut names: Vec<String> = catalog
        .search_structured(&q, 100, 0)
        .unwrap_or_else(|e| panic!("query {query:?} must not fail: {e:#}"))
        .into_iter()
        .map(|item| item.file_name)
        .collect();
    names.sort();
    names
}

#[test]
fn minus_term_excludes_fts_matches() {
    let catalog = Catalog::open_in_memory().expect("open catalog");
    catalog
        .insert_or_update_batch(&[
            sample_media_item("/p/beach.jpg", MediaType::Image, 1, None, None),
            sample_media_item("/p/beaches_sunset.jpg", MediaType::Image, 2, None, None),
            sample_media_item("/p/city.jpg", MediaType::Image, 3, None, None),
            sample_media_item("/p/city_sunset.jpg", MediaType::Image, 4, None, None),
        ])
        .expect("insert items");

    assert_eq!(
        names_for(&catalog, "-beach"),
        vec!["city.jpg", "city_sunset.jpg"],
        "-beach must hide every row matching beach, including the prefix match beaches"
    );
    assert_eq!(
        names_for(&catalog, "sunset -beach"),
        vec!["city_sunset.jpg"],
        "inclusion and exclusion combine"
    );
    assert_eq!(
        names_for(&catalog, "-beach -city"),
        Vec::<String>::new(),
        "several exclusions each hide their rows"
    );
    assert_eq!(
        catalog
            .search_structured_count(&StructuredQuery::parse("sunset -beach"))
            .expect("count"),
        1,
        "the count query applies the exclusion too"
    );
}

#[test]
fn minus_tag_excludes_tagged_rows() {
    let catalog = Catalog::open_in_memory().expect("open catalog");
    let inserted = catalog
        .insert_or_update_batch(&[
            sample_media_item("/p/a.jpg", MediaType::Image, 1, None, None),
            sample_media_item("/p/b.jpg", MediaType::Image, 2, None, None),
            sample_media_item("/p/c.jpg", MediaType::Image, 3, None, None),
        ])
        .expect("insert items");
    catalog
        .add_tags_batch(&[inserted[0].id], &["work".to_string()])
        .expect("tag a");
    catalog
        .add_tags_batch(&[inserted[1].id], &["family".to_string()])
        .expect("tag b");

    assert_eq!(
        names_for(&catalog, "-tag:work"),
        vec!["b.jpg", "c.jpg"],
        "-tag:work hides only rows tagged work, untagged rows stay"
    );
    assert_eq!(
        names_for(&catalog, "-tag:Work -tag:family"),
        vec!["c.jpg"],
        "tag exclusion is case-insensitive like tag:"
    );
    assert_eq!(
        names_for(&catalog, "has:tag -tag:work"),
        vec!["b.jpg"],
        "has:tag and -tag combine"
    );
}

#[test]
fn fts_syntax_in_query_text_is_literal() {
    let catalog = Catalog::open_in_memory().expect("open catalog");
    catalog
        .insert_or_update_batch(&[
            sample_media_item("/p/or.jpg", MediaType::Image, 1, None, None),
            sample_media_item("/p/beach.jpg", MediaType::Image, 2, None, None),
            sample_media_item("/p/city.jpg", MediaType::Image, 3, None, None),
        ])
        .expect("insert items");

    for text in [
        "OR",
        "NOT x",
        "a NEAR b",
        "\"unclosed",
        "a*b",
        "^x",
        "col:x",
        "-",
        "-\"two words\"",
        "x OR -y",
        "-\"\"",
        "-*",
        "-OR",
        "-NEAR(",
    ] {
        let q = StructuredQuery::parse(text);
        catalog
            .search_structured(&q, 10, 0)
            .unwrap_or_else(|e| panic!("{text:?} must be literal text, got SQL error {e:#}"));
        catalog
            .search_structured_count(&q)
            .unwrap_or_else(|e| panic!("{text:?} count must not fail: {e:#}"));
    }
    assert_eq!(
        names_for(&catalog, "OR"),
        vec!["or.jpg"],
        "OR alone is a word, not an operator"
    );
    assert_eq!(
        names_for(&catalog, "-"),
        vec!["beach.jpg", "city.jpg", "or.jpg"],
        "a lone - is ignored"
    );
}

#[test]
fn exclusion_plan_uses_fts_index() {
    let catalog = Catalog::open_in_memory().expect("open catalog");
    catalog
        .insert_or_update_batch(&[sample_media_item(
            "/p/beach.jpg",
            MediaType::Image,
            1,
            None,
            None,
        )])
        .expect("insert items");
    for text in ["-beach", "-beach is:favorite", "sunset -beach"] {
        let (sql, params) =
            StructuredQuery::parse(text).synthesize_query_sorted(50, 0, SortSpec::default());
        let rows = catalog
            .explain_query_plan(&inline_sql_params(&sql, &params))
            .expect("explain");
        let plan = rows.join(" | ");
        // FTS5 reports a full scan as `VIRTUAL TABLE INDEX 0:` and a MATCH
        // lookup as `0:M...`; the row right under the subquery is its scan.
        let sub = rows
            .iter()
            .position(|r| r.contains("LIST SUBQUERY"))
            .unwrap_or_else(|| panic!("{text:?}: expected a NOT IN subquery, got: {plan}"));
        assert!(
            rows.get(sub + 1)
                .is_some_and(|r| r.contains("VIRTUAL TABLE INDEX 0:M")),
            "{text:?}: the NOT IN subquery must MATCH through the FTS index, got: {plan}"
        );
        if !text.starts_with("sunset") {
            assert!(
                !plan.contains("USE TEMP B-TREE FOR ORDER BY"),
                "{text:?}: the default sort must stay an index walk, got: {plan}"
            );
        }
    }
}

/// Rating, label and flag filters under the default date sort walk an
/// `idx_media_` index: `is:pick` / `is:reject` seek `idx_media_flag` in
/// timeline order instead of sorting every match in a temp B-tree.
#[test]
fn rating_color_and_flag_filter_plans_use_an_index() {
    let catalog = Catalog::open_in_memory().expect("open catalog");
    let items: Vec<_> = (0..20)
        .map(|i| sample_media_item(&format!("/p/{i}.jpg"), MediaType::Image, i, None, None))
        .collect();
    catalog
        .insert_or_update_batch(&items)
        .expect("insert items");
    let ids: Vec<_> = catalog
        .get_item_slice(0, 20)
        .expect("slice")
        .into_iter()
        .map(|m| m.id)
        .collect();
    catalog
        .set_ratings(&[(ids[0], 5), (ids[1], 4), (ids[2], 2)])
        .expect("seed ratings");
    catalog
        .set_color_labels(&[(ids[0], 1), (ids[3], 3)])
        .expect("seed labels");
    catalog
        .set_flags(&[(ids[4], Flag::Pick), (ids[5], Flag::Reject)])
        .expect("seed flags");

    // (query, rows it matches, index the plan must name, temp B-tree banned)
    let cases = [
        ("rating:>=4", 2, "idx_media_", false),
        ("color:red", 1, "idx_media_", false),
        ("is:unrated", 17, "idx_media_", false),
        ("is:unlabeled", 18, "idx_media_", false),
        ("is:pick", 1, "idx_media_flag", true),
        ("is:reject", 1, "idx_media_flag", true),
    ];
    for (text, matches, index, no_temp_sort) in cases {
        let q = StructuredQuery::parse(text);
        assert_eq!(
            catalog.search_structured(&q, 50, 0).expect("search").len(),
            matches,
            "{text:?} must match its seeded rows"
        );
        let (sql, params) = q.synthesize_query_sorted(50, 0, SortSpec::default());
        let plan = catalog
            .explain_query_plan(&inline_sql_params(&sql, &params))
            .expect("explain")
            .join(" | ");
        assert!(
            plan.contains(index),
            "{text:?}: the default-sort plan must use {index}, got: {plan}"
        );
        assert!(
            !plan
                .split(" | ")
                .any(|row| row.contains("SCAN m") && !row.contains("INDEX")),
            "{text:?}: media_items must not be scanned without an index, got: {plan}"
        );
        assert!(
            !no_temp_sort || !plan.contains("USE TEMP B-TREE"),
            "{text:?}: the date sort must stay an index walk, got: {plan}"
        );
    }
}

/// Seeds named geotagged rows for the `within:` tests and returns the
/// catalog. `None` coordinates mean no GPS; the rows named `trashed` and
/// `hidden` are trashed and hidden.
fn geo_catalog(rows: &[(&str, Option<(f64, f64)>)]) -> Catalog {
    let catalog = Catalog::open_in_memory().expect("open catalog");
    let items: Vec<_> = rows
        .iter()
        .enumerate()
        .map(|(i, &(name, coords))| {
            let mut item = sample_media_item(
                &format!("/geo/{name}.jpg"),
                MediaType::Image,
                i as i64,
                None,
                None,
            );
            item.latitude = coords.map(|c| c.0);
            item.longitude = coords.map(|c| c.1);
            item.is_trashed = name == "trashed";
            item.is_hidden = name == "hidden";
            item
        })
        .collect();
    catalog
        .insert_or_update_batch(&items)
        .expect("insert geo items");
    catalog
}

fn search_names(catalog: &Catalog, text: &str) -> Vec<String> {
    let mut names: Vec<String> = catalog
        .search_structured(&StructuredQuery::parse(text), 100, 0)
        .unwrap_or_else(|e| panic!("{text:?} must run: {e:#}"))
        .into_iter()
        .map(|m| m.file_name.trim_end_matches(".jpg").to_string())
        .collect();
    names.sort();
    names
}

#[test]
fn within_filters_bbox_and_wraps_antimeridian() {
    let catalog = geo_catalog(&[
        ("kyoto", Some((35.0116, 135.7681))),
        ("osaka", Some((34.69, 135.50))),
        ("corner", Some((35.5, 136.0))),
        ("fiji", Some((-17.7, 178.0))),
        ("samoa", Some((-13.8, -172.1))),
        ("nogps", None),
        ("trashed", Some((35.0, 135.7))),
        ("hidden", Some((35.0, 135.7))),
    ]);

    let kansai = ["corner", "kyoto", "osaka"];
    assert_eq!(
        search_names(&catalog, "within:34.5,135,35.5,136"),
        kansai,
        "the box is inclusive (corner sits on its edge) and skips trashed, hidden and GPS-less rows"
    );
    assert_eq!(
        search_names(&catalog, "within:35.5,135,34.5,136"),
        kansai,
        "latitude order does not matter"
    );
    assert_eq!(
        search_names(&catalog, "within:-20,170,-10,-170"),
        ["fiji", "samoa"],
        "west > east wraps the antimeridian"
    );
    assert!(
        search_names(&catalog, "within:-20,-170,-10,170").is_empty(),
        "west < east is the box between them, which holds neither island"
    );

    let kyoto_id = catalog
        .search_structured(&StructuredQuery::parse("kyoto"), 10, 0)
        .expect("find kyoto")[0]
        .id;
    catalog
        .toggle_favorite(kyoto_id, true)
        .expect("favorite kyoto");
    assert_eq!(
        search_names(&catalog, "within:34.5,135,35.5,136 is:favorite"),
        ["kyoto"],
        "within: ANDs with other filters"
    );
    assert_eq!(
        search_names(&catalog, "within:34.5,135,35.5,136 -kyoto"),
        ["corner", "osaka"],
        "within: ANDs with exclusions"
    );
    assert_eq!(
        catalog
            .search_structured_count(&StructuredQuery::parse("within:34.5,135,35.5,136"))
            .expect("count"),
        3,
        "the count query uses the same box"
    );

    let saved = lumina_core::SmartAlbum::new(
        "Kansai".to_string(),
        "folder".to_string(),
        MatchMode::All,
        vec![SmartAlbumRule {
            field: RuleField::Query,
            operator: RuleOperator::Equals,
            value: RuleValue::Text("within:34.5,135,35.5,136".to_string()),
        }],
    );
    let mut album_names: Vec<String> = catalog
        .query_smart_album(&saved, 50, 0)
        .expect("saved search runs")
        .into_iter()
        .map(|m| m.file_name.trim_end_matches(".jpg").to_string())
        .collect();
    album_names.sort();
    assert_eq!(album_names, kansai, "within: works in a saved-search album");
    assert_eq!(
        catalog
            .query_smart_album_count(&saved)
            .expect("album count"),
        3,
        "and in its count"
    );

    let points = catalog.geo_points().expect("geo points");
    assert_eq!(
        points.len(),
        5,
        "geo_points lists active geotagged rows only: {points:?}"
    );
    let fiji = catalog
        .search_structured(&StructuredQuery::parse("fiji"), 10, 0)
        .expect("find fiji")[0]
        .id;
    let p = points
        .iter()
        .find(|p| p.id == fiji)
        .expect("fiji must be a geo point");
    assert_eq!((p.lat, p.lon), (-17.7, 178.0));
    assert!(
        points.windows(2).all(|w| w[0].id.0 < w[1].id.0),
        "geo points come in id order"
    );
}

/// `within:` under the default date sort. A box with few matches for the
/// page it asks for seeks `idx_media_geo` and sorts them; a box dense
/// enough to fill page 0 quickly walks `idx_media_timestamp` and filters,
/// with no temp B-tree; a deep page of that box seeks the index again.
#[test]
fn within_query_plan() {
    let rows: Vec<(String, Option<(f64, f64)>)> = (0..1200)
        .map(|i| {
            let coords = match i {
                0..=19 => Some((-33.8688, 151.2093)),
                20..=1119 => Some((35.0 + (i % 50) as f64 * 0.001, 135.7)),
                _ => None,
            };
            (format!("p{i}"), coords)
        })
        .collect();
    let refs: Vec<_> = rows.iter().map(|(n, c)| (n.as_str(), *c)).collect();
    let catalog = geo_catalog(&refs);

    for (label, text, matches, index) in [
        (
            "rare",
            "within:-33.9,151.2,-33.8,151.3",
            20,
            "idx_media_geo",
        ),
        (
            "dense",
            "within:34.9,135.6,35.1,135.8",
            1100,
            "idx_media_timestamp",
        ),
    ] {
        let q = StructuredQuery::parse(text);
        assert_eq!(
            catalog.search_structured_count(&q).expect("count"),
            matches,
            "{label} box must match its seeded rows"
        );
        assert_eq!(
            catalog.search_structured(&q, 50, 0).expect("search").len(),
            matches.min(50),
            "{label} box page 0 must be full"
        );
        let (sql, params) = catalog
            .search_sql_sorted(&q, 50, 0, SortSpec::default())
            .expect("search sql");
        let plan = catalog
            .explain_query_plan(&inline_sql_params(&sql, &params))
            .expect("explain")
            .join(" | ");
        println!("within_query_plan {label}: {plan}");
        assert!(
            plan.contains(&format!("USING INDEX {index}")),
            "{label}: the date-sorted plan must use {index}, got: {plan}"
        );
        if label == "dense" {
            assert!(
                !plan.contains("USE TEMP B-TREE"),
                "{label}: a dense box on page 0 must walk the sort index, got: {plan}"
            );
        }
    }

    // A deep page of the dense box: walking the timeline to offset 1050 would
    // read nearly every row, so sorting the box's 1,100 matches wins.
    let q = StructuredQuery::parse("within:34.9,135.6,35.1,135.8");
    assert_eq!(
        catalog
            .search_structured(&q, 50, 1050)
            .expect("deep page")
            .len(),
        50
    );
    let (sql, params) = catalog
        .search_sql_sorted(&q, 50, 1050, SortSpec::default())
        .expect("deep page sql");
    let plan = catalog
        .explain_query_plan(&inline_sql_params(&sql, &params))
        .expect("explain deep page")
        .join(" | ");
    println!("within_query_plan deep: {plan}");
    assert!(
        plan.contains("USING INDEX idx_media_geo"),
        "a deep page of a dense box must seek idx_media_geo, got: {plan}"
    );

    // An old trip: a dense box whose rows are all older than the rest of the
    // library. Page 0 of the date sort would walk every newer row before the
    // first match, so it must seek the geo index.
    let rows: Vec<(String, Option<(f64, f64)>)> = (0..3000)
        .map(|i| {
            let coords = (i < 1100).then_some((35.0 + (i % 50) as f64 * 0.001, 135.7));
            (format!("t{i}"), coords)
        })
        .collect();
    let refs: Vec<_> = rows.iter().map(|(n, c)| (n.as_str(), *c)).collect();
    let trip = geo_catalog(&refs);
    let q = StructuredQuery::parse("within:34.9,135.6,35.1,135.8");
    assert_eq!(trip.search_structured(&q, 50, 0).expect("trip").len(), 50);
    let (sql, params) = trip
        .search_sql_sorted(&q, 50, 0, SortSpec::default())
        .expect("trip sql");
    let plan = trip
        .explain_query_plan(&inline_sql_params(&sql, &params))
        .expect("explain trip")
        .join(" | ");
    println!("within_query_plan trip: {plan}");
    assert!(
        plan.contains("USING INDEX idx_media_geo"),
        "page 0 of an old dense trip must seek idx_media_geo, got: {plan}"
    );

    // The count is index-only for any box size.
    let (sql, params) =
        StructuredQuery::parse("within:34.9,135.6,35.1,135.8").synthesize_count_query();
    let plan = catalog
        .explain_query_plan(&inline_sql_params(&sql, &params))
        .expect("explain count")
        .join(" | ");
    assert!(
        plan.contains("idx_media_geo"),
        "a within: count must use idx_media_geo, got: {plan}"
    );
}

/// v15 adds `idx_media_color` so a colour filter under the default date sort
/// seeks the label and walks it in order instead of filtering the whole
/// timeline. Covers a fresh catalog, a second open, and a v14 upgrade.
#[test]
fn migration_adds_color_index_idempotently() {
    let tmp_dir = unique_temp_dir("color_index_migration");
    // Removes the temp dir even when an assertion below panics.
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(tmp_dir.clone());
    let db_path = tmp_dir.join("catalog.db");

    let check = |catalog: &Catalog, stage: &str| {
        assert_eq!(
            catalog.schema_version().unwrap(),
            SCHEMA_VERSION,
            "{stage}: user_version must be stamped to the current schema"
        );
        let (sql, params) = StructuredQuery::parse("color:red").synthesize_query_sorted(
            50,
            450,
            SortSpec::default(),
        );
        let plan = catalog
            .explain_query_plan(&inline_sql_params(&sql, &params))
            .expect("explain")
            .join(" | ");
        assert!(
            plan.contains("SEARCH m USING INDEX idx_media_color"),
            "{stage}: color:red must seek idx_media_color, got: {plan}"
        );
        assert!(
            !plan.contains("USE TEMP B-TREE FOR ORDER BY"),
            "{stage}: the index must satisfy the default date sort, got: {plan}"
        );
    };

    {
        let catalog = Catalog::open(&db_path).expect("open fresh catalog");
        check(&catalog, "fresh");
    }
    {
        let catalog = Catalog::open(&db_path).expect("reopen catalog");
        check(&catalog, "reopen");
    }
    {
        // Downgrade to the v14 layout: no colour index.
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch("DROP INDEX idx_media_color; PRAGMA user_version = 14;")
            .expect("downgrade to v14");
    }
    {
        let catalog = Catalog::open(&db_path).expect("a v14 catalog must open and migrate");
        check(&catalog, "upgrade from v14");
    }
}

fn query_rule_album(text: &str) -> lumina_core::SmartAlbum {
    lumina_core::SmartAlbum::new(
        text.to_string(),
        "folder".to_string(),
        MatchMode::All,
        vec![SmartAlbumRule {
            field: RuleField::Query,
            operator: RuleOperator::Equals,
            value: RuleValue::Text(text.to_string()),
        }],
    )
}

#[test]
fn query_rule_album_matches_search_rows() {
    let catalog = Catalog::open_in_memory().expect("open catalog");
    let mut items = vec![
        sample_media_item(
            "/p/sunset_beach.jpg",
            MediaType::Image,
            10,
            Some("Kyoto"),
            None,
        ),
        sample_media_item("/p/sunset_city.jpg", MediaType::Image, 20, None, Some("A7")),
        sample_media_item("/p/sunset_tie_a.jpg", MediaType::Image, 30, None, None),
        sample_media_item("/p/sunset_tie_b.jpg", MediaType::Image, 30, None, None),
        sample_media_item("/p/beach.mov", MediaType::Video, 40, Some("Paris"), None),
        sample_media_item("/p/forest.jpg", MediaType::Image, 50, None, None),
        sample_media_item("/p/sunset_hidden.jpg", MediaType::Image, 60, None, None),
        sample_media_item("/p/sunset_trashed.jpg", MediaType::Image, 70, None, None),
    ];
    items[1].make = Some("Canon".to_string());
    items[0].is_favorite = true;
    items[4].is_favorite = true;
    items[5].year = 2024;
    items[6].is_hidden = true;
    items[7].is_trashed = true;
    catalog
        .insert_or_update_batch(&items)
        .expect("insert items");
    let by_name = |name: &str| {
        catalog
            .get_item_slice(0, 100)
            .expect("slice")
            .into_iter()
            .find(|m| m.file_name == name)
            .unwrap_or_else(|| panic!("{name} is seeded"))
            .id
    };
    let (beach, city, tie_a, forest) = (
        by_name("sunset_beach.jpg"),
        by_name("sunset_city.jpg"),
        by_name("sunset_tie_a.jpg"),
        by_name("forest.jpg"),
    );
    catalog
        .set_ratings(&[(beach, 4), (city, 2), (tie_a, 5)])
        .expect("seed ratings");
    catalog
        .set_color_labels(&[(beach, 1), (forest, 1)])
        .expect("seed labels");
    catalog
        .add_tags_batch(&[city], &["work".to_string()])
        .expect("seed tags");

    for text in [
        "sunset",
        "sunset -beach",
        "-tag:work -sunset",
        "rating:>=3 color:red",
        "sunset camera:Sony rating:>=3",
        "is:video beach",
        "color:red",
        "tag:work",
        "is:favorite",
        "-is:favorite",
        "has:location",
        "year:2024",
    ] {
        let search: Vec<MediaId> = catalog
            .search_structured(&StructuredQuery::parse(text), 100, 0)
            .unwrap_or_else(|e| panic!("search {text:?}: {e:#}"))
            .into_iter()
            .map(|m| m.id)
            .collect();
        let album = query_rule_album(text);
        let rows: Vec<MediaId> = catalog
            .query_smart_album(&album, 100, 0)
            .unwrap_or_else(|e| panic!("album {text:?}: {e:#}"))
            .into_iter()
            .map(|m| m.id)
            .collect();
        assert!(!search.is_empty(), "{text:?} must match seeded rows");
        assert_eq!(
            rows, search,
            "{text:?}: a saved search must give the search's rows in the same order"
        );
        assert_eq!(
            catalog.query_smart_album_count(&album).expect("count"),
            search.len(),
            "{text:?}: the sidebar count must equal the search count"
        );
    }

    // Any mode: a Query rule OR another rule shows rows matching either.
    let ids_of = |text: &str| -> Vec<MediaId> {
        catalog
            .search_structured(&StructuredQuery::parse(text), 100, 0)
            .expect("search")
            .into_iter()
            .map(|m| m.id)
            .collect()
    };
    let mut union = ids_of("tag:work");
    union.extend(ids_of("is:favorite"));
    union.sort_by_key(|id| id.0);
    union.dedup();
    let mut any = query_rule_album("tag:work");
    any.match_mode = MatchMode::Any;
    any.rules.push(SmartAlbumRule {
        field: RuleField::IsFavorite,
        operator: RuleOperator::Equals,
        value: RuleValue::Bool(true),
    });
    let mut rows: Vec<MediaId> = catalog
        .query_smart_album(&any, 100, 0)
        .expect("any album")
        .into_iter()
        .map(|m| m.id)
        .collect();
    rows.sort_by_key(|id| id.0);
    assert_eq!(union.len(), 3, "work-tagged city plus two favourites");
    assert_eq!(rows, union, "an Any album is the union of its rules");
    assert_eq!(
        catalog.query_smart_album_count(&any).expect("count"),
        union.len(),
        "the Any album's count is the union's size"
    );
}

#[test]
fn query_rule_count_plan_drives_from_fts() {
    let catalog = Catalog::open_in_memory().expect("open catalog");
    catalog
        .insert_or_update_batch(&[sample_media_item(
            "/p/sunset.jpg",
            MediaType::Image,
            1,
            None,
            None,
        )])
        .expect("insert items");
    for text in ["sunset", "sunset camera:Sony rating:>=3"] {
        let (sql, params) = query_rule_album(text)
            .synthesize_count_query()
            .expect("compiles");
        let rows = catalog
            .explain_query_plan(&inline_sql_params(&sql, &params))
            .expect("explain");
        let plan = rows.join(" | ");
        assert!(
            plan.contains("VIRTUAL TABLE INDEX 0:M"),
            "{text:?}: the count must MATCH through the FTS index, got: {plan}"
        );
        assert!(
            !rows.iter().any(|r| r == "SCAN m" || r.starts_with("SCAN m ")),
            "{text:?}: a saved-search count must drive from the FTS rowids, not scan media_items, got: {plan}"
        );
    }
}

#[test]
fn query_rule_plan_uses_fts_index() {
    let catalog = Catalog::open_in_memory().expect("open catalog");
    catalog
        .insert_or_update_batch(&[sample_media_item(
            "/p/sunset.jpg",
            MediaType::Image,
            1,
            None,
            None,
        )])
        .expect("insert items");
    for text in ["sunset", "sunset camera:Sony rating:>=3", "sunset -beach"] {
        let (sql, params) = query_rule_album(text)
            .synthesize_query_sorted(50, 0, SortSpec::default())
            .expect("compiles");
        let rows = catalog
            .explain_query_plan(&inline_sql_params(&sql, &params))
            .expect("explain");
        let plan = rows.join(" | ");
        let matched = rows.iter().enumerate().any(|(i, r)| {
            r.contains("LIST SUBQUERY")
                && rows
                    .get(i + 1)
                    .is_some_and(|next| next.contains("VIRTUAL TABLE INDEX 0:M"))
        });
        assert!(
            matched,
            "{text:?}: the IN subquery must MATCH through the FTS index, got: {plan}"
        );
        assert!(
            !plan.contains("USE TEMP B-TREE FOR ORDER BY"),
            "{text:?}: the default sort must stay an index walk, got: {plan}"
        );
    }
}

#[test]
fn is_edited_matches_items_with_edits_or_rotation() {
    let catalog = Catalog::open_in_memory().expect("open catalog");
    let ids: Vec<MediaId> = catalog
        .insert_or_update_batch(&[
            sample_media_item("/p/cropped.jpg", MediaType::Image, 3, None, None),
            sample_media_item("/p/rotated.jpg", MediaType::Image, 2, None, None),
            sample_media_item("/p/plain.jpg", MediaType::Image, 1, None, None),
        ])
        .expect("insert items")
        .iter()
        .map(|item| item.id)
        .collect();
    catalog
        .set_edits(&[(
            ids[0],
            lumina_core::Edits {
                exposure_ev: 0.5,
                ..Default::default()
            },
        )])
        .expect("store an edits row");
    catalog
        .set_rotations(&[(ids[1], 1)])
        .expect("rotate a quarter turn");

    assert_eq!(
        names_for(&catalog, "is:edited"),
        ["cropped.jpg", "rotated.jpg"],
        "is:edited matches an edits row or a rotation"
    );
    assert_eq!(
        names_for(&catalog, "-is:edited"),
        ["plain.jpg"],
        "-is:edited matches only items with neither edits nor rotation"
    );
    assert_eq!(StructuredQuery::parse("is:edited").explain(), ["Edited"]);
    assert_eq!(
        StructuredQuery::parse("-is:edited").explain(),
        ["Not edited"]
    );

    // A saved-search album compiles through the same parser.
    let mut album: Vec<String> = catalog
        .query_smart_album(&query_rule_album("is:edited"), 50, 0)
        .expect("a saved search accepts is:edited")
        .into_iter()
        .map(|m| m.file_name)
        .collect();
    album.sort();
    assert_eq!(album, ["cropped.jpg", "rotated.jpg"]);

    // The plan drives from the edited ids (rowid lookups into media_items),
    // never a walk of the whole timeline index or a scan of media_items.
    let (sql, params) =
        StructuredQuery::parse("is:edited").synthesize_query_sorted(50, 0, SortSpec::default());
    let rows = catalog
        .explain_query_plan(&inline_sql_params(&sql, &params))
        .expect("explain");
    let plan = rows.join(" | ");
    assert!(
        plan.contains("SEARCH m USING INTEGER PRIMARY KEY (rowid=?)"),
        "is:edited must look up media_items by rowid from the edited ids, got: {plan}"
    );
    assert!(
        plan.contains("idx_media_rotated"),
        "rotated ids must come from the partial rotation index, got: {plan}"
    );
    assert!(
        rows.iter()
            .filter(|row| row.starts_with("SCAN "))
            .all(|row| row == "SCAN media_edits" || row.contains("idx_media_rotated")),
        "is:edited may scan only media_edits and the rotation index, got: {plan}"
    );
}

#[test]
fn is_4k_matches_long_edge_3840() {
    let catalog = Catalog::open_in_memory().expect("open catalog");
    let sized = |path: &str, width: u32, height: u32| MediaItem {
        width,
        height,
        ..sample_media_item(path, MediaType::Video, 1, None, None)
    };
    catalog
        .insert_or_update_batch(&[
            sized("/v/landscape.mov", 3840, 2160),
            sized("/v/portrait.mov", 2160, 3840),
            sized("/v/hd.mov", 1920, 1080),
            sized("/v/almost.mov", 3839, 2160),
        ])
        .expect("insert items");

    assert_eq!(
        names_for(&catalog, "is:4k"),
        ["landscape.mov", "portrait.mov"],
        "is:4k matches a long edge of 3840 or more, either orientation"
    );
    assert_eq!(
        names_for(&catalog, "-is:4k"),
        ["almost.mov", "hd.mov"],
        "-is:4k matches only items whose long edge is under 3840"
    );
    assert_eq!(
        names_for(&catalog, "IS:4K is:video"),
        ["landscape.mov", "portrait.mov"],
        "the token is case-insensitive like the other is: values"
    );
    assert_eq!(StructuredQuery::parse("is:4k").explain(), ["4K"]);
    assert_eq!(StructuredQuery::parse("-is:4k").explain(), ["Not 4K"]);

    // A saved-search album compiles through the same parser.
    let mut album: Vec<String> = catalog
        .query_smart_album(&query_rule_album("is:4k"), 50, 0)
        .expect("a saved search accepts is:4k")
        .into_iter()
        .map(|m| m.file_name)
        .collect();
    album.sort();
    assert_eq!(album, ["landscape.mov", "portrait.mov"]);

    // An expression filter on the row, not a new access path: adding it
    // leaves the plan of the query without it unchanged, alone (the
    // timeline walk) and beside `is:video` (the Videos chip's query).
    let plan = |text: &str| {
        let (sql, params) = StructuredQuery::parse(text).synthesize_query_sorted(
            50,
            0,
            lumina_core::SortSpec::default(),
        );
        catalog
            .explain_query_plan(&inline_sql_params(&sql, &params))
            .expect("explain")
            .join(" | ")
    };
    for (with, without) in [("is:4k", ""), ("is:video is:4k", "is:video")] {
        let (with_plan, without_plan) = (plan(with), plan(without));
        println!("EXPLAIN `{with}`: {with_plan}");
        assert_eq!(
            with_plan, without_plan,
            "`{with}` must filter the rows of `{without:?}`'s plan, not change it"
        );
    }
}

#[test]
fn migration_adds_rotation_index_idempotently() {
    let tmp_dir = unique_temp_dir("rotation_index_migration");
    // Removes the temp dir even when an assertion below panics.
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(tmp_dir.clone());
    let db_path = tmp_dir.join("catalog.db");
    {
        let catalog = Catalog::open(&db_path).expect("open fresh catalog");
        let id = catalog
            .insert_or_update_batch(&[sample_media_item(
                "/p/turned.jpg",
                MediaType::Image,
                1,
                None,
                None,
            )])
            .expect("insert")[0]
            .id;
        catalog.set_rotations(&[(id, 1)]).expect("rotate");
    }
    // Back to the v20 layout: no rotation index.
    raw_catalog(&db_path)
        .execute_batch("DROP INDEX idx_media_rotated; PRAGMA user_version = 20;")
        .expect("downgrade to v20");
    for pass in 0..2 {
        let catalog = Catalog::open(&db_path).expect("a v20 catalog must open and migrate");
        assert_eq!(
            catalog.schema_version().unwrap(),
            SCHEMA_VERSION,
            "pass {pass}: opening stamps the current schema version"
        );
        let (sql, params) =
            StructuredQuery::parse("is:edited").synthesize_query_sorted(50, 0, SortSpec::default());
        let plan = catalog
            .explain_query_plan(&inline_sql_params(&sql, &params))
            .expect("explain")
            .join(" | ");
        assert!(
            plan.contains("idx_media_rotated"),
            "pass {pass}: the upgrade creates the rotation index, got: {plan}"
        );
        assert_eq!(
            catalog
                .search_structured(&StructuredQuery::parse("is:edited"), 50, 0)
                .expect("search")
                .len(),
            1,
            "pass {pass}: the rotated item still matches is:edited"
        );
    }
}

fn camera_rule(operator: RuleOperator, value: RuleValue) -> lumina_core::SmartAlbum {
    lumina_core::SmartAlbum::new(
        "Camera".into(),
        "camera".into(),
        MatchMode::All,
        vec![SmartAlbumRule {
            field: RuleField::Camera,
            operator,
            value,
        }],
    )
}

fn camera_pair(make: &str, model: &str) -> RuleValue {
    RuleValue::List(vec![make.into(), model.into()])
}

#[test]
fn camera_equals_matches_the_stored_make_and_model_exactly() {
    let catalog = Catalog::open_in_memory().expect("open catalog");
    let cameras: [(Option<&str>, Option<&str>); 8] = [
        (Some("FUJIFILM"), Some("X-T5")),
        (Some("Canon"), Some("Canon EOS R5")),
        (Some("canon"), Some("Canon EOS R5")),
        (Some("Canon"), Some("Canon EOS R5 Mark II")),
        (None, Some("iPhone 15")),
        (None, None),
        (Some(""), Some("")),
        (Some("  "), Some(" ")),
    ];
    let items: Vec<_> = cameras
        .iter()
        .enumerate()
        .map(|(i, (make, model))| {
            let mut item = sample_media_item(
                &format!("/photos/cam/{i}.jpg"),
                MediaType::Image,
                1_000 + i as i64,
                None,
                *model,
            );
            item.make = make.map(str::to_string);
            item
        })
        .collect();
    let ids: Vec<u64> = catalog
        .insert_or_update_batch(&items)
        .expect("insert items")
        .iter()
        .map(|it| it.id.0)
        .collect();
    let matching = |album: &lumina_core::SmartAlbum| {
        let mut got: Vec<u64> = catalog
            .query_smart_album(album, 50, 0)
            .expect("query")
            .iter()
            .map(|m| m.id.0)
            .collect();
        got.sort();
        got
    };

    let is_r5 = camera_rule(RuleOperator::Equals, camera_pair("Canon", "Canon EOS R5"));
    assert_eq!(
        matching(&is_r5),
        vec![ids[1]],
        "is matches the stored make and model exactly: not another spelling, not the Mark II"
    );
    assert_eq!(catalog.query_smart_album_count(&is_r5).unwrap(), 1);
    let (sql, params) = is_r5.synthesize_count_query().unwrap();
    let plan = catalog
        .explain_query_plan(&inline_sql_params(&sql, &params))
        .unwrap()
        .join(" | ");
    assert!(
        plan.contains("SEARCH m USING INDEX idx_media_camera (make=? AND model=?)"),
        "the count seeks the camera index: {plan}"
    );
    let other_case = camera_rule(RuleOperator::Equals, camera_pair("canon", "Canon EOS R5"));
    assert_eq!(
        matching(&other_case),
        vec![ids[2]],
        "case is part of the stored value"
    );

    // The builder lists "Canon" and "canon" under one name and stores every
    // pair it grouped: the rule matches each of them, still by index.
    let grouped = camera_rule(
        RuleOperator::Equals,
        RuleValue::List(vec![
            "Canon".into(),
            "Canon EOS R5".into(),
            "canon".into(),
            "Canon EOS R5".into(),
        ]),
    );
    assert_eq!(
        matching(&grouped),
        vec![ids[1], ids[2]],
        "is matches every stored pair of a grouped camera"
    );
    assert_eq!(catalog.query_smart_album_count(&grouped).unwrap(), 2);
    let (sql, params) = grouped.synthesize_count_query().unwrap();
    let plan = catalog
        .explain_query_plan(&inline_sql_params(&sql, &params))
        .unwrap()
        .join(" | ");
    assert!(
        plan.contains("idx_media_camera (make=? AND model=?)"),
        "each grouped pair still seeks the camera index: {plan}"
    );
    let not_grouped = camera_rule(
        RuleOperator::NotEquals,
        RuleValue::List(vec![
            "Canon".into(),
            "Canon EOS R5".into(),
            "canon".into(),
            "Canon EOS R5".into(),
        ]),
    );
    assert_eq!(
        matching(&not_grouped),
        vec![ids[0], ids[3], ids[4], ids[5], ids[6], ids[7]],
        "is not leaves out every grouped pair"
    );

    let is_phone = camera_rule(RuleOperator::Equals, camera_pair("", "iPhone 15"));
    assert_eq!(
        matching(&is_phone),
        vec![ids[4]],
        "an empty make matches a missing make"
    );

    let not_r5 = camera_rule(
        RuleOperator::NotEquals,
        camera_pair("Canon", "Canon EOS R5"),
    );
    assert_eq!(
        matching(&not_r5),
        vec![ids[0], ids[2], ids[3], ids[4], ids[5], ids[6], ids[7]],
        "is not keeps every other camera and items with no camera"
    );

    let contains = camera_rule(RuleOperator::Contains, RuleValue::Text("canon".into()));
    assert_eq!(
        matching(&contains),
        vec![ids[1], ids[2], ids[3]],
        "old contains rules still match make or model"
    );

    for value in [
        RuleValue::Text("Canon".into()),
        RuleValue::List(vec!["Canon".into()]),
        camera_pair("", ""),
    ] {
        assert!(
            camera_rule(RuleOperator::Equals, value.clone())
                .build_sql()
                .is_err(),
            "is needs a make and model pair with at least one part: {value:?}"
        );
    }

    assert_eq!(
        catalog.distinct_cameras().expect("cameras"),
        vec![
            (String::new(), "iPhone 15".to_string()),
            ("Canon".to_string(), "Canon EOS R5".to_string()),
            ("Canon".to_string(), "Canon EOS R5 Mark II".to_string()),
            ("FUJIFILM".to_string(), "X-T5".to_string()),
            ("canon".to_string(), "Canon EOS R5".to_string()),
        ],
        "each stored camera once, in index order; items with no camera, or only spaces, are skipped"
    );
}
