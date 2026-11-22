//! Catalog integration tests: rows, timeline, schema migrations, trash,
//! date and place edits, sizes, duplicates, albums and views.

mod common;

use common::*;
use lumina_core::{
    Catalog, DecodedFrame, DeletionTarget, DiskCache, LuminaEngine, MediaId, MediaItem, MediaType,
    OfflineGeocoder, PixelFormat, Scanner, SortDirection, SortField, SortSpec, StructuredQuery,
    SystemAlbum, SCHEMA_VERSION,
};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

#[test]
fn test_catalog_in_memory() {
    let catalog = Catalog::open_in_memory().expect("open in memory catalog");
    assert_eq!(catalog.total_count().unwrap(), 0);

    let item1 = sample_media_item(
        "/test/photo1.jpg",
        MediaType::Image,
        1000,
        Some("Kyoto"),
        Some("ILCE-7M4"),
    );
    let item2 = sample_media_item(
        "/test/video1.mp4",
        MediaType::Video,
        2000,
        Some("Kyoto"),
        Some("ILCE-7M4"),
    );

    let inserted = catalog
        .insert_or_update_batch(&[item1.clone(), item2.clone()])
        .expect("insert batch");
    assert_eq!(inserted.len(), 2);
    assert_eq!(catalog.total_count().unwrap(), 2);

    let slice = catalog.get_item_slice(0, 10).expect("get item slice");
    assert_eq!(slice.len(), 2);
    // Ordered by timestamp desc: video1 (2000) should be first
    assert_eq!(slice[0].path, item2.path);
    assert_eq!(slice[1].path, item1.path);

    // Test config key-value store
    catalog
        .set_config("last_library", "/path/to/library")
        .unwrap();
    assert_eq!(
        catalog.get_config("last_library").unwrap(),
        Some("/path/to/library".to_string())
    );

    // Test removing items
    let removed = catalog
        .permanently_delete_items(&[inserted[0].id], DeletionTarget::CatalogOnly)
        .unwrap();
    assert_eq!(removed.deleted_count(), 1);
    assert_eq!(catalog.total_count().unwrap(), 1);
}

#[test]
fn test_timeline_buckets_aggregation() {
    let catalog = Catalog::open_in_memory().expect("open catalog");

    let mut item1 = sample_media_item(
        "/test/2026/oct/img1.jpg",
        MediaType::Image,
        1791633600000,
        None,
        None,
    );
    item1.year = 2026;
    item1.month = 10;
    item1.day = 10;

    let mut item2 = sample_media_item(
        "/test/2026/oct/img2.jpg",
        MediaType::Image,
        1791633700000,
        None,
        None,
    );
    item2.year = 2026;
    item2.month = 10;
    item2.day = 10;

    let mut item3 = sample_media_item(
        "/test/2025/dec/img3.jpg",
        MediaType::Image,
        1765800000000,
        None,
        None,
    );
    item3.year = 2025;
    item3.month = 12;
    item3.day = 15;

    catalog
        .insert_or_update_batch(&[item1, item2, item3])
        .unwrap();
    catalog.compute_timeline_buckets().unwrap();

    let year_buckets = catalog.get_timeline_buckets("year").unwrap();
    assert_eq!(year_buckets.len(), 2);
    assert_eq!(year_buckets[0].bucket_key, "2026");
    assert_eq!(year_buckets[0].item_count, 2);
    assert_eq!(year_buckets[1].bucket_key, "2025");
    assert_eq!(year_buckets[1].item_count, 1);
}

#[test]
fn test_offline_geocoder_nearest_lookup() {
    let geocoder = OfflineGeocoder::new();

    // Tokyo: 35.6895, 139.6917
    let tokyo = geocoder
        .reverse_geocode(35.6895, 139.6917)
        .expect("geocode tokyo");
    assert_eq!(tokyo.city, "Tokyo");
    assert_eq!(tokyo.country_code, "JP");

    // Paris: 48.8566, 2.3522
    let paris = geocoder
        .reverse_geocode(48.8566, 2.3522)
        .expect("geocode paris");
    assert_eq!(paris.city, "Paris");
    assert_eq!(paris.country_code, "FR");

    // San Francisco: 37.7749, -122.4194
    let sf = geocoder
        .reverse_geocode(37.7749, -122.4194)
        .expect("geocode sf");
    assert_eq!(sf.city, "San Francisco");
    assert_eq!(sf.country_code, "US");

    // Microsecond latency benchmark test: 1,000 lookups
    let start = Instant::now();
    for _ in 0..1000 {
        let _ = geocoder.reverse_geocode(35.0116, 135.7681); // Kyoto
    }
    let elapsed = start.elapsed();
    let per_lookup_micros = elapsed.as_micros() as f64 / 1000.0;
    println!(
        "Reverse geocode latency: {:.2} µs/lookup",
        per_lookup_micros
    );
    // Debug builds under parallel test load are noisy; keep a meaningful but
    // environment-tolerant budget instead of flaking the suite.
    let budget_micros = if cfg!(debug_assertions) { 500.0 } else { 20.0 };
    assert!(
        per_lookup_micros < budget_micros,
        "Reverse geocoding must resolve in < {budget_micros}µs (got {per_lookup_micros:.2}µs)"
    );
}

#[test]
fn test_disk_cache() {
    let temp_dir = unique_temp_dir("test_cache");
    let cache = DiskCache::with_directory(&temp_dir).expect("create test cache");
    cache.clear().expect("clear test cache");

    let test_path = PathBuf::from("/test/photo.jpg");
    let frame = DecodedFrame {
        id: MediaId(42),
        width: 4,
        height: 4,
        format: PixelFormat::Rgba8,
        data: vec![255; 4 * 4 * 4],
    };

    cache
        .put_cached(&test_path, 1024, 12345, 256, &frame)
        .expect("put cache");

    let retrieved = cache
        .get_cached(MediaId(42), &test_path, 1024, 12345, 256)
        .expect("get cached");

    assert_eq!(retrieved.width, 4);
    assert_eq!(retrieved.height, 4);
    assert_eq!(retrieved.format, PixelFormat::Rgba8);
    assert_eq!(retrieved.data.len(), 4 * 4 * 4);

    let _ = cache.clear();
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_scanner_on_temp_dir() {
    let temp_dir = unique_temp_dir("test_scanner");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let mut f1 = File::create(temp_dir.join("sample.jpg")).unwrap();
    f1.write_all(b"fake image data").unwrap();

    let mut f2 = File::create(temp_dir.join("clip.mp4")).unwrap();
    f2.write_all(b"fake video data").unwrap();

    let mut f3 = File::create(temp_dir.join("ignore.txt")).unwrap();
    f3.write_all(b"text file").unwrap();

    let (tx, rx) = crossbeam_channel::unbounded();
    let scanner = Scanner::new(temp_dir.clone(), tx);
    let count = scanner.scan().expect("scan");
    assert_eq!(count, 2);

    let mut batch_count = 0;
    let mut progress = Vec::new();
    while let Ok(evt) = rx.try_recv() {
        match evt {
            lumina_core::EngineEvent::BatchDiscovered { items } => batch_count += items.len(),
            lumina_core::EngineEvent::ScanProgress {
                items_found,
                is_complete,
                total,
                skipped,
                ..
            } => progress.push((items_found, is_complete, total, skipped)),
            _ => {}
        }
    }
    assert_eq!(batch_count, 2);
    assert_eq!(
        progress.first(),
        Some(&(0, false, 2, 0)),
        "the pre-count reports the total before any file is read (the .txt is not media)"
    );
    assert_eq!(progress.last(), Some(&(2, true, 2, 0)));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_deletion_lifecycle_and_trash_restoration() {
    let catalog = Catalog::open_in_memory().expect("open catalog");

    let tmp_dir = unique_temp_dir("del_test");

    let file_path1 = tmp_dir.join("photo_del_1.jpg");
    let file_path2 = tmp_dir.join("photo_del_2.jpg");
    let file_path3 = tmp_dir.join("photo_del_3.jpg");

    std::fs::write(&file_path1, b"dummy photo 1").unwrap();
    std::fs::write(&file_path2, b"dummy photo 2").unwrap();
    std::fs::write(&file_path3, b"dummy photo 3").unwrap();

    let item1 = sample_media_item(
        file_path1.to_str().unwrap(),
        MediaType::Image,
        1000,
        None,
        None,
    );
    let item2 = sample_media_item(
        file_path2.to_str().unwrap(),
        MediaType::Image,
        2000,
        None,
        None,
    );
    let item3 = sample_media_item(
        file_path3.to_str().unwrap(),
        MediaType::Image,
        3000,
        None,
        None,
    );

    let inserted = catalog
        .insert_or_update_batch(&[item1, item2, item3])
        .unwrap();
    let id1 = inserted[0].id;
    let id2 = inserted[1].id;
    let id3 = inserted[2].id;

    // Add tags and embeddings to verify cascade deletion
    catalog
        .add_tags_batch(&[id1, id2, id3], &["scenery".to_string()])
        .unwrap();
    assert_eq!(catalog.get_tags_for_item(id1).unwrap().len(), 1);

    // 1. Move id1 and id2 to Trash
    catalog.batch_set_trashed(&[id1, id2], true).unwrap();
    let trashed = catalog
        .query_system_album(SystemAlbum::RecentlyDeleted, 10, 0)
        .unwrap();
    assert_eq!(trashed.len(), 2);
    assert_eq!(
        catalog
            .query_system_album_count(SystemAlbum::RecentlyDeleted)
            .unwrap(),
        2
    );

    // 2. Restore id1 specifically
    let restored_count = catalog.restore_trashed_items(&[id1]).unwrap();
    assert_eq!(restored_count, 1);
    let trashed_after_restore = catalog
        .query_system_album(SystemAlbum::RecentlyDeleted, 10, 0)
        .unwrap();
    assert_eq!(trashed_after_restore.len(), 1);
    assert_eq!(trashed_after_restore[0].id, id2);

    // 3. Re-trash id1 and test restore_all_trash
    catalog.set_trashed(id1, true).unwrap();
    assert_eq!(
        catalog
            .query_system_album_count(SystemAlbum::RecentlyDeleted)
            .unwrap(),
        2
    );
    let all_restored = catalog.restore_all_trash().unwrap();
    assert_eq!(all_restored, 2);
    assert_eq!(
        catalog
            .query_system_album_count(SystemAlbum::RecentlyDeleted)
            .unwrap(),
        0
    );

    // 4. Test PermanentDisk deletion on id3; its sidecar goes with it
    assert!(file_path3.exists());
    let sidecar3 = lumina_core::sidecar_path(&file_path3);
    std::fs::write(&sidecar3, b"<x/>").unwrap();
    let sidecar1 = lumina_core::sidecar_path(&file_path1);
    std::fs::write(&sidecar1, b"<x/>").unwrap();
    let deleted_count = catalog
        .permanently_delete_items(&[id3], DeletionTarget::PermanentDisk)
        .unwrap();
    assert_eq!(deleted_count.deleted_count(), 1);
    assert!(!file_path3.exists());
    assert!(
        !sidecar3.exists(),
        "deleting media from disk must not leave its (hidden) sidecar behind"
    );
    assert!(catalog.get_item_by_id(id3).unwrap().is_none());
    assert_eq!(catalog.get_tags_for_item(id3).unwrap().len(), 0);

    // 5. Move id1 and id2 to trash, then empty_trash with CatalogOnly
    catalog.batch_set_trashed(&[id1, id2], true).unwrap();
    assert!(file_path1.exists());
    assert!(file_path2.exists());
    let emptied = catalog.empty_trash(DeletionTarget::CatalogOnly).unwrap();
    assert_eq!(emptied.deleted_count(), 2);
    assert!(
        sidecar1.exists(),
        "catalog-only removal leaves the file and its sidecar alone"
    );
    assert_eq!(
        catalog
            .query_system_album_count(SystemAlbum::RecentlyDeleted)
            .unwrap(),
        0
    );
    // Files remain untouched on disk under CatalogOnly
    assert!(file_path1.exists());
    assert!(file_path2.exists());
    assert_eq!(catalog.total_count().unwrap(), 0);

    // Clean up temporary files
    let _ = std::fs::remove_dir_all(tmp_dir);
}

#[test]
fn test_extract_video_duration_synthetic() {
    let tmp_dir = unique_temp_dir("test_duration");
    let video_path = tmp_dir.join("test.mp4");

    // Write a minimal MP4 container with mvhd (version 0, timescale 1000, duration 45200 ms)
    let mut file = File::create(&video_path).unwrap();
    let mut data = Vec::new();
    // ftyp box
    data.extend_from_slice(&[0, 0, 0, 16]); // size 16
    data.extend_from_slice(b"ftypmp42");
    data.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0]);

    // moov box containing mvhd (116 bytes total)
    data.extend_from_slice(&[0, 0, 0, 116]);
    data.extend_from_slice(b"moov");

    // mvhd box
    data.extend_from_slice(&[0, 0, 0, 108]);
    data.extend_from_slice(b"mvhd");
    data.push(0); // version 0
    data.extend_from_slice(&[0, 0, 0]); // flags
    data.extend_from_slice(&[0, 0, 0, 0]); // creation time
    data.extend_from_slice(&[0, 0, 0, 0]); // modification time
    data.extend_from_slice(&1000u32.to_be_bytes()); // timescale: 1000
    data.extend_from_slice(&45200u32.to_be_bytes()); // duration: 45200 (45.2s)
    data.resize(data.len() + (108 - 28), 0);

    file.write_all(&data).unwrap();
    drop(file);

    let res = lumina_core::extract_video_duration(&video_path);
    assert!(res.is_some(), "Expected video duration to be extracted");
    let (ms, secs) = res.unwrap();
    assert_eq!(ms, 45200);
    assert!((secs - 45.2).abs() < 0.001);

    // Also test non-faststart MP4 where moov/mvhd is placed at the tail after >128 KiB mdat
    let tail_video_path = tmp_dir.join("tail_test.mp4");
    let mut tail_file = File::create(&tail_video_path).unwrap();
    let mut tail_data = vec![0u8; 200 * 1024]; // 200 KiB dummy mdat
    tail_data.extend_from_slice(&data);
    tail_file.write_all(&tail_data).unwrap();
    drop(tail_file);

    let tail_res = lumina_core::extract_video_duration(&tail_video_path);
    assert!(
        tail_res.is_some(),
        "Expected tail moov duration to be extracted"
    );
    assert_eq!(tail_res.unwrap().0, 45200);

    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_clear_all_resets_catalog_and_fts() {
    let catalog = Catalog::open_in_memory().expect("open catalog");
    let item = sample_media_item(
        "/photos/reset/one.jpg",
        MediaType::Image,
        1000,
        Some("Kyoto"),
        Some("A7IV"),
    );
    let inserted = catalog.insert_or_update_batch(&[item]).unwrap();
    let id = inserted[0].id;
    catalog
        .add_tags_batch(&[id], &["scenery".to_string()])
        .unwrap();
    catalog.set_caption(id, "unique hedgehog at dusk").unwrap();

    assert_eq!(catalog.total_count().unwrap(), 1);
    assert_eq!(
        catalog
            .search_structured(&StructuredQuery::parse("hedgehog"), 10, 0)
            .unwrap()
            .len(),
        1
    );

    catalog
        .clear_all()
        .expect("clear_all must handle contentless FTS5");

    assert_eq!(catalog.total_count().unwrap(), 0);
    assert!(catalog
        .search_structured(&StructuredQuery::parse("hedgehog"), 10, 0)
        .unwrap()
        .is_empty());
    assert!(catalog.get_captions_map().unwrap().is_empty());
    assert!(catalog.get_tags_for_item(id).unwrap().is_empty());

    // The FTS index must remain usable after delete-all.
    let item2 = sample_media_item("/photos/reset/two.jpg", MediaType::Image, 2000, None, None);
    let inserted2 = catalog.insert_or_update_batch(&[item2]).unwrap();
    catalog
        .set_caption(inserted2[0].id, "second entry")
        .unwrap();
    assert!(catalog
        .search_structured(&StructuredQuery::parse("hedgehog"), 10, 0)
        .unwrap()
        .is_empty());
    assert_eq!(
        catalog
            .search_structured(&StructuredQuery::parse("second"), 10, 0)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn test_catalog_schema_version_integrity_and_backup() {
    let tmp_dir = unique_temp_dir("catalog_hardening");
    let db_path = tmp_dir.join("catalog.db");

    let catalog = Catalog::open(&db_path).expect("open catalog");
    assert_eq!(
        catalog.schema_version().unwrap(),
        SCHEMA_VERSION,
        "schema version must be stamped on open"
    );
    catalog
        .quick_check()
        .expect("fresh catalog should be healthy");

    let item = sample_media_item(
        "/photos/hardening/one.jpg",
        MediaType::Image,
        1000,
        None,
        None,
    );
    catalog.insert_or_update_batch(&[item]).unwrap();

    let backup = catalog
        .daily_backup()
        .expect("backup should succeed")
        .expect("first backup should be created");
    assert!(backup.exists());
    assert!(backup.metadata().unwrap().len() > 0);
    assert!(
        catalog.daily_backup().unwrap().is_none(),
        "an up-to-date backup must not be rewritten"
    );

    drop(catalog);
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_edit_timestamp_rebuilds_buckets_and_location() {
    let tmp_dir = unique_temp_dir("edit_meta");
    let catalog = Catalog::open(tmp_dir.join("catalog.db")).unwrap();
    let item = sample_media_item(
        "/photos/edit/meta.jpg",
        MediaType::Image,
        1_600_000_000_000,
        None,
        None,
    );
    let inserted = catalog.insert_or_update_batch(&[item]).unwrap();
    let id = inserted[0].id;

    let epoch_ms = 1_700_000_000_000; // 2023-11-14T22:13:20Z
    catalog.set_timestamp(id, epoch_ms).unwrap();
    let updated = catalog.get_item_by_id(id).unwrap().unwrap();
    assert_eq!(updated.timestamp_epoch_ms, epoch_ms);
    assert_eq!(updated.year, 2023);
    assert_eq!(updated.month, 11);
    let years = catalog.get_timeline_buckets("year").unwrap();
    assert!(
        years.iter().any(|bucket| bucket.bucket_key == "2023"),
        "timeline buckets must reflect the edited date: {years:?}"
    );

    catalog
        .set_location(id, Some(48.8566), Some(2.3522))
        .unwrap();
    let located = catalog.get_item_by_id(id).unwrap().unwrap();
    assert_eq!(located.latitude, Some(48.8566));
    assert_eq!(located.longitude, Some(2.3522));
    assert_eq!(
        located.city.as_deref(),
        Some("Paris"),
        "new coordinates must re-derive the place"
    );
    assert_eq!(
        catalog
            .search_structured(&StructuredQuery::parse("Paris"), 10, 0)
            .unwrap()
            .len(),
        1,
        "the derived place is searchable"
    );

    catalog.set_location(id, None, None).unwrap();
    let cleared = catalog.get_item_by_id(id).unwrap().unwrap();
    assert_eq!(cleared.latitude, None);
    assert_eq!(cleared.longitude, None);
    assert_eq!(
        (cleared.city, cleared.admin1_region, cleared.country_code),
        (None, None, None),
        "removing the location must clear the place too"
    );

    drop(catalog);
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn date_shift_crossing_month_rebuilds_buckets() {
    let catalog = Catalog::open_in_memory().expect("open catalog");
    // 2026-10-31 at 20:00, 22:00 and 23:00 UTC.
    let times = [1_793_476_800_000_i64, 1_793_484_000_000, 1_793_487_600_000];
    let items: Vec<MediaItem> = times
        .iter()
        .enumerate()
        .map(|(i, &ts)| {
            let mut item = sample_media_item(
                &format!("/photos/shift/{i}.jpg"),
                MediaType::Image,
                ts,
                None,
                None,
            );
            item.day = 31;
            item
        })
        .collect();
    let inserted = catalog.insert_or_update_batch(&items).unwrap();
    catalog.compute_timeline_buckets().unwrap();
    let months = |catalog: &Catalog| -> Vec<(String, usize)> {
        let mut months: Vec<_> = catalog
            .get_timeline_buckets("month")
            .unwrap()
            .into_iter()
            .map(|bucket| (bucket.bucket_key, bucket.item_count))
            .collect();
        months.sort();
        months
    };
    assert_eq!(months(&catalog), vec![("2026-10".to_string(), 3)]);

    // A +3h shift, as the inspector writes it: one set_timestamps call.
    let delta = 3 * 3_600_000;
    let shifted: Vec<(MediaId, i64)> = inserted
        .iter()
        .map(|item| (item.id, item.timestamp_epoch_ms + delta))
        .collect();
    catalog.set_timestamps(&shifted).unwrap();
    assert_eq!(
        months(&catalog),
        vec![("2026-10".to_string(), 1), ("2026-11".to_string(), 2)],
        "a shift across a month boundary must move items between month buckets"
    );
}

#[test]
fn test_added_date_is_stamped_once_and_survives_rescans() {
    let catalog = Catalog::open_in_memory().unwrap();
    let item = sample_media_item("/photos/added/a.jpg", MediaType::Image, 1000, None, None);
    let now_ms = || {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
    };
    let before = now_ms();
    let id = catalog
        .insert_or_update_batch(std::slice::from_ref(&item))
        .unwrap()[0]
        .id;
    let added = catalog
        .get_added_epoch_ms(id)
        .unwrap()
        .expect("a new row must record when it was added");
    assert!(
        (added - before).abs() < 5_000,
        "added date must be now, got {added} vs {before}"
    );

    // Let the clock move on so a re-stamp would show as a different value.
    while now_ms() <= added {
        std::hint::spin_loop();
    }
    let rescanned = catalog.insert_or_update_batch(&[item]).unwrap()[0].id;
    assert_eq!(rescanned, id);
    assert_eq!(
        catalog.get_added_epoch_ms(id).unwrap(),
        Some(added),
        "a rescan must not move the added date"
    );
}

#[test]
fn test_schema_v9_migration_adds_added_date_column() {
    let tmp_dir = unique_temp_dir("added_migration");
    let db_path = tmp_dir.join("catalog.db");
    let id = {
        let catalog = Catalog::open(&db_path).unwrap();
        let item = sample_media_item("/photos/old/b.jpg", MediaType::Image, 1000, None, None);
        catalog.insert_or_update_batch(&[item]).unwrap()[0].id
    };
    {
        // Downgrade to the v8 layout: no added_epoch_ms column.
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "ALTER TABLE media_items DROP COLUMN added_epoch_ms; PRAGMA user_version = 8;",
        )
        .unwrap();
    }
    let catalog = Catalog::open(&db_path).expect("a v8 catalog must open and migrate");
    assert_eq!(
        catalog.get_added_epoch_ms(id).unwrap(),
        None,
        "rows from before v9 have no known added date"
    );
    drop(catalog);
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);
    drop(conn);
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_read_media_details_reads_exif_and_tolerates_plain_files() {
    use exif::experimental::Writer;
    use exif::{Field, In, Rational, SRational, Tag, Value};

    let tmp_dir = unique_temp_dir("details");

    let fields = [
        Field {
            tag: Tag::ExposureBiasValue,
            ifd_num: In::PRIMARY,
            value: Value::SRational(vec![SRational { num: -2, denom: 3 }]),
        },
        Field {
            tag: Tag::FocalLengthIn35mmFilm,
            ifd_num: In::PRIMARY,
            value: Value::Short(vec![28]),
        },
        Field {
            tag: Tag::WhiteBalance,
            ifd_num: In::PRIMARY,
            value: Value::Short(vec![0]),
        },
        Field {
            tag: Tag::Software,
            ifd_num: In::PRIMARY,
            value: Value::Ascii(vec![b"Lumina Test 1.0".to_vec()]),
        },
        Field {
            tag: Tag::FNumber,
            ifd_num: In::PRIMARY,
            value: Value::Rational(vec![Rational { num: 28, denom: 10 }]),
        },
    ];
    let mut writer = Writer::new();
    for field in &fields {
        writer.push_field(field);
    }
    let mut tiff = std::io::Cursor::new(Vec::new());
    writer.write(&mut tiff, false).unwrap();
    let tiff = tiff.into_inner();

    // Minimal JPEG container: SOI, APP1 "Exif\0\0" + TIFF, EOI.
    let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE1];
    jpeg.extend_from_slice(&((2 + 6 + tiff.len()) as u16).to_be_bytes());
    jpeg.extend_from_slice(b"Exif\0\0");
    jpeg.extend_from_slice(&tiff);
    jpeg.extend_from_slice(&[0xFF, 0xD9]);
    let photo = tmp_dir.join("exif.jpg");
    std::fs::write(&photo, jpeg).unwrap();

    let details = lumina_core::scanner::read_media_details(&photo);
    let bias = details
        .exposure_bias_ev
        .expect("exposure bias must be read");
    assert!((bias + 2.0 / 3.0).abs() < 1e-6, "got {bias}");
    assert_eq!(details.focal_length_35mm, Some(28));
    assert_eq!(details.white_balance.as_deref(), Some("Auto white balance"));
    assert_eq!(details.software.as_deref(), Some("Lumina Test 1.0"));
    assert_eq!(details.metering_mode, None, "absent tags stay None");
    #[cfg(target_os = "macos")]
    assert!(
        details.created_epoch_ms.is_some(),
        "APFS records a birth time for Created"
    );

    let plain = tmp_dir.join("plain.jpg");
    std::fs::write(&plain, b"not really a jpeg").unwrap();
    let details = lumina_core::scanner::read_media_details(&plain);
    assert_eq!(details.exposure_bias_ev, None);
    assert_eq!(details.software, None);
    let missing = lumina_core::scanner::read_media_details(&tmp_dir.join("gone.jpg"));
    assert_eq!(
        missing,
        Default::default(),
        "a missing file yields no details"
    );

    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn capture_time_reads_weekday_day_month_year_and_time() {
    // 2022-06-13 10:03 UTC
    let item = sample_media_item("/a/b.jpg", MediaType::Image, 1_655_114_580_000, None, None);
    assert_eq!(
        item.formatted_timestamp(),
        "Mon 13 Jun 2022 · 10:03",
        "inspector capture time reads weekday, day, month, year · time"
    );
}

/// A minimal `tkhd` box (version 0) with the given matrix `a`/`b` and size.
fn tkhd_box(a: i32, b: i32, width: u32, height: u32) -> Vec<u8> {
    let mut boxed = Vec::new();
    boxed.extend_from_slice(&92u32.to_be_bytes());
    boxed.extend_from_slice(b"tkhd");
    boxed.extend_from_slice(&[0; 4]); // version 0 + flags
    boxed.extend_from_slice(&[0; 20]); // times, track id, reserved, duration
    boxed.extend_from_slice(&[0; 16]); // reserved, layer, group, volume
    let mut matrix = [0i32; 9];
    (matrix[0], matrix[1]) = (a, b);
    for v in matrix {
        boxed.extend_from_slice(&v.to_be_bytes());
    }
    boxed.extend_from_slice(&(width << 16).to_be_bytes());
    boxed.extend_from_slice(&(height << 16).to_be_bytes());
    boxed
}

#[test]
fn video_size_comes_from_the_first_visual_track_and_honours_rotation() {
    let dir = unique_temp_dir("test_vsize");
    let write = |name: &str, tracks: &[Vec<u8>]| {
        let path = dir.join(name);
        std::fs::write(&path, tracks.concat()).unwrap();
        path
    };
    const ONE: i32 = 0x0001_0000;
    // An audio track (0×0) first, then the 1920×1080 picture.
    let landscape = write(
        "landscape.mp4",
        &[tkhd_box(ONE, 0, 0, 0), tkhd_box(ONE, 0, 1920, 1080)],
    );
    // A phone portrait clip: stored 1920×1080, rotated 90°.
    let portrait = write("portrait.mp4", &[tkhd_box(0, ONE, 1920, 1080)]);
    let audio_only = write("audio.m4v", &[tkhd_box(ONE, 0, 0, 0)]);

    assert_eq!(
        lumina_core::extract_video_size(&landscape),
        Some((1920, 1080)),
        "audio tracks have no size; the picture track gives it"
    );
    assert_eq!(
        lumina_core::extract_video_size(&portrait),
        Some((1080, 1920)),
        "a 90° matrix must swap width and height"
    );
    assert_eq!(lumina_core::extract_video_size(&audio_only), None);

    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../lumina-ui-gpui/tests/fixtures/clip.mp4");
    assert_eq!(
        lumina_core::extract_video_size(&fixture),
        Some((64, 48)),
        "a real encoder's MP4 must parse too"
    );
    let (tx, _rx) = crossbeam_channel::unbounded();
    let scanned = Scanner::new(dir.clone(), tx)
        .inspect_media_file(&fixture)
        .expect("the scanner inspects the clip");
    assert_eq!(
        (scanned.width, scanned.height),
        (64, 48),
        "a scanned video is catalogued with its size"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn video_size_backfill_sets_only_the_size_of_sizeless_videos() {
    let catalog = Catalog::open_in_memory().expect("open catalog");
    let video = sample_media_item("/v/old.mp4", MediaType::Video, 1789430400000, None, None);
    let photo = sample_media_item("/p/a.jpg", MediaType::Image, 1789430400000, None, None);
    let mkv = sample_media_item("/v/clip.mkv", MediaType::Video, 1789430400000, None, None);
    let (video, photo) = (
        MediaItem {
            width: 0,
            height: 0,
            ..video
        },
        MediaItem {
            width: 0,
            height: 0,
            ..photo
        },
    );
    let mkv = MediaItem {
        width: 0,
        height: 0,
        ..mkv
    };
    let stored = catalog
        .insert_or_update_batch(&[video, photo, mkv])
        .unwrap();
    let (vid, pid) = (stored[0].id, stored[1].id);
    let edited = 1600000000000;
    catalog.set_timestamp(vid, edited).unwrap();

    let filled = catalog.fill_video_sizes(|_| Some((1920, 1080))).unwrap();
    assert_eq!(
        filled, 1,
        "only the MP4 is measured: photos and MKVs have no tkhd to read"
    );
    let v = catalog.get_item_by_id(vid).unwrap().unwrap();
    assert_eq!((v.width, v.height), (1920, 1080));
    assert_eq!(
        v.timestamp_epoch_ms, edited,
        "the backfill must not overwrite an edited date"
    );
    let p = catalog.get_item_by_id(pid).unwrap().unwrap();
    assert_eq!((p.width, p.height), (0, 0), "photos are never touched");

    assert_eq!(
        catalog
            .fill_video_sizes(|_| panic!("sized videos are not re-measured"))
            .unwrap(),
        0
    );
}

/// Photos with a size, as the scanner stored them before it applied EXIF
/// orientation: a portrait phone JPEG stored landscape, a HEIC it could not
/// size, a landscape photo, a photo whose file is unreadable, and a video.
fn dims_backfill_catalog() -> (Catalog, [MediaId; 5]) {
    let catalog = Catalog::open_in_memory().expect("open catalog");
    let at = 1789430400000;
    let sized = |path, media_type, size: (u32, u32)| MediaItem {
        width: size.0,
        height: size.1,
        ..sample_media_item(path, media_type, at, None, None)
    };
    let stored = catalog
        .insert_or_update_batch(&[
            sized("/p/portrait.jpg", MediaType::Image, (4032, 3024)),
            sized("/p/pano.heic", MediaType::Image, (0, 0)),
            sized("/p/landscape.jpg", MediaType::Image, (1920, 1080)),
            sized("/p/gone.jpg", MediaType::Image, (800, 600)),
            sized("/v/clip.mp4", MediaType::Video, (1920, 1080)),
        ])
        .expect("insert rows");
    let ids = [0, 1, 2, 3, 4].map(|i| stored[i].id);
    (catalog, ids)
}

/// What the files say now: displayed (EXIF-oriented) sizes.
fn measured_dims(path: &std::path::Path) -> Option<(u32, u32)> {
    match path.file_name()?.to_str()? {
        "portrait.jpg" => Some((3024, 4032)),
        "pano.heic" => Some((1000, 3000)),
        "landscape.jpg" => Some((1920, 1080)),
        "gone.jpg" => None,
        other => panic!("only photos are measured, not {other}"),
    }
}

fn dims_of(catalog: &Catalog, id: MediaId) -> (u32, u32) {
    let item = catalog.get_item_by_id(id).unwrap().expect("row exists");
    (item.width, item.height)
}

#[test]
fn backfill_runs_once_and_fixes_portrait_dims() {
    let (catalog, [portrait, heic, landscape, gone, video]) = dims_backfill_catalog();
    let edited = 1600000000000;
    catalog.set_timestamp(portrait, edited).unwrap();
    let cancel = std::sync::atomic::AtomicBool::new(false);

    let fixed = catalog
        .backfill_oriented_dims(measured_dims, &cancel)
        .expect("backfill runs");
    assert_eq!(fixed, 2, "only the portrait and the unsized HEIC change");
    assert_eq!(
        dims_of(&catalog, portrait),
        (3024, 4032),
        "a portrait stored landscape gets its displayed size"
    );
    assert_eq!(
        dims_of(&catalog, heic),
        (1000, 3000),
        "an unsized HEIC is sized"
    );
    assert_eq!(
        dims_of(&catalog, landscape),
        (1920, 1080),
        "a correct row is untouched"
    );
    assert_eq!(
        dims_of(&catalog, gone),
        (800, 600),
        "an unreadable file keeps its stored size, never zeroed"
    );
    assert_eq!(
        dims_of(&catalog, video),
        (1920, 1080),
        "videos are not touched"
    );
    assert_eq!(
        catalog
            .get_item_by_id(portrait)
            .unwrap()
            .unwrap()
            .timestamp_epoch_ms,
        edited,
        "the backfill writes the size only, never an edited date"
    );
    assert_eq!(
        catalog.get_config("dims_oriented_v1").unwrap().as_deref(),
        Some("1"),
        "a finished backfill sets its marker"
    );

    let again = catalog
        .backfill_oriented_dims(|_| panic!("the marker stops a second run"), &cancel)
        .expect("second run is a no-op");
    assert_eq!(again, 0);
}

#[test]
fn cancelled_dims_backfill_leaves_no_marker() {
    let (catalog, [portrait, ..]) = dims_backfill_catalog();
    let cancel = std::sync::atomic::AtomicBool::new(true);
    let fixed = catalog
        .backfill_oriented_dims(measured_dims, &cancel)
        .expect("a cancelled backfill is not an error");
    assert_eq!(fixed, 0);
    assert_eq!(dims_of(&catalog, portrait), (4032, 3024));
    assert_eq!(
        catalog.get_config("dims_oriented_v1").unwrap(),
        None,
        "a cancelled backfill runs again on the next scan"
    );
}

#[test]
fn dims_backfill_never_overwrites_a_concurrent_rescan() {
    let (catalog, [portrait, heic, ..]) = dims_backfill_catalog();
    let rescanned = MediaItem {
        width: 500,
        height: 500,
        ..sample_media_item(
            "/p/portrait.jpg",
            MediaType::Image,
            1789430400000,
            None,
            None,
        )
    };
    let fixed = catalog
        .backfill_oriented_dims(
            |path| {
                if path.ends_with("portrait.jpg") {
                    // The file changed and a rescan stored it mid-backfill.
                    catalog
                        .insert_or_update_batch(std::slice::from_ref(&rescanned))
                        .expect("concurrent rescan writes");
                }
                measured_dims(path)
            },
            &std::sync::atomic::AtomicBool::new(false),
        )
        .unwrap();
    assert_eq!(
        dims_of(&catalog, portrait),
        (500, 500),
        "the backfill's stale measurement must not overwrite the rescan"
    );
    assert_eq!(dims_of(&catalog, heic), (1000, 3000));
    assert_eq!(fixed, 1, "only the HEIC write lands");
}

#[test]
fn dims_backfill_cancel_stops_mid_batch_and_runs_again_later() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let (catalog, _) = dims_backfill_catalog();
    let cancel = AtomicBool::new(false);
    let calls = std::cell::Cell::new(0);
    catalog
        .backfill_oriented_dims(
            |path| {
                calls.set(calls.get() + 1);
                cancel.store(true, Ordering::Relaxed);
                measured_dims(path)
            },
            &cancel,
        )
        .unwrap();
    assert_eq!(
        calls.get(),
        1,
        "a cancel must stop measuring within a batch"
    );
    let again = std::cell::Cell::new(false);
    catalog
        .backfill_oriented_dims(
            |path| {
                again.set(true);
                measured_dims(path)
            },
            &AtomicBool::new(false),
        )
        .unwrap();
    assert!(
        again.get(),
        "a cancelled backfill must not set the done marker"
    );
}

#[test]
fn dims_backfill_skips_missing_files() {
    let (catalog, [portrait, heic, ..]) = dims_backfill_catalog();
    catalog
        .mark_missing_under(std::path::Path::new("/p/portrait.jpg"))
        .unwrap();
    let fixed = catalog
        .backfill_oriented_dims(
            |path| {
                assert!(
                    !path.ends_with("portrait.jpg"),
                    "a missing file is never measured"
                );
                measured_dims(path)
            },
            &std::sync::atomic::AtomicBool::new(false),
        )
        .unwrap();
    assert_eq!(fixed, 1);
    assert_eq!(
        dims_of(&catalog, portrait),
        (4032, 3024),
        "a missing row keeps its size"
    );
    assert_eq!(dims_of(&catalog, heic), (1000, 3000));
}

#[test]
fn panorama_rule_uses_oriented_dims() {
    let (catalog, [_, heic, ..]) = dims_backfill_catalog();
    let panoramas = |catalog: &Catalog| -> Vec<MediaId> {
        catalog
            .query_system_album(SystemAlbum::Panoramas, 10, 0)
            .unwrap()
            .iter()
            .map(|item| item.id)
            .collect()
    };
    assert!(
        panoramas(&catalog).is_empty(),
        "an unsized HEIC is no panorama"
    );
    catalog
        .backfill_oriented_dims(measured_dims, &std::sync::atomic::AtomicBool::new(false))
        .unwrap();
    assert_eq!(
        panoramas(&catalog),
        vec![heic],
        "a 1000x3000 vertical panorama joins Panoramas once sized"
    );
}

#[test]
fn a_changed_file_keeps_its_edited_date_and_place() {
    let catalog = Catalog::open_in_memory().unwrap();
    let edited = sample_media_item("/photos/edited.jpg", MediaType::Image, 1_000, None, None);
    let untouched = sample_media_item("/photos/untouched.jpg", MediaType::Image, 1_000, None, None);
    let unplaced = sample_media_item("/photos/unplaced.jpg", MediaType::Image, 1_000, None, None);
    let ids: Vec<MediaId> = catalog
        .insert_or_update_batch(&[edited.clone(), untouched.clone(), unplaced.clone()])
        .unwrap()
        .iter()
        .map(|item| item.id)
        .collect();
    let edited_ms = 1_600_000_000_000;
    catalog.set_timestamp(ids[0], edited_ms).unwrap();
    catalog
        .set_location(ids[0], Some(35.68), Some(139.69))
        .unwrap();
    catalog.set_location(ids[2], None, None).unwrap();
    let edited_row = catalog.get_item_by_id(ids[0]).unwrap().unwrap();
    assert!(
        edited_row.city.is_some(),
        "the edit geocodes to a named place"
    );

    // The file changes on disk (e.g. edited in another app); the watcher
    // re-reads it with the file's own date and coordinates.
    let rescan = |mut item: MediaItem| {
        item.mtime_epoch_ms += 1;
        item.timestamp_epoch_ms = 2_000;
        item.latitude = Some(48.85);
        item.longitude = Some(2.35);
        item.altitude = Some(35.0);
        item
    };
    catalog
        .insert_or_update_batch(&[rescan(edited), rescan(untouched), rescan(unplaced)])
        .unwrap();

    let kept = catalog.get_item_by_id(ids[0]).unwrap().unwrap();
    assert_eq!(
        kept.timestamp_epoch_ms, edited_ms,
        "an edited date survives a re-read"
    );
    assert_eq!(
        (kept.year, kept.month, kept.day),
        (edited_row.year, edited_row.month, edited_row.day),
        "the derived date parts stay with the edit"
    );
    assert_eq!((kept.latitude, kept.longitude), (Some(35.68), Some(139.69)));
    assert_eq!(
        kept.city, edited_row.city,
        "the edited place keeps its name"
    );
    assert_eq!(
        kept.altitude,
        Some(35.0),
        "altitude is never edited, so it follows the file"
    );
    let unplaced = catalog.get_item_by_id(ids[2]).unwrap().unwrap();
    assert_eq!(
        (unplaced.latitude, unplaced.city),
        (None, None),
        "a removed place stays removed when the file has GPS"
    );

    let fresh = catalog.get_item_by_id(ids[1]).unwrap().unwrap();
    assert_eq!(
        fresh.timestamp_epoch_ms, 2_000,
        "unedited items follow the file"
    );
    assert_eq!((fresh.latitude, fresh.longitude), (Some(48.85), Some(2.35)));
}

#[test]
fn a_v9_catalog_migrates_and_protects_later_edits() {
    let tmp_dir = unique_temp_dir("edited_migration");
    let db_path = tmp_dir.join("catalog.db");
    let item = sample_media_item("/photos/old/c.jpg", MediaType::Image, 1_000, None, None);
    let id = {
        let catalog = Catalog::open(&db_path).unwrap();
        catalog
            .insert_or_update_batch(std::slice::from_ref(&item))
            .unwrap()[0]
            .id
    };
    {
        // Downgrade to the v9 layout: no edited flags.
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "ALTER TABLE media_items DROP COLUMN date_edited;
             ALTER TABLE media_items DROP COLUMN place_edited;
             PRAGMA user_version = 9;",
        )
        .unwrap();
    }
    let catalog = Catalog::open(&db_path).expect("a v9 catalog must open and migrate");
    catalog.set_timestamp(id, 1_600_000_000_000).unwrap();
    let mut changed = item;
    changed.timestamp_epoch_ms = 2_000;
    catalog.insert_or_update_batch(&[changed]).unwrap();
    assert_eq!(
        catalog
            .get_item_by_id(id)
            .unwrap()
            .unwrap()
            .timestamp_epoch_ms,
        1_600_000_000_000,
        "a migrated catalog protects edits made after the upgrade"
    );
    drop(catalog);
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);
    drop(conn);
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn a_date_set_to_capture_time_zero_is_undated() {
    let catalog = Catalog::open_in_memory().unwrap();
    let item = sample_media_item(
        "/photos/a.jpg",
        MediaType::Image,
        1_600_000_000_000,
        None,
        None,
    );
    let id = catalog.insert_or_update_batch(&[item]).unwrap()[0].id;
    catalog.set_timestamp(id, 0).unwrap();
    let item = catalog.get_item_by_id(id).unwrap().unwrap();
    assert_eq!(
        (item.year, item.month, item.day),
        (0, 0, 0),
        "a capture time of 0 files the item as undated, not January 1970"
    );
}

#[test]
fn a_batch_edit_is_all_or_nothing() {
    let catalog = Catalog::open_in_memory().unwrap();
    let items = [
        sample_media_item("/photos/a.jpg", MediaType::Image, 1_000, None, None),
        sample_media_item("/photos/b.jpg", MediaType::Image, 1_000, None, None),
    ];
    let ids: Vec<MediaId> = catalog
        .insert_or_update_batch(&items)
        .unwrap()
        .iter()
        .map(|item| item.id)
        .collect();

    assert!(
        catalog
            .set_timestamps(&[(ids[0], 1_600_000_000_000), (ids[1], i64::MAX)])
            .is_err(),
        "an invalid date fails the batch"
    );
    assert_eq!(
        catalog
            .get_item_by_id(ids[0])
            .unwrap()
            .unwrap()
            .timestamp_epoch_ms,
        1_000,
        "a failed batch leaves earlier items untouched"
    );

    catalog.set_ratings(&[(ids[0], 9), (ids[1], 2)]).unwrap();
    catalog
        .set_color_labels(&[(ids[0], 9), (ids[1], 0)])
        .unwrap();
    catalog
        .set_titles(&[(ids[0], Some("  Kyoto ")), (ids[1], Some("   "))])
        .unwrap();
    catalog
        .set_captions(&[(ids[0], "Golden hour"), (ids[1], "")])
        .unwrap();
    let ratings = catalog.get_ratings_map().unwrap();
    assert_eq!(
        (ratings[&ids[0]], ratings[&ids[1]]),
        (5, 2),
        "ratings clamp to 5"
    );
    assert_eq!(
        catalog.get_color_labels_map().unwrap()[&ids[0]],
        6,
        "labels clamp to 6"
    );
    let a = catalog.get_item_by_id(ids[0]).unwrap().unwrap();
    let b = catalog.get_item_by_id(ids[1]).unwrap().unwrap();
    assert_eq!(
        (a.title.as_deref(), b.title),
        (Some("Kyoto"), None),
        "titles trim; blank clears"
    );
    let captions = catalog.get_captions_map().unwrap();
    assert_eq!(
        captions.get(&ids[0]).map(String::as_str),
        Some("Golden hour")
    );
    assert!(
        !captions.contains_key(&ids[1]),
        "a blank caption is removed"
    );
    assert_eq!(
        catalog
            .search_structured_count(&StructuredQuery::parse("golden"))
            .unwrap(),
        1,
        "batch captions stay searchable"
    );
}

#[test]
fn test_duplicate_detection_and_merging() {
    let tmp_dir = unique_temp_dir("dup_test");
    let library_dir = tmp_dir.join("library");
    let cache_dir = tmp_dir.join("cache");
    let db_dir = tmp_dir.join("db");
    std::fs::create_dir_all(&library_dir).unwrap();
    std::fs::create_dir_all(&cache_dir).unwrap();
    std::fs::create_dir_all(&db_dir).unwrap();

    // 1. Create duplicate files (A and A_copy), a unique file (B), and a same-size-different-content file (C)
    let mut content_a = vec![0x41u8; 100];
    content_a[0..10].copy_from_slice(b"photo-A-01");
    let mut content_b = vec![0x42u8; 200];
    content_b[0..10].copy_from_slice(b"photo-B-01");
    let mut content_c = vec![0x43u8; 100];
    content_c[0..10].copy_from_slice(b"photo-C-01");

    let p1 = library_dir.join("photo1.jpg");
    let p1_copy = library_dir.join("photo1_copy.jpg");
    let p2 = library_dir.join("photo2.jpg");
    let p3 = library_dir.join("photo3.jpg");

    std::fs::write(&p1, &content_a).unwrap();
    std::fs::write(&p1_copy, &content_a).unwrap();
    std::fs::write(&p2, &content_b).unwrap();
    std::fs::write(&p3, &content_c).unwrap();

    let db_path = db_dir.join("catalog.db");
    let (engine, _event_rx) =
        LuminaEngine::open_with_paths(library_dir.clone(), db_path, Some(cache_dir)).unwrap();

    // Scan the library
    engine.rescan();

    // Wait for scan to populate catalog items
    let mut attempts = 0;
    while engine.total_count() < 4 && attempts < 50 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        attempts += 1;
    }
    assert_eq!(engine.total_count(), 4, "all 4 items indexed in catalog");

    // Fetch items
    let all_items = engine.get_item_slice(0..10);
    let item1 = all_items
        .iter()
        .find(|i| i.file_name == "photo1.jpg")
        .unwrap();
    let item1_copy = all_items
        .iter()
        .find(|i| i.file_name == "photo1_copy.jpg")
        .unwrap();

    // Add metadata: tag & favorite on copy, rating on original
    engine
        .add_tags_batch(&[item1_copy.id], &["summer".to_string()])
        .unwrap();
    engine.batch_set_favorite(&[item1_copy.id], true).unwrap();
    engine.set_ratings(&[(item1.id, 5)]).unwrap();

    // Scan for duplicates
    let report = engine.scan_duplicates().unwrap();
    assert_eq!(report.groups.len(), 1, "exactly 1 duplicate group detected");
    assert_eq!(report.total_duplicate_items, 2);
    assert_eq!(report.potential_reclaimable_bytes, 100);

    let group = &report.groups[0];
    assert_eq!(group.items.len(), 2);
    assert_eq!(group.file_size, 100);
    let group_ids: Vec<MediaId> = group.items.iter().map(|i| i.id).collect();
    assert!(group_ids.contains(&item1.id));
    assert!(group_ids.contains(&item1_copy.id));

    // Verify query helpers
    assert_eq!(engine.query_duplicate_count(), 2);
    let slice = engine.query_duplicate_items_slice(0, 10);
    assert_eq!(slice.len(), 2);

    // Import duplicate pre-check: one copy of content A, one unique file.
    let external_dup = tmp_dir.join("external_dup.jpg");
    std::fs::write(&external_dup, &content_a).unwrap();
    let external_unique = tmp_dir.join("external_unique.jpg");
    std::fs::write(&external_unique, b"totally unique external photo bytes").unwrap();
    let candidates = engine
        .check_import_duplicates(
            &[external_dup.clone(), external_unique.clone()],
            &std::sync::atomic::AtomicBool::new(false),
        )
        .unwrap();
    assert_eq!(candidates.len(), 2);
    let cand_dup = candidates.iter().find(|c| c.path == external_dup).unwrap();
    assert!(
        cand_dup.is_duplicate,
        "external file with content A matched existing library item"
    );
    assert!(cand_dup.matched_item_id.is_some());
    let cand_unique = candidates
        .iter()
        .find(|c| c.path == external_unique)
        .unwrap();
    assert!(!cand_unique.is_duplicate);

    // Test merge duplicate group
    engine
        .merge_duplicate_group(item1.id, &[item1_copy.id])
        .unwrap();

    // Verify keeper has merged metadata
    let keeper_after = engine.get_item_by_id(item1.id).unwrap();
    assert!(
        keeper_after.is_favorite,
        "favorite flag merged from copy to keeper"
    );
    assert_eq!(engine.get_ratings_map().get(&item1.id).copied(), Some(5));
    let tags = engine.get_tags_for_item(item1.id);
    assert!(
        tags.contains(&"summer".to_string()),
        "tag merged from copy to keeper"
    );

    // Verify copy was moved to trash
    let copy_after = engine.get_item_by_id(item1_copy.id).unwrap();
    assert!(copy_after.is_trashed, "duplicate copy is trashed");

    // Verify duplicates query reflects resolved state
    assert_eq!(
        engine.query_duplicate_count(),
        0,
        "no active duplicates remain after merge"
    );

    drop(engine);
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn album_query_plan_uses_membership_index() {
    use rusqlite::types::Value;
    let catalog = Catalog::open_in_memory().expect("open catalog");
    let items: Vec<MediaItem> = (0..10_000)
        .map(|i| {
            sample_media_item(
                &format!("/lib/plan_{i:05}.jpg"),
                MediaType::Image,
                1_700_000_000_000 + i * 1_000,
                None,
                None,
            )
        })
        .collect();
    let ids: Vec<MediaId> = catalog
        .insert_or_update_batch(&items)
        .expect("insert items")
        .into_iter()
        .map(|item| item.id)
        .collect();
    let album = catalog.create_album("Picks").expect("create album");
    catalog
        .add_to_album(album.id, &ids[..500])
        .expect("add members");

    for field in [
        SortField::DateTaken,
        SortField::FileName,
        SortField::FileSize,
        SortField::Rating,
    ] {
        for direction in [SortDirection::Descending, SortDirection::Ascending] {
            let sort = SortSpec { field, direction };
            let sql = Catalog::album_slice_sql(sort);
            let params = [
                Value::Integer(album.id),
                Value::Integer(50),
                Value::Integer(0),
            ];
            // The SQL numbers its parameters; inline them by position.
            let numbered = sql.replace("?1", "?").replace("?2", "?").replace("?3", "?");
            let rows = catalog
                .explain_query_plan(&inline_sql_params(&numbered, &params))
                .expect("explain the album query");
            let plan = rows.join(" | ");
            assert!(
                rows.iter().any(|row| row.starts_with("SEARCH ai USING")
                    && (row.contains("PRIMARY KEY (album_id=?)")
                        || row.contains("idx_album_items_media"))),
                "{sort:?}: the album query must seek its members by album id, got: {plan}"
            );
            assert!(
                !rows.iter().any(|row| row.starts_with("SCAN ")),
                "{sort:?}: the album query must scan neither media_items nor album_items, got: {plan}"
            );
        }
    }
}

#[test]
fn view_totals_sum_duration_and_size() {
    use lumina_core::{SystemAlbum, ViewScope};
    let catalog = Catalog::open_in_memory().unwrap();
    let clip = |path: &str, duration: Option<u64>, size: u64, month: u32| {
        let mut item = sample_media_item(path, MediaType::Video, 1_700_000_000_000, None, None);
        item.duration_ms = duration;
        item.file_size = size;
        (item.year, item.month) = (2025, month);
        item
    };
    let stored = catalog
        .insert_or_update_batch(&[
            clip("/totals/a.mov", Some(3_600_000), 1_000_000, 6),
            clip("/totals/b.mov", Some(120_500), 2_500_000, 6),
            // An unprobed clip has no duration: it adds its size only.
            clip("/totals/c.mov", None, 500, 7),
            sample_media_item("/totals/photo.jpg", MediaType::Image, 1, None, None),
        ])
        .unwrap();
    let videos = ViewScope::System(SystemAlbum::Videos);
    let totals = catalog.view_totals(&videos).unwrap();
    assert_eq!(
        (totals.count, totals.duration_ms, totals.bytes),
        (3, 3_720_500, 3_500_500),
        "Videos totals cover exactly its three clips, not the photo"
    );
    assert_eq!(
        totals.unknown_durations, 1,
        "the unprobed clip is counted, so the header can say the duration is a lower bound"
    );
    assert_eq!(
        (totals.months.get(&(2025, 6)), totals.months.get(&(2025, 7))),
        (Some(&2), Some(&1)),
        "per-month counts follow the view's rows, for the month band"
    );
    catalog.batch_set_trashed(&[stored[0].id], true).unwrap();
    let totals = catalog.view_totals(&videos).unwrap();
    assert_eq!(
        (totals.count, totals.duration_ms, totals.bytes),
        (2, 120_500, 2_500_500),
        "a trashed clip leaves the totals, as it leaves the count"
    );
    let empty = catalog.view_totals(&ViewScope::Album(999)).unwrap();
    assert_eq!(
        (
            empty.count,
            empty.duration_ms,
            empty.bytes,
            empty.months.len()
        ),
        (0, 0, 0, 0),
        "an empty view sums to zero, not NULL"
    );
}

#[test]
fn view_date_span_ignores_undated_items() {
    use lumina_core::{DateSpan, ViewScope};
    let catalog = Catalog::open_in_memory().unwrap();
    let dated = |path: &str, ts: i64, year: i32, month: u32| {
        let mut item = sample_media_item(path, MediaType::Image, ts, None, None);
        (item.year, item.month, item.day) = (year, month, 1);
        item
    };
    let mut undated = dated("/span/undated.jpg", 0, 0, 0);
    undated.day = 0;
    let stored = catalog
        .insert_or_update_batch(&[
            dated("/span/old.jpg", 1_528_000_000_000, 2018, 6),
            dated("/span/new.jpg", 1_794_000_000_000, 2026, 11),
            undated,
        ])
        .unwrap();
    let span = Some(DateSpan {
        first: (2018, 6),
        last: (2026, 11),
    });
    assert_eq!(
        catalog.view_date_span(&ViewScope::Library).unwrap(),
        span,
        "the library spans its oldest and newest dated items; the undated one is left out"
    );
    assert_eq!(
        catalog
            .view_date_span(&ViewScope::Search("year:2018".into()))
            .unwrap(),
        Some(DateSpan {
            first: (2018, 6),
            last: (2018, 6),
        }),
        "a filtered view spans only its own items"
    );

    let album = catalog
        .create_album_with("Undated only", &[stored[2].id])
        .unwrap();
    assert_eq!(
        catalog.view_date_span(&ViewScope::Album(album.id)).unwrap(),
        None,
        "an all-undated view has no span"
    );
    catalog.batch_set_trashed(&[stored[0].id], true).unwrap();
    assert_eq!(
        catalog.view_date_span(&ViewScope::Library).unwrap(),
        Some(DateSpan {
            first: (2026, 11),
            last: (2026, 11),
        }),
        "a trashed item leaves the library's span, as it leaves its count"
    );
}

#[test]
fn view_layout_follows_the_view_order_and_aspect() {
    use lumina_core::ViewScope;
    let catalog = Catalog::open_in_memory().unwrap();
    let sized = |path: &str, ts: i64, width: u32, height: u32| {
        let mut item = sample_media_item(path, MediaType::Image, ts, None, None);
        (item.width, item.height) = (width, height);
        (item.year, item.month, item.day) = (2026, 3, 7);
        item
    };
    let mut undated = sized("/layout/undated.jpg", 0, 1000, 1000);
    (undated.year, undated.month, undated.day) = (0, 0, 0);
    // What the scanner writes for a capture time of 0: 1970-01-01.
    let mut scanned_undated = sized("/layout/scanned_undated.jpg", 0, 1000, 1000);
    (
        scanned_undated.year,
        scanned_undated.month,
        scanned_undated.day,
    ) = (1970, 1, 1);
    let stored = catalog
        .insert_or_update_batch(&[
            sized("/layout/landscape.jpg", 5_000, 4000, 3000),
            sized("/layout/turned.jpg", 4_000, 4000, 3000),
            sized("/layout/unprobed.mov", 3_000, 0, 0),
            sized("/layout/panorama.jpg", 2_000, 10_000, 1000),
            undated,
            scanned_undated,
        ])
        .unwrap();
    catalog
        .set_rotations(&[(stored[1].id, 1)])
        .expect("rotate a quarter turn");

    let ids = |cells: Vec<lumina_core::LayoutCell>| cells.iter().map(|c| c.id).collect::<Vec<_>>();
    let item_ids = |items: Vec<MediaItem>| items.iter().map(|m| m.id).collect::<Vec<_>>();
    let cells = catalog
        .view_layout(&ViewScope::Library, SortSpec::default())
        .unwrap();
    assert_eq!(
        ids(cells.clone()),
        item_ids(catalog.get_item_slice(0, 10).unwrap()),
        "layout cells come in the grid's own order"
    );
    let by_name = SortSpec {
        field: SortField::FileName,
        direction: SortDirection::Ascending,
    };
    assert_eq!(
        ids(catalog.view_layout(&ViewScope::Library, by_name).unwrap()),
        item_ids(catalog.get_item_slice_sorted(0, 10, by_name).unwrap()),
        "a non-default sort lays out in the grid's name order"
    );
    let jpg = StructuredQuery::parse("jpg");
    let search = catalog
        .search_structured_sorted(&jpg, 10, 0, SortSpec::default())
        .unwrap();
    assert_eq!(search.len(), 5, "the FTS term matches every .jpg");
    assert_eq!(
        ids(catalog
            .view_layout(&ViewScope::Search("jpg".into()), SortSpec::default())
            .unwrap()),
        item_ids(search),
        "a search layout matches the search grid's rows and order"
    );
    let album = catalog
        .create_album_with("Some", &[stored[3].id, stored[0].id, stored[5].id])
        .unwrap();
    assert_eq!(
        ids(catalog
            .view_layout(&ViewScope::Album(album.id), by_name)
            .unwrap()),
        item_ids(
            catalog
                .query_album_sorted(album.id, 10, 0, by_name)
                .unwrap()
        ),
        "an album layout holds only its members, in the album grid's order"
    );
    let cell = |i: usize| {
        *cells
            .iter()
            .find(|c| c.id == stored[i].id)
            .expect("every active item has a cell")
    };
    assert!(
        (cell(0).aspect - 4.0 / 3.0).abs() < 1e-3,
        "4000x3000 is 1.333"
    );
    assert!(
        (cell(1).aspect - 0.75).abs() < 1e-3,
        "a quarter turn swaps the aspect"
    );
    assert_eq!(cell(2).aspect, 1.0, "a 0x0 item lays out square");
    assert_eq!(cell(3).aspect, 3.0, "a 10:1 panorama is clamped to 3.0");
    assert_eq!(cell(0).day, 20260307, "day is yyyymmdd");
    assert_eq!(cell(4).day, 0, "an undated item has day 0");
    assert_eq!(
        cell(5).day,
        0,
        "a capture time of 0 is undated even though the scanner stores 1970-01-01"
    );
}

#[test]
fn day_places_picks_the_most_common_city_per_day() {
    use lumina_core::ViewScope;
    let catalog = Catalog::open_in_memory().unwrap();
    let at = |path: &str, day: u32, place: Option<(&str, &str)>| {
        let mut item = sample_media_item(path, MediaType::Image, day as i64, None, None);
        (item.year, item.month, item.day) = (2026, 5, day);
        item.city = place.map(|p| p.0.to_string());
        item.country_code = place.map(|p| p.1.to_string());
        item
    };
    let lisbon = Some(("Lisbon", "PT"));
    let stored = catalog
        .insert_or_update_batch(&[
            at("/p/1.jpg", 1, lisbon),
            at("/p/2.jpg", 1, lisbon),
            at("/p/3.jpg", 1, lisbon),
            at("/p/4.jpg", 1, Some(("Porto", "PT"))),
            at("/p/5.jpg", 2, Some(("Porto", "PT"))),
            at("/p/6.jpg", 2, Some(("Faro", "PT"))),
            at("/p/7.jpg", 3, None),
            {
                // A capture time of 0 with a city, as the scanner stores it.
                let mut undated = at("/p/undated.jpg", 1, lisbon);
                undated.timestamp_epoch_ms = 0;
                (undated.year, undated.month, undated.day) = (1970, 1, 1);
                undated
            },
        ])
        .unwrap();
    let places = catalog.day_places(&ViewScope::Library).unwrap();
    assert_eq!(
        places.len(),
        2,
        "only the two placed dated days have a place; the undated item (capture time 0, \
         stored 1970-01-01) has none: {places:?}"
    );
    assert_eq!(
        places.get(&20260501),
        Some(&("Lisbon".to_string(), Some("PT".to_string()))),
        "the most common city wins the day"
    );
    assert_eq!(
        places.get(&20260502).map(|p| p.0.as_str()),
        Some("Faro"),
        "a tie goes to the alphabetically first city"
    );
    assert!(
        !places.contains_key(&20260503),
        "a day with no city has no place"
    );

    let album = catalog
        .create_album_with("Trip", &[stored[0].id, stored[3].id])
        .unwrap();
    catalog
        .remove_from_album(album.id, &[stored[0].id])
        .unwrap();
    let album_places = catalog.day_places(&ViewScope::Album(album.id)).unwrap();
    assert_eq!(
        album_places.get(&20260501).map(|p| p.0.as_str()),
        Some("Porto"),
        "an album counts only its members, not the library's Lisbon majority"
    );
    assert!(
        !album_places.contains_key(&20260502),
        "days without members are absent"
    );
}

#[test]
fn album_cards_pick_newest_active_member() {
    use lumina_core::AlbumCard;
    let catalog = Catalog::open_in_memory().expect("open catalog");
    const T: i64 = 1_700_000_000_000;
    // Items 4 and 5 share a timestamp so the tie-breaker is exercised; 6 is
    // the newest of all but hidden; 7 is undated (capture time 0).
    let stamps = [
        T + 1_000,
        T + 3_000,
        T + 2_000,
        T + 5_000,
        T + 4_000,
        T + 4_000,
        T + 6_000,
        0,
    ];
    let items: Vec<MediaItem> = stamps
        .iter()
        .enumerate()
        .map(|(i, &ts)| {
            let mut item = sample_media_item(
                &format!("/lib/card_{i}.jpg"),
                MediaType::Image,
                ts,
                None,
                None,
            );
            item.is_hidden = i == 6;
            item
        })
        .collect();
    let ids: Vec<MediaId> = catalog
        .insert_or_update_batch(&items)
        .expect("insert items")
        .into_iter()
        .map(|item| item.id)
        .collect();

    let empty = catalog.create_album("Empty").expect("create album");
    let trashed = catalog
        .create_album_with("Trashed newest", &ids[..4])
        .expect("create album");
    let normal = catalog
        .create_album_with("Normal, hidden newest", &[ids[0], ids[4], ids[5], ids[6]])
        .expect("create album");
    let only_trashed = catalog
        .create_album_with("Only trashed", &[ids[3]])
        .expect("create album");
    let undated = catalog
        .create_album_with("Undated", &[ids[7]])
        .expect("create album");
    catalog.set_trashed(ids[3], true).expect("trash newest");

    let cards = catalog.album_cards().expect("album cards");
    assert_eq!(
        cards,
        vec![
            AlbumCard {
                album_id: empty.id,
                cover: None,
                newest_epoch_ms: None,
            },
            AlbumCard {
                album_id: trashed.id,
                cover: Some(ids[1]),
                newest_epoch_ms: Some(T + 3_000),
            },
            AlbumCard {
                album_id: normal.id,
                cover: Some(MediaId(ids[4].0.max(ids[5].0))),
                newest_epoch_ms: Some(T + 4_000),
            },
            AlbumCard {
                album_id: only_trashed.id,
                cover: None,
                newest_epoch_ms: None,
            },
            AlbumCard {
                album_id: undated.id,
                cover: Some(ids[7]),
                newest_epoch_ms: None,
            },
        ],
        "every album gets one card in album order; the cover is the newest active member \
         (larger id on a tie), trashed and hidden members are skipped, an album with no \
         active member has no cover or date, and an undated cover has no date"
    );
}

#[test]
fn album_stats_counts_kinds_and_range() {
    use lumina_core::AlbumStats;
    let catalog = Catalog::open_in_memory().expect("open catalog");
    const T: i64 = 1_700_000_000_000;
    // 0-1 photos, 2 a video, 3 an undated photo, 4 a trashed video (the
    // oldest), 5 a hidden photo (the newest).
    let kinds = [
        (MediaType::Image, T + 2_000),
        (MediaType::Image, T + 9_000),
        (MediaType::Video, T + 5_000),
        (MediaType::Image, 0),
        (MediaType::Video, T),
        (MediaType::Image, T + 20_000),
    ];
    let items: Vec<MediaItem> = kinds
        .iter()
        .enumerate()
        .map(|(i, &(kind, ts))| {
            let mut item = sample_media_item(&format!("/lib/stats_{i}"), kind, ts, None, None);
            item.is_hidden = i == 5;
            item
        })
        .collect();
    let ids: Vec<MediaId> = catalog
        .insert_or_update_batch(&items)
        .expect("insert items")
        .into_iter()
        .map(|item| item.id)
        .collect();
    let album = catalog.create_album_with("Trip", &ids).expect("album");
    catalog.set_trashed(ids[4], true).expect("trash");
    let empty = catalog.create_album("Empty").expect("empty album");

    assert_eq!(
        catalog.album_stats(album.id).expect("stats"),
        AlbumStats {
            photos: 3,
            videos: 1,
            first_epoch_ms: Some(T + 2_000),
            last_epoch_ms: Some(T + 9_000),
            cover: Some(ids[1]),
        },
        "stats count active members by kind and span their dated capture times; \
         trashed, hidden and undated members never widen the range; the cover is \
         the newest active member"
    );
    assert_eq!(
        catalog.album_stats(empty.id).expect("empty stats"),
        AlbumStats::default(),
        "an empty album has no items and no range"
    );
}

#[test]
fn album_cover_set_cleared_on_purge_and_migrated() {
    let tmp_dir = unique_temp_dir("album_cover");
    // Removes the temp dir even when an assertion below panics.
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(tmp_dir.clone());
    let db_path = tmp_dir.join("catalog.db");
    const T: i64 = 1_700_000_000_000;
    // 0 oldest, 2 newest; 3 is in the library but not in the album.
    let (ids, album_id) = {
        let catalog = Catalog::open(&db_path).unwrap();
        let items: Vec<MediaItem> = (0..4)
            .map(|i| {
                sample_media_item(
                    &format!("/lib/cover_{i}.jpg"),
                    MediaType::Image,
                    T + i * 1_000,
                    None,
                    None,
                )
            })
            .collect();
        let ids: Vec<MediaId> = catalog
            .insert_or_update_batch(&items)
            .unwrap()
            .into_iter()
            .map(|item| item.id)
            .collect();
        let album = catalog.create_album_with("Trip", &ids[..3]).unwrap();
        (ids, album.id)
    };
    {
        // Back to the v21 layout: albums without cover_media_id. DROP COLUMN
        // refuses a foreign-key column, so rebuild the table.
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            r#"
            PRAGMA foreign_keys = OFF;
            DROP INDEX IF EXISTS idx_albums_cover;
            CREATE TABLE albums_v21 (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                created_at_epoch_ms INTEGER NOT NULL,
                pinned INTEGER NOT NULL DEFAULT 0
            );
            INSERT INTO albums_v21 SELECT id, title, created_at_epoch_ms, pinned FROM albums;
            DROP TABLE albums;
            ALTER TABLE albums_v21 RENAME TO albums;
            PRAGMA user_version = 21;
            "#,
        )
        .unwrap();
    }
    for _ in 0..2 {
        let catalog = Catalog::open(&db_path).expect("a v21 catalog must open and migrate");
        assert_eq!(catalog.schema_version().unwrap(), SCHEMA_VERSION);
        let (album, count) = catalog.list_albums_with_counts().unwrap().remove(0);
        assert_eq!(
            (album.id, album.cover_media_id, count),
            (album_id, None, 3),
            "a migrated album keeps its members and starts without a key photo"
        );
    }

    let library = tmp_dir.join("library");
    std::fs::create_dir_all(&library).unwrap();
    let (engine, _rx) =
        LuminaEngine::open_with_paths(library, db_path.clone(), Some(tmp_dir.join("cache")))
            .expect("engine");
    let catalog = Catalog::open(&db_path).unwrap();
    let card_cover = |catalog: &Catalog| catalog.album_cards().unwrap()[0].cover;
    let stats_cover = |catalog: &Catalog| catalog.album_stats(album_id).unwrap().cover;
    assert_eq!(card_cover(&catalog), Some(ids[2]), "no key photo: newest");

    engine.set_album_cover(album_id, Some(ids[0])).unwrap();
    assert_eq!(
        catalog.list_albums_with_counts().unwrap()[0]
            .0
            .cover_media_id,
        Some(ids[0]),
        "the key photo is stored on the album row"
    );
    assert_eq!(
        card_cover(&catalog),
        Some(ids[0]),
        "cards use the key photo"
    );
    assert_eq!(
        stats_cover(&catalog),
        Some(ids[0]),
        "stats use the key photo"
    );

    assert!(
        engine.set_album_cover(album_id, Some(ids[3])).is_err(),
        "a non-member cannot be the key photo"
    );
    for cover in [None, Some(ids[0])] {
        let err = engine
            .set_album_cover(album_id + 100, cover)
            .expect_err("an unknown album is an error");
        assert!(
            err.to_string().contains("album no longer exists"),
            "an unknown album reads as unknown, not as a non-member: {err}"
        );
    }
    assert_eq!(
        catalog.list_albums_with_counts().unwrap()[0]
            .0
            .cover_media_id,
        Some(ids[0]),
        "a rejected cover leaves the stored one alone"
    );

    catalog.set_trashed(ids[0], true).unwrap();
    assert_eq!(
        (card_cover(&catalog), stats_cover(&catalog)),
        (Some(ids[2]), Some(ids[2])),
        "a trashed key photo falls back to the newest member"
    );
    catalog.set_trashed(ids[0], false).unwrap();
    assert_eq!(
        card_cover(&catalog),
        Some(ids[0]),
        "restoring it brings it back"
    );

    catalog.remove_from_album(album_id, &[ids[0]]).unwrap();
    assert_eq!(
        (card_cover(&catalog), stats_cover(&catalog)),
        (Some(ids[2]), Some(ids[2])),
        "a key photo removed from the album falls back to the newest member"
    );
    catalog.add_to_album(album_id, &[ids[0]]).unwrap();

    engine.set_album_cover(album_id, Some(ids[1])).unwrap();
    catalog
        .permanently_delete_items(&[ids[1]], DeletionTarget::CatalogOnly)
        .unwrap();
    assert_eq!(
        catalog.list_albums_with_counts().unwrap()[0]
            .0
            .cover_media_id,
        None,
        "purging the key photo clears the column"
    );
    assert_eq!(
        card_cover(&catalog),
        Some(ids[2]),
        "a purged key photo falls back to the newest member"
    );

    engine.set_album_cover(album_id, Some(ids[0])).unwrap();
    engine.set_album_cover(album_id, None).unwrap();
    assert_eq!(
        card_cover(&catalog),
        Some(ids[2]),
        "clearing restores newest"
    );
    drop((engine, catalog));
}

#[test]
fn storage_summary_sums_by_kind() {
    let catalog = Catalog::open_in_memory().expect("open catalog");
    let sized = |path: &str, kind: MediaType, bytes: u64| MediaItem {
        file_size: bytes,
        ..sample_media_item(path, kind, 1000, None, None)
    };
    catalog
        .insert_or_update_batch(&[
            sized("/lib/a.jpg", MediaType::Image, 100),
            sized("/lib/b.dng", MediaType::Raw, 200),
            sized("/lib/c.mp4", MediaType::Video, 4_000),
        ])
        .unwrap();
    catalog
        .insert_with_source(
            &sized("/lib/Imports/2026/d.mp4", MediaType::Video, 50_000),
            "https://example.com/watch?v=1",
            None,
        )
        .unwrap();
    // A sidecar's web address alone is not a download: still a video.
    let sidecar = catalog
        .insert_or_update_batch(&[sized("/lib/e.mp4", MediaType::Video, 600_000)])
        .unwrap()[0]
        .id;
    catalog
        .record_media_source(sidecar, "https://example.com/e", None)
        .unwrap();
    // Trashed items are left out, as the library's item count leaves them out.
    let trashed = catalog
        .insert_or_update_batch(&[sized("/lib/f.jpg", MediaType::Image, 7_000_000)])
        .unwrap()[0]
        .id;
    catalog.set_trashed(trashed, true).unwrap();

    let summary = catalog.storage_summary().unwrap();
    assert_eq!(
        summary,
        lumina_core::StorageSummary {
            photo_bytes: 300,
            video_bytes: 604_000,
            download_bytes: 50_000,
        },
        "photos (images and RAW), videos and downloads are summed apart; \
         a download counts once, as a download"
    );
    assert_eq!(
        Catalog::open_in_memory()
            .unwrap()
            .storage_summary()
            .unwrap(),
        lumina_core::StorageSummary::default(),
        "an empty library sums to zero"
    );
}

/// 2019-07-14T09:30:00Z, the date the files say now.
const SHOT_MS: i64 = 1_563_096_600_000;

/// Rows as the old scanner stored them: dated by the file's mtime when no
/// EXIF date was found. Returns the catalog and the ids of a download dated
/// its import day, a photo with an EXIF date, a file whose mtime is old, a
/// date the user edited, an unreadable file and a video.
fn capture_dates_catalog() -> (Catalog, [MediaId; 6]) {
    let catalog = Catalog::open_in_memory().expect("open catalog");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let exif_dated = MediaItem {
        timestamp_epoch_ms: 1_420_070_400_000,
        ..sample_media_item("/p/exif.jpg", MediaType::Image, now, None, None)
    };
    let stored = catalog
        .insert_or_update_batch(&[
            sample_media_item("/p/download.jpg", MediaType::Image, now, None, None),
            exif_dated,
            sample_media_item(
                "/p/old.jpg",
                MediaType::Image,
                1_500_000_000_000,
                None,
                None,
            ),
            sample_media_item("/p/edited.jpg", MediaType::Image, now, None, None),
            sample_media_item("/p/gone.jpg", MediaType::Image, now, None, None),
            sample_media_item("/v/clip.mov", MediaType::Video, now, None, None),
        ])
        .expect("insert rows");
    let ids = [0, 1, 2, 3, 4, 5].map(|i| stored[i].id);
    catalog
        .set_timestamp(ids[3], now)
        .expect("the user edits a date");
    (catalog, ids)
}

/// What the scanner would store now. Only import-day candidates are read.
fn measured_capture_time(path: &std::path::Path) -> Option<i64> {
    match path.file_name()?.to_str()? {
        "download.jpg" | "clip.mov" => Some(SHOT_MS),
        "gone.jpg" => None,
        other => panic!("only rows dated their import day are read, not {other}"),
    }
}

fn timestamp_of(catalog: &Catalog, id: MediaId) -> i64 {
    catalog
        .get_item_by_id(id)
        .unwrap()
        .expect("row exists")
        .timestamp_epoch_ms
}

#[test]
fn capture_date_backfill_redates_only_import_day_rows_once() {
    let (catalog, [download, exif, old, edited, gone, clip]) = capture_dates_catalog();
    let before = |id| timestamp_of(&catalog, id);
    let (exif_ts, old_ts, edited_ts, gone_ts) =
        (before(exif), before(old), before(edited), before(gone));
    let cancel = std::sync::atomic::AtomicBool::new(false);

    let fixed = catalog
        .backfill_capture_dates(measured_capture_time, &cancel)
        .expect("backfill runs");
    assert_eq!(fixed, 2, "the download and the video are re-dated");
    for id in [download, clip] {
        let item = catalog.get_item_by_id(id).unwrap().unwrap();
        assert_eq!(
            item.timestamp_epoch_ms, SHOT_MS,
            "an import-day date is replaced"
        );
        assert_eq!(
            (item.year, item.month, item.day),
            (2019, 7, 14),
            "the date columns move with the capture time"
        );
    }
    assert_eq!(
        timestamp_of(&catalog, exif),
        exif_ts,
        "an EXIF date is kept"
    );
    assert_eq!(
        timestamp_of(&catalog, old),
        old_ts,
        "an old file date is kept"
    );
    assert_eq!(
        timestamp_of(&catalog, edited),
        edited_ts,
        "an edited date is kept"
    );
    assert_eq!(
        timestamp_of(&catalog, gone),
        gone_ts,
        "an unreadable file keeps its date"
    );
    assert_eq!(
        catalog.get_config("capture_dates_v1").unwrap().as_deref(),
        Some("1"),
        "a finished backfill sets its marker"
    );

    let again = catalog
        .backfill_capture_dates(|_| panic!("the marker stops a second run"), &cancel)
        .expect("second run is a no-op");
    assert_eq!(again, 0);
}

#[test]
fn cancelled_capture_date_backfill_changes_nothing_and_runs_again() {
    let (catalog, [download, ..]) = capture_dates_catalog();
    let before = timestamp_of(&catalog, download);
    let cancel = std::sync::atomic::AtomicBool::new(true);
    let fixed = catalog
        .backfill_capture_dates(measured_capture_time, &cancel)
        .expect("a cancelled backfill is not an error");
    assert_eq!(fixed, 0);
    assert_eq!(timestamp_of(&catalog, download), before);
    assert_eq!(
        catalog.get_config("capture_dates_v1").unwrap(),
        None,
        "a cancelled backfill runs again on the next scan"
    );
}
