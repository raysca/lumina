use lumina_core::{
    read_sidecar, write_sidecar, Catalog, DecodedFrame, DeletionTarget, DiskCache, EngineEvent,
    LuminaEngine, MatchMode, MediaId, MediaItem, MediaType, OfflineGeocoder, PixelFormat,
    RuleField, RuleOperator, RuleValue, Scanner, SidecarMetadata, SmartAlbumRule, SortDirection,
    SortField, SortSpec, StructuredQuery, SystemAlbum, SCHEMA_VERSION,
};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

fn sample_media_item(
    path_str: &str,
    media_type: MediaType,
    ts: i64,
    city: Option<&str>,
    model: Option<&str>,
) -> MediaItem {
    let path = PathBuf::from(path_str);
    let parent_dir = path
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    MediaItem {
        id: MediaId(0),
        path,
        parent_dir,
        file_name,
        title: None,
        file_size: 2048,
        mtime_epoch_ms: ts,
        media_type,
        width: 1920,
        height: 1080,
        duration_ms: if media_type == MediaType::Video {
            Some(12500)
        } else {
            None
        },
        timestamp_epoch_ms: ts,
        year: 2026,
        month: 10,
        day: 10,
        latitude: if city.is_some() { Some(35.0116) } else { None },
        longitude: if city.is_some() { Some(135.7681) } else { None },
        altitude: if city.is_some() { Some(50.0) } else { None },
        country_code: if city == Some("Paris") {
            Some("FR".to_string())
        } else if city.is_some() {
            Some("JP".to_string())
        } else {
            None
        },
        admin1_region: if city == Some("Paris") {
            Some("Île-de-France".to_string())
        } else if city.is_some() {
            Some("Kansai".to_string())
        } else {
            None
        },
        city: city.map(|s| s.to_string()),
        make: Some("Sony".to_string()),
        model: model.map(|s| s.to_string()),
        lens: Some("FE 35mm F1.4 GM".to_string()),
        focal_length: Some(35.0),
        f_number: Some(1.4),
        iso: Some(100),
        exposure_time: Some("1/500".to_string()),
        is_favorite: false,
        is_hidden: false,
        is_trashed: false,
    }
}

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
    assert_eq!(removed, 1);
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

    let matches_kyoto = catalog.search_fts("Kyoto", 10).unwrap();
    assert_eq!(matches_kyoto.len(), 1);
    assert_eq!(matches_kyoto[0].city.as_deref(), Some("Kyoto"));

    let matches_paris = catalog.search_fts("Paris", 10).unwrap();
    assert_eq!(matches_paris.len(), 1);
    assert_eq!(matches_paris[0].city.as_deref(), Some("Paris"));

    let matches_camera = catalog.search_fts("A7IV", 10).unwrap();
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

    let matches = catalog.search_fts("elephant", 10).unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].id, id);

    // Metadata columns remain searchable alongside the new caption column.
    let by_city = catalog.search_fts("Kyoto", 10).unwrap();
    assert_eq!(by_city.len(), 1);
    assert_eq!(by_city[0].id, id);

    // Re-scanning the same file fires the UPDATE trigger, which must keep the
    // caption terms in the contentless index.
    let mut rescanned = item;
    rescanned.mtime_epoch_ms = 2000;
    catalog.insert_or_update_batch(&[rescanned]).unwrap();
    assert_eq!(catalog.search_fts("elephant", 10).unwrap().len(), 1);
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
    assert_eq!(catalog.search_fts("walrus", 10).unwrap().len(), 1);

    catalog.set_caption(id, "crimson giraffe at dawn").unwrap();
    assert!(catalog.search_fts("walrus", 10).unwrap().is_empty());
    let matches = catalog.search_fts("giraffe", 10).unwrap();
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
    assert_eq!(catalog.search_fts("badger", 10).unwrap().len(), 1);

    catalog.set_caption(id, "   ").unwrap();
    assert!(catalog.search_fts("badger", 10).unwrap().is_empty());
    assert!(!catalog.get_captions_map().unwrap().contains_key(&id));

    // Clearing an already-empty caption stays a no-op.
    catalog.set_caption(id, "").unwrap();
    assert!(!catalog.get_captions_map().unwrap().contains_key(&id));
}

#[test]
fn test_fts5_caption_removed_when_media_deleted() {
    let tmp_dir = std::env::temp_dir().join(format!(
        "lumina_caption_delete_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&tmp_dir).unwrap();
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
    assert_eq!(catalog.search_fts("narwhal", 10).unwrap().len(), 1);

    let removed = catalog
        .permanently_delete_items(&[doomed_id], DeletionTarget::CatalogOnly)
        .unwrap();
    assert_eq!(removed, 1);

    // The deleted media row no longer surfaces through the FTS join.
    assert!(catalog.search_fts("narwhal", 10).unwrap().is_empty());
    // The FTS index remains usable for the surviving rows.
    assert_eq!(catalog.search_fts("panorama", 10).unwrap().len(), 1);
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
    let tmp_dir = std::env::temp_dir().join(format!(
        "lumina_caption_migration_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&tmp_dir).unwrap();
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
    let matches = catalog.search_fts("elephant", 10).unwrap();
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
    assert_eq!(catalog.search_fts("elephant", 10).unwrap().len(), 1);
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
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "lumina_test_cache_{}_{}",
        std::process::id(),
        nanos
    ));
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
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "lumina_test_scanner_{}_{}",
        std::process::id(),
        nanos
    ));
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
fn test_desktop_pics_integration() {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "lumina_test_engine_{}_{}",
        std::process::id(),
        nanos
    ));
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

    let mut total_discovered = 0;
    while let Ok(evt) = rx.recv_timeout(std::time::Duration::from_secs(5)) {
        if let lumina_core::EngineEvent::ScanComplete { total_items } = evt {
            total_discovered = total_items;
            break;
        }
    }

    assert_eq!(total_discovered, 3, "Should discover all 3 media items");

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
    while let Ok(evt) = rx.recv_timeout(std::time::Duration::from_secs(5)) {
        if let lumina_core::EngineEvent::ScanComplete { .. } = evt {
            break;
        }
    }

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

    // Verify all 9 system albums
    assert_eq!(
        catalog
            .query_system_album(SystemAlbum::Favorites, 10, 0)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        catalog
            .query_system_album(SystemAlbum::Videos, 10, 0)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        catalog
            .query_system_album(SystemAlbum::Raw, 10, 0)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        catalog
            .query_system_album(SystemAlbum::Screenshots, 10, 0)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        catalog
            .query_system_album(SystemAlbum::LongExposures, 10, 0)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        catalog
            .query_system_album(SystemAlbum::Panoramas, 10, 0)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        catalog
            .query_system_album(SystemAlbum::Bursts, 10, 0)
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        catalog
            .query_system_album(SystemAlbum::RecentlyDeleted, 10, 0)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        catalog
            .query_system_album(SystemAlbum::Places, 10, 0)
            .unwrap()
            .len(),
        1
    );

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

    catalog
        .insert_or_update_batch(&[item1, item2, item3])
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
fn test_batch_operations_and_keywords() {
    let catalog = Catalog::open_in_memory().expect("open catalog");

    let item1 = sample_media_item(
        "/photos/lake_tahoe.jpg",
        MediaType::Image,
        1728560000000,
        Some("Tahoe"),
        Some("A7IV"),
    );
    let item2 = sample_media_item(
        "/photos/yosemite_valley.jpg",
        MediaType::Image,
        1728570000000,
        Some("Yosemite"),
        Some("EOS R5"),
    );
    let item3 = sample_media_item(
        "/photos/zion_canyon.jpg",
        MediaType::Image,
        1728580000000,
        Some("Zion"),
        Some("Z8"),
    );

    let inserted = catalog
        .insert_or_update_batch(&[item1, item2, item3])
        .unwrap();
    let ids: Vec<MediaId> = inserted.iter().map(|it| it.id).collect();
    assert_eq!(ids.len(), 3);

    // Test batch favorite
    catalog.batch_set_favorite(&ids[0..2], true).unwrap();
    let it1 = catalog.get_item_by_id(ids[0]).unwrap().unwrap();
    let it2 = catalog.get_item_by_id(ids[1]).unwrap().unwrap();
    let it3 = catalog.get_item_by_id(ids[2]).unwrap().unwrap();
    assert!(it1.is_favorite);
    assert!(it2.is_favorite);
    assert!(!it3.is_favorite);

    // Test batch tags
    let tags = vec![
        "nature".to_string(),
        "travel".to_string(),
        "vacation".to_string(),
    ];
    catalog.add_tags_batch(&ids, &tags).unwrap();

    let all_tags = catalog.get_all_tags().unwrap();
    assert_eq!(all_tags, vec!["nature", "travel", "vacation"]);

    let tags_for_1 = catalog.get_tags_for_item(ids[0]).unwrap();
    assert_eq!(tags_for_1.len(), 3);

    // Query by tag
    let nature_items = catalog.get_items_by_tag("nature", 10, 0).unwrap();
    assert_eq!(nature_items.len(), 3);

    // Structured search with tag
    let q = StructuredQuery::parse("tag:nature");
    let search_results = catalog.search_structured(&q, 10, 0).unwrap();
    assert_eq!(search_results.len(), 3);

    // Remove a tag from item 1
    catalog
        .remove_tags_batch(&[ids[0]], &["vacation".to_string()])
        .unwrap();
    let tags_for_1_after = catalog.get_tags_for_item(ids[0]).unwrap();
    assert_eq!(tags_for_1_after.len(), 2);
    assert!(!tags_for_1_after.contains(&"vacation".to_string()));

    // Test query_tag_counts
    let tag_counts = catalog.query_tag_counts().unwrap();
    assert_eq!(
        tag_counts,
        vec![
            ("nature".to_string(), 3),
            ("travel".to_string(), 3),
            ("vacation".to_string(), 2)
        ]
    );

    // Test Tag in Smart Album
    let tag_rule = SmartAlbumRule {
        field: RuleField::Tag,
        operator: RuleOperator::Equals,
        value: RuleValue::Text("Nature".to_string()),
    };
    let tag_album = catalog
        .create_smart_album("Nature Album", "tag", MatchMode::All, &[tag_rule])
        .unwrap();
    let tag_album_matches = catalog.query_smart_album(&tag_album, 10, 0).unwrap();
    assert_eq!(tag_album_matches.len(), 3);

    // Test batch trash
    catalog.batch_set_trashed(&ids[1..3], true).unwrap();
    let active_slice = catalog.get_item_slice(0, 10).unwrap();
    assert_eq!(active_slice.len(), 1);

    // Tag counts after trashing items 1 and 2: only item 0 remains active (nature, travel)
    let tag_counts_after = catalog.query_tag_counts().unwrap();
    assert_eq!(
        tag_counts_after,
        vec![("nature".to_string(), 1), ("travel".to_string(), 1)]
    );

    let tag_album_matches_after = catalog.query_smart_album(&tag_album, 10, 0).unwrap();
    assert_eq!(tag_album_matches_after.len(), 1);
    assert_eq!(active_slice[0].id, ids[0]);
}

#[test]
fn test_tag_management_lifecycle() {
    let catalog = Catalog::open_in_memory().expect("open catalog");

    let item1 = sample_media_item("/photos/img1.jpg", MediaType::Image, 1000, None, None);
    let item2 = sample_media_item("/photos/img2.jpg", MediaType::Image, 2000, None, None);
    let item3 = sample_media_item("/photos/img3.jpg", MediaType::Image, 3000, None, None);

    let inserted = catalog
        .insert_or_update_batch(&[item1, item2, item3])
        .unwrap();
    let ids: Vec<MediaId> = inserted.iter().map(|it| it.id).collect();

    // 1. Tag items: item1 & item2 get "nature", item2 & item3 get "travel", item3 gets "vacation"
    catalog
        .add_tags_batch(&[ids[0], ids[1]], &["nature".to_string()])
        .unwrap();
    catalog
        .add_tags_batch(&[ids[1], ids[2]], &["travel".to_string()])
        .unwrap();
    catalog
        .add_tags_batch(&[ids[2]], &["vacation".to_string()])
        .unwrap();

    let counts = catalog.query_tag_counts().unwrap();
    assert_eq!(
        counts,
        vec![
            ("nature".to_string(), 2),
            ("travel".to_string(), 2),
            ("vacation".to_string(), 1),
        ]
    );

    // 2. Create smart album targeting tag:nature
    let tag_rule = SmartAlbumRule {
        field: RuleField::Tag,
        operator: RuleOperator::Equals,
        value: RuleValue::Text("nature".to_string()),
    };
    let album = catalog
        .create_smart_album("Nature Shots", "leaf", MatchMode::All, &[tag_rule])
        .unwrap();
    assert_eq!(catalog.query_smart_album_count(&album).unwrap(), 2);

    // 3. Rename "nature" -> "landscape"
    let affected = catalog.rename_tag("nature", "landscape").unwrap();
    assert_eq!(affected, 2);

    // Verify tag counts: "nature" gone, "landscape" exists
    let counts_renamed = catalog.query_tag_counts().unwrap();
    assert_eq!(
        counts_renamed,
        vec![
            ("landscape".to_string(), 2),
            ("travel".to_string(), 2),
            ("vacation".to_string(), 1),
        ]
    );
    assert_eq!(
        catalog.get_tags_for_item(ids[0]).unwrap(),
        vec!["landscape"]
    );

    // Verify Smart Album cascade: album rules now reference "landscape"
    let reloaded_album = catalog.get_smart_album(album.id).unwrap().unwrap();
    assert_eq!(
        reloaded_album.rules[0].value,
        RuleValue::Text("landscape".to_string())
    );
    assert_eq!(catalog.query_smart_album_count(&reloaded_album).unwrap(), 2);

    // 4. Merge "travel" into "landscape"
    let merged_affected = catalog.merge_tags("travel", "landscape").unwrap();
    assert_eq!(merged_affected, 2);

    // Item 1 had landscape. Item 2 had landscape + travel (now deduplicated landscape). Item 3 had travel (now landscape) + vacation.
    let counts_merged = catalog.query_tag_counts().unwrap();
    assert_eq!(
        counts_merged,
        vec![("landscape".to_string(), 3), ("vacation".to_string(), 1),]
    );
    assert_eq!(catalog.query_smart_album_count(&reloaded_album).unwrap(), 3);

    // 5. Delete "vacation" globally
    let deleted_count = catalog.delete_tag_globally("vacation").unwrap();
    assert_eq!(deleted_count, 1);
    let counts_final = catalog.query_tag_counts().unwrap();
    assert_eq!(counts_final, vec![("landscape".to_string(), 3)]);
    assert_eq!(
        catalog.get_tags_for_item(ids[2]).unwrap(),
        vec!["landscape"]
    );
}

#[test]
fn test_deletion_lifecycle_and_trash_restoration() {
    let catalog = Catalog::open_in_memory().expect("open catalog");

    let tmp_dir = std::env::temp_dir().join(format!(
        "lumina_del_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&tmp_dir).unwrap();

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
    assert_eq!(deleted_count, 1);
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
    assert_eq!(emptied.len(), 2);
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
    let tmp_dir = std::env::temp_dir().join(format!(
        "lumina_test_duration_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&tmp_dir).unwrap();
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
    assert_eq!(catalog.search_fts("hedgehog", 10).unwrap().len(), 1);

    catalog
        .clear_all()
        .expect("clear_all must handle contentless FTS5");

    assert_eq!(catalog.total_count().unwrap(), 0);
    assert!(catalog.search_fts("hedgehog", 10).unwrap().is_empty());
    assert!(catalog.get_captions_map().unwrap().is_empty());
    assert!(catalog.get_tags_for_item(id).unwrap().is_empty());

    // The FTS index must remain usable after delete-all.
    let item2 = sample_media_item("/photos/reset/two.jpg", MediaType::Image, 2000, None, None);
    let inserted2 = catalog.insert_or_update_batch(&[item2]).unwrap();
    catalog
        .set_caption(inserted2[0].id, "second entry")
        .unwrap();
    assert!(catalog.search_fts("hedgehog", 10).unwrap().is_empty());
    assert_eq!(catalog.search_fts("second", 10).unwrap().len(), 1);
}

#[test]
fn test_catalog_schema_version_integrity_and_backup() {
    let tmp_dir = std::env::temp_dir().join(format!(
        "lumina_catalog_hardening_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&tmp_dir).unwrap();
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
fn test_edit_timestamp_rebuilds_buckets_and_location() {
    let tmp_dir = std::env::temp_dir().join(format!(
        "lumina_edit_meta_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&tmp_dir).unwrap();
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
        catalog.search_fts("Paris", 10).unwrap().len(),
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
fn test_import_sidecars_applies_title_caption_rating_and_tags() {
    let tmp_dir = std::env::temp_dir().join(format!(
        "lumina_import_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&tmp_dir).unwrap();
    let media_path = tmp_dir.join("import_me.jpg");
    std::fs::write(&media_path, b"test-jpeg-content").unwrap();

    let (engine, rx) = LuminaEngine::open_with_paths(
        tmp_dir.clone(),
        tmp_dir.join("catalog.db"),
        Some(tmp_dir.join("cache")),
    )
    .unwrap();
    while let Ok(event) = rx.recv_timeout(std::time::Duration::from_secs(10)) {
        if let EngineEvent::ScanComplete { .. } = event {
            break;
        }
    }
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
fn test_export_items_copies_files_and_writes_xmp_sidecars() {
    let tmp_dir = std::env::temp_dir().join(format!(
        "lumina_export_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let media_dir = tmp_dir.join("library");
    std::fs::create_dir_all(&media_dir).unwrap();
    for name in ["trip_01.jpg", "trip_02.jpg"] {
        std::fs::write(media_dir.join(name), b"test-jpeg-content").unwrap();
    }

    let (engine, rx) = LuminaEngine::open_with_paths(
        media_dir.clone(),
        tmp_dir.join("catalog.db"),
        Some(tmp_dir.join("cache")),
    )
    .unwrap();
    while let Ok(event) = rx.recv_timeout(std::time::Duration::from_secs(10)) {
        if let EngineEvent::ScanComplete { .. } = event {
            break;
        }
    }

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
    let report = engine.export_items(&ids, &destination).unwrap();
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
    let second = engine.export_items(&[rated.id], &destination).unwrap();
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
fn test_tag_counts_for_items_split_full_and_partial_tags() {
    let catalog = Catalog::open_in_memory().expect("open catalog");
    let items: Vec<_> = (0..3)
        .map(|i| {
            sample_media_item(
                &format!("/photos/count{i}.jpg"),
                MediaType::Image,
                1000 + i as i64,
                None,
                None,
            )
        })
        .collect();
    let ids: Vec<MediaId> = catalog
        .insert_or_update_batch(&items)
        .unwrap()
        .iter()
        .map(|it| it.id)
        .collect();
    catalog.add_tags_batch(&ids, &["trip".to_string()]).unwrap();
    catalog
        .add_tags_batch(&[ids[0]], &["street".to_string()])
        .unwrap();
    // A tag only on an unselected item must not appear.
    catalog
        .add_tags_batch(&[ids[2]], &["other".to_string()])
        .unwrap();

    assert_eq!(
        catalog.tag_counts_for_items(&ids[..2]).unwrap(),
        vec![("street".to_string(), 1), ("trip".to_string(), 2)],
        "counts must cover only the given items, ordered by tag"
    );
    assert!(catalog.tag_counts_for_items(&[]).unwrap().is_empty());
}

#[test]
fn test_ids_with_tag_returns_only_tagged_subset() {
    let catalog = Catalog::open_in_memory().expect("open catalog");
    let items: Vec<_> = (0..3)
        .map(|i| {
            sample_media_item(
                &format!("/photos/has{i}.jpg"),
                MediaType::Image,
                2000 + i as i64,
                None,
                None,
            )
        })
        .collect();
    let ids: Vec<MediaId> = catalog
        .insert_or_update_batch(&items)
        .unwrap()
        .iter()
        .map(|it| it.id)
        .collect();
    catalog
        .add_tags_batch(&[ids[0], ids[2]], &["street".to_string()])
        .unwrap();

    let mut found = catalog.ids_with_tag(&ids[..2], "street").unwrap();
    found.sort_by_key(|id| id.0);
    assert_eq!(
        found,
        vec![ids[0]],
        "only the queried ids that carry the tag are returned"
    );
    assert!(catalog.ids_with_tag(&ids, "missing").unwrap().is_empty());
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

#[test]
fn test_added_date_is_stamped_once_and_survives_rescans() {
    let catalog = Catalog::open_in_memory().unwrap();
    let item = sample_media_item("/photos/added/a.jpg", MediaType::Image, 1000, None, None);
    let before = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
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

    std::thread::sleep(std::time::Duration::from_millis(20));
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
    let tmp_dir = std::env::temp_dir().join(format!(
        "lumina_added_migration_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&tmp_dir).unwrap();
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

    let tmp_dir = std::env::temp_dir().join(format!(
        "lumina_details_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&tmp_dir).unwrap();

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
    let dir = std::env::temp_dir().join(format!(
        "lumina_test_vsize_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
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

/// A one-photo library, scanned, with its catalog path for breaking it.
fn scanned_library(
    prefix: &str,
) -> (
    PathBuf,
    PathBuf,
    std::sync::Arc<LuminaEngine>,
    crossbeam_channel::Receiver<EngineEvent>,
) {
    let tmp_dir = std::env::temp_dir().join(format!(
        "lumina_{prefix}_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let media_dir = tmp_dir.join("library");
    std::fs::create_dir_all(&media_dir).unwrap();
    std::fs::write(media_dir.join("one.jpg"), b"test-jpeg-content").unwrap();
    let db_path = tmp_dir.join("catalog.db");
    let (engine, rx) =
        LuminaEngine::open_with_paths(media_dir, db_path.clone(), Some(tmp_dir.join("cache")))
            .unwrap();
    let scanned = std::iter::from_fn(|| rx.recv_timeout(std::time::Duration::from_secs(10)).ok())
        .any(|event| matches!(event, EngineEvent::ScanComplete { .. }));
    assert!(scanned, "the initial scan must finish");
    (tmp_dir, db_path, engine, rx)
}

/// A second connection to break the catalog; waits out a watcher write.
fn raw_catalog(db_path: &std::path::Path) -> rusqlite::Connection {
    let conn = rusqlite::Connection::open(db_path).unwrap();
    conn.busy_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    conn
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
    assert_eq!(engine.write_metadata_sidecars(&[item.id]).unwrap(), 1);

    raw_catalog(&db_path)
        .execute_batch("DROP TABLE media_captions;")
        .unwrap();
    assert!(
        engine.write_metadata_sidecars(&[item.id]).is_err(),
        "an unreadable caption table must fail the write, not write no caption"
    );
    assert!(engine
        .export_items(&[item.id], &tmp_dir.join("exported"))
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
    engine.set_color_label(item.id, 2).unwrap();
    assert_eq!(
        engine.write_metadata_sidecars(&[item.id]).unwrap(),
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
    let tmp_dir = std::env::temp_dir().join(format!(
        "lumina_edited_migration_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&tmp_dir).unwrap();
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
fn a_batch_that_fails_partway_changes_nothing() {
    let (tmp_dir, db_path, engine, rx) = scanned_library("batch_abort");
    std::fs::write(
        tmp_dir.join("library").join("two.jpg"),
        b"test-jpeg-content",
    )
    .unwrap();
    engine.rescan();
    let scanned = std::iter::from_fn(|| rx.recv_timeout(std::time::Duration::from_secs(10)).ok())
        .any(|event| matches!(event, EngineEvent::ScanComplete { .. }));
    assert!(scanned && engine.total_count() == 2, "both photos indexed");
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
fn test_duplicate_detection_and_merging() {
    let tmp_dir = std::env::temp_dir().join(format!(
        "lumina_dup_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
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

    // Test import duplicate checking
    // 1. Single file check
    let external_dup = tmp_dir.join("external_dup.jpg");
    std::fs::write(&external_dup, &content_a).unwrap();
    let match_result = engine.check_file_is_duplicate(&external_dup).unwrap();
    assert!(
        match_result.is_some(),
        "external file with content A matched existing library item"
    );

    let external_unique = tmp_dir.join("external_unique.jpg");
    std::fs::write(&external_unique, b"totally unique external photo bytes").unwrap();
    let no_match = engine.check_file_is_duplicate(&external_unique).unwrap();
    assert!(
        no_match.is_none(),
        "unique external file is not a duplicate"
    );

    // 2. Batch import pre-check
    let candidates = engine
        .check_import_duplicates(
            &[external_dup.clone(), external_unique.clone()],
            &std::sync::atomic::AtomicBool::new(false),
        )
        .unwrap();
    assert_eq!(candidates.len(), 2);
    let cand_dup = candidates.iter().find(|c| c.path == external_dup).unwrap();
    assert!(cand_dup.is_duplicate);
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
fn test_network_import_source_attribution_and_cascade() {
    let (tmp_dir, _db_path, engine, _rx) = scanned_library("net_import_attr");

    let item = engine.get_item_slice(0..1).remove(0);

    let source_url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";
    let uploader = "Rick Astley";

    // Record source attribution
    engine
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
