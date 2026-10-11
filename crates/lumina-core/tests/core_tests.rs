use lumina_core::{
    Catalog, DecodedFrame, DiskCache, MatchMode, MediaId, MediaItem, MediaType, OfflineGeocoder,
    PixelFormat, RuleField, RuleOperator, RuleValue, Scanner, SmartAlbumRule, StructuredQuery,
    SystemAlbum,
};
use std::collections::HashSet;
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
    let parent_dir = path.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    let file_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();

    MediaItem {
        id: MediaId(0),
        path,
        parent_dir,
        file_name,
        file_size: 2048,
        mtime_epoch_ms: ts,
        media_type,
        width: 1920,
        height: 1080,
        duration_ms: if media_type == MediaType::Video { Some(12500) } else { None },
        duration_secs: if media_type == MediaType::Video { 12.5 } else { 0.0 },
        timestamp_epoch_ms: ts,
        year: 2026,
        month: 10,
        day: 10,
        latitude: if city.is_some() { Some(35.0116) } else { None },
        longitude: if city.is_some() { Some(135.7681) } else { None },
        altitude: if city.is_some() { Some(50.0) } else { None },
        country_code: if city == Some("Paris") { Some("FR".to_string()) } else if city.is_some() { Some("JP".to_string()) } else { None },
        admin1_region: if city == Some("Paris") { Some("Île-de-France".to_string()) } else if city.is_some() { Some("Kansai".to_string()) } else { None },
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

    let item1 = sample_media_item("/test/photo1.jpg", MediaType::Image, 1000, Some("Kyoto"), Some("ILCE-7M4"));
    let item2 = sample_media_item("/test/video1.mp4", MediaType::Video, 2000, Some("Kyoto"), Some("ILCE-7M4"));

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
    catalog.set_config("last_library", "/path/to/library").unwrap();
    assert_eq!(
        catalog.get_config("last_library").unwrap(),
        Some("/path/to/library".to_string())
    );

    // Test removing items
    let mut valid = HashSet::new();
    valid.insert(item2.path.clone());
    let removed = catalog.remove_items_not_in(&valid).unwrap();
    assert_eq!(removed.len(), 1);
    assert_eq!(catalog.total_count().unwrap(), 1);
}

#[test]
fn test_timeline_buckets_aggregation() {
    let catalog = Catalog::open_in_memory().expect("open catalog");

    let mut item1 = sample_media_item("/test/2026/oct/img1.jpg", MediaType::Image, 1791633600000, None, None);
    item1.year = 2026;
    item1.month = 10;
    item1.day = 10;

    let mut item2 = sample_media_item("/test/2026/oct/img2.jpg", MediaType::Image, 1791633700000, None, None);
    item2.year = 2026;
    item2.month = 10;
    item2.day = 10;

    let mut item3 = sample_media_item("/test/2025/dec/img3.jpg", MediaType::Image, 1765800000000, None, None);
    item3.year = 2025;
    item3.month = 12;
    item3.day = 15;

    catalog.insert_or_update_batch(&[item1, item2, item3]).unwrap();
    catalog.compute_timeline_buckets().unwrap();

    let year_buckets = catalog.get_timeline_buckets("year").unwrap();
    assert_eq!(year_buckets.len(), 2);
    assert_eq!(year_buckets[0].bucket_key, "2026");
    assert_eq!(year_buckets[0].item_count, 2);
    assert_eq!(year_buckets[1].bucket_key, "2025");
    assert_eq!(year_buckets[1].item_count, 1);

    let month_buckets = catalog.get_timeline_buckets("month").unwrap();
    assert_eq!(month_buckets.len(), 2);
    assert_eq!(month_buckets[0].bucket_key, "2026-10");
    assert_eq!(month_buckets[0].item_count, 2);
    assert_eq!(month_buckets[1].bucket_key, "2025-12");
    assert_eq!(month_buckets[1].item_count, 1);

    let day_buckets = catalog.get_timeline_buckets("day").unwrap();
    assert_eq!(day_buckets.len(), 2);
    assert_eq!(day_buckets[0].bucket_key, "2026-10-10");
    assert_eq!(day_buckets[0].item_count, 2);
}

#[test]
fn test_fts5_search() {
    let catalog = Catalog::open_in_memory().expect("open catalog");

    let item1 = sample_media_item("/photos/vacation/sunset_kyoto.jpg", MediaType::Image, 1000, Some("Kyoto"), Some("A7IV"));
    let item2 = sample_media_item("/photos/paris/eiffel_tower.jpg", MediaType::Image, 2000, Some("Paris"), Some("EOS R5"));

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
fn test_offline_geocoder_nearest_lookup() {
    let geocoder = OfflineGeocoder::new();

    // Tokyo: 35.6895, 139.6917
    let tokyo = geocoder.reverse_geocode(35.6895, 139.6917).expect("geocode tokyo");
    assert_eq!(tokyo.city, "Tokyo");
    assert_eq!(tokyo.country_code, "JP");

    // Paris: 48.8566, 2.3522
    let paris = geocoder.reverse_geocode(48.8566, 2.3522).expect("geocode paris");
    assert_eq!(paris.city, "Paris");
    assert_eq!(paris.country_code, "FR");

    // San Francisco: 37.7749, -122.4194
    let sf = geocoder.reverse_geocode(37.7749, -122.4194).expect("geocode sf");
    assert_eq!(sf.city, "San Francisco");
    assert_eq!(sf.country_code, "US");

    // Microsecond latency benchmark test: 1,000 lookups
    let start = Instant::now();
    for _ in 0..1000 {
        let _ = geocoder.reverse_geocode(35.0116, 135.7681); // Kyoto
    }
    let elapsed = start.elapsed();
    let per_lookup_micros = elapsed.as_micros() as f64 / 1000.0;
    println!("Reverse geocode latency: {:.2} µs/lookup", per_lookup_micros);
    assert!(per_lookup_micros < 4.0, "Reverse geocoding must resolve in < 4µs");
}

#[test]
fn test_disk_cache() {
    let temp_dir = std::env::temp_dir().join("lumina_test_cache");
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
}

#[test]
fn test_scanner_on_temp_dir() {
    let temp_dir = std::env::temp_dir().join("lumina_test_scanner");
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
    while let Ok(evt) = rx.try_recv() {
        if let lumina_core::EngineEvent::BatchDiscovered { items } = evt {
            batch_count += items.len();
        }
    }
    assert_eq!(batch_count, 2);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_desktop_pics_integration() {
    let desktop_pics = PathBuf::from("/Users/raymondottun/Desktop/pics");
    if !desktop_pics.exists() {
        return;
    }

    let (engine, rx) = lumina_core::LuminaEngine::open(desktop_pics).expect("open engine on desktop pics");

    let mut total_discovered = 0;
    while let Ok(evt) = rx.recv_timeout(std::time::Duration::from_secs(5)) {
        match evt {
            lumina_core::EngineEvent::ScanComplete { total_items } => {
                total_discovered = total_items;
                break;
            }
            _ => {}
        }
    }

    assert!(total_discovered > 0, "Should discover items in Desktop/pics");

    let slice = engine.get_item_slice(0..10);
    assert!(!slice.is_empty());

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

    // Test thumbnail decoding
    let first = &slice[0];
    let (tx_thumb, rx_thumb) = crossbeam_channel::bounded(1);
    engine.request_thumbnail(first.id, 256, move |frame| {
        let _ = tx_thumb.send(frame);
    });

    let frame_opt = rx_thumb.recv_timeout(std::time::Duration::from_secs(5)).expect("receive thumbnail");
    assert!(frame_opt.is_some(), "Thumbnail decode succeeded");
    let frame = frame_opt.unwrap();
    assert!(frame.width > 0 && frame.height > 0);
}

#[test]
fn test_structured_search_engine() {
    let catalog = Catalog::open_in_memory().expect("open catalog");

    let mut item1 = sample_media_item("/photos/sunset_kyoto.jpg", MediaType::Image, 1789430400000, Some("Kyoto"), Some("ILCE-7M4"));
    item1.make = Some("Sony".to_string());
    item1.model = Some("Sony A7IV".to_string());
    item1.lens = Some("FE 35mm F1.4 GM".to_string());
    item1.iso = Some(800);
    item1.f_number = Some(2.8);
    item1.is_favorite = true;
    item1.year = 2026;
    item1.month = 9;
    item1.day = 15;

    let mut item2 = sample_media_item("/photos/shibuya_night.jpg", MediaType::Image, 1786320000000, Some("Tokyo"), Some("EOS R5"));
    item2.make = Some("Canon".to_string());
    item2.model = Some("Canon EOS R5".to_string());
    item2.lens = Some("RF 50mm F1.2L".to_string());
    item2.iso = Some(1600);
    item2.f_number = Some(1.2);
    item2.is_favorite = false;
    item2.year = 2026;
    item2.month = 8;
    item2.day = 10;

    let mut item3 = sample_media_item("/photos/eiffel_tower.jpg", MediaType::Image, 1746057600000, Some("Paris"), Some("Z8"));
    item3.make = Some("Nikon".to_string());
    item3.model = Some("Nikon Z8".to_string());
    item3.iso = Some(200);
    item3.f_number = Some(1.8);
    item3.is_favorite = true;
    item3.year = 2025;
    item3.month = 5;
    item3.day = 1;

    let mut item4 = sample_media_item("/videos/kyoto_temple.mp4", MediaType::Video, 1789862400000, Some("Kyoto"), Some("FX3"));
    item4.make = Some("Sony".to_string());
    item4.model = Some("Sony FX3".to_string());
    item4.iso = Some(3200);
    item4.is_favorite = false;
    item4.year = 2026;
    item4.month = 9;
    item4.day = 20;

    let mut item5 = sample_media_item("/raw/arashiyama.arw", MediaType::Raw, 1790035200000, Some("Kyoto"), Some("Sony A7IV"));
    item5.make = Some("Sony".to_string());
    item5.model = Some("Sony A7IV".to_string());
    item5.iso = Some(100);
    item5.is_favorite = false;
    item5.year = 2026;
    item5.month = 9;
    item5.day = 22;

    catalog.insert_or_update_batch(&[item1, item2, item3, item4, item5]).expect("insert items");

    // 1. Search camera:Sony
    let q_sony = StructuredQuery::parse("camera:Sony");
    let sony_res = catalog.search_structured(&q_sony, 10, 0).expect("search sony");
    assert_eq!(sony_res.len(), 3);

    // 2. Search camera:Sony is:favorite
    let q_sony_fav = StructuredQuery::parse("camera:Sony is:favorite");
    let sony_fav_res = catalog.search_structured(&q_sony_fav, 10, 0).expect("search sony fav");
    assert_eq!(sony_fav_res.len(), 1);
    assert_eq!(sony_fav_res[0].file_name, "sunset_kyoto.jpg");

    // 3. Search city:Kyoto is:video
    let q_kyoto_vid = StructuredQuery::parse("city:Kyoto is:video");
    let kyoto_vid_res = catalog.search_structured(&q_kyoto_vid, 10, 0).expect("search kyoto video");
    assert_eq!(kyoto_vid_res.len(), 1);
    assert_eq!(kyoto_vid_res[0].file_name, "kyoto_temple.mp4");

    // 4. Search iso:>1000
    let q_iso = StructuredQuery::parse("iso:>1000");
    let iso_res = catalog.search_structured(&q_iso, 10, 0).expect("search iso");
    assert_eq!(iso_res.len(), 2);

    // 5. Search year:2026 month:9
    let q_sep = StructuredQuery::parse("year:2026 month:september");
    let sep_res = catalog.search_structured(&q_sep, 10, 0).expect("search sep");
    assert_eq!(sep_res.len(), 3);

    // 6. Search is:raw
    let q_raw = StructuredQuery::parse("is:raw");
    let raw_res = catalog.search_structured(&q_raw, 10, 0).expect("search raw");
    assert_eq!(raw_res.len(), 1);
    assert_eq!(raw_res[0].file_name, "arashiyama.arw");

    // 7. Search date range
    let q_date = StructuredQuery::parse("date:2026-09-01..2026-09-30");
    let date_res = catalog.search_structured(&q_date, 10, 0).expect("search date range");
    assert_eq!(date_res.len(), 3);

    // 8. FTS Keyword + Structured
    let q_comb = StructuredQuery::parse("sunset camera:Sony");
    let comb_res = catalog.search_structured(&q_comb, 10, 0).expect("search comb");
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
    assert!(per_query_ms < 5.0, "Structured search must execute in < 5ms");
}

#[test]
fn test_system_smart_albums() {
    let catalog = Catalog::open_in_memory().expect("open catalog");

    let mut item_fav = sample_media_item("/photos/fav.jpg", MediaType::Image, 1000, None, Some("A7IV"));
    item_fav.is_favorite = true;

    let item_vid = sample_media_item("/photos/clip.mp4", MediaType::Video, 2000, None, Some("FX3"));

    let item_raw = sample_media_item("/photos/raw.cr3", MediaType::Raw, 3000, None, Some("R5"));

    let mut item_shot = sample_media_item("/photos/Screenshot_2026.png", MediaType::Image, 4000, None, None);
    item_shot.width = 2560;
    item_shot.height = 1440;
    item_shot.lens = None;
    item_shot.focal_length = None;
    item_shot.iso = None;
    item_shot.make = None;

    let mut item_long = sample_media_item("/photos/waterfall.jpg", MediaType::Image, 5000, None, Some("A7IV"));
    item_long.exposure_time = Some("2.5s".to_string());

    let mut item_pano = sample_media_item("/photos/horizon.jpg", MediaType::Image, 6000, None, Some("A7IV"));
    item_pano.width = 3000;
    item_pano.height = 1000;

    // Bursts: 2 photos within 200ms from same camera model
    let item_burst1 = sample_media_item("/photos/burst1.jpg", MediaType::Image, 7000, None, Some("ILCE-1"));
    let item_burst2 = sample_media_item("/photos/burst2.jpg", MediaType::Image, 7200, None, Some("ILCE-1"));

    let mut item_trash = sample_media_item("/photos/deleted.jpg", MediaType::Image, 8000, None, None);
    item_trash.is_trashed = true;

    let mut item_places = sample_media_item("/photos/gps.jpg", MediaType::Image, 9000, None, None);
    item_places.latitude = Some(35.6895);
    item_places.longitude = Some(139.6917);

    catalog.insert_or_update_batch(&[
        item_fav, item_vid, item_raw, item_shot, item_long, item_pano,
        item_burst1, item_burst2, item_trash, item_places,
    ]).expect("insert batch");

    // Verify all 9 system albums
    assert_eq!(catalog.query_system_album(SystemAlbum::Favorites, 10, 0).unwrap().len(), 1);
    assert_eq!(catalog.query_system_album(SystemAlbum::Videos, 10, 0).unwrap().len(), 1);
    assert_eq!(catalog.query_system_album(SystemAlbum::Raw, 10, 0).unwrap().len(), 1);
    assert_eq!(catalog.query_system_album(SystemAlbum::Screenshots, 10, 0).unwrap().len(), 1);
    assert_eq!(catalog.query_system_album(SystemAlbum::LongExposures, 10, 0).unwrap().len(), 1);
    assert_eq!(catalog.query_system_album(SystemAlbum::Panoramas, 10, 0).unwrap().len(), 1);
    assert_eq!(catalog.query_system_album(SystemAlbum::Bursts, 10, 0).unwrap().len(), 2);
    assert_eq!(catalog.query_system_album(SystemAlbum::RecentlyDeleted, 10, 0).unwrap().len(), 1);
    assert_eq!(catalog.query_system_album(SystemAlbum::Places, 10, 0).unwrap().len(), 1);

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
    let per_album_ms = elapsed.as_secs_f64() * 1000.0 / 9.0;
    println!("System smart album evaluation latency: {:.3} ms/album", per_album_ms);
    assert!(per_album_ms < 8.0, "System smart album evaluation must be < 8ms");
}

#[test]
fn test_user_defined_smart_albums_json_engine() {
    let catalog = Catalog::open_in_memory().expect("open catalog");

    let mut item1 = sample_media_item("/photos/sony_fav.jpg", MediaType::Image, 1000, Some("Kyoto"), Some("ILCE-7M4"));
    item1.make = Some("Sony".to_string());
    item1.is_favorite = true;
    item1.iso = Some(1600);

    let mut item2 = sample_media_item("/photos/sony_regular.jpg", MediaType::Image, 2000, Some("Tokyo"), Some("ILCE-7M4"));
    item2.make = Some("Sony".to_string());
    item2.is_favorite = false;
    item2.iso = Some(400);

    let mut item3 = sample_media_item("/photos/canon_fav.jpg", MediaType::Image, 3000, Some("Paris"), Some("EOS R5"));
    item3.make = Some("Canon".to_string());
    item3.is_favorite = true;
    item3.iso = Some(1600);

    catalog.insert_or_update_batch(&[item1, item2, item3]).unwrap();

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

    let created = catalog.create_smart_album(
        "High ISO Sony Favorites",
        "heart",
        MatchMode::All,
        &rules,
    ).expect("create smart album");

    assert!(created.id > 0);
    assert_eq!(created.title, "High ISO Sony Favorites");
    assert_eq!(created.rules.len(), 3);

    // List smart albums
    let albums = catalog.list_smart_albums().expect("list smart albums");
    assert_eq!(albums.len(), 1);
    assert_eq!(albums[0].title, "High ISO Sony Favorites");

    // Execute smart album query
    let results = catalog.query_smart_album(&albums[0], 10, 0).expect("query smart album");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].file_name, "sony_fav.jpg");

    let count = catalog.query_smart_album_count(&albums[0]).expect("count smart album");
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

    let any_album = catalog.create_smart_album(
        "Canon or Kyoto",
        "folder",
        MatchMode::Any,
        &any_rules,
    ).unwrap();

    let any_results = catalog.query_smart_album(&any_album, 10, 0).unwrap();
    assert_eq!(any_results.len(), 2);

    // Benchmark multi-condition rule evaluation latency (< 8ms)
    let start = Instant::now();
    for _ in 0..100 {
        let _ = catalog.query_smart_album(&albums[0], 10, 0);
    }
    let elapsed = start.elapsed();
    let per_eval_ms = elapsed.as_secs_f64() * 1000.0 / 100.0;
    println!("Multi-condition smart album evaluation latency: {:.3} ms/eval", per_eval_ms);
    assert!(per_eval_ms < 8.0, "Rule evaluation must execute in < 8ms");

    // Delete smart album
    let deleted = catalog.delete_smart_album(created.id).unwrap();
    assert!(deleted);
    let remaining = catalog.list_smart_albums().unwrap();
    assert_eq!(remaining.len(), 1);
}
