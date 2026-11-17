//! Engine integration tests on a scanned library: sidecar import and
//! export, catalog failures, and file export.

mod common;

use common::*;
use lumina_core::{
    read_sidecar, write_sidecar, DeletionTarget, EngineEvent, ExportPreset, LuminaEngine, MediaId,
    MediaItem, Rights, SidecarMetadata,
};
use std::fs::File;
use std::io::Write;

#[test]
fn test_desktop_pics_integration() {
    let temp_dir = unique_temp_dir("test_engine");
    let media_dir = temp_dir.join("media");
    let db_path = temp_dir.join("isolated_catalog.db");
    let cache_dir = temp_dir.join("isolated_cache");
    std::fs::create_dir_all(&media_dir).unwrap();

    // Create real valid JPEG images using the image crate
    let img1 = image::RgbImage::from_fn(32, 32, |x, y| {
        image::Rgb([(x * 8) as u8, (y * 8) as u8, 128])
    });
    img1.save(media_dir.join("photo1.jpg")).unwrap();

    let img2 = image::RgbImage::from_fn(24, 24, |x, y| {
        image::Rgb([200, (x * 10) as u8, (y * 10) as u8])
    });
    img2.save(media_dir.join("photo2.jpg")).unwrap();

    let mut vid = File::create(media_dir.join("clip1.mp4")).unwrap();
    vid.write_all(b"dummy mp4 video bytes for scanner test")
        .unwrap();

    // Open LuminaEngine with ISOLATED db and cache paths
    let (engine, rx) =
        lumina_core::LuminaEngine::open_with_paths(media_dir.clone(), db_path, Some(cache_dir))
            .expect("open engine with isolated paths");

    assert_eq!(wait_for_scan(&rx), 3, "Should discover all 3 media items");

    let slice = engine.get_item_slice(0..10);
    assert_eq!(slice.len(), 3);

    // Verify rich metadata parsed
    for item in &slice {
        assert!(item.year >= 1970);
        assert!(item.month >= 1 && item.month <= 12);
        assert!(item.day >= 1 && item.day <= 31);
        assert!(!item.file_name.is_empty());
    }

    // Verify timeline buckets computed
    let year_buckets = engine.get_timeline_buckets("year");
    assert!(!year_buckets.is_empty(), "Should compute year buckets");

    // Test thumbnail decoding on real JPEG
    let photo = slice
        .iter()
        .find(|i| i.media_type == lumina_core::MediaType::Image)
        .expect("found photo");
    let (tx_thumb, rx_thumb) = crossbeam_channel::bounded(1);
    engine.request_thumbnail(photo.id, 64, move |frame| {
        let _ = tx_thumb.send(frame);
    });

    let frame_opt = rx_thumb
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("receive thumbnail");
    assert!(frame_opt.is_some(), "Thumbnail decode succeeded");
    let frame = frame_opt.unwrap();
    assert!(frame.width > 0 && frame.height > 0);

    // Verify P0 Fix: Rescan MUST NOT wipe out favorites or user tags
    let target_id = slice[0].id;
    engine
        .batch_set_favorite(&[target_id], true)
        .expect("set favorite");
    engine
        .add_tags_batch(&[target_id], &["my_vacation".to_string()])
        .expect("add tag");

    // Trigger rescan
    engine.rescan();
    wait_for_scan(&rx);

    // Assert favorite and tags survived rescan
    let rescanned_slice = engine.get_item_slice(0..10);
    let target_item = rescanned_slice
        .iter()
        .find(|i| i.id == target_id)
        .expect("find target");
    assert!(
        target_item.is_favorite,
        "User favorite MUST survive rescan!"
    );
    let tags = engine.get_tags_for_item(target_id);
    assert_eq!(
        tags,
        vec!["my_vacation".to_string()],
        "Tags MUST survive rescan!"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_import_sidecars_applies_title_caption_rating_and_tags() {
    let (tmp_dir, _db_path, engine, _rx) = scanned_library("import");
    let item = engine.get_item_slice(0..1).remove(0);

    write_sidecar(
        &item.path,
        &SidecarMetadata {
            title: Some("Imported title".to_string()),
            caption: Some("Imported caption".to_string()),
            rating: 5,
            color_label: 4,
            tags: vec!["imported".to_string()],
            ..Default::default()
        },
    )
    .unwrap();

    let report = engine.import_sidecars(&[item.id]).unwrap();
    assert_eq!(report.imported, 1);
    assert_eq!(report.updated, 1);

    let updated = engine.catalog().get_item_by_id(item.id).unwrap().unwrap();
    assert_eq!(updated.title.as_deref(), Some("Imported title"));
    assert_eq!(engine.catalog().get_rating(item.id).unwrap(), 5);
    assert_eq!(
        engine.get_color_labels_map().get(&item.id),
        Some(&4),
        "a sidecar's color label is imported"
    );
    assert_eq!(
        engine
            .catalog()
            .get_captions_map()
            .unwrap()
            .get(&item.id)
            .map(String::as_str),
        Some("Imported caption")
    );
    assert!(engine
        .catalog()
        .get_tags_for_item(item.id)
        .unwrap()
        .contains(&"imported".to_string()));

    drop(engine);
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn import_never_clears_rights() {
    let (tmp_dir, _db_path, engine, _rx) = scanned_library("rights_import");
    let item = engine.get_item_slice(0..1).remove(0);
    let full = Rights {
        creator: Some("Ada".to_string()),
        copyright: Some("© 2026 Ada".to_string()),
        usage_terms: Some("Editorial use only".to_string()),
        contact_email: Some("ada@example.com".to_string()),
        contact_url: Some("https://ada.example".to_string()),
    };
    engine
        .catalog()
        .set_rights(&[(item.id, full.clone())])
        .unwrap();

    // The engine's sidecar sync writes the catalog's rights.
    engine.write_metadata_sidecars(&[item.id]).unwrap();
    assert_eq!(
        read_sidecar(&item.path)
            .unwrap()
            .map(|sidecar| sidecar.rights),
        Some(full.clone()),
        "write_metadata_sidecars must carry rights into the sidecar"
    );

    // A sidecar naming only a new creator updates it and keeps the rest.
    write_sidecar(
        &item.path,
        &SidecarMetadata {
            rights: Rights {
                creator: Some("Bea".to_string()),
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .unwrap();
    let report = engine.import_sidecars(&[item.id]).unwrap();
    assert_eq!(report.updated, 1, "a changed creator counts as an update");
    assert_eq!(
        engine.catalog().get_rights(&[item.id]).unwrap()[&item.id],
        Rights {
            creator: Some("Bea".to_string()),
            ..full
        },
        "import applies the fields a sidecar sets and never clears the others"
    );
    let again = engine.import_sidecars(&[item.id]).unwrap();
    assert_eq!(again.updated, 0, "an unchanged sidecar is not an update");

    drop(engine);
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn unedited_date_is_not_written() {
    let (tmp_dir, _db_path, engine, _rx) = scanned_library("xmp_unedited_date");
    let item = engine.get_item_slice(0..1).remove(0);
    engine.catalog().set_rating(item.id, 3).unwrap();
    engine.write_metadata_sidecars(&[item.id]).unwrap();
    let sidecar = read_sidecar(&item.path)
        .unwrap()
        .expect("a rating writes a sidecar");
    assert_eq!(
        (sidecar.date_taken, sidecar.location),
        (None, None),
        "a date and place read from the file are not written; file EXIF stays the source"
    );
    // Coordinates from the scan (place_edited unset) stay out too.
    raw_catalog(&tmp_dir.join("catalog.db"))
        .execute(
            "UPDATE media_items SET latitude = 1.5, longitude = 2.5 WHERE id = ?1",
            [item.id.0 as i64],
        )
        .unwrap();
    engine.write_metadata_sidecars(&[item.id]).unwrap();
    assert_eq!(
        read_sidecar(&item.path).unwrap().unwrap().location,
        None,
        "an unedited place is not written, even when the item has coordinates"
    );

    engine.set_timestamp(item.id, 1_700_000_000_000).unwrap();
    engine
        .set_location(item.id, Some(-33.8688), Some(151.2093))
        .unwrap();
    engine.write_metadata_sidecars(&[item.id]).unwrap();
    let sidecar = read_sidecar(&item.path).unwrap().unwrap();
    assert_eq!(
        sidecar
            .date_taken
            .map(|date| date.and_utc().timestamp_millis()),
        Some(1_700_000_000_000),
        "an edited date is written as the catalog's wall time"
    );
    let (lat, lon) = sidecar.location.expect("an edited place is written");
    assert!((lat + 33.8688).abs() < 1e-6 && (lon - 151.2093).abs() < 1e-6);

    drop(engine);
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn date_only_sidecar_keeps_the_time_of_day() {
    let (tmp_dir, _db_path, engine, _rx) = scanned_library("xmp_date_only");
    let item = engine.get_item_slice(0..1).remove(0);
    let time_of_day = chrono::DateTime::from_timestamp_millis(item.timestamp_epoch_ms)
        .unwrap()
        .time();
    let sidecar = |day: &str| {
        std::fs::write(
            lumina_core::sidecar_path(&item.path),
            format!(
                r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="" xmlns:photoshop="http://ns.adobe.com/photoshop/1.0/" photoshop:DateCreated="{day}"/></rdf:RDF></x:xmpmeta>"#
            ),
        )
        .unwrap();
    };
    sidecar("2020-02-03");
    let report = engine.import_sidecars(&[item.id]).unwrap();
    assert_eq!(report.updated, 1, "a new day is an update");
    let expected = chrono::NaiveDate::from_ymd_opt(2020, 2, 3)
        .unwrap()
        .and_time(time_of_day)
        .and_utc()
        .timestamp_millis();
    assert_eq!(
        engine.get_item_by_id(item.id).unwrap().timestamp_epoch_ms,
        expected,
        "a date without a time changes the day and keeps the catalog's time of day"
    );
    let again = engine.import_sidecars(&[item.id]).unwrap();
    assert_eq!(again.updated, 0, "the same day again is not an update");

    drop(engine);
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn import_sets_edited_flags_so_rescan_keeps_date() {
    let (tmp_dir, _db_path, engine, rx) = scanned_library("xmp_import_date");
    let item = engine.get_item_slice(0..1).remove(0);
    let date = chrono::DateTime::from_timestamp(1_400_000_000, 0)
        .unwrap()
        .naive_utc();
    write_sidecar(
        &item.path,
        &SidecarMetadata {
            date_taken: Some(date),
            location: Some((40.712776, -74.005974)),
            ..Default::default()
        },
    )
    .unwrap();
    let report = engine.import_sidecars(&[item.id]).unwrap();
    assert_eq!(report.updated, 1, "a new date and place count as an update");
    assert_eq!(
        engine.catalog().edited_flags(&[item.id]).unwrap()[&item.id],
        (true, true),
        "imported dates and places are marked edited"
    );
    let again = engine.import_sidecars(&[item.id]).unwrap();
    assert_eq!(again.updated, 0, "an unchanged sidecar is not an update");

    // A changed file is re-read by the rescan; the imported values win.
    std::fs::write(&item.path, b"test-jpeg-content, edited elsewhere").unwrap();
    engine.rescan();
    wait_for_scan(&rx);
    let after = engine.get_item_by_id(item.id).unwrap();
    assert_ne!(
        after.file_size, item.file_size,
        "the rescan read the new file"
    );
    assert_eq!(
        after.timestamp_epoch_ms, 1_400_000_000_000,
        "a rescan keeps the imported date"
    );
    assert_eq!(
        (after.latitude, after.longitude),
        (Some(40.712776), Some(-74.005974)),
        "a rescan keeps the imported place"
    );

    drop(engine);
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_export_items_copies_files_and_writes_xmp_sidecars() {
    let (tmp_dir, engine, _rx) = library_with(
        "export",
        &[
            ("trip_01.jpg", b"test-jpeg-content".to_vec()),
            ("trip_02.jpg", b"test-jpeg-content".to_vec()),
        ],
    );

    let items = engine.get_item_slice(0..10);
    assert_eq!(items.len(), 2, "both seeded files should be indexed");
    let rated = &items[0];
    engine.catalog().set_rating(rated.id, 4).unwrap();
    engine
        .catalog()
        .set_caption(rated.id, "Golden hour in Kyoto")
        .unwrap();
    engine
        .catalog()
        .add_tags_batch(&[rated.id], &["travel".to_string()])
        .unwrap();
    engine.catalog().set_color_label(rated.id, 5).unwrap();

    let ids: Vec<MediaId> = items.iter().map(|item| item.id).collect();
    let destination = tmp_dir.join("exported");
    let report = engine
        .export_items_with(
            &ids,
            &destination,
            ExportPreset::Originals,
            &std::sync::atomic::AtomicBool::new(false),
        )
        .unwrap();
    assert_eq!(report.exported, 2);
    assert_eq!(report.sidecars, 1, "only the annotated item gets a sidecar");
    assert!(
        report.failed.is_empty(),
        "unexpected failures: {:?}",
        report.failed
    );

    let copied = destination.join(&rated.file_name);
    assert!(copied.exists(), "original must be copied");
    let metadata = read_sidecar(&copied)
        .expect("sidecar read")
        .expect("sidecar should exist");
    assert_eq!(metadata.rating, 4);
    assert_eq!(metadata.caption.as_deref(), Some("Golden hour in Kyoto"));
    assert_eq!(metadata.tags, vec!["travel".to_string()]);
    assert_eq!(metadata.color_label, 5, "the color label is exported");

    // Re-exporting must not overwrite the first copy.
    let second = engine
        .export_items_with(
            &[rated.id],
            &destination,
            ExportPreset::Originals,
            &std::sync::atomic::AtomicBool::new(false),
        )
        .unwrap();
    assert_eq!(second.exported, 1);
    let renamed = destination.join(format!(
        "{} (1).jpg",
        rated
            .file_name
            .strip_suffix(".jpg")
            .unwrap_or(&rated.file_name)
    ));
    assert!(renamed.exists(), "collision should produce a numbered copy");

    drop(engine);
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

fn error_events(rx: &crossbeam_channel::Receiver<EngineEvent>) -> Vec<String> {
    rx.try_iter()
        .filter_map(|event| match event {
            EngineEvent::Error(message) => Some(message),
            _ => None,
        })
        .collect()
}

#[test]
fn a_broken_catalog_raises_one_error_instead_of_a_silent_empty_library() {
    let (tmp_dir, db_path, engine, rx) = scanned_library("broken_read");
    assert_eq!(engine.total_count(), 1);
    error_events(&rx);

    raw_catalog(&db_path)
        .execute_batch("ALTER TABLE media_items RENAME TO media_items_gone;")
        .unwrap();
    assert!(engine.get_item_slice(0..10).is_empty());
    assert_eq!(engine.total_count(), 0);

    let errors = error_events(&rx);
    assert_eq!(
        errors.len(),
        1,
        "a failed read must raise exactly one banner, not one per query: {errors:?}"
    );
    assert!(
        errors[0].starts_with("Could not read the catalog:"),
        "{errors:?}"
    );

    engine.rearm_read_errors();
    engine.total_count();
    assert_eq!(
        error_events(&rx).len(),
        1,
        "after the banner is dismissed, a later failure must raise it again"
    );

    drop(engine);
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn sidecars_are_not_rewritten_when_captions_cannot_be_read() {
    let (tmp_dir, db_path, engine, _rx) = scanned_library("broken_sidecar");
    let item = engine.get_item_slice(0..1).remove(0);
    engine.catalog().set_rating(item.id, 3).unwrap();
    engine.catalog().set_caption(item.id, "keep me").unwrap();
    assert_eq!(
        engine.write_metadata_sidecars(&[item.id]).unwrap().written,
        1
    );

    raw_catalog(&db_path)
        .execute_batch("DROP TABLE media_captions;")
        .unwrap();
    assert!(
        engine.write_metadata_sidecars(&[item.id]).is_err(),
        "an unreadable caption table must fail the write, not write no caption"
    );
    assert!(engine
        .export_items_with(
            &[item.id],
            &tmp_dir.join("exported"),
            ExportPreset::Originals,
            &std::sync::atomic::AtomicBool::new(false)
        )
        .is_err());
    let kept = read_sidecar(&item.path).unwrap().unwrap();
    assert_eq!(
        kept.caption.as_deref(),
        Some("keep me"),
        "the existing sidecar keeps its caption"
    );

    drop(engine);
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn a_rescan_stops_when_the_catalog_file_list_cannot_be_read() {
    let (tmp_dir, db_path, engine, rx) = scanned_library("broken_scan");
    error_events(&rx);
    raw_catalog(&db_path)
        .execute_batch("ALTER TABLE media_items RENAME TO media_items_gone;")
        .unwrap();

    engine.rescan();
    let mut errors = Vec::new();
    while let Ok(event) = rx.recv_timeout(std::time::Duration::from_secs(10)) {
        match event {
            EngineEvent::Error(message) => errors.push(message),
            EngineEvent::ScanComplete { .. } => break,
            _ => {}
        }
    }
    assert!(
        errors.iter().any(|m| m.starts_with("Library scan failed:")),
        "an unknown catalog must stop the scan, not re-insert every file: {errors:?}"
    );

    drop(engine);
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn a_label_alone_gets_a_sidecar_and_a_labelless_sidecar_keeps_the_label() {
    let (tmp_dir, _db_path, engine, _rx) = scanned_library("label_sidecar");
    let item = engine.get_item_slice(0..1).remove(0);
    engine.set_color_labels(&[(item.id, 2)]).unwrap();
    assert_eq!(
        engine.write_metadata_sidecars(&[item.id]).unwrap().written,
        1,
        "a color label alone is worth a sidecar"
    );
    assert_eq!(read_sidecar(&item.path).unwrap().unwrap().color_label, 2);

    // A sidecar written before labels were exported has no xmp:Label.
    write_sidecar(
        &item.path,
        &SidecarMetadata {
            rating: 1,
            ..Default::default()
        },
    )
    .unwrap();
    engine.import_sidecars(&[item.id]).unwrap();
    assert_eq!(
        engine.get_color_labels_map().get(&item.id),
        Some(&2),
        "a sidecar without a label must not clear the catalog's label"
    );

    drop(engine);
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn keyword_paths_survive_a_sidecar_round_trip() {
    let (tmp_dir, _db_path, engine, _rx) = scanned_library("keyword_sidecar");
    let item = engine.get_item_slice(0..1).remove(0);
    let tags = vec!["europe".to_string(), "places/europe/paris".to_string()];
    engine.add_tags_batch(&[item.id], &tags).unwrap();
    assert_eq!(
        engine.write_metadata_sidecars(&[item.id]).unwrap().written,
        1
    );
    let xml = std::fs::read_to_string(lumina_core::sidecar_path(&item.path)).unwrap();
    assert!(
        xml.contains("<rdf:li>places|europe|paris</rdf:li>"),
        "the sidecar carries the path in Lightroom's hierarchy: {xml}"
    );

    engine.remove_tags_batch(&[item.id], &tags).unwrap();
    assert!(engine.get_tags_for_item(item.id).is_empty());
    let report = engine.import_sidecars(&[item.id]).unwrap();
    assert!(report.failed.is_empty(), "{:?}", report.failed);
    let mut back = engine.get_tags_for_item(item.id);
    back.sort();
    assert_eq!(
        back, tags,
        "import restores the path and the flat tag, and no segment becomes a tag"
    );

    drop(engine);
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn a_batch_that_fails_partway_changes_nothing() {
    let (tmp_dir, db_path, engine, rx) = scanned_library("batch_abort");
    std::fs::write(
        tmp_dir.join("library").join("two.jpg"),
        b"test-jpeg-content",
    )
    .unwrap();
    engine.rescan();
    wait_for_scan(&rx);
    assert_eq!(engine.total_count(), 2, "both photos indexed");
    let ids: Vec<MediaId> = engine.get_item_slice(0..2).iter().map(|i| i.id).collect();
    let first = engine.get_item_by_id(ids[0]).unwrap();
    engine.set_caption(ids[0], "old caption").unwrap();

    // Any write that reaches the second item aborts its statement.
    raw_catalog(&db_path)
        .execute_batch(&format!(
            "CREATE TRIGGER fail_second BEFORE UPDATE ON media_items WHEN new.id = {0}
             BEGIN SELECT RAISE(ABORT, 'injected'); END;
             CREATE TRIGGER fail_second_caption BEFORE INSERT ON media_captions WHEN new.media_id = {0}
             BEGIN SELECT RAISE(ABORT, 'injected'); END;",
            ids[1].0
        ))
        .unwrap();

    assert!(engine.set_ratings(&[(ids[0], 4), (ids[1], 4)]).is_err());
    assert!(engine
        .set_color_labels(&[(ids[0], 2), (ids[1], 2)])
        .is_err());
    assert!(engine
        .set_titles(&[(ids[0], Some("new")), (ids[1], Some("new"))])
        .is_err());
    assert!(engine
        .set_timestamps(&[(ids[0], 1_600_000_000_000), (ids[1], 1_600_000_000_000)])
        .is_err());
    let place = (Some(35.68), Some(139.69));
    assert!(engine
        .set_locations(&[(ids[0], place), (ids[1], place)])
        .is_err());
    assert!(engine
        .set_captions(&[(ids[0], "new caption"), (ids[1], "new caption")])
        .is_err());

    let after = engine.get_item_by_id(ids[0]).unwrap();
    assert_eq!(
        engine.get_ratings_map().get(&ids[0]),
        None,
        "rating rolled back"
    );
    assert_eq!(
        engine.get_color_labels_map().get(&ids[0]),
        None,
        "label rolled back"
    );
    assert_eq!(after.title, first.title, "title rolled back");
    assert_eq!(
        after.timestamp_epoch_ms, first.timestamp_epoch_ms,
        "date rolled back"
    );
    assert_eq!(
        (after.latitude, after.city),
        (first.latitude, first.city),
        "place rolled back"
    );
    assert_eq!(
        engine.get_captions_map().get(&ids[0]).map(String::as_str),
        Some("old caption"),
        "caption rolled back"
    );
    assert_eq!(
        engine.search_query_count("old"),
        1,
        "the search index still holds the old caption"
    );

    // A failed timeline rebuild rolls the dates back with it.
    raw_catalog(&db_path)
        .execute_batch(
            "CREATE TRIGGER fail_buckets BEFORE INSERT ON timeline_buckets
             BEGIN SELECT RAISE(ABORT, 'injected'); END;",
        )
        .unwrap();
    assert!(engine
        .set_timestamps(&[(ids[0], 1_600_000_000_000)])
        .is_err());
    assert_eq!(
        engine.get_item_by_id(ids[0]).unwrap().timestamp_epoch_ms,
        first.timestamp_epoch_ms,
        "dates and their buckets commit together"
    );

    drop(engine);
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_network_import_source_attribution_and_cascade() {
    let (tmp_dir, _db_path, engine, _rx) = scanned_library("net_import_attr");

    let item = engine.get_item_slice(0..1).remove(0);

    let source_url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";
    let uploader = "Rick Astley";

    // Record source attribution
    engine
        .catalog()
        .record_media_source(item.id, source_url, Some(uploader))
        .unwrap();

    // Verify lookup
    let record = engine.get_media_source(item.id).unwrap();
    assert!(record.is_some(), "source record should be present");
    let (url, up, ts): (String, Option<String>, i64) = record.unwrap();
    assert_eq!(url, source_url);
    assert_eq!(up.as_deref(), Some(uploader));
    assert!(ts > 0);

    // Network imports land in the dated Imports folder
    assert!(engine
        .network_import_dir()
        .starts_with(tmp_dir.join("library/Imports")));

    // Verify cascade deletion
    engine
        .permanently_delete_items(&[item.id], DeletionTarget::CatalogOnly)
        .unwrap();

    let record_after = engine.get_media_source(item.id).unwrap();
    assert!(
        record_after.is_none(),
        "source record must be cascade deleted when media item is removed"
    );

    drop(engine);
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

/// darktable keeps its edit history in `photo.jpg.xmp`, the name Lumina uses.
const DARKTABLE_SIDECAR: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="XMP Core 4.4.0-Exiv2">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:xmp="http://ns.adobe.com/xap/1.0/"
    xmlns:darktable="http://darktable.sf.net/"
   xmp:Rating="1"
   darktable:xmp_version="5"
   darktable:history_end="1">
   <darktable:history>
    <rdf:Seq>
     <rdf:li darktable:num="0" darktable:operation="exposure" darktable:enabled="1"/>
    </rdf:Seq>
   </darktable:history>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>
"#;

#[test]
fn foreign_sidecar_is_not_overwritten() {
    let (tmp_dir, _db_path, engine, _rx) = scanned_library("foreign_sidecar");
    let item = engine.get_item_slice(0..1).remove(0);
    let sidecar = lumina_core::sidecar_path(&item.path);
    std::fs::write(&sidecar, DARKTABLE_SIDECAR).unwrap();

    engine.catalog().set_rating(item.id, 4).unwrap();
    let report = engine.write_metadata_sidecars(&[item.id]).unwrap();
    assert_eq!(
        std::fs::read_to_string(&sidecar).unwrap(),
        DARKTABLE_SIDECAR,
        "another app's sidecar must be byte-identical after a Lumina edit"
    );
    assert_eq!(report.written, 0, "nothing was written");
    assert_eq!(
        report.skipped_foreign,
        vec![sidecar.clone()],
        "the skipped sidecar is reported"
    );

    engine.catalog().set_rating(item.id, 0).unwrap();
    let report = engine.write_metadata_sidecars(&[item.id]).unwrap();
    assert_eq!(
        std::fs::read_to_string(&sidecar).unwrap(),
        DARKTABLE_SIDECAR,
        "blank Lumina fields must not delete another app's sidecar"
    );
    assert_eq!(report.skipped_foreign, vec![sidecar]);

    drop(engine);
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

// ---- Export presets ----
// ponytail: fixtures stay at or below 2560 px; larger JPEGs decode through
// sips, whose orientation path is checked by the format matrix's export cell.

/// A solid red baseline JPEG, `width` x `height`.
fn red_jpeg(width: u32, height: u32) -> Vec<u8> {
    let image = image::RgbImage::from_pixel(width, height, image::Rgb([230, 10, 10]));
    let mut bytes = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 95)
        .encode_image(&image)
        .unwrap();
    bytes
}

/// `jpeg` with an EXIF APP1 holding a GPS position, right after SOI.
fn with_gps_exif(jpeg: &[u8]) -> Vec<u8> {
    use exif::{Field, In, Rational, Tag, Value};
    let degrees =
        |d: u32| Value::Rational(vec![Rational::from((d, 1)), (0, 1).into(), (0, 1).into()]);
    let fields = [
        Field {
            tag: Tag::GPSLatitudeRef,
            ifd_num: In::PRIMARY,
            value: Value::Ascii(vec![b"N".to_vec()]),
        },
        Field {
            tag: Tag::GPSLatitude,
            ifd_num: In::PRIMARY,
            value: degrees(35),
        },
        Field {
            tag: Tag::GPSLongitudeRef,
            ifd_num: In::PRIMARY,
            value: Value::Ascii(vec![b"E".to_vec()]),
        },
        Field {
            tag: Tag::GPSLongitude,
            ifd_num: In::PRIMARY,
            value: degrees(135),
        },
    ];
    with_exif(jpeg, &fields)
}

/// `jpeg` with an EXIF APP1 holding `fields`, right after SOI.
fn with_exif(jpeg: &[u8], fields: &[exif::Field]) -> Vec<u8> {
    let mut writer = exif::experimental::Writer::new();
    for field in fields {
        writer.push_field(field);
    }
    let mut tiff = std::io::Cursor::new(Vec::new());
    writer.write(&mut tiff, false).unwrap();
    let tiff = tiff.into_inner();
    let mut out = jpeg[..2].to_vec();
    out.extend_from_slice(&[0xFF, 0xE1]);
    out.extend_from_slice(&((2 + 6 + tiff.len()) as u16).to_be_bytes());
    out.extend_from_slice(b"Exif\0\0");
    out.extend_from_slice(&tiff);
    out.extend_from_slice(&jpeg[2..]);
    out
}

fn item_named(engine: &LuminaEngine, name: &str) -> MediaItem {
    engine
        .get_item_slice(0..100)
        .into_iter()
        .find(|item| item.file_name == name)
        .unwrap_or_else(|| panic!("{name} must be catalogued"))
}

fn export_jpeg(
    engine: &LuminaEngine,
    ids: &[MediaId],
    destination: &std::path::Path,
) -> lumina_core::ExportReport {
    engine
        .export_items_with(
            ids,
            destination,
            lumina_core::ExportPreset::Jpeg2048,
            &std::sync::atomic::AtomicBool::new(false),
        )
        .expect("a JPEG export must not fail as a whole")
}

/// True when the JPEG has an APP1 segment (EXIF or XMP).
fn has_app1(jpeg: &[u8]) -> bool {
    jpeg.windows(2).any(|pair| pair == [0xFF, 0xE1])
}

#[test]
fn jpeg_export_applies_rotation_and_strips_gps() {
    let source = with_gps_exif(&red_jpeg(64, 32));
    assert!(has_app1(&source), "control: the fixture carries EXIF GPS");
    let (tmp_dir, engine, _rx) = library_with("jpeg_rotation", &[("photo.jpg", source)]);
    let photo = item_named(&engine, "photo.jpg");
    engine.set_rotations(&[(photo.id, 1)]).unwrap();

    let destination = tmp_dir.join("out");
    let report = export_jpeg(&engine, &[photo.id], &destination);
    assert_eq!(report.exported, 1, "failures: {:?}", report.failed);

    let bytes = std::fs::read(destination.join("photo.jpg")).unwrap();
    assert!(
        !has_app1(&bytes),
        "the exported JPEG must carry no EXIF/XMP APP1, so no GPS"
    );
    assert!(
        !bytes.windows(2).any(|pair| pair == [0xFF, 0xE2]),
        "the exported JPEG must carry no APP2 ICC profile"
    );
    let decoded = image::load_from_memory(&bytes).unwrap();
    assert_eq!(
        (decoded.width(), decoded.height()),
        (32, 64),
        "a quarter turn in the catalog must turn the exported pixels"
    );

    drop(engine);
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn jpeg_export_applies_quick_edit_crop() {
    let (tmp_dir, engine, _rx) = library_with("jpeg_crop", &[("photo.jpg", red_jpeg(64, 32))]);
    let photo = item_named(&engine, "photo.jpg");
    let edits = lumina_core::Edits {
        rotation: 0,
        crop: Some(lumina_core::CropRect {
            x: 0.0,
            y: 0.0,
            w: 0.25,
            h: 1.0,
        }),
        exposure_ev: 0.0,
        auto_enhance: false,
    };
    engine.set_edits(&[(photo.id, edits)]).unwrap();

    let destination = tmp_dir.join("out");
    let report = export_jpeg(&engine, &[photo.id], &destination);
    assert_eq!(report.exported, 1, "failures: {:?}", report.failed);
    let decoded = image::open(destination.join("photo.jpg")).unwrap();
    assert_eq!(
        (decoded.width(), decoded.height()),
        (16, 32),
        "the JPEG export applies the viewer's crop"
    );

    drop(engine);
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn jpeg_export_never_overwrites() {
    let (tmp_dir, engine, _rx) = library_with("jpeg_collide", &[("photo.jpg", red_jpeg(16, 16))]);
    let photo = item_named(&engine, "photo.jpg");
    let destination = tmp_dir.join("out");
    std::fs::create_dir_all(&destination).unwrap();
    std::fs::write(destination.join("photo.jpg"), b"keep me").unwrap();

    let report = export_jpeg(&engine, &[photo.id], &destination);
    assert_eq!(report.exported, 1, "failures: {:?}", report.failed);
    assert_eq!(
        std::fs::read(destination.join("photo.jpg")).unwrap(),
        b"keep me",
        "an existing file in the export folder is never overwritten"
    );
    let renamed = std::fs::read(destination.join("photo (1).jpg"))
        .expect("the export takes the next free numbered name");
    image::load_from_memory(&renamed).expect("the numbered file is the exported JPEG");

    drop(engine);
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn export_cancel_stops_between_files() {
    let (tmp_dir, engine, _rx) = library_with(
        "jpeg_cancel",
        &[("a.jpg", red_jpeg(16, 16)), ("b.jpg", red_jpeg(16, 16))],
    );
    let ids = [
        item_named(&engine, "a.jpg").id,
        item_named(&engine, "b.jpg").id,
    ];
    let destination = tmp_dir.join("out");
    let cancel = std::sync::atomic::AtomicBool::new(true);
    for preset in [
        lumina_core::ExportPreset::Originals,
        lumina_core::ExportPreset::Jpeg2048,
    ] {
        let report = engine
            .export_items_with(&ids, &destination, preset, &cancel)
            .unwrap();
        assert!(report.cancelled, "{preset:?}: the report says it stopped");
        assert_eq!(report.exported, 0, "{preset:?}: no file after a cancel");
    }
    let written = std::fs::read_dir(&destination)
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(written, 0, "a cancelled export writes nothing more");

    drop(engine);
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn missing_source_reported_not_fatal() {
    let (tmp_dir, engine, _rx) = library_with(
        "jpeg_missing",
        &[
            ("gone.jpg", red_jpeg(16, 16)),
            ("here.jpg", red_jpeg(16, 16)),
        ],
    );
    let gone = item_named(&engine, "gone.jpg");
    let here = item_named(&engine, "here.jpg");
    std::fs::remove_file(&gone.path).unwrap();

    let destination = tmp_dir.join("out");
    let report = export_jpeg(&engine, &[gone.id, here.id], &destination);
    assert_eq!(report.exported, 1, "the file that exists is still exported");
    assert_eq!(
        report
            .failed
            .iter()
            .map(|(path, _)| path)
            .collect::<Vec<_>>(),
        vec![&gone.path],
        "the missing source is reported by path"
    );
    assert!(destination.join("here.jpg").exists());

    drop(engine);
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn jpeg_export_copies_videos_as_originals() {
    let video = b"not really a movie".to_vec();
    let (tmp_dir, engine, _rx) = library_with(
        "jpeg_videos",
        &[
            ("photo.jpg", red_jpeg(16, 16)),
            ("clip.mp4", video.clone()),
            ("clip2.mov", video.clone()),
        ],
    );
    let photo = item_named(&engine, "photo.jpg");
    engine.catalog().set_rating(photo.id, 5).unwrap();
    let ids = [
        photo.id,
        item_named(&engine, "clip.mp4").id,
        item_named(&engine, "clip2.mov").id,
    ];

    let destination = tmp_dir.join("out");
    let report = export_jpeg(&engine, &ids, &destination);
    assert_eq!(report.exported, 3, "failures: {:?}", report.failed);
    assert_eq!(
        report.videos_as_originals, 2,
        "the report counts videos copied as originals"
    );
    assert_eq!(report.sidecars, 0, "the JPEG preset writes no sidecar");
    assert_eq!(std::fs::read(destination.join("clip.mp4")).unwrap(), video);
    assert_eq!(std::fs::read(destination.join("clip2.mov")).unwrap(), video);
    let names: Vec<String> = std::fs::read_dir(&destination)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        !names.iter().any(|name| name.ends_with(".xmp")),
        "no sidecar for a rated photo or a video: {names:?}"
    );

    drop(engine);
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn jpeg_export_pixels_are_rgb_not_bgr() {
    let (tmp_dir, engine, _rx) = library_with("jpeg_red", &[("red.jpg", red_jpeg(32, 32))]);
    let red = item_named(&engine, "red.jpg");
    let destination = tmp_dir.join("out");
    let report = export_jpeg(&engine, &[red.id], &destination);
    assert_eq!(report.exported, 1, "failures: {:?}", report.failed);
    let decoded = image::open(destination.join("red.jpg")).unwrap().to_rgb8();
    let image::Rgb([r, g, b]) = *decoded.get_pixel(16, 16);
    assert!(
        r > 200 && g < 50 && b < 50,
        "a red source stays red, not blue (BGRA must be swizzled): {:?}",
        (r, g, b)
    );

    drop(engine);
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn jpeg_export_caps_long_side_at_2048() {
    let (tmp_dir, engine, _rx) = library_with("jpeg_cap", &[("wide.jpg", red_jpeg(2560, 1280))]);
    let wide = item_named(&engine, "wide.jpg");
    let destination = tmp_dir.join("out");
    let report = export_jpeg(&engine, &[wide.id], &destination);
    assert_eq!(report.exported, 1, "failures: {:?}", report.failed);
    let decoded = image::open(destination.join("wide.jpg")).unwrap();
    assert_eq!(
        (decoded.width(), decoded.height()),
        (2048, 1024),
        "a 2560 px photo exports at exactly 2048 px on the long side"
    );

    drop(engine);
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn jpeg_export_caps_a_crop_of_an_exif_rotated_photo() {
    use exif::{Field, In, Tag, Value};
    let source = with_exif(
        &red_jpeg(2560, 1280),
        &[Field {
            tag: Tag::Orientation,
            ifd_num: In::PRIMARY,
            value: Value::Short(vec![6]),
        }],
    );
    let (tmp_dir, engine, _rx) = library_with("jpeg_cap_crop", &[("turned.jpg", source)]);
    let turned = item_named(&engine, "turned.jpg");
    let edits = lumina_core::Edits {
        rotation: 0,
        crop: Some(lumina_core::CropRect {
            x: 0.0,
            y: 0.0,
            w: 0.5,
            h: 1.0,
        }),
        exposure_ev: 0.0,
        auto_enhance: false,
    };
    engine.set_edits(&[(turned.id, edits)]).unwrap();

    let destination = tmp_dir.join("out");
    let report = export_jpeg(&engine, &[turned.id], &destination);
    assert_eq!(report.exported, 1, "failures: {:?}", report.failed);
    let decoded = image::open(destination.join("turned.jpg")).unwrap();
    assert!(
        decoded.width().max(decoded.height()) <= 2048,
        "the long side never exceeds 2048 px, got {}x{}",
        decoded.width(),
        decoded.height()
    );
    assert!(
        decoded.height() > decoded.width(),
        "the crop of the portrait (EXIF 6) photo stays portrait, got {}x{}",
        decoded.width(),
        decoded.height()
    );

    drop(engine);
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[cfg(target_os = "macos")]
#[test]
fn jpeg_export_never_upscales_a_small_heic() {
    let scratch = unique_temp_dir("heic_src");
    let png = scratch.join("small.png");
    image::RgbImage::from_pixel(64, 32, image::Rgb([230, 10, 10]))
        .save(&png)
        .unwrap();
    let heic = scratch.join("small.heic");
    let made = std::process::Command::new("sips")
        .args(["-s", "format", "heic"])
        .arg(&png)
        .arg("--out")
        .arg(&heic)
        .output()
        .is_ok_and(|out| out.status.success() && heic.exists());
    assert!(
        made,
        "sips must write HEIC on a supported Mac; this test cannot run without it"
    );
    let bytes = std::fs::read(&heic).unwrap();
    let _ = std::fs::remove_dir_all(&scratch);

    let (tmp_dir, engine, _rx) = library_with("jpeg_heic", &[("small.heic", bytes)]);
    let small = item_named(&engine, "small.heic");
    let destination = tmp_dir.join("out");
    let report = export_jpeg(&engine, &[small.id], &destination);
    assert_eq!(report.exported, 1, "failures: {:?}", report.failed);
    let decoded = image::open(destination.join("small.jpg")).unwrap();
    assert!(
        decoded.width().max(decoded.height()) <= 64,
        "a 64 px HEIC is never upscaled, got {}x{}",
        decoded.width(),
        decoded.height()
    );

    drop(engine);
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn jpeg_export_counts_videos_that_lose_their_rotation() {
    let video = b"not really a movie".to_vec();
    let (tmp_dir, engine, _rx) = library_with(
        "jpeg_video_rotation",
        &[("turned.mp4", video.clone()), ("plain.mp4", video)],
    );
    let turned = item_named(&engine, "turned.mp4");
    let plain = item_named(&engine, "plain.mp4");
    engine.set_rotations(&[(turned.id, 1)]).unwrap();

    let report = export_jpeg(&engine, &[turned.id, plain.id], &tmp_dir.join("out"));
    assert_eq!(report.videos_as_originals, 2);
    assert_eq!(
        report.videos_without_rotation, 1,
        "only the rotated video is reported as copied without its rotation"
    );

    drop(engine);
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn originals_export_is_byte_identical() {
    let payload: Vec<u8> = (0..=255u8).cycle().take(300_000).collect();
    let (tmp_dir, engine, _rx) = library_with("originals_bytes", &[("raw.jpg", payload.clone())]);
    let item = item_named(&engine, "raw.jpg");
    let destination = tmp_dir.join("out");
    let report = engine
        .export_items_with(
            &[item.id],
            &destination,
            ExportPreset::Originals,
            &std::sync::atomic::AtomicBool::new(false),
        )
        .unwrap();
    assert_eq!(report.exported, 1, "failures: {:?}", report.failed);
    assert_eq!(
        std::fs::read(destination.join("raw.jpg")).unwrap(),
        payload,
        "an original is copied byte for byte"
    );

    drop(engine);
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[cfg(target_os = "macos")]
#[test]
fn originals_export_keeps_mode_dates_and_xattrs() {
    use std::os::unix::fs::PermissionsExt;
    let (tmp_dir, engine, _rx) = library_with("originals_meta", &[("kept.jpg", b"jpeg".to_vec())]);
    let item = item_named(&engine, "kept.jpg");
    let ok = |cmd: &mut std::process::Command| {
        assert!(cmd.status().unwrap().success(), "fixture command {cmd:?}");
    };
    ok(std::process::Command::new("xattr")
        .args(["-w", "org.lumina.test", "kept"])
        .arg(&item.path));
    ok(std::process::Command::new("touch")
        .args(["-t", "201001021530"])
        .arg(&item.path));
    std::fs::set_permissions(&item.path, std::fs::Permissions::from_mode(0o640)).unwrap();
    let source_mtime = std::fs::metadata(&item.path).unwrap().modified().unwrap();

    let destination = tmp_dir.join("out");
    let report = engine
        .export_items_with(
            &[item.id],
            &destination,
            ExportPreset::Originals,
            &std::sync::atomic::AtomicBool::new(false),
        )
        .unwrap();
    assert_eq!(report.exported, 1, "failures: {:?}", report.failed);
    let copy = destination.join("kept.jpg");
    let meta = std::fs::metadata(&copy).unwrap();
    assert_eq!(
        meta.permissions().mode() & 0o777,
        0o640,
        "an exported original keeps its mode"
    );
    assert_eq!(
        meta.modified().unwrap(),
        source_mtime,
        "an exported original keeps its modification date"
    );
    let xattr = std::process::Command::new("xattr")
        .args(["-p", "org.lumina.test"])
        .arg(&copy)
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&xattr.stdout).trim(),
        "kept",
        "an exported original keeps its extended attributes (Finder tags)"
    );

    drop(engine);
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

/// An old catalog that stored a portrait photo's size unrotated is fixed by
/// the next open's scan, even though the unchanged file is not re-scanned.
#[test]
fn reopening_an_old_catalog_fixes_portrait_dims_once() {
    let orientation_6 = exif::Field {
        tag: exif::Tag::Orientation,
        ifd_num: exif::In::PRIMARY,
        value: exif::Value::Short(vec![6]),
    };
    let jpeg = with_exif(&red_jpeg(16, 8), &[orientation_6]);
    let (tmp_dir, engine, _rx) = library_with("dims_backfill", &[("portrait.jpg", jpeg)]);
    let id = item_named(&engine, "portrait.jpg").id;
    let size = |engine: &LuminaEngine| {
        let item = engine.catalog().get_item_by_id(id).unwrap().unwrap();
        (item.width, item.height)
    };
    assert_eq!(
        size(&engine),
        (8, 16),
        "a new scan stores the displayed size"
    );
    assert_eq!(
        engine
            .catalog()
            .get_config("dims_oriented_v1")
            .unwrap()
            .as_deref(),
        Some("1"),
        "a new catalog needs no backfill"
    );
    let library = engine.library_root().to_path_buf();
    // The startup scan's own backfill must not see the seeded catalog, so
    // the fix below can only come from the reopen.
    engine.cancel_scan();
    drop(engine);

    // What an older Lumina left behind: the stored size and no marker.
    let db_path = tmp_dir.join("catalog.db");
    raw_catalog(&db_path)
        .execute_batch(
            "UPDATE media_items SET width = 16, height = 8;
             DELETE FROM app_config WHERE key = 'dims_oriented_v1';",
        )
        .unwrap();
    let (engine, rx) =
        LuminaEngine::open_with_paths(library, db_path, Some(tmp_dir.join("cache"))).unwrap();
    // The scan's ScanComplete, then the backfill's once it changed a row.
    let completes = std::iter::from_fn(|| rx.recv_timeout(std::time::Duration::from_secs(30)).ok())
        .filter(|event| matches!(event, EngineEvent::ScanComplete { .. }))
        .take(2)
        .count();
    assert_eq!(
        completes, 2,
        "a backfill that changed sizes sends a second ScanComplete"
    );
    assert_eq!(size(&engine), (8, 16), "the backfill fixes the stored size");
    assert_eq!(
        engine
            .catalog()
            .get_config("dims_oriented_v1")
            .unwrap()
            .as_deref(),
        Some("1"),
        "the backfill marks itself done"
    );
    drop(engine);
    let _ = std::fs::remove_dir_all(&tmp_dir);
}
